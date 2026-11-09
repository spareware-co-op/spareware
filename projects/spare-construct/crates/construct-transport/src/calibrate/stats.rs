//! The statistics of calibrate.js: the validation criteria with their sources (`Calibrate.RULES`), the through-origin
//! and OLS fits, the GEH bands and the statistics of a set of counted sites.

use crate::results::plain;
use serde::Serialize;

/// The TMDG edition the criteria cite.
pub const TMDG: &str = "NZTA Transport Model Development Guidelines (TMDG) 2019";
/// The TAG edition the criteria cite.
pub const TAG: &str = "UK TAG M3.1 (May 2026)";

macro_rules! tmdg {
    ($s:expr) => {
        concat!(
            "NZTA Transport Model Development Guidelines (TMDG) 2019",
            $s
        )
    };
}
macro_rules! tag {
    ($s:expr) => {
        concat!("UK TAG M3.1 (May 2026)", $s)
    };
}

/// One validation criterion with its source (`Calibrate.RULES[id]`).
///
/// `kind` is "share" (of sites), "r2", "range" or "max"; `use_` is "required" (decides the overall status), "info"
/// (reported only) or "warn" (applies to the matrix change after estimation).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Rule {
    /// Kind: "share", "r2", "range" or "max".
    pub kind: &'static str,
    /// Minimum (share, r2 and range kinds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Maximum (range and max kinds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// %RMSE above which no explanation can excuse it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clarify: Option<f64>,
    /// Slope range (matrix cell and trip-end tests).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slope: Option<[f64; 2]>,
    /// "required", "info" or "warn".
    #[serde(rename = "use")]
    pub use_: &'static str,
    /// Display name.
    pub label: &'static str,
    /// The target in words.
    pub target: &'static str,
    /// What the criterion is based on.
    pub basis: &'static str,
    /// Source document.
    pub source: &'static str,
    /// Table in the source.
    pub table: &'static str,
    /// Checked against the source.
    pub verified: bool,
}

const fn rule(kind: &'static str) -> Rule {
    Rule {
        kind,
        min: None,
        max: None,
        clarify: None,
        slope: None,
        use_: "required",
        label: "",
        target: "",
        basis: "",
        source: TMDG,
        table: "",
        verified: true,
    }
}

/// `Calibrate.RULES`, in its order: geh5, geh75, geh10, r2, slope, band, flow, rmse, matrixTotal, cells, tripEnds.
pub const RULES: [(&str, Rule); 11] = [
    (
        "geh5",
        Rule {
            min: Some(0.85),
            label: "Sites with GEH < 5",
            target: "more than 85% of sites",
            basis: tmdg!(", Category E, individual counts"),
            table: "individual counts, GEH",
            ..rule("share")
        },
    ),
    (
        "geh75",
        Rule {
            min: Some(0.90),
            label: "Sites with GEH < 7.5",
            target: "more than 90% of sites",
            basis: tmdg!(", Category E, individual counts"),
            table: "individual counts, GEH",
            ..rule("share")
        },
    ),
    (
        "geh10",
        Rule {
            min: Some(0.95),
            label: "Sites with GEH < 10",
            target: "more than 95% of sites",
            basis: tmdg!(", Category E, individual counts"),
            table: "individual counts, GEH",
            ..rule("share")
        },
    ),
    (
        "r2",
        Rule {
            min: Some(0.95),
            label: "R\u{b2} of model against counts (through the origin, centred)",
            target: "more than 0.95",
            basis: tmdg!(", Category E, scatter forced through the origin"),
            table: "scatter plot criteria",
            ..rule("r2")
        },
    ),
    (
        "slope",
        Rule {
            min: Some(0.95),
            max: Some(1.05),
            label: "Slope of model against counts (through the origin)",
            target: "0.95 to 1.05",
            basis: tmdg!(", Category E, scatter forced through the origin"),
            table: "scatter plot criteria",
            ..rule("range")
        },
    ),
    (
        "band",
        Rule {
            min: Some(0.90),
            use_: "info",
            label: "Sites within the TMDG count band",
            target: "more than 90% of sites (within 50 veh/h below 400, 12.5% to 2,000, 250 veh/h above)",
            basis: tmdg!(", Category E, potentially useful"),
            table: "count band criteria",
            ..rule("share")
        },
    ),
    (
        "flow",
        Rule {
            min: Some(0.85),
            use_: "info",
            label: "Sites with GEH < 5 or within the TAG flow band",
            target: "more than 85% of sites (within 100 veh/h below 700, 15% to 2,700, 400 veh/h above)",
            basis: tag!(", link flow criteria, for information"),
            source: TAG,
            table: "Table 2",
            ..rule("share")
        },
    ),
    (
        "rmse",
        Rule {
            max: Some(15.0),
            clarify: Some(25.0),
            use_: "info",
            label: "%RMSE",
            target: "under 15% (15\u{2013}25% needs explaining)",
            basis: tmdg!(", Category E, potentially useful"),
            table: "%RMSE criteria",
            ..rule("max")
        },
    ),
    (
        "matrixTotal",
        Rule {
            max: Some(5.0),
            use_: "warn",
            label: "Change in matrix total",
            target: "within 5% (a good prior; 10% for a poor one)",
            basis: tmdg!(", section 8, Category E"),
            table: "section 8 (matrix estimation)",
            ..rule("max")
        },
    ),
    (
        "cells",
        Rule {
            min: Some(0.95),
            slope: Some([0.98, 1.02]),
            use_: "warn",
            label: "R\u{b2} of matrix cells, prior against estimated",
            target: "more than 0.95, slope 0.98 to 1.02",
            basis: tag!(", matrix estimation significance"),
            source: TAG,
            table: "Table 5",
            ..rule("r2")
        },
    ),
    (
        "tripEnds",
        Rule {
            min: Some(0.98),
            slope: Some([0.99, 1.01]),
            use_: "warn",
            label: "R\u{b2} of trip ends, prior against estimated",
            target: "more than 0.98, slope 0.99 to 1.01",
            basis: tag!(", matrix estimation significance"),
            source: TAG,
            table: "Table 5",
            ..rule("r2")
        },
    ),
];

