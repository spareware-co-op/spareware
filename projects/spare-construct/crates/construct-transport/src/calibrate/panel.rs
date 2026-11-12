//! The calibration workflow of the panel: matrix estimation from the counts given the calibration role (`runME`,
//! `reduceME`), the matrix estimation report of Table M1 (`meReport`), the network checks, the stress test and the
//! trip-end factors (`fitTripEnds`).

use super::common::calib_cfg;
use super::criteria::{corridor, tag};
use super::estimate::base_view;
use super::fit::{FitOpts, FitResult, fit, lend};
use super::m1::{M1, M1CellChange, M1Cells, M1Ends, M1EndsBoth, M1Sectors, M1Tld, M1Top, M1TripEndGeh, me_stats};
use super::roles::{Ctx, method_weight};
use super::rows::{CalStats, count_rows, stats_of_count_rows};
use super::screen::sl_rows;
use super::sites::Group;
use crate::assign::assign;
use crate::datahub::{Count, geh, regression};
use crate::demand::{apply_me, demand};
use crate::graph::{Graph, build_graph, js_sort_by};
use crate::jsmath;
use crate::jsnum::{math_max, math_min, usize_to_f64};
use crate::project::{Project, Scenario};
use crate::results::{AssignOpts, Res};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

// A progress callback, called as (pass, passes, message). Private, so the calibrate module's glob re-exports
// cannot clash with another module's alias.
type PanelProgress<'a> = &'a mut dyn FnMut(f64, f64, &str);

fn without_me(p: &Project, period: &str) -> Project {
    let mut q = p.clone();
    if let Some(me) = q.me.get_mut() {
        me.factors.remove(period);
    }
    q
}

/* ---------------- k-means sectors ---------------- */

/// `kmeans(pts, k)`: k-means labels with a deterministic farthest-point start (for the sector matrix).
#[must_use]
pub fn kmeans(pts: &[[f64; 2]], k: usize) -> Vec<usize> {
    let Some(&first) = pts.first() else {
        return Vec::new();
    };
    let d2 = |p: &[f64; 2], c: &[f64; 2]| (p[0] - c[0]) * (p[0] - c[0]) + (p[1] - c[1]) * (p[1] - c[1]);
    let mut c: Vec<[f64; 2]> = vec![first];
    while c.len() < k.min(pts.len()) {
        let (mut bi, mut bd) = (0usize, -1.0f64);
        for (i, p) in pts.iter().enumerate() {
            let d = c.iter().fold(f64::INFINITY, |m, q| math_min(m, d2(p, q)));
            if d > bd {
                bd = d;
                bi = i;
            }
        }
        // bi is a position in pts, so the point is always there
        let Some(&far) = pts.get(bi) else {
            break;
        };
        c.push(far);
    }
    let mut lab = vec![0usize; pts.len()];
    for _ in 0..20 {
        lab = pts
            .iter()
            .map(|p| {
                let (mut b, mut bd) = (0usize, f64::INFINITY);
                for (j, q) in c.iter().enumerate() {
                    let d = d2(p, q);
                    if d < bd {
                        bd = d;
                        b = j;
                    }
                }
                b
            })
            .collect();
        // lab has one label per point, so zipping visits the points in order as indexing did
        for (j, cj) in c.iter_mut().enumerate() {
            let m: Vec<&[f64; 2]> = pts.iter().zip(&lab).filter(|&(_, &lj)| lj == j).map(|(p, _)| p).collect();
            if !m.is_empty() {
                let n = usize_to_f64(m.len());
                *cj = [m.iter().fold(0.0f64, |s, p| s + p[0]) / n, m.iter().fold(0.0f64, |s, p| s + p[1]) / n];
            }
        }
    }
    lab
}

/* ---------------- Table M1 ---------------- */

// x[i] of a matrix or a per-zone vector, 0 past the end (each holds zn * zn cells or zn zones, so every index used is in range)
fn at(x: &[f64], i: usize) -> f64 {
    x.get(i).copied().unwrap_or(0.0)
}

