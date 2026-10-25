//! Queries on results (engine.js travel, isochrone, selectLink, exportReady, lts) and the
//! model status metadata (modelStatus, modelMeta). The settings readers they use (yearOf,
//! growthBasis, junctionSet, capMethodOf) and the STANDARDS table are in crate::standards.

// `!(x > 0)` is the JavaScript's own test: unlike `x <= 0` it is true for NaN
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::js_sort_by;
use super::plain;
use super::types::{Convergence, ModelMeta, ModelStatus, OdFlow, Res, Vintage};
use crate::graph::{Graph, build_graph};
use crate::jsnum::{
    Maybe, Num, js_trim, math_max, or_default, to_int32, to_precision, value_to_js_string,
    value_truthy,
};
use crate::junction::{PerfSet, arc_time};
use crate::project::{Link, OrderedMap, Project, Scenario, Settings};
use crate::standards::{STANDARDS, cap_method_of, growth_basis, junction_set, year_of};
use crate::util::LatLng;
use serde::Serialize;
use serde_json::Value;

/// SELECT_LABEL: Frank-Wolfe path splits are not unique, so select-link results are indicative.
pub const SELECT_LABEL: &str =
    "Frank-Wolfe path splits are not unique; select-link results are indicative.";

/* ---------------- travel and isochrones ---------------- */

/// What travel and isochrone need from a network graph: engine.js `buildGraph`, `spt` and the
/// arc fields they read. Implemented for the graph port's [`Graph`]; tests use small graphs of their own.
pub trait RouteGraph {
    /// `G.idx.get(id)`: a node's index; zone centroids are "z:<zone id>".
    fn node_index(&self, id: &str) -> Option<usize>;
    /// `G.N`: nodes, zone centroids included.
    fn node_count(&self) -> usize;
    /// `G.ids[i]`.
    fn node_id(&self, i: usize) -> &str;
    /// `G.isC[i]`: a zone centroid.
    fn is_centroid(&self, i: usize) -> bool;
    /// `G.M`: arcs, zone connectors included.
    fn arc_count(&self) -> usize;
    /// `G.arcs[a].key`.
    fn arc_key(&self, a: usize) -> &str;
    /// `G.arcs[a].len` (m).
    fn arc_len(&self, a: usize) -> f64;
    /// `G.arcs[a].ff`: free-flow time (s).
    fn arc_ff(&self, a: usize) -> f64;
    /// `G.arcs[a].pts`: the arc's shape in its direction.
    fn arc_pts(&self, a: usize) -> &[LatLng];
    /// `arcTime(G.arcs[a], 0, Store.S)`: the arc's time with no flow (s), under the project settings.
    fn empty_time(&self, a: usize) -> f64;
    /// `spt(G, cost, src, W, target)` (target -1 when None): the edge-based search's node
    /// arrival times `W.nd`, arrival arcs `W.na` and arc predecessors `W.pred`.
    fn shortest(&self, cost: &[f64], src: usize, target: Option<usize>) -> SearchTree;
}

/// What one edge-based search leaves in its workspace (engine.js `W`).
#[derive(Clone, Debug, PartialEq)]
pub struct SearchTree {
    /// `W.src`: the node searched from.
    pub src: usize,
    /// `W.nd`: best arrival time at each node (s), Infinity when not reached.
    pub nd: Vec<f64>,
    /// `W.na`: the arc each node is reached by, -1 when none.
    pub na: Vec<i32>,
    /// `W.pred`: each arc's predecessor arc on the tree, -1 when none.
    pub pred: Vec<i32>,
}

impl SearchTree {
    /// `pathTo(G, W, v)`: arc indexes from the source to node v, or None when v was not reached.
    pub fn path_to(&self, v: usize) -> Option<Vec<usize>> {
        if v == self.src {
            return Some(Vec::new());
        }
        let mut a = self.na[v];
        if a < 0 {
            return None;
        }
        let m = self.pred.len();
        let mut p = Vec::new();
        while a >= 0 && p.len() <= m {
            p.push(a as usize);
            a = self.pred[a as usize];
        }
        p.reverse();
        Some(p)
    }
}

