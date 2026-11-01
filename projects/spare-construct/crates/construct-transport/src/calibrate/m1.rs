//! Table M1 of the matrix estimation report (TMDG section 8, TAG M3.1 Table 5): the records `meReport` makes and the
//! checks `meStats` makes of them.

use super::criteria::tag;
use crate::jsnum::Num;
use crate::project::Project;
use crate::results::plain;
use crate::util::fmt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Fit of matrix cells, prior against estimated (OLS).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct M1Cells {
    /// Cells that carry trips in the prior.
    pub n: usize,
    /// OLS slope.
    #[serde(with = "plain")]
    pub slope: Option<f64>,
    /// OLS intercept.
    #[serde(with = "plain")]
    pub intercept: Option<f64>,
    /// OLS R².
    #[serde(with = "plain")]
    pub r2: Option<f64>,
    /// Mean prior cell (veh/h).
    #[serde(with = "plain")]
    pub mean_prior: f64,
    /// The intercept is more than 1% of the mean prior cell.
    pub intercept_flag: bool,
}

/// Fit of trip ends, prior against estimated (OLS).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct M1Ends {
    /// OLS slope.
    #[serde(with = "plain")]
    pub slope: Option<f64>,
    /// OLS intercept.
    #[serde(with = "plain")]
    pub intercept: Option<f64>,
    /// OLS R².
    #[serde(with = "plain")]
    pub r2: Option<f64>,
    /// Zones counted.
    #[serde(with = "plain")]
    pub n: f64,
}

/// Trip ends for all zones and without the gateways.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct M1EndsBoth {
    /// All zones.
    pub all: M1Ends,
    /// Without gateways.
    pub no_gateways: M1Ends,
}

/// Trip-length distribution, prior against estimated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct M1Tld {
    /// Distance band labels.
    pub bins: Vec<String>,
    /// Share of prior trips per band.
    #[serde(with = "plain")]
    pub prior: Vec<f64>,
    /// Share of estimated trips per band.
    #[serde(with = "plain")]
    pub est: Vec<f64>,
    /// Mean trip length, prior (m).
    #[serde(with = "plain")]
    pub mean_p: f64,
    /// Mean trip length, estimated (m).
    #[serde(with = "plain")]
    pub mean_e: f64,
    /// Standard deviation, prior (m).
    #[serde(rename = "sdP", with = "plain")]
    pub sd_p: f64,
    /// Standard deviation, estimated (m).
    #[serde(rename = "sdE", with = "plain")]
    pub sd_e: f64,
    /// Mean change (%).
    #[serde(with = "plain")]
    pub mean_pct: f64,
    /// Standard deviation change (%).
    #[serde(with = "plain")]
    pub sd_pct: f64,
    /// Coincidence ratio.
    #[serde(with = "plain")]
    pub cr: Option<f64>,
    /// What the lengths are measured on.
    pub basis: String,
}

/// Trip-end GEH bands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct M1TripEndGeh {
    /// Band labels.
    pub bands: Vec<String>,
    /// Trip ends per band.
    pub n: Vec<usize>,
}

/// How many cells changed by how much.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct M1CellChange {
    /// Cells changed by more than 50%.
    pub over50: usize,
    /// Cells changed by 40 to 50%.
    pub b40: usize,
    /// Cells changed by 30 to 40%.
    pub b30: usize,
    /// Cells that carry trips.
    pub n: usize,
}

/// Sector-to-sector check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct M1Sectors {
    /// Sectors (including the gateways).
    pub n: usize,
    /// Sector cells tested.
    pub tested: usize,
    /// Sector cells within 5%.
    pub within5: usize,
    /// What was tested.
    pub note: String,
}

/// One of the largest cell changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct M1Top {
    /// Origin zone name.
    pub from: String,
    /// Destination zone name.
    pub to: String,
    /// Prior trips (veh/h).
    #[serde(with = "plain")]
    pub prior: f64,
    /// Estimated trips (veh/h).
    #[serde(with = "plain")]
    pub est: f64,
    /// Change (veh/h).
    #[serde(with = "plain")]
    pub d: f64,
    /// Change as a share of the prior.
    #[serde(with = "plain")]
    pub pct: f64,
}

/// The matrix estimation report of Table M1 (`meReport`): prior against estimated, cells, trip ends, trip lengths, sectors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct M1 {
    /// When it was made.
    pub date: Value,
    /// Period.
    pub period: String,
    /// Σ prior (veh/h).
    #[serde(with = "plain")]
    pub prior_total: f64,
    /// Σ estimated (veh/h).
    #[serde(with = "plain")]
    pub total: f64,
    /// Change in the total (%).
    #[serde(with = "plain")]
    pub total_pct: f64,
    /// Cells.
    pub cells: M1Cells,
    /// Trip ends.
    pub ends: M1EndsBoth,
    /// Trip lengths.
    pub tld: M1Tld,
    /// Trip-end GEH.
    pub trip_end_geh: M1TripEndGeh,
    /// Cell changes.
    pub cell_change: M1CellChange,
    /// Sectors.
    pub sectors: M1Sectors,
    /// The ten largest cell changes.
    pub top: Vec<M1Top>,
}