/// `Calibrate.RULES[id]`; panics for an id that is not a rule.
pub fn rule_of(id: &str) -> &'static Rule {
    RULES
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, r)| r)
        .unwrap_or_else(|| panic!("no calibration rule {id:?}"))
}

/// `Calibrate.MIN_SITES`: fewer sites than this are "Indicative", never pass or fail (Spare Construct convention).
pub const MIN_SITES: usize = 10;

/// `flowOK(m, o)`: within the TAG M3.1 Table 2 flow band (100 veh/h below 700, 15% to 2,700, 400 veh/h above).
pub fn flow_ok(m: f64, o: f64) -> bool {
    if o < 700.0 {
        (m - o).abs() <= 100.0
    } else if o <= 2700.0 {
        (m - o).abs() <= 0.15 * o
    } else {
        (m - o).abs() <= 400.0
    }
}

/// `bandOK(m, o)`: within the TMDG count band (50 veh/h below 400, 12.5% to 2,000, 250 veh/h above).
pub fn band_ok(m: f64, o: f64) -> bool {
    if o < 400.0 {
        (m - o).abs() <= 50.0
    } else if o <= 2000.0 {
        (m - o).abs() <= 0.125 * o
    } else {
        (m - o).abs() <= 250.0
    }
}

/// `pearson2(x, y)`: the square of Pearson's r, None for fewer than two pairs or no spread.
pub fn pearson2(x: &[f64], y: &[f64]) -> Option<f64> {
    let n = x.len();
    if n < 2 {
        return None;
    }
    let (mut mx, mut my) = (0.0f64, 0.0f64);
    for i in 0..n {
        mx += x[i];
        my += y[i];
    }
    mx /= n as f64;
    my /= n as f64;
    let (mut sxy, mut sxx, mut syy) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let a = x[i] - mx;
        let b = y[i] - my;
        sxy += a * b;
        sxx += a * a;
        syy += b * b;
    }
    if sxx > 0.0 && syy > 0.0 {
        Some(sxy * sxy / (sxx * syy))
    } else {
        None
    }
}

/// The through-origin fit `Calibrate.origin` gives: slope b0 and the two R² (centred on the mean of y, and against Σy²).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct OriginFit {
    /// Slope through the origin.
    #[serde(with = "plain")]
    pub b0: Option<f64>,
    /// R² centred on the mean of y (the TMDG test).
    #[serde(with = "plain")]
    pub r2c: Option<f64>,
    /// R² against Σy² (Excel's).
    #[serde(with = "plain")]
    pub r2e: Option<f64>,
}

