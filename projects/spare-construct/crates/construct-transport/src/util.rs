//! The prototype's shared helpers (util.js, the `U` object) that the calculation scripts use.
//!
//! Geodesy on [lat, lng] pairs (distance, bearing, cumulative length, point along a line,
//! local flat projection, point to segment), angle differences and turns, the binary heap
//! behind the shortest-path searches, rounding, number formats, HTML escaping and CSV.
//! Every function mirrors util.js operation for operation, so the results are the same to
//! the bit (sin, cos and asin are V8's own, see [`crate::jsmath`]; atan2 is the platform's, so
//! a bearing may differ in the last bit; the truth tests in `tests/util_cases.rs` check this
//! against Node).
//! Not ported: `uid` (random ids; callers pass ids in), `download`, `debounce`, `$`, `$$`
//! and `sleep` (browser only), `parseCSV` and `deepMerge` (not used by the calculation core).

use crate::jsmath;
use crate::jsnum::{
    intl_format, is_js_space, math_hypot, math_max, math_min, math_round, number_to_js_string,
    or_default, split_digits, string_to_number, to_fixed, truthy_number, u64_to_f64,
    value_to_js_string,
};
use serde_json::Value;

/// A point as the prototype stores it: [latitude, longitude] in degrees.
pub type LatLng = [f64; 2];

/// Earth radius of `U.dist` (m): the IUGG mean radius, 6,371,008.8 m.
pub const EARTH_R: f64 = 6_371_008.8;

/// Degrees to radians, `Math.PI / 180`.
pub const D2R: f64 = std::f64::consts::PI / 180.0;

/// `U.dist(a, b)`: great-circle distance (m) by the haversine formula.
#[must_use]
pub fn dist(a: &LatLng, b: &LatLng) -> f64 {
    let d_lat = (b[0] - a[0]) * D2R;
    let d_lng = (b[1] - a[1]) * D2R;
    // V8 evaluates x ** 2 as x * x
    let s1 = jsmath::sin(d_lat / 2.0);
    let s2 = jsmath::sin(d_lng / 2.0);
    let s = s1 * s1 + jsmath::cos(a[0] * D2R) * jsmath::cos(b[0] * D2R) * (s2 * s2);
    2.0 * EARTH_R * jsmath::asin(math_min(1.0, s.sqrt()))
}

/// `U.cumLen(pts)`: cumulative length (m) at each point, starting at 0 (`[0]` for no points).
#[must_use]
pub fn cum_len(pts: &[LatLng]) -> Vec<f64> {
    let mut c = Vec::with_capacity(pts.len().max(1));
    let mut total = 0.0;
    c.push(total);
    for pair in pts.windows(2) {
        if let [a, b] = pair {
            total += dist(a, b);
            c.push(total);
        }
    }
    c
}

/// `U.polyLen(pts)`: length (m) of a polyline.
#[must_use]
pub fn poly_len(pts: &[LatLng]) -> f64 {
    cum_len(pts).last().copied().unwrap_or(0.0)
}

// `xs[i]` read as a number in JavaScript: past the end it is undefined, which reads as NaN
fn num_at(xs: &[f64], i: usize) -> f64 {
    xs.get(i).copied().unwrap_or(f64::NAN)
}

// An index as the signed segment index `U.along` returns. Bound: an index of something in memory
// is below isize::MAX, so the saturation never applies.
fn signed_index(i: usize) -> isize {
    isize::try_from(i).unwrap_or(isize::MAX)
}

/// `U.bearing(a, b)`: initial bearing from a to b, degrees clockwise from north in [0, 360).
#[must_use]
pub fn bearing(a: &LatLng, b: &LatLng) -> f64 {
    let p1 = a[0] * D2R;
    let p2 = b[0] * D2R;
    let dl = (b[1] - a[1]) * D2R;
    let y = jsmath::sin(dl) * jsmath::cos(p2);
    let x = jsmath::cos(p1) * jsmath::sin(p2) - jsmath::sin(p1) * jsmath::cos(p2) * jsmath::cos(dl);
    // JavaScript's % is the C fmod, as Rust's % on f64 is
    (y.atan2(x) / D2R + 360.0) % 360.0
}

