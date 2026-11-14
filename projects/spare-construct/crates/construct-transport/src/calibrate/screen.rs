//! Screenlines and cordons (`Calibration.slCrossings`, `slRows`, `slObservations`, `cordonOD`): the roads a line or a
//! closed area crosses, the counted and modelled totals per direction, and the observations the validation tests use.

use super::common::{dir_of, hhmm, is_mw, point_in_poly};
use super::criteria::tag;
use super::roles::Ctx;
use super::store::Screenline;
use crate::datahub::{Count, period_vol, geh};
use crate::jsnum::{math_max, math_min, math_round, usize_to_f64};
use crate::project::Scenario;
use crate::results::{Res, plain};
use crate::standards::pcu_f;
use crate::util::{LatLng, bearing, flat, r1, r7};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// A place where a link crosses a screenline or cordon boundary (`slCrossings` entry).
#[derive(Clone, Debug, PartialEq)]
pub struct Crossing {
    /// Link id.
    pub link: String,
    /// +1 when the link's a to b direction crosses from the right of the line segment to its left, else -1.
    pub side: i32,
    /// Where they cross.
    pub ll: LatLng,
    /// Heading of the link's a to b direction there (degrees).
    pub b_ab: f64,
    /// A motorway or ramp.
    pub mw: bool,
}

// The least (lo) or greatest x (axis 0) or y (axis 1) of a set of points: Infinity or -Infinity for none, as Math.min() and Math.max() give
fn extent(pts: &[[f64; 2]], axis: usize, lo: bool) -> f64 {
    pts.iter().fold(if lo { f64::INFINITY } else { f64::NEG_INFINITY }, |m, p| {
        // axis is 0 or 1, so the NaN fallback is never used
        let v = p.get(axis).copied().unwrap_or(f64::NAN);
        if lo { math_min(m, v) } else { math_max(m, v) }
    })
}

/// `slCrossings(sc, sl)`: every link crossing a screenline, or the boundary of a cordon or core area (a closed area is read counter-clockwise, so crossing an edge to its left means going inside).
#[must_use]
pub fn sl_crossings(sc: &Scenario, sl: &Screenline) -> Vec<Crossing> {
    let closed = !sl.is_type("screen");
    let mut pts: Vec<LatLng> = sl.pts.iter().map(|p| p.0).collect();
    if pts.len() < 2 {
        return Vec::new();
    }
    if closed && pts.len() >= 3 {
        // each point with the next, the last with the first
        let mut area = 0.0f64;
        for (p, q) in pts.iter().zip(pts.iter().cycle().skip(1)) {
            area += (q[1] - p[1]) * (q[0] + p[0]);
        }
        if area > 0.0 {
            pts.reverse();
        }
        if let Some(&first) = pts.first() {
            pts.push(first);
        }
    }
    // pts has at least two points (checked above)
    let Some(origin) = pts.first() else { return Vec::new() };
    let proj = flat(origin[0]);
    let xy = |ll: &LatLng| [proj.x(ll), proj.y(ll)];
    let line: Vec<[f64; 2]> = pts.iter().map(xy).collect();
    let bx = [extent(&line, 0, true), extent(&line, 0, false), extent(&line, 1, true), extent(&line, 1, false)];
    let mut out = Vec::new();
    for l in sc.links.values() {
        if l.status.is("closed") || !sc.nodes.contains_key(&l.a) || !sc.nodes.contains_key(&l.b) {
            continue;
        }
        let geo: Vec<[f64; 2]> = sc.geom(l).iter().map(xy).collect();
        if extent(&geo, 0, false) < bx[0] || extent(&geo, 0, true) > bx[1] || extent(&geo, 1, false) < bx[2] || extent(&geo, 1, true) > bx[3] {
            continue;
        }
        for link_seg in geo.windows(2) {
            let &[g0, g1] = link_seg else { continue };
            for line_seg in line.windows(2) {
                let &[l0, l1] = line_seg else { continue };
                let dg = [g1[0] - g0[0], g1[1] - g0[1]];
                let sv = [l1[0] - l0[0], l1[1] - l0[1]];
                let den = dg[0] * sv[1] - dg[1] * sv[0];
                if den.abs() < 1e-9 {
                    continue;
                }
                let t = ((l0[0] - g0[0]) * sv[1] - (l0[1] - g0[1]) * sv[0]) / den;
                let u = ((l0[0] - g0[0]) * dg[1] - (l0[1] - g0[1]) * dg[0]) / den;
                if t < 0.0 || t > 1.0 || u < 0.0 || u > 1.0 {
                    continue;
                }
                // + when A to B crosses from the right of the screenline segment to its left
                let side = if sv[0] * dg[1] - sv[1] * dg[0] > 0.0 { 1 } else { -1 };
                let ll = [(g0[1] + t * dg[1]) / proj.ky, (g0[0] + t * dg[0]) / proj.kx];
                out.push(Crossing {
                    link: l.id.clone(),
                    side,
                    ll,
                    b_ab: (dg[0].atan2(dg[1]) * 180.0 / std::f64::consts::PI + 360.0) % 360.0,
                    mw: is_mw(Some(l)),
                });
            }
        }
    }
    out
}

