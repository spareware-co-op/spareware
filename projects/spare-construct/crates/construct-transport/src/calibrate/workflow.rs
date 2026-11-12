//! Calibrate in order (calibrate-workflow): network checks, the stress test, and trip-end factors fitted to the
//! calibration counts and applied to existing land use (`networkChecks`, `stressTest`, `tripEndFactors`, `canApply`,
//! `applyFactors`, `revertFactors`).

use super::common::{dir_of, is_mw};
use super::criteria::{corridor, panel_method};
use super::estimate::base_view;
use super::fit::{Progress, stored_res};
use super::roles::Ctx;
use super::rows::{CountRow, count_rows};
use super::screen::sl_rows;
use crate::assign::assign;
use crate::demand::trip_ends;
use crate::edit::net::{degree, links_at, nearest_node};
use crate::graph::{build_graph, js_sort_by};
use crate::jsnum::{Maybe, Num, math_max, math_round, usize_to_f64, value_to_number, value_truthy};
use crate::project::{LANDUSES, OrderedMap, Project, Scenario, Settings};
use crate::results::{AssignOpts, Res};
use crate::util::{bearing, clamp};
use serde_json::{Map, Value};
use std::collections::HashMap;

fn without_me_lu(p: &Project, period: &str) -> Project {
    let mut q = p.clone();
    if let Some(me) = q.me.get_mut() {
        me.factors.remove(period);
    }
    q
}

/* ---------------- network checks ---------------- */

/// A road direction over capacity with no junction delay (`networkChecks().noDelay` entry).
#[derive(Clone, Debug, PartialEq)]
pub struct NoDelay {
    /// Arc key.
    pub key: String,
    /// The link id.
    pub link: String,
    /// Volume over capacity.
    pub vc: f64,
}

/// What `networkChecks` finds: counts far from their road, the worst fits, zones that connect only to a motorway, roads over capacity with no junction delay.
#[derive(Clone, Debug)]
pub struct NetworkChecks {
    /// Counts matched more than 30 m from the road (first eight).
    pub far: Vec<CountRow>,
    /// The five worst fits.
    pub worst: Vec<CountRow>,
    /// Ids of zones that would connect only to a motorway or ramp.
    pub mw_zones: Vec<String>,
    /// Roads over capacity without junction delay, worst first.
    pub no_delay: Vec<NoDelay>,
}

/// `networkChecks(sc, period)`: things to fix in the network before calibrating.
pub fn network_checks(ctx: &Ctx, sc: &Scenario, period: &str, res: Option<&Res>) -> NetworkChecks {
    let rows = count_rows(ctx, sc, period, res, false);
    let far: Vec<CountRow> = rows.iter().filter(|r| r.mt.d > 30.0).take(8).cloned().collect();
    let worst: Vec<CountRow> = rows.iter().filter(|r| r.scored).take(5).cloned().collect();
    let deg = degree(sc);
    let mut mw_zones = Vec::new();
    for z in sc.zones.values() {
        if z.kind.is("gateway") {
            continue;
        }
        let near = nearest_node(sc, z.ll(), 2000.0, Some(&|n: &crate::project::Node| deg.get(&n.id).copied().unwrap_or(0) > 0));
        if let Some(near) = near {
            let l = links_at(sc, &near);
            if !l.is_empty() && l.iter().all(|x| is_mw(Some(x))) {
                mw_zones.push(z.id.clone());
            }
        }
    }
    let mut no_delay: Vec<NoDelay> = Vec::new();
    if let Some(res) = res {
        for (k, a) in res.arcs.iter() {
            if a.vc > 1.0 && (a.nd <= 0.0 || a.nd.is_nan()) {
                let lid = k.split(':').next().unwrap_or("");
                if sc.links.contains_key(lid) {
                    no_delay.push(NoDelay { key: k.to_string(), link: lid.to_string(), vc: a.vc });
                }
            }
        }
    }
    js_sort_by(&mut no_delay, |a, b| b.vc - a.vc);
    NetworkChecks { far, worst, mw_zones, no_delay }
}

/* ---------------- stress test ---------------- */

/// A junction's degree of saturation at one demand factor (`stressTest().nodes[].runs` entry).
#[derive(Clone, Debug, PartialEq)]
pub struct StressRun {
    /// Demand factor.
    pub f: f64,
    /// Degree of saturation (None when the junction has no result).
    pub dos: Option<f64>,
    /// Rise over the base run's.
    pub rise: Option<f64>,
}

/// A junction in the stress test.
#[derive(Clone, Debug, PartialEq)]
pub struct StressNode {
    /// Node id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Degree of saturation in the base run.
    pub dos0: f64,
    /// Each stressed run.
    pub runs: Vec<StressRun>,
    /// The largest rise per unit of demand increase.
    pub sens: f64,
    /// `DoS` rises more than twice the demand increase.
    pub flag: bool,
}