/// `U.angDiff(a, b)`: unsigned difference between two headings, degrees in [0, 180].
#[must_use]
pub fn ang_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).abs() % 360.0;
    if d > 180.0 { 360.0 - d } else { d }
}

/// `U.turn(a, b)`: signed turn from heading a to heading b, degrees, positive clockwise (a right turn).
#[must_use]
pub fn turn(a: f64, b: f64) -> f64 {
    ((b - a + 540.0) % 360.0) - 180.0
}

/// `U.flat(lat0)`: flat projection in metres around a reference latitude (about 0.1% across a few km).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flat {
    /// Metres per degree of longitude.
    pub kx: f64,
    /// Metres per degree of latitude.
    pub ky: f64,
}

/// `U.flat(lat0)`.
#[must_use]
pub fn flat(lat0: f64) -> Flat {
    Flat {
        kx: 111_320.0 * jsmath::cos(lat0 * D2R),
        ky: 110_574.0,
    }
}

impl Flat {
    /// `F.x(ll)`: easting (m).
    #[must_use]
    pub fn x(&self, ll: &LatLng) -> f64 {
        ll[1] * self.kx
    }
    /// `F.y(ll)`: northing (m).
    #[must_use]
    pub fn y(&self, ll: &LatLng) -> f64 {
        ll[0] * self.ky
    }
}

/// The point `U.along` finds: position p, segment index i (from point i to i + 1) and fraction f along it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Along {
    /// The point.
    pub p: LatLng,
    /// Segment index; -1 when s is past the end of a one-point line, as in the JavaScript.
    pub i: isize,
    /// Fraction along the segment.
    pub f: f64,
}

