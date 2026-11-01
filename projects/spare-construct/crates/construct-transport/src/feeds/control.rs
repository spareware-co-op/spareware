//! Junction controls from Auckland Transport's controlled intersections and regulatory signs, and turn bans from
//! OpenStreetMap restrictions (sections 2 and 2b of feeds.js).
//!
//! AT signal sites within 40 m of a junction (the nearest junction node and the rest of its junction site) and signs
//! within 25 m, on the approach whose last 30 m is nearest with the sign on the driver's left, give each junction a
//! proposed control; a sign never downgrades signals or a roundabout. Restrictions through a junction node become
//! banned movements. Everything is proposed as rows for the user to tick; the apply functions change the scenario.

use super::arcgis::{Bounds, FetchFailure, HttpRequest};
use super::flow::{
    Answer, ArcQuery, Flow, LayerMeta, MetaCache, env_params, form_body, host_text, resolve_fields,
};
use super::geo::live_links;
use super::jsregex::Regex;
use super::load::strip_site_prefix;
use super::sources::{
    CONTROL_PED_CYCLE, CONTROL_SIGN_M, CONTROL_SITE_M, CONTROL_TAIL_M, SRC_CONTROLLED,
};
use super::vintage::{feeds_of, stamp};
use crate::calibrate::junction_name;
use crate::datahub::norm;
use crate::edit::collate::locale_compare_f64;
use crate::edit::ids::iso_string;
use crate::edit::json::JsonValue;
use crate::edit::net::{nearest_link, split_link};
use crate::edit::{IdSource, sites};
use crate::graph::js_sort_by;
use crate::importers::OVERPASS;
use crate::jsnum::{Maybe, json_number, math_max, to_fixed, value_to_js_string, value_truthy};
use crate::project::{Project, Scenario};
use crate::util::{self, LatLng, dist, flat};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

/// One AT controlled intersection from the controlled intersections layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlSite {
    /// The intersection id (its OBJECTID when it has none).
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub id: Value,
    /// AT's site number, or null.
    pub no: Value,
    /// Description without the leading "(number) ".
    pub name: String,
    /// Intersection type.
    #[serde(rename = "type")]
    pub kind: String,
    /// Traffic signals are in control.
    pub controlled: bool,
    /// A signalised pedestrian crossing.
    pub ped: bool,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lng: f64,
}

/// One AT regulatory sign (RG 5 stop, RG 6 give way, RG 6R roundabout).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlSign {
    /// The sign's id (its OBJECTID when it has none).
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub id: Value,
    /// "RG 5", "RG 6" or "RG 6R".
    #[serde(rename = "type")]
    pub kind: String,
    /// Which side of the road.
    pub side: String,
    /// The road's name.
    pub road: String,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lng: f64,
}

/// The downloaded signal sites and signs (`mem.control`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ControlData {
    /// Controlled intersections.
    pub sites: Vec<ControlSite>,
    /// Regulatory signs.
    pub signs: Vec<ControlSign>,
}

fn geometry_xy(x: &JsonValue) -> (f64, f64) {
    let g = x.get("geometry");
    let n = |k: &str| {
        crate::util::r7(
            g.and_then(|g| g.get(k))
                .map_or(f64::NAN, JsonValue::to_number),
        )
    };
    (n("y"), n("x"))
}

fn text_or(v: Option<&JsonValue>, d: &str) -> String {
    match v {
        Some(v) if v.truthy() => v.js_string(),
        _ => d.to_string(),
    }
}

fn nn(v: Option<&JsonValue>) -> Value {
    v.filter(|v| !v.is_null())
        .map_or(Value::Null, JsonValue::to_value)
}

/// The sites and signs of the download's two layers.
pub fn parse_control(ctl: &[JsonValue], signs: &[JsonValue]) -> ControlData {
    let empty = JsonValue::object();
    let pedestrian = Regex::lit("pedestrian", "i");
    let sign_ok = Regex::lit("^RG ?(5|6|6R)$", "i");
    let space = Regex::lit(r"\s+", "");
    let sites = ctl
        .iter()
        .filter(|x| x.get("geometry").is_some_and(JsonValue::truthy))
        .map(|x| {
            let a = x.get("attributes").filter(|a| a.truthy()).unwrap_or(&empty);
            let (lat, lng) = geometry_xy(x);
            let id = match a.get("intersection_id") {
                Some(v) if !v.is_null() => v.to_value(),
                _ => nn(a.get("OBJECTID")),
            };
            let kind = text_or(a.get("intersection_type"), "");
            ControlSite {
                id,
                no: nn(a.get("intersection_no")),
                name: strip_site_prefix(&text_or(a.get("intersection_desc"), "")),
                controlled: a
                    .get("controlled")
                    .is_some_and(|v| v.strict_eq(&JsonValue::String("Yes".to_string()))),
                ped: a
                    .get("ped_crossing_flg")
                    .is_some_and(|v| v.strict_eq(&JsonValue::String("Yes".to_string())))
                    || pedestrian.test(&kind),
                kind,
                lat,
                lng,
            }
        })
        .collect();
    let signs = signs
        .iter()
        .filter(|x| {
            x.get("geometry").is_some_and(JsonValue::truthy)
                && sign_ok.test(&text_or(
                    x.get("attributes").and_then(|a| a.get("sign_type")),
                    "",
                ))
        })
        .map(|x| {
            let a = x.get("attributes").filter(|a| a.truthy()).unwrap_or(&empty);
            let (lat, lng) = geometry_xy(x);
            let id = match a.get("sign_id") {
                Some(v) if !v.is_null() => v.to_value(),
                _ => nn(a.get("OBJECTID")),
            };
            let st = a
                .get("sign_type")
                .map_or_else(|| "undefined".to_string(), JsonValue::js_string);
            ControlSign {
                id,
                kind: space.replace(&st, |_, _| " ".to_string()).to_uppercase(),
                side: text_or(a.get("side"), ""),
                road: text_or(a.get("road_name"), ""),
                lat,
                lng,
            }
        })
        .collect();
    ControlData { sites, signs }
}

