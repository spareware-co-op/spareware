//! Calendar days as the compliance rules count them: JavaScript's UTC `Date` arithmetic, New Zealand's day for a moment
//! in time, and the long dates the reports print.

use super::jv::{Jx, V, pad_start};
use super::rx::rx;
use crate::jsnum::{f64_to_i64, f64_to_usize, i64_to_f64, number_to_js_string, u64_to_f64};

/// A day in milliseconds.
pub const DAY: f64 = 864e5;

/// The year, month (0 to 11), day of month and weekday (0 is Sunday) of a moment given in milliseconds since 1970 (UTC).
#[must_use]
pub fn parts(t: f64) -> Option<(i64, u32, u32, u32)> {
    if !t.is_finite() || t.abs() > 8.64e15 {
        return None;
    }
    // |t| <= 8.64e15, so the day count is within 10^8 and exact
    let days = f64_to_i64((t / DAY).floor());
    let (y, m, d) = civil_from_days(days);
    // rem_euclid(7) is in 0..7, so the conversion never falls back
    let wd = u32::try_from(days.saturating_add(4).rem_euclid(7)).unwrap_or(0);
    // m is in 1..=12
    Some((y, m.saturating_sub(1), d, wd))
}

/// The proleptic Gregorian date of a day number (days since 1970-01-01): (year, month 1 to 12, day).
#[must_use]
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    // Bound: doe is in 0..146_097, so yoe, doy and mp are small; era only saturates for an absurd day count
    let z = z.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = doe.saturating_sub(doe / 1460).saturating_add(doe / 36524).saturating_sub(doe / 146_096) / 365;
    let y = yoe.saturating_add(era.saturating_mul(400));
    let doy = doe.saturating_sub(yoe.saturating_mul(365).saturating_add(yoe / 4).saturating_sub(yoe / 100));
    let mp = doy.saturating_mul(5).saturating_add(2) / 153;
    // d is in 1..=31 and m in 1..=12, so the conversions never fall back
    let d = u32::try_from(doy.saturating_sub(mp.saturating_mul(153).saturating_add(2) / 5).saturating_add(1)).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp.saturating_add(3) } else { mp.saturating_sub(9) }).unwrap_or(1);
    (if m <= 2 { y.saturating_add(1) } else { y }, m, d)
}

/// The day number (days since 1970-01-01) of a date.
#[must_use]
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    // Bound: years of real dates are far inside i64 (a JavaScript date is within 275,760 years of 1970), and yoe, mp and
    // doy are small; the saturating steps only keep an absurd year from overflowing
    let y = if m <= 2 { y.saturating_sub(1) } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from(if m > 2 { m.saturating_sub(3) } else { m.saturating_add(9) });
    let doy = (mp.saturating_mul(153).saturating_add(2) / 5).saturating_add(i64::from(d)).saturating_sub(1);
    let doe = yoe.saturating_mul(365).saturating_add(yoe / 4).saturating_sub(yoe / 100).saturating_add(doy);
    era.saturating_mul(146_097).saturating_add(doe).saturating_sub(719_468)
}

// `x as u32`: truncated toward zero, NaN and anything below 0 giving 0, anything above u32::MAX giving u32::MAX
fn f64_to_u32(x: f64) -> u32 {
    u32::try_from(f64_to_usize(x)).unwrap_or(u32::MAX)
}

/// `Date.UTC(y, m0, d)`: midnight UTC of a date; a month or day out of range rolls over, and a year from 0 to 99 means 1900 to 1999.
#[must_use]
pub fn date_utc(y: f64, m0: f64, d: f64) -> f64 {
    if !(y.is_finite() && m0.is_finite() && d.is_finite()) {
        return f64::NAN;
    }
    let (y, m0, d) = (y.trunc(), m0.trunc(), d.trunc());
    let y = if (0.0..=99.0).contains(&y) { 1900.0 + y } else { y };
    let ym = y + (m0 / 12.0).floor();
    let mm = m0.rem_euclid(12.0);
    // ym is a whole number and mm a whole number in 0..12
    let days = i64_to_f64(days_from_civil(f64_to_i64(ym), f64_to_u32(mm).saturating_add(1), 1)) + d - 1.0;
    days * DAY
}

/// The weekday (0 is Sunday) of a moment.
#[must_use]
pub fn weekday(t: f64) -> u32 {
    parts(t).map_or(0, |p| p.3)
}

/// `isoUTC(t)`: "YYYY-MM-DD" of a moment (the year is not padded, as `getUTCFullYear` is not).
#[must_use]
pub fn iso_utc(t: f64) -> String {
    match parts(t) {
        Some((y, m0, d, _)) => format!("{}-{}-{}", y, pad_start(&m0.saturating_add(1).to_string(), 2, '0'), pad_start(&d.to_string(), 2, '0')),
        None => "NaN-NaN-NaN".to_string(),
    }
}

/// `parseDay(s)`: midnight UTC of the YYYY-MM-DD at the start of the text, None when it does not start with one.
#[must_use]
pub fn parse_day(s: &str) -> Option<f64> {
    let c = rx(r"^(\d{4})-(\d{2})-(\d{2})", "").exec(s)?;
    let n = |i: usize| c.get(i).parse::<f64>().unwrap_or(f64::NAN);
    Some(date_utc(n(1), n(2) - 1.0, n(3)))
}

/// `parseDay` for a loose value (`String(s || '')`).
#[must_use]
pub fn parse_day_v(v: &V) -> Option<f64> {
    if v.t() { parse_day(&v.s()) } else { None }
}

