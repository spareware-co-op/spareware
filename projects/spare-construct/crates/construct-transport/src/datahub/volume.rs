//! A count as a peak-hour volume (`periodVol`), its observation method and its weight.

use super::{AsText, Count, is_estimate, obs_method_of_key};
use crate::jsnum::{Maybe, string_to_number, value_to_number, value_truthy};
use crate::project::Settings;
use crate::util::fmt;
use serde_json::Value;

/// What `DataHub.periodVol` gives for a count: the volume (veh/h), how it was made and the observation method it rests on.
#[derive(Clone, Debug, PartialEq)]
pub struct PeriodVol {
    /// Volume in the modelled period (veh/h).
    pub v: f64,
    /// How the volume was made ("8.5% of daily", "counted peak hour (07:45)").
    pub how: String,
    /// Observation method key: "measured_hour", "weekday_factored", "adt_factored", "ramm_estimate" or "virtual".
    pub basis: &'static str,
}

fn pv(v: f64, how: String, basis: &'static str) -> Option<PeriodVol> {
    Some(PeriodVol { v, how, basis })
}

// `+s.slice(a, b)` for text read as UTF-16 code units (counts hold ASCII times)
fn slice_num(s: &str, a: usize, b: Option<usize>) -> f64 {
    let u: Vec<u16> = s.encode_utf16().collect();
    let end = b.unwrap_or(u.len()).min(u.len());
    let part = if a >= end {
        String::new()
    } else {
        String::from_utf16_lossy(&u[a..end])
    };
    string_to_number(&part)
}

// `typeof x === 'object' && x`, then x[key] read through `+x`: NaN for anything else (undefined)
fn obj_num(v: &Maybe<Value>, key: &str) -> f64 {
    match v.get() {
        Some(Value::Object(m)) => m.get(key).map_or(f64::NAN, value_to_number),
        _ => f64::NAN,
    }
}

/// `DataHub.periodVol(c, period)`: a count as a modelled-period volume, or None when it gives none.
///
/// A user count gives its own AM or PM hour, else its daily volume times the Settings share. An AT count gives its
/// counted peak hour when that falls in the period's window (AM 6 to 10, PM 14:30 to 19), else the daily volume
/// times the share. A RAMM estimate gives its daily volume times the share. An NZTA count gives the TMS Tue to Thu
/// term-time mean (not flagged indicative) or the AADT, times the site's own TMS peak share when it has one, else
/// the Settings share. `period` is "AM" or "PM" (anything else reads as PM, as the prototype does).
pub fn period_vol(c: &Count, period: &str, s: &Settings) -> Option<PeriodVol> {
    let am = period == "AM";
    let pm = period == "PM";
    let data = s.data.get();
    let (share_n, share_txt) = match data {
        Some(d) => {
            let raw = if am { &d.am_share } else { &d.pm_share };
            (raw.num() / 100.0, format!("{}% of daily", raw.js_string()))
        }
        None => (f64::NAN, "undefined% of daily".to_string()),
    };
    let share = share_n;
    if c.src_is("user") {
        let v = if am { &c.am } else { &c.pm };
        if v.truthy() {
            return pv(v.num(), "your count".to_string(), "measured_hour");
        }
        return if c.adt.truthy() {
            pv(c.adt.num() * share, share_txt, "adt_factored")
        } else {
            None
        };
    }
    if c.src_is("at") && c.peak.truthy() && c.peak_time.truthy() {
        let pt = c.peak_time.js_string();
        let h = slice_num(&pt, 0, Some(2)) + slice_num(&pt, 3, None) / 60.0;
        if (am && h >= 6.0 && h < 10.0) || (pm && h >= 14.5 && h < 19.0) {
            return pv(
                c.peak.num(),
                format!("counted peak hour ({})", pt),
                "measured_hour",
            );
        }
    }
    if is_estimate(c) {
        return if c.adt.num() > 0.0 {
            pv(
                c.adt.num() * share,
                format!("RAMM estimate \u{d7} {}", share_txt),
                "ramm_estimate",
            )
        } else {
            None
        };
    }
    if c.src_is("nzta") {
        // const t = c.tms && typeof c.tms === 'object' ? c.tms.target : null
        let t: Option<&Value> = match c.tms.get() {
            Some(Value::Object(m)) => m.get("target"),
            Some(Value::Array(_)) => None,
            _ => None,
        };
        let t_truthy = t.is_some_and(value_truthy);
        let t_obj = match t {
            Some(Value::Object(m)) if t_truthy => Some(m),
            _ => None,
        };
        let mean = t_obj.map_or(f64::NAN, |m| {
            m.get("mean").map_or(f64::NAN, value_to_number)
        });
        let ok_not_false = t_obj.is_none_or(|m| m.get("ok") != Some(&Value::Bool(false)));
        let tm = if t_truthy && mean > 0.0 && mean.is_finite() && ok_not_false {
            Some(mean)
        } else {
            None
        };
        let ps = obj_num(&c.share, period);
        let own = if ps > 0.0 && ps < 1.0 { Some(ps) } else { None };
        let daily = match tm {
            Some(m) => m,
            None => c.adt.num(),
        };
        if !(daily > 0.0) {
            return None;
        }
        let virt = c.virtual_.truthy();
        if tm.is_none() && own.is_none() {
            return pv(
                daily * share,
                share_txt,
                if virt { "virtual" } else { "adt_factored" },
            );
        }
        let how = format!(
            "{} \u{d7} {}",
            match tm {
                Some(m) => format!("TMS Tue\u{2013}Thu term-time mean ({}/day)", fmt::int(m)),
                None => "AADT".to_string(),
            },
            match own {
                Some(o) => format!("{}% (TMS peak profile)", fmt::num(o * 100.0, 1)),
                None => share_txt,
            }
        );
        return pv(
            daily * own.unwrap_or(share),
            how,
            if virt {
                "virtual"
            } else if tm.is_some() {
                "weekday_factored"
            } else {
                "adt_factored"
            },
        );
    }
    if c.adt.truthy() {
        pv(
            c.adt.num() * share,
            share_txt,
            if c.virtual_.truthy() {
                "virtual"
            } else {
                "adt_factored"
            },
        )
    } else {
        None
    }
}

/// `DataHub.obsMethod(c, period)`: the observation method of a count in a period (an importer's own `obs_method` wins, then RAMM estimate, virtual, then what the volume rests on).
pub fn obs_method(c: &Count, period: &str, s: &Settings) -> String {
    if let Some(m) = c.obs_method.as_text()
        && obs_method_of_key(m).is_some()
    {
        return m.to_string();
    }
    if is_estimate(c) {
        return "ramm_estimate".to_string();
    }
    if c.virtual_.truthy() {
        return "virtual".to_string();
    }
    period_vol(c, period, s)
        .map(|p| p.basis.to_string())
        .unwrap_or_else(|| "adt_factored".to_string())
}

/// `DataHub.countWeight(c, period)`: the matrix-estimation weight of a count by its observation method (measured 1, weekday 0.7, ADT 0.4, RAMM 0.25, virtual 0).
pub fn count_weight(c: &Count, period: &str, s: &Settings) -> f64 {
    obs_method_of_key(&obs_method(c, period, s)).map_or(0.4, |m| m.weight)
}
