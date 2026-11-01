//! The small steps the sample build is made of: finding roads and junction sites by name, the made-up turning count, lane coding, cutting OpenStreetMap
//! roads at the edge of the study area, leaving out dead ends, simplifying road shapes and the two network options.

use super::hooks::SampleHooks;
use super::{BIN, ILLUSTRATIVE, PERIODS, turn_vol, window};
use crate::census::Bx;
use crate::datahub::{Data, matches};
use crate::edit::IdSource;
use crate::edit::net::{LinkProps, add_link, auto_control, delete_link, links_at, nearest_node};
use crate::graph::{connector_map, js_sort_by};
use crate::jsnum::{
    Maybe, Num, math_max, math_round, number_to_js_string, value_to_js_string, value_to_number,
};
use crate::project::{Point, Scenario, Settings};
use crate::util::{self, LatLng};
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// `names(sc, id)`: the names of the roads at a junction node, each once, joined with " | " (a road with no name is an empty name).
pub fn names(sc: &Scenario, id: &str) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for l in links_at(sc, id) {
        let nm = l.name.as_str().unwrap_or("");
        if !seen.contains(&nm) {
            seen.push(nm);
        }
    }
    seen.join(" | ")
}

// `new RegExp(word, 'i').test(text)` for a plain word (the sample only searches for road names)
fn has_word(text: &str, word: &str) -> bool {
    text.to_lowercase().contains(&word.to_lowercase())
}

/// `nodeOn(sc, ll, road, maxM)`: the junction node nearest to a point, within `max_m` metres, on a road whose name has `road` in it (a plain word, any case).
pub fn node_on(sc: &Scenario, ll: LatLng, road: &str, max_m: f64) -> Option<String> {
    let mut ids: HashSet<&str> = HashSet::new();
    for l in sc.links.values() {
        if has_word(l.name.as_str().unwrap_or(""), road) {
            ids.insert(&l.a);
            ids.insert(&l.b);
        }
    }
    nearest_node(
        sc,
        ll,
        max_m,
        Some(&|n: &crate::project::Node| ids.contains(n.id.as_str())),
    )
}

/// A junction site `site_with` found and where its middle is.
#[derive(Clone, Debug, PartialEq)]
pub struct SiteHit {
    /// The site's id.
    pub site: String,
    /// The middle of the site's nodes, `[lat, lng]`.
    pub c: LatLng,
}

/// `siteWith(sc, pats, ll)`: the junction site (`Sites.detect`) whose roads include every word, nearest to the point.
pub fn site_with(sc: &Scenario, pats: &[&str], ll: LatLng) -> Option<SiteHit> {
    let sites = sc.sites.get()?;
    let mut list: Vec<SiteHit> = Vec::new();
    for s in sites.values() {
        let t = s
            .nodes
            .iter()
            .map(|id| names(sc, id))
            .collect::<Vec<_>>()
            .join(" | ");
        if !pats.iter().all(|p| has_word(&t, p)) {
            continue;
        }
        let n: Vec<&crate::project::Node> =
            s.nodes.iter().filter_map(|id| sc.nodes.get(id)).collect();
        let (mut sa, mut so) = (0.0, 0.0);
        for x in &n {
            sa += x.lat.num();
        }
        for x in &n {
            so += x.lng.num();
        }
        list.push(SiteHit {
            site: s.id.clone(),
            c: [sa / n.len() as f64, so / n.len() as f64],
        });
    }
    js_sort_by(&mut list, |a, b| {
        util::dist(&a.c, &ll) - util::dist(&b.c, &ll)
    });
    list.into_iter().next()
}

/// `compass(b)`: the compass side N, E, S or W a bearing points to (the prototype's "undefined" for a bearing that is not a number).
pub fn compass(b: f64) -> &'static str {
    const COMPASS: [&str; 4] = ["N", "E", "S", "W"];
    let i = math_round((((b % 360.0) + 360.0) % 360.0) / 90.0) % 4.0;
    if i.is_nan() {
        "undefined"
    } else {
        COMPASS[i as usize]
    }
}

