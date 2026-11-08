//! Matrix estimation against traffic counts (`Calibrate.fit`): one ME2-style step, the matrix change report, the
//! base-year view the counts are compared in, and the fit itself.
//!
//! Method (SATURN ME2-style, multiplicative, damped): assign; compare each counted site with the model; site ratio
//! r = obs / model clamped to [1/maxStep, maxStep]; each OD pair's factor is multiplied by exp(damp x path-flow-weighted
//! mean, over its paths, of the weighted mean log r of the counted sites the path crosses). A path that crosses no
//! counted site adds 0, so an OD pair that is only partly counted moves only partly. Cumulative factors are clamped to
//! [1/maxFactor, maxFactor]. Only OD pairs with both zones existing land use or gateways change. Counts are today's
//! traffic, so the fit runs on a base-year view of the scenario. Each pass must keep the matrix change inside the
//! TMDG section 8 total and TAG M3.1 Table 5 trip-end limits; when the first pass breaks them the step is reduced
//! (maxStep 2, 1.5, 1.25, 1.1). A fit starts from the prior matrix and keeps the best pass (most weighted sites with
//! GEH under 5, then lowest weighted mean GEH).
//!
//! The prototype's `fit` is asynchronous only to let the page draw between assignments; this one is synchronous and
//! reports progress through a callback. It changes the project it is given (the factors in `P.me`, the fit record, the
//! scenario's result) exactly where the prototype does.

use super::jslog::log;
use super::sites::{SiteRow, Unit};
use super::stats::{Stats, origin, pearson2, rule_of, stats_of};
use crate::jsmath;
use crate::jsnum::{Maybe, Num, math_min, or_default};
use crate::project::{OrderedMap, Scenario, Settings, Zone};
use crate::results::{Res, plain};
use crate::standards::year_of;
use crate::util::clamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::borrow::Cow;

/// `eligible(z)`: a gateway or an existing-land-use zone (the only ones matrix estimation changes).
pub fn eligible(z: Option<&Zone>) -> bool {
    z.is_some_and(|z| z.kind.is("gateway") || z.existing.is_true())
}

/// `keyOf(a, b)`: the key of an OD pair in the factors, "<origin id>><destination id>".
pub fn key_of(a: &str, b: &str) -> String {
    format!("{a}>{b}")
}

/// The sites a fit works on and the weighted fit it judges a pass by (`wfit`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WFit {
    /// Weighted share of sites with GEH under 5.
    pub pass: f64,
    /// Weighted mean GEH.
    pub geh: f64,
}

/// `wfit(rows)`: what a pass is judged on: the weighted share of sites with GEH < 5, then the weighted mean GEH (with every weight 1 these are the plain statistics).
pub fn wfit(rows: &[SiteRow]) -> WFit {
    let (mut sw, mut p5, mut g) = (0.0f64, 0.0f64, 0.0f64);
    for r in rows {
        let w = r.w;
        if !(w > 0.0) {
            continue;
        }
        sw += w;
        g += w * r.geh;
        if r.geh < 5.0 {
            p5 += w;
        }
    }
    if sw > 0.0 {
        WFit {
            pass: p5 / sw,
            geh: g / sw,
        }
    } else {
        WFit {
            pass: 0.0,
            geh: f64::INFINITY,
        }
    }
}

/// `better(a, b)`: pass a has more weighted sites under GEH 5 than b, or the same and a lower weighted mean GEH (by at least 0.5%).
pub fn better(a: &WFit, b: &WFit) -> bool {
    a.pass > b.pass + 1e-9 || (a.pass > b.pass - 1e-9 && a.geh < b.geh * 0.995)
}

/// `Stats` of a set of site rows (`Calibrate.stats(rows)`).
pub fn stats(rows: &[SiteRow]) -> Stats {
    let v: Vec<(f64, f64, f64)> = rows.iter().map(|r| (r.obs, r.model, r.geh)).collect();
    stats_of(&v)
}

/// The options of one fit, as the prototype's `fit` resolves them (`o`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FitOptions {
    /// Passes.
    #[serde(with = "plain")]
    pub iters: f64,
    /// Damping of each step.
    #[serde(with = "plain")]
    pub damp: f64,
    /// Largest cumulative factor.
    #[serde(with = "plain")]
    pub max_factor: f64,
    /// Largest site ratio per step.
    #[serde(with = "plain")]
    pub max_step: f64,
    /// Sites with a modelled or counted flow below this are left out of a step.
    #[serde(with = "plain")]
    pub min_vol: f64,
    /// A pass that breaks the matrix-change limits ends the fit.
    pub constrain: bool,
    /// Gateway-to-gateway cells keep the prior.
    #[serde(rename = "freezeGG")]
    pub freeze_gg: bool,
    /// How counts are weighed: "method", "caller" or "none".
    pub weights: &'static str,
    /// Number of counts that take part, when the caller chose them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_counts: Option<usize>,
    /// Number of count groups, when the caller gave groups.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<usize>,
}

