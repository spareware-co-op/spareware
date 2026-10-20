//! The prototype's shared helpers (util.js, the `U` object) that the calculation scripts use.
//!
//! Geodesy on [lat, lng] pairs (distance, bearing, cumulative length, point along a line,
//! local flat projection, point to segment), angle differences and turns, the binary heap
//! behind the shortest-path searches, rounding, number formats, HTML escaping and CSV.
//! Every function mirrors util.js operation for operation, so the results are the same to
//! the bit wherever the platform's sin, cos, asin and atan2 agree with V8's (the truth
//! tests in tests/util_cases.rs check this against Node).
//! Not ported: `uid` (random ids; callers pass ids in), `download`, `debounce`, `$`, `$$`
//! and `sleep` (browser only), `parseCSV` and `deepMerge` (not used by the calculation core).

use crate::jsnum::{
    intl_format, is_js_space, math_hypot, math_max, math_min, math_round, number_to_js_string,
    or_default, string_to_number, to_fixed, truthy_number, value_to_js_string,
};
use serde_json::Value;

/// A point as the prototype stores it: [latitude, longitude] in degrees.
pub type LatLng = [f64; 2];

/// Earth radius of `U.dist` (m): the IUGG mean radius, 6,371,008.8 m.
pub const EARTH_R: f64 = 6371008.8;

/// Degrees to radians, `Math.PI / 180`.
pub const D2R: f64 = std::f64::consts::PI / 180.0;

/// `U.dist(a, b)`: great-circle distance (m) by the haversine formula.
pub fn dist(a: &LatLng, b: &LatLng) -> f64 {
    let d_lat = (b[0] - a[0]) * D2R;
    let d_lng = (b[1] - a[1]) * D2R;
    // V8 evaluates x ** 2 as x * x
    let s1 = (d_lat / 2.0).sin();
    let s2 = (d_lng / 2.0).sin();
    let s = s1 * s1 + (a[0] * D2R).cos() * (b[0] * D2R).cos() * (s2 * s2);
    2.0 * EARTH_R * math_min(1.0, s.sqrt()).asin()
}

/// `U.cumLen(pts)`: cumulative length (m) at each point, starting at 0 (`[0]` for no points).
pub fn cum_len(pts: &[LatLng]) -> Vec<f64> {
    let mut c = Vec::with_capacity(pts.len().max(1));
    c.push(0.0);
    for i in 1..pts.len() {
        c.push(c[i - 1] + dist(&pts[i - 1], &pts[i]));
    }
    c
}

/// `U.polyLen(pts)`: length (m) of a polyline.
pub fn poly_len(pts: &[LatLng]) -> f64 {
    let c = cum_len(pts);
    c[c.len() - 1]
}

/// `U.bearing(a, b)`: initial bearing from a to b, degrees clockwise from north in [0, 360).
pub fn bearing(a: &LatLng, b: &LatLng) -> f64 {
    let p1 = a[0] * D2R;
    let p2 = b[0] * D2R;
    let dl = (b[1] - a[1]) * D2R;
    let y = dl.sin() * p2.cos();
    let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
    // JavaScript's % is the C fmod, as Rust's % on f64 is
    (y.atan2(x) / D2R + 360.0) % 360.0
}

/// `U.angDiff(a, b)`: unsigned difference between two headings, degrees in [0, 180].
pub fn ang_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).abs() % 360.0;
    if d > 180.0 { 360.0 - d } else { d }
}

/// `U.turn(a, b)`: signed turn from heading a to heading b, degrees, positive clockwise (a right turn).
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
pub fn flat(lat0: f64) -> Flat {
    Flat {
        kx: 111320.0 * (lat0 * D2R).cos(),
        ky: 110574.0,
    }
}

