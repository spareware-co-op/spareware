//! New Zealand time and JavaScript date readings, with no time zone database.
//!
//! The prototype asks the browser for the date in `Pacific/Auckland` (`Intl.DateTimeFormat` with that time zone) and for
//! the local date (`getFullYear`, `toLocaleDateString` without a time zone), which for its users is New Zealand time too.
//! This module does the same from the daylight saving rules: until 2006 from the first Sunday in October to the third
//! Sunday in March, from 2007 from the last Sunday in September at 02:00 to the first Sunday in April at 03:00 (13 hours
//! ahead of UTC in summer, 12 in winter). The truth tests check the rule against Node's own time zone data.
//! Local time here always means New Zealand time.

use crate::jsnum::{f64_to_i64, f64_to_usize, i64_to_f64, number_to_js_string, string_to_number, usize_to_f64};
use crate::edit::json::JsonValue;

/// Milliseconds in a day.
pub const DAY_MS: f64 = 86_400_000.0;

// `x as u32`: truncated toward zero, NaN and anything below 0 giving 0, anything above u32::MAX giving u32::MAX
fn f64_to_u32(x: f64) -> u32 {
    u32::try_from(f64_to_usize(x)).unwrap_or(u32::MAX)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (month 1 to 12).
#[must_use]
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    // Bound: years of real dates are far inside i64 (a JavaScript date is within 275,760 years of 1970), and yoe, mp and
    // doy are small; the saturating steps only keep an absurd year from overflowing
    let y = if m <= 2 { y.saturating_sub(1) } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from(m).saturating_add(9) % 12;
    let doy = (mp.saturating_mul(153).saturating_add(2) / 5).saturating_add(i64::from(d)).saturating_sub(1);
    let doe = yoe.saturating_mul(365).saturating_add(yoe / 4).saturating_sub(yoe / 100).saturating_add(doy);
    era.saturating_mul(146_097).saturating_add(doe).saturating_sub(719_468)
}

/// The (year, month 1 to 12, day 1 to 31) of a day count since 1970-01-01.
#[must_use]
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    // Bound: doe is in 0..146_097, so yoe, doy and mp are small; era only saturates for an absurd day count
    let z = z.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = doe.saturating_sub(doe / 1460).saturating_add(doe / 36_524).saturating_sub(doe / 146_096) / 365;
    let y = yoe.saturating_add(era.saturating_mul(400));
    let doy = doe.saturating_sub(yoe.saturating_mul(365).saturating_add(yoe / 4).saturating_sub(yoe / 100));
    let mp = doy.saturating_mul(5).saturating_add(2) / 153;
    // d is in 1..=31 and m in 1..=12, so the conversions never fall back
    let d = u32::try_from(doy.saturating_sub(mp.saturating_mul(153).saturating_add(2) / 5).saturating_add(1)).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp.saturating_add(3) } else { mp.saturating_sub(9) }).unwrap_or(1);
    (if m <= 2 { y.saturating_add(1) } else { y }, m, d)
}

/// `Date.UTC(year, month0, day)`: month 0 to 11 and day 1 to 31, either may run over into the next period.
#[must_use]
pub fn date_utc(year: f64, month0: f64, day: f64) -> f64 {
    let y = year + (month0 / 12.0).floor();
    let m = month0 - (month0 / 12.0).floor() * 12.0;
    i64_to_f64(days_from_civil(f64_to_i64(y), f64_to_u32(m).saturating_add(1), 1)) * DAY_MS + (day - 1.0) * DAY_MS
}

/// Day of the week of a day count since 1970-01-01, 0 = Sunday.
#[must_use]
pub fn weekday_of_days(days: i64) -> i64 {
    days.saturating_add(4).rem_euclid(7)
}

// The Sunday on or after a date, as a day count
fn sunday_on_or_after(y: i64, m: u32, d: u32) -> i64 {
    let z = days_from_civil(y, m, d);
    z.saturating_add(7_i64.saturating_sub(weekday_of_days(z)) % 7)
}

// The Sunday on or before a date, as a day count
fn sunday_on_or_before(y: i64, m: u32, d: u32) -> i64 {
    let z = days_from_civil(y, m, d);
    z.saturating_sub(weekday_of_days(z))
}

// The UTC instant daylight saving starts in a year: a Sunday 02:00 standard time is 14:00 UTC the day before
fn dst_start_ms(y: i64) -> f64 {
    let sun = if y <= 2006 { sunday_on_or_after(y, 10, 1) } else { sunday_on_or_before(y, 9, 30) };
    i64_to_f64(sun) * DAY_MS - 10.0 * 3_600_000.0
}

// The UTC instant daylight saving ends in a year: a Sunday 03:00 summer time is 14:00 UTC the day before
fn dst_end_ms(y: i64) -> f64 {
    let sun = if y <= 2007 { sunday_on_or_after(y, 3, 15) } else { sunday_on_or_after(y, 4, 1) };
    i64_to_f64(sun) * DAY_MS - 10.0 * 3_600_000.0
}