/// `slLeft(sl)`: the heading of the screenline's left side (traffic crossing in the + sense heads this way).
#[must_use]
pub fn sl_left(sl: &Screenline) -> f64 {
    let a = sl.pts.first().map_or([f64::NAN, f64::NAN], |p| p.0);
    let b = sl.pts.last().map_or([f64::NAN, f64::NAN], |p| p.0);
    (bearing(&a, &b) + 270.0) % 360.0
}

/// One direction of a screenline or cordon (a row of `slRows`): counted and modelled totals, GEH and whether every crossing was counted.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors the fixed JSON rows of slRows (ok, w5, w10, w15, notIndependent)"
)]
pub struct SlRow {
    /// "Inbound", "Outbound", a compass direction ("NB") or "Both directions".
    pub label: String,
    /// Crossing arcs.
    pub n: usize,
    /// Crossing arcs with a count.
    pub counted: usize,
    /// Σ counted on counted arcs (veh/h).
    #[serde(rename = "C", with = "plain")]
    pub c: f64,
    /// Σ modelled on counted arcs.
    #[serde(rename = "Mc", with = "plain")]
    pub mc: f64,
    /// Σ modelled on every crossing arc.
    #[serde(rename = "M", with = "plain")]
    pub m: f64,
    /// Share of the modelled flow that is counted.
    #[serde(with = "plain")]
    pub complete: f64,
    /// Complete enough to judge (every crossing, or the share the settings accept).
    pub ok: bool,
    /// GEH of the counted totals; None when both are zero.
    #[serde(with = "plain")]
    pub geh: Option<f64>,
    /// Modelled less counted (veh/h).
    #[serde(with = "plain")]
    pub d: f64,
    /// That as a share of the counted total.
    #[serde(with = "plain")]
    pub pct: Option<f64>,
    /// Within 5% (TAG Table 1).
    pub w5: bool,
    /// Within 10%.
    pub w10: bool,
    /// Within 15%.
    pub w15: bool,
    /// A validation screenline that adds in counts used for calibration or gateways.
    pub not_independent: bool,
    /// Ids of the counts used.
    pub used: Vec<String>,
    /// Arcs counted.
    pub arcs: Vec<String>,
    /// Roles of the counts used.
    pub roles: Vec<&'static str>,
    /// "complete" or "incomplete (3 of 5 counted)".
    pub text: String,
    /// +1, -1, or 0 for both directions.
    pub dir: i32,
}

struct Ent {
    count: std::rc::Rc<Count>,
    v: f64,
    role: &'static str,
}

struct Item {
    key: String,
    m: f64,
    cnt: Option<Ent2>,
}