/// What `turn_count_csv` is asked for: the name to print (else the site's, else "junction") and the date of the count (default 2026-03-10).
#[derive(Clone, Debug, Default)]
pub struct CsvOpts {
    /// The junction's name.
    pub name: Option<String>,
    /// The count date.
    pub date: Option<String>,
}

/// An illustrative turning-count file.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnCsv {
    /// The long-format CSV.
    pub csv: String,
    /// The road leaving the site on each compass side, in the order they were met.
    pub legs: Vec<(String, String)>,
    /// The middle of the site, `[lat, lng]`.
    pub centre: LatLng,
    /// The count's name, with the label that it is illustrative.
    pub name: String,
}

/// `turnCountCSV(sc, site, opts)`: an illustrative long-format turning-count file for a junction site (calibration feature layout): legs from the site's roads by
/// compass side, 15-minute light and heavy counts for 07:00 to 09:00 and 16:00 to 18:00.
pub fn turn_count_csv(sc: &Scenario, site: &crate::project::Site, opts: &CsvOpts) -> TurnCsv {
    let ms: HashSet<&str> = site.nodes.iter().map(String::as_str).collect();
    let pts: Vec<&crate::project::Node> = site
        .nodes
        .iter()
        .filter_map(|id| sc.nodes.get(id))
        .collect();
    let (mut sa, mut so) = (0.0, 0.0);
    for n in &pts {
        sa += n.lat.num();
    }
    for n in &pts {
        so += n.lng.num();
    }
    let c: LatLng = [sa / pts.len() as f64, so / pts.len() as f64];
    // each road leaving the site, by the compass side it leaves on (the bearing of its first segment)
    let mut legs: Vec<(String, String)> = Vec::new();
    for l in sc.links.values() {
        let nm = l.name.as_str().unwrap_or("");
        if l.status.is("closed")
            || ms.contains(l.a.as_str()) == ms.contains(l.b.as_str())
            || nm.is_empty()
            || l.ring.truthy()
        {
            continue;
        }
        let g = sc.geom(l);
        let b = if ms.contains(l.a.as_str()) {
            util::bearing(&g[0], &g[1])
        } else {
            util::bearing(&g[g.len() - 1], &g[g.len() - 2])
        };
        let k = compass(b);
        if !legs.iter().any(|(x, _)| x == k) {
            legs.push((k.to_string(), nm.to_string()));
        }
    }
    let hv = 0.06;
    let shown = opts
        .name
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(site.name.as_str().filter(|s| !s.is_empty()))
        .unwrap_or("junction");
    let name = format!("{ILLUSTRATIVE}: {shown}");
    let date = opts
        .date
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("2026-03-10");
    let s = |x: &str| Value::String(x.to_string());
    let mut rows: Vec<Vec<Value>> = vec![
        [
            "site_id",
            "site_name",
            "lat",
            "lng",
            "date",
            "time_start",
            "interval_min",
            "from_leg",
            "from_road",
            "movement",
            "class",
            "count",
        ]
        .iter()
        .map(|h| s(h))
        .collect(),
    ];
    let (lat7, lng7) = (util::r7(c[0]), util::r7(c[1]));
    for p in PERIODS {
        for (i, bin) in BIN.iter().enumerate() {
            let t = window(p) + 15.0 * i as f64;
            let hh = format!("{:02}:{:02}", (t / 60.0).floor() as i64, (t % 60.0) as i64);
            for (leg, mv) in turn_vol(p) {
                let Some((_, road)) = legs.iter().find(|(k, _)| k == leg) else {
                    continue;
                };
                for (m, v) in mv {
                    let n = v * bin;
                    let h = math_round(n * hv);
                    let l = math_max(0.0, math_round(n) - h);
                    for (class, count) in [("LV", l), ("HV", h)] {
                        rows.push(vec![
                            s("SAMPLE-1"),
                            s(&name),
                            crate::jsnum::json_number(lat7),
                            crate::jsnum::json_number(lng7),
                            s(date),
                            s(&hh),
                            json!(15),
                            s(leg),
                            s(road),
                            s(m),
                            s(class),
                            crate::jsnum::json_number(count),
                        ]);
                    }
                }
            }
        }
    }
    TurnCsv {
        csv: util::csv(&rows),
        legs,
        centre: c,
        name,
    }
}

