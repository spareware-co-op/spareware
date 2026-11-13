//! Distances between a position, a road and the map layers (packs/nz/compliance.js "geometry"): the nearest road, rings and lines in
//! flat coordinates, and the conflation of map lines onto model links.

use super::jv::{Jx, V};
use super::net;
use crate::jsnum::math_min;
use crate::util::{self, Flat, LatLng};

/// `flatAt(lat)`: the flat projection around a latitude.
#[must_use]
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
#[must_use]
pub fn to_line(p: &LatLng, pts: &[LatLng], f: &Flat) -> ToLine {
    let q = xy(f, p);
    let mut best = ToLine { d: f64::INFINITY, b: None };
    for seg in pts.windows(2) {
        let [start, end] = seg else { continue };
        let (s0, s1) = (xy(f, start), xy(f, end));
        let hit = util::seg_dist(q[0], q[1], s0[0], s0[1], s1[0], s1[1]);
        if hit.d < best.d {
            best = ToLine { d: hit.d, b: Some(util::bearing(start, end)) };
        }
    }
    best
}

/// `inRing(ll, ring)`: a point is inside a ring of `[lat, lng]` points (even-odd rule).
#[must_use]
pub fn in_ring(ll: &LatLng, ring: &[LatLng]) -> bool {
    let mut inside = false;
    // Each point is paired with the one before it, the first with the last
    let Some(mut prev) = ring.last() else { return false };
    for cur in ring {
        let (yi, xi) = (cur[0], cur[1]);
        let (yj, xj) = (prev[0], prev[1]);
        let den = yj - yi;
        let den = if den == 0.0 || den.is_nan() { 1e-12 } else { den };
        if ((yi > ll[0]) != (yj > ll[0])) && ll[1] < (xj - xi) * (ll[0] - yi) / den + xi {
            inside = !inside;
        }
        prev = cur;
    }
    inside
}

/// `toPolys(P, rings, f)`: the distance from a point to the nearest of several rings, 0 inside one.
#[must_use]
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
#[must_use]
pub fn line_to_line(a: &[LatLng], b: &[LatLng], f: &Flat) -> f64 {
    let line_a: Vec<[f64; 2]> = a.iter().map(|ll| xy(f, ll)).collect();
    let line_b: Vec<[f64; 2]> = b.iter().map(|ll| xy(f, ll)).collect();
    let orient = |p0: [f64; 2], p1: [f64; 2], p2: [f64; 2]| (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p1[1] - p0[1]) * (p2[0] - p0[0]);
    let pt = |at: [f64; 2], s0: [f64; 2], s1: [f64; 2]| util::seg_dist(at[0], at[1], s0[0], s0[1], s1[0], s1[1]).d;
    if let ([one], other) | (other, [one]) = (line_a.as_slice(), line_b.as_slice()) {
        let one = *one;
        let mut dist = f64::INFINITY;
        for seg in other.windows(2) {
            if let &[s0, s1] = seg {
                dist = math_min(dist, pt(one, s0, s1));
            }
        }
        return if let [only] = other { crate::jsnum::math_hypot(&[one[0] - only[0], one[1] - only[1]]) } else { dist };
    }
    let mut dist = f64::INFINITY;
    for seg_a in line_a.windows(2) {
        let &[p1, p2] = seg_a else { continue };
        for seg_b in line_b.windows(2) {
            let &[q1, q2] = seg_b else { continue };
            if (orient(q1, q2, p1) > 0.0) != (orient(q1, q2, p2) > 0.0) && (orient(p1, p2, q1) > 0.0) != (orient(p1, p2, q2) > 0.0) {
                return 0.0;
            }
            dist = math_min(dist, math_min(math_min(pt(p1, q1, q2), pt(p2, q1, q2)), math_min(pt(q1, p1, p2), pt(q2, p1, p2))));
        }
    }
    dist
}