// x[i] += v; a position past the end takes nothing
fn add_at(x: &mut [f64], i: usize, v: f64) {
    if let Some(c) = x.get_mut(i) {
        *c += v;
    }
}

// The index of cell (i, j) of an n by n matrix; past the end (usize::MAX) if it would overflow
fn ij(n: usize, i: usize, j: usize) -> usize {
    i.checked_mul(n).and_then(|r| r.checked_add(j)).unwrap_or(usize::MAX)
}

// x[i] of a flag vector, false past the end (the flags are per zone, so every index used is in range)
fn flag(x: &[bool], i: usize) -> bool {
    x.get(i).copied().unwrap_or(false)
}

// The prior and estimated matrices read cell by cell (the first pass of meReport)
struct CellSums {
    // Σ prior and Σ estimated
    tp: f64,
    te: f64,
    // The cells with a prior: prior and estimated
    cp: Vec<f64>,
    ce: Vec<f64>,
    top: Vec<M1Top>,
    chg: M1CellChange,
    // Trip ends per zone, each [prior, estimated]: origins and destinations
    orig: [Vec<f64>; 2],
    dest: [Vec<f64>; 2],
}

fn cell_sums(prior: &[f64], est: &[f64], zn: usize, name: &dyn Fn(usize) -> String) -> CellSums {
    let (mut tp, mut te) = (0.0f64, 0.0f64);
    let (mut cp, mut ce): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    let mut top: Vec<M1Top> = Vec::new();
    // Bound: the counts are of matrix cells, far below usize::MAX
    let mut chg = M1CellChange { over50: 0, b40: 0, b30: 0, n: 0 };
    let mut orig = [vec![0.0f64; zn], vec![0.0f64; zn]];
    let mut dest = [vec![0.0f64; zn], vec![0.0f64; zn]];
    for i in 0..zn {
        for j in 0..zn {
            let pv = at(prior, ij(zn, i, j));
            let ev = at(est, ij(zn, i, j));
            tp += pv;
            te += ev;
            add_at(&mut orig[0], i, pv);
            add_at(&mut orig[1], i, ev);
            add_at(&mut dest[0], j, pv);
            add_at(&mut dest[1], j, ev);
            if pv > 0.0 {
                cp.push(pv);
                ce.push(ev);
                chg.n = chg.n.saturating_add(1);
                let ratio = (ev / pv - 1.0).abs();
                if ratio > 0.5 {
                    chg.over50 = chg.over50.saturating_add(1);
                } else if ratio > 0.4 {
                    chg.b40 = chg.b40.saturating_add(1);
                } else if ratio > 0.3 {
                    chg.b30 = chg.b30.saturating_add(1);
                }
                if (ev - pv).abs() > 0.05 {
                    top.push(M1Top { from: name(i), to: name(j), prior: pv, est: ev, d: ev - pv, pct: ev / pv - 1.0 });
                }
            }
        }
    }
    CellSums { tp, te, cp, ce, top, chg, orig, dest }
}

// Trip ends (origins and destinations of every zone) per GEH band, prior against estimated
fn trip_end_geh(sums: &CellSums, zn: usize) -> Vec<usize> {
    let gb = [2.5, 5.0, 7.5, 10.0, f64::INFINITY];
    let mut tg = vec![0usize; gb.len()];
    for i in 0..zn {
        let ends = [
            (at(&sums.orig[0], i), at(&sums.orig[1], i)),
            (at(&sums.dest[0], i), at(&sums.dest[1], i)),
        ];
        for (pv, ev) in ends {
            if pv + ev > 0.0 {
                let gv = geh(ev, pv);
                // Bound: two trip ends per zone, far below usize::MAX
                if let Some(slot) = gb.iter().position(|&edge| gv < edge).and_then(|band| tg.get_mut(band)) {
                    *slot = slot.saturating_add(1);
                }
            }
        }
    }
    tg
}