/// One ME2 step (`Calibrate.step`): new cumulative factors from a result.
///
/// `rows` are fitting units. A path takes the weighted mean of the log ratios of the units it crosses (so counts in
/// sequence do not compound), times the largest weight among them (weight 0: left out). `f` holds the cumulative
/// factors by OD pair key, in the order the pairs were first changed.
pub fn step(view: &Scenario, res: &Res, rows: &[Unit], f: &mut OrderedMap<f64>, o: &FitOptions) {
    let lr: Vec<Option<f64>> = rows
        .iter()
        .map(|r| {
            if r.model < o.min_vol || r.obs < o.min_vol || !(r.w > 0.0) {
                None
            } else {
                Some(log(clamp(r.obs / r.model, 1.0 / o.max_step, o.max_step)))
            }
        })
        .collect();
    let wt: Vec<f64> = rows.iter().map(|r| r.w).collect();
    let mut at: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for (s, r) in rows.iter().enumerate() {
        if lr[s].is_some() {
            for &a in &r.arcs {
                at.entry(a).or_default().push(s);
            }
        }
    }
    let mut seen = vec![-1i64; rows.len()];
    let lo = 1.0 / o.max_factor;
    let mut stamp: i64 = 0;
    let Some(paths) = res.paths.get() else { return };
    for od in paths {
        let zi = view.zones.get(&res.zone_ids[od.i]);
        let zj = view.zones.get(&res.zone_ids[od.j]);
        if od.i == od.j || !eligible(zi) || !eligible(zj) {
            continue;
        }
        let (zi, zj) = (zi.expect("eligible"), zj.expect("eligible"));
        if o.freeze_gg && zi.kind.is("gateway") && zj.kind.is("gateway") {
            continue; // through traffic keeps the prior
        }
        let (mut acc, mut w) = (0.0f64, 0.0f64);
        for p in &od.list {
            stamp += 1;
            let (mut s, mut sw, mut wm) = (0.0f64, 0.0f64, 0.0f64);
            for a in &p.arcs {
                let Some(l) = at.get(a) else { continue };
                for &t in l {
                    if seen[t] != stamp {
                        seen[t] = stamp;
                        s += wt[t] * lr[t].expect("counted unit");
                        sw += wt[t];
                        if wt[t] > wm {
                            wm = wt[t];
                        }
                    }
                }
            }
            acc += p.w * (if sw > 0.0 { s / sw * wm } else { 0.0 });
            w += p.w;
        }
        if !(w > 0.0) || acc == 0.0 {
            continue;
        }
        let key = key_of(&zi.id, &zj.id);
        let cur = or_default(f.get(&key).copied().unwrap_or(f64::NAN), 1.0);
        f.insert(
            key,
            clamp(cur * jsmath::exp(o.damp * acc / w), lo, o.max_factor),
        );
    }
}

/// `toObj(f)`: the factors as they are stored: only pairs that moved more than 0.001%, rounded towards 1 so a stored factor never passes the clamp.
pub fn to_obj(f: &OrderedMap<f64>) -> OrderedMap<Num> {
    let mut o = OrderedMap::new();
    for (k, &v) in f.iter() {
        if (v - 1.0).abs() > 1e-5 {
            let r = if v < 1.0 {
                (v * 1e5).ceil()
            } else {
                (v * 1e5).floor()
            } / 1e5;
            o.insert(k.to_string(), Num::number(r));
        }
    }
    o
}

/// How far a matrix moved (`Calibrate.matrixChange`): prior against estimated, in total, cell by cell and by trip end.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixChange {
    /// Σ prior (veh/h).
    #[serde(with = "plain")]
    pub prior_total: f64,
    /// Σ estimated (veh/h).
    #[serde(with = "plain")]
    pub total: f64,
    /// Change in the total (%).
    #[serde(with = "plain")]
    pub total_pct: f64,
    /// Mean absolute change of the cells that carry trips (%).
    #[serde(with = "plain")]
    pub mean_abs_cell_pct: f64,
    /// Cells that moved by more than 0.0001%.
    pub pairs: usize,
    /// Cells that carry trips in the prior.
    pub cells: usize,
    /// R² of cells, prior against estimated.
    #[serde(with = "plain")]
    pub cell_r2: Option<f64>,
    /// Slope of cells through the origin.
    #[serde(with = "plain")]
    pub cell_slope: Option<f64>,
    /// R² of origin totals.
    #[serde(with = "plain")]
    pub row_r2: Option<f64>,
    /// R² of destination totals.
    #[serde(with = "plain")]
    pub col_r2: Option<f64>,
    /// Slope of origin totals through the origin.
    #[serde(with = "plain")]
    pub row_slope: Option<f64>,
    /// Slope of destination totals through the origin.
    #[serde(with = "plain")]
    pub col_slope: Option<f64>,
    /// The lower of the two trip-end R² (the one that exists when only one does).
    #[serde(rename = "tripEndR2", with = "plain")]
    pub trip_end_r2: Option<f64>,
}

