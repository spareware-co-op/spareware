//! Calendar days as the compliance rules count them: JavaScript's UTC `Date` arithmetic, New Zealand's day for a moment
//! in time, and the long dates the reports print.

use super::jv::{Jx, V, pad_start};
use super::rx::rx;
use crate::jsnum::number_to_js_string;

/// A day in milliseconds.
pub const DAY: f64 = 864e5;

/// The year, month (0 to 11), day of month and weekday (0 is Sunday) of a moment given in milliseconds since 1970 (UTC).
pub fn parts(t: f64) -> Option<(i64, u32, u32, u32)> {
    if !t.is_finite() || t.abs() > 8.64e15 {
        return None;
    }
    let days = (t / DAY).floor() as i64;
    let (y, m, d) = civil_from_days(days);
    let wd = (days + 4).rem_euclid(7) as u32;
    Some((y, m - 1, d, wd))
}

/// The proleptic Gregorian date of a day number (days since 1970-01-01): (year, month 1 to 12, day).
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The day number (days since 1970-01-01) of a date.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// `Date.UTC(y, m0, d)`: midnight UTC of a date; a month or day out of range rolls over, and a year from 0 to 99 means 1900 to 1999.
pub fn date_utc(y: f64, m0: f64, d: f64) -> f64 {
    if !(y.is_finite() && m0.is_finite() && d.is_finite()) {
        return f64::NAN;
    }
    let (y, m0, d) = (y.trunc(), m0.trunc(), d.trunc());
    let y = if (0.0..=99.0).contains(&y) { 1900.0 + y } else { y };
    let ym = y + (m0 / 12.0).floor();
    let mm = m0.rem_euclid(12.0);
    let days = days_from_civil(ym as i64, mm as u32 + 1, 1) as f64 + d - 1.0;
    days * DAY
}

/// The weekday (0 is Sunday) of a moment.
pub fn weekday(t: f64) -> u32 {
    parts(t).map_or(0, |p| p.3)
}

/// `isoUTC(t)`: "YYYY-MM-DD" of a moment (the year is not padded, as `getUTCFullYear` is not).
pub fn iso_utc(t: f64) -> String {
    match parts(t) {
        Some((y, m0, d, _)) => format!("{}-{}-{}", y, pad_start(&(m0 + 1).to_string(), 2, '0'), pad_start(&d.to_string(), 2, '0')),
        None => "NaN-NaN-NaN".to_string(),
    }
}

/// `parseDay(s)`: midnight UTC of the YYYY-MM-DD at the start of the text, None when it does not start with one.
pub fn parse_day(s: &str) -> Option<f64> {
    let c = rx(r"^(\d{4})-(\d{2})-(\d{2})", "").exec(s)?;
    let n = |i: usize| c.get(i).parse::<f64>().unwrap_or(f64::NAN);
    Some(date_utc(n(1), n(2) - 1.0, n(3)))
}

/// `parseDay` for a loose value (`String(s || '')`).
pub fn parse_day_v(v: &V) -> Option<f64> {
    if v.t() { parse_day(&v.s()) } else { None }
}

/// `validDay(s)`.
pub fn valid_day(v: &V) -> bool {
    parse_day_v(v).is_some()
}

/// `validDay(s)` for text.
pub fn valid_day_s(s: &str) -> bool {
    parse_day(s).is_some()
}

/// The month names `longDate` prints.
pub const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// The weekday names.
pub const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/// `longDate(s)`: "21 October 2026", or an en dash for text that is not a day.
pub fn long_date(s: &str) -> String {
    match parse_day(s).and_then(parts) {
        Some((y, m0, d, _)) => format!("{} {} {}", d, MONTHS[m0 as usize], y),
        None => "\u{2013}".to_string(),
    }
}

/// `longDate(s)` for a loose value.
pub fn long_date_v(v: &V) -> String {
    if v.t() { long_date(&v.s()) } else { "\u{2013}".to_string() }
}

/// `dayName(s)`: "Wednesday", or empty for text that is not a day.
pub fn day_name(s: &str) -> String {
    parse_day(s).map_or(String::new(), |t| WEEKDAYS[weekday(t) as usize].to_string())
}

/// The moments (milliseconds) at which New Zealand changes between standard time (UTC+12) and daylight time (UTC+13) in a UTC year
/// (the current rule: the last Sunday of September to the first Sunday of April).
fn nz_changes(y: i64) -> (f64, f64) {
    let last_sun_sep = {
        let d = days_from_civil(y, 9, 30);
        d - (d + 4).rem_euclid(7)
    };
    let first_sun_apr = {
        let d = days_from_civil(y, 4, 1);
        d + (7 - (d + 4).rem_euclid(7)) % 7
    };
    // 02:00 standard time and 03:00 daylight time are both 14:00 UTC the day before
    ((last_sun_sep as f64) * DAY - 10.0 * 3600e3, (first_sun_apr as f64) * DAY - 10.0 * 3600e3)
}

