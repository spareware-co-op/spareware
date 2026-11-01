//! The report's number formats (`R`): one decimal rules for each kind of figure; a missing or non-finite value prints an en dash.
//! Values are `f64` with NaN for missing; `v` reads a loose project value the way `v == null || !isFinite(v)` does.

use crate::edit::json::JsonValue;
use crate::util::fmt;

/// A loose project value as the number the formats see: missing and null are NaN.
pub(crate) fn v(x: Option<&JsonValue>) -> f64 {
    match x {
        None | Some(JsonValue::Null) => f64::NAN,
        Some(x) => x.to_number(),
    }
}

/// `R.veh`: whole veh/h in tables.
pub(crate) fn veh(x: f64) -> String {
    if x.is_finite() {
        fmt::int(x)
    } else {
        fmt::DASH.to_string()
    }
}

/// `R.adt`: ADT to the nearest 10 vpd.
pub(crate) fn adt(x: f64) -> String {
    if x > 0.0 && x.is_finite() {
        fmt::int(crate::jsnum::math_round(x / 10.0) * 10.0)
    } else {
        fmt::DASH.to_string()
    }
}

/// `R.delay`: seconds to 1 decimal, "> 300" above 300.
pub(crate) fn delay(d: f64) -> String {
    if !d.is_finite() {
        fmt::DASH.to_string()
    } else if d > 300.0 {
        "> 300".to_string()
    } else {
        fmt::num(d, 1)
    }
}

/// `R.dos`: a ratio to 2 decimals.
pub(crate) fn dos(x: f64) -> String {
    if x.is_finite() {
        fmt::num(x, 2)
    } else {
        fmt::DASH.to_string()
    }
}

/// `R.q`: queues in whole metres.
pub(crate) fn q(m: f64) -> String {
    if m.is_finite() {
        fmt::int(m)
    } else {
        fmt::DASH.to_string()
    }
}

/// `R.pct`: a percentage (already in %) to 1 decimal.
pub(crate) fn pct(p: f64) -> String {
    if p.is_finite() {
        format!("{}%", fmt::num(p, 1))
    } else {
        fmt::DASH.to_string()
    }
}

/// `R.num(x, d)`.
pub(crate) fn num(x: f64, d: usize) -> String {
    if x.is_finite() {
        fmt::num(x, d)
    } else {
        fmt::DASH.to_string()
    }
}
