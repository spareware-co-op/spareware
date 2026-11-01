//! Small helpers of the calibration panel: its settings (`cfg`), the TMDG category, text and time readers, and the
//! geometry its areas and screenlines use.

use super::criteria::CATS;
use crate::jsnum::{Maybe, Num, js_trim, math_min, value_to_number, value_truthy};
use crate::project::{Link, Settings};
use crate::util::{LatLng, flat, seg_dist};
use serde_json::Value;

/// The calibration settings the panel reads (`cfg()`): `settings.calib` over the panel's defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct CalibCfg {
    /// Criteria set: "tmdg", "rms" or "atap" (`standard`).
    pub standard: Value,
    /// Counts the statistics use: "valid", "calib" or "all" (`set`).
    pub set: Value,
    /// Area: "auto", "inside", "outside" or "all" (`area`).
    pub area: Value,
    /// Share of a screenline's modelled flow that must be counted (`minComplete`).
    pub min_complete: Value,
    /// "high" or "low" quality prior (`priorQuality`).
    pub prior_quality: Value,
    /// Largest site ratio and cumulative factor of matrix estimation (`xamax`).
    pub xamax: Value,
    /// Matrix estimation keeps through traffic as it is (`freezeGG`).
    pub freeze_gg: Value,
    /// Balancing uses the larger flow (`noSinks`).
    pub no_sinks: Value,
    /// Balanced turning counts are used (`useBalanced`).
    pub use_balanced: Value,
    /// Buffer (m) round the roads that differ from the base scenario (`coreBuffer`).
    pub core_buffer: Value,
    /// Peak-hour windows by period (`windows`: period to [start, end]).
    pub windows: Vec<(String, [String; 2])>,
}

impl CalibCfg {
    /// `cfg().minComplete` as a number.
    pub fn min_complete_n(&self) -> f64 {
        value_to_number(&self.min_complete)
    }
    /// `cfg().coreBuffer` as a number.
    pub fn core_buffer_n(&self) -> f64 {
        value_to_number(&self.core_buffer)
    }
    /// `cfg().xamax` as a number.
    pub fn xamax_n(&self) -> f64 {
        value_to_number(&self.xamax)
    }
    /// `cfg().priorQuality !== 'low'`.
    pub fn prior_high(&self) -> bool {
        self.prior_quality != Value::String("low".to_string())
    }
    /// `cfg().freezeGG !== false`.
    pub fn freeze_gg_on(&self) -> bool {
        self.freeze_gg != Value::Bool(false)
    }
    /// `!!cfg().useBalanced`.
    pub fn use_balanced_on(&self) -> bool {
        value_truthy(&self.use_balanced)
    }
    /// `!!cfg().noSinks`.
    pub fn no_sinks_on(&self) -> bool {
        value_truthy(&self.no_sinks)
    }
    /// The window of a period (`cfg().windows[period]`).
    pub fn window(&self, period: &str) -> Option<&[String; 2]> {
        self.windows
            .iter()
            .find(|(p, _)| p == period)
            .map(|(_, w)| w)
    }
}