// Trip lengths on the free-flow shortest paths of the base-year graph, prior against estimated
fn trip_lengths(graph: &Graph, prior: &[f64], est: &[f64], zn: usize) -> M1Tld {
    let mut ws = graph.workspace();
    let bins = [2000.0, 5000.0, 10000.0, 20000.0, f64::INFINITY];
    let mut fp = vec![0.0f64; bins.len()];
    let mut fe = vec![0.0f64; bins.len()];
    let (mut sp, mut se, mut sp2, mut se2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..zn {
        let any = (0..zn).any(|j| at(prior, ij(zn, i, j)) > 0.0 || at(est, ij(zn, i, j)) > 0.0);
        if !any {
            continue;
        }
        // every zone has a centroid node
        let Some(&src) = graph.zone_node.get(i) else {
            continue;
        };
        graph.spt(&graph.ffc, src, &mut ws, -1, -1);
        for j in 0..zn {
            let pv = at(prior, ij(zn, i, j));
            let ev = at(est, ij(zn, i, j));
            let Some(&dst) = graph.zone_node.get(j) else {
                continue;
            };
            if !(pv > 0.0 || ev > 0.0) || i == j || !ws.nd.get(dst).is_some_and(|d| d.is_finite()) {
                continue;
            }
            let mut len = 0.0f64;
            // a path is made of the graph's arcs, so every arc is there
            for arc in graph.path_to(&ws, dst).unwrap_or_default() {
                len += graph.arcs.get(arc).map_or(0.0, |a| a.len);
            }
            if let Some(bin) = bins.iter().position(|&edge| len < edge) {
                add_at(&mut fp, bin, pv);
                add_at(&mut fe, bin, ev);
            }
            sp += pv * len;
            se += ev * len;
            sp2 += pv * len * len;
            se2 += ev * len * len;
        }
    }
    let t_p = fp.iter().fold(0.0f64, |acc, v| acc + v);
    let t_e = fe.iter().fold(0.0f64, |acc, v| acc + v);
    let m_p = if t_p == 0.0 { 0.0 } else { sp / t_p };
    let m_e = if t_e == 0.0 { 0.0 } else { se / t_e };
    let sd_p = if t_p == 0.0 { 0.0 } else { math_max(0.0, sp2 / t_p - m_p * m_p).sqrt() };
    let sd_e = if t_e == 0.0 { 0.0 } else { math_max(0.0, se2 / t_e - m_e * m_e).sqrt() };
    let (mut mn, mut mx) = (0.0f64, 0.0f64);
    for (&bp, &be) in fp.iter().zip(&fe) {
        let share_p = if t_p == 0.0 { 0.0 } else { bp / t_p };
        let share_e = if t_e == 0.0 { 0.0 } else { be / t_e };
        mn += math_min(share_p, share_e);
        mx += math_max(share_p, share_e);
    }
    M1Tld {
        bins: ["0\u{2013}2 km", "2\u{2013}5 km", "5\u{2013}10 km", "10\u{2013}20 km", "20+ km"].iter().map(std::string::ToString::to_string).collect(),
        prior: fp.iter().map(|v| if t_p == 0.0 { 0.0 } else { v / t_p }).collect(),
        est: fe.iter().map(|v| if t_e == 0.0 { 0.0 } else { v / t_e }).collect(),
        mean_p: m_p,
        mean_e: m_e,
        sd_p,
        sd_e,
        mean_pct: if m_p == 0.0 { 0.0 } else { (m_e / m_p - 1.0) * 100.0 },
        sd_pct: if sd_p == 0.0 { 0.0 } else { (sd_e / sd_p - 1.0) * 100.0 },
        cr: if mx == 0.0 { None } else { Some(mn / mx) },
        basis: "free-flow shortest path, base-year network".to_string(),
    }
}

// The sector-to-sector check: the gateways together, plus up to 8 k-means sectors of the zones
fn sector_check(graph: &Graph, is_g: &[bool], prior: &[f64], est: &[f64]) -> M1Sectors {
    let zn = graph.zones.len();
    let inner: Vec<(usize, [f64; 2])> = graph
        .zones
        .iter()
        .enumerate()
        .filter(|&(i, _)| !flag(is_g, i))
        .map(|(i, z)| {
            let (la, ln) = (z.lat.num(), z.lng.num());
            (i, [la, ln * jsmath::cos(la * std::f64::consts::PI / 180.0)])
        })
        .collect();
    let pts: Vec<[f64; 2]> = inner.iter().map(|&(_, pt)| pt).collect();
    let lab = kmeans(&pts, 8);
    let mut sec = vec![0usize; zn];
    // kmeans gives one label per point, in the points' order
    for (&(i, _), &label) in inner.iter().zip(&lab) {
        if let Some(slot) = sec.get_mut(i) {
            // Bound: a label is below 8
            *slot = label.saturating_add(1);
        }
    }
    // Bound: at most 8 labels, so n_s is at most 10
    let n_s = lab.iter().copied().max().unwrap_or(0).saturating_add(2);
    let mut sector_prior = vec![0.0f64; n_s.saturating_mul(n_s)];
    let mut sector_est = vec![0.0f64; n_s.saturating_mul(n_s)];
    for i in 0..zn {
        let from = sec.get(i).copied().unwrap_or(0);
        for j in 0..zn {
            let to = sec.get(j).copied().unwrap_or(0);
            add_at(&mut sector_prior, ij(n_s, from, to), at(prior, ij(zn, i, j)));
            add_at(&mut sector_est, ij(n_s, from, to), at(est, ij(zn, i, j)));
        }
    }
    let (mut s_test, mut s_ok) = (0usize, 0usize);
    for (&cell_p, &cell_e) in sector_prior.iter().zip(&sector_est) {
        if cell_p >= corridor::ME_SECTOR_PRIOR {
            s_test = s_test.saturating_add(1);
            if (cell_e / cell_p - 1.0).abs() <= tag::SECTORS_V {
                s_ok = s_ok.saturating_add(1);
            }
        }
    }
    M1Sectors {
        n: n_s,
        tested: s_test,
        within5: s_ok,
        note: "gateways plus up to 8 k-means sectors; cells with a prior of 50 trips or more; report only under TMDG".to_string(),
    }
}

/// `meReport(sc, period)`: Table M1 (TMDG section 8, TAG M3.1 Table 5): the prior matrix against the estimated one on the base-year view, None when the period has no factors.
///
/// Trip lengths use the free-flow shortest path; bins 0 to 2, 2 to 5, 5 to 10, 10 to 20, 20+ km. `date` is the time written into the report.
#[must_use]
pub fn me_report(p: &Project, sc: &Scenario, period: &str, date: &str) -> Option<M1> {
    let has = p.me.get().and_then(|m| m.factors.get(period)).is_some_and(crate::jsnum::Maybe::is_set);
    if !has {
        return None;
    }
    let st = &p.settings;
    let bv = base_view(sc, st);
    let view: &Scenario = &bv.view;
    let graph = build_graph(view, st, None);
    let zn = graph.zones.len();
    let prior = {
        let q = without_me(p, period);
        demand(&q, view, &graph, period, st).t
    };
    let mut est = prior.clone();
    apply_me(p, &graph.zones, period, &mut est, None);
    let is_g: Vec<bool> = graph.zones.iter().map(|z| z.kind.is("gateway")).collect();
    // cell_sums asks only for zones below zn, which are all there
    let name = |i: usize| -> String {
        graph.zones.get(i).map_or_else(String::new, |z| match z.name.as_str() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => z.id.clone(),
        })
    };
    let sums = cell_sums(&prior, &est, zn, &name);
    let reg = regression(&sums.cp, &sums.ce);
    let mean_cell = if sums.cp.is_empty() { 0.0 } else { sums.tp / usize_to_f64(sums.cp.len()) };
    let ends_of = |keep: &dyn Fn(usize) -> bool| -> M1Ends {
        let (mut xs, mut ys): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
        for i in 0..zn {
            if keep(i) {
                xs.push(at(&sums.orig[0], i));
                xs.push(at(&sums.dest[0], i));
                ys.push(at(&sums.orig[1], i));
                ys.push(at(&sums.dest[1], i));
            }
        }
        let r = regression(&xs, &ys);
        M1Ends { slope: r.ols_b, intercept: r.ols_a, r2: r.r2_ols, n: usize_to_f64(xs.len()) / 2.0 }
    };
    let ends = M1EndsBoth { all: ends_of(&|_| true), no_gateways: ends_of(&|i| !flag(&is_g, i)) };
    let tg = trip_end_geh(&sums, zn);
    let tld = trip_lengths(&graph, &prior, &est, zn);
    let sectors = sector_check(&graph, &is_g, &prior, &est);
    let CellSums { tp, te, cp, mut top, chg, .. } = sums;
    js_sort_by(&mut top, |a, b| b.d.abs() - a.d.abs());
    top.truncate(10);
    Some(M1 {
        date: Value::String(date.to_string()),
        period: period.to_string(),
        prior_total: tp,
        total: te,
        total_pct: if tp > 0.0 { (te - tp) / tp * 100.0 } else { 0.0 },
        cells: M1Cells {
            n: cp.len(),
            slope: reg.ols_b,
            intercept: reg.ols_a,
            r2: reg.r2_ols,
            mean_prior: mean_cell,
            intercept_flag: reg.ols_a.is_some_and(|a| a.abs() > corridor::ME_INTERCEPT * mean_cell),
        },
        ends,
        tld,
        trip_end_geh: M1TripEndGeh {
            bands: ["< 2.5", "2.5\u{2013}5", "5\u{2013}7.5", "7.5\u{2013}10", "> 10"].iter().map(std::string::ToString::to_string).collect(),
            n: tg,
        },
        cell_change: chg,
        sectors,
        top,
    })
}