/// A road direction over capacity with no junction delay in some run.
#[derive(Clone, Debug, PartialEq)]
pub struct StressLink {
    /// Arc key.
    pub key: String,
    /// Link id.
    pub link: String,
    /// Volume over capacity.
    pub vc: f64,
    /// The demand factor of the first run that showed it.
    pub f: f64,
}

/// What `stressTest` gives: junctions whose saturation rises faster than demand, and roads over capacity.
#[derive(Clone, Debug)]
pub struct StressState {
    /// Scenario id.
    pub sc_id: String,
    /// Period.
    pub period: String,
    /// Junctions, most sensitive first.
    pub nodes: Vec<StressNode>,
    /// Roads over capacity with no junction delay.
    pub links: Vec<StressLink>,
    /// The demand factors run.
    pub factors: Vec<f64>,
}

/// `Kit.junctionName(id, sc)`: the road names at a junction ("A / B / C"), or "Unnamed junction".
#[must_use]
pub fn junction_name(sc: &Scenario, id: &str) -> String {
    let mut names: Vec<String> = Vec::new();
    for l in links_at(sc, id) {
        if let Some(n) = l.name.as_str()
            && !n.is_empty()
            && !names.iter().any(|x| x == n)
        {
            names.push(n.to_string());
        }
    }
    if names.is_empty() {
        "Unnamed junction".to_string()
    } else {
        names.truncate(3);
        names.join(" / ")
    }
}

/// `stressTest(sc, opts)`: assigns the scenario at demand times 1.1 and times 1.2 (or the factors given).
///
/// A junction is flagged when its degree of saturation rises by more than twice the demand increase; a road when V/C is
/// over 1 with no junction delay. The base result is the scenario's own when it is not out of date, else a fresh run.
///
/// # Errors
///
/// When the project has no scenario `sc_id`.
pub fn stress_test(
    p: &Project,
    sc_id: &str,
    period: Option<&str>,
    factors: Option<&[f64]>,
    mut progress: Option<Progress<'_>>,
) -> Result<StressState, String> {
    let sc = p.scenario(sc_id).ok_or_else(|| format!("the project has no scenario {sc_id:?}"))?;
    let s = &p.settings;
    let period = match period {
        Some(x) if !x.is_empty() => x.to_string(),
        _ => s.period.as_str().filter(|x| !x.is_empty()).unwrap_or("AM").to_string(),
    };
    let factors: Vec<f64> = factors.map_or_else(|| corridor::STRESS_FACTORS.to_vec(), <[f64]>::to_vec);
    let ao = |df: Option<f64>| AssignOpts {
        period: Some(period.clone()),
        demand_factor: df.map_or(Maybe::Missing, Num::number),
        ..AssignOpts::default()
    };
    let base = match stored_res(sc, &period) {
        Some(r) if !r.stale => r,
        _ => assign(p, sc, s, &ao(None), None),
    };
    let mut runs: Vec<(f64, Res)> = Vec::new();
    for (k, &f) in factors.iter().enumerate() {
        if let Some(cb) = progress.as_deref_mut() {
            cb(usize_to_f64(k), usize_to_f64(factors.len()), &format!("Assigning demand \u{d7} {}", crate::jsnum::number_to_js_string(f)));
        }
        runs.push((f, assign(p, sc, s, &ao(Some(f)), None)));
    }
    let mut nodes: Vec<StressNode> = Vec::new();
    for (id, n0) in base.nodes.iter() {
        if n0.dos <= 0.05 || n0.dos.is_nan() {
            continue;
        }
        let r: Vec<StressRun> = runs
            .iter()
            .map(|(f, res)| {
                let n = res.nodes.get(id);
                StressRun { f: *f, dos: n.map(|n| n.dos), rise: n.map(|n| n.dos / n0.dos - 1.0) }
            })
            .collect();
        let sens = r.iter().fold(f64::NEG_INFINITY, |m, x| math_max(m, x.rise.map_or(0.0, |rise| rise / (x.f - 1.0))));
        nodes.push(StressNode {
            id: id.to_string(),
            name: junction_name(sc, id),
            dos0: n0.dos,
            sens,
            flag: r.iter().any(|x| x.rise.is_some_and(|rise| rise > corridor::STRESS_RISE * (x.f - 1.0))),
            runs: r,
        });
    }
    js_sort_by(&mut nodes, |a, b| b.sens - a.sens);
    let mut links: Vec<StressLink> = Vec::new();
    let mut all: Vec<(f64, &Res)> = vec![(1.0, &base)];
    all.extend(runs.iter().map(|(f, r)| (*f, r)));
    for (f, res) in all {
        for (k, a) in res.arcs.iter() {
            if a.vc > 1.0 && (a.nd <= 0.0 || a.nd.is_nan()) {
                let lid = k.split(':').next().unwrap_or("");
                if sc.links.contains_key(lid) && !links.iter().any(|l| l.key == k) {
                    links.push(StressLink { key: k.to_string(), link: lid.to_string(), vc: a.vc, f });
                }
            }
        }
    }
    Ok(StressState { sc_id: sc_id.to_string(), period, nodes, links, factors })
}