/// `cfg(S)`: the calibration settings over the panel's defaults (key by key; the windows by period).
pub fn calib_cfg(s: Option<&Settings>) -> CalibCfg {
    let mut standard = Value::String("tmdg".into());
    let mut set = Value::String("valid".into());
    let mut area = Value::String("auto".into());
    let mut min_complete = Value::from(1);
    let mut prior_quality = Value::String("high".into());
    let mut xamax = Value::from(2);
    let mut freeze_gg = Value::Bool(true);
    let mut no_sinks = Value::Bool(false);
    let mut use_balanced = Value::Bool(true);
    let mut core_buffer = Value::from(500);
    let mut windows: Vec<(String, [String; 2])> = vec![
        ("AM".into(), ["07:00".into(), "09:00".into()]),
        ("IP".into(), ["09:00".into(), "15:00".into()]),
        ("PM".into(), ["16:00".into(), "18:00".into()]),
    ];
    if let Some(c) = s.and_then(|s| s.calib.get()) {
        if let Maybe::Set(_) | Maybe::Null = c.standard {
            standard = num_value(&c.standard);
        }
        let put = |key: &str, slot: &mut Value| {
            if let Some(v) = c.extra.get(key) {
                *slot = v.clone();
            }
        };
        put("set", &mut set);
        put("area", &mut area);
        put("minComplete", &mut min_complete);
        put("priorQuality", &mut prior_quality);
        put("xamax", &mut xamax);
        put("freezeGG", &mut freeze_gg);
        put("noSinks", &mut no_sinks);
        put("useBalanced", &mut use_balanced);
        put("coreBuffer", &mut core_buffer);
        if let Some(Value::Object(w)) = c.extra.get("windows") {
            for (k, v) in w {
                let pair = match v {
                    Value::Array(a) => [
                        a.first().map(js_text).unwrap_or_default(),
                        a.get(1).map(js_text).unwrap_or_default(),
                    ],
                    _ => [String::new(), String::new()],
                };
                match windows.iter_mut().find(|(p, _)| p == k) {
                    Some(slot) => slot.1 = pair,
                    None => windows.push((k.clone(), pair)),
                }
            }
        }
    }
    CalibCfg {
        standard,
        set,
        area,
        min_complete,
        prior_quality,
        xamax,
        freeze_gg,
        no_sinks,
        use_balanced,
        core_buffer,
        windows,
    }
}

fn js_text(v: &Value) -> String {
    crate::jsnum::value_to_js_string(v)
}

fn num_value(n: &Num) -> Value {
    match n {
        Maybe::Set(j) => j.to_value(),
        _ => Value::Null,
    }
}

/// `category(S)`: the TMDG purpose category the project is assessed against (settings.tmdg.category, upper-cased; E when missing or not A to G).
pub fn category(s: Option<&Settings>) -> char {
    let raw = s
        .and_then(|s| s.tmdg.get())
        .map(|t| {
            if t.category.truthy() {
                t.category.js_string()
            } else {
                "E".to_string()
            }
        })
        .unwrap_or_else(|| "E".to_string());
    let up = raw.to_uppercase();
    let mut cs = up.chars();
    match (cs.next(), cs.next()) {
        (Some(c), None) if CATS.contains(&c) => c,
        _ => 'E',
    }
}