/* ---------------- matrix estimation from the panel ---------------- */

/// What [`run_me`] gives (`runME`'s result): whether estimation ran, what the fit said, and a sentence.
#[derive(Clone, Debug)]
pub struct RunMe {
    /// Estimation ran and its record was completed.
    pub ok: bool,
    /// The fit (None when there were no calibration counts).
    pub out: Option<FitResult>,
    /// A sentence saying what happened.
    pub note: String,
}

/// The options of [`run_me`] (`opts` of `Calibration.runME`).
#[derive(Clone, Copy, Default)]
pub struct RunMeOpts<'a> {
    /// Period (the settings' when None).
    pub period: Option<&'a str>,
    /// XAMAX: the largest site ratio and cumulative factor (the panel's setting when None or 0).
    pub xamax: Option<f64>,
    /// The time written into the records (the clock when None).
    pub date: Option<&'a str>,
}

// A record as JSON with its non-finite numbers as null. The records here (plain structs of numbers, text and
// lists) always serialise, so the null fallback never happens.
fn json_of<T: Serialize>(x: &T) -> Value {
    serde_json::to_value(x).map_or(Value::Null, |mut v| {
        null_non_finite(&mut v);
        v
    })
}

// The counts of one role (scored rows only) and their statistics
fn stats_by_role(rows: &[super::rows::CountRow], role: &str) -> CalStats {
    let sel: Vec<&super::rows::CountRow> = rows.iter().filter(|x| x.scored && x.role == role).collect();
    stats_of_count_rows(&sel)
}