#[derive(Clone)]
struct Ent2 {
    v: f64,
    role: &'static str,
    id: String,
}

// The counts in use for a period, keyed by arc ("link:ab") or, for two-way counts, by link
type ByKey = HashMap<String, Ent>;

fn ent2(e: &Ent) -> Ent2 {
    Ent2 {
        v: e.v,
        role: e.role,
        id: e.count.id.clone(),
    }
}

// The counts in use for the period with a role: directional ones by arc, two-way ones by link (the first of each)
fn count_entries(ctx: &Ctx, sc: &Scenario, period: &str) -> (ByKey, ByKey) {
    let matched = ctx.matches(sc);
    let mut by_arc: ByKey = HashMap::new();
    let mut by_link: ByKey = HashMap::new();
    for c in &ctx.data.counts {
        if !ctx.data.in_use(c) || c.virtual_.truthy() {
            continue;
        }
        let Some(mt) = matched.get(&c.id) else { continue };
        let Some(pv) = period_vol(c, period, ctx.s()) else { continue };
        let role = ctx.role_of(c, period, false);
        if role == "off" {
            continue;
        }
        let entry = Ent {
            count: c.clone(),
            v: pv.v,
            role,
        };
        if mt.dir == "both" {
            by_link.entry(mt.link.clone()).or_insert(entry);
        } else {
            by_arc.entry(format!("{}:{}", mt.link, mt.dir)).or_insert(entry);
        }
    }
    (by_arc, by_link)
}

// One row of slRows: the counted and modelled totals of a set of crossing arcs (dir 0; the caller sets it)
fn sl_row(label: String, items: &[Item], min_complete: f64, validation: bool) -> SlRow {
    let (mut c_sum, mut mc, mut mm, mut nc) = (0.0f64, 0.0f64, 0.0f64, 0usize);
    let mut used: Vec<&Ent2> = Vec::new();
    for it in items {
        mm += it.m;
        if let Some(cnt) = &it.cnt {
            c_sum += cnt.v;
            mc += it.m;
            // Bound: at most the number of items
            nc = nc.saturating_add(1);
            used.push(cnt);
        }
    }
    let complete = if mm > 0.0 {
        mc / mm
    } else if !items.is_empty() {
        usize_to_f64(nc) / usize_to_f64(items.len())
    } else {
        0.0
    };
    let ok = !items.is_empty() && complete >= min_complete;
    let geh_v = if c_sum + mc > 0.0 { Some(geh(mc, c_sum)) } else { None };
    let diff = mc - c_sum;
    let pct = if c_sum > 0.0 { Some(diff / c_sum) } else { None };
    let within = |x: f64| pct.is_some_and(|p| p.abs() <= x);
    SlRow {
        label,
        n: items.len(),
        counted: nc,
        c: c_sum,
        mc,
        m: mm,
        complete,
        ok,
        geh: geh_v,
        d: diff,
        pct,
        w5: within(tag::SCREEN_WITHIN),
        w10: within(0.10),
        w15: within(0.15),
        not_independent: validation && used.iter().any(|u| u.role != "valid"),
        used: used.iter().map(|u| u.id.clone()).collect(),
        arcs: items
            .iter()
            .filter(|it| it.cnt.is_some())
            .flat_map(|it| {
                if it.key.contains(':') {
                    vec![it.key.clone()]
                } else {
                    vec![format!("{}:ab", it.key), format!("{}:ba", it.key)]
                }
            })
            .collect(),
        roles: used.iter().map(|u| u.role).collect(),
        text: if ok {
            "complete".to_string()
        } else {
            format!("incomplete ({} of {} counted)", nc, items.len())
        },
        dir: 0,
    }
}

