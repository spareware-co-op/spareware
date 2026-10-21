//! A turning count against the model: what a count is compared with (`junctionView`), turn volumes (counted, balanced or
//! estimated from leg totals by Furness), each counted movement against the modelled one (`siteRows`, `turnRows`),
//! and balancing adjacent sites (`balance`).

use super::criteria::{DOS_SOURCE, corridor, criteria_source};
use super::store::NumText;
use super::turncounts::match_sites;
use super::roles::Ctx;
use super::rows::{core_area, in_core};
use super::store::{CalibStore, TurnSite};
use super::turncounts::{LEGS, leg_bearing, multi_node, site_node};
use crate::datahub::{geh, norm};
use crate::edit::net::{degree, len as link_len, links_at};
use crate::edit::sites;
use crate::graph::js_sort_by;
use crate::jsnum::{Maybe, math_max, value_to_number};
use crate::project::{OrderedMap, Project, Scenario};
use crate::results::Res;
use crate::turns::{Arm, MovementRow, classify, movements, site_rows as turns_site_rows, turn_code};
use crate::util::{ang_diff, fmt, turn};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

/// An arm of the junction view a turning count is compared with: where the count's legs are matched.
#[derive(Clone, Debug)]
pub struct ViewArm {
    /// The key vehicles arrive on (a node's arc key, or a site's leg code).
    pub in_key: String,
    /// The key vehicles leave on.
    pub out_key: String,
    /// Outward bearing (degrees).
    pub b: f64,
    /// Road name.
    pub road: String,
    /// Link ids of the arm, in order.
    pub links: Vec<String>,
}

/// A movement of the junction view.
#[derive(Clone, Debug)]
pub struct ViewRow {
    /// The arm key vehicles arrive on.
    pub approach: String,
    /// The arm key they leave on.
    pub to: String,
    /// Movement: L, T, R or U.
    pub mv: String,
    /// Turn designation ("L2").
    pub turn: String,
    /// Modelled flow (veh/h).
    pub v: f64,
}

/// What a turning count is compared with (`junctionView`).
#[derive(Clone, Debug)]
pub struct JunctionView {
    /// "node", "site" (a junction site's legs) or "cluster" (one of several nodes of a junction: not scored).
    pub kind: &'static str,
    /// Node or site id.
    pub id: String,
    /// The count's node.
    pub node: String,
    /// The nodes in the junction.
    pub members: Vec<String>,
    /// The arms.
    pub arms: Vec<ViewArm>,
    /// The modelled movements.
    pub rows: Vec<ViewRow>,
    /// Why it cannot be scored.
    pub note: Option<String>,
    /// The site's name.
    pub name: Option<String>,
}

fn lid_of(sc: &Scenario, k: &str) -> Option<String> {
    let i = k.rfind(':').unwrap_or(0);
    sc.links.get(&k[..i]).map(|l| l.id.clone())
}

fn view_rows_of(m: &[MovementRow]) -> Vec<ViewRow> {
    m.iter()
        .map(|r| ViewRow { approach: r.approach.clone(), to: r.to.clone(), mv: r.mv.clone(), turn: r.turn.to_string(), v: r.v })
        .collect()
}

/// Movements through each junction site, entry arc to exit arc (`siteTraces`): site id to (entry arc, exit arc, veh/h), in the order first met.
pub type SiteTraces = Vec<(String, Vec<(String, String, f64)>)>;

/// `siteTraces(sc, res)`: the movements through each junction site, from what was kept for this result (`sc.calib.siteTurns`), else traced along the result's paths (the entry arc and exit arc of each path through the site, flows in veh/h to 0.01). None when the result has no sites, or no paths and nothing was kept.
///
/// The prototype also keeps the traced movements in the scenario for its own current result; [`keep_site_traces`] does that.
pub fn site_traces(sc: &Scenario, res: Option<&Res>, s: &crate::project::Settings) -> Option<SiteTraces> {
    let res = res?;
    let sites_res = res.sites.as_ref()?;
    let key = trace_key(res);
    if let Some(kept) = sc.extra.get("calib").and_then(|c| c.get("siteTurns")).and_then(|t| t.get(&res.period))
        && kept.get("result").and_then(|r| r.as_str()) == Some(key.as_str())
        && let Some(Value::Object(m)) = kept.get("sites")
    {
        let mut out: SiteTraces = Vec::new();
        for (id, list) in m {
            if let Value::Array(a) = list {
                out.push((
                    id.clone(),
                    a.iter()
                        .filter_map(|e| {
                            let e = e.as_array()?;
                            Some((e.first()?.as_str()?.to_string(), e.get(1)?.as_str()?.to_string(), value_to_number(e.get(2)?)))
                        })
                        .collect(),
                ));
            }
        }
        return Some(out);
    }
    let (Some(paths), Some(arc_keys)) = (res.paths.get(), res.arc_keys.as_ref()) else { return None };
    let idx: HashMap<&str, usize> = arc_keys.iter().enumerate().map(|(i, k)| (k.as_str(), i)).collect();
    // arc index to [(site, is entry, arc key)]
    let mut at: HashMap<usize, Vec<(String, bool, String)>> = HashMap::new();
    for st in sites_res.values() {
        for l in &st.legs {
            for k in &l.ins {
                if let Some(&i) = idx.get(k.as_str()) {
                    at.entry(i).or_default().push((st.id.clone(), true, k.clone()));
                }
            }
            for k in &l.outs {
                if let Some(&i) = idx.get(k.as_str()) {
                    at.entry(i).or_default().push((st.id.clone(), false, k.clone()));
                }
            }
        }
    }
    let pf = crate::standards::pcu_f(s);
    let mut flow: Vec<(String, Vec<((String, String), f64)>)> = Vec::new();
    for od in paths {
        for p in &od.list {
            let mut open: Vec<(String, String)> = Vec::new();
            for a in &p.arcs {
                let Some(hs) = at.get(a) else { continue };
                for (site, entry, k) in hs {
                    if *entry {
                        match open.iter_mut().find(|(x, _)| x == site) {
                            Some(o) => o.1 = k.clone(),
                            None => open.push((site.clone(), k.clone())),
                        }
                        continue;
                    }
                    let Some(pos) = open.iter().position(|(x, _)| x == site) else { continue };
                    let e = open[pos].1.clone();
                    let fi = match flow.iter().position(|(x, _)| x == site) {
                        Some(i) => i,
                        None => {
                            flow.push((site.clone(), Vec::new()));
                            flow.len() - 1
                        }
                    };
                    let m = &mut flow[fi].1;
                    let add = p.w / pf;
                    match m.iter_mut().find(|((x, y), _)| *x == e && y == k) {
                        Some(c) => c.1 += add,
                        None => m.push(((e, k.clone()), add)),
                    }
                    open.remove(pos);
                }
            }
        }
    }
    Some(
        flow.into_iter()
            .map(|(id, m)| (id, m.into_iter().map(|((a, b), v)| (a, b, crate::jsnum::math_round(v * 100.0) / 100.0)).collect()))
            .collect(),
    )
}

