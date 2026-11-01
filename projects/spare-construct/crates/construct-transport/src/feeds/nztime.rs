//! New Zealand time and JavaScript date readings, with no time zone database.
//!
//! The prototype asks the browser for the date in `Pacific/Auckland` (`Intl.DateTimeFormat` with that time zone) and for
//! the local date (`getFullYear`, `toLocaleDateString` without a time zone), which for its users is New Zealand time too.
//! This module does the same from the daylight saving rules: until 2006 from the first Sunday in October to the third
//! Sunday in March, from 2007 from the last Sunday in September at 02:00 to the first Sunday in April at 03:00 (13 hours
//! ahead of UTC in summer, 12 in winter). The truth tests check the rule against Node's own time zone data.
//! Local time here always means New Zealand time.

use crate::edit::json::JsonValue;
use crate::jsnum::{number_to_js_string, string_to_number};

/// Milliseconds in a day.
pub const DAY_MS: f64 = 86_400_000.0;

/// Days since 1970-01-01 of a proleptic Gregorian date (month 1 to 12).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The (year, month 1 to 12, day 1 to 31) of a day count since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `Date.UTC(year, month0, day)`: month 0 to 11 and day 1 to 31, either may run over into the next period.
pub fn date_utc(year: f64, month0: f64, day: f64) -> f64 {
    let y = year + (month0 / 12.0).floor();
    let m = month0 - (month0 / 12.0).floor() * 12.0;
    days_from_civil(y as i64, m as u32 + 1, 1) as f64 * DAY_MS + (day - 1.0) * DAY_MS
}

/// Day of the week of a day count since 1970-01-01, 0 = Sunday.
pub fn weekday_of_days(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

// The Sunday on or after a date, as a day count
fn sunday_on_or_after(y: i64, m: u32, d: u32) -> i64 {
    let z = days_from_civil(y, m, d);
    z + (7 - weekday_of_days(z)) % 7
}

// The Sunday on or before a date, as a day count
fn sunday_on_or_before(y: i64, m: u32, d: u32) -> i64 {
    let z = days_from_civil(y, m, d);
    z - weekday_of_days(z)
}

// The UTC instant daylight saving starts in a year: a Sunday 02:00 standard time is 14:00 UTC the day before
fn dst_start_ms(y: i64) -> f64 {
    let sun = if y <= 2006 {
        sunday_on_or_after(y, 10, 1)
    } else {
        sunday_on_or_before(y, 9, 30)
    };
    sun as f64 * DAY_MS - 10.0 * 3_600_000.0
}

// The UTC instant daylight saving ends in a year: a Sunday 03:00 summer time is 14:00 UTC the day before
fn dst_end_ms(y: i64) -> f64 {
    let sun = if y <= 2007 {
        sunday_on_or_after(y, 3, 15)
    } else {
        sunday_on_or_after(y, 4, 1)
    };
    sun as f64 * DAY_MS - 10.0 * 3_600_000.0
}

/// Hours New Zealand is ahead of UTC at an instant (12, or 13 in daylight saving time).
pub fn nz_offset_hours(utc_ms: f64) -> f64 {
    let (y, _, _) = civil_from_days((utc_ms / DAY_MS).floor() as i64);
    if utc_ms < dst_end_ms(y) || utc_ms >= dst_start_ms(y) {
        13.0
    } else {
        12.0
    }
}

/// A New Zealand calendar date and clock time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NzParts {
    /// Year.
    pub year: i64,
    /// Month, 1 to 12.
    pub month: u32,
    /// Day of the month.
    pub day: u32,
    /// Hour, 0 to 23.
    pub hour: u32,
    /// Minute.
    pub minute: u32,
    /// Weekday, 0 = Sunday.
    pub weekday: i64,
}

/// The New Zealand date, time and weekday of an instant.
pub fn nz_parts(utc_ms: f64) -> NzParts {
    let local = utc_ms + nz_offset_hours(utc_ms) * 3_600_000.0;
    let days = (local / DAY_MS).floor();
    let rem = local - days * DAY_MS;
    let (y, m, d) = civil_from_days(days as i64);
    let hour = (rem / 3_600_000.0).floor();
    let minute = ((rem - hour * 3_600_000.0) / 60_000.0).floor();
    NzParts {
        year: y,
        month: m,
        day: d,
        hour: hour as u32,
        minute: minute as u32,
        weekday: weekday_of_days(days as i64),
    }
}