// The arcs crossing in one sense (+1 or -1) that have a result, each once, with their directional counts
fn dir_items(xs: &[Crossing], res: &Res, by_arc: &ByKey, sgn: i32) -> Vec<Item> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut items = Vec::new();
    for x in xs {
        let key = format!("{}{}", x.link, if x.side == sgn { ":ab" } else { ":ba" });
        let Some(arc) = res.arcs.get(&key) else { continue };
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key.clone());
        items.push(Item {
            m: arc.v,
            cnt: by_arc.get(&key).map(ent2),
            key,
        });
    }
    items
}

// Both directions: two-way counts against the two-way model, plus directional counts on both arcs of a link
fn both_items(xs: &[Crossing], res: &Res, by_arc: &ByKey, by_link: &ByKey) -> Vec<Item> {
    let mut links: Vec<String> = Vec::new();
    for x in xs {
        if !links.contains(&x.link) {
            links.push(x.link.clone());
        }
    }
    let mut both: Vec<Item> = Vec::new();
    for id in &links {
        let ab = res.arcs.get(&format!("{id}:ab"));
        let ba = res.arcs.get(&format!("{id}:ba"));
        let mm = ab.map_or(0.0, |a| a.v) + ba.map_or(0.0, |b| b.v);
        let mut cnt: Option<Ent2> = by_link.get(id).map(ent2);
        let ca = by_arc.get(&format!("{id}:ab"));
        let cb = by_arc.get(&format!("{id}:ba"));
        if cnt.is_none()
            && (ca.is_some() || ab.is_none())
            && (cb.is_some() || ba.is_none())
            && let Some(first) = ca.or(cb)
        {
            let any_not_valid = [ca, cb].iter().any(|x| x.is_some_and(|x| x.role != "valid"));
            cnt = Some(Ent2 {
                v: ca.map_or(0.0, |x| x.v) + cb.map_or(0.0, |x| x.v),
                role: if any_not_valid { "me" } else { "valid" },
                id: first.count.id.clone(),
            });
        }
        both.push(Item { key: id.clone(), m: mm, cnt });
    }
    both
}

/// `slRows(sc, sl, period, res, opts)`: per direction, Σ counted on counted arcs, Σ modelled on counted arcs and on every crossing arc.
///
/// A two-way count covers both directions only, so it feeds the "Both directions" row. `motorways` false leaves
/// motorway and ramp crossings out. Empty when there is no result.
pub fn sl_rows(ctx: &Ctx, sc: &Scenario, sl: &Screenline, period: &str, res: Option<&Res>, motorways: bool) -> Vec<SlRow> {
    let Some(res) = res else { return Vec::new() };
    let xs: Vec<Crossing> = sl_crossings(sc, sl).into_iter().filter(|x| motorways || !x.mw).collect();
    let (by_arc, by_link) = count_entries(ctx, sc, period);
    let closed = !sl.is_type("screen");
    let left = sl_left(sl);
    let mut rows: Vec<SlRow> = Vec::new();
    let min_complete = math_min(1.0, ctx.cfg.min_complete_n()) - 1e-9;
    let validation = sl.is_valid();
    for sgn in [1, -1] {
        let items = dir_items(&xs, res, &by_arc, sgn);
        if !items.is_empty() {
            let label = if closed {
                if sgn == 1 { "Inbound".to_string() } else { "Outbound".to_string() }
            } else {
                dir_of(if sgn == 1 { left } else { left + 180.0 }).to_string()
            };
            let mut row = sl_row(label, &items, min_complete, validation);
            row.dir = sgn;
            rows.push(row);
        }
    }
    let both = both_items(&xs, res, &by_arc, &by_link);
    if !both.is_empty() {
        rows.push(sl_row("Both directions".to_string(), &both, min_complete, validation));
    }
    rows
}

/// What `slObservations` gives: the screenline totals the validation tests judge, and the shares meeting each criterion.
#[derive(Clone, Debug)]
pub struct SlSummary {
    /// Number of observations.
    pub n: usize,
    /// The observations: (screenline index in the store, row).
    pub rows: Vec<(usize, SlRow)>,
    /// Validation totals left out because they add in calibration or gateway counts.
    pub dropped: Vec<String>,
    /// Observations with GEH < 5, < 7.5, < 10, < 4, within 5%, 10%, 15%.
    pub cnt: [usize; 7],
}

