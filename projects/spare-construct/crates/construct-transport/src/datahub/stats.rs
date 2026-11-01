//! The model against its counts: GEH, the model volume at a count, the regression the criteria use, the
//! model-against-counts summary (`DataHub.calibration`) and the journey-time check (`DataHub.jtCheck`).

use super::{Count, Data, Match, is_estimate, matches, period_vol};
use crate::graph::js_sort_by;
use crate::jsnum::math_max;
use crate::project::{Scenario, Settings};
use crate::results::{Res, plain};
use serde::Serialize;
use std::rc::Rc;

/// `Math.sqrt(2 * (m - c) ** 2 / Math.max(1, m + c))`: the GEH statistic of a modelled flow m and a count c (veh/h).
pub fn geh(m: f64, c: f64) -> f64 {
    let d = m - c;
    (2.0 * (d * d) / math_max(1.0, m + c)).sqrt()
}

/// The modelled flow at a matched count: the arc it counts (or both arcs of the link added), None when that arc has no result.
pub fn model_flow(res: &Res, mt: &Match) -> Option<f64> {
    let a = res.arcs.get(&format!("{}:ab", mt.link));
    let b = res.arcs.get(&format!("{}:ba", mt.link));
    match mt.dir {
        "ab" => a.map(|x| x.v),
        "ba" => b.map(|x| x.v),
        _ => Some(a.map_or(0.0, |x| x.v) + b.map_or(0.0, |x| x.v)),
    }
}

/// What `DataHub.regression` gives: the fit of modelled flows (y) against counts (x).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Regression {
    /// Number of pairs.
    pub n: usize,
    /// Slope through the origin, Σxy / Σx².
    #[serde(with = "plain")]
    pub b0: Option<f64>,
    /// R² through the origin centred on the mean of y (the TMDG test).
    #[serde(with = "plain")]
    pub r2_origin_centred: Option<f64>,
    /// R² through the origin against Σy² (what Excel shows).
    #[serde(with = "plain")]
    pub r2_origin_excel: Option<f64>,
    /// OLS intercept.
    #[serde(with = "plain")]
    pub ols_a: Option<f64>,
    /// OLS slope.
    #[serde(with = "plain")]
    pub ols_b: Option<f64>,
    /// OLS R², the square of Pearson's r.
    #[serde(with = "plain")]
    pub r2_ols: Option<f64>,
}

/// `DataHub.regression(x, y)`: slope through the origin and the three R² of model (y) against counts (x); None where undefined.
pub fn regression(x: &[f64], y: &[f64]) -> Regression {
    let n = x.len().min(y.len());
    let mut out = Regression {
        n,
        b0: None,
        r2_origin_centred: None,
        r2_origin_excel: None,
        ols_a: None,
        ols_b: None,
        r2_ols: None,
    };
    if n < 2 {
        return out;
    }
    let (mut sx, mut sy, mut sxy, mut sxx, mut syy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        sx += x[i];
        sy += y[i];
        sxy += x[i] * y[i];
        sxx += x[i] * x[i];
        syy += y[i] * y[i];
    }
    let mx = sx / n as f64;
    let my = sy / n as f64;
    if sxx > 0.0 {
        let b0 = sxy / sxx;
        let (mut se, mut st) = (0.0f64, 0.0f64);
        for i in 0..n {
            let e = y[i] - b0 * x[i];
            let t = y[i] - my;
            se += e * e;
            st += t * t;
        }
        out.b0 = Some(b0);
        out.r2_origin_centred = if st > 0.0 { Some(1.0 - se / st) } else { None };
        out.r2_origin_excel = if syy > 0.0 {
            Some(1.0 - se / syy)
        } else {
            None
        };
    }
    let (mut cxy, mut cxx, mut cyy) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let a = x[i] - mx;
        let b = y[i] - my;
        cxy += a * b;
        cxx += a * a;
        cyy += b * b;
    }
    if cxx > 0.0 {
        let b = cxy / cxx;
        out.ols_b = Some(b);
        out.ols_a = Some(my - b * mx);
    }
    if cxx > 0.0 && cyy > 0.0 {
        out.r2_ols = Some(cxy * cxy / (cxx * cyy));
    }
    out
}

