//! The map box the study area is drawn as (Leaflet's `LatLngBounds`, the parts the prototype uses) and New Zealand dates.

use crate::census::Bx;
use crate::jsnum::{math_max, math_min};

/// `L.latLngBounds(points)`: the box that holds every [lat, lng] point (a list of two corners in any order, or more points).
pub fn latlng_bounds(points: &[[f64; 2]]) -> Bx {
    let mut b = Bx::new(f64::NAN, f64::NAN, f64::NAN, f64::NAN);
    let mut first = true;
    for p in points {
        if first {
            b = Bx::new(p[0], p[1], p[0], p[1]);
            first = false;
        } else {
            extend(&mut b, p[0], p[1]);
        }
    }
    b
}

/// `bounds.extend([lat, lng])`: grows the box to hold the point.
pub fn extend(b: &mut Bx, lat: f64, lng: f64) {
    b.s = math_min(lat, b.s);
    b.w = math_min(lng, b.w);
    b.n = math_max(lat, b.n);
    b.e = math_max(lng, b.e);
}

/// `bounds.pad(ratio)`: the box grown on every side by a share of its height and width.
pub fn pad(b: &Bx, ratio: f64) -> Bx {
    let hb = (b.s - b.n).abs() * ratio;
    let wb = (b.w - b.e).abs() * ratio;
    Bx::new(b.s - hb, b.w - wb, b.n + hb, b.e + wb)
}

/// `bounds.getCenter()`: [lat, lng] of the middle.
pub fn center(b: &Bx) -> [f64; 2] {
    [(b.s + b.n) / 2.0, (b.w + b.e) / 2.0]
}

/// `bounds.contains([lat, lng])`: the point is inside or on the edge.
pub fn contains(b: &Bx, lat: f64, lng: f64) -> bool {
    lat >= b.s && lat <= b.n && lng >= b.w && lng <= b.e
}

/// A New Zealand local date.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NzDate {
    /// Year.
    pub y: i64,
    /// Month, 1 to 12.
    pub m: i64,
    /// Day of the month.
    pub d: i64,
}

// Days since 1970-01-01 of a calendar date (Howard Hinnant's algorithm)
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// The weekday of a day number, 0 for Sunday
fn weekday(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

const HOUR: f64 = 3_600_000.0;
const DAY: f64 = 86_400_000.0;

// The day number of the last Sunday of September, and of the first Sunday of April, of a year (New Zealand's daylight saving since 2007)
fn dst_start_day(y: i64) -> i64 {
    let last = days_from_civil(y, 9, 30);
    last - weekday(last)
}
fn dst_end_day(y: i64) -> i64 {
    let first = days_from_civil(y, 4, 1);
    first + (7 - weekday(first)) % 7
}

/// The offset of New Zealand time from UTC at an instant (ms since 1970): 13 hours in daylight saving time (the last Sunday of September at 2 am to the first Sunday of April at 3 am), else 12.
pub fn nz_offset_ms(utc_ms: f64) -> f64 {
    let days = (utc_ms / DAY).floor() as i64;
    let y = civil_from_days(days).0;
    // the instants daylight time starts and ends in this year (in UTC)
    let start = dst_start_day(y) as f64 * DAY + 2.0 * HOUR - 12.0 * HOUR;
    let end = dst_end_day(y) as f64 * DAY + 3.0 * HOUR - 13.0 * HOUR;
    if utc_ms < end || utc_ms >= start { 13.0 * HOUR } else { 12.0 * HOUR }
}

/// `nzDate(now)`: the New Zealand date at an instant (ms since 1970).
pub fn nz_date(utc_ms: f64) -> NzDate {
    let local = utc_ms + nz_offset_ms(utc_ms);
    let (y, m, d) = civil_from_days((local / DAY).floor() as i64);
    NzDate { y, m, d }
}

/// `new Date(ms).toLocaleDateString('en-NZ', { month: 'short', year: 'numeric' })` in New Zealand time: "Mar 2025" ("Sept" for September).
pub fn nz_month_year(utc_ms: f64) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sept", "Oct", "Nov", "Dec"];
    let d = nz_date(utc_ms);
    format!("{} {}", MONTHS[(d.m - 1) as usize], d.y)
}

/// `new Date(ms).toISOString().slice(0, 10)`: the UTC date, YYYY-MM-DD.
pub fn utc_date(utc_ms: f64) -> String {
    let (y, m, d) = civil_from_days((utc_ms / DAY).floor() as i64);
    format!("{y:04}-{m:02}-{d:02}")
}