/// `codeLanes(sc, site, JN)`: illustrative lane coding on every approach into a junction site: a kerb lane for left and through and a right lane for through and
/// right; a one-lane road gets a 40 m flare for the right lane. Stored on the site (junction tools).
pub fn code_lanes(sc: &mut Scenario, site_id: &str, hooks: &mut dyn SampleHooks) {
    const SRC: &str = "illustrative (sample)";
    let Some(nodes) = sc
        .sites
        .get()
        .and_then(|m| m.get(site_id))
        .map(|s| s.nodes.clone())
    else {
        return;
    };
    let ms: HashSet<&str> = nodes.iter().map(String::as_str).collect();
    if let Some(site) = sc.sites.get_mut().and_then(|m| m.get_mut(site_id))
        && !site.lanes.is_set()
    {
        site.lanes = Maybe::Set(crate::project::OrderedMap::new());
    }
    for nid in &nodes {
        for ap in hooks.approaches(sc, nid) {
            let Some(l) = sc.links.get(&ap.link) else {
                continue;
            };
            if ms.contains(l.a.as_str()) && ms.contains(l.b.as_str()) {
                continue;
            }
            let site = sc
                .sites
                .get_mut()
                .and_then(|m| m.get_mut(site_id))
                .expect("the site");
            let lanes = site.lanes.get_mut().expect("lanes");
            // an empty record on the site first, so the lanes are stored on the junction site rather than the node
            if !lanes.get(&ap.link).is_some_and(crate::jsnum::value_truthy) {
                lanes.insert(ap.link.clone(), json!({ "lanes": [], "src": SRC }));
            }
            // `Junctions.editLanes`: the lanes of the record, then who set them (the user); the sample then says it is illustrative
            let mut cur = lanes.get(&ap.link).cloned().unwrap_or(Value::Null);
            if let Some(o) = cur.as_object_mut() {
                o.insert("lanes".into(), json!([{ "d": "LT", "len": 0 }, { "d": "TR", "len": if ap.gl >= 2.0 { 0 } else { 40 } }]));
                o.insert("src".into(), json!("user"));
                let mut by = o
                    .get("srcBy")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                by.insert("lanes".into(), json!("user"));
                by.insert("len".into(), json!("user"));
                o.insert("srcBy".into(), Value::Object(by));
                o.insert("src".into(), json!(SRC));
                o.insert("srcBy".into(), json!({ "lanes": SRC, "len": SRC }));
            }
            lanes.insert(ap.link.clone(), cur);
        }
    }
}

/// What `clip_ways` made.
#[derive(Clone, Debug, PartialEq)]
pub struct Clipped {
    /// The elements: everything that is not a way, the ways cut at the edge, then the new nodes on the edge.
    pub elements: Vec<Value>,
    /// How many ways were cut.
    pub cut: usize,
}

// A Map key for an OpenStreetMap id (a number or a string), the way `Map` tells them apart
fn id_key(v: &Value) -> String {
    match v {
        Value::Number(n) => format!("n{}", number_to_js_string(n.as_f64().unwrap_or(f64::NAN))),
        Value::String(s) => format!("s{s}"),
        other => format!("o{other}"),
    }
}

fn in_box(b: &Bx, lat: f64, lon: f64) -> bool {
    lat >= b.s && lat <= b.n && lon >= b.w && lon <= b.e
}