/// The key names of [`SlSummary::cnt`], in order.
pub const SL_KEYS: [&str; 7] = ["sgeh5", "sgeh75", "sgeh10", "sgeh4", "sw5", "sw10", "sw15"];

impl SlSummary {
    /// `s[key]`: the share of observations meeting a criterion (None without observations).
    #[must_use]
    pub fn share(&self, key: &str) -> Option<f64> {
        let i = SL_KEYS.iter().position(|k| *k == key)?;
        // SL_KEYS and cnt have the same length, so this is always there
        let hits = *self.cnt.get(i)?;
        if self.n > 0 { Some(usize_to_f64(hits) / usize_to_f64(self.n)) } else { None }
    }
    /// `s.cnt[key]`.
    #[must_use]
    pub fn cnt_of(&self, key: &str) -> Option<usize> {
        SL_KEYS.iter().position(|k| *k == key).and_then(|i| self.cnt.get(i).copied())
    }
}

/// `slObservations(sc, period, set)`: each complete directional total; a screenline with neither direction complete but a complete two-way total gives that one observation.
///
/// Validation statistics use validation counts only: a total that adds in calibration or gateway counts is left out (and listed).
pub fn sl_observations(ctx: &Ctx, sc: &Scenario, period: &str, set: &str, res: Option<&Res>) -> SlSummary {
    let mut out: Vec<(usize, SlRow)> = Vec::new();
    let mut dropped: Vec<String> = Vec::new();
    for (i, sl) in ctx.store.screenlines.iter().enumerate() {
        if sl.is_type("core") {
            continue;
        }
        if set == "valid" && !sl.is_valid() {
            continue;
        }
        if set == "calib" && sl.is_valid() {
            continue;
        }
        let rows = sl_rows(ctx, sc, sl, period, res, !sl.motorways.is_false());
        let indep = |r: &SlRow| set != "valid" || !r.not_independent;
        for r in &rows {
            if r.ok && !indep(r) {
                dropped.push(format!("{} {}", sl.name.js_string(), r.label));
            }
        }
        let dirs: Vec<&SlRow> = rows.iter().filter(|r| r.dir != 0 && r.ok && indep(r)).collect();
        let both = rows.iter().find(|r| r.dir == 0 && r.ok && indep(r));
        if !dirs.is_empty() {
            for r in dirs {
                out.push((i, r.clone()));
            }
        } else if let Some(b) = both {
            out.push((i, b.clone()));
        }
    }
    // r.geh < 5 with a null GEH reads null as 0
    let g = |r: &SlRow| r.geh.unwrap_or(0.0);
    let k = |f: &dyn Fn(&SlRow) -> bool| out.iter().filter(|(_, r)| f(r)).count();
    let cnt = [
        k(&|r| g(r) < 5.0),
        k(&|r| g(r) < 7.5),
        k(&|r| g(r) < 10.0),
        k(&|r| g(r) < 4.0),
        k(&|r| r.w5),
        k(&|r| r.w10),
        k(&|r| r.w15),
    ];
    SlSummary {
        n: out.len(),
        rows: out,
        dropped,
        cnt,
    }
}

/// The OD matrix of trips across a cordon (`cordonOD`): as PTV Visum ($O), a long CSV and the boundary zones as `GeoJSON`.
#[derive(Clone, Debug)]
pub struct CordonOd {
    /// The matrix in PTV $O format.
    pub o: String,
    /// The matrix as CSV (with a byte order mark).
    pub csv: String,
    /// The boundary zones as `GeoJSON` text.
    pub geojson: String,
    /// Zones in the matrix.
    pub zones: usize,
    /// Boundary zones.
    pub boundary: usize,
    /// Σ trips (veh/h).
    pub trips: f64,
}

