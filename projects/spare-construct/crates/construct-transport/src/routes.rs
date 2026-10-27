//! Bus route frequencies and loads from engine.js.
//!
//! The buses an hour a route runs in a period (its own GTFS frequency for the period, else the
//! route's single frequency) and the passenger car units a bus counts for in the preload. The
//! assignment preloads route arcs with them and the simulation releases buses at the frequency.
//! Port of engine.js `busFreq` and `busPcuOf`.

use crate::jsnum::{math_max, value_to_number};
use crate::project::{Route, Settings};
use crate::standards::STANDARDS;
use serde_json::Value;

/// `busFreq(r, period)`: buses an hour on a route in a period.
///
/// The period's own frequency (`r.freqs[period]`, from GTFS) when it is set and a finite number
/// (text such as "12" counts, empty text and null do not); otherwise the route's `freq`, read as
/// `+freq || 0`. Never below 0. An unset period reads the key "undefined", as `r.freqs[undefined]` does.
pub fn bus_freq(r: &Route, period: Option<&str>) -> f64 {
    if let Some(Value::Object(freqs)) = r.freqs.get()
        && let Some(f) = freqs.get(period.unwrap_or("undefined"))
        && !f.is_null()
        && f.as_str() != Some("")
    {
        let x = value_to_number(f);
        if x.is_finite() {
            return math_max(0.0, x);
        }
    }
    math_max(0.0, r.freq.or(0.0))
}

/// `busPcuOf(S)`: passenger car units per bus for the route preload.
///
/// `settings.busPcu` when it is a positive finite number, otherwise the engine's earlier value
/// (for a settings object saved before the setting existed).
pub fn bus_pcu_of(s: &Settings) -> f64 {
    let p = s.bus_pcu.num();
    if p > 0.0 && p.is_finite() {
        p
    } else {
        STANDARDS.bus.legacy
    }
}