enum ControlStage {
    MetaControlled,
    MetaSigns,
    Controlled,
    Signs,
    Done,
}

/// `loadControl(b)`: the download of AT signal sites and signs for an area: the two layers' descriptions (optional), then the controlled intersections, then the signs.
pub struct ControlLoad {
    stage: ControlStage,
    bounds: Bounds,
    timeout_ms: f64,
    meta_c: LayerMeta,
    meta_s: LayerMeta,
    query: Option<ArcQuery>,
    controlled: Vec<JsonValue>,
}

impl ControlLoad {
    /// A download for an area.
    pub fn new(bounds: Bounds, timeout_ms: f64, cache: &MetaCache) -> ControlLoad {
        ControlLoad {
            stage: ControlStage::MetaControlled,
            bounds,
            timeout_ms,
            meta_c: LayerMeta::new(SRC_CONTROLLED.url, cache, timeout_ms),
            meta_s: LayerMeta::new(super::sources::SRC_SIGNS.url, cache, timeout_ms),
            query: None,
            controlled: Vec::new(),
        }
    }
    fn query_for(&self, url: &str, where_: &str, fields: Vec<String>) -> ArcQuery {
        let mut p: Vec<(String, String)> = vec![
            ("where".into(), where_.to_string()),
            ("outFields".into(), fields.join(",")),
        ];
        p.extend(env_params(&self.bounds));
        p.push(("orderByFields".into(), "OBJECTID".into()));
        ArcQuery::new(url, p, 40, self.timeout_ms)
    }
    /// The signal sites and signs, once the download is done.
    pub fn finish(self) -> ControlData {
        let signs = self.query.map(ArcQuery::into_features).unwrap_or_default();
        parse_control(&self.controlled, &signs)
    }
}

impl Flow for ControlLoad {
    fn next_request(&mut self) -> Option<HttpRequest> {
        loop {
            match self.stage {
                ControlStage::MetaControlled => match self.meta_c.next_request() {
                    Some(r) => return Some(r),
                    None => self.stage = ControlStage::MetaSigns,
                },
                ControlStage::MetaSigns => match self.meta_s.next_request() {
                    Some(r) => return Some(r),
                    None => {
                        let cf = resolve_fields(
                            self.meta_c.meta(),
                            &[
                                "OBJECTID",
                                "intersection_id",
                                "intersection_no",
                                "intersection_desc",
                                "intersection_type",
                                "controlled",
                                "ped_crossing_flg",
                            ],
                        );
                        self.query =
                            Some(self.query_for(SRC_CONTROLLED.url, "controlled='Yes'", cf));
                        self.stage = ControlStage::Controlled;
                    }
                },
                ControlStage::Controlled | ControlStage::Signs => {
                    let q = self.query.as_mut().expect("query");
                    return q.next_request();
                }
                ControlStage::Done => return None,
            }
        }
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        match self.stage {
            ControlStage::MetaControlled => {
                let _ = self.meta_c.answer(a);
                self.stage = ControlStage::MetaSigns;
                Ok(())
            }
            ControlStage::MetaSigns => {
                let _ = self.meta_s.answer(a);
                let cf = resolve_fields(
                    self.meta_c.meta(),
                    &[
                        "OBJECTID",
                        "intersection_id",
                        "intersection_no",
                        "intersection_desc",
                        "intersection_type",
                        "controlled",
                        "ped_crossing_flg",
                    ],
                );
                self.query = Some(self.query_for(SRC_CONTROLLED.url, "controlled='Yes'", cf));
                self.stage = ControlStage::Controlled;
                Ok(())
            }
            ControlStage::Controlled => {
                let q = self.query.as_mut().expect("query");
                q.answer(a)?;
                if q.is_done() {
                    self.controlled = self.query.take().expect("query").into_features();
                    let sf = resolve_fields(
                        self.meta_s.meta(),
                        &[
                            "OBJECTID",
                            "sign_id",
                            "sign_type",
                            "side",
                            "road_name",
                            "legend_note",
                        ],
                    );
                    self.query = Some(self.query_for(
                        super::sources::SRC_SIGNS.url,
                        "sign_type IN ('RG 5','RG 6','RG 6R')",
                        sf,
                    ));
                    self.stage = ControlStage::Signs;
                }
                Ok(())
            }
            ControlStage::Signs => {
                let q = self.query.as_mut().expect("query");
                q.answer(a)?;
                if q.is_done() {
                    self.stage = ControlStage::Done;
                }
                Ok(())
            }
            ControlStage::Done => Ok(()),
        }
    }
    fn is_done(&self) -> bool {
        matches!(self.stage, ControlStage::Done)
    }
}