// A boundary zone of a cordon: a road where traffic crosses into or out of it
struct Bz {
    id: String,
    name: String,
    ll: Option<LatLng>,
    kind: &'static str,
}

// The boundary zones met so far, in the order met, with their places
struct Boundary<'a> {
    sc: &'a Scenario,
    xs: &'a [Crossing],
    zones: Vec<Bz>,
    idx: HashMap<String, usize>,
}

impl Boundary<'_> {
    // The id of the boundary zone of a crossing arc ("b:link:ab"), added the first time it is met
    fn zone_of(&mut self, key: &str, kind: &'static str) -> String {
        let (sc, xs) = (self.sc, self.xs);
        let id = format!("b:{key}");
        if !self.idx.contains_key(&id) {
            let lid = key.split(':').next().unwrap_or("");
            // every crossing arc is on a link of the scenario, so the link is always there
            let link = sc.links.get(lid);
            let x = link.and_then(|l| xs.iter().find(|q| q.link == l.id));
            let road = match link.and_then(|l| l.name.as_str()) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => format!("{} road", link.map_or("", |l| l.class().label)),
            };
            let dir = dir_of(if key.ends_with(":ab") {
                x.map_or(f64::NAN, |x| x.b_ab)
            } else {
                x.map_or(f64::NAN, |x| x.b_ab) + 180.0
            });
            self.idx.insert(id.clone(), self.zones.len());
            self.zones.push(Bz {
                id: id.clone(),
                name: format!("{} {} ({})", road, if kind == "in" { "in" } else { "out" }, dir),
                ll: x.map(|x| x.ll),
                kind,
            });
        }
        id
    }

    // The name of a boundary zone
    fn name_of(&self, id: &str) -> Option<&str> {
        self.idx.get(id).and_then(|&i| self.zones.get(i)).map(|b| b.name.as_str())
    }
}

// The trips across a cordon, traced along the result's stored paths and keyed "o\0d" in the order first met, with the
// boundary zones they use
fn cordon_trips<'a>(
    ctx: &Ctx,
    sc: &'a Scenario,
    res: &Res,
    sl: &Screenline,
    xs: &'a [Crossing],
    paths: &[crate::results::OdPaths],
    arc_keys: &[String],
) -> (Vec<(String, f64)>, Boundary<'a>) {
    let mut dir_of_arc: HashMap<String, &'static str> = HashMap::new();
    for x in xs {
        dir_of_arc.insert(format!("{}:ab", x.link), if x.side == 1 { "in" } else { "out" });
        dir_of_arc.insert(format!("{}:ba", x.link), if x.side == 1 { "out" } else { "in" });
    }
    let poly: Vec<LatLng> = sl.pts.iter().map(|p| p.0).collect();
    let inside = |id: &str| -> bool { sc.zones.get(id).is_some_and(|z| point_in_poly(&z.ll(), &poly)) };
    let pf = pcu_f(ctx.s());
    let mut t_order: Vec<(String, f64)> = Vec::new();
    let mut t_idx: HashMap<String, usize> = HashMap::new();
    let mut boundary = Boundary { sc, xs, zones: Vec::new(), idx: HashMap::new() };
    for od in paths {
        // a path's zones are places in the result's zone list
        let (Some(zi), Some(zj)) = (res.zone_ids.get(od.i), res.zone_ids.get(od.j)) else { continue };
        let (ii, jj) = (inside(zi), inside(zj));
        for path in &od.list {
            let mut from: Option<String> = if ii { Some(zi.clone()) } else { None };
            let mut to: Option<String> = if jj { Some(zj.clone()) } else { None };
            let mut seen = ii;
            for &a in &path.arcs {
                // a path's arcs are places in the result's arc keys
                let Some(k) = arc_keys.get(a) else { continue };
                let Some(dir) = dir_of_arc.get(k).copied() else { continue };
                if dir == "in" && from.is_none() {
                    from = Some(boundary.zone_of(k, "in"));
                    seen = true;
                }
                if dir == "out" && seen && !jj {
                    to = Some(boundary.zone_of(k, "out"));
                }
            }
            let (Some(from), Some(to)) = (from, to) else { continue };
            if from == to {
                continue;
            }
            let key = format!("{from}\u{0}{to}");
            let add = path.w / pf;
            let slot = match t_idx.get(&key) {
                Some(&i) => t_order.get_mut(i),
                None => None,
            };
            if let Some(entry) = slot {
                entry.1 += add;
            } else {
                t_idx.insert(key.clone(), t_order.len());
                t_order.push((key, add));
            }
        }
    }
    (t_order, boundary)
}