/* ---------------- trip-end factors ---------------- */

/// The options of [`trip_end_factors`].
#[derive(Clone, Copy, Default)]
pub struct TefOpts<'a> {
    /// Period (the settings' when None).
    pub period: Option<&'a str>,
    /// The time written into the record (the clock when None).
    pub date: Option<&'a str>,
}

/// What [`trip_end_factors`] gives: the record `sc.calib.tripEndFactors[period]` (None when nothing could be fitted) and a sentence.
#[derive(Clone, Debug, PartialEq)]
pub struct TefResult {
    /// A record was made.
    pub ok: bool,
    /// Why not, when not.
    pub note: String,
    /// The record: period, date, basis, groups, factors, tried, objBefore, objAfter, objLinear, kept, nObs, rmsBefore, rmsAfter, applied, countIds, bounds, note, atBound.
    pub rec: Option<Value>,
}

struct Obs {
    arcs: Vec<String>,
    c: f64,
    w: f64,
    ids: Vec<String>,
}

// A group of counts with the same road class and compass sector (when there are no calibration screenlines)
struct Grp {
    arcs: Vec<String>,
    c: f64,
    wsum: f64,
    n: usize,
    ids: Vec<String>,
}

// A factor per group
type Factors = HashMap<String, f64>;

// The change in each observation's volume per unit of each group's factor
type Gradients = HashMap<String, Vec<f64>>;

fn rate_scaled(s: &mut Settings, key: &str, period: &str, f: f64) {
    if let Some(rates) = s.rates.get_mut()
        && let Some(Maybe::Set(pr)) = rates.get_mut(key)
        && let Some(Maybe::Set(tr)) = pr.get_mut(period)
    {
        tr.rate = Num::number(tr.rate.num() * f);
    }
}

fn lu_label(g: &str) -> String {
    if g == "gateways" {
        "estimated gateways".to_string()
    } else {
        LANDUSES.iter().find(|l| l.key == g).map_or(g.to_string(), |l| l.label.to_lowercase())
    }
}

// `f[g]`: NaN when missing (every map of factors here holds every group)
fn factor(f: &Factors, g: &str) -> f64 {
    f.get(g).copied().unwrap_or(f64::NAN)
}

// `grad[g][i]`: NaN when missing (every group has a gradient for every observation)
fn grad_at(grad: &Gradients, g: &str, i: usize) -> f64 {
    grad.get(g).and_then(|v| v.get(i)).copied().unwrap_or(f64::NAN)
}

// `m[i]`: NaN past the end (every vector here has one value per observation)
fn at(m: &[f64], i: usize) -> f64 {
    m.get(i).copied().unwrap_or(f64::NAN)
}

// The trips of each main land use (and of the estimated gateways) over the zones of the view, from the trip rates as they are
fn land_use_totals(prior: &Project, view: &Scenario, s0: &Settings, period: &str) -> OrderedMap<f64> {
    let graph = build_graph(view, s0, None);
    let mut tot: OrderedMap<f64> = OrderedMap::new();
    for z in &graph.zones {
        if z.kind.is("gateway") {
            if !z.gw_src.get().is_some_and(value_truthy) {
                let ends = trip_ends(prior, z, s0, period, view);
                let cur = tot.get("gateways").copied().unwrap_or(0.0);
                tot.insert("gateways".to_string(), cur + ends.o + ends.d);
            }
            continue;
        }
        for lu in &LANDUSES {
            let qv = z.lu.get().map_or(f64::NAN, |m| m.num_at(lu.key).num());
            let qv = crate::jsnum::or_default(qv, 0.0);
            let rate = s0.rates.get().and_then(|r| r.get(lu.key)).and_then(|r| r.get()).and_then(|r| r.get(period)).and_then(|r| r.get());
            if qv != 0.0
                && let Some(rate) = rate
            {
                let cur = tot.get(lu.key).copied().unwrap_or(0.0);
                tot.insert(lu.key.to_string(), cur + qv / lu.per * rate.rate.num());
            }
        }
    }
    tot
}

// The land uses to factor (the six largest over 1 trip, largest first), and the groups: those, then "gateways" when over 1 trip
fn fit_groups(tot: &OrderedMap<f64>) -> (Vec<String>, Vec<String>) {
    let mut lu: Vec<String> = tot.iter().filter(|(k, v)| *k != "gateways" && **v > 1.0).map(|(k, _)| k.to_string()).collect();
    js_sort_by(&mut lu, |a, b| tot.get(b).copied().unwrap_or(0.0) - tot.get(a).copied().unwrap_or(0.0));
    lu.truncate(6);
    let groups: Vec<String> = if tot.get("gateways").copied().unwrap_or(f64::NAN) > 1.0 {
        let mut v = lu.clone();
        v.push("gateways".to_string());
        v
    } else {
        lu.clone()
    };
    (lu, groups)
}