/// After `loadControl`: shares the sites with junction sites (`P.data.controlled`), records the load and stamps the vintage table for both layers.
pub fn apply_control_load(p: &mut Project, data: &ControlData, now_ms: f64) {
    crate::datahub::ensure_data(p);
    let s = &SRC_CONTROLLED;
    {
        let d = p.data.get_mut().expect("data");
        d.controlled = Maybe::Set(serde_json::to_value(&data.sites).expect("sites"));
        d.loaded.get_mut().expect("loaded").insert(
            "controlled".to_string(),
            json!({ "at": json_number(now_ms), "n": data.sites.len(), "source": s.name, "url": s.url, "licence": s.licence }),
        );
    }
    feeds_of(p).insert(
        "control".to_string(),
        json!({ "at": iso_string(now_ms), "sites": data.sites.len(), "signs": data.signs.len() }),
    );
    let mut e1 = Map::new();
    e1.insert("n".into(), json!(data.sites.len()));
    e1.insert("note".into(), json!(s.note.unwrap_or("")));
    stamp(p, "controlled", &e1, now_ms);
    let mut e2 = Map::new();
    e2.insert("n".into(), json!(data.signs.len()));
    e2.insert("note".into(), json!("Signs within 25 m of a junction set its control: RG 6R roundabout, RG 5 stop, RG 6 give way."));
    stamp(p, "signs", &e2, now_ms);
}

/// `CTL_RANK`: how strong a control is (none 0, priority 1, stop 2, roundabout 3, signals 4).
pub fn control_rank(c: &str) -> Option<f64> {
    match c {
        "none" => Some(0.0),
        "priority" => Some(1.0),
        "stop" => Some(2.0),
        "roundabout" => Some(3.0),
        "signals" => Some(4.0),
        _ => None,
    }
}

/// `SIGN_CTL[type]`: the control a sign type gives.
pub fn sign_control(kind: &str) -> Option<&'static str> {
    match kind {
        "RG 5" => Some("stop"),
        "RG 6" => Some("priority"),
        "RG 6R" => Some("roundabout"),
        _ => None,
    }
}

/// `userControl(n)`: a control the user changed after Spare Construct set it.
pub fn user_control(n: &crate::project::Node) -> bool {
    let src = n.extra.get("controlSrc").is_some_and(value_truthy);
    let auto = n.extra.get("controlAuto").filter(|v| !v.is_null());
    src && auto.is_some_and(|a| a.as_str() != n.control.as_str())
}

/// A proposed control, or the road a pedestrian crossing would split: the row is what the review list shows.
pub type Row = Map<String, Value>;

/// What `controlProposals` gives: the rows to review and the AT sites with no junction near.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ControlProposals {
    /// Proposed controls, changes first and by junction name.
    pub rows: Vec<Row>,
    /// AT sites with no modelled junction within 40 m (each `{ site }`).
    pub unmatched: Vec<Row>,
}

struct Plan {
    entries: Vec<(String, Row)>,
}

impl Plan {
    fn get(&self, id: &str) -> Option<&Row> {
        self.entries.iter().find(|(k, _)| k == id).map(|(_, v)| v)
    }
    fn get_mut(&mut self, id: &str) -> Option<&mut Row> {
        self.entries
            .iter_mut()
            .find(|(k, _)| k == id)
            .map(|(_, v)| v)
    }
    fn set(&mut self, id: &str, v: Row) {
        match self.entries.iter_mut().find(|(k, _)| k == id) {
            Some(e) => e.1 = v,
            None => self.entries.push((id.to_string(), v)),
        }
    }
}

fn ranks_at_least(a: Option<&Value>, b: Option<&str>) -> bool {
    match (
        a.and_then(Value::as_str).and_then(control_rank),
        b.and_then(control_rank),
    ) {
        (Some(x), Some(y)) => x >= y,
        _ => false,
    }
}

fn add(plan: &mut Plan, node: &str, to: Option<&str>, src_text: &str, why: String, extra: Row) {
    let p = plan.get(node).cloned();
    if let Some(p) = &p
        && ranks_at_least(p.get("to"), to)
    {
        if let Some(Value::Object(es)) = extra.get("signs")
            && let Some(Value::Object(ps)) = plan.get_mut(node).and_then(|r| r.get_mut("signs"))
        {
            for (k, v) in es {
                ps.insert(k.clone(), v.clone());
            }
        }
        return;
    }
    let mut signs: Map<String, Value> = match p.as_ref().and_then(|p| p.get("signs")) {
        Some(Value::Object(o)) => o.clone(),
        _ => Map::new(),
    };
    if let Some(Value::Object(es)) = extra.get("signs") {
        for (k, v) in es {
            signs.insert(k.clone(), v.clone());
        }
    }
    let mut e: Row = Map::new();
    e.insert("node".into(), json!(node));
    if let Some(t) = to {
        e.insert("to".into(), json!(t));
    }
    e.insert("src".into(), json!(src_text));
    e.insert("why".into(), json!(why));
    e.insert("signs".into(), Value::Object(signs));
    // ...extra: its own signs replace the merged ones
    for (k, v) in &extra {
        e.insert(k.clone(), v.clone());
    }
    if let Some(p) = &p
        && p.get("atSiteNo").is_some_and(value_truthy)
        && !extra.get("atSiteNo").is_some_and(value_truthy)
    {
        e.insert("atSiteNo".into(), p["atSiteNo"].clone());
    }
    plan.set(node, e);
}