/// `U.along(pts, cum, s)`: point at distance s along a polyline with cumulative lengths cum.
///
/// cum holds one length per point, as [`cum_len`] gives them. With no points (which the
/// JavaScript does not handle) the point is NaN, NaN.
#[must_use]
pub fn along(pts: &[LatLng], cum: &[f64], s: f64) -> Along {
    let (Some(&first), Some(&last)) = (pts.first(), pts.last()) else {
        return Along {
            p: [f64::NAN; 2],
            i: -1,
            f: f64::NAN,
        };
    };
    let n = pts.len();
    let total = num_at(cum, n.saturating_sub(1));
    if s <= 0.0 {
        return Along {
            p: first,
            i: 0,
            f: 0.0,
        };
    }
    if s >= total {
        return Along {
            p: last,
            i: signed_index(n).saturating_sub(2),
            f: 1.0,
        };
    }
    let (mut lo, mut hi) = (0usize, n.saturating_sub(1));
    while hi.saturating_sub(lo) > 1 {
        let mid = usize::midpoint(lo, hi);
        if num_at(cum, mid) <= s {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let (c0, c1) = (num_at(cum, lo), num_at(cum, lo.saturating_add(1)));
    let f = (s - c0) / or_default(c1 - c0, 1.0);
    let point = match pts.get(lo..) {
        Some([from, to, ..]) => [
            from[0] + (to[0] - from[0]) * f,
            from[1] + (to[1] - from[1]) * f,
        ],
        // cum longer than pts: the JavaScript reads undefined coordinates
        _ => [f64::NAN; 2],
    };
    Along {
        p: point,
        i: signed_index(lo),
        f,
    }
}

/// What `U.segDist` returns: distance d from the point to the segment, parameter t in [0, 1] and the nearest point (x, y).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegDist {
    /// Distance.
    pub d: f64,
    /// Position along the segment, 0 at a, 1 at b.
    pub t: f64,
    /// Nearest point, x.
    pub x: f64,
    /// Nearest point, y.
    pub y: f64,
}

/// `U.segDist(px, py, ax, ay, bx, by)`: nearest point on segment a-b to point p, in flat coordinates.
#[must_use]
pub fn seg_dist(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> SegDist {
    let dx = bx - ax;
    let dy = by - ay;
    let l2 = dx * dx + dy * dy;
    let mut t = if truthy_number(l2) {
        ((px - ax) * dx + (py - ay) * dy) / l2
    } else {
        0.0
    };
    // t < 0 ? 0 : t > 1 ? 1 : t, which is U.clamp's own expression
    t = clamp(t, 0.0, 1.0);
    let x = ax + t * dx;
    let y = ay + t * dy;
    SegDist {
        d: math_hypot(&[px - x, py - y]),
        t,
        x,
        y,
    }
}

/// `U.Heap`: binary min-heap of (number key, integer value), as util.js orders it; callers skip stale entries.
///
/// The sift steps are the JavaScript's, comparison for comparison, so equal keys leave the
/// heap in the same order and shortest-path ties break the same way.
#[derive(Clone, Debug, Default)]
pub struct Heap {
    // (key, value) slots; the first n are the heap, as util.js keeps its typed arrays
    e: Vec<(f64, i32)>,
    n: usize,
    /// Key of the entry the last `pop` returned (`heap.top`; 0 before any pop).
    pub top: f64,
}

impl Heap {
    /// An empty heap.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(1024)
    }
    /// An empty heap with room for cap entries before it grows.
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Heap {
            e: Vec::with_capacity(cap),
            n: 0,
            top: 0.0,
        }
    }
    fn key(&self, i: usize) -> f64 {
        self.e.get(i).map_or(f64::NAN, |e| e.0)
    }
    fn set(&mut self, i: usize, entry: (f64, i32)) {
        if let Some(slot) = self.e.get_mut(i) {
            *slot = entry;
        }
    }
    fn entry(&self, i: usize) -> (f64, i32) {
        self.e.get(i).copied().unwrap_or((f64::NAN, 0))
    }
    /// `heap.clear()`.
    pub fn clear(&mut self) {
        self.n = 0;
    }
    /// `heap.n`: number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.n
    }
    /// True when `heap.n` is 0.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    /// `heap.push(key, val)`.
    pub fn push(&mut self, key: f64, val: i32) {
        if self.n == self.e.len() {
            self.e.push((0.0, 0));
        }
        let mut i = self.n;
        self.n = self.n.saturating_add(1);
        while i > 0 {
            let p = i.saturating_sub(1) / 2;
            let parent = self.entry(p);
            if parent.0 <= key {
                break;
            }
            self.set(i, parent);
            i = p;
        }
        self.set(i, (key, val));
    }
    /// `heap.pop()`: removes the entry with the smallest key and returns its value; sets `top` to its key.
    ///
    /// None when the heap is empty (the JavaScript reads past its entries there; callers check
    /// `heap.n` first).
    pub fn pop(&mut self) -> Option<i32> {
        let n = self.n.checked_sub(1)?;
        let (top, rv) = self.entry(0);
        self.top = top;
        self.n = n;
        if n > 0 {
            let (key, val) = self.entry(n);
            let mut i = 0usize;
            loop {
                // Bound: i < n, and n counts slots in memory, so 2i + 2 stays far below usize::MAX
                let mut c = i.saturating_mul(2).saturating_add(1);
                if c >= n {
                    break;
                }
                let right = c.saturating_add(1);
                if right < n && self.key(right) < self.key(c) {
                    c = right;
                }
                let child = self.entry(c);
                if child.0 >= key {
                    break;
                }
                self.set(i, child);
                i = c;
            }
            self.set(i, (key, val));
        }
        Some(rv)
    }
}

/// The clock the engine stamps runs with, set by a host that has no system clock (WebAssembly).
static CLOCK: std::sync::OnceLock<fn() -> f64> = std::sync::OnceLock::new();

/// Sets the clock [`now_ms`] reads (milliseconds since 1970). The first call wins. Natively the system clock is used unless this is set.
pub fn set_clock(f: fn() -> f64) {
    let _ = CLOCK.set(f);
}