/// Hours New Zealand is ahead of UTC at an instant (12, or 13 in daylight saving time).
#[must_use]
pub fn nz_offset_hours(utc_ms: f64) -> f64 {
    let (y, _, _) = civil_from_days(f64_to_i64((utc_ms / DAY_MS).floor()));
    if utc_ms < dst_end_ms(y) || utc_ms >= dst_start_ms(y) { 13.0 } else { 12.0 }
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
#[must_use]
pub fn nz_parts(utc_ms: f64) -> NzParts {
    let local = utc_ms + nz_offset_hours(utc_ms) * 3_600_000.0;
    let days = (local / DAY_MS).floor();
    let rem = local - days * DAY_MS;
    let (y, m, d) = civil_from_days(f64_to_i64(days));
    let hour = (rem / 3_600_000.0).floor();
    let minute = ((rem - hour * 3_600_000.0) / 60_000.0).floor();
    NzParts { year: y, month: m, day: d, hour: f64_to_u32(hour), minute: f64_to_u32(minute), weekday: weekday_of_days(f64_to_i64(days)) }
}

/// The UTC instant of midnight at the start of a New Zealand date (`new Date(y, m0, d)` in a New Zealand browser).
#[must_use]
pub fn nz_midnight_ms(year: f64, month0: f64, day: f64) -> f64 {
    let local = date_utc(year, month0, day);
    // The offset in force at that local time: try summer time first, and take it when the instant agrees
    let guess13 = local - 13.0 * 3_600_000.0;
    if nz_offset_hours(guess13) == 13.0 { guess13 } else { local - 12.0 * 3_600_000.0 }
}

/// `today(now)` of feeds.js: the New Zealand date as YYYY-MM-DD.
#[must_use]
pub fn today(utc_ms: f64) -> String {
    let p = nz_parts(utc_ms);
    format!("{:04}-{:02}-{:02}", p.year, p.month, p.day)
}

/// `nzYear(now)`: the New Zealand year.
#[must_use]
pub fn nz_year(utc_ms: f64) -> i64 {
    nz_parts(utc_ms).year
}

/// What a JavaScript `Date` holds: whole milliseconds within 8.64e15 of 1970, else invalid (None).
#[must_use]
pub fn time_clip(ms: f64) -> Option<f64> {
    if ms.is_finite() && ms.abs() <= 8.64e15 { Some(ms.trunc() + 0.0) } else { None }
}

fn digits(s: &[u8], from: usize, n: usize) -> Option<f64> {
    let part = s.get(from..from.checked_add(n)?)?;
    if !part.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut v = 0.0;
    for b in part {
        // b is an ASCII digit, so this never saturates
        v = v * 10.0 + f64::from(b.saturating_sub(b'0'));
    }
    Some(v)
}

const MONTH_NAMES: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

// The date a server sends in a header, "Wed, 21 Oct 2026 07:28:00 GMT" (the weekday and the seconds are optional; the zone is GMT, UTC, Z or +hhmm, else New Zealand time)
fn parse_header_date(text: &str) -> Option<f64> {
    let t = text.trim();
    let t = match t.split_once(',') {
        Some((day, rest)) if day.trim().len() >= 3 && day.trim().chars().all(|c| c.is_ascii_alphabetic()) => rest.trim(),
        _ => t,
    };
    let mut parts = t.split_whitespace();
    let day: f64 = parts.next()?.parse().ok()?;
    let mon = parts.next()?.to_ascii_lowercase();
    let month = usize_to_f64(MONTH_NAMES.iter().position(|m| mon.starts_with(m))?);
    let year: f64 = parts.next()?.parse().ok()?;
    let clock = parts.next().unwrap_or("00:00:00");
    let mut c = clock.split(':');
    let h: f64 = c.next()?.parse().ok()?;
    let mi: f64 = c.next().unwrap_or("0").parse().ok()?;
    let sec: f64 = c.next().unwrap_or("0").parse().ok()?;
    let zone = parts.next();
    if parts.next().is_some() || !(1.0..=31.0).contains(&day) || h > 24.0 || mi > 59.0 || sec > 59.0 {
        return None;
    }
    let local = date_utc(year, month, day) + h * 3_600_000.0 + mi * 60_000.0 + sec * 1000.0;
    let ms = match zone.map(str::to_ascii_uppercase).as_deref() {
        Some("GMT" | "UTC" | "Z" | "UT") => local,
        Some(z) if (z.starts_with('+') || z.starts_with('-')) && z.len() == 5 => {
            let oh: f64 = z.get(1..3)?.parse().ok()?;
            let om: f64 = z.get(3..5)?.parse().ok()?;
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
#[must_use]
pub fn parse_date(text: &str) -> Option<f64> {
    let s = text.as_bytes();
    let Some(year) = digits(s, 0, 4) else { return parse_header_date(text) };
    // s[k] in JavaScript terms: None past the end
    let at = |k: usize| s.get(k).copied();
    // Bound: i only moves past fields that were read, so it stays at most s.len() and the sums below never saturate
    let mut i: usize = 4;
    let (mut month, mut day) = (1.0, 1.0);
    if at(i) == Some(b'-') {
        month = digits(s, i.saturating_add(1), 2)?;
        i = i.saturating_add(3);
        if at(i) == Some(b'-') {
            day = digits(s, i.saturating_add(1), 2)?;
            i = i.saturating_add(3);
        }
    }
    if !(1.0..=12.0).contains(&month) || !(1.0..=31.0).contains(&day) {
        return None;
    }
    let date_ms = date_utc(year, month - 1.0, day);
    if i == s.len() {
        return time_clip(date_ms);
    }
    if !matches!(at(i), Some(b'T' | b' ')) {
        return None;
    }
    let h = digits(s, i.saturating_add(1), 2)?;
    if at(i.saturating_add(3)) != Some(b':') {
        return None;
    }
    let mi = digits(s, i.saturating_add(4), 2)?;
    i = i.saturating_add(6);
    let (mut sec, mut milli) = (0.0, 0.0);
    if at(i) == Some(b':') {
        sec = digits(s, i.saturating_add(1), 2)?;
        i = i.saturating_add(3);
        if at(i) == Some(b'.') {
            let start = i.saturating_add(1);
            let mut j = start;
            let mut frac = 0.0;
            let mut scale = 100.0;
            while let Some(c) = at(j)
                && c.is_ascii_digit()
            {
                frac += f64::from(c.saturating_sub(b'0')) * scale;
                scale /= 10.0;
                j = j.saturating_add(1);
            }
            if j == start {
                return None;
            }
            milli = frac.floor();
            i = j;
        }
    }
    if h > 24.0 || mi > 59.0 || sec > 59.0 || (h == 24.0 && (mi > 0.0 || sec > 0.0 || milli > 0.0)) {
        return None;
    }
    let clock = h * 3_600_000.0 + mi * 60_000.0 + sec * 1000.0 + milli;
    let ms = if i == s.len() {
        // No offset: local (New Zealand) time
        let local = date_ms + clock;
        local - nz_offset_for_local(local) * 3_600_000.0
    } else if at(i) == Some(b'Z') && i.saturating_add(1) == s.len() {
        date_ms + clock
    } else if matches!(at(i), Some(b'+' | b'-')) && i.saturating_add(6) == s.len() && at(i.saturating_add(3)) == Some(b':') {
        let oh = digits(s, i.saturating_add(1), 2)?;
        let om = digits(s, i.saturating_add(4), 2)?;
        let off = (oh * 60.0 + om) * 60_000.0;
        date_ms + clock - if at(i) == Some(b'+') { off } else { -off }
    } else {
        return None;
    };
    time_clip(ms)
}

// The hours ahead of UTC that apply to a New Zealand wall-clock time given as if it were UTC
fn nz_offset_for_local(local_as_utc: f64) -> f64 {
    if nz_offset_hours(local_as_utc - 13.0 * 3_600_000.0) == 13.0 { 13.0 } else { 12.0 }
}

/// `new Date(v).getTime()` for a value read from JSON: a number is milliseconds, text is parsed, true is 1 and null is 0;
/// None for an invalid date (`NaN`).
#[must_use]
pub fn date_of(v: &JsonValue) -> Option<f64> {
    match v {
        JsonValue::Number(n) => time_clip(*n),
        JsonValue::String(s) => parse_date(s),
        JsonValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        JsonValue::Null => Some(0.0),
        _ => None,
    }
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sept", "Oct", "Nov", "Dec"];

/// The short month name `toLocaleDateString('en-NZ', { month: 'short' })` gives (September is "Sept").
#[must_use]
pub fn month_short(month: u32) -> &'static str {
    // The index is at most 11, so the fallback never applies
    MONTHS.get((month as usize).saturating_sub(1).min(11)).copied().unwrap_or("Dec")
}

/// `toLocaleDateString('en-NZ', { day: 'numeric', month: 'short', year: 'numeric' })` in New Zealand: "9 Oct 2026".
#[must_use]
pub fn day_month_year(ms: f64) -> String {
    let p = nz_parts(ms);
    format!("{} {} {}", p.day, month_short(p.month), p.year)
}

/// `toLocaleDateString('en-NZ', { month: 'short', year: 'numeric' })` in New Zealand: "Mar 2025".
#[must_use]
pub fn month_year(ms: f64) -> String {
    let p = nz_parts(ms);
    format!("{} {}", month_short(p.month), p.year)
}

/// `dateText(iso)` of feeds.js: "9 Oct 2026"; nothing (or an empty text) gives "", an unreadable date the text as stored (first 10 characters).
#[must_use]
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
#[must_use]
pub fn utc_date(ms: f64) -> String {
    let (y, m, d) = civil_from_days(f64_to_i64((ms / DAY_MS).floor()));
    format!("{y:04}-{m:02}-{d:02}")
}

/// A number as JavaScript prints it (`String(n)`).
#[must_use]
pub fn num_text(x: f64) -> String {
    number_to_js_string(x)
}

/// `+text`: the number a text reads as (NaN when it is not one).
#[must_use]
pub fn to_num(text: &str) -> f64 {
    string_to_number(text)
}