/// `controlProposals(sc, data)`: proposed controls for one scenario.
///
/// AT signal sites within 40 m of a junction give signals to the nearest junction node and the rest of its junction site;
/// a signalised pedestrian crossing goes on the nearest two-road node within 40 m, or (if there is none) splits the nearest
/// road. Signs within 25 m belong to the approach whose last 30 m is nearest with the sign on the driver's left. Each row says
/// what changes, why and from which source; rows the user changed themselves are marked `user`.
pub fn control_proposals(sc: &Scenario, data: Option<&ControlData>) -> ControlProposals {
    let Some(data) = data else {
        return ControlProposals {
            rows: Vec::new(),
            unmatched: Vec::new(),
        };
    };
    let mut at: HashMap<&str, Vec<&crate::project::Link>> = HashMap::new();
    let mut ring: HashSet<&str> = HashSet::new();
    for l in live_links(sc) {
        for n in [l.a.as_str(), l.b.as_str()] {
            at.entry(n).or_default().push(l);
        }
        if l.ring.truthy() {
            ring.insert(l.a.as_str());
            ring.insert(l.b.as_str());
        }
    }
    let nodes: Vec<&crate::project::Node> = sc
        .nodes
        .values()
        .filter(|n| at.contains_key(n.id.as_str()))
        .collect();
    let deg = |n: &crate::project::Node| at.get(n.id.as_str()).map_or(0, Vec::len);
    let junction = |n: &crate::project::Node| deg(n) >= 3 || ring.contains(n.id.as_str());
    let ll = |n: &crate::project::Node| -> LatLng { [n.lat.num(), n.lng.num()] };
    let near = |p: &LatLng,
                filt: &dyn Fn(&crate::project::Node) -> bool,
                max_m: f64|
     -> Option<(&crate::project::Node, f64)> {
        let mut best: Option<&crate::project::Node> = None;
        let mut bd = max_m;
        for n in &nodes {
            if !filt(n) {
                continue;
            }
            let d = dist(p, &ll(n));
            if d <= bd {
                bd = d;
                best = Some(n);
            }
        }
        best.map(|n| (n, bd))
    };
    let mut plan = Plan {
        entries: Vec::new(),
    };
    let mut unmatched: Vec<Row> = Vec::new();
    let site_json = |s: &ControlSite| serde_json::to_value(s).expect("site");
    for s in &data.sites {
        let p: LatLng = [s.lat, s.lng];
        if s.ped {
            let m = near(
                &p,
                &|n| deg(n) == 2 && !ring.contains(n.id.as_str()),
                CONTROL_SITE_M,
            );
            if let Some((n, d)) = m {
                let mut ex = Map::new();
                ex.insert("atSiteNo".into(), s.no.clone());
                ex.insert("d".into(), json_number(d));
                ex.insert("ped".into(), json!(true));
                add(
                    &mut plan,
                    &n.id,
                    Some("signals"),
                    SRC_CONTROLLED.name,
                    format!(
                        "Signalised pedestrian crossing, AT site {}",
                        value_to_js_string(&s.no)
                    ),
                    ex,
                );
            } else {
                let lk = nearest_link(
                    sc,
                    p,
                    CONTROL_SITE_M,
                    Some(&|l: &crate::project::Link| !l.status.is("closed")),
                );
                let mut u: Row = Map::new();
                u.insert("site".into(), site_json(s));
                if let Some(lk) = lk {
                    u.insert("link".into(), json!(lk.id));
                    u.insert("d".into(), json_number(lk.d));
                    u.insert("split".into(), json!(true));
                }
                unmatched.push(u);
            }
            continue;
        }
        let Some((m, _)) = near(&p, &junction, CONTROL_SITE_M) else {
            let mut u: Row = Map::new();
            u.insert("site".into(), site_json(s));
            unmatched.push(u);
            continue;
        };
        let site = sites::of(sc, &m.id);
        let group: Vec<&crate::project::Node> = match site {
            Some(site) => site
                .nodes
                .iter()
                .filter_map(|id| sc.nodes.get(id))
                .collect(),
            None => vec![m],
        };
        for n in group {
            let mut ex = Map::new();
            ex.insert("atSiteNo".into(), s.no.clone());
            ex.insert("d".into(), json_number(dist(&p, &ll(n))));
            ex.insert("site".into(), site.map_or(Value::Null, |s| json!(s.id)));
            let why = format!(
                "AT signal site {}{}",
                value_to_js_string(&s.no),
                if s.name.is_empty() {
                    String::new()
                } else {
                    format!(": {}", s.name)
                }
            );
            add(
                &mut plan,
                &n.id,
                Some("signals"),
                SRC_CONTROLLED.name,
                why,
                ex,
            );
        }
    }
    // Signs: which approach a sign belongs to. The approach's last 30 m runs into the node; the sign must be on its left.
    let caps = Regex::lit(r"\b[a-z]", "g");
    for sg in &data.signs {
        let p: LatLng = [sg.lat, sg.lng];
        let Some((m, m_d)) = near(&p, &junction, CONTROL_SIGN_M) else {
            continue;
        };
        let f = flat(p[0]);
        let (px, py) = (f.x(&p), f.y(&p));
        struct Best<'a> {
            l: &'a crate::project::Link,
            score: f64,
            left: bool,
        }
        let mut best: Option<Best> = None;
        for l in at.get(m.id.as_str()).map(Vec::as_slice).unwrap_or(&[]) {
            if l.ring.truthy() {
                continue;
            }
            let mut g = sc.geom(l);
            if l.a == m.id {
                g.reverse(); // travel towards the node
            }
            let cum = util::cum_len(&g);
            let len = cum[cum.len() - 1];
            let from = math_max(0.0, len - CONTROL_TAIL_M);
            let s0 = util::along(&g, &cum, from).p;
            let mut pts: Vec<LatLng> = vec![s0];
            for (i, q) in g.iter().enumerate() {
                if cum[i] > from {
                    pts.push(*q);
                }
            }
            let mut bd = f64::INFINITY;
            let mut left = false;
            for i in 0..pts.len().saturating_sub(1) {
                let (ax, ay, bx, by) = (
                    f.x(&pts[i]),
                    f.y(&pts[i]),
                    f.x(&pts[i + 1]),
                    f.y(&pts[i + 1]),
                );
                let r = util::seg_dist(px, py, ax, ay, bx, by);
                if r.d < bd {
                    bd = r.d;
                    left = (bx - ax) * (py - ay) - (by - ay) * (px - ax) > 0.0;
                }
            }
            let score = bd + if left { 0.0 } else { 1000.0 };
            if best.as_ref().is_none_or(|b| score < b.score) {
                best = Some(Best { l, score, left });
            }
        }
        let Some(best) = best else { continue };
        let to = sign_control(&sg.kind);
        if to == Some("roundabout") && deg(m) < 3 && !ring.contains(m.id.as_str()) {
            continue;
        }
        let cur = m.control.as_str();
        if (ring.contains(m.id.as_str()) && to != Some("roundabout"))
            || cur == Some("signals")
            || (cur == Some("roundabout") && to != Some("roundabout"))
        {
            if let Some(Value::Object(ps)) = plan.get_mut(&m.id).and_then(|r| r.get_mut("signs")) {
                ps.insert(best.l.id.clone(), json!(sg.kind));
            }
            continue;
        }
        let road = if sg.road.is_empty() {
            String::new()
        } else {
            let t = norm(&sg.road);
            format!(
                " on {}",
                caps.replace(&t, |mm, u| String::from_utf16_lossy(&u[mm.start..mm.end])
                    .to_uppercase())
            )
        };
        let why = format!(
            "{} sign{}{}",
            sg.kind,
            road,
            if best.left {
                ""
            } else {
                " (sign not on the driver\u{2019}s left: check)"
            }
        );
        let mut ex = Map::new();
        let mut sgn = Map::new();
        sgn.insert(best.l.id.clone(), json!(sg.kind));
        ex.insert("signs".into(), Value::Object(sgn));
        ex.insert("d".into(), json_number(m_d));
        add(&mut plan, &m.id, to, SRC_CONTROLLED_SIGNS_TEXT, why, ex);
    }
    let mut rows: Vec<Row> = Vec::new();
    for (_, p) in &plan.entries {
        let node_id = p["node"].as_str().expect("node id");
        let n = &sc.nodes[node_id];
        let user = user_control(n);
        let to = p.get("to").and_then(Value::as_str);
        let change = n.control.as_str() != to;
        let at_no = p.get("atSiteNo");
        let ns_at = n.extra.get("atSiteNo");
        let meta = !change
            && ((at_no.is_some_and(|a| !a.is_null()) && !loose_eq_strict(ns_at, at_no))
                || !n.extra.get("controlSrc").is_some_and(value_truthy));
        if !change && !meta {
            continue;
        }
        let mut r: Row = Map::new();
        r.insert("id".into(), json!(node_id));
        for (k, v) in p {
            r.insert(k.clone(), v.clone());
        }
        r.insert("name".into(), json!(junction_name(sc, node_id)));
        r.insert("from".into(), json!(n.control.str_or("none")));
        r.insert("change".into(), json!(change));
        r.insert("user".into(), json!(user));
        rows.push(r);
    }
    for u in unmatched.iter().filter(|u| u.contains_key("split")) {
        let site = &u["site"];
        let link = u["link"].as_str().expect("link");
        let link_name = sc
            .links
            .get(link)
            .and_then(|l| l.name.as_str())
            .filter(|n| !n.is_empty())
            .unwrap_or("the road");
        let mut r: Row = Map::new();
        r.insert(
            "id".into(),
            json!(format!(
                "split:{}",
                site.get("id")
                    .map_or_else(|| "undefined".to_string(), value_to_js_string)
            )),
        );
        r.insert("node".into(), Value::Null);
        r.insert(
            "split".into(),
            json!({ "link": link, "ll": [site["lat"], site["lng"]] }),
        );
        r.insert("to".into(), json!("signals"));
        r.insert("ped".into(), json!(true));
        r.insert("change".into(), json!(true));
        r.insert("src".into(), json!(SRC_CONTROLLED.name));
        r.insert(
            "why".into(),
            json!(format!(
                "Signalised pedestrian crossing, AT site {}: adds a node on {}",
                value_to_js_string(&site["no"]),
                link_name
            )),
        );
        r.insert("atSiteNo".into(), site["no"].clone());
        r.insert("from".into(), json!("none"));
        r.insert(
            "name".into(),
            match site["name"].as_str() {
                Some(n) if !n.is_empty() => json!(n),
                _ => json!("Pedestrian crossing"),
            },
        );
        r.insert("d".into(), u["d"].clone());
        r.insert("signs".into(), json!({}));
        rows.push(r);
    }
    js_sort_by(&mut rows, |x, y| {
        let c = |r: &Row| {
            if r["change"].as_bool() == Some(true) {
                1.0
            } else {
                0.0
            }
        };
        let d = c(y) - c(x);
        if d != 0.0 {
            return d;
        }
        locale_compare_f64(
            &value_to_js_string(&x["name"]),
            &value_to_js_string(&y["name"]),
        )
    });
    ControlProposals {
        rows,
        unmatched: unmatched
            .into_iter()
            .filter(|u| !u.contains_key("split"))
            .collect(),
    }
}