/// `DataHub.REG_LABELS`: the names of the regression statistics.
pub const REG_LABELS: [(&str, &str); 6] = [
    ("b0", "Slope through the origin"),
    (
        "r2OriginCentred",
        "R\u{b2} through the origin (centred; TMDG test)",
    ),
    ("r2OriginExcel", "R\u{b2} through the origin (Excel)"),
    ("r2Ols", "R\u{b2} (OLS)"),
    ("olsSlope", "OLS slope"),
    ("olsIntercept", "OLS intercept (veh/h)"),
];

/// One counted site in the model-against-counts summary.
#[derive(Clone, Debug)]
pub struct CalibRow {
    /// The count.
    pub c: Rc<Count>,
    /// Where it was matched.
    pub mt: Match,
    /// Counted volume in the period (veh/h).
    pub obs: f64,
    /// Modelled flow (veh/h).
    pub model: f64,
    /// GEH.
    pub geh: f64,
    /// How the counted volume was made.
    pub how: String,
}

/// What `DataHub.calibration` gives: the rows (worst GEH first) and the statistics; the statistics are None with no rows.
#[derive(Clone, Debug)]
pub struct CalibrationSummary {
    /// Counted sites, worst GEH first.
    pub rows: Vec<CalibRow>,
    /// Number of sites.
    pub n: usize,
    /// The statistics (None with no rows).
    pub stats: Option<CalibrationStats>,
}

/// The statistics of [`CalibrationSummary`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationStats {
    /// Share of sites with GEH under 5.
    #[serde(with = "plain")]
    pub pass: f64,
    /// OLS R² (`r2` and `r2Ols`, the same value).
    #[serde(with = "plain")]
    pub r2: Option<f64>,
    /// OLS slope.
    #[serde(with = "plain")]
    pub ols_slope: Option<f64>,
    /// OLS intercept.
    #[serde(with = "plain")]
    pub ols_intercept: Option<f64>,
    /// Slope through the origin (`b0` and `slope`).
    #[serde(with = "plain")]
    pub b0: Option<f64>,
    /// R² through the origin, centred (the TMDG test).
    #[serde(with = "plain")]
    pub r2_origin_centred: Option<f64>,
    /// R² through the origin, Excel's.
    #[serde(with = "plain")]
    pub r2_origin_excel: Option<f64>,
    /// Σ counted.
    #[serde(with = "plain")]
    pub tot_obs: f64,
    /// Σ modelled.
    #[serde(with = "plain")]
    pub tot_mod: f64,
}

/// `DataHub.calibration(sc, period)`: the model's flows against the counts in use for a period, None without results.
///
/// RAMM estimates are never validation evidence. Rows are sorted worst GEH first.
pub fn calibration(
    sc: &Scenario,
    res: Option<&Res>,
    period: &str,
    s: &Settings,
    data: &Data,
) -> Option<CalibrationSummary> {
    let res = res?;
    let m = matches(sc, s, data);
    let mut rows: Vec<CalibRow> = Vec::new();
    for c in &data.counts {
        if !data.in_use(c) || is_estimate(c) {
            continue;
        }
        let Some(mt) = m.get(&c.id) else { continue };
        let Some(pv) = period_vol(c, period, s) else {
            continue;
        };
        let Some(model) = model_flow(res, mt) else {
            continue;
        };
        let obs = pv.v;
        rows.push(CalibRow {
            c: c.clone(),
            mt: mt.clone(),
            obs,
            model,
            geh: geh(model, obs),
            how: pv.how,
        });
    }
    if rows.is_empty() {
        return Some(CalibrationSummary {
            rows,
            n: 0,
            stats: None,
        });
    }
    let n = rows.len();
    let xs: Vec<f64> = rows.iter().map(|r| r.obs).collect();
    let ys: Vec<f64> = rows.iter().map(|r| r.model).collect();
    let reg = regression(&xs, &ys);
    let tot_obs = rows.iter().fold(0.0f64, |s, r| s + r.obs);
    let tot_mod = rows.iter().fold(0.0f64, |s, r| s + r.model);
    js_sort_by(&mut rows, |x, y| y.geh - x.geh);
    let pass = rows.iter().filter(|r| r.geh < 5.0).count() as f64 / n as f64;
    Some(CalibrationSummary {
        rows,
        n,
        stats: Some(CalibrationStats {
            pass,
            r2: reg.r2_ols,
            ols_slope: reg.ols_b,
            ols_intercept: reg.ols_a,
            b0: reg.b0,
            r2_origin_centred: reg.r2_origin_centred,
            r2_origin_excel: reg.r2_origin_excel,
            tot_obs,
            tot_mod,
        }),
    })
}

