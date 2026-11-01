//! The calibration workflow of the panel: matrix estimation from the counts given the calibration role (`runME`,
//! `reduceME`), the matrix estimation report of Table M1 (`meReport`), the network checks, the stress test and the
//! trip-end factors (`fitTripEnds`).

use super::common::calib_cfg;
use super::criteria::{corridor, tag};
use super::estimate::base_view;
use super::fit::{FitOpts, FitResult, fit, lend};
use super::m1::{
    M1, M1CellChange, M1Cells, M1Ends, M1EndsBoth, M1Sectors, M1Tld, M1Top, M1TripEndGeh, me_stats,
};
use super::roles::{Ctx, method_weight};
use super::rows::{CalStats, count_rows, stats_of_count_rows};
use super::screen::sl_rows;
use super::sites::Group;
use crate::assign::assign;
use crate::datahub::{Count, geh, regression};
use crate::demand::{apply_me, demand};
use crate::graph::{build_graph, js_sort_by};
use crate::jsmath;
use crate::jsnum::{math_max, math_min};
use crate::project::{Project, Scenario};
use crate::results::{AssignOpts, Res};
use serde_json::Value;
use std::collections::HashSet;

fn without_me(p: &Project, period: &str) -> Project {
    let mut q = p.clone();
    if let Some(me) = q.me.get_mut() {
        me.factors.remove(period);
    }
    q
}

/* ---------------- k-means sectors ---------------- */

/// `kmeans(pts, k)`: k-means labels with a deterministic farthest-point start (for the sector matrix).
pub fn kmeans(pts: &[[f64; 2]], k: usize) -> Vec<usize> {
    if pts.is_empty() {
        return Vec::new();
    }
    let d2 =
        |p: &[f64; 2], c: &[f64; 2]| (p[0] - c[0]) * (p[0] - c[0]) + (p[1] - c[1]) * (p[1] - c[1]);
    let mut c: Vec<[f64; 2]> = vec![pts[0]];
    while c.len() < k.min(pts.len()) {
        let (mut bi, mut bd) = (0usize, -1.0f64);
        for (i, p) in pts.iter().enumerate() {
            let d = c.iter().fold(f64::INFINITY, |m, q| math_min(m, d2(p, q)));
            if d > bd {
                bd = d;
                bi = i;
            }
        }
        c.push(pts[bi]);
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
        for j in 0..c.len() {
            let m: Vec<&[f64; 2]> = pts
                .iter()
                .enumerate()
                .filter(|(i, _)| lab[*i] == j)
                .map(|(_, p)| p)
                .collect();
            if !m.is_empty() {
                let n = m.len() as f64;
                c[j] = [
                    m.iter().fold(0.0f64, |s, p| s + p[0]) / n,
                    m.iter().fold(0.0f64, |s, p| s + p[1]) / n,
                ];
            }
        }
    }
    lab
}

/* ---------------- Table M1 ---------------- */