/// `costFromRes(G, res)`: each arc's time from the result (`res.arcs[key].t`), otherwise its time with no flow.
pub fn cost_from_res(g: &impl RouteGraph, res: Option<&Res>) -> Vec<f64> {
    (0..g.arc_count())
        .map(|i| match res.and_then(|r| r.arcs.get(g.arc_key(i))) {
            Some(r) => r.t,
            None => g.empty_time(i),
        })
        .collect()
}

/// A route between two nodes on a result's times (engine.js travel).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Travel {
    /// `{ none: true }`: the destination cannot be reached.
    Unreached {
        /// Always true.
        none: bool,
    },
    /// The quickest route.
    Route(TravelRoute),
}

/// The quickest route between two nodes.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TravelRoute {
    /// Arc keys in order.
    pub arcs: Vec<String>,
    /// The route's shape.
    #[serde(with = "plain")]
    pub pts: Vec<[f64; 2]>,
    /// Travel time with turn penalties (s).
    #[serde(with = "plain")]
    pub time: f64,
    /// Free-flow time (s).
    #[serde(with = "plain")]
    pub ff: f64,
    /// Length (m).
    #[serde(with = "plain")]
    pub len: f64,
    /// Turn penalties' share of the time (s).
    #[serde(with = "plain")]
    pub pen: f64,
}

/// The graph travel and isochrone search: `buildGraph(sc, Store.S, (res && res.period) || null)`.
pub fn query_graph(sc: &Scenario, s: &Settings, res: Option<&Res>) -> Graph {
    let period = res.map(|r| r.period.as_str()).filter(|p| !p.is_empty());
    build_graph(sc, s, period)
}

/// `Engine.travel(sc, res, fromId, toId)`: the quickest route between two node ids (zone centroids are "z:<zone id>") on the result's times, under the project settings s; None when either id is not a node.
pub fn travel(
    sc: &Scenario,
    s: &Settings,
    res: Option<&Res>,
    from_id: &str,
    to_id: &str,
) -> Option<Travel> {
    travel_on(&query_graph(sc, s, res), res, from_id, to_id)
}

/// travel on a graph already built (see [`query_graph`]).
pub fn travel_on(
    g: &impl RouteGraph,
    res: Option<&Res>,
    from_id: &str,
    to_id: &str,
) -> Option<Travel> {
    // the JavaScript works out the costs first; they have no effect when an id is not a node
    let s = g.node_index(from_id)?;
    let t = g.node_index(to_id)?;
    let cost = cost_from_res(g, res);
    let w = g.shortest(&cost, s, Some(t));
    if !w.nd[t].is_finite() {
        return Some(Travel::Unreached { none: true });
    }
    // pathTo returns a path whenever the node was reached
    let p = w.path_to(t).unwrap_or_default();
    let (mut len, mut ff, mut run) = (0.0, 0.0, 0.0);
    let mut pts: Vec<LatLng> = Vec::new();
    for &ai in &p {
        len += g.arc_len(ai);
        ff += g.arc_ff(ai);
        run += cost[ai];
        let ap = g.arc_pts(ai);
        if pts.is_empty() {
            pts.extend_from_slice(ap);
        } else {
            pts.extend_from_slice(ap.get(1..).unwrap_or(&[]));
        }
    }
    Some(Travel::Route(TravelRoute {
        arcs: p.iter().map(|&i| g.arc_key(i).to_string()).collect(),
        pts,
        time: w.nd[t],
        ff,
        len,
        pen: math_max(0.0, w.nd[t] - run),
    }))
}