// The observations of the calibration screenlines: only counts with a calibration or gateway role (a validation count
// on a calibration screenline is left out)
fn screenline_obs(ctx: &Ctx, view: &Scenario, period: &str, base0: &Res) -> Vec<Obs> {
    let mut obs: Vec<Obs> = Vec::new();
    for sl in ctx.store.screenlines.iter().filter(|x| !x.is_type("core") && !x.is_valid()) {
        for r in sl_rows(ctx, view, sl, period, Some(base0), true) {
            if r.dir == 0 || r.counted == 0 || r.roles.contains(&"valid") {
                continue;
            }
            if !r.arcs.is_empty() && r.c > 0.0 {
                obs.push(Obs { arcs: r.arcs.clone(), c: r.c, w: 1.0, ids: r.used.clone() });
            }
        }
    }
    obs
}

// The road class a count is grouped by
fn class_group(link: Option<&crate::project::Link>) -> &'static str {
    match link {
        Some(l) if is_mw(Some(l)) => "motorway",
        Some(l) if l.cls.is("arterial") => "arterial",
        Some(l) if l.cls.is("collector") => "collector",
        None | Some(_) => "local",
    }
}

/* No calibration screenlines: count groups (calibrate-workflow). Single counts would let a 20 veh/h count with a large
   relative error decide every factor, so counts are grouped by road class and compass sector round the counts' centre
   (Spare Construct convention), and each group's totals are fitted. Each count adds its observation-method weight to
   the group weight; groups under 100 veh/h are left out. */
fn count_group_obs(ctx: &Ctx, view: &Scenario, period: &str, base0: &Res) -> Vec<Obs> {
    let rows = count_rows(ctx, view, period, Some(base0), true);
    let cand: Vec<&CountRow> = rows
        .iter()
        .filter(|r| r.scored && r.role != "valid" && r.obs > 0.0 && panel_method(r.method).is_some_and(|m| m.weight > 0.0))
        .collect();
    let nn = math_max(1.0, usize_to_f64(cand.len()));
    let c_lat = cand.iter().fold(0.0f64, |s, r| s + r.c.lat.num()) / nn;
    let c_lng = cand.iter().fold(0.0f64, |s, r| s + r.c.lng.num()) / nn;
    let mut grp: Vec<(String, Grp)> = Vec::new();
    for r in cand {
        let sec = dir_of(bearing(&[c_lat, c_lng], &[r.c.lat.num(), r.c.lng.num()])).replace('B', "");
        let cls = class_group(view.links.get(&r.mt.link));
        let key = format!("{cls}|{sec}");
        let pos = if let Some(i) = grp.iter().position(|(k, _)| *k == key) { i } else {
            grp.push((key, Grp { arcs: Vec::new(), c: 0.0, wsum: 0.0, n: 0, ids: Vec::new() }));
            grp.len().saturating_sub(1)
        };
        // pos is always a place in grp (found there, or just pushed)
        let Some((_, gr)) = grp.get_mut(pos) else { continue };
        if r.mt.dir == "both" {
            gr.arcs.push(format!("{}:ab", r.mt.link));
            gr.arcs.push(format!("{}:ba", r.mt.link));
        } else {
            gr.arcs.push(format!("{}:{}", r.mt.link, r.mt.dir));
        }
        gr.c += r.obs;
        gr.wsum += panel_method(r.method).map_or(0.4, |m| m.weight);
        // Bound: a count of counts in memory, far below usize::MAX
        gr.n = gr.n.saturating_add(1);
        gr.ids.push(r.c.id.clone());
    }
    let mut obs: Vec<Obs> = Vec::new();
    for (_, gr) in grp {
        if gr.c >= 100.0 {
            obs.push(Obs { c: gr.c, w: gr.wsum / usize_to_f64(gr.n), ids: gr.ids, arcs: gr.arcs });
        }
    }
    obs
}

// The observations to fit and their basis: the calibration screenlines, else count groups
fn calibration_obs(p: &Project, view: &Scenario, period: &str, base0: &Res) -> (Vec<Obs>, &'static str) {
    let ctx = Ctx::new(p);
    let obs = screenline_obs(&ctx, view, period, base0);
    if obs.is_empty() {
        (count_group_obs(&ctx, view, period, base0), "count groups")
    } else {
        (obs, "screenlines")
    }
}

// What a run of the fit needs: the project with the applied factors set aside, the view, the settings, the period,
// the land uses to factor, the assignment options and the observations
struct FitRun<'a> {
    prior: &'a Project,
    view: &'a Scenario,
    s0: &'a Settings,
    period: &'a str,
    lu: &'a [String],
    ao: &'a AssignOpts,
    obs: &'a [Obs],
}