/// `traceKey(res)`: what identifies a result for the kept traces.
pub fn trace_key(res: &Res) -> String {
    format!(
        "{}|{}|{}|{}",
        res.period,
        crate::jsnum::number_to_js_string(res.ts),
        crate::jsnum::number_to_js_string(res.it),
        crate::jsnum::number_to_js_string(res.gap)
    )
}

/// Keeps traced site movements in the scenario for its own current result of the period (`sc.calib.siteTurns[period]`), because paths are not saved with the project.
pub fn keep_site_traces(sc: &mut Scenario, res: &Res, traces: &SiteTraces) {
    let mut sites = Map::new();
    for (id, list) in traces {
        sites.insert(
            id.clone(),
            Value::Array(
                list.iter()
                    .map(|(a, b, v)| Value::Array(vec![Value::String(a.clone()), Value::String(b.clone()), crate::jsnum::json_number(*v)]))
                    .collect(),
            ),
        );
    }
    let mut rec = Map::new();
    rec.insert("result".into(), Value::String(trace_key(res)));
    rec.insert("basis".into(), Value::String("paths".to_string()));
    rec.insert("sites".into(), Value::Object(sites));
    let calib = sc.extra.entry("calib".to_string()).or_insert_with(|| Value::Object(Map::new()));
    if !calib.is_object() {
        *calib = Value::Object(Map::new());
    }
    let st = calib
        .as_object_mut()
        .expect("an object")
        .entry("siteTurns".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !st.is_object() {
        *st = Value::Object(Map::new());
    }
    st.as_object_mut().expect("an object").insert(res.period.clone(), Value::Object(rec));
}

fn traced_rows(arms: &[Arm], rows: &[MovementRow], list: &[(String, String, f64)]) -> Vec<ViewRow> {
    let mut in_leg: HashMap<&str, usize> = HashMap::new();
    let mut out_leg: HashMap<&str, usize> = HashMap::new();
    for (i, a) in arms.iter().enumerate() {
        for k in &a.ins {
            in_leg.insert(k.as_str(), i);
        }
        for k in &a.outs {
            out_leg.insert(k.as_str(), i);
        }
    }
    let mut mv_of: HashMap<(String, String), String> = HashMap::new();
    for x in rows {
        mv_of.insert((x.approach.clone(), x.to.clone()), x.mv.clone());
    }
    let mut out = Vec::new();
    for (f, t, v) in list {
        let (Some(&ai), Some(&xi)) = (in_leg.get(f.as_str()), out_leg.get(t.as_str())) else { continue };
        let (a, x) = (&arms[ai], &arms[xi]);
        if a.leg.is_empty() || x.leg.is_empty() || !(*v > 0.0) {
            continue;
        }
        let tc = turn_code(a.o, x.o);
        let mv = if ai == xi { "U".to_string() } else { mv_of.get(&(f.clone(), t.clone())).cloned().unwrap_or_else(|| tc[..1].to_string()) };
        out.push(ViewRow { approach: a.leg.to_string(), to: x.leg.to_string(), mv, turn: tc.to_string(), v: *v });
    }
    out
}

/// `junctionView(sc, s, res, nodeId)`: what a turning count is compared with.
///
/// A node in a junction site (a roundabout ring, a divided-road crossing) is compared as the whole site: its legs and
/// the movements the engine traced along the paths. A node that is one of several making up a junction with no site
/// defined is not scored: one ring node carries only part of each movement. Otherwise the node's own arms and movements.
/// None when the node is missing.
pub fn junction_view(ctx: &Ctx, sc: &Scenario, res: Option<&Res>, node_id: Option<&str>) -> Option<JunctionView> {
    let node = node_id.filter(|n| sc.nodes.contains_key(*n))?.to_string();
    if let Some(site) = sites::of(sc, &node) {
        let rs = res.and_then(|r| r.sites.as_ref()).and_then(|m| m.get(&site.id));
        let mut base = JunctionView {
            kind: "site",
            id: site.id.clone(),
            node: node.clone(),
            members: site.nodes.clone(),
            arms: Vec::new(),
            rows: Vec::new(),
            note: None,
            name: None,
        };
        let Some(rs) = rs else {
            base.note = Some(if res.is_some() {
                format!("Run the model again: the junction site {} is newer than these results.", site.name.as_str().unwrap_or(""))
            } else {
                "Run the model first.".to_string()
            });
            return Some(base);
        };
        base.name = Some(rs.name.clone());
        let r = turns_site_rows(sc, res, ctx.s(), &site.id);
        let paths_basis = res.and_then(|x| x.site_turns_basis.as_deref()) == Some("paths");
        let tr: Option<SiteTraces> = if paths_basis { None } else { site_traces(sc, res, ctx.s()) };
        if !paths_basis && tr.is_none() {
            let many = res.map_or(0, |r| r.zone_ids.len()) > 300;
            base.note = Some(if many {
                "Not scored: movements through a junction site are traced along the model's paths, and models with more than 300 zones do not keep them.".to_string()
            } else {
                "Run the model again in this session to compare turns: movements through a junction site are traced along the model's paths, which are not saved with the project.".to_string()
            });
            return Some(base);
        }
        base.arms = r
            .arms
            .iter()
            .filter(|a| !a.leg.is_empty())
            .map(|a| {
                let mut links: Vec<String> = Vec::new();
                for k in a.outs.iter().chain(a.ins.iter()) {
                    if let Some(l) = lid_of(sc, k)
                        && !links.contains(&l)
                    {
                        links.push(l);
                    }
                }
                ViewArm { in_key: a.leg.to_string(), out_key: a.leg.to_string(), b: a.b, road: a.road.clone(), links }
            })
            .collect();
        base.rows = if let Some(tr) = &tr {
            let list: &[(String, String, f64)] = tr.iter().find(|(id, _)| *id == site.id).map_or(&[], |(_, l)| l.as_slice());
            traced_rows(&r.arms, &r.rows, list)
        } else {
            r.rows
                .iter()
                .map(|m| ViewRow { approach: m.leg.to_string(), to: m.to_leg.to_string(), mv: m.mv.clone(), turn: m.turn.to_string(), v: m.v })
                .collect()
        };
        return Some(base);
    }
    if multi_node(sc, &node) {
        return Some(JunctionView {
            kind: "cluster",
            id: node.clone(),
            node: node.clone(),
            members: vec![node],
            arms: Vec::new(),
            rows: Vec::new(),
            note: Some("Multi-node junction (a roundabout ring or closely spaced nodes): define a junction site in the Junctions tab and run the model again to compare turns. Not scored.".to_string()),
            name: None,
        });
    }
    let list = res.and_then(|r| r.turns.get(&node)).map(|v| v.as_slice()).unwrap_or(&[]);
    let arms: Vec<ViewArm> = crate::turns::arms(sc, &node, list, None)
        .into_iter()
        .filter(|a| !a.conn)
        .map(|a| ViewArm { in_key: a.in_key.clone(), out_key: a.out_key.clone(), b: a.b, road: a.road.clone(), links: a.link.iter().cloned().collect() })
        .collect();
    let rows = if res.is_some() { view_rows_of(&movements(sc, res, ctx.s(), &node)) } else { Vec::new() };
    Some(JunctionView { kind: "node", id: node.clone(), node: node.clone(), members: vec![node], arms, rows, note: None, name: None })
}

/// Where a count's legs sit among the view's arms (`legArms`): leg to arm index, in the order the legs were matched.
#[derive(Clone, Debug, Default)]
pub struct LegMap {
    /// Leg code to arm index.
    pub map: Vec<(String, usize)>,
}

impl LegMap {
    /// `map[leg]`.
    pub fn get(&self, leg: &str) -> Option<usize> {
        self.map.iter().find(|(l, _)| l == leg).map(|(_, i)| *i)
    }
}

/// `legArms(sc, nodeId, s, res, view)`: counted legs onto model arms: outward bearing within 30 degrees, else the leg's road name.
pub fn leg_arms(s: &TurnSite, view: Option<&JunctionView>) -> LegMap {
    let arms: &[ViewArm] = view.map_or(&[], |v| v.arms.as_slice());
    let mut legs: Vec<String> = Vec::new();
    for bin in s.bins.values() {
        for k in bin.keys() {
            let l = k.split('-').next().unwrap_or("").to_string();
            if !legs.contains(&l) {
                legs.push(l);
            }
        }
    }
    let mut map: Vec<(String, usize)> = Vec::new();
    let mut used: HashSet<&str> = HashSet::new();
    struct P {
        l: String,
        a: usize,
        e: f64,
    }
    let mut pairs: Vec<P> = Vec::new();
    for l in &legs {
        for (ai, a) in arms.iter().enumerate() {
            pairs.push(P { l: l.clone(), a: ai, e: ang_diff(a.b, leg_bearing(l)) });
        }
    }
    js_sort_by(&mut pairs, |p, q| p.e - q.e);
    for p in &pairs {
        if p.e <= 30.0 && !map.iter().any(|(l, _)| *l == p.l) && !used.contains(arms[p.a].in_key.as_str()) {
            map.push((p.l.clone(), p.a));
            used.insert(arms[p.a].in_key.as_str());
        }
    }
    for l in &legs {
        if map.iter().any(|(x, _)| x == l) {
            continue;
        }
        let road = s.leg_roads.get(l).filter(|r| r.truthy());
        if let Some(road) = road {
            let nm = norm(&road.js_string());
            if let Some(ai) = arms.iter().position(|x| !used.contains(x.in_key.as_str()) && norm(&x.road) == nm) {
                map.push((l.clone(), ai));
                used.insert(arms[ai].in_key.as_str());
            }
        }
    }
    LegMap { map }
}

fn canon(c: char) -> f64 {
    match c {
        'L' => -90.0,
        'T' => 0.0,
        'R' => 90.0,
        _ => 180.0,
    }
}

/// `exitArm(arms, from, mv)`: the arm a movement leaves by (left-hand traffic: a turn above 0 is a right turn).
pub fn exit_arm(arms: &[ViewArm], from: usize, mv: &str) -> Option<usize> {
    let m0 = mv.chars().next().unwrap_or('T');
    let in_b = (arms[from].b + 180.0) % 360.0;
    let want = canon(m0);
    let (mut best, mut be) = (None, f64::INFINITY);
    for (xi, x) in arms.iter().enumerate() {
        if xi == from && m0 != 'U' {
            continue;
        }
        let t = if xi == from { 180.0 } else { turn(in_b, x.b) };
        let e = (((t - want + 540.0) % 360.0) - 180.0).abs();
        if (classify(in_b, x.b) == m0.to_string() || xi == from) && e < be {
            be = e;
            best = Some(xi);
        }
    }
    best
}

/// What `furness` gives: the matrix, how many rounds it took and whether it converged.
#[derive(Clone, Debug, PartialEq)]
pub struct FurnessResult {
    /// The balanced matrix, row by row.
    pub m: Vec<Vec<f64>>,
    /// Rounds.
    pub it: usize,
    /// The largest relative error left.
    pub err: f64,
    /// It converged.
    pub converged: bool,
    /// The out totals were scaled to the in total first.
    pub outs_scaled: bool,
}

/// `furness(ins, outs, seed, opts)`: Furness turn estimation from leg in and out totals (turning-counts).
///
/// `seed[i][j]` = model turn shares where the model has traffic on that approach, else T 70 / L 15 / R 15 (0 for U).
/// Rows and columns are scaled in turn until the largest relative error is under `tol` (0.1%) or `max_it` (50)
/// rounds. Out totals are scaled to the in total first.
pub fn furness(ins: &[f64], outs: &[f64], seed: &[Vec<f64>], tol: f64, max_it: usize) -> FurnessResult {
    let n = ins.len();
    let mut m: Vec<Vec<f64>> = seed.iter().map(|r| r.iter().map(|&v| math_max(0.0, crate::jsnum::or_default(v, 0.0))).collect()).collect();
    let ti = ins.iter().fold(0.0f64, |a, b| a + b);
    let to = outs.iter().fold(0.0f64, |a, b| a + b);
    let o: Vec<f64> = if to > 0.0 { outs.iter().map(|v| v * ti / to).collect() } else { outs.to_vec() };
    let mut it = 0usize;
    let mut err = f64::INFINITY;
    it += 1;
    while it <= max_it {
        for i in 0..n {
            let r = m[i].iter().fold(0.0f64, |a, b| a + b);
            if r > 0.0 {
                for j in 0..n {
                    m[i][j] *= ins[i] / r;
                }
            }
        }
        for j in 0..n {
            let mut c = 0.0f64;
            for i in 0..n {
                c += m[i][j];
            }
            if c > 0.0 {
                for i in 0..n {
                    m[i][j] *= o[j] / c;
                }
            }
        }
        err = 0.0;
        for i in 0..n {
            let r = m[i].iter().fold(0.0f64, |a, b| a + b);
            if ins[i] > 0.0 {
                err = math_max(err, (r - ins[i]).abs() / ins[i]);
            }
        }
        for j in 0..n {
            let mut c = 0.0f64;
            for i in 0..n {
                c += m[i][j];
            }
            if o[j] > 0.0 {
                err = math_max(err, (c - o[j]).abs() / o[j]);
            }
        }
        if err < tol {
            break;
        }
        it += 1;
    }
    FurnessResult { m, it: it.min(max_it), err, converged: err < tol, outs_scaled: (ti - to).abs() > 1e-6 }
}

/// Turn volumes for a site and period: counted movements, or Furness estimates from leg totals (`siteVolumes`).
#[derive(Clone, Debug)]
pub struct SiteVolumes {
    /// Veh/h by "<leg>-<movement>".
    pub mv: OrderedMap<f64>,
    /// Estimated from leg totals.
    pub estimated: bool,
    /// Balanced with adjacent sites.
    pub balanced: bool,
    /// The Furness run, when estimated.
    pub furness: Option<FurnessResult>,
    /// The legs of the Furness matrix, in the order its rows and columns use (the order the count's movements were first met; empty when not estimated).
    pub legs: Vec<String>,
}

/// `siteVolumes(sc, s, period, res)`: counted turn volumes (the balanced ones when the setting says so), or Furness estimates from leg totals for a count that has only totals. None without a peak hour in the period.
pub fn site_volumes(ctx: &Ctx, sc: &Scenario, s: &TurnSite, period: &str, res: Option<&Res>) -> Option<SiteVolumes> {
    let pk = s.peak.get().and_then(|p| p.get(period))?;
    if !pk.leg_only {
        let bal = if ctx.cfg.use_balanced_on() { s.bal.get().and_then(|b| b.get(period)) } else { None };
        return Some(SiteVolumes { mv: bal.cloned().unwrap_or_else(|| pk.mv.clone()), estimated: false, balanced: bal.is_some(), furness: None, legs: Vec::new() });
    }
    let mut legs: Vec<String> = Vec::new();
    for k in pk.mv.keys() {
        let l = k.split('-').next().unwrap_or("").to_string();
        if !legs.contains(&l) {
            legs.push(l);
        }
    }
    let ins: Vec<f64> = legs.iter().map(|l| pk.mv.get(&format!("{l}-IN")).copied().unwrap_or(0.0)).collect();
    let outs: Vec<f64> = legs.iter().map(|l| pk.mv.get(&format!("{l}-OUT")).copied().unwrap_or(0.0)).collect();
    let node = s.node.as_str().filter(|n| sc.nodes.contains_key(*n));
    let view = node.and_then(|n| junction_view(ctx, sc, res, Some(n)));
    let la = view.as_ref().filter(|v| !v.arms.is_empty()).map(|v| leg_arms(s, Some(v)));
    let rows: &[ViewRow] = match (&view, res) {
        (Some(v), Some(_)) => v.rows.as_slice(),
        _ => &[],
    };
    let seed: Vec<Vec<f64>> = legs
        .iter()
        .enumerate()
        .map(|(i, li)| {
            legs.iter()
                .enumerate()
                .map(|(j, lj)| {
                    let in_h = (leg_bearing(li) + 180.0) % 360.0;
                    let mv = if i == j { "U".to_string() } else { classify(in_h, leg_bearing(lj)).to_string() };
                    if mv == "U" {
                        return 0.0;
                    }
                    let a = la.as_ref().and_then(|l| l.get(li));
                    let x = la.as_ref().and_then(|l| l.get(lj));
                    if let (Some(a), Some(x), Some(v)) = (a, x, &view) {
                        let (ak, xk) = (&v.arms[a].in_key, &v.arms[x].out_key);
                        let tot = rows.iter().filter(|r| r.approach == *ak).fold(0.0f64, |q, r| q + r.v);
                        if tot > 0.0 {
                            return rows.iter().filter(|r| r.approach == *ak && r.to == *xk).fold(0.0f64, |q, r| q + r.v) / tot;
                        }
                    }
                    if mv == "T" { 0.7 } else { 0.15 }
                })
                .collect()
        })
        .collect();
    let f = furness(&ins, &outs, &seed, 0.001, 50);
    let mut mv: OrderedMap<f64> = OrderedMap::new();
    for (i, li) in legs.iter().enumerate() {
        for (j, lj) in legs.iter().enumerate() {
            if !(f.m[i][j] > 0.0) {
                continue;
            }
            let m = if i == j { "U".to_string() } else { classify((leg_bearing(li) + 180.0) % 360.0, leg_bearing(lj)).to_string() };
            let key = format!("{li}-{m}");
            let cur = mv.get(&key).copied().unwrap_or(0.0);
            mv.insert(key, cur + f.m[i][j]);
        }
    }
    Some(SiteVolumes { mv, estimated: true, balanced: false, furness: Some(f), legs })
}

/// One counted turning movement compared with the model (a row of `siteRows`).
#[derive(Clone, Debug, PartialEq)]
pub struct TurnCheckRow {
    /// The turning-count site (its id).
    pub site_id: String,
    /// The leg vehicles come from ("N", "SE").
    pub leg: String,
    /// Movement ("L", "T", "R", "U", "L1" ...).
    pub mv: String,
    /// "<leg>-<movement>".
    pub key: String,
    /// Counted volume in the peak hour (veh/h).
    pub obs: f64,
    /// Modelled volume (veh/h); None when the leg could not be matched to a road.
    pub model: Option<f64>,
    /// GEH.
    pub geh: Option<f64>,
    /// The road of the matched arm.
    pub arm: Option<String>,
    /// The volume was estimated from leg totals.
    pub estimated: bool,
    /// The volume was balanced with adjacent sites.
    pub balanced: bool,
}

/// What `siteRows` gives for one site and period.
#[derive(Clone, Debug)]
pub struct SiteRowsResult {
    /// The movements, counted against modelled.
    pub rows: Vec<TurnCheckRow>,
    /// The matched junction node.
    pub node: Option<String>,
    /// What the count is compared with.
    pub view: Option<JunctionView>,
    /// Things to know (a saturated junction, legs not matched to a road).
    pub flags: Vec<String>,
    /// The turn volumes used.
    pub vol: Option<SiteVolumes>,
    /// Why there are no rows, when there are none.
    pub note: String,
}

fn num_text(x: f64, d: usize) -> String {
    if x.is_finite() { fmt::num(x, d) } else { "\u{2013}".to_string() }
}

/// `siteRows(sc, s, period, res)`: turn validation rows for one site: counted against modelled, per movement.
pub fn site_rows(ctx: &Ctx, sc: &Scenario, s: &TurnSite, period: &str, res: Option<&Res>) -> SiteRowsResult {
    let node: Option<String> = s.node.as_str().filter(|n| sc.nodes.contains_key(*n)).map(String::from);
    let (Some(node), Some(res)) = (node.clone(), res) else {
        return SiteRowsResult {
            rows: Vec::new(),
            note: if node.is_none() { "Not matched to a junction within 40 m.".to_string() } else { "Run the model first.".to_string() },
            node,
            view: None,
            flags: Vec::new(),
            vol: None,
        };
    };
    let Some(vol) = site_volumes(ctx, sc, s, period, Some(res)) else {
        let w = ctx.cfg.window(period).map(|w| format!("{}\u{2013}{}", w[0], w[1])).unwrap_or_default();
        return SiteRowsResult {
            rows: Vec::new(),
            node: Some(node),
            view: None,
            flags: Vec::new(),
            vol: None,
            note: format!("No {period} peak hour in the count (window {w})."),
        };
    };
    let view = junction_view(ctx, sc, Some(res), Some(&node));
    let scored = view.as_ref().is_some_and(|v| !v.arms.is_empty());
    let la = if scored { leg_arms(s, view.as_ref()) } else { LegMap::default() };
    let empty: Vec<ViewRow> = Vec::new();
    let mrows: &[ViewRow] = if scored { &view.as_ref().expect("scored").rows } else { &empty };
    let mut out: Vec<TurnCheckRow> = Vec::new();
    for (k, v) in vol.mv.iter() {
        let mut it = k.split('-');
        let (leg, mv) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
        if mv == "IN" || mv == "OUT" {
            continue;
        }
        let a = la.get(leg).and_then(|i| view.as_ref().map(|v| &v.arms[i]));
        let model = a.map(|a| {
            mrows
                .iter()
                .filter(|r| r.approach == a.in_key && (if mv.chars().count() == 1 { r.mv == mv } else { r.turn == mv }))
                .fold(0.0f64, |q, r| q + r.v)
        });
        let obs = crate::jsnum::or_default(*v, 0.0);
        out.push(TurnCheckRow {
            site_id: s.id.clone(),
            leg: leg.to_string(),
            mv: mv.to_string(),
            key: k.to_string(),
            obs,
            model,
            geh: model.map(|m| geh(m, obs)),
            arm: a.map(|a| a.road.clone()),
            estimated: vol.estimated,
            balanced: vol.balanced,
        });
    }
    let mut flags: Vec<String> = Vec::new();
    let members: Vec<String> = view.as_ref().map_or(vec![node.clone()], |v| v.members.clone());
    let dos = members.iter().fold(0.0f64, |m, n| {
        let d = res.nodes.get(n).map_or(0.0, |x| crate::jsnum::or_default(x.dos, 0.0));
        math_max(m, d)
    });
    if dos > corridor::DOS_V {
        flags.push(format!(
            "Model DoS {} > {}: a stop-line count understates demand here ({})",
            num_text(dos, 2),
            num_text(corridor::DOS_V, 2),
            criteria_source(DOS_SOURCE).map_or("", |s| s.short)
        ));
    }
    if scored {
        let mut un: Vec<&str> = Vec::new();
        for r in out.iter().filter(|r| r.model.is_none()) {
            if !un.contains(&r.leg.as_str()) {
                un.push(r.leg.as_str());
            }
        }
        let all_un: Vec<&str> = out.iter().filter(|r| r.model.is_none()).map(|r| r.leg.as_str()).collect();
        if !all_un.is_empty() {
            flags.push(format!(
                "leg{} {} not matched to a road at the junction, so not scored",
                if un.len() == 1 { "" } else { "s" },
                un.join(", ")
            ));
        }
    }
    js_sort_by(&mut out, |a, b| {
        let la = LEGS.iter().position(|x| *x == a.leg).map_or(-1.0, |i| i as f64);
        let lb = LEGS.iter().position(|x| *x == b.leg).map_or(-1.0, |i| i as f64);
        let d = la - lb;
        if d != 0.0 && !d.is_nan() {
            return d;
        }
        let ma = "LTRU".find(a.mv.chars().next().unwrap_or(' ')).map_or(-1.0, |i| i as f64);
        let mb = "LTRU".find(b.mv.chars().next().unwrap_or(' ')).map_or(-1.0, |i| i as f64);
        ma - mb
    });
    let note = if scored {
        String::new()
    } else {
        match view.as_ref().and_then(|v| v.note.clone()) {
            Some(n) if !n.is_empty() => n,
            _ => "No roads at this junction.".to_string(),
        }
    };
    SiteRowsResult { rows: out, node: Some(node), view, flags, vol: Some(vol), note }
}

/// `turnRows(sc, period, set, area)`: the counted turning movements of the sites in a set ("valid", "calib" or all) and area ("inside", "outside" or all) that were matched to a road, against the model.
pub fn turn_rows(ctx: &Ctx, sc: &Scenario, period: &str, set: &str, area: &str, res: Option<&Res>) -> Vec<TurnCheckRow> {
    let mut out = Vec::new();
    let core = if area == "all" { None } else { Some(core_area(ctx, sc)) };
    for s in &ctx.store.turn_sites {
        if s.role.as_text_is("off") {
            continue;
        }
        if let Some(core) = &core
            && (area == "inside") != in_core(core, &[s.lat.num(), s.lng.num()])
        {
            continue;
        }
        if set == "valid" && !s.role.as_text_is("valid") {
            continue;
        }
        if set == "calib" && s.role.as_text_is("valid") {
            continue;
        }
        for r in site_rows(ctx, sc, s, period, res).rows {
            if r.model.is_some() {
                out.push(r);
            }
        }
    }
    out
}

/// One change balancing made to a counted movement (`audit` entry).
#[derive(Clone, Debug, PartialEq)]
pub struct BalanceAudit {
    /// The site's name.
    pub site: String,
    /// The site's id.
    pub site_id: String,
    /// The movement ("N-T").
    pub movement: String,
    /// The counted volume.
    pub observed: f64,
    /// The balanced volume.
    pub balanced: f64,
    /// The change.
    pub d: f64,
    /// The change as a share of the counted volume.
    pub pct: Option<f64>,
    /// Why ("A \u{2192} B: leaving 340, arriving 410").
    pub reason: String,
    /// The gap is a midblock source or sink.
    pub sink: bool,
}

/// `balance(sc, period, opts)`: balances adjacent turning-count sites (turning-counts).
///
/// The flow leaving site A towards site B (E) and arriving at B from A (A) move to F = (E + A) / 2, or max(E, A) with
/// "no sinks". Movements are scaled proportionally. A gap above max(50 veh/h, 10%) is flagged as a midblock source or
/// sink. The balanced counts and the audit are saved in each site (`bal`, `audit`). Returns the audit and how many
/// sites were considered. Only sites matched to a node with a peak hour (not leg totals only) and arms in the model
/// are balanced; `no_sinks` takes the panel's setting when None.
pub fn balance(p: &mut Project, sc_id: &str, period: &str, no_sinks: Option<bool>, res: Option<&Res>) -> (Vec<BalanceAudit>, usize) {
    match_sites(p, sc_id);
    let ctx = Ctx::new(p);
    let sc = p.scenario(sc_id).expect("scenario");
    let no_sinks = no_sinks.unwrap_or_else(|| ctx.cfg.no_sinks_on());
    let deg = degree(sc);
    let all: Vec<usize> = ctx
        .store
        .turn_sites
        .iter()
        .enumerate()
        .filter(|(_, s)| s.node.as_str().is_some() && s.peak.get().and_then(|k| k.get(period)).is_some_and(|k| !k.leg_only))
        .map(|(i, _)| i)
        .collect();
    let mut info: HashMap<usize, (Option<JunctionView>, LegMap)> = HashMap::new();
    for &i in &all {
        let s = &ctx.store.turn_sites[i];
        let view = junction_view(&ctx, sc, res, s.node.as_str());
        let la = leg_arms(s, view.as_ref());
        info.insert(i, (view, la));
    }
    // only sites compared with a node or a junction site with results (a multi-node junction without a site is left out)
    let sites_: Vec<usize> = all.iter().copied().filter(|i| info[i].0.as_ref().is_some_and(|v| !v.arms.is_empty())).collect();
    let mut by_node: HashMap<String, usize> = HashMap::new();
    for &i in &sites_ {
        for n in &info[&i].0.as_ref().expect("view").members {
            by_node.insert(n.clone(), i);
        }
    }
    let mut work: HashMap<usize, OrderedMap<f64>> =
        sites_.iter().map(|&i| (i, ctx.store.turn_sites[i].peak.get().and_then(|k| k.get(period)).expect("peak").mv.clone())).collect();
    let mut audit: Vec<BalanceAudit> = Vec::new();
    let exits_by = |i: usize, work: &HashMap<usize, OrderedMap<f64>>| -> Vec<(String, usize)> {
        let (view, la) = &info[&i];
        let arms = &view.as_ref().expect("view").arms;
        let mut o = Vec::new();
        for k in work[&i].keys() {
            let mut it = k.split('-');
            let (leg, mv) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
            let Some(a) = la.get(leg) else { continue };
            if let Some(x) = exit_arm(arms, a, mv) {
                o.push((k.to_string(), x));
            }
        }
        o
    };
    for &si in &sites_ {
        let (view_s, _) = &info[&si];
        let view_s = view_s.as_ref().expect("view");
        for (xi, x) in view_s.arms.iter().enumerate() {
            // leave the junction along this arm's road (either carriageway) and walk through plain bends to the next junction
            let mut hit: Option<(usize, String)> = None;
            for id in &x.links {
                let mut l = sc.links.get(id);
                let mut node: Option<String> = l.and_then(|l| {
                    if view_s.members.contains(&l.a) {
                        Some(l.a.clone())
                    } else if view_s.members.contains(&l.b) {
                        Some(l.b.clone())
                    } else {
                        None
                    }
                });
                let (mut far, mut guard) = (0.0f64, 0);
                let mut end: Option<String> = None;
                if node.is_none() {
                    continue;
                }
                while let Some(lk) = l {
                    guard += 1;
                    if guard > 50 {
                        break;
                    }
                    let nd = node.clone().expect("node");
                    let nxt = if lk.a == nd { lk.b.clone() } else { lk.a.clone() };
                    far += link_len(sc, lk);
                    if far > 2000.0 {
                        break;
                    }
                    if by_node.contains_key(&nxt) {
                        end = Some(nxt);
                        break;
                    }
                    if view_s.members.contains(&nxt) || deg.get(&nxt).copied().unwrap_or(0) != 2 {
                        break;
                    }
                    let prev = lk.id.clone();
                    l = links_at(sc, &nxt).into_iter().find(|q| q.id != prev);
                    node = Some(nxt);
                }
                if let Some(e) = &end
                    && by_node[e] != si
                {
                    // the link the walk ended on
                    let lid = l.map(|q| q.id.clone()).unwrap_or_default();
                    hit = Some((by_node[e], lid));
                    break;
                }
            }
            let Some((ti_idx, lid)) = hit else { continue };
            let (view_t, la_t) = &info[&ti_idx];
            let view_t = view_t.as_ref().expect("view");
            let Some(y) = view_t.arms.iter().position(|a| a.links.contains(&lid)) else { continue };
            let Some(leg_t) = la_t.map.iter().find(|(_, a)| *a == y).map(|(l, _)| l.clone()) else { continue };
            let ex = exits_by(si, &work);
            let keys_e: Vec<String> = ex.iter().filter(|(_, a)| *a == xi).map(|(k, _)| k.clone()).collect();
            let keys_a: Vec<String> = work[&ti_idx].keys().filter(|k| k.starts_with(&format!("{leg_t}-"))).map(String::from).collect();
            let e_tot = keys_e.iter().fold(0.0f64, |q, k| q + work[&si].get(k).copied().unwrap_or(f64::NAN));
            let a_tot = keys_a.iter().fold(0.0f64, |q, k| q + work[&ti_idx].get(k).copied().unwrap_or(f64::NAN));
            if !(e_tot > 0.0) || !(a_tot > 0.0) {
                continue;
            }
            let f = if no_sinks { math_max(e_tot, a_tot) } else { (e_tot + a_tot) / 2.0 };
            let gap = (e_tot - a_tot).abs() > math_max(corridor::BALANCE_ABS, corridor::BALANCE_REL * math_max(e_tot, a_tot));
            let name_s = ctx.store.turn_sites[si].name.js_string();
            let name_t = ctx.store.turn_sites[ti_idx].name.js_string();
            let why = format!(
                "{} \u{2192} {}: leaving {}, arriving {}{}",
                name_s,
                name_t,
                fmt::int(e_tot),
                fmt::int(a_tot),
                if gap { "; midblock source or sink, consider a driveway zone" } else { "" }
            );
            for (site, keys, tot) in [(si, &keys_e, e_tot), (ti_idx, &keys_a, a_tot)] {
                for k in keys {
                    let w = work.get_mut(&site).expect("work");
                    let o = w.get(k).copied().unwrap_or(f64::NAN);
                    let b = o * f / tot;
                    w.insert(k.clone(), b);
                    let s_ref = &ctx.store.turn_sites[site];
                    let observed = s_ref.peak.get().and_then(|pk| pk.get(period)).and_then(|pk| pk.mv.get(k)).copied().unwrap_or(f64::NAN);
                    audit.push(BalanceAudit {
                        site: s_ref.name.js_string(),
                        site_id: s_ref.id.clone(),
                        movement: k.clone(),
                        observed,
                        balanced: b,
                        d: b - observed,
                        pct: if observed > 0.0 { Some(b / observed - 1.0) } else { None },
                        reason: why.clone(),
                        sink: gap,
                    });
                }
            }
        }
    }
    let mut st = ctx.store.clone();
    let n_sites = sites_.len();
    for &i in &sites_ {
        let s = &mut st.turn_sites[i];
        let mut bal = s.bal.get().cloned().unwrap_or_default();
        bal.insert(period.to_string(), work[&i].clone());
        s.bal = Maybe::Set(bal);
        let mut aud = match s.audit.get() {
            Some(Value::Object(m)) => m.clone(),
            _ => Map::new(),
        };
        let mine: Vec<Value> = audit
            .iter()
            .filter(|a| a.site_id == s.id)
            .map(|a| {
                let mut m = Map::new();
                m.insert("site".into(), Value::String(a.site.clone()));
                m.insert("siteId".into(), Value::String(a.site_id.clone()));
                m.insert("movement".into(), Value::String(a.movement.clone()));
                m.insert("observed".into(), crate::jsnum::json_number(a.observed));
                m.insert("balanced".into(), crate::jsnum::json_number(a.balanced));
                m.insert("d".into(), crate::jsnum::json_number(a.d));
                m.insert("pct".into(), a.pct.map_or(Value::Null, crate::jsnum::json_number));
                m.insert("reason".into(), Value::String(a.reason.clone()));
                m.insert("sink".into(), Value::Bool(a.sink));
                Value::Object(m)
            })
            .collect();
        aud.insert(period.to_string(), Value::Array(mine));
        s.audit = Maybe::Set(Value::Object(aud));
    }
    drop(ctx);
    st.write(p);
    (audit, n_sites)
}

/// `atJunction(sc, s, id)`: whether a turning-count site is at this node, or at the junction site (id or any of its nodes) the node belongs to.
pub fn at_junction(sc: &Scenario, s: &TurnSite, id: &str) -> bool {
    let n = match s.node.as_str() {
        Some(n) => Some(n.to_string()),
        None => site_node(sc, s),
    };
    let Some(n) = n else { return false };
    if n == id {
        return true;
    }
    sites::of(sc, &n).is_some_and(|st| st.id == id || st.nodes.iter().any(|x| x == id))
}

/// `phf(id, period, sc)`: the peak hour factor of the turning count at a junction (node or junction-site id), for the SIDRA export.
pub fn phf(store: &CalibStore, sc: &Scenario, id: &str, period: &str) -> Option<f64> {
    let s = store.turn_sites.iter().find(|x| {
        !x.role.as_text_is("off") && x.peak.get().and_then(|p| p.get(period)).is_some() && at_junction(sc, x, id)
    })?;
    s.peak.get()?.get(period)?.phf
}