fn node_in_box(b: &Bx, o: Option<&Value>) -> bool {
    match o {
        Some(o) => {
            let g = |k: &str| o.get(k).map_or(f64::NAN, value_to_number);
            in_box(b, g("lat"), g("lon"))
        }
        None => false,
    }
}

/// `clipWays(elements, B)`: cuts OpenStreetMap ways (Overpass `out body; >; out body qt`) where they leave the box: each run of nodes inside becomes its own way,
/// ending at a new node on the edge.
pub fn clip_ways(elements: &[Value], b: &Bx) -> Clipped {
    let mut nodes: std::collections::HashMap<String, &Value> = std::collections::HashMap::new();
    for e in elements {
        if e.get("type").and_then(Value::as_str) == Some("node") {
            nodes.insert(id_key(e.get("id").unwrap_or(&Value::Null)), e);
        }
    }
    let mut out: Vec<Value> = elements
        .iter()
        .filter(|e| e.get("type").and_then(Value::as_str) != Some("way"))
        .cloned()
        .collect();
    let mut extra: Vec<Value> = Vec::new();
    let (mut cut, mut k) = (0usize, 0usize);
    let num = |o: &Value, key: &str| o.get(key).map_or(f64::NAN, value_to_number);
    // the point where p (inside) to q (outside) crosses the edge
    let mut edge = |p: &Value, q: &Value| -> Value {
        let (plat, plon, qlat, qlon) = (num(p, "lat"), num(p, "lon"), num(q, "lat"), num(q, "lon"));
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        for _ in 0..30 {
            let m = (lo + hi) / 2.0;
            if in_box(b, plat + (qlat - plat) * m, plon + (qlon - plon) * m) {
                lo = m;
            } else {
                hi = m;
            }
        }
        k += 1;
        let id = format!("edge{k}");
        let n = json!({ "type": "node", "id": id, "lat": crate::jsnum::json_number(util::r7(plat + (qlat - plat) * lo)), "lon": crate::jsnum::json_number(util::r7(plon + (qlon - plon) * lo)) });
        extra.push(n);
        Value::String(id)
    };
    for w in elements {
        if w.get("type").and_then(Value::as_str) != Some("way") {
            continue;
        }
        let ids: Vec<&Value> = w
            .get("nodes")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter(|id| nodes.contains_key(&id_key(id)))
                    .collect()
            })
            .unwrap_or_default();
        let ins: Vec<bool> = ids
            .iter()
            .map(|id| node_in_box(b, nodes.get(&id_key(id)).copied()))
            .collect();
        if ins.iter().all(|x| *x) {
            out.push(w.clone());
            continue;
        }
        cut += 1;
        let (mut i, mut r) = (0usize, 0usize);
        while i < ids.len() {
            if !ins[i] {
                i += 1;
                continue;
            }
            let mut j = i;
            while j + 1 < ids.len() && ins[j + 1] {
                j += 1;
            }
            let mut run: Vec<Value> = ids[i..=j].iter().map(|v| (*v).clone()).collect();
            if i > 0 {
                let e = edge(nodes[&id_key(ids[i])], nodes[&id_key(ids[i - 1])]);
                run.insert(0, e);
            }
            if j < ids.len() - 1 {
                let e = edge(nodes[&id_key(ids[j])], nodes[&id_key(ids[j + 1])]);
                run.push(e);
            }
            if run.len() > 1 {
                let mut piece = w.as_object().cloned().unwrap_or_default();
                let id = if r > 0 {
                    Value::String(format!(
                        "{}-{}",
                        value_to_js_string(w.get("id").unwrap_or(&Value::Null)),
                        r
                    ))
                } else {
                    w.get("id").cloned().unwrap_or(Value::Null)
                };
                piece.insert("id".into(), id);
                piece.insert("nodes".into(), Value::Array(run));
                out.push(Value::Object(piece));
            }
            r += 1;
            i = j + 1;
        }
    }
    out.extend(extra);
    Clipped { elements: out, cut }
}