impl FitRun<'_> {
    // The modelled volume of each observation in a result
    fn vol(&self, res: &Res) -> Vec<f64> {
        self.obs.iter().map(|o| o.arcs.iter().fold(0.0f64, |s, k| s + res.arcs.get(k).map_or(0.0, |a| a.v))).collect()
    }

    // The sum of w x ((M - C) / C) squared over the observations
    fn objective(&self, m: &[f64]) -> f64 {
        self.obs.iter().enumerate().fold(0.0f64, |s, (i, o)| {
            let d = (at(m, i) - o.c) / o.c;
            s + o.w * (d * d)
        })
    }

    // The volumes of an assignment with the land uses' trip rates, and the estimated gateways' flows, scaled by the factors
    fn run(&self, f: &Factors) -> Vec<f64> {
        let period = self.period;
        let mut s1 = self.s0.clone();
        for k in self.lu {
            rate_scaled(&mut s1, k, period, factor(f, k));
        }
        let mut v2 = self.view.clone();
        let mut zones: OrderedMap<crate::project::Zone> = OrderedMap::new();
        for (id, z) in self.view.zones.iter() {
            let gf = f.get("gateways");
            let z2 = if z.kind.is("gateway") && !z.gw_src.get().is_some_and(value_truthy) && gf.is_some() && z.gw.is_set() {
                let mut z2 = z.clone();
                if let Some(gw) = z2.gw.get_mut() {
                    let fin = gw.num_at(&format!("{period}in")).or(0.0) * gf.copied().unwrap_or(1.0);
                    let fout = gw.num_at(&format!("{period}out")).or(0.0) * gf.copied().unwrap_or(1.0);
                    gw.insert(format!("{period}in"), Num::number(fin));
                    gw.insert(format!("{period}out"), Num::number(fout));
                }
                z2
            } else {
                z.clone()
            };
            zones.insert(id.to_string(), z2);
        }
        v2.zones = zones;
        let res = assign(self.prior, &v2, &s1, self.ao, None);
        self.vol(&res)
    }
}

// The change in each observation's volume per unit of each group's factor: one run per group with its factor at 1.5
fn gradients(fit: &FitRun, groups: &[String], ones: &Factors, m0: &[f64], prog: &mut dyn FnMut(f64, f64, &str)) -> Gradients {
    let mut grad: Gradients = HashMap::new();
    let mut step = 0.0;
    for gk in groups {
        step += 1.0;
        prog(step, usize_to_f64(groups.len()) + 1.0, &format!("Measuring the effect of {} trips", lu_label(gk)));
        let mut f1 = ones.clone();
        f1.insert(gk.clone(), 1.5);
        let m = fit.run(&f1);
        grad.insert(gk.clone(), m.iter().enumerate().map(|(i, v)| (v - at(m0, i)) / 0.5).collect());
    }
    grad
}

// The linear model m(f) = m0 + sum g_k (f_k - 1)
fn linear(m0: &[f64], groups: &[String], grad: &Gradients, f: &Factors) -> Vec<f64> {
    m0.iter()
        .enumerate()
        .map(|(i, v)| v + groups.iter().fold(0.0f64, |s, g| s + grad_at(grad, g, i) * (factor(f, g) - 1.0)))
        .collect()
}

// Bounded coordinate descent on the linear model, from factors of 1; the factors are then rounded to 3 decimals
fn descend(obs: &[Obs], m0: &[f64], groups: &[String], grad: &Gradients, ones: &Factors) -> Factors {
    let [lo, hi] = corridor::TEF_BOUNDS;
    let mut f = ones.clone();
    for _sweep in 0..60 {
        let mut moved = 0.0f64;
        for g in groups {
            let m = linear(m0, groups, grad, &f);
            let (mut num, mut den) = (0.0f64, 0.0f64);
            for (i, o) in obs.iter().enumerate() {
                let gi = grad_at(grad, g, i);
                let a = at(&m, i) - gi * factor(&f, g);
                let c2 = o.c * o.c;
                num += o.w * gi * (o.c - a) / c2;
                den += o.w * (gi * gi) / c2;
            }
            if den <= 0.0 || den.is_nan() {
                continue;
            }
            let nf = clamp(num / den, lo, hi);
            moved = math_max(moved, (nf - factor(&f, g)).abs());
            f.insert(g.clone(), nf);
        }
        if moved < 1e-4 {
            break;
        }
    }
    for g in groups {
        let v = math_round(factor(&f, g) * 1000.0) / 1000.0;
        f.insert(g.clone(), v);
    }
    f
}

// The record's note: why every factor is 1, or which factors reached a bound
fn tef_note(keep: bool, at_bound: &[String], lo: f64, hi: f64) -> String {
    let mut note = if keep { String::new() } else { "The confirming run did not improve on the trip rates as they are, so every factor is 1.".to_string() };
    if !at_bound.is_empty() {
        note = format!(
            "{} reached the {}\u{2013}{} bound. A factor at its bound usually means missing or wrong land use, gateway flows or roads: check those before applying.",
            at_bound.iter().map(|g| lu_label(g)).collect::<Vec<_>>().join(", "),
            crate::jsnum::number_to_js_string(lo),
            crate::jsnum::number_to_js_string(hi)
        );
    }
    note
}