// "AT signs": the name of the signs source up to its first " ("
const SRC_CONTROLLED_SIGNS_TEXT: &str = "AT signs";

// a !== b for saved values (a node's site number against the proposed one): equal only when the same type and value
fn loose_eq_strict(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(Value::Number(x)), Some(Value::Number(y))) => x.as_f64() == y.as_f64(),
        (Some(Value::String(x)), Some(Value::String(y))) => x == y,
        (Some(Value::Bool(x)), Some(Value::Bool(y))) => x == y,
        (Some(Value::Null), Some(Value::Null)) => true,
        _ => false,
    }
}

/// `applyControls(sc, rows, ids, when)`: applies the chosen rows to a scenario and gives how many junction controls changed.
///
/// A pedestrian crossing that splits a road adds the junction first. Sets the node's control, `controlAuto`, `controlSrc`, `controlAt`
/// (the date given), its AT site number, pedestrian crossing and signs, and gives its junction site the AT site number when it has none.
pub fn apply_controls(
    sc: &mut Scenario,
    ids: &mut dyn IdSource,
    rows: &[Row],
    pick: &[String],
    when: &str,
) -> usize {
    let pick: HashSet<&str> = pick.iter().map(String::as_str).collect();
    let mut n = 0;
    for r in rows {
        let rid = r.get("id").and_then(Value::as_str).unwrap_or("");
        if !pick.contains(rid) {
            continue;
        }
        let mut id: String = r
            .get("node")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if let Some(Value::Object(sp)) = r.get("split").filter(|v| value_truthy(v)) {
            let link = sp.get("link").and_then(Value::as_str).unwrap_or("");
            let ll = sp
                .get("ll")
                .and_then(Value::as_array)
                .map(|a| {
                    [
                        a[0].as_f64().unwrap_or(f64::NAN),
                        a[1].as_f64().unwrap_or(f64::NAN),
                    ]
                })
                .unwrap_or([f64::NAN, f64::NAN]);
            match split_link(sc, ids, link, ll) {
                Ok(nid) if !nid.is_empty() => id = nid,
                _ => continue,
            }
        }
        let to = r.get("to").and_then(Value::as_str);
        let Some(node) = sc.nodes.get_mut(&id) else {
            continue;
        };
        if node.control.as_str() != to {
            n += 1;
        }
        node.control = to.map_or(Maybe::Missing, |t| Maybe::Set(t.to_string()));
        node.extra
            .insert("controlAuto".into(), to.map_or(Value::Null, |t| json!(t)));
        node.extra.insert(
            "controlSrc".into(),
            r.get("src").cloned().unwrap_or(Value::Null),
        );
        node.extra.insert("controlAt".into(), json!(when));
        node.upgrade = Maybe::Set(false.into());
        let at_no = r.get("atSiteNo").filter(|v| !v.is_null()).cloned();
        if let Some(a) = &at_no {
            node.extra.insert("atSiteNo".into(), a.clone());
        }
        if r.get("ped").is_some_and(value_truthy) {
            node.extra.insert("ped".into(), json!(true));
            if !node.cycle.truthy() {
                node.cycle = Maybe::number(CONTROL_PED_CYCLE);
            }
        }
        if let Some(Value::Object(rs)) = r.get("signs")
            && !rs.is_empty()
        {
            let mut merged: Map<String, Value> = match node.extra.get("signs") {
                Some(Value::Object(o)) => o.clone(),
                _ => Map::new(),
            };
            for (k, v) in rs {
                merged.insert(k.clone(), v.clone());
            }
            node.extra.insert("signs".into(), Value::Object(merged));
        }
        if let Some(a) = at_no {
            let site_id = sites::of(sc, &id).map(|s| s.id.clone());
            if let Some(sid) = site_id
                && let Maybe::Set(sites_map) = &mut sc.sites
                && let Some(site) = sites_map.get_mut(&sid)
                && matches!(site.at_site_no, Maybe::Missing | Maybe::Null)
            {
                site.at_site_no = Maybe::Set(a);
            }
        }
    }
    n
}