/// `Calibrate.origin(x, y)`: fits y = b0 x through the origin.
pub fn origin(x: &[f64], y: &[f64]) -> OriginFit {
    let none = OriginFit {
        b0: None,
        r2c: None,
        r2e: None,
    };
    let n = x.len();
    if n < 2 {
        return none;
    }
    let (mut sxy, mut sxx, mut sy, mut syy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        sxy += x[i] * y[i];
        sxx += x[i] * x[i];
        sy += y[i];
        syy += y[i] * y[i];
    }
    if !(sxx > 0.0) {
        return none;
    }
    let b0 = sxy / sxx;
    let my = sy / n as f64;
    let (mut se, mut st) = (0.0f64, 0.0f64);
    for i in 0..n {
        let e = y[i] - b0 * x[i];
        let t = y[i] - my;
        se += e * e;
        st += t * t;
    }
    OriginFit {
        b0: Some(b0),
        r2c: if st > 0.0 { Some(1.0 - se / st) } else { None },
        r2e: if syy > 0.0 {
            Some(1.0 - se / syy)
        } else {
            None
        },
    }
}

/// The ordinary least-squares fit `Calibrate.ols` gives: y = a + b x.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct OlsFit {
    /// Intercept.
    #[serde(with = "plain")]
    pub a: Option<f64>,
    /// Slope.
    #[serde(with = "plain")]
    pub b: Option<f64>,
}

/// `Calibrate.ols(x, y)`: ordinary least squares, reported beside the through-origin fit (never the pass or fail test).
pub fn ols(x: &[f64], y: &[f64]) -> OlsFit {
    let n = x.len();
    if n < 2 {
        return OlsFit { a: None, b: None };
    }
    let (mut mx, mut my) = (0.0f64, 0.0f64);
    for i in 0..n {
        mx += x[i];
        my += y[i];
    }
    mx /= n as f64;
    my /= n as f64;
    let (mut sxy, mut sxx) = (0.0f64, 0.0f64);
    for i in 0..n {
        let a = x[i] - mx;
        sxy += a * (y[i] - my);
        sxx += a * a;
    }
    if sxx > 0.0 {
        OlsFit {
            a: Some(my - sxy / sxx * mx),
            b: Some(sxy / sxx),
        }
    } else {
        OlsFit { a: None, b: None }
    }
}

/// One GEH band of `Calibrate.gehBands`: its limits, label, count and share, and the cumulative share.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GehBand {
    /// Lower limit (included).
    #[serde(with = "plain")]
    pub lo: f64,
    /// Upper limit (excluded); infinity for the last.
    #[serde(with = "plain")]
    pub hi: f64,
    /// "< 5", "5\u{2013}7.5", "\u{2265} 12".
    pub label: String,
    /// Sites in the band.
    pub n: usize,
    /// Share of sites.
    #[serde(with = "plain")]
    pub share: Option<f64>,
    /// Cumulative share.
    #[serde(with = "plain")]
    pub cum: Option<f64>,
}

/// `Calibrate.GEH_BANDS`: the bands of the criteria library.
pub const GEH_BANDS: [(f64, f64); 5] = [
    (0.0, 5.0),
    (5.0, 7.5),
    (7.5, 10.0),
    (10.0, 12.0),
    (12.0, f64::INFINITY),
];

/// `Calibrate.gehBands(rows)`: count and cumulative share of sites per GEH band.
pub fn geh_bands(gehs: &[f64]) -> Vec<GehBand> {
    use crate::jsnum::number_to_js_string as t;
    let n = gehs.len();
    let mut cum = 0usize;
    GEH_BANDS
        .iter()
        .map(|&(lo, hi)| {
            let k = gehs.iter().filter(|&&g| g >= lo && g < hi).count();
            cum += k;
            GehBand {
                lo,
                hi,
                label: if hi == f64::INFINITY {
                    format!("\u{2265} {}", t(lo))
                } else if lo == 0.0 {
                    format!("< {}", t(hi))
                } else {
                    format!("{}\u{2013}{}", t(lo), t(hi))
                },
                n: k,
                share: if n > 0 {
                    Some(k as f64 / n as f64)
                } else {
                    None
                },
                cum: if n > 0 {
                    Some(cum as f64 / n as f64)
                } else {
                    None
                },
            }
        })
        .collect()
}