// The period of a run: the option's, else the settings', else AM
fn period_of(p: &Project, opts: &RunMeOpts) -> String {
    match opts.period {
        Some(x) if !x.is_empty() => x.to_string(),
        _ => p.settings.period.as_str().filter(|x| !x.is_empty()).unwrap_or("AM").to_string(),
    }
}

// The calibration screenlines' directions with two or more calibration counts, as fit groups (each fitted as a group total)
fn me_groups(ctx: &Ctx, view: &Scenario, period: &str, res0: &Res, ids: &[String]) -> Vec<Group> {
    let id_set: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut groups: Vec<Group> = Vec::new();
    for sl in ctx.store.screenlines.iter().filter(|x| !x.is_type("core") && !x.is_valid()) {
        let rows = sl_rows(ctx, view, sl, period, Some(res0), true);
        for r in rows {
            let mut g: Vec<String> = Vec::new();
            for id in &r.used {
                if id_set.contains(id.as_str()) && !g.contains(id) {
                    g.push(id.clone());
                }
            }
            if r.dir != 0 && g.len() >= 2 {
                groups.push(Group {
                    id: Some(Value::String(format!("{}:{}", sl.id, r.dir))),
                    label: Some(format!("{} {}", sl.name.js_string(), r.label)),
                    count_ids: g,
                });
            }
        }
    }
    groups
}