impl Flat {
    /// `F.x(ll)`: easting (m).
    pub fn x(&self, ll: &LatLng) -> f64 {
        ll[1] * self.kx
    }
    /// `F.y(ll)`: northing (m).
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

/// `U.along(pts, cum, s)`: point at distance s along a polyline with cumulative lengths cum (pts must not be empty).
pub fn along(pts: &[LatLng], cum: &[f64], s: f64) -> Along {
    let n = pts.len();
    let l = cum[n - 1];
    if s <= 0.0 {
        return Along {
            p: pts[0],
            i: 0,
            f: 0.0,
        };
    }
    if s >= l {
        return Along {
            p: pts[n - 1],
            i: n as isize - 2,
            f: 1.0,
        };
    }
    let (mut lo, mut hi) = (0usize, n - 1);
    while hi - lo > 1 {
        let m = (lo + hi) >> 1;
        if cum[m] <= s {
            lo = m;
        } else {
            hi = m;
        }
    }
    let f = (s - cum[lo]) / or_default(cum[lo + 1] - cum[lo], 1.0);
    let p = [
        pts[lo][0] + (pts[lo + 1][0] - pts[lo][0]) * f,
        pts[lo][1] + (pts[lo + 1][1] - pts[lo][1]) * f,
    ];
    Along {
        p,
        i: lo as isize,
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
    k: Vec<f64>,
    v: Vec<i32>,
    n: usize,
    /// Key of the entry the last `pop` returned (`heap.top`; 0 before any pop).
    pub top: f64,
}

impl Heap {
    /// An empty heap.
    pub fn new() -> Self {
        Self::with_capacity(1024)
    }
    /// An empty heap with room for cap entries before it grows.
    pub fn with_capacity(cap: usize) -> Self {
        Heap {
            k: Vec::with_capacity(cap),
            v: Vec::with_capacity(cap),
            n: 0,
            top: 0.0,
        }
    }
    /// `heap.clear()`.
    pub fn clear(&mut self) {
        self.n = 0;
    }
    /// `heap.n`: number of entries.
    pub fn len(&self) -> usize {
        self.n
    }
    /// True when `heap.n` is 0.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    /// `heap.push(key, val)`.
    pub fn push(&mut self, key: f64, val: i32) {
        if self.n == self.k.len() {
            self.k.push(0.0);
            self.v.push(0);
        }
        let mut i = self.n;
        self.n += 1;
        while i > 0 {
            let p = (i - 1) >> 1;
            if self.k[p] <= key {
                break;
            }
            self.k[i] = self.k[p];
            self.v[i] = self.v[p];
            i = p;
        }
        self.k[i] = key;
        self.v[i] = val;
    }
    /// `heap.pop()`: removes the entry with the smallest key and returns its value; sets `top` to its key. The heap must not be empty.
    pub fn pop(&mut self) -> i32 {
        assert!(self.n > 0, "pop from an empty heap");
        let rv = self.v[0];
        self.top = self.k[0];
        self.n -= 1;
        let n = self.n;
        if n > 0 {
            let key = self.k[n];
            let val = self.v[n];
            let mut i = 0;
            loop {
                let mut c = 2 * i + 1;
                if c >= n {
                    break;
                }
                if c + 1 < n && self.k[c + 1] < self.k[c] {
                    c += 1;
                }
                if self.k[c] >= key {
                    break;
                }
                self.k[i] = self.k[c];
                self.v[i] = self.v[c];
                i = c;
            }
            self.k[i] = key;
            self.v[i] = val;
        }
        rv
    }
}

/// `U.clamp(x, a, b)`: x limited to [a, b]; NaN stays NaN.
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
pub fn r7(x: f64) -> f64 {
    math_round(x * 1e7) / 1e7
}

/// `U.r1(x)`: rounded to 1 decimal place (`Math.round(x * 10) / 10`).
pub fn r1(x: f64) -> f64 {
    math_round(x * 10.0) / 10.0
}

/// `U.r3(x)`: rounded to 3 decimal places (`Math.round(x * 1000) / 1000`).
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
    use super::*;

    /// The en dash the formats print for a missing or non-finite value.
    pub const DASH: &str = "\u{2013}";

    /// `U.fmt.num(x, d)`: grouped with commas, exactly d decimals; "–" for NaN and infinities.
    pub fn num(x: f64, d: usize) -> String {
        if !x.is_finite() {
            return DASH.to_string();
        }
        intl_format(x, d)
    }

    /// `U.fmt.int(x)`: `num(Math.round(x))`.
    pub fn int(x: f64) -> String {
        num(math_round(x), 0)
    }

    /// `U.fmt.pct(x, d)`: x as a percentage with d decimals ("0.25" gives "25%").
    pub fn pct(x: f64, d: usize) -> String {
        if x.is_finite() {
            num(x * 100.0, d) + "%"
        } else {
            DASH.to_string()
        }
    }

    /// `U.fmt.money(m)`: NZ$ millions as "$1.25b", "$12.5m", "$2.50m" or "$40k"; negatives with a minus sign (U+2212).
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
    pub fn signed(x: f64, d: usize) -> String {
        if !x.is_finite() {
            return DASH.to_string();
        }
        let r = string_to_number(&to_fixed(x, d));
        sign_of(r).to_string() + &num(r.abs(), d)
    }

    /// `U.fmt.signedPct(x, d)`: x as a signed percentage, "+4.5%".
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
                    format!("0{}", t)
                } else {
                    t
                }
            })
            .collect::<Vec<_>>()
            .join(":")
    }
}

/// `U.esc(s)`: HTML-escapes & < > " and ' (the JavaScript first turns null and undefined into "").
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
    let len = b.len();
    let digits_from = |mut i: usize| {
        while i < len && b[i].is_ascii_digit() {
            i += 1;
        }
        i
    };
    let mut i = 0;
    if i < len && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    if i < len && b[i].is_ascii_digit() {
        i = digits_from(i);
        if i < len && b[i] == b'.' {
            i = digits_from(i + 1);
        }
    } else if i < len && b[i] == b'.' {
        let start = i + 1;
        i = digits_from(start);
        if i == start {
            return false;
        }
    } else {
        return false;
    }
    if i < len && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < len && (b[i] == b'-' || b[i] == b'+') {
            i += 1;
        }
        let start = i;
        i = digits_from(start);
        if i == start {
            return false;
        }
    }
    i == len
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
pub fn csv(rows: &[Vec<Value>]) -> String {
    rows.iter()
        .map(|r| r.iter().map(csv_cell).collect::<Vec<_>>().join(","))
        .collect::<Vec<_>>()
        .join("\r\n")
}