/// The pair of a TAG matrix test: whether it is met and in words.
#[derive(Clone, Debug, PartialEq)]
pub struct PairVal {
    /// Met.
    pub ok: bool,
    /// In words ("slope 1.002, R² 0.991").
    pub text: String,
}

/// `meStats(period)`: the matrix estimation checks of a period's fit record (None without a record).
#[derive(Clone, Debug)]
pub struct MeStats {
    /// 1 when there is a record, 0 when not.
    pub n: usize,
    /// |change in the total| (%).
    pub total_abs: f64,
    /// Change in the total (%).
    pub total_pct: f64,
    /// Cells test.
    pub cells: PairVal,
    /// Trip ends test.
    pub ends: PairVal,
    /// Trip-length mean and SD test (when Table M1 was made).
    pub tld: Option<PairVal>,
    /// Trip-length coincidence ratio.
    pub cr: Option<f64>,
}

fn num(x: Option<f64>, d: usize) -> String {
    match x {
        Some(v) if v.is_finite() => fmt::num(v, d),
        _ => "\u{2013}".to_string(),
    }
}

fn mc_field(mc: &Value, k: &str) -> Option<f64> {
    match mc.get(k) {
        Some(Value::Null) | None => None,
        Some(v) => {
            let x = crate::jsnum::value_to_number(v);
            Some(x)
        }
    }
}

/// `meStats(period)` for the project's record of the period: None when there is no record.
pub fn me_stats(p: &Project, period: &str) -> Option<MeStats> {
    let info =
        p.me.get()
            .and_then(|m| m.info.get())
            .and_then(|i| i.get(period))
            .and_then(|i| i.get())?;
    let m1: Option<M1> = info
        .extra
        .get("m1")
        .filter(|v| !v.is_null())
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let empty = Value::Object(serde_json::Map::new());
    let mc = info
        .extra
        .get("matrixChange")
        .filter(|v| !v.is_null())
        .unwrap_or(&empty);
    let (cells_slope, cells_r2, cells_intercept, cells_flag) = match &m1 {
        Some(m) => (
            m.cells.slope,
            m.cells.r2,
            m.cells.intercept,
            m.cells.intercept_flag,
        ),
        None => (
            mc_field(mc, "cellSlope"),
            mc_field(mc, "cellR2"),
            None,
            false,
        ),
    };
    let (ends_slope, ends_r2) = match &m1 {
        Some(m) => (m.ends.all.slope, m.ends.all.r2),
        None => (mc_field(mc, "rowSlope"), mc_field(mc, "tripEndR2")),
    };
    let total_pct = match &m1 {
        Some(m) => m.total_pct,
        None => mc_field(mc, "totalPct").unwrap_or(f64::NAN),
    };
    let cells_ok = cells_slope
        .is_some_and(|s| s >= tag::CELLS_SLOPE[0] && s <= tag::CELLS_SLOPE[1])
        && cells_r2.is_some_and(|r| r > tag::CELLS_R2)
        && !(m1.is_some() && cells_flag);
    let ends_ok = ends_slope.is_some_and(|s| s >= tag::ENDS_SLOPE[0] && s <= tag::ENDS_SLOPE[1])
        && ends_r2.is_some_and(|r| r > tag::ENDS_R2);
    let tld = m1.as_ref().map(|m| PairVal {
        ok: m.tld.mean_pct.abs() <= tag::TLD_V && m.tld.sd_pct.abs() <= tag::TLD_V,
        text: format!(
            "mean {}, SD {}",
            fmt::signed_pct(m.tld.mean_pct / 100.0, 1),
            fmt::signed_pct(m.tld.sd_pct / 100.0, 1)
        ),
    });
    Some(MeStats {
        n: 1,
        total_abs: total_pct.abs(),
        total_pct,
        cells: PairVal {
            ok: cells_ok,
            text: format!(
                "slope {}, R\u{b2} {}{}",
                num(cells_slope, 3),
                num(cells_r2, 3),
                if m1.is_some() {
                    format!(", intercept {}", num(cells_intercept, 2))
                } else {
                    String::new()
                }
            ),
        },
        ends: PairVal {
            ok: ends_ok,
            text: format!("slope {}, R\u{b2} {}", num(ends_slope, 3), num(ends_r2, 3)),
        },
        tld,
        cr: m1.as_ref().and_then(|m| m.tld.cr),
    })
}

#[allow(dead_code)]
fn _keep(_: &Num) {}
