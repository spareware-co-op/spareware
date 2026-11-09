//! Flood plains and coastal inundation (section 7 of feeds.js): climate screening of the roads.
//!
//! Auckland Council's flood plain polygons and the coastal inundation layers (1% AEP storm tide and mean high water springs
//! plus 0.5, 1.0 and 1.5 m of sea-level rise) are downloaded for the study area; [`hazard_links`] measures the length of
//! every road inside each hazard layer and finds the roads that are the only access to a proposal (development) zone.

use super::arcgis::{Bounds, FetchFailure, HttpRequest, encode_uri_component};
use super::flow::{
    Answer, ArcQuery, Flow, LayerMeta, MetaCache, env_params, get_error, resolve_fields,
};
use super::geo::{Ring, RingsBox, live_links, rings_box, rings_of};
use super::jsregex::Regex;
use super::sources::{AC, SRC_FLOOD};
use super::vintage::{fail_stamp, stamp};
use crate::edit::json::JsonValue;
use crate::graph::{connector_map, js_sort_by};
use crate::jsnum::{math_hypot, math_max, math_max_of, math_min, math_min_of};
use crate::project::{Project, Scenario, Settings, road_class};
use crate::util::{Flat, flat, fmt, r1};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

/// The coastal layers used (`COASTAL`): the 1% AEP storm tide and mean high water springs plus 0.5, 1.0 and 1.5 m of sea-level rise, as (service name, label).
pub const COASTAL: [(&str, &str); 4] = [
    (
        "Coastal_Inundation_1_AEP",
        "Coastal inundation, 1% AEP storm tide",
    ),
    (
        "Mean_High_Water_Springs_05m_sea_level_rise",
        "Coastal inundation, MHWS + 0.5 m sea-level rise",
    ),
    (
        "Mean_High_Water_Springs_10m_sea_level_rise",
        "Coastal inundation, MHWS + 1.0 m sea-level rise",
    ),
    (
        "Mean_High_Water_Springs_15m_sea_level_rise",
        "Coastal inundation, MHWS + 1.5 m sea-level rise",
    ),
];

/// The flood layer's fields asked for.
pub const FLOOD_FIELDS: [&str; 7] = [
    "OBJECTID",
    "Hazard",
    "RAINFALL_EVENT",
    "CLIMATE_CHANGE_ADJUSTED",
    "Superseded_Date",
    "YEAR_PRODUCED",
    "DOCUMENT_NAME",
];

fn haz_geom() -> Vec<(String, String)> {
    vec![
        ("geometryPrecision".to_string(), "6".to_string()),
        ("maxAllowableOffset".to_string(), "0.00001".to_string()),
    ]
}

/// `floodLabel(A)`: the label a flood plain polygon is merged under: "Flood plain, 1% AEP, climate-change adjusted".
pub fn flood_label(a: &JsonValue) -> String {
    let cc = match a.get("CLIMATE_CHANGE_ADJUSTED") {
        Some(v) if v.truthy() => v.js_string(),
        _ => String::new(),
    };
    let t = crate::jsnum::js_trim(&cc);
    let adj =
        !cc.is_empty() && !Regex::lit("^none$", "i").test(t) && !Regex::lit(r"^no\b", "i").test(t);
    let ev = a
        .get("RAINFALL_EVENT")
        .map_or(f64::NAN, JsonValue::to_number);
    let aep = if ev > 0.0 {
        format!(
            "{}% AEP",
            fmt::num(100.0 / ev, if ev > 100.0 { 1 } else { 0 })
        )
    } else {
        "event not stated".to_string()
    };
    format!(
        "Flood plain, {}, {}",
        aep,
        if adj {
            "climate-change adjusted"
        } else {
            "current climate"
        }
    )
}

/// One hazard polygon.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HazardPoly {
    /// Rings, [lng, lat] points.
    pub rings: Vec<Ring>,
    /// The box around them.
    pub bb: RingsBox,
    /// The polygon's attributes as downloaded.
    pub attrs: Value,
}

/// A hazard layer: every polygon that shares a label.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HazardLayer {
    /// The label ("Flood plain, 1% AEP, current climate").
    pub label: String,
    /// "flood" or "coastal".
    pub src: String,
    /// The polygons.
    pub polys: Vec<HazardPoly>,
}

