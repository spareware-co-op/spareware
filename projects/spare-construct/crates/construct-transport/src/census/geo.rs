//! Study boxes, polygons and the small helpers of the land use feature (landuse-plus.js "small helpers" and "geometry").

use crate::jsnum::{
    JsNum, Maybe, math_max, math_max_of, math_min, math_min_of, math_round, string_to_number,
    to_fixed, value_to_js_string, value_to_number, value_truthy,
};
use crate::project::{Project, Scenario};
use crate::util::{self, fmt};
use serde_json::Value;

/// A study box: south and north latitudes, west and east longitudes, with where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct Bx {
    /// South edge (latitude).
    pub s: f64,
    /// West edge (longitude).
    pub w: f64,
    /// North edge (latitude).
    pub n: f64,
    /// East edge (longitude).
    pub e: f64,
    /// What the box is taken from ("the study area"); None when it is just a box.
    pub basis: Option<String>,
}

impl Bx {
    /// A box with no basis.
    pub fn new(s: f64, w: f64, n: f64, e: f64) -> Bx {
        Bx {
            s,
            w,
            n,
            e,
            basis: None,
        }
    }
    /// The same box with a basis.
    pub fn with_basis(mut self, basis: &str) -> Bx {
        self.basis = Some(basis.to_string());
        self
    }
    /// The box as JSON, `{ s, w, n, e, basis }` (the basis only when there is one).
    pub fn to_value(&self) -> Value {
        let mut o = serde_json::Map::new();
        for (k, v) in [("s", self.s), ("w", self.w), ("n", self.n), ("e", self.e)] {
            o.insert(k.into(), crate::jsnum::json_number(v));
        }
        if let Some(b) = &self.basis {
            o.insert("basis".into(), Value::String(b.clone()));
        }
        Value::Object(o)
    }
}

/// `isFinite(x)` of a loose value (`Number(x)` is finite).
pub fn finite_value(v: Option<&Value>) -> bool {
    v.is_some_and(|v| value_to_number(v).is_finite())
}

/// `box(b)`: a box from `[[s, w], [n, e]]` (corners in any order), `{ south, west, north, east }` or `{ s, w, n, e }`; None for anything else.
pub fn box_of(b: &Value) -> Option<Bx> {
    if !value_truthy(b) {
        return None;
    }
    let num = |v: &Value| value_to_number(v);
    if let Value::Array(a) = b {
        let at = |i: usize, j: usize| a.get(i).and_then(|x| x.get(j)).map_or(f64::NAN, num);
        return Some(Bx::new(
            math_min(at(0, 0), at(1, 0)),
            math_min(at(0, 1), at(1, 1)),
            math_max(at(0, 0), at(1, 0)),
            math_max(at(0, 1), at(1, 1)),
        ));
    }
    let key = |k: &str| b.get(k).map_or(f64::NAN, num);
    if b.get("south").is_some() {
        return Some(Bx::new(
            key("south"),
            key("west"),
            key("north"),
            key("east"),
        ));
    }
    if b.get("s").is_some() && ["s", "w", "n", "e"].iter().all(|k| finite_value(b.get(*k))) {
        return Some(Bx::new(key("s"), key("w"), key("n"), key("e")));
    }
    None
}

/// `areaBox(sc)`: the study area, in this order: the box drawn in onboarding, the extent of the scenario's network and zones, the box of the last fill. None when there is none.
pub fn area_box(p: &Project, sc: &Scenario) -> Option<Bx> {
    if let Some(drawn) = p
        .extra
        .get("onboarding")
        .and_then(|o| o.get("area"))
        .and_then(box_of)
    {
        return Some(drawn.with_basis("the study area"));
    }
    let mut lats: Vec<f64> = Vec::new();
    let mut lngs: Vec<f64> = Vec::new();
    for (lat, lng) in sc
        .nodes
        .values()
        .map(|n| (n.lat.num(), n.lng.num()))
        .chain(sc.zones.values().map(|z| (z.lat.num(), z.lng.num())))
    {
        if lat.is_finite() && lng.is_finite() {
            lats.push(lat);
            lngs.push(lng);
        }
    }
    if !lats.is_empty() {
        return Some(Bx {
            s: math_min_of(&lats),
            n: math_max_of(&lats),
            w: math_min_of(&lngs),
            e: math_max_of(&lngs),
            basis: Some("the extent of the network and zones".into()),
        });
    }
    p.extra
        .get("landuse")
        .and_then(|l| l.get("bounds"))
        .and_then(box_of)
        .map(|b| b.with_basis("the area of the last fill"))
}

/// `boxText(B)`: the size of a box in kilometres, "1.4 \u{d7} 1.1 km".
pub fn box_text(b: &Bx) -> String {
    let f = util::flat((b.s + b.n) / 2.0);
    format!(
        "{} \u{d7} {} km",
        fmt::num((b.e - b.w) * f.kx / 1000.0, 1),
        fmt::num((b.n - b.s) * f.ky / 1000.0, 1)
    )
}