/// The UTC instant of midnight at the start of a New Zealand date (`new Date(y, m0, d)` in a New Zealand browser).
pub fn nz_midnight_ms(year: f64, month0: f64, day: f64) -> f64 {
    let local = date_utc(year, month0, day);
    // The offset in force at that local time: try summer time first, and take it when the instant agrees
    let guess13 = local - 13.0 * 3_600_000.0;
    if nz_offset_hours(guess13) == 13.0 {
        guess13
    } else {
        local - 12.0 * 3_600_000.0
    }
}

/// `today(now)` of feeds.js: the New Zealand date as YYYY-MM-DD.
pub fn today(utc_ms: f64) -> String {
    let p = nz_parts(utc_ms);
    format!("{:04}-{:02}-{:02}", p.year, p.month, p.day)
}

/// `nzYear(now)`: the New Zealand year.
pub fn nz_year(utc_ms: f64) -> i64 {
    nz_parts(utc_ms).year
}

/// What a JavaScript `Date` holds: whole milliseconds within 8.64e15 of 1970, else invalid (None).
pub fn time_clip(ms: f64) -> Option<f64> {
    if ms.is_finite() && ms.abs() <= 8.64e15 {
        Some(ms.trunc() + 0.0)
    } else {
        None
    }
}

fn digits(s: &[u8], from: usize, n: usize) -> Option<f64> {
    if from + n > s.len() || !s[from..from + n].iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut v = 0.0;
    for b in &s[from..from + n] {
        v = v * 10.0 + (b - b'0') as f64;
    }
    Some(v)
}

const MONTH_NAMES: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

// The date a server sends in a header, "Wed, 21 Oct 2026 07:28:00 GMT" (the weekday and the seconds are optional; the zone is GMT, UTC, Z or +hhmm, else New Zealand time)
fn parse_header_date(text: &str) -> Option<f64> {
    let t = text.trim();
    let t = match t.split_once(',') {
        Some((day, rest))
            if day.trim().len() >= 3 && day.trim().chars().all(|c| c.is_ascii_alphabetic()) =>
        {
            rest.trim()
        }
        _ => t,
    };
    let mut parts = t.split_whitespace();
    let day: f64 = parts.next()?.parse().ok()?;
    let mon = parts.next()?.to_ascii_lowercase();
    let month = MONTH_NAMES.iter().position(|m| mon.starts_with(m))? as f64;
    let year: f64 = parts.next()?.parse().ok()?;
    let clock = parts.next().unwrap_or("00:00:00");
    let mut c = clock.split(':');
    let h: f64 = c.next()?.parse().ok()?;
    let mi: f64 = c.next().unwrap_or("0").parse().ok()?;
    let sec: f64 = c.next().unwrap_or("0").parse().ok()?;
    let zone = parts.next();
    if parts.next().is_some() || !(1.0..=31.0).contains(&day) || h > 24.0 || mi > 59.0 || sec > 59.0
    {
        return None;
    }
    let local = date_utc(year, month, day) + h * 3_600_000.0 + mi * 60_000.0 + sec * 1000.0;
    let ms = match zone.map(str::to_ascii_uppercase).as_deref() {
        Some("GMT") | Some("UTC") | Some("Z") | Some("UT") => local,
        Some(z) if (z.starts_with('+') || z.starts_with('-')) && z.len() == 5 => {
            let oh: f64 = z[1..3].parse().ok()?;
            let om: f64 = z[3..5].parse().ok()?;
            let off = (oh * 60.0 + om) * 60_000.0;
            local - if z.starts_with('+') { off } else { -off }
        }
        None => local - nz_offset_for_local(local) * 3_600_000.0,
        _ => return None,
    };
    time_clip(ms)
}