/// The downloaded hazards (`mem.hazards`). The flat projection of the first scenario measured is kept for every later one, as the prototype does.
#[derive(Debug, Default)]
pub struct Hazards {
    /// The layers, in the order their labels first appeared.
    pub layers: Vec<HazardLayer>,
    lat0: Cell<Option<f64>>,
}

/// What `loadHazards` reports: the layers found and the problems met.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HazardsLoaded {
    /// Each layer's label and polygon count.
    pub layers: Vec<HazardLayerCount>,
    /// What went wrong (the prototype's wording).
    pub errors: Vec<String>,
}

/// One layer's label and polygon count.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HazardLayerCount {
    /// The label.
    pub label: String,
    /// Polygons.
    pub n: usize,
}

enum HStage {
    FloodMeta,
    Flood,
    List,
    Coastal(usize),
    Done,
}

/// `loadHazards(b)`: the download of flood plains (the layer's description, then the polygons) and coastal inundation (the list of services, then each wanted layer).
///
/// The prototype asks for the coastal layers together; this asks one after another. A layer that fails is left out and
/// reported in `errors`; the load goes on.
pub struct HazardsLoad {
    bounds: Bounds,
    timeout_ms: f64,
    meta: LayerMeta,
    stage: HStage,
    query: Option<ArcQuery>,
    layers: Vec<HazardLayer>,
    errors: Vec<String>,
    use_: Vec<(&'static str, &'static str)>,
    got: Vec<bool>,
    coastal_n: usize,
    flood_stamp: Option<(usize, usize)>,
    flood_err: Option<String>,
    coastal_stamp: Option<(usize, Vec<&'static str>)>,
    coastal_err: Option<String>,
    statuses: Vec<String>,
}

impl HazardsLoad {
    /// A download for an area.
    pub fn new(bounds: Bounds, timeout_ms: f64, cache: &MetaCache) -> HazardsLoad {
        let mut l = HazardsLoad {
            bounds,
            timeout_ms,
            meta: LayerMeta::new(SRC_FLOOD.url, cache, timeout_ms),
            stage: HStage::FloodMeta,
            query: None,
            layers: Vec::new(),
            errors: Vec::new(),
            use_: Vec::new(),
            got: Vec::new(),
            coastal_n: 0,
            flood_stamp: None,
            flood_err: None,
            coastal_stamp: None,
            coastal_err: None,
            statuses: Vec::new(),
        };
        l.statuses
            .push("Downloading Auckland Council flood plains\u{2026}".to_string());
        l
    }
    /// The progress lines the prototype passes to its status callback, in order.
    pub fn statuses(&self) -> &[String] {
        &self.statuses
    }
    fn add_poly(&mut self, label: String, src: &str, rings: Vec<Ring>, attrs: Value) {
        let poly = HazardPoly {
            bb: rings_box(&rings),
            rings,
            attrs,
        };
        match self.layers.iter_mut().find(|l| l.label == label) {
            Some(l) => l.polys.push(poly),
            None => self.layers.push(HazardLayer {
                label,
                src: src.to_string(),
                polys: vec![poly],
            }),
        }
    }
    fn start_flood(&mut self) {
        let fields = resolve_fields(self.meta.meta(), &FLOOD_FIELDS);
        let mut p: Vec<(String, String)> = vec![
            ("where".into(), "1=1".into()),
            ("outFields".into(), fields.join(",")),
        ];
        p.extend(env_params(&self.bounds));
        p.extend(haz_geom());
        p.push(("orderByFields".into(), "OBJECTID".into()));
        self.query = Some(ArcQuery::new(SRC_FLOOD.url, p, 40, self.timeout_ms));
        self.stage = HStage::Flood;
    }
    fn finish_flood(&mut self, q: ArcQuery) {
        let feats = q.into_features();
        let mut sup = 0usize;
        let empty = JsonValue::object();
        for x in &feats {
            let a = x.get("attributes").filter(|a| a.truthy()).unwrap_or(&empty);
            let Some(rings) = x
                .get("geometry")
                .and_then(|g| g.get("rings"))
                .filter(|r| r.truthy())
            else {
                continue;
            };
            if a.get("Superseded_Date").is_some_and(JsonValue::truthy) {
                sup += 1;
                continue;
            }
            self.add_poly(flood_label(a), "flood", rings_of(Some(rings)), a.to_value());
        }
        self.flood_stamp = Some((feats.len() - sup, sup));
    }
    fn start_list(&mut self) {
        self.statuses
            .push("Looking for coastal inundation layers\u{2026}".to_string());
        self.stage = HStage::List;
    }
    fn start_coastal(&mut self, i: usize) {
        let (nm, _) = self.use_[i];
        let url = format!("{}/{}/FeatureServer/0", AC, encode_uri_component(nm));
        let mut p: Vec<(String, String)> = vec![
            ("where".into(), "1=1".into()),
            ("outFields".into(), "OBJECTID".into()),
        ];
        p.extend(env_params(&self.bounds));
        p.extend(haz_geom());
        self.query = Some(ArcQuery::new(&url, p, 40, self.timeout_ms));
        self.stage = HStage::Coastal(i);
    }
    fn end_coastal(&mut self) {
        if self.got.iter().all(|g| !g) {
            let e = "no coastal layer could be downloaded".to_string();
            self.errors.push(format!("Coastal inundation: {e}"));
            self.coastal_err = Some(e);
        } else {
            self.coastal_stamp = Some((
                self.coastal_n,
                self.use_
                    .iter()
                    .zip(&self.got)
                    .filter(|(_, g)| **g)
                    .map(|((nm, _), _)| *nm)
                    .collect(),
            ));
        }
        self.stage = HStage::Done;
    }
    /// After the download: the layers kept and the problems, with the vintage table stamped (flood plains and coastal inundation).
    pub fn finish(self, p: &mut Project, now_ms: f64) -> (Hazards, HazardsLoaded) {
        match (&self.flood_stamp, &self.flood_err) {
            (Some((n, sup)), _) => {
                let mut e = Map::new();
                e.insert("n".into(), json!(n));
                e.insert(
                    "note".into(),
                    json!(format!("{}Each label merges overlapping models, so a length is never counted twice.", if *sup > 0 { format!("{sup} superseded polygons left out. ") } else { String::new() })),
                );
                stamp(p, "flood", &e, now_ms);
            }
            (None, Some(err)) => {
                fail_stamp(p, "flood", err, &Map::new(), now_ms);
            }
            _ => {}
        }
        match (&self.coastal_stamp, &self.coastal_err) {
            (Some((n, names)), _) => {
                let mut e = Map::new();
                e.insert("n".into(), json!(n));
                e.insert(
                    "note".into(),
                    json!(format!("Layers: {}.", names.join(", "))),
                );
                stamp(p, "coastal", &e, now_ms);
            }
            (None, Some(err)) => {
                fail_stamp(p, "coastal", err, &Map::new(), now_ms);
            }
            _ => {}
        }
        let layers = self
            .layers
            .iter()
            .map(|l| HazardLayerCount {
                label: l.label.clone(),
                n: l.polys.len(),
            })
            .collect();
        (
            Hazards {
                layers: self.layers,
                lat0: Cell::new(None),
            },
            HazardsLoaded {
                layers,
                errors: self.errors,
            },
        )
    }
}

impl Flow for HazardsLoad {
    fn next_request(&mut self) -> Option<HttpRequest> {
        loop {
            match self.stage {
                HStage::FloodMeta => match self.meta.next_request() {
                    Some(r) => return Some(r),
                    None => self.start_flood(),
                },
                HStage::Flood | HStage::Coastal(_) => {
                    return self.query.as_mut().and_then(Flow::next_request);
                }
                HStage::List => {
                    return Some(HttpRequest::get_json(
                        format!("{AC}?f=json"),
                        self.timeout_ms,
                    ));
                }
                HStage::Done => return None,
            }
        }
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        match self.stage {
            HStage::FloodMeta => {
                let _ = self.meta.answer(a);
                self.start_flood();
            }
            HStage::Flood => {
                let q = self.query.as_mut().expect("query");
                if let Err(e) = q.answer(a) {
                    // the prototype puts the whole problem in the errors and stamps the failure
                    self.errors.push(format!("Flood plains: {e}"));
                    self.flood_err = Some(e);
                    self.query = None;
                    self.start_list();
                } else if q.is_done() {
                    let q = self.query.take().expect("query");
                    self.finish_flood(q);
                    self.start_list();
                }
            }
            HStage::List => {
                let list: Result<JsonValue, String> = match a {
                    Ok(Answer::Json(j)) => Ok(j),
                    Ok(_) => Err(get_error(
                        AC,
                        self.timeout_ms,
                        &FetchFailure::Unreadable("not JSON".into()),
                    )),
                    Err(f) => Err(get_error(AC, self.timeout_ms, &f)),
                };
                match list {
                    Err(e) => {
                        self.errors.push(format!("Coastal inundation: {e}"));
                        self.coastal_err = Some(e);
                        self.stage = HStage::Done;
                    }
                    Ok(j) => {
                        let have: HashSet<String> = match j.get("services") {
                            Some(JsonValue::Array(a)) => a
                                .iter()
                                .map(|s| {
                                    let n = s.get("name").map_or_else(
                                        || "undefined".to_string(),
                                        JsonValue::js_string,
                                    );
                                    n.rsplit('/').next().unwrap_or("").to_string()
                                })
                                .collect(),
                            _ => HashSet::new(),
                        };
                        self.use_ = COASTAL
                            .iter()
                            .copied()
                            .filter(|(nm, _)| have.contains(*nm))
                            .collect();
                        let gone: Vec<&str> = COASTAL
                            .iter()
                            .filter(|(nm, _)| !have.contains(*nm))
                            .map(|(_, l)| *l)
                            .collect();
                        if self.use_.is_empty() {
                            let e = "none of the coastal inundation services Spare Construct uses is listed any more. Tell the Spare Construct maintainers".to_string();
                            self.errors.push(format!("Coastal inundation: {e}"));
                            self.coastal_err = Some(e);
                            self.stage = HStage::Done;
                        } else {
                            if !gone.is_empty() {
                                self.errors.push(format!(
                                    "Not listed by Auckland Council any more: {}.",
                                    gone.join("; ")
                                ));
                            }
                            self.got = vec![false; self.use_.len()];
                            self.start_coastal(0);
                        }
                    }
                }
            }
            HStage::Coastal(i) => {
                let q = self.query.as_mut().expect("query");
                let label = self.use_[i].1;
                if let Err(e) = q.answer(a) {
                    // each layer fails on its own without stopping the others
                    self.errors.push(format!("{label}: {e}"));
                    self.query = None;
                } else if q.is_done() {
                    let q = self.query.take().expect("query");
                    self.got[i] = true;
                    for f in q.into_features() {
                        if let Some(rings) = f
                            .get("geometry")
                            .and_then(|g| g.get("rings"))
                            .filter(|r| r.truthy())
                        {
                            let attrs = f
                                .get("attributes")
                                .filter(|a| a.truthy())
                                .map_or_else(|| json!({}), JsonValue::to_value);
                            self.add_poly(
                                label.to_string(),
                                "coastal",
                                rings_of(Some(rings)),
                                attrs,
                            );
                            self.coastal_n += 1;
                        }
                    }
                } else {
                    return Ok(());
                }
                if i + 1 < self.use_.len() {
                    self.start_coastal(i + 1);
                } else {
                    self.end_coastal();
                }
            }
            HStage::Done => {}
        }
        Ok(())
    }
    fn is_done(&self) -> bool {
        matches!(self.stage, HStage::Done)
    }
}

/// The edge grid of one polygon for exact segment tests in flat metres (cells of 50 m).
struct PolyGrid {
    cs: f64,
    e: Vec<f64>,
    cells: Vec<Option<Vec<usize>>>,
    i0: f64,
    j0: f64,
    ni: f64,
    nj: f64,
    bx: [f64; 4],
    seen: RefCell<Vec<u32>>,
    tick: Cell<u32>,
}

fn prep_poly(rings: &[Ring], bb: &RingsBox, fl: &Flat) -> PolyGrid {
    let cs = 50.0;
    let bx = [bb[0] * fl.kx, bb[1] * fl.ky, bb[2] * fl.kx, bb[3] * fl.ky];
    let i0 = (bx[0] / cs).floor();
    let j0 = (bx[1] / cs).floor();
    let ni = (bx[2] / cs).floor() - i0 + 1.0;
    let nj = (bx[3] / cs).floor() - j0 + 1.0;
    let mut cells: Vec<Option<Vec<usize>>> = vec![None; (ni * nj).max(0.0) as usize];
    let mut ex: Vec<f64> = Vec::new();
    for r in rings {
        let mut j = r.len().wrapping_sub(1);
        for i in 0..r.len() {
            let (x1, y1, x2, y2) = (
                r[j][0] * fl.kx,
                r[j][1] * fl.ky,
                r[i][0] * fl.kx,
                r[i][1] * fl.ky,
            );
            let e = ex.len() / 4;
            ex.extend([x1, y1, x2, y2]);
            let a0 = math_max(0.0, (math_min(x1, x2) / cs).floor() - i0);
            let a1 = math_min(ni - 1.0, (math_max(x1, x2) / cs).floor() - i0);
            let c0 = math_max(0.0, (math_min(y1, y2) / cs).floor() - j0);
            let c1 = math_min(nj - 1.0, (math_max(y1, y2) / cs).floor() - j0);
            let mut a = a0;
            while a <= a1 {
                let mut c = c0;
                while c <= c1 {
                    let k = (a * nj + c) as usize;
                    cells[k].get_or_insert_with(Vec::new).push(e);
                    c += 1.0;
                }
                a += 1.0;
            }
            j = i;
        }
    }
    let n_edges = ex.len() / 4;
    PolyGrid {
        cs,
        e: ex,
        cells,
        i0,
        j0,
        ni,
        nj,
        bx,
        seen: RefCell::new(vec![0; n_edges]),
        tick: Cell::new(0),
    }
}

impl PolyGrid {
    fn cell(&self, i: f64, j: f64) -> &[usize] {
        if i < self.i0 || j < self.j0 || i >= self.i0 + self.ni || j >= self.j0 + self.nj {
            return &[];
        }
        self.cells[((i - self.i0) * self.nj + (j - self.j0)) as usize]
            .as_deref()
            .unwrap_or(&[])
    }
    fn next(&self) -> u32 {
        let t = self.tick.get() + 1;
        if t == 0xFFFF_FFFF {
            self.seen.borrow_mut().fill(0);
            self.tick.set(1);
            1
        } else {
            self.tick.set(t);
            t
        }
    }
}

// Even-odd test along the shorter horizontal ray (to the left or right edge of the polygon's box)
fn pip_flat(g: &PolyGrid, x: f64, y: f64) -> bool {
    if x < g.bx[0] || x > g.bx[2] || y < g.bx[1] || y > g.bx[3] {
        return false;
    }
    let t = g.next();
    let cy = (y / g.cs).floor();
    let c0 = (x / g.cs).floor();
    let right = g.bx[2] - x <= x - g.bx[0];
    let mut inside = false;
    let mut cx = c0;
    let mut seen = g.seen.borrow_mut();
    while if right { cx < g.i0 + g.ni } else { cx >= g.i0 } {
        for &e in g.cell(cx, cy) {
            if seen[e] == t {
                continue;
            }
            seen[e] = t;
            let (x1, y1, x2, y2) = (g.e[e * 4], g.e[e * 4 + 1], g.e[e * 4 + 2], g.e[e * 4 + 3]);
            if (y1 > y) != (y2 > y) {
                let xc = (x2 - x1) * (y - y1) / (y2 - y1) + x1;
                if if right { x < xc } else { x > xc } {
                    inside = !inside;
                }
            }
        }
        cx += if right { 1.0 } else { -1.0 };
    }
    inside
}

// Parameter intervals [t0, t1] of segment A to B (flat metres) inside the polygon
fn seg_inside(g: &PolyGrid, ax: f64, ay: f64, bx: f64, by: f64) -> Vec<[f64; 2]> {
    if math_max(ax, bx) < g.bx[0]
        || math_min(ax, bx) > g.bx[2]
        || math_max(ay, by) < g.bx[1]
        || math_min(ay, by) > g.bx[3]
    {
        return Vec::new();
    }
    let mut ts: Vec<f64> = vec![0.0, 1.0];
    let t = g.next();
    let (dx, dy) = (bx - ax, by - ay);
    {
        let mut seen = g.seen.borrow_mut();
        let mut i = (math_min(ax, bx) / g.cs).floor();
        while i <= (math_max(ax, bx) / g.cs).floor() {
            let mut j = (math_min(ay, by) / g.cs).floor();
            while j <= (math_max(ay, by) / g.cs).floor() {
                for &e in g.cell(i, j) {
                    if seen[e] == t {
                        continue;
                    }
                    seen[e] = t;
                    let x1 = g.e[e * 4];
                    let y1 = g.e[e * 4 + 1];
                    let ex = g.e[e * 4 + 2] - x1;
                    let ey = g.e[e * 4 + 3] - y1;
                    let den = dx * ey - dy * ex;
                    if den == 0.0 || den.is_nan() {
                        continue;
                    }
                    let s = ((x1 - ax) * ey - (y1 - ay) * ex) / den;
                    let u = ((x1 - ax) * dy - (y1 - ay) * dx) / den;
                    if s > 0.0 && s < 1.0 && u >= 0.0 && u <= 1.0 {
                        ts.push(s);
                    }
                }
                j += 1.0;
            }
            i += 1.0;
        }
    }
    js_sort_by(&mut ts, |p, q| p - q);
    let mut out = Vec::new();
    for k in 0..ts.len() - 1 {
        if ts[k + 1] - ts[k] < 1e-9 {
            continue;
        }
        let m = (ts[k] + ts[k + 1]) / 2.0;
        if pip_flat(g, ax + dx * m, ay + dy * m) {
            out.push([ts[k], ts[k + 1]]);
        }
    }
    out
}

fn merge_len(mut iv: Vec<[f64; 2]>) -> f64 {
    js_sort_by(&mut iv, |p, q| p[0] - q[0]);
    let mut s = 0.0;
    let mut c: Option<[f64; 2]> = None;
    for [a0, a1] in iv {
        match &mut c {
            Some(cc) if a0 <= cc[1] => cc[1] = math_max(cc[1], a1),
            _ => {
                if let Some(cc) = c {
                    s += cc[1] - cc[0];
                }
                c = Some([a0, a1]);
            }
        }
    }
    if let Some(cc) = c {
        s += cc[1] - cc[0];
    }
    s
}

/// A road inside a hazard: its length there and whether it is the only access to a proposal zone.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HazardRow {
    /// Link id.
    pub link: String,
    /// Road name, or "Unnamed road".
    pub name: String,
    /// Road class.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cls: Option<String>,
    /// Future Connect general traffic class, else the road class's label ("Local (road class)").
    pub fc: String,
    /// The hazard layer's label.
    pub layer: String,
    /// Length inside the hazard (m).
    pub len: f64,
    /// Removing the road cuts a proposal zone off from every gateway.
    pub only: bool,
}