/// The New Zealand calendar day of a moment, "YYYY-MM-DD" (`Intl.DateTimeFormat('en-NZ', { timeZone: 'Pacific/Auckland' })`).
pub fn nz_day(t: f64) -> String {
    let Some((y, _, _, _)) = parts(t) else { return "NaN-NaN-NaN".to_string() };
    let (start, end) = nz_changes(y);
    let off = if t >= start || t < end { 13.0 } else { 12.0 };
    iso_utc(t + off * 3600e3)
}

/// `new Date(x).getTime()` for a loose value: a number is milliseconds, text is read as an ISO date or date and time.
pub fn to_ms(v: &V, clock_ms: f64) -> f64 {
    match v {
        V::Null => clock_ms,
        V::Number(x) => *x,
        V::String(s) => parse_date_text(s),
        V::Bool(b) => *b as i32 as f64,
        _ => f64::NAN,
    }
}

/// `Date.parse` for ISO text: "2026-10-05", "2026-10-05T01:02:03Z", with milliseconds or an offset.
pub fn parse_date_text(s: &str) -> f64 {
    let re = rx(r"^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,9}))?)?)?\s*(Z|[+-]\d{2}:?\d{2})?$", "i");
    let Some(c) = re.exec(s.trim()) else { return f64::NAN };
    let n = |i: usize| c.get(i).parse::<f64>().unwrap_or(0.0);
    let has_time = !c.get(4).is_empty();
    let frac = if c.get(7).is_empty() { 0.0 } else { (format!("0.{}", c.get(7))).parse::<f64>().unwrap_or(0.0) * 1000.0 };
    let mut t = date_utc(n(1), n(2) - 1.0, n(3)) + n(4) * 3600e3 + n(5) * 60e3 + n(6) * 1e3 + frac.floor();
    let z = c.get(8);
    if !z.is_empty() && !z.eq_ignore_ascii_case("z") {
        let sign = if z.starts_with('-') { -1.0 } else { 1.0 };
        let digits: String = z.chars().filter(|c| c.is_ascii_digit()).collect();
        let (hh, mm) = (digits[..2].parse::<f64>().unwrap_or(0.0), digits[2..].parse::<f64>().unwrap_or(0.0));
        t -= sign * (hh * 3600e3 + mm * 60e3);
    } else if has_time && z.is_empty() {
        // date and time without a zone is local time in the prototype's browser; the tests run on UTC
    }
    t
}

/// The system clock in milliseconds since 1970.
pub fn system_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_millis() as f64)
}

thread_local! {
    static CLOCK: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
}

/// Fixes the clock the checks read (`Date.now()` in the prototype) to a moment in milliseconds since 1970, or back to the system
/// clock with None. The setting belongs to the calling thread; tests use it to get the same dates the prototype's tests did.
pub fn set_clock(ms: Option<f64>) {
    CLOCK.with(|c| c.set(ms));
}

/// `Date.now()`: the fixed clock when one is set ([`set_clock`]), otherwise the system clock.
pub fn now_ms() -> f64 {
    CLOCK.with(|c| c.get()).unwrap_or_else(system_ms)
}

/// `today(now)` of the compliance tab: today in New Zealand as YYYY-MM-DD. `now` can be a day as text (returned as given), a
/// timestamp in milliseconds, other date text, or null for the system clock.
pub fn today(now: &V) -> String {
    if let V::String(s) = now {
        if valid_day_s(s) {
            return crate::compliance::jv::slice(s, 0, Some(10));
        }
    }
    let t = to_ms(now, now_ms());
    match parts(t) {
        Some(_) => nz_day(t),
        None => "NaN-NaN-NaN".to_string(),
    }
}

/// `String(n)` of a day count or year.
pub fn istr(n: i64) -> String {
    number_to_js_string(n as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_work_like_js() {
        assert_eq!(iso_utc(date_utc(2026.0, 9.0, 21.0)), "2026-10-21");
        assert_eq!(iso_utc(date_utc(2026.0, 12.0, 1.0)), "2027-01-01");
        assert_eq!(iso_utc(date_utc(2026.0, 0.0, 0.0)), "2025-12-31");
        assert_eq!(weekday(date_utc(2026.0, 9.0, 21.0)), 3);
        assert_eq!(long_date("2026-10-21"), "21 October 2026");
        assert_eq!(day_name("2026-10-21"), "Wednesday");
        assert_eq!(nz_day(date_utc(2026.0, 9.0, 21.0)), "2026-10-21");
        // 11:30 UTC on 20 Oct is 00:30 on 21 Oct in New Zealand (daylight time)
        assert_eq!(nz_day(date_utc(2026.0, 9.0, 20.0) + 11.5 * 3600e3), "2026-10-21");
        // in winter 11:30 UTC is 23:30 the same day
        assert_eq!(nz_day(date_utc(2026.0, 6.0, 20.0) + 11.5 * 3600e3), "2026-07-20");
        assert_eq!(today(&V::String("2026-05-01T10:00:00Z".into())), "2026-05-01");
        assert_eq!(parse_date_text("2026-10-05T01:02:03Z"), date_utc(2026.0, 9.0, 5.0) + 3723e3);
    }
}