/// `inBox(B, lat, lng)`.
pub fn in_box(b: &Bx, lat: f64, lng: f64) -> bool {
    lat >= b.s && lat <= b.n && lng >= b.w && lng <= b.e
}

/// `boxKey(B)`: the box as text to four decimals, the cache key of a download.
pub fn box_key(b: &Bx) -> String {
    [b.s, b.w, b.n, b.e]
        .iter()
        .map(|v| to_fixed(*v, 4))
        .collect::<Vec<_>>()
        .join(",")
}

/// `parseNum(v)`: '12,000', '5000 m2', '5 000 m\u{b2}' and '250 sqm' become numbers; '' gives None; anything else NaN (Some).
///
/// A number is returned as it is when finite and NaN otherwise.
pub fn parse_num(v: &Value) -> Option<f64> {
    if let Value::Number(n) = v {
        let x = n.as_f64().unwrap_or(f64::NAN);
        return Some(if x.is_finite() { x } else { f64::NAN });
    }
    parse_num_text(&if v.is_null() {
        String::new()
    } else {
        value_to_js_string(v)
    })
}

/// `parseNum` of text.
pub fn parse_num_text(text: &str) -> Option<f64> {
    // .replace(/m²|m2|sqm/gi, '') then .replace(/[\s, ]/g, '')
    let low = text.to_lowercase();
    let mut kept = String::new();
    let chars: Vec<char> = text.chars().collect();
    let lowc: Vec<char> = low.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let rest: String = lowc[i..].iter().take(3).collect();
        if lowc.len() == chars.len() && (rest.starts_with("m\u{b2}") || rest.starts_with("m2")) {
            i += 2;
            continue;
        }
        if lowc.len() == chars.len() && rest.starts_with("sqm") {
            i += 3;
            continue;
        }
        kept.push(chars[i]);
        i += 1;
    }
    let s: String = kept
        .chars()
        .filter(|c| !(crate::jsnum::is_js_space(*c) || *c == ',' || *c == ' '))
        .collect();
    if s.is_empty() {
        return None;
    }
    if numlike(&s) {
        Some(string_to_number(&s))
    } else {
        Some(f64::NAN)
    }
}

// /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i
fn numlike(s: &str) -> bool {
    let b = s.as_bytes();
    let len = b.len();
    let digits = |mut i: usize| {
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
        i = digits(i);
        if i < len && b[i] == b'.' {
            i = digits(i + 1);
        }
    } else if i < len && b[i] == b'.' {
        let start = i + 1;
        i = digits(start);
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
        i = digits(start);
        if i == start {
            return false;
        }
    }
    i == len
}

/// `lr(total, weights)`: largest-remainder split of an integer total by weights (it sums exactly).
pub fn lr(total: f64, weights: &[f64]) -> Vec<f64> {
    let s = weights.iter().fold(0.0, |a, b| a + b);
    if !(s > 0.0) || !(total > 0.0) {
        return weights.iter().map(|_| 0.0).collect();
    }
    let raw: Vec<f64> = weights.iter().map(|w| total * w / s).collect();
    let mut out: Vec<f64> = raw.iter().map(|r| r.floor()).collect();
    let mut left = total - out.iter().fold(0.0, |a, b| a + b);
    let mut rem: Vec<(f64, usize)> = raw
        .iter()
        .enumerate()
        .map(|(i, r)| (r - out[i], i))
        .collect();
    crate::graph::js_sort_by(&mut rem, |a, b| {
        let d = b.0 - a.0;
        if d != 0.0 && !d.is_nan() {
            d
        } else {
            a.1 as f64 - b.1 as f64
        }
    });
    for (_, i) in rem {
        if left > 0.0 {
            out[i] += 1.0;
            left -= 1.0;
        }
    }
    out
}

/// A ring of a polygon: [lng, lat] points (ArcGIS order).
pub type Ring = Vec<[f64; 2]>;

/// A polygon with its bounding box [west, south, east, north].
#[derive(Clone, Debug, PartialEq)]
pub struct Poly {
    /// Rings, each [lng, lat] points.
    pub rings: Vec<Ring>,
    /// Bounding box [west, south, east, north].
    pub bb: [f64; 4],
}

impl Poly {
    /// A polygon from rings, with its bounding box.
    pub fn new(rings: Vec<Ring>) -> Poly {
        let bb = bb_of(&rings);
        Poly { rings, bb }
    }
}

/// `bbOf(rings)`: [west, south, east, north] over every point.
pub fn bb_of(rings: &[Ring]) -> [f64; 4] {
    let mut bb = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for r in rings {
        for p in r {
            bb[0] = math_min(bb[0], p[0]);
            bb[1] = math_min(bb[1], p[1]);
            bb[2] = math_max(bb[2], p[0]);
            bb[3] = math_max(bb[3], p[1]);
        }
    }
    bb
}