/// `hazardLinks(sc, data)`: the length (m) of each link inside each hazard layer (half a metre or more), longest first, and `only`: the link is a connector road whose removal cuts a proposal (development) zone off from every gateway.
///
/// The flat projection is the first measured scenario's first junction, kept in `data` for every scenario after it
/// (as the prototype keeps it on `mem.hazards`); `s` is the project's settings.
pub fn hazard_links(sc: &Scenario, data: &Hazards, s: &Settings) -> Vec<HazardRow> {
    let lat0 = match data.lat0.get() {
        Some(l) => l,
        None => {
            let l = sc.nodes.values().next().map_or(-36.85, |n| n.lat.num());
            data.lat0.set(Some(l));
            l
        }
    };
    let fl = flat(lat0);
    let mut grids: HashMap<(usize, usize), PolyGrid> = HashMap::new();
    let mut rows: Vec<HazardRow> = Vec::new();
    for l in live_links(sc) {
        let g = sc.geom(l);
        let xs: Vec<f64> = g.iter().map(|p| p[1] * fl.kx).collect();
        let ys: Vec<f64> = g.iter().map(|p| p[0] * fl.ky).collect();
        let lng: Vec<f64> = g.iter().map(|p| p[1]).collect();
        let lat: Vec<f64> = g.iter().map(|p| p[0]).collect();
        let bb = [
            math_min_of(&lng),
            math_min_of(&lat),
            math_max_of(&lng),
            math_max_of(&lat),
        ];
        for (li, lay) in data.layers.iter().enumerate() {
            let mut len = 0.0;
            for i in 0..g.len().saturating_sub(1) {
                let mut iv: Vec<[f64; 2]> = Vec::new();
                for (pi, p) in lay.polys.iter().enumerate() {
                    if bb[2] < p.bb[0] || bb[0] > p.bb[2] || bb[3] < p.bb[1] || bb[1] > p.bb[3] {
                        continue;
                    }
                    let grid = grids
                        .entry((li, pi))
                        .or_insert_with(|| prep_poly(&p.rings, &p.bb, &fl));
                    iv.extend(seg_inside(grid, xs[i], ys[i], xs[i + 1], ys[i + 1]));
                }
                if !iv.is_empty() {
                    len += merge_len(iv) * math_hypot(&[xs[i + 1] - xs[i], ys[i + 1] - ys[i]]);
                }
            }
            if len >= 0.5 {
                let fc =
                    l.fc.get()
                        .and_then(|f| f.get("gt"))
                        .filter(|g| crate::jsnum::value_truthy(g))
                        .map(crate::jsnum::value_to_js_string);
                rows.push(HazardRow {
                    link: l.id.clone(),
                    name: l.name.str_or("Unnamed road").to_string(),
                    cls: l.cls.as_str().map(str::to_string),
                    fc: fc.unwrap_or_else(|| {
                        format!("{} (road class)", road_class(l.cls.as_str()).label)
                    }),
                    layer: lay.label.clone(),
                    len: r1(len),
                    only: false,
                });
            }
        }
    }
    // only access: undirected reachability from each development zone's connector nodes to any gateway node
    let dev: Vec<&crate::project::Zone> = sc
        .zones
        .values()
        .filter(|z| z.kind.is("zone") && !super::ags::existing_zone(z))
        .collect();
    let gws: Vec<&crate::project::Zone> =
        sc.zones.values().filter(|z| z.kind.is("gateway")).collect();
    if !rows.is_empty() && !dev.is_empty() && !gws.is_empty() {
        let cm = connector_map(sc, s);
        let mut adj: HashMap<&str, Vec<(&str, &str)>> = HashMap::new();
        for l in live_links(sc) {
            adj.entry(l.a.as_str())
                .or_default()
                .push((l.b.as_str(), l.id.as_str()));
            adj.entry(l.b.as_str())
                .or_default()
                .push((l.a.as_str(), l.id.as_str()));
        }
        let gw_n: HashSet<&str> = gws
            .iter()
            .flat_map(|z| {
                cm.get(&z.id)
                    .map(|v| v.iter().map(String::as_str).collect::<Vec<_>>())
                    .unwrap_or_default()
            })
            .collect();
        let reach = |from: &[String], skip: Option<&str>| -> bool {
            let mut seen: HashSet<&str> = from.iter().map(String::as_str).collect();
            let mut st: Vec<&str> = from.iter().map(String::as_str).collect();
            while let Some(x) = st.pop() {
                if gw_n.contains(x) {
                    return true;
                }
                for (y, id) in adj.get(x).map(Vec::as_slice).unwrap_or(&[]) {
                    if Some(*id) != skip && !seen.contains(y) {
                        seen.insert(y);
                        st.push(y);
                    }
                }
            }
            false
        };
        let mut only: HashMap<String, bool> = HashMap::new();
        for r in &rows {
            if only.contains_key(&r.link) {
                continue;
            }
            let v = dev.iter().any(|z| {
                let c: &[String] = cm.get(&z.id).map(Vec::as_slice).unwrap_or(&[]);
                !c.is_empty() && reach(c, None) && !reach(c, Some(&r.link))
            });
            only.insert(r.link.clone(), v);
        }
        for r in rows.iter_mut() {
            r.only = *only.get(&r.link).unwrap_or(&false);
        }
    }
    js_sort_by(&mut rows, |p, q| q.len - p.len);
    rows
}

/// What a hazard check keeps in `P.feeds.hazards[scenario]`: when it was done, the layers and the roads inside them.
pub fn record_hazards(
    p: &mut Project,
    scenario: &str,
    layers: &[HazardLayerCount],
    rows: &[HazardRow],
    now_ms: f64,
) {
    let f = super::vintage::feeds_of(p);
    if !f.get("hazards").is_some_and(crate::jsnum::value_truthy) {
        f.insert("hazards".to_string(), json!({}));
    }
    if let Some(Value::Object(h)) = f.get_mut("hazards") {
        h.insert(
            scenario.to_string(),
            json!({ "at": crate::edit::ids::iso_string(now_ms), "layers": layers, "rows": rows }),
        );
    }
}