// The zones the trips use, ordinary zones before boundary zones, each group in string order
fn cordon_zones(t_order: &[(String, f64)]) -> Vec<String> {
    let mut zones: Vec<String> = Vec::new();
    for (k, _) in t_order {
        for z in k.split('\u{0}') {
            if !zones.iter().any(|x| x == z) {
                zones.push(z.to_string());
            }
        }
    }
    crate::graph::js_sort_by(&mut zones, |a, b| {
        let ab = f64::from(i32::from(a.starts_with("b:")));
        let bb = f64::from(i32::from(b.starts_with("b:")));
        let d = ab - bb;
        if d == 0.0 {
            js_locale_compare(a, b)
        } else {
            d
        }
    });
    zones
}

// The zone numbers of the matrix: ordinary zones from 1, boundary zones from 1001
fn cordon_numbers(zones: &[String]) -> HashMap<String, usize> {
    let mut num0: HashMap<String, usize> = HashMap::new();
    let (mut ni, mut nb) = (1usize, 1001usize);
    // Bound: counts of zones in memory, far below usize::MAX
    for z in zones {
        if z.starts_with("b:") {
            num0.insert(z.clone(), nb);
            nb = nb.saturating_add(1);
        } else {
            num0.insert(z.clone(), ni);
            ni = ni.saturating_add(1);
        }
    }
    num0
}

// `num0[z]`: every zone a trip uses is numbered, so the 0 is never used
fn num_of(num0: &HashMap<String, usize>, z: &str) -> usize {
    num0.get(z).copied().unwrap_or(0)
}