fn null_non_finite(v: &mut Value) {
    match v {
        Value::String(s) if s == "NaN" || s == "Infinity" || s == "-Infinity" => *v = Value::Null,
        Value::Array(a) => a.iter_mut().for_each(null_non_finite),
        Value::Object(m) => m.values_mut().for_each(null_non_finite),
        _ => {}
    }
}

/// `runME(sc, opts)`: matrix estimation from the calibration counts only, weighted by observation method.
///
/// Calibration screenlines are fitted as group totals, through traffic (gateway to gateway) keeps the prior when the
/// panel's setting says so, and XAMAX is both the largest site ratio and the largest cumulative factor. The validation
/// and calibration counts' statistics before and after, and Table M1, are added to the fit record. `progress` is called
/// as (pass, passes, message).
///
/// # Errors
///
/// When the project has no scenario `sc_id`.
pub fn run_me(
    p: &mut Project,
    sc_id: &str,
    opts: &RunMeOpts,
    mut progress: Option<PanelProgress<'_>>,
) -> Result<RunMe, String> {
    let sc = p.scenarios.iter().find(|s| s.id == sc_id).cloned().ok_or_else(|| format!("the project has no scenario {sc_id:?}"))?;
    let cfg = calib_cfg(Some(&p.settings));
    let period = period_of(p, opts);
    let xamax = match opts.xamax {
        Some(x) if x != 0.0 && !x.is_nan() => x,
        _ => cfg.xamax_n(),
    };
    let mut prog = |k: f64, n: f64, m: &str| {
        if let Some(f) = progress.as_deref_mut() {
            f(k, n, m);
        }
    };
    let ids: Vec<String> = {
        let ctx = Ctx::new(p);
        ctx.data
            .counts
            .iter()
            .filter(|c| ctx.data.in_use(c) && !c.virtual_.truthy() && ctx.role_of(c, &period, true) == "me")
            .map(|c| c.id.clone())
            .collect()
    };
    if ids.is_empty() {
        return Ok(RunMe {
            ok: false,
            out: None,
            note: "No calibration counts: every count is a gateway, validation or unused count. Give some counts the calibration role first.".to_string(),
        });
    }
    let bv = base_view(&sc, &p.settings);
    let view: &Scenario = &bv.view;
    prog(0.0, 1.0, "Running the base year without matrix estimation");
    let s = p.settings.clone();
    let ao = AssignOpts { period: Some(period.clone()), ..AssignOpts::default() };
    let res0 = {
        // me[period] is set aside for the run and put back
        let q = without_me(p, &period);
        assign(&q, view, &s, &ao, None)
    };
    let (valid_before, calib_before, groups): (CalStats, CalStats, Vec<Group>) = {
        let ctx = Ctx::new(p);
        let groups = me_groups(&ctx, view, &period, &res0, &ids);
        let rows = count_rows(&ctx, view, &period, Some(&res0), true);
        (stats_by_role(&rows, "valid"), stats_by_role(&rows, "me"), groups)
    };
    let weight = |c: &Count, pd: &str| method_weight(c, pd);
    let fo = FitOpts {
        period: Some(&period),
        max_step: Some(xamax),
        max_factor: Some(xamax),
        count_ids: Some(ids.as_slice()),
        weights: true,
        weight: Some(&weight),
        groups: Some(groups.as_slice()),
        freeze_gg: cfg.freeze_gg_on(),
        date: opts.date,
        ..FitOpts::default()
    };
    let out = fit(p, sc_id, &fo, lend(&mut progress))?;
    let has_info = p.me.get().and_then(|m| m.info.get()).and_then(|i| i.get(&period)).is_some_and(crate::jsnum::Maybe::is_set);
    let Some(res) = out.res.as_ref().filter(|_| has_info) else {
        let note = out.note.clone();
        return Ok(RunMe { ok: false, out: Some(out), note });
    };
    let (valid_after, calib_after) = {
        let ctx = Ctx::new(p);
        let rows = count_rows(&ctx, view, &period, Some(res), true);
        (stats_by_role(&rows, "valid"), stats_by_role(&rows, "me"))
    };
    let date = opts.date.map_or_else(|| super::fit::iso_of_ms(crate::util::now_ms()), str::to_string);
    let m1 = me_report(p, &sc, &period, &date);
    if let Some(info) = p.me.get_mut().and_then(|m| m.info.get_mut()).and_then(|i| i.get_mut(&period)).and_then(|i| i.get_mut()) {
        info.extra.insert("countIds".into(), Value::Array(ids.iter().map(|i| Value::String(i.clone())).collect()));
        info.extra.insert("xamax".into(), crate::jsnum::json_number(xamax));
        info.extra.insert("roleAware".into(), Value::Bool(true));
        info.extra.insert("weighted".into(), Value::Bool(true));
        info.extra.insert("groups".into(), Value::from(groups.len() as u64));
        info.extra.insert("freezeGG".into(), Value::Bool(cfg.freeze_gg_on()));
        info.extra.insert("validBefore".into(), json_of(&valid_before));
        info.extra.insert("validAfter".into(), json_of(&valid_after));
        info.extra.insert("calibBefore".into(), json_of(&calib_before));
        info.extra.insert("calibAfter".into(), json_of(&calib_after));
        info.extra.insert("m1".into(), m1.as_ref().map_or(Value::Null, json_of));
    }
    let note = out.note.clone();
    Ok(RunMe { ok: true, out: Some(out), note })
}

