//! Small helpers of the calibration panel: its settings (`cfg`), the TMDG category, text and time readers, and the
//! geometry its areas and screenlines use.

use super::criteria::CATS;
use crate::jsnum::{Maybe, Num, f64_to_i64, f64_to_usize, js_trim, math_min, value_to_number, value_truthy};
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
    #[must_use]
    pub fn min_complete_n(&self) -> f64 {
        value_to_number(&self.min_complete)
    }
    /// `cfg().coreBuffer` as a number.
    #[must_use]
    pub fn core_buffer_n(&self) -> f64 {
        value_to_number(&self.core_buffer)
    }
    /// `cfg().xamax` as a number.
    #[must_use]
    pub fn xamax_n(&self) -> f64 {
        value_to_number(&self.xamax)
    }
    /// `cfg().priorQuality !== 'low'`.
    #[must_use]
    pub fn prior_high(&self) -> bool {
        self.prior_quality != Value::String("low".to_string())
    }
    /// `cfg().freezeGG !== false`.
    #[must_use]
    pub fn freeze_gg_on(&self) -> bool {
        self.freeze_gg != Value::Bool(false)
    }
    /// `!!cfg().useBalanced`.
    #[must_use]
    pub fn use_balanced_on(&self) -> bool {
        value_truthy(&self.use_balanced)
    }
    /// `!!cfg().noSinks`.
    #[must_use]
    pub fn no_sinks_on(&self) -> bool {
        value_truthy(&self.no_sinks)
    }
    /// The window of a period (`cfg().windows[period]`).
    #[must_use]
    pub fn window(&self, period: &str) -> Option<&[String; 2]> {
        self.windows.iter().find(|(p, _)| p == period).map(|(_, w)| w)
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
#[must_use]
pub fn category(s: Option<&Settings>) -> char {
    let raw = s
        .and_then(|s| s.tmdg.get()).map_or_else(|| "E".to_string(), |t| if t.category.truthy() { t.category.js_string() } else { "E".to_string() });
    let up = raw.to_uppercase();
    let mut cs = up.chars();
    match (cs.next(), cs.next()) {
        (Some(c), None) if CATS.contains(&c) => c,
        _ => 'E',
    }
}

/// `hash(s)`: FNV-1a over the UTF-16 units of the text (used to order counts deterministically).
#[must_use]
pub fn hash(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for u in s.encode_utf16() {
        h ^= u32::from(u);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// `isMw(L)`: a motorway or ramp.
#[must_use]
pub fn is_mw(l: Option<&Link>) -> bool {
    l.is_some_and(|l| l.cls.is("motorway") || l.cls.is("ramp"))
}

/// `dirOf(b)`: the compass direction code of a heading: "NB", "EB", "SB" or "WB".
#[must_use]
pub fn dir_of(b: f64) -> &'static str {
    const D: [&str; 4] = ["NB", "EB", "SB", "WB"];
    let k = crate::jsnum::math_round((((b % 360.0) + 360.0) % 360.0) / 90.0);
    // the index is below 4 (% 4), so the code is always there
    D.get(f64_to_usize(k % 4.0) % 4).copied().unwrap_or("NB")
}

/// `hhmm(m)`: minutes after midnight as "HH:MM".
#[must_use]
pub fn hhmm(m: f64) -> String {
    let h = ((m / 60.0).floor()) % 24.0;
    let mm = m % 60.0;
    format!("{:02}:{:02}", f64_to_i64(h), f64_to_i64(mm))
}

// The run of min to max ASCII digits starting at from: (from, end), or None when fewer than min. j never falls below
// from, so the differences are exact; Bound: positions are within the text, far below usize::MAX.
fn digits(s: &[char], from: usize, min: usize, max: usize) -> Option<(usize, usize)> {
    let mut j = from;
    while s.get(j).is_some_and(char::is_ascii_digit) && j.saturating_sub(from) < max {
        j = j.saturating_add(1);
    }
    if j.saturating_sub(from) >= min { Some((from, j)) } else { None }
}

/// `toMin(t)`: minutes after midnight from "HH:MM", "H.MM", "HHMM", "7:15 am" or a date-time such as "2026-03-10 07:15:00" (the last time in the text); None when no valid time can be read.
#[must_use]
pub fn to_min(t: &str) -> Option<f64> {
    let s: Vec<char> = js_trim(t).chars().collect();
    // /(\d{1,2})[:.](\d{2})(?::\d{2}(?:\.\d+)?)?\s*([ap]\.?m\.?)?/gi: the last match in the text
    let at = |i: usize| s.get(i).copied();
    let mut last: Option<(String, String, Option<String>)> = None;
    // Bound: positions are within the text, far below usize::MAX, so the saturating steps never saturate
    let mut pos = 0;
    while pos < s.len() {
        if let Some((h0, h1)) = digits(&s, pos, 1, 2)
            && matches!(at(h1), Some(':' | '.'))
            && let Some((m0, m1)) = digits(&s, h1.saturating_add(1), 2, 2)
        {
            let mut end = m1;
            // (?::\d{2}(?:\.\d+)?)?
            if at(end) == Some(':')
                && let Some((_, sec_end)) = digits(&s, end.saturating_add(1), 2, 2)
            {
                end = sec_end;
                if at(end) == Some('.') {
                    let frac0 = end.saturating_add(1);
                    let mut frac1 = frac0;
                    while at(frac1).is_some_and(|ch| ch.is_ascii_digit()) {
                        frac1 = frac1.saturating_add(1);
                    }
                    if frac1 > frac0 {
                        end = frac1;
                    }
                }
            }
            // \s*([ap]\.?m\.?)?
            let mut ap0 = end;
            while at(ap0).is_some_and(crate::jsnum::is_js_space) {
                ap0 = ap0.saturating_add(1);
            }
            let mut ap = None;
            if at(ap0).is_some_and(|ch| matches!(ch.to_ascii_lowercase(), 'a' | 'p')) {
                let mut ap1 = ap0.saturating_add(1);
                if at(ap1) == Some('.') {
                    ap1 = ap1.saturating_add(1);
                }
                if at(ap1).is_some_and(|ch| ch.eq_ignore_ascii_case(&'m')) {
                    ap1 = ap1.saturating_add(1);
                    if at(ap1) == Some('.') {
                        ap1 = ap1.saturating_add(1);
                    }
                    ap = Some(text(&s, ap0, ap1));
                    end = ap1;
                }
            }
            last = Some((text(&s, h0, h1), text(&s, m0, m1), ap));
            pos = end.max(pos.saturating_add(1));
            continue;
        }
        pos = pos.saturating_add(1);
    }
    let (mut hours, mins, ap) = match last {
        Some((hh, mm, ap)) => (hh.parse::<f64>().ok()?, mm.parse::<f64>().ok()?, ap),
        None => {
            // /^(\d{1,2})(\d{2})$/
            if s.len() >= 3 && s.len() <= 4 && s.iter().all(char::is_ascii_digit) {
                // s.len() is at least 3 here
                let split = s.len().saturating_sub(2);
                let hh = text(&s, 0, split);
                let mm = text(&s, split, s.len());
                (hh.parse::<f64>().ok()?, mm.parse::<f64>().ok()?, None)
            } else {
                return None;
            }
        }
    };
    if let Some(ap) = ap {
        let pm = ap.to_ascii_lowercase().starts_with('p');
        if hours < 1.0 || hours > 12.0 {
            return None;
        }
        hours = hours % 12.0 + if pm { 12.0 } else { 0.0 };
    }
    if hours < 24.0 && mins < 60.0 { Some(hours * 60.0 + mins) } else { None }
}

// The characters from..to of the text as a String (empty when the range is not inside it)
fn text(s: &[char], from: usize, to: usize) -> String {
    s.get(from..to).map(|cs| cs.iter().collect()).unwrap_or_default()
}

/// `isoDate(v)`: a count date as ISO "yyyy-mm-dd" from ISO, yyyy/mm/dd, or NZ dd/mm/yyyy (also d-m-yy and dd.mm.yyyy); Some("") for an empty cell; None when the text is not a real date.
#[must_use]
pub fn iso_date(v: &str) -> Option<String> {
    let s: Vec<char> = js_trim(v).chars().collect();
    if s.is_empty() {
        return Some(String::new());
    }
    let sep_at = |i: usize| s.get(i).is_some_and(|&ch| ch == '-' || ch == '/' || ch == '.');
    let digit_at = |i: usize| s.get(i).is_some_and(char::is_ascii_digit);
    let num = |from: usize, min: usize, max: usize| digits(&s, from, min, max);
    // Bound: positions are within the text, far below usize::MAX, so the saturating steps never saturate
    // /^(\d{4})[-/.](\d{1,2})[-/.](\d{1,2})(?!\d)/
    let try_ymd = || -> Option<(String, String, String)> {
        let (y0, y1) = num(0, 4, 4)?;
        if !sep_at(y1) {
            return None;
        }
        let (m0, m1) = num(y1.saturating_add(1), 1, 2)?;
        if !sep_at(m1) {
            return None;
        }
        // the month's digits are greedy but may give one back; the pattern needs a separator after them, so two digits or one
        let (d0, d1) = num(m1.saturating_add(1), 1, 2)?;
        if digit_at(d1) {
            return None;
        }
        Some((text(&s, y0, y1), text(&s, m0, m1), text(&s, d0, d1)))
    };
    // /^(\d{1,2})[-/.](\d{1,2})[-/.](\d{2}|\d{4})(?!\d)/
    let try_dmy = || -> Option<(String, String, String)> {
        let (d0, d1) = num(0, 1, 2)?;
        if !sep_at(d1) {
            return None;
        }
        let (m0, m1) = num(d1.saturating_add(1), 1, 2)?;
        if !sep_at(m1) {
            return None;
        }
        // up to four digits (a minimum of none always matches)
        let (start, stop) = num(m1.saturating_add(1), 0, 4)?;
        let len = stop.saturating_sub(start);
        // (\d{2}|\d{4}) then (?!\d): two digits not followed by a digit, or four
        let end = if len >= 4 {
            start.saturating_add(4)
        } else if len >= 2 {
            if digit_at(start.saturating_add(2)) {
                return None;
            }
            start.saturating_add(2)
        } else {
            return None;
        };
        if digit_at(end) {
            return None;
        }
        let year = text(&s, start, end);
        let year = if year.len() == 2 { format!("20{year}") } else { year };
        Some((year, text(&s, m0, m1), text(&s, d0, d1)))
    };
    let (year, month, day) = try_ymd().or_else(try_dmy)?;
    let (yn, mn, dn) = (year.parse::<i64>().ok()?, month.parse::<i64>().ok()?, day.parse::<i64>().ok()?);
    // new Date(Date.UTC(y, m - 1, d)) must read back as the same y, m, d (years 0 to 99 would map to 1900 to 1999 in Date.UTC, as the prototype's check does)
    // Bound: yn is 0 to 99 here, so the sum cannot overflow
    let yy = if (0..=99).contains(&yn) { yn.saturating_add(1900) } else { yn };
    let dim = |yr: i64, month_n: i64| -> i64 {
        match month_n {
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
    Some(format!("{year}-{month:0>2}-{day:0>2}"))
}

/// `pointInPoly(ll, poly)`: a [lat, lng] point inside a polygon (even-odd rule).
#[must_use]
pub fn point_in_poly(ll: &LatLng, poly: &[LatLng]) -> bool {
    let mut inside = false;
    // each vertex is paired with the one before it, the first with the last
    let Some(mut prev) = poly.last() else {
        return false;
    };
    for cur in poly {
        let (yi, xi) = (cur[0], cur[1]);
        let (yj, xj) = (prev[0], prev[1]);
        if (yi > ll[0]) != (yj > ll[0]) && ll[1] < (xj - xi) * (ll[0] - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        prev = cur;
    }
    inside
}

/// `distToLine(ll, pts)`: distance (m) from a point to a polyline of [lat, lng].
#[must_use]
pub fn dist_to_line(ll: &LatLng, pts: &[LatLng]) -> f64 {
    let f = flat(ll[0]);
    let (px, py) = (f.x(ll), f.y(ll));
    let mut d = f64::INFINITY;
    for pair in pts.windows(2) {
        // windows(2) gives two points each time
        let [p0, p1] = pair else { continue };
        d = math_min(d, seg_dist(px, py, f.x(p0), f.y(p0), f.x(p1), f.y(p1)).d);
    }
    d
}