/// `pruneStubs(sc)`: removes dead-end chains that cannot carry traffic: a road end (one link) that is not a zone connector or gateway node, not in a junction site,
/// not on a bus route, and whose link has no count matched to it. Repeats inward along the chain. Exact for the assignment (no path can use a dead end it does not
/// start or end in). Gives how many links were removed.
pub fn prune_stubs(sc: &mut Scenario, s: &Settings, data: &Data) -> usize {
    let mut keep_n: HashSet<String> = HashSet::new();
    let mut keep_l: HashSet<String> = HashSet::new();
    for ids in connector_map(sc, s).values() {
        for id in ids {
            keep_n.insert(id.clone());
        }
    }
    for z in sc.zones.values() {
        if let Some(n) = z.node.as_str().filter(|n| !n.is_empty()) {
            keep_n.insert(n.to_string());
        }
    }
    if let Some(sites) = sc.sites.get() {
        for st in sites.values() {
            for id in &st.nodes {
                keep_n.insert(id.clone());
            }
        }
    }
    for r in sc.routes.values() {
        for id in r.wp.get().map(Vec::as_slice).unwrap_or(&[]) {
            keep_n.insert(id.clone());
        }
    }
    for mt in matches(sc, s, data).values() {
        keep_l.insert(mt.link.clone());
    }
    let mut n = 0;
    let mut again = true;
    while again {
        again = false;
        // `Net.degree(sc)`: how many link ends meet at each node, in the order the nodes are first met
        let mut order: Vec<(String, usize)> = Vec::new();
        let mut at: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for l in sc.links.values() {
            for id in [&l.a, &l.b] {
                let i = *at.entry(id.clone()).or_insert_with(|| {
                    order.push((id.clone(), 0));
                    order.len() - 1
                });
                order[i].1 += 1;
            }
        }
        for (id, d) in order {
            if d != 1 || keep_n.contains(&id) {
                continue;
            }
            let first = links_at(sc, &id)
                .first()
                .map(|l| (l.id.clone(), l.status.is("existing")));
            match first {
                Some((lid, existing)) if existing && !keep_l.contains(&lid) => {
                    delete_link(sc, &lid);
                    n += 1;
                    again = true;
                }
                _ => {
                    keep_n.insert(id);
                }
            }
        }
    }
    n
}

/// `simplify(sc, tol)`: Douglas-Peucker simplification of each link's shape points to within `tol` metres (lengths change by well under 0.1%). Gives how many
/// shape points were dropped.
pub fn simplify(sc: &mut Scenario, tol: f64) -> usize {
    let mut dropped = 0usize;
    let ids: Vec<String> = sc.links.keys().map(str::to_string).collect();
    for id in ids {
        let l = &sc.links[id.as_str()];
        let g = sc.geom(l);
        if g.len() < 3 {
            continue;
        }
        let f = util::flat(g[0][0]);
        let xy: Vec<[f64; 2]> = g.iter().map(|p| [f.x(p), f.y(p)]).collect();
        let mut keep = vec![false; g.len()];
        keep[0] = true;
        keep[g.len() - 1] = true;
        let mut st: Vec<(usize, usize)> = vec![(0, g.len() - 1)];
        while let Some((i, j)) = st.pop() {
            let mut k: Option<usize> = None;
            let mut dm = tol;
            for m in i + 1..j {
                let d =
                    util::seg_dist(xy[m][0], xy[m][1], xy[i][0], xy[i][1], xy[j][0], xy[j][1]).d;
                if d > dm {
                    dm = d;
                    k = Some(m);
                }
            }
            if let Some(k) = k {
                keep[k] = true;
                st.push((i, k));
                st.push((k, j));
            }
        }
        let pts: Vec<Point> = g[1..g.len() - 1]
            .iter()
            .enumerate()
            .filter(|(m, _)| keep[m + 1])
            .map(|(_, p)| Point(*p))
            .collect();
        let before = sc.links[id.as_str()].pts.get().map_or(0, Vec::len);
        dropped += before - pts.len();
        sc.links.get_mut(&id).expect("the link").pts = Maybe::Set(pts);
    }
    dropped
}

