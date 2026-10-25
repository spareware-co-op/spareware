//! Screenline suggestions from OpenStreetMap (`suggestScreenlines`, `joinWays`, `addSuggested`).
//!
//! Rivers, canals and main railways funnel traffic onto their crossings, so each makes a natural screenline. The
//! features come from the Overpass API (OpenStreetMap contributors, ODbL) for the network's bounding box; fetching them
//! is the caller's job ([`osm_bbox`] and [`osm_query`] say what to ask), and [`suggest_screenlines`] works on the reply.

use super::common::hash;
use super::criteria::tag;
use super::screen::sl_crossings;
use super::store::{CalibStore, Screenline};
use crate::edit::IdSource;
use crate::graph::js_sort_by;
use crate::jsnum::{JsNum, Maybe, js_trim, number_to_js_string, value_to_js_string, value_to_number, value_truthy};
use crate::project::{Point, Project, Scenario};
use crate::util::{LatLng, r7};
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// Where the suggestions come from, as written into an added screenline's `src`.
pub const OSM_SOURCE: &str = "OpenStreetMap contributors (Overpass API)";

/// The licence of the OpenStreetMap data.
pub const OSM_LICENCE: &str = "ODbL 1.0";

/// How long to wait for each Overpass server (ms).
pub const OSM_TIMEOUT_MS: f64 = 30000.0;

/// `OSM_SL.query(bb)`: the Overpass query for rivers, canals and main railways in a bounding box ("south,west,north,east" with six decimals).
pub fn osm_query(bb: &str) -> String {
    format!(
        "[out:json][timeout:50];(way[\"waterway\"~\"^(river|canal)$\"]({bb});way[\"railway\"=\"rail\"][!\"service\"]({bb}););out geom;"
    )
}

/// The network's bounding box as [south, west, north, east], or None with fewer than two nodes.
pub fn osm_bbox(sc: &Scenario) -> Option<[f64; 4]> {
    if sc.nodes.len() < 2 {
        return None;
    }
    let (mut s, mut w, mut n, mut e) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for nd in sc.nodes.values() {
        let (la, ln) = (nd.lat.num(), nd.lng.num());
        s = crate::jsnum::math_min(s, la);
        w = crate::jsnum::math_min(w, ln);
        n = crate::jsnum::math_max(n, la);
        e = crate::jsnum::math_max(e, ln);
    }
    Some([s, w, n, e])
}

/// A polyline made of ways that share end points, and the ids of those ways.
#[derive(Clone, Debug, PartialEq)]
pub struct JoinedWay {
    /// The points, [lat, lng].
    pub pts: Vec<LatLng>,
    /// The ids of the ways joined, in the order they were added.
    pub ids: Vec<Value>,
}