/// `loadRestrictions(b)`: the download of OpenStreetMap turn restrictions: the relations of type restriction inside the area, from the first Overpass server that answers.
pub struct RestrictionsLoad {
    query: String,
    timeout_ms: f64,
    next: usize,
    last: Option<String>,
    rels: Option<Vec<JsonValue>>,
    url: Option<String>,
}

impl RestrictionsLoad {
    /// A download for an area.
    pub fn new(b: &Bounds, timeout_ms: f64) -> RestrictionsLoad {
        let bb = [b.south, b.west, b.north, b.east]
            .iter()
            .map(|v| to_fixed(*v, 6))
            .collect::<Vec<_>>()
            .join(",");
        RestrictionsLoad {
            query: format!(
                "[out:json][timeout:60];relation[\"type\"=\"restriction\"]({bb});out body;"
            ),
            timeout_ms,
            next: 0,
            last: None,
            rels: None,
            url: None,
        }
    }
    /// The relations and the server that sent them.
    pub fn finish(self) -> Result<(Vec<JsonValue>, String), String> {
        match (self.rels, self.url) {
            (Some(r), Some(u)) => Ok((r, u)),
            _ => Err(self.last.unwrap_or_else(|| {
                "No OpenStreetMap server answered. Try again later.".to_string()
            })),
        }
    }
}