/// `cordonOD(sc, res, sl)`: trips between the zones inside a cordon, the zones outside it and the points where traffic crosses its boundary, traced along the result's stored paths.
///
/// # Errors
///
/// The prototype's message when the cordon or the result's stored paths are missing.
pub fn cordon_od(ctx: &Ctx, sc: &Scenario, res: Option<&Res>, sl: Option<&Screenline>) -> Result<CordonOd, String> {
    let Some(sl) = sl.filter(|s| s.is_type("cordon") && s.pts.len() >= 3) else {
        return Err("Draw a cordon (a closed area) first.".to_string());
    };
    let (Some(res), true) = (res, true) else {
        return Err("Run the model again in this session: the cordon export traces the stored paths.".to_string());
    };
    let (Some(paths), Some(arc_keys)) = (res.paths.get(), res.arc_keys.as_ref()) else {
        return Err("Run the model again in this session: the cordon export traces the stored paths.".to_string());
    };
    let xs = sl_crossings(sc, sl);
    let (t_order, boundary) = cordon_trips(ctx, sc, res, sl, &xs, paths, arc_keys);
    let zones = cordon_zones(&t_order);
    let num0 = cordon_numbers(&zones);
    let name_of = |z: &str| -> String {
        if z.starts_with("b:") {
            // every "b:" zone was added to the boundary zones when it was met
            boundary.name_of(z).unwrap_or(z).to_string()
        } else {
            match sc.zones.get(z) {
                Some(zz) if zz.name.as_str().is_some_and(|n| !n.is_empty()) => zz.name.str_or("").to_string(),
                _ => z.to_string(),
            }
        }
    };
    let per = res.period.as_str();
    let hours = if per == "PM" { "16.00 17.00" } else { "7.00 8.00" };
    let mut lines: Vec<String> = vec![
        "$O;D2".to_string(),
        "* From  To".to_string(),
        hours.to_string(),
        "* Factor".to_string(),
        "1.00".to_string(),
        format!("* Spare Construct {per} peak hour, veh/h, cordon \u{201c}{}\u{201d} (peak-hour times indicative)", sl.name.js_string()),
    ];
    let mut sorted: Vec<(String, f64)> = t_order.clone();
    crate::graph::js_sort_by(&mut sorted, |a, b| default_sort_cmp(&entry_text(a), &entry_text(b)));
    for (k, v) in &sorted {
        let mut it = k.split('\u{0}');
        let (o, d) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
        if *v >= 0.005 {
            lines.push(format!("{} {} {}", num_of(&num0, o), num_of(&num0, d), crate::jsnum::to_fixed(*v, 2)));
        }
    }
    lines.push("$NAMES".to_string());
    for z in &zones {
        lines.push(format!("{} \"{}\"", num_of(&num0, z), name_of(z).replace('"', "'")));
    }
    let mut csv_rows: Vec<Vec<Value>> = vec![["from_id", "from_name", "to_id", "to_name", "veh_h"].iter().map(|s| Value::from(*s)).collect()];
    for (k, v) in &sorted {
        let mut it = k.split('\u{0}');
        let (o, d) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
        csv_rows.push(vec![
            Value::from(num_of(&num0, o)),
            Value::from(name_of(o)),
            Value::from(num_of(&num0, d)),
            Value::from(name_of(d)),
            crate::jsnum::json_number(r1(*v)),
        ]);
    }
    let csv = format!("\u{feff}{}", crate::util::csv(&csv_rows));
    let features: Vec<Value> = boundary
        .zones
        .iter()
        .filter(|b| b.ll.is_some() && num0.contains_key(&b.id))
        .filter_map(|b| {
            let ll = b.ll?;
            Some(serde_json::json!({ "type": "Feature", "geometry": { "type": "Point", "coordinates": [crate::jsnum::json_number(r7(ll[1])), crate::jsnum::json_number(r7(ll[0]))] },
                "properties": { "zone": num_of(&num0, &b.id), "name": b.name, "kind": b.kind } }))
        })
        .collect();
    // Writing a JSON value as text cannot fail
    let geojson = serde_json::to_string(&serde_json::json!({ "type": "FeatureCollection", "features": features })).unwrap_or_default();
    Ok(CordonOd {
        o: format!("{}\r\n", lines.join("\r\n")),
        csv,
        geojson,
        zones: zones.len(),
        boundary: boundary.zones.len(),
        trips: t_order.iter().fold(0.0f64, |a, b| a + b.1),
    })
}

// `[k, v]` sorted with the default sort: by the text of the pair ("key,value")
fn entry_text(e: &(String, f64)) -> String {
    format!("{},{}", e.0, crate::jsnum::number_to_js_string(e.1))
}

fn default_sort_cmp(a: &str, b: &str) -> f64 {
    match a.encode_utf16().cmp(b.encode_utf16()) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Greater => 1.0,
        std::cmp::Ordering::Equal => 0.0,
    }
}

// `a.localeCompare(b)` for the ASCII ids and names used here: letters compare without regard to case first, then lower case before upper
fn js_locale_compare(a: &str, b: &str) -> f64 {
    let key = |s: &str| s.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect::<String>();
    match key(a).cmp(&key(b)) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Greater => 1.0,
        std::cmp::Ordering::Equal => match a.cmp(b) {
            std::cmp::Ordering::Equal => 0.0,
            // lower case sorts before upper case in the ICU collation
            std::cmp::Ordering::Less => 1.0,
            std::cmp::Ordering::Greater => -1.0,
        },
    }
}

#[allow(dead_code)]
fn _keep() -> f64 {
    math_round(usize_to_f64(hhmm(0.0).len()))
}