/// Times from one node to every node reached (engine.js isochrone).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Isochrone {
    /// The node searched from.
    pub from: String,
    /// Earliest arrival (s) by node id, zone centroids left out, in node order.
    #[serde(with = "plain")]
    pub times: OrderedMap<f64>,
}

/// `Engine.isochrone(sc, res, fromId)`: earliest arrival at every node from one node on the result's times, under the project settings s; None when the id is not a node.
pub fn isochrone(
    sc: &Scenario,
    s: &Settings,
    res: Option<&Res>,
    from_id: &str,
) -> Option<Isochrone> {
    isochrone_on(&query_graph(sc, s, res), res, from_id)
}

/// isochrone on a graph already built (see [`query_graph`]).
pub fn isochrone_on(g: &impl RouteGraph, res: Option<&Res>, from_id: &str) -> Option<Isochrone> {
    let s = g.node_index(from_id)?;
    let cost = cost_from_res(g, res);
    let w = g.shortest(&cost, s, None);
    let mut times = OrderedMap::new();
    for i in 0..g.node_count() {
        if !g.is_centroid(i) && w.nd[i].is_finite() {
            times.insert(g.node_id(i), w.nd[i]);
        }
    }
    Some(Isochrone {
        from: from_id.to_string(),
        times,
    })
}

/// The graph port's graph. Times with no flow use the settings it was built with (travel and
/// isochrone build it with the project settings, as `arcTime(a, 0, Store.S)` reads them).
impl RouteGraph for Graph {
    fn node_index(&self, id: &str) -> Option<usize> {
        self.idx.get(id).copied()
    }
    fn node_count(&self) -> usize {
        self.n
    }
    fn node_id(&self, i: usize) -> &str {
        &self.ids[i]
    }
    fn is_centroid(&self, i: usize) -> bool {
        self.is_c()[i] != 0
    }
    fn arc_count(&self) -> usize {
        self.m
    }
    fn arc_key(&self, a: usize) -> &str {
        &self.arcs[a].key
    }
    fn arc_len(&self, a: usize) -> f64 {
        self.arcs[a].len
    }
    fn arc_ff(&self, a: usize) -> f64 {
        self.arcs[a].ff
    }
    fn arc_pts(&self, a: usize) -> &[LatLng] {
        &self.arcs[a].pts
    }
    fn empty_time(&self, a: usize) -> f64 {
        arc_time(&self.arcs[a], 0.0, &PerfSet::new(&self.settings))
    }
    fn shortest(&self, cost: &[f64], src: usize, target: Option<usize>) -> SearchTree {
        let mut w = self.workspace();
        self.spt(cost, src, &mut w, target.map_or(-1, |t| t as i32), -1);
        SearchTree {
            src,
            nd: w.nd,
            na: w.na,
            pred: w.pred,
        }
    }
}

/* ---------------- select-link from stored paths ---------------- */

/// The traffic through one arc and where it goes (engine.js selectLink).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SelectLink {
    /// The arc key.
    pub key: String,
    /// Flow of the selected paths (pcu/h) by arc key, in the order first met.
    #[serde(with = "plain")]
    pub flows: OrderedMap<f64>,
    /// OD pairs over 0.05 pcu/h through the arc, largest first.
    pub ods: Vec<OdFlow>,
    /// Total of those pairs (pcu/h).
    #[serde(with = "plain")]
    pub tot: f64,
}