impl Flow for RestrictionsLoad {
    fn next_request(&mut self) -> Option<HttpRequest> {
        if self.rels.is_some() || self.next >= OVERPASS.len() {
            return None;
        }
        Some(HttpRequest {
            method: "POST".to_string(),
            url: OVERPASS[self.next].to_string(),
            body: Some(form_body("data", &self.query)),
            headers: vec![(
                "Content-Type".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            )],
            timeout_ms: self.timeout_ms,
            expect: super::arcgis::Expect::Json,
        })
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        let url = OVERPASS[self.next].to_string();
        self.next += 1;
        let res: Result<Vec<JsonValue>, String> = match a {
            Ok(Answer::Json(j)) => match j.get("elements") {
                Some(JsonValue::Array(els)) if j.truthy() => Ok(els
                    .iter()
                    .filter(|e| {
                        e.get("type")
                            .is_some_and(|t| t.strict_eq(&JsonValue::String("relation".into())))
                            && matches!(e.get("members"), Some(JsonValue::Array(_)))
                    })
                    .cloned()
                    .collect()),
                _ => Err(format!("{} sent no OpenStreetMap data", host_text(&url))),
            },
            Ok(_) => Err(format!("{} sent no OpenStreetMap data", host_text(&url))),
            Err(f) => Err(super::flow::get_error(&url, self.timeout_ms, &f)),
        };
        match res {
            Ok(rels) => {
                self.rels = Some(rels);
                self.url = Some(url);
                Ok(())
            }
            Err(e) => {
                self.last = Some(e);
                if self.next >= OVERPASS.len() {
                    return Err(self.last.clone().expect("last error"));
                }
                Ok(())
            }
        }
    }
    fn is_done(&self) -> bool {
        self.rels.is_some() || self.next >= OVERPASS.len()
    }
}

/// After `loadRestrictions`: stamps the vintage table with the server and the number of restrictions.
pub fn apply_restrictions(p: &mut Project, rels: &[JsonValue], url: &str, now_ms: f64) {
    let mut e = Map::new();
    e.insert("url".into(), json!(url));
    e.insert("n".into(), json!(rels.len()));
    e.insert("note".into(), json!("Turn restrictions (relation type=restriction) through a junction node become banned movements after review."));
    stamp(p, "osm", &e, now_ms);
}

/// `RESTRICT_TEXT[kind]`: how a restriction is worded.
pub fn restrict_text(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "no_left_turn" => "no left turn",
        "no_right_turn" => "no right turn",
        "no_straight_on" => "no straight on",
        "no_u_turn" => "no U-turn",
        "only_left_turn" => "left turn only",
        "only_right_turn" => "right turn only",
        "only_straight_on" => "straight on only",
        "only_u_turn" => "U-turn only",
        _ => return None,
    })
}

/// One proposed turn ban.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BanRow {
    /// Row id, "ban:<relation>:<node>".
    pub id: String,
    /// The junction node.
    pub node: String,
    /// The junction's name.
    pub name: String,
    /// The restriction kind (no_left_turn, only_straight_on, ...).
    pub kind: String,
    /// The movements to ban, "<in link>><out link>".
    pub bans: Vec<String>,
    /// The OpenStreetMap relation id.
    pub rel: Value,
    /// Where the ban comes from.
    pub src: String,
    /// What the restriction says, in words.
    pub why: String,
    /// The user removed a ban this row would add.
    pub user: bool,
}

/// A restriction left out, and why.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BanSkipped {
    /// The relation id.
    pub rel: Value,
    /// Why it is not read.
    pub why: String,
}

/// What `banProposals` gives: rows to review and the restrictions left out.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BanProposals {
    /// Bans to propose, by junction name.
    pub rows: Vec<BanRow>,
    /// Restrictions left out.
    pub skipped: Vec<BanSkipped>,
}

fn string_set(v: &Maybe<Value>) -> HashSet<String> {
    match v.get() {
        Some(Value::Array(a)) => a.iter().map(value_to_js_string).collect(),
        _ => HashSet::new(),
    }
}