/// `validDay(s)`.
#[must_use]
pub fn valid_day(v: &V) -> bool {
    parse_day_v(v).is_some()
}

/// `validDay(s)` for text.
#[must_use]
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
        // m0 from parts is in 0..12; JavaScript would print "undefined" for any other
        Some((y, m0, d, _)) => format!("{} {} {}", d, usize::try_from(m0).ok().and_then(|i| MONTHS.get(i)).copied().unwrap_or("undefined"), y),
        None => "\u{2013}".to_string(),
    }
}

/// `longDate(s)` for a loose value.
#[must_use]
pub fn long_date_v(v: &V) -> String {
    if v.t() { long_date(&v.s()) } else { "\u{2013}".to_string() }
}

/// `dayName(s)`: "Wednesday", or empty for text that is not a day.
#[must_use]
pub fn day_name(s: &str) -> String {
    // weekday is in 0..7; JavaScript would give "undefined" for any other
    parse_day(s).map_or(String::new(), |t| usize::try_from(weekday(t)).ok().and_then(|i| WEEKDAYS.get(i)).copied().unwrap_or("undefined").to_string())
}

/// The moments (milliseconds) at which New Zealand changes between standard time (UTC+12) and daylight time (UTC+13) in a UTC year
/// (the current rule: the last Sunday of September to the first Sunday of April).
fn nz_changes(y: i64) -> (f64, f64) {
    let last_sun_sep = {
        let d = days_from_civil(y, 9, 30);
        d.saturating_sub(d.saturating_add(4).rem_euclid(7))
    };
    let first_sun_apr = {
        let d = days_from_civil(y, 4, 1);
        d.saturating_add(7_i64.saturating_sub(d.saturating_add(4).rem_euclid(7)) % 7)
    };
    // 02:00 standard time and 03:00 daylight time are both 14:00 UTC the day before
    (i64_to_f64(last_sun_sep) * DAY - 10.0 * 3600e3, i64_to_f64(first_sun_apr) * DAY - 10.0 * 3600e3)
}

/// The New Zealand calendar day of a moment, "YYYY-MM-DD" (`Intl.DateTimeFormat('en-NZ', { timeZone: 'Pacific/Auckland' })`).
#[must_use]
pub fn nz_day(t: f64) -> String {
    let Some((y, _, _, _)) = parts(t) else { return "NaN-NaN-NaN".to_string() };
    let (start, end) = nz_changes(y);
    let off = if t >= start || t < end { 13.0 } else { 12.0 };
    iso_utc(t + off * 3600e3)
}

/// `new Date(x).getTime()` for a loose value: a number is milliseconds, text is read as an ISO date or date and time.
#[must_use]
pub fn to_ms(v: &V, clock_ms: f64) -> f64 {
    match v {
        V::Null => clock_ms,
        V::Number(x) => *x,
        V::String(s) => parse_date_text(s),
        V::Bool(b) => f64::from(u8::from(*b)),
        _ => f64::NAN,
    }
}

/// `Date.parse` for ISO text: "2026-10-05", "2026-10-05T01:02:03Z", with milliseconds or an offset.
#[must_use]
pub fn parse_date_text(s: &str) -> f64 {
    let re = rx(r"^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,9}))?)?)?\s*(Z|[+-]\d{2}:?\d{2})?$", "i");
    let Some(caps) = re.exec(s.trim()) else { return f64::NAN };
    let n = |i: usize| caps.get(i).parse::<f64>().unwrap_or(0.0);
    let has_time = !caps.get(4).is_empty();
    let frac = if caps.get(7).is_empty() { 0.0 } else { (format!("0.{}", caps.get(7))).parse::<f64>().unwrap_or(0.0) * 1000.0 };
    let mut t = date_utc(n(1), n(2) - 1.0, n(3)) + n(4) * 3600e3 + n(5) * 60e3 + n(6) * 1e3 + frac.floor();
    let zone = caps.get(8);
    if !zone.is_empty() && !zone.eq_ignore_ascii_case("z") {
        let sign = if zone.starts_with('-') { -1.0 } else { 1.0 };
        let digits: String = zone.chars().filter(char::is_ascii_digit).collect();
        let (hh, mm) = (digits[..2].parse::<f64>().unwrap_or(0.0), digits[2..].parse::<f64>().unwrap_or(0.0));
        t -= sign * (hh * 3600e3 + mm * 60e3);
    } else if has_time && zone.is_empty() {
        // date and time without a zone is local time in the prototype's browser; the tests run on UTC
    }
    t
}

/// The system clock in milliseconds since 1970.
#[must_use]
pub fn system_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| u64_to_f64(u64::try_from(d.as_millis()).unwrap_or(u64::MAX)))
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
    CLOCK.with(std::cell::Cell::get).unwrap_or_else(system_ms)
}

/// `today(now)` of the compliance tab: today in New Zealand as YYYY-MM-DD. `now` can be a day as text (returned as given), a
/// timestamp in milliseconds, other date text, or null for the system clock.
#[must_use]
pub fn today(now: &V) -> String {
    if let V::String(s) = now
        && valid_day_s(s) {
            return crate::compliance::jv::slice(s, 0, Some(10));
        }
    let t = to_ms(now, now_ms());
    match parts(t) {
        Some(_) => nz_day(t),
        None => "NaN-NaN-NaN".to_string(),
    }
}

/// `String(n)` of a day count or year.
#[must_use]
pub fn istr(n: i64) -> String {
    number_to_js_string(i64_to_f64(n))
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