/// `Date.parse(text)` for the date and date-time forms of ECMAScript (YYYY, YYYY-MM, YYYY-MM-DD, with T or a space and
/// hours, minutes, optional seconds and milliseconds, and Z or +hh:mm; a time with no offset is New Zealand time) and for the
/// date of an HTTP header ("Wed, 21 Oct 2026 07:28:00 GMT"). Other spellings the browser accepts (a padded text, month names in
/// other orders, and so on) are not read (None).
pub fn parse_date(text: &str) -> Option<f64> {
    let s = text.as_bytes();
    let Some(year) = digits(s, 0, 4) else {
        return parse_header_date(text);
    };
    let mut i = 4;
    let (mut month, mut day) = (1.0, 1.0);
    if i < s.len() && s[i] == b'-' {
        month = digits(s, i + 1, 2)?;
        i += 3;
        if i < s.len() && s[i] == b'-' {
            day = digits(s, i + 1, 2)?;
            i += 3;
        }
    }
    if !(1.0..=12.0).contains(&month) || !(1.0..=31.0).contains(&day) {
        return None;
    }
    let date_ms = date_utc(year, month - 1.0, day);
    if i == s.len() {
        return time_clip(date_ms);
    }
    if s[i] != b'T' && s[i] != b' ' {
        return None;
    }
    let h = digits(s, i + 1, 2)?;
    if s.get(i + 3) != Some(&b':') {
        return None;
    }
    let mi = digits(s, i + 4, 2)?;
    i += 6;
    let (mut sec, mut milli) = (0.0, 0.0);
    if i < s.len() && s[i] == b':' {
        sec = digits(s, i + 1, 2)?;
        i += 3;
        if i < s.len() && s[i] == b'.' {
            let mut j = i + 1;
            let mut frac = 0.0;
            let mut scale = 100.0;
            while j < s.len() && s[j].is_ascii_digit() {
                frac += (s[j] - b'0') as f64 * scale;
                scale /= 10.0;
                j += 1;
            }
            if j == i + 1 {
                return None;
            }
            milli = frac.floor();
            i = j;
        }
    }
    if h > 24.0 || mi > 59.0 || sec > 59.0 || (h == 24.0 && (mi > 0.0 || sec > 0.0 || milli > 0.0))
    {
        return None;
    }
    let clock = h * 3_600_000.0 + mi * 60_000.0 + sec * 1000.0 + milli;
    let ms = if i == s.len() {
        // No offset: local (New Zealand) time
        let local = date_ms + clock;
        local - nz_offset_for_local(local) * 3_600_000.0
    } else if s[i] == b'Z' && i + 1 == s.len() {
        date_ms + clock
    } else if (s[i] == b'+' || s[i] == b'-') && i + 6 == s.len() && s[i + 3] == b':' {
        let oh = digits(s, i + 1, 2)?;
        let om = digits(s, i + 4, 2)?;
        let off = (oh * 60.0 + om) * 60_000.0;
        date_ms + clock - if s[i] == b'+' { off } else { -off }
    } else {
        return None;
    };
    time_clip(ms)
}

// The hours ahead of UTC that apply to a New Zealand wall-clock time given as if it were UTC
fn nz_offset_for_local(local_as_utc: f64) -> f64 {
    if nz_offset_hours(local_as_utc - 13.0 * 3_600_000.0) == 13.0 {
        13.0
    } else {
        12.0
    }
}

/// `new Date(v).getTime()` for a value read from JSON: a number is milliseconds, text is parsed, true is 1 and null is 0;
/// None for an invalid date (`NaN`).
pub fn date_of(v: &JsonValue) -> Option<f64> {
    match v {
        JsonValue::Number(n) => time_clip(*n),
        JsonValue::String(s) => parse_date(s),
        JsonValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        JsonValue::Null => Some(0.0),
        _ => None,
    }
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sept", "Oct", "Nov", "Dec",
];

/// The short month name `toLocaleDateString('en-NZ', { month: 'short' })` gives (September is "Sept").
pub fn month_short(month: u32) -> &'static str {
    MONTHS[(month as usize).saturating_sub(1).min(11)]
}

/// `toLocaleDateString('en-NZ', { day: 'numeric', month: 'short', year: 'numeric' })` in New Zealand: "9 Oct 2026".
pub fn day_month_year(ms: f64) -> String {
    let p = nz_parts(ms);
    format!("{} {} {}", p.day, month_short(p.month), p.year)
}

/// `toLocaleDateString('en-NZ', { month: 'short', year: 'numeric' })` in New Zealand: "Mar 2025".
pub fn month_year(ms: f64) -> String {
    let p = nz_parts(ms);
    format!("{} {}", month_short(p.month), p.year)
}

/// `dateText(iso)` of feeds.js: "9 Oct 2026"; nothing (or an empty text) gives "", an unreadable date the text as stored (first 10 characters).
pub fn date_text(v: Option<&JsonValue>) -> String {
    let Some(v) = v else { return String::new() };
    if matches!(v, JsonValue::Null) {
        return String::new();
    }
    if let JsonValue::String(s) = v
        && s.is_empty()
    {
        return String::new();
    }
    match date_of(v) {
        Some(ms) => day_month_year(ms),
        None => v.js_string().chars().take(10).collect(),
    }
}

/// `new Date(ms).toISOString().slice(0, 10)`: the UTC date as YYYY-MM-DD.
pub fn utc_date(ms: f64) -> String {
    let (y, m, d) = civil_from_days((ms / DAY_MS).floor() as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A number as JavaScript prints it (`String(n)`).
pub fn num_text(x: f64) -> String {
    number_to_js_string(x)
}

/// `+text`: the number a text reads as (NaN when it is not one).
pub fn to_num(text: &str) -> f64 {
    string_to_number(text)
}