/// One attempt of [`reduce_me`].
#[derive(Clone, Debug, PartialEq)]
pub struct Tried {
    /// XAMAX tried.
    pub xamax: f64,
    /// The TAG Table 5 rows passed.
    pub ok: bool,
    /// What the attempt said.
    pub note: String,
}

/// `reduceME(sc, opts)`: matrix estimation with XAMAX 2, then 1.5, 1.25 and 1.1 until the TAG Table 5 rows pass (or an attempt fails to run). Returns the last run and every attempt.
///
/// # Errors
///
/// When the project has no scenario `sc_id` (as [`run_me`]).
pub fn reduce_me(
    p: &mut Project,
    sc_id: &str,
    opts: &RunMeOpts,
    from: Option<f64>,
    mut progress: Option<PanelProgress<'_>>,
) -> Result<(RunMe, Vec<Tried>), String> {
    let period = period_of(p, opts);
    let lim = match from {
        Some(x) if x != 0.0 && !x.is_nan() => x,
        _ => 2.0,
    };
    let steps: Vec<f64> = [2.0, 1.5, 1.25, 1.1].into_iter().filter(|&x| x <= lim).collect();
    let mut tried = Vec::new();
    let mut last: Option<RunMe> = None;
    for x in steps {
        let o = RunMeOpts { period: Some(&period), xamax: Some(x), date: opts.date };
        let r = run_me(p, sc_id, &o, lend(&mut progress))?;
        let m = if r.ok { me_stats(p, &period) } else { None };
        let ok = m.as_ref().is_some_and(|m| m.cells.ok && m.ends.ok && m.tld.as_ref().is_none_or(|t| t.ok));
        tried.push(Tried { xamax: x, ok, note: r.note.clone() });
        let stop = ok || !r.ok;
        last = Some(r);
        if stop {
            break;
        }
    }
    let last = last.unwrap_or(RunMe { ok: false, out: None, note: String::new() });
    Ok((last, tried))
}