/// `Engine.selectLink(res, key)`: the paths through an arc, from the result's stored paths; None without paths or for an unknown key.
pub fn select_link(res: Option<&Res>, key: &str) -> Option<SelectLink> {
    let res = res?;
    let paths = res.paths.get()?;
    let arc_keys = res.arc_keys.as_ref()?;
    let ai = arc_keys.iter().position(|k| k == key)?;
    let mut flows: OrderedMap<f64> = OrderedMap::new();
    let mut ods = Vec::new();
    let mut tot = 0.0;
    for od in paths {
        let mut q = 0.0;
        for p in &od.list {
            if !p.arcs.contains(&ai) {
                continue;
            }
            q += p.w;
            for &a in &p.arcs {
                let k = arc_keys.get(a).map_or("undefined", String::as_str);
                let f = flows.get(k).copied().map_or(0.0, |x| or_default(x, 0.0));
                flows.insert(k, f + p.w);
            }
        }
        if q > 0.05 {
            ods.push(OdFlow {
                o: res.zone_ids.get(od.i).cloned().unwrap_or_default(),
                d: res.zone_ids.get(od.j).cloned().unwrap_or_default(),
                q,
            });
            tot += q;
        }
    }
    js_sort_by(&mut ods, |a, b| b.q - a.q);
    Some(SelectLink {
        key: key.to_string(),
        flows,
        ods,
        tot,
    })
}

/* ---------------- export readiness ---------------- */

/// Whether turn volumes can go to SIDRA, and why not (engine.js exportReady).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ExportReady {
    /// The run is converged well enough.
    pub ok: bool,
    /// Why not ("" when ok).
    pub why: String,
}

/// `Engine.exportReady(res)`: turn-volume exports need a gap of STANDARDS.sidraGap.max (0.01%) or better.
pub fn export_ready(res: Option<&Res>) -> ExportReady {
    let max = STANDARDS.sidra_gap.max;
    let Some(res) = res else {
        return ExportReady {
            ok: false,
            why: "Run the model first.".to_string(),
        };
    };
    if !(res.gap <= max) {
        return ExportReady {
            ok: false,
            why: format!(
                "The run's gap is {}%; turn volumes for SIDRA need {}% or better. Re-run with a tighter convergence target.",
                to_precision(res.gap * 100.0, 2),
                to_precision(max * 100.0, 1)
            ),
        };
    }
    ExportReady {
        ok: true,
        why: String::new(),
    }
}

/* ---------------- cycling level of traffic stress ---------------- */

/// `Engine.lts(L, res)`: cycling level of traffic stress, 1 (all ages) to 4 (confident riders only); 0 where cycling is not allowed.
///
/// Daily traffic is both directions' flow times `Store.S.peakToDaily` (the project settings `s`).
pub fn lts(l: &Link, res: Option<&Res>, s: &Settings) -> u8 {
    let c = l.class();
    let sp = l.speed.or(c.speed);
    let lanes = to_int32(l.lanes_ab.num()).max(to_int32(l.lanes_ba.num()));
    if c.rank >= 5.0 {
        return 0;
    }
    let mut aadt: Option<f64> = None;
    if let Some(res) = res {
        let a = res.arcs.get(&format!("{}:ab", l.id));
        let b = res.arcs.get(&format!("{}:ba", l.id));
        aadt = Some((a.map_or(0.0, |x| x.v) + b.map_or(0.0, |x| x.v)) * s.peak_to_daily.num());
    }
    if l.cycle.is("separated") {
        return 1;
    }
    if l.cycle.is("painted") {
        return if sp <= 50.0 && lanes <= 1 {
            2
        } else if sp <= 50.0 {
            3
        } else {
            4
        };
    }
    if sp <= 30.0 && lanes <= 1 && aadt.is_none_or(|x| x < 3000.0) {
        return 1;
    }
    if sp <= 40.0 && lanes <= 1 && aadt.is_none_or(|x| x < 6000.0) {
        return 2;
    }
    if sp <= 50.0 && lanes <= 1 {
        return 3;
    }
    4
}

/* ---------------- model status and metadata ---------------- */

const STD_NAME: [(&str, &str); 4] = [
    ("tmdg", "TMDG"),
    ("tag", "UK TAG M3.1"),
    ("rms", "RMS 2013"),
    ("atap", "ATAP"),
];

fn std_name(s: &str) -> Option<&'static str> {
    STD_NAME.iter().find(|(k, _)| *k == s).map(|(_, v)| *v)
}