/// `inRings(x, y, rings)`: even-odd over every ring, so holes work.
pub fn in_rings(x: f64, y: f64, rings: &[Ring]) -> bool {
    let mut c = false;
    for r in rings {
        let n = r.len();
        if n == 0 {
            continue;
        }
        let mut j = n - 1;
        for i in 0..n {
            let (xi, yi, xj, yj) = (r[i][0], r[i][1], r[j][0], r[j][1]);
            if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                c = !c;
            }
            j = i;
        }
    }
    c
}

/// `inPoly(f, lng, lat)`: the point is inside the polygon (bounding box first).
pub fn in_poly(f: &Poly, lng: f64, lat: f64) -> bool {
    lng >= f.bb[0]
        && lng <= f.bb[2]
        && lat >= f.bb[1]
        && lat <= f.bb[3]
        && in_rings(lng, lat, &f.rings)
}

/// `bbInBox(bb, B)`: the bounding box lies wholly inside the study box.
pub fn bb_in_box(bb: &[f64; 4], b: &Bx) -> bool {
    bb[0] >= b.w && bb[2] <= b.e && bb[1] >= b.s && bb[3] <= b.n
}

/// `bbMeets(bb, B)`: the bounding box touches the study box.
pub fn bb_meets(bb: &[f64; 4], b: &Bx) -> bool {
    bb[0] <= b.e && bb[2] >= b.w && bb[1] <= b.n && bb[3] >= b.s
}

/// `ringsArea(rings)`: area in m\u{b2}, holes subtracted (ArcGIS winds holes the other way).
pub fn rings_area(rings: &[Ring]) -> f64 {
    if rings.is_empty() {
        return 0.0;
    }
    let f = util::flat(rings[0][0][1]);
    let mut a = 0.0;
    for r in rings {
        let n = r.len();
        for i in 0..n {
            let (p, q) = (r[i], r[(i + 1) % n]);
            a += (p[0] * f.kx) * (q[1] * f.ky) - (q[0] * f.kx) * (p[1] * f.ky);
        }
    }
    (a / 2.0).abs()
}

/// What `sampleShare` finds: the share of a polygon in a box, the sample points inside both and how many samples fell inside the polygon.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    /// Share of the polygon's area inside the box.
    pub share: f64,
    /// Sample points [lng, lat] inside both.
    pub pts: Vec<[f64; 2]>,
    /// Sample points inside the polygon.
    pub inside: usize,
}

/// `sampleShare(f, B, n)`: the sampled share of a polygon's area inside box B, and the sample points inside both.
pub fn sample_share(f: &Poly, b: &Bx, n: usize) -> Sample {
    let [w, s, e, nn] = f.bb;
    let mut pts = Vec::new();
    let mut inside = 0usize;
    let nf = n as f64;
    for j in 0..n {
        for i in 0..n {
            let x = w + (e - w) * (i as f64 + 0.5) / nf;
            let y = s + (nn - s) * (j as f64 + 0.5) / nf;
            if !in_rings(x, y, &f.rings) {
                continue;
            }
            inside += 1;
            if in_box(b, y, x) {
                pts.push([x, y]);
            }
        }
    }
    let share = if inside > 0 {
        pts.len() as f64 / inside as f64
    } else if bb_meets(&f.bb, b) {
        1.0
    } else {
        0.0
    };
    Sample { share, pts, inside }
}

/// `ringsOf(g)`: the rings of an ArcGIS geometry that have more than two points, each point as [lng, lat].
pub fn rings_of(g: Option<&Value>) -> Vec<Ring> {
    let Some(Value::Array(rs)) = g.and_then(|g| g.get("rings")) else {
        return Vec::new();
    };
    rs.iter()
        .filter(|r| r.as_array().is_some_and(|a| a.len() > 2))
        .map(|r| {
            r.as_array()
                .expect("a ring")
                .iter()
                .map(|p| {
                    [
                        p.get(0).map_or(f64::NAN, value_to_number),
                        p.get(1).map_or(f64::NAN, value_to_number),
                    ]
                })
                .collect()
        })
        .collect()
}

/// `n1(x)`: one decimal, "\u{2013}" for a missing value.
pub fn n1(x: Option<f64>) -> String {
    match x {
        Some(v) if v.is_finite() => fmt::num(v, 1),
        _ => fmt::DASH.to_string(),
    }
}

/// `n0(x)`: a whole number, "\u{2013}" for a missing value.
pub fn n0(x: Option<f64>) -> String {
    match x {
        Some(v) if v.is_finite() => fmt::int(v),
        _ => fmt::DASH.to_string(),
    }
}

/// `r2(x)`: rounded to two decimals.
pub fn r2(x: f64) -> f64 {
    math_round(x * 100.0) / 100.0
}

/// A loose scalar as a number: `+x` (null 0, missing NaN).
pub fn plus(v: Option<&Value>) -> f64 {
    v.map_or(f64::NAN, value_to_number)
}

/// `Num` helper: a number field set to a number.
pub fn set_num(x: f64) -> crate::jsnum::Num {
    Maybe::Set(JsNum::Number(x))
}