// The ids of the counts in the observations, each once, in order
fn count_ids_of(obs: &[Obs]) -> Vec<String> {
    let mut count_ids: Vec<String> = Vec::new();
    for o in obs {
        for id in &o.ids {
            if !count_ids.contains(id) {
                count_ids.push(id.clone());
            }
        }
    }
    count_ids
}

// The earlier record's applied factors, which stay with the new record
fn prev_applied(sc: Option<&Scenario>, period: &str) -> Value {
    sc.and_then(|s| s.extra.get("calib"))
        .and_then(|c| c.get("tripEndFactors"))
        .and_then(|t| t.get(period))
        .and_then(|r| r.get("applied"))
        .filter(|a| value_truthy(a))
        .cloned()
        .unwrap_or(Value::Null)
}

// sc.calib.tripEndFactors[period] = rec
fn save_tef(target: &mut Scenario, period: &str, rec: Value) {
    let calib = target.extra.entry("calib".to_string()).or_insert_with(|| Value::Object(Map::new()));
    if !calib.is_object() {
        *calib = Value::Object(Map::new());
    }
    // calib is an object now, so this always saves
    if let Some(calib) = calib.as_object_mut() {
        let tef = calib.entry("tripEndFactors".to_string()).or_insert_with(|| Value::Object(Map::new()));
        if !tef.is_object() {
            *tef = Value::Object(Map::new());
        }
        if let Some(tef) = tef.as_object_mut() {
            tef.insert(period.to_string(), rec);
        }
    }
}