/// The bounding box of a polyline: (south, north, west, east).
#[must_use]
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
#[must_use]
pub fn line_to_polys(pts: &[LatLng], shapes: &[Vec<LatLng>], f: &Flat, within: f64, open: bool) -> f64 {
    let (ls, ln, lw, le) = box_of(pts);
    let pad = within.min(1e5) / 111_320.0;
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
        let closed: Vec<LatLng> = match (r.first(), r.last()) {
            (Some(first), Some(last)) if !open && r.len() > 2 && (first[0] != last[0] || first[1] != last[1]) => {
                let mut c = r.clone();
                c.push(*first);
                c
            }
            _ => r.clone(),
        };
        d = math_min(d, line_to_line(pts, &closed, f));
    }
    d
}

/// `boundsOf(sc)`: the extent of the scenario's junctions as `{ s, w, n, e }`, None without any.
#[must_use]
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
#[must_use]
pub fn conflate(sc: &V, lines: &[(V, Vec<LatLng>)], radius: f64, bearing: f64) -> Vec<(String, V)> {
    let mut out = Vec::new();
    let Some(bounds) = bounds_of(sc) else { return out };
    if lines.is_empty() {
        return out;
    }
    let (bs, bw, bn, be) = bounds;
    let mid = f64::midpoint(bs, bn);
    let flat = flat_at(mid);
    let pad_lat = 200.0 / 111_320.0;
    let pad_lng = 200.0 / (111_320.0 * crate::jsmath::cos(mid * std::f64::consts::PI / 180.0));
    let near: Vec<(&V, Vec<LatLng>)> = lines
        .iter()
        .map(|(tag, pts)| (tag, pts.iter().filter(|p| p[0] > bs - pad_lat && p[0] < bn + pad_lat && p[1] > bw - pad_lng && p[1] < be + pad_lng).copied().collect::<Vec<_>>()))
        .filter(|(_, pts)| pts.len() > 1)
        .collect();
    if near.is_empty() {
        return out;
    }
    for link in sc.g("links").vals() {
        if !sc.g("nodes").has(&link.g("a").s()) || !sc.g("nodes").has(&link.g("b").s()) {
            continue;
        }
        let Some(geo) = net::geom(sc, link) else { continue };
        let cum = util::cum_len(&geo);
        // cum_len always gives at least one entry; NaN (never) would skip the link as a zero length does
        let len = cum.last().copied().unwrap_or(f64::NAN);
        if len <= 0.0 || len.is_nan() {
            continue;
        }
        let mut best: Option<&V> = None;
        for (tag, pts) in &near {
            // Bound: at most one hit for each of the three points
            let mut hits = 0u32;
            for frac in [0.25, 0.5, 0.75] {
                let at = util::along(&geo, &cum, len * frac);
                let seg = at.i.max(0).unsigned_abs();
                // A line of positive length has at least two points, so the segment's ends are there; NaN (never) gives no hit
                let lb = match (geo.get(seg), geo.get(seg.saturating_add(1))) {
                    (Some(p0), Some(p1)) => util::bearing(p0, p1),
                    _ => f64::NAN,
                };
                let hit = to_line(&at.p, pts, &flat);
                let rb = hit.b.unwrap_or(f64::NAN);
                if hit.d <= radius && (util::ang_diff(lb, rb) <= bearing || util::ang_diff(lb, rb) >= 180.0 - bearing) {
                    hits = hits.saturating_add(1);
                }
            }
            if hits >= 2 {
                best = Some(tag);
                break;
            }
        }
        if let Some(tag) = best {
            out.push((link.g("id").s(), tag.clone()));
        }
    }
    out
}

/// `paths(g)`: the lines or rings of an `ArcGIS` geometry as `[lat, lng]` points rounded to 7 places.
#[must_use]
pub fn paths(g: &V) -> Vec<Vec<LatLng>> {
    let src = if g.g("paths").t() { g.g("paths") } else { g.g("rings") };
    src.items().iter().map(|p| p.items().iter().map(|q| [util::r7(q.ix(1).n()), util::r7(q.ix(0).n())]).collect()).collect()
}