/// `optionA(sc)`: Option A, East links: a collector loop to Appleby Road and an arterial north to Sutton Road (proposal links). Gives the ids of the links made.
pub fn option_a(sc: &mut Scenario, ids: &mut dyn IdSource) -> Vec<String> {
    let s = node_on(sc, [-37.0988, 174.9600], "Takotowhenua", 600.0);
    let e = node_on(sc, [-37.1006, 174.9722], "Appleby", 600.0);
    let n2 = node_on(sc, [-37.0936, 174.9552], "Sutton", 600.0);
    let mut made: Vec<String> = Vec::new();
    let props = |cls: &str, name: &str| {
        let mut p = LinkProps::new()
            .cls(cls)
            .lanes(1.0, 1.0)
            .speed(50.0)
            .status("proposed")
            .name(name)
            .cycle("separated");
        p.0.stage = Maybe::Set("proposal".to_string());
        p
    };
    if let (Some(s), Some(e)) = (&s, &e) {
        made.push(add_link(
            sc,
            ids,
            s,
            e,
            &[
                [-37.0983, 174.9628],
                [-37.0981, 174.9660],
                [-37.0987, 174.9692],
                [-37.0997, 174.9710],
            ],
            &props("collector", "Drury East Link (Option A, illustrative)"),
        ));
    }
    if let (Some(s), Some(n2)) = (&s, &n2) {
        made.push(add_link(
            sc,
            ids,
            s,
            n2,
            &[[-37.0968, 174.9590], [-37.0950, 174.9570]],
            &props(
                "arterial",
                "Takotowhenua Avenue extension (Option A, illustrative)",
            ),
        ));
    }
    let list: Vec<String> = [&s, &e, &n2].into_iter().flatten().cloned().collect();
    auto_control(sc, &list);
    if let Some(s) = &s
        && made.len() == 2
        && let Some(n) = sc.nodes.get_mut(s)
    {
        n.control = Maybe::Set("roundabout".to_string());
        n.upgrade = true.into();
    }
    made
}

/// `optionB(sc)`: Option B, Waihoehoe Road four-laned between Great South Road and Takotowhenua Avenue (mitigation stage). Gives how many links were widened.
pub fn option_b(sc: &mut Scenario) -> usize {
    let mut n = 0;
    let ids: Vec<String> = sc.links.keys().map(str::to_string).collect();
    for id in ids {
        let l = &sc.links[id.as_str()];
        if !has_word(l.name.as_str().unwrap_or(""), "waihoehoe") || l.ring.truthy() {
            continue;
        }
        let (Some(a), Some(b)) = (sc.nodes.get(&l.a), sc.nodes.get(&l.b)) else {
            continue;
        };
        if math_max(a.lng.num(), b.lng.num()) > 174.9600
            || crate::jsnum::math_min(a.lng.num(), b.lng.num()) < 174.9519
        {
            continue;
        }
        let l = sc.links.get_mut(&id).expect("the link");
        l.lanes_ab = Num::number(math_max(2.0, l.lanes_ab.num()));
        l.lanes_ba = Num::number(math_max(2.0, l.lanes_ba.num()));
        l.cls = Maybe::Set("arterial".to_string());
        l.speed = Num::number(50.0);
        l.status = Maybe::Set("proposed".to_string());
        l.stage = Maybe::Set("mitigation".to_string());
        n += 1;
    }
    n
}

/// The object a JSON value is, made when it is not one (`x ||= {}` on a project key).
pub(crate) fn object_at<'a>(
    m: &'a mut Map<String, Value>,
    key: &str,
) -> &'a mut Map<String, Value> {
    let v = m
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !v.is_object() {
        *v = Value::Object(Map::new());
    }
    v.as_object_mut().expect("an object")
}