/// `tripEndFactors(sc, opts)`: fits at most seven trip-end factors (one per main land use and one for estimated gateways) to the calibration counts.
///
/// Minimises the sum of w x ((sum M - sum C) / sum C) squared over calibration screenlines, or over groups of
/// calibration and gateway counts when there are none (road class by compass sector, groups under 100 veh/h left out,
/// weights by observation method); never validation counts. Factors are bounded to 0.5 to 2. Linearised: one run per
/// factor at 1.5, a bounded coordinate descent on that model, then a confirming run; the factors are only kept if the
/// confirming run improves the objective. The fit always starts from the trip rates as they are (applied factors and
/// estimation are set aside while it runs). The record is saved in the scenario; `progress` is called as (step, steps, message).
///
/// # Errors
///
/// When the project has no scenario `sc_id`.
pub fn trip_end_factors(p: &mut Project, sc_id: &str, opts: &TefOpts, mut progress: Option<Progress<'_>>) -> Result<TefResult, String> {
    let idx = p.scenarios.iter().position(|s| s.id == sc_id).ok_or_else(|| format!("the project has no scenario {sc_id:?}"))?;
    let sc = p.scenarios.get(idx).cloned().ok_or_else(|| format!("the project has no scenario {sc_id:?}"))?;
    let s0 = p.settings.clone();
    let period: String = match opts.period {
        Some(x) if !x.is_empty() => x.to_string(),
        _ => s0.period.as_str().filter(|x| !x.is_empty()).unwrap_or("AM").to_string(),
    };
    // the fit starts from the trip rates as they are: applied factors set aside
    let mut prior = without_me_lu(p, &period);
    if let Some(lf) = prior.lu_factors.get_mut() {
        lf.remove(&period);
    }
    let [lo, hi] = corridor::TEF_BOUNDS;
    let bv = base_view(&sc, &s0);
    let view: &Scenario = &bv.view;
    let tot = land_use_totals(&prior, view, &s0, &period);
    let (lu, groups) = fit_groups(&tot);
    if groups.is_empty() {
        return Ok(TefResult {
            ok: false,
            note: "No base-year land use or estimated gateways to factor. Mark existing zones as existing land use first.".to_string(),
            rec: None,
        });
    }
    let ao = AssignOpts { period: Some(period.clone()), ..AssignOpts::default() };
    let base0 = assign(&prior, view, &s0, &ao, None);
    let (mut obs, basis) = calibration_obs(p, view, &period, &base0);
    obs.retain(|o| o.w > 0.0);
    if obs.is_empty() {
        return Ok(TefResult {
            ok: false,
            note: "No calibration counts or calibration screenlines to fit. Give some counts the calibration role first.".to_string(),
            rec: None,
        });
    }
    let fit = FitRun { prior: &prior, view, s0: &s0, period: &period, lu: &lu, ao: &ao, obs: &obs };
    let mut prog = |step: f64, steps: f64, msg: &str| {
        if let Some(cb) = progress.as_deref_mut() {
            cb(step, steps, msg);
        }
    };
    let ones: Factors = groups.iter().map(|k| (k.clone(), 1.0)).collect();
    let m0 = fit.vol(&base0);
    let obj0 = fit.objective(&m0);
    let grad = gradients(&fit, &groups, &ones, &m0, &mut prog);
    let fitted = descend(&obs, &m0, &groups, &grad, &ones);
    let steps = usize_to_f64(groups.len()) + 1.0;
    prog(steps, steps, "Confirming the factors");
    let m1 = fit.run(&fitted);
    let obj1 = fit.objective(&m1);
    let obj_lin = fit.objective(&linear(&m0, &groups, &grad, &fitted));
    let keep = obj1 < obj0 - 1e-9;
    let sw = obs.iter().fold(0.0f64, |s, o| s + o.w);
    let obj_after = if keep { obj1 } else { obj0 };
    let num = |x: f64| crate::jsnum::json_number(x);
    let fac = |m: &Factors| -> Value {
        // the keys in the order of the groups
        let mut o = Map::new();
        for g in &groups {
            o.insert(g.clone(), num(factor(m, g)));
        }
        Value::Object(o)
    };
    let at_bound: Vec<String> = if keep { groups.iter().filter(|g| factor(&fitted, g) <= lo + 1e-9 || factor(&fitted, g) >= hi - 1e-9).cloned().collect() } else { Vec::new() };
    let note = tef_note(keep, &at_bound, lo, hi);
    let date = opts.date.map_or_else(|| super::fit::iso_of_ms(crate::util::now_ms()), str::to_string);
    let mut rec = Map::new();
    rec.insert("period".into(), Value::String(period.clone()));
    rec.insert("date".into(), Value::String(date));
    rec.insert("basis".into(), Value::String(basis.to_string()));
    rec.insert("groups".into(), Value::Array(groups.iter().map(|g| Value::String(g.clone())).collect()));
    rec.insert("factors".into(), if keep { fac(&fitted) } else { fac(&ones) });
    rec.insert("tried".into(), fac(&fitted));
    rec.insert("objBefore".into(), num(obj0));
    rec.insert("objAfter".into(), num(obj_after));
    rec.insert("objLinear".into(), num(obj_lin));
    rec.insert("kept".into(), Value::Bool(keep));
    rec.insert("nObs".into(), Value::from(obs.len() as u64));
    rec.insert("rmsBefore".into(), num((obj0 / sw).sqrt()));
    rec.insert("rmsAfter".into(), num((obj_after / sw).sqrt()));
    rec.insert("applied".into(), prev_applied(p.scenarios.get(idx), &period));
    rec.insert("countIds".into(), Value::Array(count_ids_of(&obs).into_iter().map(Value::String).collect()));
    rec.insert("bounds".into(), Value::Array(vec![num(lo), num(hi)]));
    rec.insert("note".into(), Value::String(note));
    rec.insert("atBound".into(), Value::Array(at_bound.iter().map(|g| Value::String(g.clone())).collect()));
    let rec = Value::Object(rec);
    // idx is a place in p.scenarios (found above, and nothing was removed since)
    let Some(target) = p.scenarios.get_mut(idx) else {
        return Err(format!("the project has no scenario {sc_id:?}"));
    };
    save_tef(target, &period, rec.clone());
    Ok(TefResult { ok: true, note: String::new(), rec: Some(rec) })
}

fn tef_rec<'a>(sc: &'a Scenario, period: &str) -> Option<&'a Value> {
    sc.extra
        .get("calib")
        .and_then(|c| c.get("tripEndFactors"))
        .and_then(|t| t.get(period))
        .filter(|r| value_truthy(r))
}

/// Whether the fitted trip-end factors may be applied (`canApply`): ok, and why not.
#[derive(Clone, Debug, PartialEq)]
pub struct CanApply {
    /// They may be applied.
    pub ok: bool,
    /// Why not.
    pub why: String,
}

/// `canApply(sc, period)`: whether the fitted factors may be applied.
///
/// They go to `P.luFactors[period]`, which scales existing-land-use zones and estimated gateways only (development
/// keeps its own trip rates). A factor at its bound points at land use, gateway flows or roads, not trip rates, so it
/// is refused.
pub fn can_apply(sc: &Scenario, period: &str) -> CanApply {
    let no = |why: &str| CanApply { ok: false, why: why.to_string() };
    let Some(rec) = tef_rec(sc, period) else { return no("Fit the trip-end factors first.") };
    if rec.get("applied").is_some_and(value_truthy) {
        return no("The factors are already applied.");
    }
    if !rec.get("kept").is_some_and(value_truthy) {
        return match rec.get("note").filter(|n| value_truthy(n)) {
            Some(n) => CanApply { ok: false, why: crate::jsnum::value_to_js_string(n) },
            None => no("The factors did not improve the fit, so there is nothing to apply."),
        };
    }
    if matches!(rec.get("atBound"), Some(Value::Array(a)) if !a.is_empty()) {
        return no("Not applied: a factor at its bound points at missing or wrong land use, gateway flows or roads. Fix those, then fit again.");
    }
    CanApply { ok: true, why: String::new() }
}