/// One journey-time tier: within the larger of a share of the observed time and a number of seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JtTier {
    /// Key ("tier1", "tier2").
    pub id: &'static str,
    /// Share of the observed time.
    pub rel: f64,
    /// Seconds.
    pub abs: f64,
}

/// `DataHub.JT_CRITERIA`: the journey-time validation criteria.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JtCriteria {
    /// Key.
    pub id: &'static str,
    /// Name.
    pub name: &'static str,
    /// Source edition.
    pub edition: &'static str,
    /// Checked against the source.
    pub verified: bool,
    /// The two tiers.
    pub tiers: [JtTier; 2],
    /// Plain-English criterion.
    pub text: &'static str,
    /// Label for Google typical times.
    pub google_label: &'static str,
}

/// `DataHub.JT_CRITERIA`.
pub const JT_CRITERIA: JtCriteria = JtCriteria {
    id: "tmdg-jt",
    name: "Journey time validation",
    edition: "NZTA TMDG 2019; UK TAG M3.1 Table 3",
    verified: false,
    tiers: [
        JtTier {
            id: "tier1",
            rel: 0.15,
            abs: 60.0,
        },
        JtTier {
            id: "tier2",
            rel: 0.25,
            abs: 90.0,
        },
    ],
    text: "Within 15% or 1 minute, whichever is larger (TMDG 2019; TAG M3.1 Table 3).",
    google_label: "Google typical (predicted, not observed)",
};

/// What `DataHub.jtCheck` gives: modelled and observed time (s), the difference and whether each tier is met.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JtCheck {
    /// Modelled time (s).
    #[serde(with = "plain")]
    pub model: f64,
    /// Observed time (s).
    #[serde(with = "plain")]
    pub obs: f64,
    /// Modelled less observed (s).
    #[serde(with = "plain")]
    pub diff: f64,
    /// The difference as a share of the observed time.
    #[serde(with = "plain")]
    pub pct: f64,
    /// The criterion in words.
    pub text: String,
    /// Tier 1 met.
    pub tier1: bool,
    /// Tier 1 tolerance (s).
    #[serde(with = "plain")]
    pub tier1_tol: f64,
    /// Tier 2 met.
    pub tier2: bool,
    /// Tier 2 tolerance (s).
    #[serde(with = "plain")]
    pub tier2_tol: f64,
}

/// `DataHub.jtCheck(modelS, obsS)`: the journey-time check of a modelled against an observed time (s), None unless the modelled time is 0 or more and the observed time above 0.
pub fn jt_check(model_s: f64, obs_s: f64) -> Option<JtCheck> {
    if !(model_s >= 0.0) || !(obs_s > 0.0) {
        return None;
    }
    let diff = model_s - obs_s;
    let tol = |t: &JtTier| math_max(t.rel * obs_s, t.abs);
    let t1 = tol(&JT_CRITERIA.tiers[0]);
    let t2 = tol(&JT_CRITERIA.tiers[1]);
    Some(JtCheck {
        model: model_s,
        obs: obs_s,
        diff,
        pct: diff / obs_s,
        text: JT_CRITERIA.text.to_string(),
        tier1: diff.abs() <= t1,
        tier1_tol: t1,
        tier2: diff.abs() <= t2,
        tier2_tol: t2,
    })
}