/// `Calibrate.matrixChange(G, prior, est)`: prior against estimated over `z` zones (matrices row by row).
pub fn matrix_change(z: usize, prior: &[f64], est: &[f64]) -> MatrixChange {
    let mut ro = vec![0.0f64; z];
    let mut re = vec![0.0f64; z];
    let mut co = vec![0.0f64; z];
    let mut ce = vec![0.0f64; z];
    let (mut cp, mut cq): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    let (mut tp, mut te) = (0.0f64, 0.0f64);
    let (mut cells, mut dev, mut pairs) = (0usize, 0.0f64, 0usize);
    for i in 0..z {
        for j in 0..z {
            let p = prior[i * z + j];
            let e = est[i * z + j];
            ro[i] += p;
            re[i] += e;
            co[j] += p;
            ce[j] += e;
            tp += p;
            te += e;
            if p > 0.0 {
                cells += 1;
                dev += (e / p - 1.0).abs();
                cp.push(p);
                cq.push(e);
                if (e / p - 1.0).abs() > 1e-6 {
                    pairs += 1;
                }
            }
        }
    }
    // Zones with no trips either way are left out of the trip-end tests
    let live: Vec<usize> = (0..z)
        .filter(|&i| ro[i] + co[i] + re[i] + ce[i] > 0.0)
        .collect();
    let pick = |a: &[f64]| -> Vec<f64> { live.iter().map(|&i| a[i]).collect() };
    let row_r2 = pearson2(&pick(&ro), &pick(&re));
    let col_r2 = pearson2(&pick(&co), &pick(&ce));
    let rs = origin(&pick(&ro), &pick(&re)).b0;
    let cs = origin(&pick(&co), &pick(&ce)).b0;
    let cl = origin(&cp, &cq).b0;
    MatrixChange {
        prior_total: tp,
        total: te,
        total_pct: if tp > 0.0 {
            (te - tp) / tp * 100.0
        } else {
            0.0
        },
        mean_abs_cell_pct: if cells > 0 {
            dev / cells as f64 * 100.0
        } else {
            0.0
        },
        pairs,
        cells,
        cell_r2: pearson2(&cp, &cq),
        cell_slope: cl,
        row_r2,
        col_r2,
        row_slope: rs,
        col_slope: cs,
        trip_end_r2: match (row_r2, col_r2) {
            (None, c) => c,
            (r, None) => r,
            (Some(r), Some(c)) => Some(math_min(r, c)),
        },
    }
}

/// `breach(mc)`: the limit a pass broke ("matrix total" or "trip ends"), or "" when within them (TMDG section 8 total, TAG Table 5 trip-end R²).
pub fn breach(mc: &MatrixChange) -> &'static str {
    if mc.total_pct.abs() > rule_of("matrixTotal").max.unwrap_or(f64::NAN) {
        return "matrix total";
    }
    if let Some(t) = mc.trip_end_r2
        && t < rule_of("tripEnds").min.unwrap_or(f64::NAN)
    {
        return "trip ends";
    }
    ""
}

/// What the counts measure (`Calibrate.baseView`): the scenario in the base year with existing land use and existing roads.
#[derive(Clone, Debug)]
pub struct BaseView<'a> {
    /// The view: the scenario itself when it already is that, otherwise a copy with the base year's existing zones and existing links.
    pub view: Cow<'a, Scenario>,
    /// The scenario is already the base-year view.
    pub same: bool,
}

/// `Calibrate.baseView(sc, S)`: the scenario in the base year (no background growth), existing land use and gateways open by then, and no proposed links.
pub fn base_view<'a>(sc: &'a Scenario, s: &Settings) -> BaseView<'a> {
    let by = s.base_year.num();
    let mut zones: OrderedMap<Zone> = OrderedMap::new();
    for (id, z) in sc.zones.iter() {
        if eligible(Some(z)) && !(z.year.num() > by) {
            zones.insert(id.to_string(), z.clone());
        }
    }
    let mut links = OrderedMap::new();
    for (id, l) in sc.links.iter() {
        if !l.status.is("proposed") {
            links.insert(id.to_string(), l.clone());
        }
    }
    let same = year_of(Some(sc), s) == by
        && zones.len() == sc.zones.len()
        && links.len() == sc.links.len();
    if same {
        return BaseView {
            view: Cow::Borrowed(sc),
            same,
        };
    }
    let mut v = sc.clone();
    v.id = format!("{}:me", sc.id);
    v.name = Maybe::Set(format!("{} (base year)", sc.name.str_or("")));
    v.year = Num::number(by);
    v.zones = zones;
    v.links = links;
    v.res = Maybe::Set(Value::Object(Map::new()));
    BaseView {
        view: Cow::Owned(v),
        same,
    }
}
