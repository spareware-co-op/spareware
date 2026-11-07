//! The CAS query window and the crash tuple (`casWindow`, `nzDate`, `casTuple`, `CAS_FIELDS`).

use crate::edit::json::JsonValue;
use crate::feeds::nztime::nz_parts;
use crate::jsnum::{math_max, number_to_js_string, value_to_number};
use crate::util::r7;
use serde::Serialize;
use serde_json::Value;

/// `CAS_WINDOW.years`: full calendar years of crashes in the main tables.
pub const CAS_YEARS_DEFAULT: f64 = 5.0;
/// `CAS_WINDOW.longYears`: the longer window for low-volume roads and state highway accesses.
pub const CAS_LONG_YEARS: f64 = 10.0;
/// `CAS_WINDOW.lagMonths`: months a year may still be incomplete in CAS.
pub const CAS_LAG_MONTHS: f64 = 3.0;
/// `CAS_WINDOW.ytdLabel`: the label of the year to date.
pub const CAS_YTD_LABEL: &str = "YTD, incomplete";
/// `CAS_WINDOW.source`.
pub const CAS_SOURCE: &str = "NZTA CAS; MBCM v1.7.5 crash analysis period";

/// The extra CAS fields a crash tuple keeps, at positions 7 and up (`CAS_FIELDS`).
pub const CAS_FIELDS: [&str; 8] = [
    "fatalCount",
    "seriousInjuryCount",
    "minorInjuryCount",
    "light",
    "weatherA",
    "OBJECTID",
    "trafficControl",
    "urban",
];

/// `CAS_POS[field]`: where a field sits in a crash tuple (7 and up), None for a field a tuple does not keep.
pub fn cas_pos(field: &str) -> Option<usize> {
    CAS_FIELDS.iter().position(|f| *f == field).map(|i| 7 + i)
}

/// `CAS_OUT`: the fields asked of the crash service.
pub const CAS_OUT: &str = "OBJECTID,crashSeverity,crashYear,crashLocation1,crashLocation2,speedLimit,pedestrian,bicycle,motorcycle,fatalCount,seriousInjuryCount,minorInjuryCount,light,weatherA,trafficControl,urban";

/// What `casWindow` gives: the years of crashes to ask for and the notes the tables carry.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CasWindow {
    /// First full year.
    pub from: f64,
    /// Last full year.
    pub to: f64,
    /// The year to date.
    pub ytd: f64,
    /// Full years in the window.
    pub years: f64,
    /// The last full year when it may still be incomplete (before 1 April), else null.
    pub lag_year: Option<f64>,
    /// "YTD, incomplete".
    pub ytd_label: String,
    /// The note about CAS entry lag ("" when none applies).
    pub lag_note: String,
    /// Text for the tables.
    pub label: String,
    /// The service query's where clause.
    #[serde(rename = "where")]
    pub where_: String,
}

/// `nzDate(now)`: the New Zealand year, month and day of an instant (milliseconds since 1970).
pub fn nz_date(now_ms: f64) -> (f64, f64, f64) {
    let p = nz_parts(now_ms);
    (p.year as f64, p.month as f64, p.day as f64)
}

/// `casWindow(now, years)`: the last `years` full calendar years (the year before this one back), plus this year to date.
///
/// `years` is rounded and at least 1; None (or a value that is not a number, or 0) gives 5. Before 1 April the last full
/// year is flagged as possibly incomplete.
pub fn cas_window(now_ms: f64, years: Option<f64>) -> CasWindow {
    let (y, m, _) = nz_date(now_ms);
    let want = match years {
        Some(v) if v != 0.0 && !v.is_nan() => v,
        _ => CAS_YEARS_DEFAULT,
    };
    // Math.max(1, Math.round(+years || 5)); the +years of a missing value is NaN, which || turns into 5
    let n = math_max(1.0, crate::jsnum::math_round(want));
    let from = y - n;
    let to = y - 1.0;
    let lag_year = if m < 4.0 { Some(to) } else { None };
    let nt = number_to_js_string(n);
    CasWindow {
        from,
        to,
        ytd: y,
        years: n,
        lag_year,
        ytd_label: CAS_YTD_LABEL.to_string(),
        lag_note: match lag_year {
            Some(l) => format!(
                "{} may be incomplete (CAS entry lag up to {} months, MBCM)",
                number_to_js_string(l),
                number_to_js_string(CAS_LAG_MONTHS)
            ),
            None => String::new(),
        },
        label: format!(
            "{}\u{2013}{} ({} full years), plus {} to date ({})",
            number_to_js_string(from),
            number_to_js_string(to),
            nt,
            number_to_js_string(y),
            CAS_YTD_LABEL
        ),
        where_: format!("crashYear>={}", number_to_js_string(from)),
    }
}

// (c) => c.toUpperCase() for a lower case ASCII letter
// title(s): String(s || '').toLowerCase().replace(/\b[a-z]/g, c => c.toUpperCase())
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `title(s)` of data.js: lower case, then a capital on every a to z letter that starts a word (after anything but a letter, digit or underscore, so "mary's" is "Mary'S").
pub fn title(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut prev_word = false;
    for c in lower.chars() {
        if c.is_ascii_lowercase() && !prev_word {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c);
        }
        prev_word = is_word_char(c);
    }
    out
}