/// `ids.join(',')` for way ids (numbers as JavaScript writes them; null as nothing).
fn join_ids(ids: &[Value]) -> String {
    ids.iter()
        .map(|v| match v {
            Value::Null => String::new(),
            Value::Number(n) => number_to_js_string(n.as_f64().unwrap_or(f64::NAN)),
            Value::String(s) => s.clone(),
            other => value_to_js_string(other),
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn same(p: &LatLng, q: &LatLng) -> bool {
    (p[0] - q[0]).abs() < 1e-7 && (p[1] - q[1]).abs() < 1e-7
}

/// `joinWays(ways)`: joins ways (Overpass elements with `id` and `geometry` as [{lat, lon}]) that share an end point into polylines, longest first.
///
/// Ways with a point that is not a finite number are left out; a way with no points is left out too (the prototype stops with an error on one).
pub fn join_ways(ways: &[&Value]) -> Vec<JoinedWay> {
    let mut left: Vec<(Value, Vec<LatLng>)> = ways
        .iter()
        .map(|w| {
            let pts: Vec<LatLng> = w
                .get("geometry")
                .and_then(Value::as_array)
                .map(|g| g.iter().map(|p| [p.get("lat").map_or(f64::NAN, value_to_number), p.get("lon").map_or(f64::NAN, value_to_number)]).collect())
                .unwrap_or_default();
            (w.get("id").cloned().unwrap_or(Value::Null), pts)
        })
        .filter(|(_, pts)| !pts.is_empty() && pts.iter().all(|p| p[0].is_finite() && p[1].is_finite()))
        .collect();
    let mut out: Vec<JoinedWay> = Vec::new();
    while !left.is_empty() {
        let (id, mut pts) = left.remove(0);
        let mut ids = vec![id];
        let mut grew = true;
        while grew {
            grew = false;
            for i in 0..left.len() {
                let w = &left[i].1;
                let (a, b) = (pts[0], pts[pts.len() - 1]);
                let j: Option<Vec<LatLng>> = if same(&b, &w[0]) {
                    Some(pts.iter().copied().chain(w[1..].iter().copied()).collect())
                } else if same(&b, &w[w.len() - 1]) {
                    Some(pts.iter().copied().chain(w[..w.len() - 1].iter().rev().copied()).collect())
                } else if same(&a, &w[w.len() - 1]) {
                    Some(w.iter().copied().chain(pts[1..].iter().copied()).collect())
                } else if same(&a, &w[0]) {
                    Some(w.iter().rev().copied().chain(pts[1..].iter().copied()).collect())
                } else {
                    None
                };
                if let Some(j) = j {
                    pts = j;
                    ids.push(left[i].0.clone());
                    left.remove(i);
                    grew = true;
                    break;
                }
            }
        }
        out.push(JoinedWay { pts, ids });
    }
    js_sort_by(&mut out, |p, q| q.pts.len() as f64 - p.pts.len() as f64);
    out
}

/// One suggested screenline: a river, canal or railway crossing at least two roads of the network.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    /// Its name (the feature's, else "Unnamed river", "Unnamed canal" or "Railway").
    pub name: String,
    /// "river", "canal" or "railway".
    pub kind: String,
    /// The line, [lat, lng] to 7 decimals.
    pub pts: Vec<LatLng>,
    /// The OpenStreetMap way ids (the first 50).
    pub osm: Vec<Value>,
    /// Roads crossing it.
    pub crossings: usize,
    /// Fewer crossings than TAG M3.1 asks of a screenline.
    pub few: bool,
    /// A screenline from the same ways is already in the project.
    pub added: bool,
}

/// What [`suggest_screenlines`] gives: whether it could suggest, the suggestions (most crossings first) and a sentence.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggestions {
    /// Suggestions could be made.
    pub ok: bool,
    /// The suggestions.
    pub list: Vec<Suggestion>,
    /// Why none, when none.
    pub note: String,
}

/// The sentence for a failed fetch (`suggestScreenlines` when no server answered).
pub fn osm_failure_note(message: &str) -> String {
    format!(
        "Could not get rivers and railways from OpenStreetMap: {message}. Draw screenlines with the Screenline tool (S) instead, or try again later."
    )
}

fn text_of(v: Option<&Value>) -> String {
    match v {
        Some(x) if value_truthy(x) => value_to_js_string(x),
        _ => String::new(),
    }
}

/// `suggestScreenlines(sc)` on an Overpass reply: rivers, canals and main railways (no sidings, yards or spurs) that cross at least two roads, joined into lines, most crossings first.
pub fn suggest_screenlines(sc: &Scenario, store: &CalibStore, reply: &Value) -> Suggestions {
    if sc.nodes.len() < 2 {
        return Suggestions { ok: false, list: Vec::new(), note: "Import or draw roads first.".to_string() };
    }
    let mut keys: Vec<String> = Vec::new();
    let mut groups: Vec<Vec<&Value>> = Vec::new();
    if let Some(Value::Array(elements)) = reply.get("elements") {
        for e in elements {
            let geom_ok = matches!(e.get("geometry"), Some(Value::Array(g)) if g.len() >= 2);
            let tags = e.get("tags").filter(|t| value_truthy(t));
            if e.get("type").and_then(Value::as_str) != Some("way") || !geom_ok || tags.is_none() {
                continue;
            }
            let tags = tags.expect("checked");
            let waterway = text_of(tags.get("waterway"));
            let kind = if waterway == "river" || waterway == "canal" {
                Some(waterway)
            } else if tags.get("railway").and_then(Value::as_str) == Some("rail") && !tags.get("service").is_some_and(value_truthy) {
                Some("railway".to_string())
            } else {
                None
            };
            let Some(kind) = kind else { continue };
            let name = js_trim(&text_of(tags.get("name"))).to_string();
            let k = format!("{kind}\u{0}{name}");
            match keys.iter().position(|x| *x == k) {
                Some(i) => groups[i].push(e),
                None => {
                    keys.push(k);
                    groups.push(vec![e]);
                }
            }
        }
    }
    let have: HashSet<String> = store
        .screenlines
        .iter()
        .map(|s| match s.src.get().and_then(|x| x.get("osm")) {
            Some(Value::Array(a)) => join_ids(a),
            _ => String::new(),
        })
        .filter(|x| !x.is_empty())
        .collect();
    let mut list: Vec<Suggestion> = Vec::new();
    for (k, ws) in keys.iter().zip(&groups) {
        let mut parts = k.split('\u{0}');
        let (kind, name) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        for ch in join_ways(ws) {
            let sl = Screenline {
                id: format!("osm:{}", hash(&join_ids(&ch.ids))),
                name: Maybe::Missing,
                kind: Maybe::Set(JsNum::Text("screen".to_string())),
                role: Maybe::Missing,
                motorways: Maybe::Missing,
                pts: ch.pts.iter().map(|p| Point(*p)).collect(),
                src: Maybe::Missing,
                extra: Map::new(),
            };
            let links: HashSet<String> = sl_crossings(sc, &sl).into_iter().map(|x| x.link).collect();
            if links.len() < 2 {
                continue;
            }
            let nm = if name.is_empty() {
                match kind {
                    "river" => "Unnamed river",
                    "canal" => "Unnamed canal",
                    _ => "Railway",
                }
                .to_string()
            } else {
                name.to_string()
            };
            let osm: Vec<Value> = ch.ids.iter().take(50).cloned().collect();
            list.push(Suggestion {
                name: nm,
                kind: kind.to_string(),
                pts: ch.pts.iter().map(|p| [r7(p[0]), r7(p[1])]).collect(),
                crossings: links.len(),
                few: (links.len() as f64) < tag::SCREEN_MIN_LINKS,
                added: have.contains(&join_ids(&osm)),
                osm,
            });
        }
    }
    js_sort_by(&mut list, |a, b| b.crossings as f64 - a.crossings as f64);
    let note = if list.is_empty() { "No river, canal or railway crosses two or more roads in this network.".to_string() } else { String::new() };
    Suggestions { ok: true, list, note }
}

/// `addSuggested(item, role)`: adds a suggestion to the project's screenlines as a screenline (role "valid" or "me"), crediting OpenStreetMap, and marks it added. None for a suggestion with fewer than two points.
pub fn add_suggested(p: &mut Project, item: &mut Suggestion, role: &str, ids: &mut dyn IdSource) -> Option<Screenline> {
    if item.pts.len() < 2 {
        return None;
    }
    let name: String = String::from_utf16_lossy(&item.name.encode_utf16().take(80).collect::<Vec<u16>>());
    let sl = Screenline {
        id: ids.uid("sl"),
        name: Maybe::Set(JsNum::Text(name)),
        kind: Maybe::Set(JsNum::Text("screen".to_string())),
        role: Maybe::Set(JsNum::Text(role.to_string())),
        motorways: Maybe::Set(JsNum::Bool(true)),
        pts: item.pts.iter().map(|q| Point(*q)).collect(),
        src: Maybe::Set(json!({ "source": OSM_SOURCE, "licence": OSM_LICENCE, "basis": "derived", "date": ids.today(), "osm": item.osm, "feature": item.kind })),
        extra: Map::new(),
    };
    let mut st = CalibStore::read(p);
    st.screenlines.push(sl.clone());
    st.write(p);
    item.added = true;
    Some(sl)
}