/// `Date.now()`: milliseconds since 1970 from the clock set with [`set_clock`], otherwise the system clock (0 where there is none, as in a browser build that set no clock).
pub fn now_ms() -> f64 {
    if let Some(f) = CLOCK.get() {
        return f();
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|d| u64::try_from(d.as_millis()).ok())
            .map_or(0.0, u64_to_f64)
    }
    #[cfg(target_arch = "wasm32")]
    {
        0.0
    }
}

/// `U.clamp(x, a, b)`: x limited to [a, b]; NaN stays NaN.
#[must_use]
pub fn clamp(x: f64, a: f64, b: f64) -> f64 {
    if x < a {
        a
    } else if x > b {
        b
    } else {
        x
    }
}

/// `U.r7(x)`: rounded to 7 decimal places (`Math.round(x * 1e7) / 1e7`).
#[must_use]
pub fn r7(x: f64) -> f64 {
    math_round(x * 1e7) / 1e7
}

/// `U.r1(x)`: rounded to 1 decimal place (`Math.round(x * 10) / 10`).
#[must_use]
pub fn r1(x: f64) -> f64 {
    math_round(x * 10.0) / 10.0
}

/// `U.r3(x)`: rounded to 3 decimal places (`Math.round(x * 1000) / 1000`).
#[must_use]
pub fn r3(x: f64) -> f64 {
    math_round(x * 1000.0) / 1000.0
}

/// `U.fmt`: number formats for text outputs (Intl.NumberFormat 'en-NZ' underneath).
///
/// The JavaScript takes any value; these take the number the JavaScript would see. For a
/// loose value pass `+x` (jsnum's `Num::num`), with one exception: `fmt.num`, `money`,
/// `dur`, `len`, `signed` and `signedPct` print "–" for null as well as undefined, so pass
/// NaN for null there.
pub mod fmt {
    use super::{intl_format, math_round, string_to_number, to_fixed, math_max, number_to_js_string};

    /// The en dash the formats print for a missing or non-finite value.
    pub const DASH: &str = "\u{2013}";

    /// `U.fmt.num(x, d)`: grouped with commas, exactly d decimals; "–" for NaN and infinities.
    #[must_use]
    pub fn num(x: f64, d: usize) -> String {
        if !x.is_finite() {
            return DASH.to_string();
        }
        intl_format(x, d)
    }

    /// `U.fmt.int(x)`: `num(Math.round(x))`.
    #[must_use]
    pub fn int(x: f64) -> String {
        num(math_round(x), 0)
    }

    /// `U.fmt.pct(x, d)`: x as a percentage with d decimals ("0.25" gives "25%").
    #[must_use]
    pub fn pct(x: f64, d: usize) -> String {
        if x.is_finite() {
            num(x * 100.0, d) + "%"
        } else {
            DASH.to_string()
        }
    }

    /// `U.fmt.money(m)`: NZ$ millions as "$1.25b", "$12.5m", "$2.50m" or "$40k"; negatives with a minus sign (U+2212).
    #[must_use]
    pub fn money(m: f64) -> String {
        if !m.is_finite() {
            return DASH.to_string();
        }
        let a = m.abs();
        let s = if m < 0.0 { "\u{2212}" } else { "" };
        if a >= 1000.0 {
            format!("{}${}b", s, num(a / 1000.0, 2))
        } else if a >= 10.0 {
            format!("{}${}m", s, num(a, 1))
        } else if a >= 0.1 {
            format!("{}${}m", s, num(a, 2))
        } else {
            format!("{}${}k", s, num(a * 1000.0, 0))
        }
    }

    /// `U.fmt.dur(s)`: seconds as "45 s", "12.5 min" or "2.3 h".
    #[must_use]
    pub fn dur(s: f64) -> String {
        if !s.is_finite() {
            return DASH.to_string();
        }
        if s < 90.0 {
            return num(s, 0) + " s";
        }
        let m = s / 60.0;
        if m < 90.0 {
            num(m, 1) + " min"
        } else {
            num(m / 60.0, 1) + " h"
        }
    }

    /// `U.fmt.len(m)`: metres as "850 m" or "1.25 km".
    #[must_use]
    pub fn len(m: f64) -> String {
        if !m.is_finite() {
            return DASH.to_string();
        }
        if m < 1000.0 {
            num(m, 0) + " m"
        } else {
            num(m / 1000.0, 2) + " km"
        }
    }