// String(x || d): the value's text when truthy, otherwise d
fn text_or(x: &Num, d: &str) -> String {
    if x.truthy() {
        x.js_string()
    } else {
        d.to_string()
    }
}

/// `Engine.modelStatus(P, period)`: Uncalibrated, Calibrated (matrix estimation fitted) or Validated
/// (independent counts met the targets, recorded for the category and standard the settings now name).
pub fn model_status(p: &Project, period: &str) -> ModelStatus {
    let s = &p.settings;
    let tm = s.tmdg.get();
    let cat = text_or(tm.map_or(&Maybe::Missing, |t| &t.category), "E").to_uppercase();
    let just = js_trim(&text_or(
        tm.map_or(&Maybe::Missing, |t| &t.justification),
        "",
    ))
    .to_string();
    let std = text_or(
        s.calib.get().map_or(&Maybe::Missing, |c| &c.standard),
        "tmdg",
    );
    let info =
        p.me.get()
            .and_then(|me| me.info.get())
            .and_then(|i| i.get(period))
            .and_then(Maybe::get);
    let val = p
        .validation
        .get()
        .and_then(|v| v.get(period))
        .and_then(Maybe::get);
    let v_cat = val.and_then(|v| match &v.category {
        Maybe::Set(c) if c.as_str() != Some("") => Some(c.to_js_string().to_uppercase()),
        _ => None,
    });
    let v_std = val.and_then(|v| {
        if v.standard.truthy() {
            Some(v.standard.js_string())
        } else {
            None
        }
    });
    // `vCat || cat` and `vStd || std`: text that is empty reads as false
    let v_cat_or = v_cat.as_deref().filter(|c| !c.is_empty());
    let v_std_or = v_std.as_deref().filter(|x| !x.is_empty());
    let matches = val.is_some()
        && v_cat.as_ref().is_none_or(|c| *c == cat)
        && v_std.as_ref().is_none_or(|x| *x == std);
    let passed = val.is_some_and(|v| v.pass.is_true() && v.independent.truthy());
    let status = if passed && matches {
        "validated"
    } else if info.is_some() {
        "calibrated"
    } else {
        "uncalibrated"
    };
    let short = match status {
        "validated" => "Validated",
        "calibrated" => "Calibrated",
        _ => "Uncalibrated",
    };
    let std_text = |x: &str| -> String {
        if x == "tmdg" {
            format!("TMDG Category {}", v_cat_or.unwrap_or(&cat))
        } else {
            std_name(x).unwrap_or(x).to_string()
        }
    };
    let label = match status {
        "uncalibrated" => "Uncalibrated".to_string(),
        "calibrated" => "Calibrated: base matched to counts used in calibration".to_string(),
        _ => format!(
            "Validated: independent validation counts meet the {} targets",
            std_text(v_std_or.unwrap_or(&std))
        ),
    };
    let stale = if passed && !matches {
        let recorded = match v_std_or {
            Some(x) if x != "tmdg" => std_name(x).unwrap_or(x).to_string(),
            _ => format!("TMDG Category {}", v_cat_or.unwrap_or(&cat)),
        };
        let now = if std != "tmdg" {
            std_name(&std).unwrap_or(&std).to_string()
        } else {
            format!("TMDG Category {}", cat)
        };
        format!(
            "The validation was recorded for {}; the settings now name {}. Validate the base year again.",
            recorded, now
        )
    } else {
        String::new()
    };
    let truthy_date = |d: Option<&Value>| d.filter(|x| value_truthy(x)).cloned();
    let date = truthy_date(val.and_then(|v| v.date.get()))
        .or_else(|| truthy_date(info.and_then(|i| i.date.get())))
        .unwrap_or(Value::Null);
    let category = match (v_cat_or, status) {
        (Some(c), "validated") => c.to_string(),
        _ => cat.clone(),
    };
    let footer = format!(
        "Model status ({}): {} \u{b7} TMDG category {}{}",
        period,
        short,
        cat,
        if just.is_empty() {
            " (justification not given)"
        } else {
            ""
        }
    );
    ModelStatus {
        status: status.to_string(),
        short: short.to_string(),
        label,
        period: period.to_string(),
        category,
        standard: std,
        justification: just,
        date,
        stale: if stale.is_empty() { None } else { Some(stale) },
        footer,
        standing: STANDARDS.tmdg.standing.to_string(),
    }
}

