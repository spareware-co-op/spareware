//! Distances between a position, a road and the map layers (packs/nz/compliance.js "geometry"): the nearest road, rings and lines in
//! flat coordinates, and the conflation of map lines onto model links.

use super::jv::{Jx, V};
use super::net;
use crate::jsnum::math_min;
use crate::util::{self, Flat, LatLng};

/// `flatAt(lat)`: the flat projection around a latitude.
pub fn flat_at(lat: f64) -> Flat {
    util::flat(lat)
}

fn xy(f: &Flat, ll: &LatLng) -> [f64; 2] {
    [f.x(ll), f.y(ll)]
}

/// The result of [`to_line`]: the distance and the bearing of the nearest segment (None when there is no segment).
#[derive(Clone, Copy, Debug)]
pub struct ToLine {
    /// Distance (m), infinity without a segment.
    pub d: f64,
    /// Bearing of the nearest segment.
    pub b: Option<f64>,
}

/// `toLine(P, pts, f)`: the distance (m) from a point to a polyline, with the bearing of the nearest segment.
pub fn to_line(p: &LatLng, pts: &[LatLng], f: &Flat) -> ToLine {
    let q = xy(f, p);
    let mut best = ToLine { d: f64::INFINITY, b: None };
    for i in 1..pts.len() {
        let (a, c) = (xy(f, &pts[i - 1]), xy(f, &pts[i]));
        let r = util::seg_dist(q[0], q[1], a[0], a[1], c[0], c[1]);
        if r.d < best.d {
            best = ToLine { d: r.d, b: Some(util::bearing(&pts[i - 1], &pts[i])) };
        }
    }
    best
}