/// A JSON value as `Value`, `undefined` (absent) as null (as a saved array holds it).
fn val(v: Option<&JsonValue>) -> Value {
    v.map_or(Value::Null, JsonValue::to_value)
}

/// `casTuple(x)`: one crash record from the service's answer as a tuple, the project's compact form.
///
/// `[0]` latitude, `[1]` longitude, `[2]` severity F, S, M or N, `[3]` year, `[4]` location, `[5]` flags (p walking, c cycling,
/// m motorcycle), `[6]` speed limit, then at fixed positions the extra fields of [`CAS_FIELDS`] (counts are numbers, the rest
/// text, unknown is null, and trailing nulls are left off). `x` is one feature of the answer (`attributes` and `geometry` with x and y).
pub fn cas_tuple(x: &JsonValue) -> Vec<Value> {
    let empty = JsonValue::object();
    let a = match x.get("attributes") {
        Some(v) if v.truthy() => v,
        _ => &empty,
    };
    let sev_text = {
        let v = a.get("crashSeverity");
        match v {
            Some(v) if v.truthy() => v.js_string(),
            _ => "N".to_string(),
        }
    };
    // String(...)[0].toUpperCase(): the first UTF-16 unit
    let first: String = sev_text
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default();
    let s = if "FSMN".contains(first.as_str()) && !first.is_empty() {
        first
    } else {
        "N".to_string()
    };
    let geom = x.get("geometry");
    let coord = |k: &str| -> f64 {
        r7(geom
            .and_then(|g| g.get(k))
            .map_or(f64::NAN, JsonValue::to_number))
    };
    let locations: Vec<String> = ["crashLocation1", "crashLocation2"]
        .iter()
        .filter_map(|k| a.get(k))
        .filter(|v| v.truthy())
        .map(|v| title(&v.js_string()))
        .collect();
    let mut flags = String::new();
    for (k, f) in [("pedestrian", 'p'), ("bicycle", 'c'), ("motorcycle", 'm')] {
        if a.get(k).is_some_and(JsonValue::truthy) {
            flags.push(f);
        }
    }
    let mut t: Vec<Value> = vec![
        crate::jsnum::json_number(coord("y")),
        crate::jsnum::json_number(coord("x")),
        Value::String(s),
        val(a.get("crashYear")),
        Value::String(locations.join(" / ")),
        Value::String(flags),
        val(a.get("speedLimit")),
    ];
    // num: null for a missing, empty or non-finite value, else the number; txt: null for missing or empty, else the text
    let mut extra: Vec<Value> = CAS_FIELDS
        .iter()
        .map(|k| {
            let v = a.get(k);
            let missing = match v {
                None | Some(JsonValue::Null) => true,
                Some(JsonValue::String(s)) => s.is_empty(),
                _ => false,
            };
            if k.ends_with("Count") || *k == "OBJECTID" {
                match v {
                    _ if missing => Value::Null,
                    Some(v) => {
                        let n = v.to_number();
                        if n.is_finite() {
                            crate::jsnum::json_number(n)
                        } else {
                            Value::Null
                        }
                    }
                    None => Value::Null,
                }
            } else if missing {
                Value::Null
            } else {
                Value::String(v.map(JsonValue::js_string).unwrap_or_default())
            }
        })
        .collect();
    while extra.last().is_some_and(Value::is_null) {
        extra.pop();
    }
    t.extend(extra);
    t
}

/// A crash tuple read for the analysis: its position, severity, year, location text and flags, and the tuple itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Crash {
    /// `[0]` latitude.
    pub lat: f64,
    /// `[1]` longitude.
    pub lng: f64,
    /// `[2]` severity: F, S, M or N.
    pub severity: String,
    /// `[3]` year as a number (0 when missing, as `null > 0` reads); NaN when it is not a number.
    pub year: f64,
    /// `[4]` location text, "First / Second".
    pub location: String,
    /// `[5]` flags: p, c, m.
    pub flags: String,
    /// The whole tuple.
    pub tuple: Vec<Value>,
}

impl Crash {
    /// Reads a tuple (a saved crash record), as loosely as the prototype's code reads it.
    pub fn from_tuple(t: &[Value]) -> Crash {
        let num = |i: usize| t.get(i).map_or(f64::NAN, value_to_number);
        let text = |i: usize| match t.get(i) {
            Some(v) if crate::jsnum::value_truthy(v) => crate::jsnum::value_to_js_string(v),
            _ => String::new(),
        };
        Crash {
            lat: num(0),
            lng: num(1),
            severity: match t.get(2) {
                Some(Value::String(s)) => s.clone(),
                Some(v) => crate::jsnum::value_to_js_string(v),
                None => "undefined".to_string(),
            },
            year: num(3),
            location: text(4),
            flags: text(5),
            tuple: t.to_vec(),
        }
    }
}