const SRC_NAME: [(&str, &str); 5] = [
    ("nzta", "NZTA state highway counts"),
    ("at", "Auckland Transport counts"),
    ("user", "Your counts"),
    ("fc", "AT Future Connect"),
    ("controlled", "AT controlled intersections"),
];

/// `Engine.modelMeta(sc, period, S, c)`: the metadata every result carries (model status for the
/// project, convergence c, data vintages from `P.data.loaded`, base and scenario years, growth
/// basis and junction methods under the settings s).
pub fn model_meta(
    p: &Project,
    sc: Option<&Scenario>,
    period: &str,
    s: &Settings,
    c: Convergence,
) -> ModelMeta {
    let loaded = p.data.get().and_then(|d| d.loaded.get());
    let mut vintages = OrderedMap::new();
    let entry = |k: &str| loaded.and_then(|l| l.get(k)).filter(|v| value_truthy(v));
    let field = |v: &Value, k: &str| v.get(k).cloned();
    for (k, name) in SRC_NAME {
        let Some(x) = entry(k) else { continue };
        let Some(at) = field(x, "at").filter(value_truthy) else {
            continue;
        };
        let source = field(x, "source")
            .filter(value_truthy)
            .unwrap_or_else(|| Value::String(name.to_string()));
        vintages.insert(
            k,
            Vintage {
                source,
                date: js_iso_day(&at),
                n: field(x, "n").unwrap_or(Value::Null),
                licence: field(x, "licence").filter(value_truthy),
                window: Maybe::Missing,
            },
        );
    }
    if let Some(cas) = entry("cas")
        && let Some(at) = field(cas, "at").filter(value_truthy)
    {
        vintages.insert(
            "cas",
            Vintage {
                source: Value::String("NZTA Crash Analysis System".to_string()),
                date: js_iso_day(&at),
                n: field(cas, "n").unwrap_or(Value::Null),
                licence: None,
                window: Maybe::Set(
                    field(cas, "label")
                        .filter(value_truthy)
                        .unwrap_or(Value::Null),
                ),
            },
        );
    }
    ModelMeta {
        status: model_status(p, period),
        convergence: c,
        vintages,
        base_year: s.base_year.clone(),
        year: year_of(sc, s),
        growth_basis: growth_basis(s),
        los_method: junction_set(s).method.to_string(),
        cap_method: cap_method_of(s).as_str().to_string(),
    }
}

/* ---------------- dates ---------------- */

/// `new Date(t).toISOString().slice(0, 10)` for a stored time stamp: the UTC day "YYYY-MM-DD".
///
/// Numbers are milliseconds since 1970; true is 1 ms. Text is read in the ECMAScript date
/// time string format (YYYY, YYYY-MM or YYYY-MM-DD, optionally THH:mm, :ss and .sss, then Z or
/// an offset; years may be +/-YYYYYY); a date-time without an offset is read as UTC, where
/// JavaScript uses the local time zone. Arrays are read through their text. None where
/// JavaScript has an invalid date (toISOString throws) or a format only V8's fallback parser
/// reads.
pub fn js_iso_day(t: &Value) -> Option<String> {
    let ms = match t {
        Value::Number(n) => n.as_f64()?,
        Value::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::String(s) => parse_iso_date(s)?,
        Value::Array(_) => parse_iso_date(&value_to_js_string(t))?,
        _ => return None,
    };
    iso_day_of_ms(ms)
}