fn mark_all_stale(p: &mut Project) {
    for s in &mut p.scenarios {
        if let Some(Value::Object(r)) = s.res.get_mut() {
            for v in r.values_mut() {
                if let Value::Object(o) = v {
                    o.insert("stale".to_string(), Value::Bool(true));
                }
            }
        }
    }
}

/// `applyFactors(sc, period)`: applies the fitted trip-end factors as `P.luFactors[period]` (land use to factor, and "gateways"): existing-land-use zones and estimated gateways in every scenario use them; the trip rates and gateway flows themselves are not changed. Marks every result out of date. False when they cannot be applied.
pub fn apply_factors(p: &mut Project, sc_id: &str, period: &str, now: &str) -> bool {
    let Some(sc) = p.scenario(sc_id) else { return false };
    let Some(rec) = tef_rec(sc, period).cloned() else { return false };
    if !can_apply(sc, period).ok {
        return false;
    }
    let before: Value = match p.lu_factors.get().and_then(|m| m.get(period)).and_then(|m| m.get()) {
        Some(m) => Value::Object(m.iter().map(|(k, v)| (k.to_string(), v.to_value_or_null())).collect()),
        None => Value::Null,
    };
    let mut factors: OrderedMap<Num> = OrderedMap::new();
    let mut fv = Map::new();
    if let Some(Value::Array(gs)) = rec.get("groups") {
        for g in gs {
            let k = crate::jsnum::value_to_js_string(g);
            let v = rec.get("factors").and_then(|f| f.get(&k)).map_or(f64::NAN, value_to_number);
            if v.is_finite() && v > 0.0 && (v - 1.0).abs() > 1e-9 {
                let rounded = math_round(v * 1000.0) / 1000.0;
                factors.insert(k.clone(), Num::number(rounded));
                fv.insert(k, crate::jsnum::json_number(rounded));
            }
        }
    }
    if !p.lu_factors.is_set() {
        p.lu_factors = Maybe::Set(OrderedMap::new());
    }
    // lu_factors is set just above, so this always applies
    if let Some(lf) = p.lu_factors.get_mut() {
        lf.insert(period.to_string(), Maybe::Set(factors));
    }
    let mut applied = Map::new();
    applied.insert("at".into(), Value::String(now.to_string()));
    applied.insert("via".into(), Value::String("luFactors".to_string()));
    applied.insert("before".into(), before);
    applied.insert("factors".into(), Value::Object(fv));
    set_applied(p, sc_id, period, Value::Object(applied));
    mark_all_stale(p);
    true
}

fn set_applied(p: &mut Project, sc_id: &str, period: &str, v: Value) {
    if let Some(sc) = p.scenarios.iter_mut().find(|s| s.id == sc_id)
        && let Some(r) = sc
            .extra
            .get_mut("calib")
            .and_then(|c| c.get_mut("tripEndFactors"))
            .and_then(|t| t.get_mut(period))
            .and_then(|r| r.as_object_mut())
    {
        r.insert("applied".to_string(), v);
    }
}

/// `revertFactors(sc, period)`: puts back the land-use factors that were there before the fitted ones were applied. False when nothing was applied (or it was applied by an older method this port does not read).
pub fn revert_factors(p: &mut Project, sc_id: &str, period: &str) -> bool {
    let Some(sc) = p.scenario(sc_id) else { return false };
    let Some(rec) = tef_rec(sc, period) else { return false };
    let Some(applied) = rec.get("applied").filter(|a| value_truthy(a)).cloned() else { return false };
    if applied.get("via").and_then(|v| v.as_str()) != Some("luFactors") {
        return false;
    }
    if !p.lu_factors.is_set() {
        p.lu_factors = Maybe::Set(OrderedMap::new());
    }
    // lu_factors is set just above, so this always applies
    if let Some(lf) = p.lu_factors.get_mut() {
        match applied.get("before").filter(|b| value_truthy(b)) {
            Some(Value::Object(b)) => {
                let m: OrderedMap<Num> = b.iter().map(|(k, v)| (k.clone(), crate::datahub::value_num(v))).collect();
                lf.insert(period.to_string(), Maybe::Set(m));
            }
            _ => {
                lf.remove(period);
            }
        }
    }
    if p.lu_factors.get().is_some_and(crate::project::OrderedMap::is_empty) {
        p.lu_factors = Maybe::Missing;
    }
    set_applied(p, sc_id, period, Value::Null);
    mark_all_stale(p);
    true
}

trait ToValueOrNull {
    fn to_value_or_null(&self) -> Value;
}

impl ToValueOrNull for Num {
    fn to_value_or_null(&self) -> Value {
        match self {
            Maybe::Set(j) => j.to_value(),
            _ => Value::Null,
        }
    }
}