/// `meReport(sc, period)`: Table M1 (TMDG section 8, TAG M3.1 Table 5): the prior matrix against the estimated one on the base-year view, None when the period has no factors.
///
/// Trip lengths use the free-flow shortest path; bins 0 to 2, 2 to 5, 5 to 10, 10 to 20, 20+ km. `date` is the time written into the report.
pub fn me_report(p: &Project, sc: &Scenario, period: &str, date: &str) -> Option<M1> {
    let has =
        p.me.get()
            .and_then(|m| m.factors.get(period))
            .is_some_and(|f| f.is_set());
    if !has {
        return None;
    }
    let s = &p.settings;
    let bv = base_view(sc, s);
    let view: &Scenario = &bv.view;
    let g = build_graph(view, s, None);
    let zn = g.zones.len();
    let prior = {
        let q = without_me(p, period);
        demand(&q, view, &g, period, s).t
    };
    let mut est = prior.clone();
    apply_me(p, &g.zones, period, &mut est, None);
    let is_g: Vec<bool> = g.zones.iter().map(|z| z.kind.is("gateway")).collect();
    let name = |i: usize| -> String {
        let z = &g.zones[i];
        match z.name.as_str() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => z.id.clone(),
        }
    };
    let (mut tp, mut te) = (0.0f64, 0.0f64);
    let (mut cp, mut ce): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    let mut top: Vec<M1Top> = Vec::new();
    let mut chg = M1CellChange {
        over50: 0,
        b40: 0,
        b30: 0,
        n: 0,
    };
    let mut o = [vec![0.0f64; zn], vec![0.0f64; zn]];
    let mut dd = [vec![0.0f64; zn], vec![0.0f64; zn]];
    for i in 0..zn {
        for j in 0..zn {
            let pv = prior[i * zn + j];
            let e = est[i * zn + j];
            tp += pv;
            te += e;
            o[0][i] += pv;
            o[1][i] += e;
            dd[0][j] += pv;
            dd[1][j] += e;
            if pv > 0.0 {
                cp.push(pv);
                ce.push(e);
                chg.n += 1;
                let r = (e / pv - 1.0).abs();
                if r > 0.5 {
                    chg.over50 += 1;
                } else if r > 0.4 {
                    chg.b40 += 1;
                } else if r > 0.3 {
                    chg.b30 += 1;
                }
                if (e - pv).abs() > 0.05 {
                    top.push(M1Top {
                        from: name(i),
                        to: name(j),
                        prior: pv,
                        est: e,
                        d: e - pv,
                        pct: e / pv - 1.0,
                    });
                }
            }
        }
    }
    let reg = regression(&cp, &ce);
    let mean_cell = if !cp.is_empty() {
        tp / cp.len() as f64
    } else {
        0.0
    };
    let ends_of = |keep: &dyn Fn(usize) -> bool| -> M1Ends {
        let (mut x, mut y): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
        for i in 0..zn {
            if keep(i) {
                x.push(o[0][i]);
                x.push(dd[0][i]);
                y.push(o[1][i]);
                y.push(dd[1][i]);
            }
        }
        let r = regression(&x, &y);
        M1Ends {
            slope: r.ols_b,
            intercept: r.ols_a,
            r2: r.r2_ols,
            n: x.len() as f64 / 2.0,
        }
    };
    let gb = [2.5, 5.0, 7.5, 10.0, f64::INFINITY];
    let mut tg = vec![0usize; gb.len()];
    for i in 0..zn {
        for (a, b) in [(o[0][i], o[1][i]), (dd[0][i], dd[1][i])] {
            if a + b > 0.0 {
                let gv = geh(b, a);
                if let Some(k) = gb.iter().position(|&x| gv < x) {
                    tg[k] += 1;
                }
            }
        }
    }
    // trip lengths on the free-flow shortest paths
    let mut w = g.workspace();
    let bins = [2000.0, 5000.0, 10000.0, 20000.0, f64::INFINITY];
    let mut fp = vec![0.0f64; bins.len()];
    let mut fe = vec![0.0f64; bins.len()];
    let (mut sp, mut se, mut sp2, mut se2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..zn {
        let mut any = false;
        for j in 0..zn {
            if prior[i * zn + j] > 0.0 || est[i * zn + j] > 0.0 {
                any = true;
                break;
            }
        }
        if !any {
            continue;
        }
        g.spt(&g.ffc, g.zone_node[i], &mut w, -1, -1);
        for j in 0..zn {
            let pv = prior[i * zn + j];
            let e = est[i * zn + j];
            if !(pv > 0.0 || e > 0.0) || i == j || !w.nd[g.zone_node[j]].is_finite() {
                continue;
            }
            let mut len = 0.0f64;
            for a in g.path_to(&w, g.zone_node[j]).unwrap_or_default() {
                len += g.arcs[a].len;
            }
            if let Some(b) = bins.iter().position(|&x| len < x) {
                fp[b] += pv;
                fe[b] += e;
            }
            sp += pv * len;
            se += e * len;
            sp2 += pv * len * len;
            se2 += e * len * len;
        }
    }
    let t_p = fp.iter().fold(0.0f64, |a, b| a + b);
    let t_e = fe.iter().fold(0.0f64, |a, b| a + b);
    let m_p = if t_p != 0.0 { sp / t_p } else { 0.0 };
    let m_e = if t_e != 0.0 { se / t_e } else { 0.0 };
    let sd_p = if t_p != 0.0 {
        math_max(0.0, sp2 / t_p - m_p * m_p).sqrt()
    } else {
        0.0
    };
    let sd_e = if t_e != 0.0 {
        math_max(0.0, se2 / t_e - m_e * m_e).sqrt()
    } else {
        0.0
    };
    let (mut mn, mut mx) = (0.0f64, 0.0f64);
    for k in 0..bins.len() {
        let a = if t_p != 0.0 { fp[k] / t_p } else { 0.0 };
        let c = if t_e != 0.0 { fe[k] / t_e } else { 0.0 };
        mn += math_min(a, c);
        mx += math_max(a, c);
    }
    // sectors: the gateways together, plus up to 8 k-means sectors of the zones
    let inner: Vec<usize> = (0..zn).filter(|&i| !is_g[i]).collect();
    let pts: Vec<[f64; 2]> = inner
        .iter()
        .map(|&i| {
            let z = &g.zones[i];
            let (la, ln) = (z.lat.num(), z.lng.num());
            [la, ln * jsmath::cos(la * std::f64::consts::PI / 180.0)]
        })
        .collect();
    let lab = kmeans(&pts, 8);
    let mut sec = vec![0usize; zn];
    for (k, &i) in inner.iter().enumerate() {
        sec[i] = lab[k] + 1;
    }
    let n_s = lab.iter().copied().max().unwrap_or(0).max(0) + 2;
    let mut sm_p = vec![0.0f64; n_s * n_s];
    let mut sm_e = vec![0.0f64; n_s * n_s];
    for i in 0..zn {
        for j in 0..zn {
            sm_p[sec[i] * n_s + sec[j]] += prior[i * zn + j];
            sm_e[sec[i] * n_s + sec[j]] += est[i * zn + j];
        }
    }
    let (mut s_test, mut s_ok) = (0usize, 0usize);
    for k in 0..n_s * n_s {
        if sm_p[k] >= corridor::ME_SECTOR_PRIOR {
            s_test += 1;
            if (sm_e[k] / sm_p[k] - 1.0).abs() <= tag::SECTORS_V {
                s_ok += 1;
            }
        }
    }
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
        ends: M1EndsBoth { all: ends_of(&|_| true), no_gateways: ends_of(&|i| !is_g[i]) },
        tld: M1Tld {
            bins: ["0\u{2013}2 km", "2\u{2013}5 km", "5\u{2013}10 km", "10\u{2013}20 km", "20+ km"].iter().map(|s| s.to_string()).collect(),
            prior: fp.iter().map(|v| if t_p != 0.0 { v / t_p } else { 0.0 }).collect(),
            est: fe.iter().map(|v| if t_e != 0.0 { v / t_e } else { 0.0 }).collect(),
            mean_p: m_p,
            mean_e: m_e,
            sd_p,
            sd_e,
            mean_pct: if m_p != 0.0 { (m_e / m_p - 1.0) * 100.0 } else { 0.0 },
            sd_pct: if sd_p != 0.0 { (sd_e / sd_p - 1.0) * 100.0 } else { 0.0 },
            cr: if mx != 0.0 { Some(mn / mx) } else { None },
            basis: "free-flow shortest path, base-year network".to_string(),
        },
        trip_end_geh: M1TripEndGeh {
            bands: ["< 2.5", "2.5\u{2013}5", "5\u{2013}7.5", "7.5\u{2013}10", "> 10"].iter().map(|s| s.to_string()).collect(),
            n: tg,
        },
        cell_change: chg,
        sectors: M1Sectors {
            n: n_s,
            tested: s_test,
            within5: s_ok,
            note: "gateways plus up to 8 k-means sectors; cells with a prior of 50 trips or more; report only under TMDG".to_string(),
        },
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

fn stats_value(s: &CalStats) -> Value {
    serde_json::to_value(s)
        .map(|mut v| {
            null_non_finite(&mut v);
            v
        })
        .expect("stats serialise")
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
pub fn run_me(
    p: &mut Project,
    sc_id: &str,
    opts: &RunMeOpts,
    mut progress: Option<&mut dyn FnMut(f64, f64, &str)>,
) -> Result<RunMe, String> {
    let idx = p
        .scenarios
        .iter()
        .position(|s| s.id == sc_id)
        .ok_or_else(|| format!("the project has no scenario {sc_id:?}"))?;
    let sc = p.scenarios[idx].clone();
    let cfg = calib_cfg(Some(&p.settings));
    let period: String = match opts.period {
        Some(x) if !x.is_empty() => x.to_string(),
        _ => p
            .settings
            .period
            .as_str()
            .filter(|x| !x.is_empty())
            .unwrap_or("AM")
            .to_string(),
    };
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
            .filter(|c| {
                ctx.data.in_use(c) && !c.virtual_.truthy() && ctx.role_of(c, &period, true) == "me"
            })
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
    let ao = AssignOpts {
        period: Some(period.clone()),
        ..AssignOpts::default()
    };
    let res0 = {
        // me[period] is set aside for the run and put back
        let q = without_me(p, &period);
        assign(&q, view, &s, &ao, None)
    };
    let (valid_before, calib_before, groups): (CalStats, CalStats, Vec<Group>) = {
        let ctx = Ctx::new(p);
        let vr = |r: &Res, role: &str| -> CalStats {
            let rows = count_rows(&ctx, view, &period, Some(r), true);
            let sel: Vec<&super::rows::CountRow> =
                rows.iter().filter(|x| x.scored && x.role == role).collect();
            stats_of_count_rows(&sel)
        };
        let id_set: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let mut groups: Vec<Group> = Vec::new();
        for sl in ctx
            .store
            .screenlines
            .iter()
            .filter(|x| !x.is_type("core") && !x.is_valid())
        {
            let rows = sl_rows(&ctx, view, sl, &period, Some(&res0), true);
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
        (vr(&res0, "valid"), vr(&res0, "me"), groups)
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
    let has_info =
        p.me.get()
            .and_then(|m| m.info.get())
            .and_then(|i| i.get(&period))
            .is_some_and(|i| i.is_set());
    let Some(res) = out.res.as_ref().filter(|_| has_info) else {
        let note = out.note.clone();
        return Ok(RunMe {
            ok: false,
            out: Some(out),
            note,
        });
    };
    let (valid_after, calib_after) = {
        let ctx = Ctx::new(p);
        let rows = count_rows(&ctx, view, &period, Some(res), true);
        let sel = |role: &str| -> CalStats {
            let v: Vec<&super::rows::CountRow> =
                rows.iter().filter(|x| x.scored && x.role == role).collect();
            stats_of_count_rows(&v)
        };
        (sel("valid"), sel("me"))
    };
    let date = opts
        .date
        .map(str::to_string)
        .unwrap_or_else(|| super::fit::iso_of_ms(crate::util::now_ms()));
    let m1 = me_report(p, &sc, &period, &date);
    if let Some(info) =
        p.me.get_mut()
            .and_then(|m| m.info.get_mut())
            .and_then(|i| i.get_mut(&period))
            .and_then(|i| i.get_mut())
    {
        info.extra.insert(
            "countIds".into(),
            Value::Array(ids.iter().map(|i| Value::String(i.clone())).collect()),
        );
        info.extra
            .insert("xamax".into(), crate::jsnum::json_number(xamax));
        info.extra.insert("roleAware".into(), Value::Bool(true));
        info.extra.insert("weighted".into(), Value::Bool(true));
        info.extra
            .insert("groups".into(), Value::from(groups.len() as u64));
        info.extra
            .insert("freezeGG".into(), Value::Bool(cfg.freeze_gg_on()));
        info.extra
            .insert("validBefore".into(), stats_value(&valid_before));
        info.extra
            .insert("validAfter".into(), stats_value(&valid_after));
        info.extra
            .insert("calibBefore".into(), stats_value(&calib_before));
        info.extra
            .insert("calibAfter".into(), stats_value(&calib_after));
        info.extra.insert(
            "m1".into(),
            m1.as_ref().map_or(Value::Null, |m| {
                let mut v = serde_json::to_value(m).expect("m1 serialises");
                null_non_finite(&mut v);
                v
            }),
        );
    }
    let note = out.note.clone();
    Ok(RunMe {
        ok: true,
        out: Some(out),
        note,
    })
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
pub fn reduce_me(
    p: &mut Project,
    sc_id: &str,
    opts: &RunMeOpts,
    from: Option<f64>,
    mut progress: Option<&mut dyn FnMut(f64, f64, &str)>,
) -> Result<(RunMe, Vec<Tried>), String> {
    let period: String = match opts.period {
        Some(x) if !x.is_empty() => x.to_string(),
        _ => p
            .settings
            .period
            .as_str()
            .filter(|x| !x.is_empty())
            .unwrap_or("AM")
            .to_string(),
    };
    let lim = match from {
        Some(x) if x != 0.0 && !x.is_nan() => x,
        _ => 2.0,
    };
    let steps: Vec<f64> = [2.0, 1.5, 1.25, 1.1]
        .into_iter()
        .filter(|&x| x <= lim)
        .collect();
    let mut tried = Vec::new();
    let mut last: Option<RunMe> = None;
    for x in steps {
        let o = RunMeOpts {
            period: Some(&period),
            xamax: Some(x),
            date: opts.date,
        };
        let r = run_me(p, sc_id, &o, lend(&mut progress))?;
        let m = if r.ok { me_stats(p, &period) } else { None };
        let ok = m
            .as_ref()
            .is_some_and(|m| m.cells.ok && m.ends.ok && m.tld.as_ref().is_none_or(|t| t.ok));
        tried.push(Tried {
            xamax: x,
            ok,
            note: r.note.clone(),
        });
        let stop = ok || !r.ok;
        last = Some(r);
        if stop {
            break;
        }
    }
    let last = last.unwrap_or(RunMe {
        ok: false,
        out: None,
        note: String::new(),
    });
    Ok((last, tried))
}