    /// `U.fmt.signed(x, d)`: "+12", "−3" or "±0", rounded first by `toFixed(d)`.
    #[must_use]
    pub fn signed(x: f64, d: usize) -> String {
        if !x.is_finite() {
            return DASH.to_string();
        }
        let r = string_to_number(&to_fixed(x, d));
        sign_of(r).to_string() + &num(r.abs(), d)
    }

    /// `U.fmt.signedPct(x, d)`: x as a signed percentage, "+4.5%".
    #[must_use]
    pub fn signed_pct(x: f64, d: usize) -> String {
        if !x.is_finite() {
            return DASH.to_string();
        }
        let r = string_to_number(&to_fixed(x * 100.0, d));
        sign_of(r).to_string() + &num(r.abs(), d) + "%"
    }

    fn sign_of(r: f64) -> &'static str {
        if r > 0.0 {
            "+"
        } else if r < 0.0 {
            "\u{2212}"
        } else {
            "\u{b1}"
        }
    }

    /// `U.fmt.clock(s)`: seconds as "hh:mm:ss" on a 24-hour clock.
    #[must_use]
    pub fn clock(s: f64) -> String {
        let s = math_max(0.0, s.floor());
        let h = (s / 3600.0).floor() % 24.0;
        let m = (s / 60.0).floor() % 60.0;
        let ss = s % 60.0;
        [h, m, ss]
            .iter()
            .map(|v| {
                let t = number_to_js_string(*v);
                if t.chars().count() < 2 {
                    format!("0{t}")
                } else {
                    t
                }
            })
            .collect::<Vec<_>>()
            .join(":")
    }
}

/// `U.esc(s)`: HTML-escapes & < > " and ' (the JavaScript first turns null and undefined into "").
#[must_use]
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

// NUMLIKE in util.js: /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i
fn is_numlike(s: &str) -> bool {
    let b = s.as_bytes();
    let b = match b.split_first() {
        Some((b'-' | b'+', unsigned)) => unsigned,
        _ => b,
    };
    let rest = match b.split_first() {
        Some((c, _)) if c.is_ascii_digit() => {
            let (_, after) = split_digits(b);
            match after.split_first() {
                Some((b'.', frac)) => split_digits(frac).1,
                _ => after,
            }
        }
        Some((b'.', frac)) => {
            let (digits, after) = split_digits(frac);
            if digits.is_empty() {
                return false;
            }
            after
        }
        _ => return false,
    };
    let rest = match rest.split_first() {
        Some((b'e' | b'E', exp)) => {
            let exp = match exp.split_first() {
                Some((b'-' | b'+', unsigned)) => unsigned,
                _ => exp,
            };
            let (digits, after) = split_digits(exp);
            if digits.is_empty() {
                return false;
            }
            after
        }
        _ => rest,
    };
    rest.is_empty()
}

/// `U.csvCell(v)`: one RFC 4180 cell; text that a spreadsheet would read as a formula gets a leading apostrophe.
///
/// null gives an empty cell (as undefined does in the JavaScript); numbers, booleans,
/// arrays and objects are written as `String(v)`.
pub fn csv_cell(v: &Value) -> String {
    let mut s = if v.is_null() {
        String::new()
    } else {
        value_to_js_string(v)
    };
    if v.is_string() {
        let first = s.chars().next();
        let trimmed = s.trim_matches(is_js_space);
        if matches!(first, Some('=' | '+' | '-' | '@' | '\t' | '\r')) && !is_numlike(trimmed) {
            s.insert(0, '\'');
        }
    }
    if s.contains(['"', ',', '\r', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

/// `U.csv(rows)`: cells joined with commas, rows with CRLF.
#[must_use]
pub fn csv(rows: &[Vec<Value>]) -> String {
    rows.iter()
        .map(|r| r.iter().map(csv_cell).collect::<Vec<_>>().join(","))
        .collect::<Vec<_>>()
        .join("\r\n")
}