/// The UTC day "YYYY-MM-DD" (extended years "+YYYYYY-MM") of a JavaScript time value; None outside the Date range.
pub fn iso_day_of_ms(ms: f64) -> Option<String> {
    if !ms.is_finite() || ms.abs() > 8.64e15 {
        return None;
    }
    let t = ms.trunc() + 0.0;
    let days = (t / 86_400_000.0).floor() as i64;
    let (y, m, d) = civil_from_days(days);
    let year = if (0..=9999).contains(&y) {
        format!("{:04}", y)
    } else if y < 0 {
        format!("-{:06}", -y)
    } else {
        format!("+{:06}", y)
    };
    let full = format!("{}-{:02}-{:02}", year, m, d);
    Some(full.chars().take(10).collect())
}

// Days since 1970-01-01 to (year, month, day) in the proleptic Gregorian calendar (H. Hinnant's algorithm)
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

// The ECMAScript date time string format: milliseconds since 1970, or None
fn parse_iso_date(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let mut i = 0;
    let digits = |i: &mut usize, n: usize| -> Option<i64> {
        let part = b.get(*i..*i + n)?;
        if !part.iter().all(u8::is_ascii_digit) {
            return None;
        }
        *i += n;
        std::str::from_utf8(part).ok()?.parse().ok()
    };
    let year = match b.first()? {
        b'+' | b'-' => {
            let neg = b[0] == b'-';
            i = 1;
            let y = digits(&mut i, 6)?;
            if neg && y == 0 {
                return None;
            }
            if neg { -y } else { y }
        }
        _ => digits(&mut i, 4)?,
    };
    let (mut month, mut day) = (1, 1);
    if b.get(i) == Some(&b'-') {
        i += 1;
        month = digits(&mut i, 2)?;
        if b.get(i) == Some(&b'-') {
            i += 1;
            day = digits(&mut i, 2)?;
        }
    }
    let (mut hh, mut mm, mut ss, mut msec, mut offset) = (0, 0, 0, 0, 0);
    if b.get(i) == Some(&b'T') {
        i += 1;
        hh = digits(&mut i, 2)?;
        if b.get(i) != Some(&b':') {
            return None;
        }
        i += 1;
        mm = digits(&mut i, 2)?;
        if b.get(i) == Some(&b':') {
            i += 1;
            ss = digits(&mut i, 2)?;
            if b.get(i) == Some(&b'.') {
                i += 1;
                let start = i;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                if i == start {
                    return None;
                }
                // milliseconds from the first three digits
                let frac = &s[start..i];
                let three: String = frac.chars().chain("000".chars()).take(3).collect();
                msec = three.parse().ok()?;
            }
        }
        match b.get(i) {
            Some(b'Z') => i += 1,
            Some(&c) if c == b'+' || c == b'-' => {
                i += 1;
                let oh = digits(&mut i, 2)?;
                if b.get(i) != Some(&b':') {
                    return None;
                }
                i += 1;
                let om = digits(&mut i, 2)?;
                if oh > 23 || om > 59 {
                    return None;
                }
                offset = (oh * 60 + om) * if c == b'+' { 1 } else { -1 };
            }
            _ => {}
        }
    }
    if i != b.len() {
        return None;
    }
    let dim = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    if !(1..=12).contains(&month) || day < 1 {
        return None;
    }
    let max_day = if month == 2 && leap {
        29
    } else {
        dim[month as usize - 1]
    };
    if day > max_day || hh > 24 || mm > 59 || ss > 59 {
        return None;
    }
    if hh == 24 && (mm != 0 || ss != 0 || msec != 0) {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let ms = days as f64 * 86_400_000.0
        + ((hh * 60 + mm - offset) * 60 + ss) as f64 * 1000.0
        + msec as f64;
    if ms.abs() > 8.64e15 { None } else { Some(ms) }
}