/// `hash(s)`: FNV-1a over the UTF-16 units of the text (used to order counts deterministically).
pub fn hash(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for u in s.encode_utf16() {
        h ^= u as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// `isMw(L)`: a motorway or ramp.
pub fn is_mw(l: Option<&Link>) -> bool {
    l.is_some_and(|l| l.cls.is("motorway") || l.cls.is("ramp"))
}

/// `dirOf(b)`: the compass direction code of a heading: "NB", "EB", "SB" or "WB".
pub fn dir_of(b: f64) -> &'static str {
    const D: [&str; 4] = ["NB", "EB", "SB", "WB"];
    let k = crate::jsnum::math_round((((b % 360.0) + 360.0) % 360.0) / 90.0);
    D[((k % 4.0) as usize) % 4]
}

/// `hhmm(m)`: minutes after midnight as "HH:MM".
pub fn hhmm(m: f64) -> String {
    let h = ((m / 60.0).floor()) % 24.0;
    let mm = m % 60.0;
    format!("{:02}:{:02}", h as i64, mm as i64)
}

fn digits(s: &[char], from: usize, min: usize, max: usize) -> Option<(usize, usize)> {
    let mut j = from;
    while j < s.len() && s[j].is_ascii_digit() && j - from < max {
        j += 1;
    }
    if j - from >= min {
        Some((from, j))
    } else {
        None
    }
}

/// `toMin(t)`: minutes after midnight from "HH:MM", "H.MM", "HHMM", "7:15 am" or a date-time such as "2026-03-10 07:15:00" (the last time in the text); None when no valid time can be read.
pub fn to_min(t: &str) -> Option<f64> {
    let s: Vec<char> = js_trim(t).chars().collect();
    // /(\d{1,2})[:.](\d{2})(?::\d{2}(?:\.\d+)?)?\s*([ap]\.?m\.?)?/gi: the last match in the text
    let mut last: Option<(String, String, Option<String>)> = None;
    let mut i = 0;
    while i < s.len() {
        if let Some((a, b)) = digits(&s, i, 1, 2)
            && b < s.len()
            && (s[b] == ':' || s[b] == '.')
            && let Some((c, d)) = digits(&s, b + 1, 2, 2)
        {
            let mut j = d;
            // (?::\d{2}(?:\.\d+)?)?
            if j < s.len()
                && s[j] == ':'
                && let Some((_, e)) = digits(&s, j + 1, 2, 2)
            {
                j = e;
                if j < s.len() && s[j] == '.' {
                    let mut k = j + 1;
                    while k < s.len() && s[k].is_ascii_digit() {
                        k += 1;
                    }
                    if k > j + 1 {
                        j = k;
                    }
                }
            }
            // \s*([ap]\.?m\.?)?
            let mut k = j;
            while k < s.len() && crate::jsnum::is_js_space(s[k]) {
                k += 1;
            }
            let mut ap = None;
            if k < s.len() && matches!(s[k].to_ascii_lowercase(), 'a' | 'p') {
                let mut q = k + 1;
                if q < s.len() && s[q] == '.' {
                    q += 1;
                }
                if q < s.len() && s[q].to_ascii_lowercase() == 'm' {
                    q += 1;
                    if q < s.len() && s[q] == '.' {
                        q += 1;
                    }
                    ap = Some(s[k..q].iter().collect::<String>());
                    j = q;
                }
            }
            last = Some((s[a..b].iter().collect(), s[c..d].iter().collect(), ap));
            i = j.max(i + 1);
            continue;
        }
        i += 1;
    }
    let (mut h, m, ap) = match last {
        Some((h, m, ap)) => (h.parse::<f64>().ok()?, m.parse::<f64>().ok()?, ap),
        None => {
            // /^(\d{1,2})(\d{2})$/
            if s.len() >= 3 && s.len() <= 4 && s.iter().all(|c| c.is_ascii_digit()) {
                let split = s.len() - 2;
                let h: String = s[..split].iter().collect();
                let m: String = s[split..].iter().collect();
                (h.parse::<f64>().ok()?, m.parse::<f64>().ok()?, None)
            } else {
                return None;
            }
        }
    };
    if let Some(ap) = ap {
        let pm = ap.to_ascii_lowercase().starts_with('p');
        if h < 1.0 || h > 12.0 {
            return None;
        }
        h = h % 12.0 + if pm { 12.0 } else { 0.0 };
    }
    if h < 24.0 && m < 60.0 {
        Some(h * 60.0 + m)
    } else {
        None
    }
}

/// `isoDate(v)`: a count date as ISO "yyyy-mm-dd" from ISO, yyyy/mm/dd, or NZ dd/mm/yyyy (also d-m-yy and dd.mm.yyyy); Some("") for an empty cell; None when the text is not a real date.
pub fn iso_date(v: &str) -> Option<String> {
    let s: Vec<char> = js_trim(v).chars().collect();
    if s.is_empty() {
        return Some(String::new());
    }
    let sep = |c: char| c == '-' || c == '/' || c == '.';
    let num = |from: usize, min: usize, max: usize| digits(&s, from, min, max);
    let (y, mo, d);
    // /^(\d{4})[-/.](\d{1,2})[-/.](\d{1,2})(?!\d)/
    let try_ymd = || -> Option<(String, String, String)> {
        let (a, b) = num(0, 4, 4)?;
        if !(b < s.len() && sep(s[b])) {
            return None;
        }
        let (c, d1) = num(b + 1, 1, 2)?;
        if !(d1 < s.len() && sep(s[d1])) {
            return None;
        }
        // the month's digits are greedy but may give one back; the pattern needs a separator after them, so two digits or one
        let (e, f) = num(d1 + 1, 1, 2)?;
        if f < s.len() && s[f].is_ascii_digit() {
            return None;
        }
        Some((
            s[a..b].iter().collect(),
            s[c..d1].iter().collect(),
            s[e..f].iter().collect(),
        ))
    };
    // /^(\d{1,2})[-/.](\d{1,2})[-/.](\d{2}|\d{4})(?!\d)/
    let try_dmy = || -> Option<(String, String, String)> {
        let (a, b) = num(0, 1, 2)?;
        if !(b < s.len() && sep(s[b])) {
            return None;
        }
        let (c, d1) = num(b + 1, 1, 2)?;
        if !(d1 < s.len() && sep(s[d1])) {
            return None;
        }
        let (e, f4) = {
            let mut j = d1 + 1;
            while j < s.len() && s[j].is_ascii_digit() && j - (d1 + 1) < 4 {
                j += 1;
            }
            (d1 + 1, j)
        };
        let len = f4 - e;
        // (\d{2}|\d{4}) then (?!\d): two digits not followed by a digit, or four
        let (end, yy) = if len >= 4 {
            (e + 4, s[e..e + 4].iter().collect::<String>())
        } else if len >= 2 {
            if e + 2 < s.len() && s[e + 2].is_ascii_digit() {
                return None;
            }
            (e + 2, s[e..e + 2].iter().collect::<String>())
        } else {
            return None;
        };
        if end < s.len() && s[end].is_ascii_digit() {
            return None;
        }
        let yy = if yy.len() == 2 { format!("20{yy}") } else { yy };
        Some((
            s[a..b].iter().collect::<String>(),
            s[c..d1].iter().collect::<String>(),
            yy,
        ))
        .map(|(dd, mm, y)| (y, mm, dd))
    };
    if let Some((yy, mm, dd)) = try_ymd() {
        y = yy;
        mo = mm;
        d = dd;
    } else if let Some((yy, mm, dd)) = try_dmy() {
        y = yy;
        mo = mm;
        d = dd;
    } else {
        return None;
    }
    let (yn, mn, dn) = (
        y.parse::<i64>().ok()?,
        mo.parse::<i64>().ok()?,
        d.parse::<i64>().ok()?,
    );
    // new Date(Date.UTC(y, m - 1, d)) must read back as the same y, m, d (years 0 to 99 would map to 1900 to 1999 in Date.UTC, as the prototype's check does)
    let yy = if (0..=99).contains(&yn) {
        1900 + yn
    } else {
        yn
    };
    let dim = |yr: i64, m: i64| -> i64 {
        match m {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if (yr % 4 == 0 && yr % 100 != 0) || yr % 400 == 0 {
                    29
                } else {
                    28
                }
            }
            _ => 0,
        }
    };
    let ok = (1..=12).contains(&mn) && dn >= 1 && dn <= dim(yy, mn) && yn == yy;
    if !ok {
        return None;
    }
    Some(format!("{}-{:0>2}-{:0>2}", y, mo, d))
}

/// `pointInPoly(ll, poly)`: a [lat, lng] point inside a polygon (even-odd rule).
pub fn point_in_poly(ll: &LatLng, poly: &[LatLng]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n == 0 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (yi, xi) = (poly[i][0], poly[i][1]);
        let (yj, xj) = (poly[j][0], poly[j][1]);
        if (yi > ll[0]) != (yj > ll[0]) && ll[1] < (xj - xi) * (ll[0] - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// `distToLine(ll, pts)`: distance (m) from a point to a polyline of [lat, lng].
pub fn dist_to_line(ll: &LatLng, pts: &[LatLng]) -> f64 {
    let f = flat(ll[0]);
    let (px, py) = (f.x(ll), f.y(ll));
    let mut d = f64::INFINITY;
    for i in 0..pts.len().saturating_sub(1) {
        d = math_min(
            d,
            seg_dist(
                px,
                py,
                f.x(&pts[i]),
                f.y(&pts[i]),
                f.x(&pts[i + 1]),
                f.y(&pts[i + 1]),
            )
            .d,
        );
    }
    d
}