/// `inRing(ll, ring)`: a point is inside a ring of `[lat, lng]` points (even-odd rule).
pub fn in_ring(ll: &LatLng, ring: &[LatLng]) -> bool {
    let mut inside = false;
    let n = ring.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (yi, xi) = (ring[i][0], ring[i][1]);
        let (yj, xj) = (ring[j][0], ring[j][1]);
        let den = yj - yi;
        let den = if den == 0.0 || den.is_nan() { 1e-12 } else { den };
        if ((yi > ll[0]) != (yj > ll[0])) && ll[1] < (xj - xi) * (ll[0] - yi) / den + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// `toPolys(P, rings, f)`: the distance from a point to the nearest of several rings, 0 inside one.
pub fn to_polys(p: &LatLng, rings: &[Vec<LatLng>], f: &Flat) -> f64 {
    let mut d = f64::INFINITY;
    for r in rings {
        if in_ring(p, r) {
            return 0.0;
        }
        d = math_min(d, to_line(p, r, f).d);
    }
    d
}

/// `lineToLine(A, B, f)`: the shortest distance (m) between two polylines (0 where they cross).
pub fn line_to_line(a: &[LatLng], b: &[LatLng], f: &Flat) -> f64 {
    let a: Vec<[f64; 2]> = a.iter().map(|p| xy(f, p)).collect();
    let b: Vec<[f64; 2]> = b.iter().map(|p| xy(f, p)).collect();
    let o = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    let pt = |p: [f64; 2], s: [f64; 2], e: [f64; 2]| util::seg_dist(p[0], p[1], s[0], s[1], e[0], e[1]).d;
    if a.len() == 1 || b.len() == 1 {
        let (p, l) = if a.len() == 1 { (a[0], &b) } else { (b[0], &a) };
        let mut d = f64::INFINITY;
        for i in 1..l.len() {
            d = math_min(d, pt(p, l[i - 1], l[i]));
        }
        return if l.len() == 1 { crate::jsnum::math_hypot(&[p[0] - l[0][0], p[1] - l[0][1]]) } else { d };
    }
    let mut d = f64::INFINITY;
    for i in 1..a.len() {
        for j in 1..b.len() {
            let (p1, p2, q1, q2) = (a[i - 1], a[i], b[j - 1], b[j]);
            if (o(q1, q2, p1) > 0.0) != (o(q1, q2, p2) > 0.0) && (o(p1, p2, q1) > 0.0) != (o(p1, p2, q2) > 0.0) {
                return 0.0;
            }
            d = math_min(d, math_min(math_min(pt(p1, q1, q2), pt(p2, q1, q2)), math_min(pt(q1, p1, p2), pt(q2, p1, p2))));
        }
    }
    d
}

/// The bounding box of a polyline: (south, north, west, east).
pub fn box_of(pts: &[LatLng]) -> (f64, f64, f64, f64) {
    let (mut s, mut n, mut w, mut e) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for p in pts {
        s = math_min(s, p[0]);
        n = crate::jsnum::math_max(n, p[0]);
        w = math_min(w, p[1]);
        e = crate::jsnum::math_max(e, p[1]);
    }
    (s, n, w, e)
}

/// `lineToPolys(pts, shapes, f, within, open)`: the shortest distance (m) from a polyline to rings (0 when a vertex is inside one or the
/// line crosses an edge), or to open lines when `open` is true. Shapes further than `within` m (by bounding box) are skipped.
pub fn line_to_polys(pts: &[LatLng], shapes: &[Vec<LatLng>], f: &Flat, within: f64, open: bool) -> f64 {
    let (ls, ln, lw, le) = box_of(pts);
    let pad = within.min(1e5) / 111320.0;
    let pad_lng = pad / crate::jsnum::math_max(0.2, crate::jsmath::cos(ls * std::f64::consts::PI / 180.0));
    let mut d = f64::INFINITY;
    for r in shapes {
        if r.is_empty() {
            continue;
        }
        let (rs, rn, rw, re) = box_of(r);
        if within.is_finite() && (rs > ln + pad || rn < ls - pad || rw > le + pad_lng || re < lw - pad_lng) {
            continue;
        }
        if !open && pts.iter().any(|p| in_ring(p, r)) {
            return 0.0;
        }
        let closed: Vec<LatLng> = if !open && r.len() > 2 && (r[0][0] != r[r.len() - 1][0] || r[0][1] != r[r.len() - 1][1]) {
            let mut c = r.clone();
            c.push(r[0]);
            c
        } else {
            r.clone()
        };
        d = math_min(d, line_to_line(pts, &closed, f));
    }
    d
}

/// `boundsOf(sc)`: the extent of the scenario's junctions as `{ s, w, n, e }`, None without any.
pub fn bounds_of(sc: &V) -> Option<(f64, f64, f64, f64)> {
    let (mut s, mut w, mut n, mut e) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for nd in sc.g("nodes").vals() {
        let (lat, lng) = (nd.g("lat").n(), nd.g("lng").n());
        s = math_min(s, lat);
        n = crate::jsnum::math_max(n, lat);
        w = math_min(w, lng);
        e = crate::jsnum::math_max(e, lng);
    }
    if s.is_finite() { Some((s, w, n, e)) } else { None }
}

/// `conflate(sc, lines, { radius, bearing })`: map polylines conflated to model links. A link matches a line when 2 of its 3 points
/// (a quarter, half and three quarters along) are within `radius` m of the line with the line's bearing within `bearing` degrees (either
/// direction). `lines` are (tag, points); the answer is (link id, tag) in link order.
pub fn conflate(sc: &V, lines: &[(V, Vec<LatLng>)], radius: f64, bearing: f64) -> Vec<(String, V)> {
    let mut out = Vec::new();
    let Some(b) = bounds_of(sc) else { return out };
    if lines.is_empty() {
        return out;
    }
    let (bs, bw, bn, be) = b;
    let mid = (bs + bn) / 2.0;
    let f = flat_at(mid);
    let pad_lat = 200.0 / 111320.0;
    let pad_lng = 200.0 / (111320.0 * crate::jsmath::cos(mid * std::f64::consts::PI / 180.0));
    let near: Vec<(&V, Vec<LatLng>)> = lines
        .iter()
        .map(|(tag, pts)| (tag, pts.iter().filter(|p| p[0] > bs - pad_lat && p[0] < bn + pad_lat && p[1] > bw - pad_lng && p[1] < be + pad_lng).copied().collect::<Vec<_>>()))
        .filter(|(_, pts)| pts.len() > 1)
        .collect();
    if near.is_empty() {
        return out;
    }
    for l in sc.g("links").vals() {
        if !sc.g("nodes").has(&l.g("a").s()) || !sc.g("nodes").has(&l.g("b").s()) {
            continue;
        }
        let Some(g) = net::geom(sc, l) else { continue };
        let cum = util::cum_len(&g);
        let len = cum[cum.len() - 1];
        if !(len > 0.0) {
            continue;
        }
        let mut best: Option<&V> = None;
        for (tag, pts) in &near {
            let mut hits = 0;
            for s in [0.25, 0.5, 0.75] {
                let a = util::along(&g, &cum, len * s);
                let i = a.i.max(0) as usize;
                let lb = util::bearing(&g[i], &g[i + 1]);
                let r = to_line(&a.p, pts, &f);
                let rb = r.b.unwrap_or(f64::NAN);
                if r.d <= radius && (util::ang_diff(lb, rb) <= bearing || util::ang_diff(lb, rb) >= 180.0 - bearing) {
                    hits += 1;
                }
            }
            if hits >= 2 {
                best = Some(tag);
                break;
            }
        }
        if let Some(tag) = best {
            out.push((l.g("id").s(), tag.clone()));
        }
    }
    out
}

/// `paths(g)`: the lines or rings of an ArcGIS geometry as `[lat, lng]` points rounded to 7 places.
pub fn paths(g: &V) -> Vec<Vec<LatLng>> {
    let src = if g.g("paths").t() { g.g("paths") } else { g.g("rings") };
    src.items().iter().map(|p| p.items().iter().map(|q| [util::r7(q.ix(1).n()), util::r7(q.ix(0).n())]).collect()).collect()
}