/// The statistics of a set of counted sites (`Calibrate.stats`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// Number of sites.
    pub n: usize,
    /// Share with GEH < 5.
    #[serde(with = "plain")]
    pub pass: Option<f64>,
    /// Share with GEH < 7.5.
    #[serde(with = "plain")]
    pub pass75: Option<f64>,
    /// Share with GEH < 10.
    #[serde(with = "plain")]
    pub pass10: Option<f64>,
    /// Share within the TMDG count band.
    #[serde(with = "plain")]
    pub band: Option<f64>,
    /// Share with GEH < 5 or within the TAG flow band.
    #[serde(with = "plain")]
    pub flow: Option<f64>,
    /// Mean GEH.
    #[serde(with = "plain")]
    pub geh: Option<f64>,
    /// R² through the origin, centred (the TMDG test).
    #[serde(with = "plain")]
    pub r2: Option<f64>,
    /// R² through the origin against Σy² (Excel's).
    #[serde(with = "plain")]
    pub r2_excel: Option<f64>,
    /// R² of the ordinary least-squares fit.
    #[serde(with = "plain")]
    pub r2_ols: Option<f64>,
    /// Slope through the origin.
    #[serde(with = "plain")]
    pub slope: Option<f64>,
    /// OLS slope.
    #[serde(with = "plain")]
    pub ols_slope: Option<f64>,
    /// OLS intercept.
    #[serde(with = "plain")]
    pub ols_intercept: Option<f64>,
    /// GEH bands.
    pub geh_bands: Vec<GehBand>,
    /// %RMSE.
    #[serde(with = "plain")]
    pub rmse: Option<f64>,
    /// Σ counted (veh/h).
    #[serde(with = "plain")]
    pub tot_obs: f64,
    /// Σ modelled (veh/h).
    #[serde(with = "plain")]
    pub tot_mod: f64,
}

/// `Calibrate.stats(rows)` for sites given as (counted, modelled, GEH): shares under each GEH limit, within the bands, mean GEH, the fits and %RMSE.
pub fn stats_of(obs_model_geh: &[(f64, f64, f64)]) -> Stats {
    let n = obs_model_geh.len();
    if n == 0 {
        return Stats {
            n: 0,
            pass: None,
            pass75: None,
            pass10: None,
            band: None,
            flow: None,
            geh: None,
            r2: None,
            r2_excel: None,
            r2_ols: None,
            slope: None,
            ols_slope: None,
            ols_intercept: None,
            geh_bands: geh_bands(&[]),
            rmse: None,
            tot_obs: 0.0,
            tot_mod: 0.0,
        };
    }
    let (mut so, mut sm, mut g, mut se) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut p5, mut p75, mut p10, mut fl, mut bd) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for &(obs, model, geh) in obs_model_geh {
        so += obs;
        sm += model;
        g += geh;
        let d = model - obs;
        se += d * d;
        if geh < 5.0 {
            p5 += 1;
        }
        if geh < 7.5 {
            p75 += 1;
        }
        if geh < 10.0 {
            p10 += 1;
        }
        if geh < 5.0 || flow_ok(model, obs) {
            fl += 1;
        }
        if band_ok(model, obs) {
            bd += 1;
        }
    }
    let x: Vec<f64> = obs_model_geh.iter().map(|r| r.0).collect();
    let y: Vec<f64> = obs_model_geh.iter().map(|r| r.1).collect();
    let gehs: Vec<f64> = obs_model_geh.iter().map(|r| r.2).collect();
    let o = origin(&x, &y);
    let l = ols(&x, &y);
    let nf = n as f64;
    Stats {
        n,
        pass: Some(p5 as f64 / nf),
        pass75: Some(p75 as f64 / nf),
        pass10: Some(p10 as f64 / nf),
        band: Some(bd as f64 / nf),
        flow: Some(fl as f64 / nf),
        geh: Some(g / nf),
        r2: o.r2c,
        r2_excel: o.r2e,
        r2_ols: pearson2(&x, &y),
        slope: o.b0,
        ols_slope: l.b,
        ols_intercept: l.a,
        geh_bands: geh_bands(&gehs),
        rmse: if n > 1 && so > 0.0 {
            Some(100.0 * (se / (n - 1) as f64).sqrt() / (so / nf))
        } else {
            None
        },
        tot_obs: so,
        tot_mod: sm,
    }
}