/// `banProposals(sc, data)`: banned movements from OpenStreetMap restrictions through a junction node.
///
/// `no_*` bans the one movement; `only_*` bans every other movement from that road. Restrictions for one vehicle type, with a time
/// condition, that exempt cars or with a via way are left out and counted. A ban the user removed after Spare Construct added it is the user's.
pub fn ban_proposals(sc: &Scenario, rels: Option<&[JsonValue]>) -> BanProposals {
    let Some(rels) = rels else {
        return BanProposals {
            rows: Vec::new(),
            skipped: Vec::new(),
        };
    };
    let mut by_osm: HashMap<String, &crate::project::Node> = HashMap::new();
    for n in sc.nodes.values() {
        if let Some(o) = n.osm.get().filter(|v| !v.is_null()) {
            by_osm.insert(value_to_js_string(o), n);
        }
    }
    let mut at: HashMap<&str, Vec<&crate::project::Link>> = HashMap::new();
    for l in live_links(sc) {
        for n in [l.a.as_str(), l.b.as_str()] {
            at.entry(n).or_default().push(l);
        }
    }
    let way_of = |l: &crate::project::Link| -> String {
        let s = match l.osm.get() {
            Some(v) if value_truthy(v) => value_to_js_string(v),
            _ => String::new(),
        };
        s.split(':').next().unwrap_or("").to_string()
    };
    let name_of = |l: &crate::project::Link| l.name.str_or("the unnamed road").to_string();
    let except_re = Regex::lit(r"\bmotorcar\b|\bmotor_vehicle\b", "");
    let mut rows = Vec::new();
    let mut skipped = Vec::new();
    for r in rels {
        let rel_id = r.get("id").map_or(Value::Null, JsonValue::to_value);
        let empty = JsonValue::object();
        let t = r.get("tags").filter(|t| t.truthy()).unwrap_or(&empty);
        let kind_v = t.get("restriction");
        let mut skip = |why: String| {
            skipped.push(BanSkipped {
                rel: rel_id.clone(),
                why,
            })
        };
        let kind_s = kind_v.filter(|k| k.truthy()).map(JsonValue::js_string);
        let kind = kind_s.as_deref().and_then(|k| restrict_text(k).map(|_| k));
        let Some(kind) = kind else {
            skip(match &kind_s {
                Some(k) => format!("{k} is not a turn restriction Spare Construct reads"),
                None => "only for some vehicles or times".to_string(),
            });
            continue;
        };
        if t.get("restriction:conditional")
            .is_some_and(JsonValue::truthy)
            || except_re.test(&text_or(t.get("except"), ""))
        {
            skip("conditional, or cars are exempt".to_string());
            continue;
        }
        let members: Vec<&JsonValue> = match r.get("members") {
            Some(JsonValue::Array(a)) => a.iter().collect(),
            _ => Vec::new(),
        };
        let m = |role: &str| -> Vec<&JsonValue> {
            members
                .iter()
                .copied()
                .filter(|x| {
                    x.get("role")
                        .is_some_and(|v| v.strict_eq(&JsonValue::String(role.to_string())))
                })
                .collect()
        };
        let (from, via, to) = (m("from"), m("via"), m("to"));
        let ty = |x: &JsonValue| x.get("type").map(JsonValue::js_string);
        if from.len() != 1
            || to.len() != 1
            || via.len() != 1
            || ty(from[0]).as_deref() != Some("way")
            || ty(to[0]).as_deref() != Some("way")
        {
            skip("not one from way, one via and one to way".to_string());
            continue;
        }
        if ty(via[0]).as_deref() != Some("node") {
            skip("via a way (not read yet)".to_string());
            continue;
        }
        let Some(n) = by_osm.get(
            &via[0]
                .get("ref")
                .map_or_else(|| "undefined".to_string(), JsonValue::js_string),
        ) else {
            continue;
        };
        let links: &[&crate::project::Link] =
            at.get(n.id.as_str()).map(Vec::as_slice).unwrap_or(&[]);
        let from_ref = from[0]
            .get("ref")
            .map_or_else(|| "undefined".to_string(), JsonValue::js_string);
        let to_ref = to[0]
            .get("ref")
            .map_or_else(|| "undefined".to_string(), JsonValue::js_string);
        let fl: Vec<&&crate::project::Link> =
            links.iter().filter(|l| way_of(l) == from_ref).collect();
        let tl: Vec<&&crate::project::Link> =
            links.iter().filter(|l| way_of(l) == to_ref).collect();
        // The from and to ways must end at the via node (OSM rule); a way running through it is ambiguous
        if fl.len() != 1 || tl.len() != 1 {
            skip(if !fl.is_empty() && !tl.is_empty() {
                "the from or to way runs through the junction (split it in OSM)".to_string()
            } else {
                "its roads are not in the model".to_string()
            });
            continue;
        }
        let (f0, t0): (&crate::project::Link, &crate::project::Link) = (*fl[0], *tl[0]);
        let only = kind.starts_with("only_");
        let bans: Vec<String> = if only {
            links
                .iter()
                .filter(|l| !std::ptr::eq(**l, t0))
                .map(|l| format!("{}>{}", f0.id, l.id))
                .collect()
        } else {
            vec![format!("{}>{}", f0.id, t0.id)]
        };
        let have = string_set(&n.bans);
        let auto = string_set(&match n.extra.get("bansAuto") {
            Some(v) => Maybe::Set(v.clone()),
            None => Maybe::Missing,
        });
        let add: Vec<String> = bans.into_iter().filter(|x| !have.contains(x)).collect();
        if add.is_empty() {
            continue;
        }
        let why = format!(
            "{} from {}{}",
            restrict_text(kind).expect("kind"),
            name_of(f0),
            if only {
                format!(" (only into {})", name_of(t0))
            } else {
                format!(" into {}", name_of(t0))
            }
        );
        let user = add.iter().any(|x| auto.contains(x));
        rows.push(BanRow {
            id: format!("ban:{}:{}", value_to_js_string(&rel_id), n.id),
            node: n.id.clone(),
            name: junction_name(sc, &n.id),
            kind: kind.to_string(),
            bans: add,
            src: format!("OpenStreetMap relation {}", value_to_js_string(&rel_id)),
            rel: rel_id,
            why,
            user,
        });
    }
    js_sort_by(&mut rows, |x, y| locale_compare_f64(&x.name, &y.name));
    BanProposals { rows, skipped }
}

/// `applyBans(sc, rows, ids, when)`: bans the chosen rows' movements (keeping what is there) and gives how many were added.
pub fn apply_bans(sc: &mut Scenario, rows: &[BanRow], pick: &[String], when: &str) -> usize {
    let pick: HashSet<&str> = pick.iter().map(String::as_str).collect();
    let mut n = 0;
    for r in rows {
        if !pick.contains(r.id.as_str()) {
            continue;
        }
        let Some(node) = sc.nodes.get_mut(&r.node) else {
            continue;
        };
        // Set(N.bans || []) keeps first-seen order
        let list = |v: Option<&Value>| -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            if let Some(Value::Array(a)) = v {
                for x in a {
                    let s = value_to_js_string(x);
                    if !out.contains(&s) {
                        out.push(s);
                    }
                }
            }
            out
        };
        let mut have = list(node.bans.get());
        let mut auto = list(node.extra.get("bansAuto"));
        let mut src: Map<String, Value> = match node.extra.get("banSrc") {
            Some(Value::Object(o)) => o.clone(),
            _ => Map::new(),
        };
        for x in &r.bans {
            if !have.contains(x) {
                n += 1;
                have.push(x.clone());
            }
            if !auto.contains(x) {
                auto.push(x.clone());
            }
            src.insert(
                x.clone(),
                json!(format!("{} ({}), {}", r.src, r.kind, when)),
            );
        }
        node.bans = Maybe::Set(json!(have));
        node.extra.insert("bansAuto".into(), json!(auto));
        node.extra.insert("banSrc".into(), Value::Object(src));
    }
    n
}
