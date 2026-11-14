//! Shapes in local metres for the land-use build (landuse.js "geometry"): ring area and centroid, point in ring, joining the
//! member ways of a multipolygon into rings, and the small uniform grid that finds shapes near a point.

use crate::jsnum::{f64_to_i64, usize_to_f64};
use crate::util::{self};
use std::collections::{HashMap, HashSet};

/// A point in local metres (x east, y north).
pub type Pt = [f64; 2];

// Each point of a ring with the one after it, the last with the first (`r[i]`, `r[(i + 1) % n]` for i in order)
fn edges(r: &[Pt]) -> impl Iterator<Item = (&Pt, &Pt)> {
    r.iter().zip(r.iter().cycle().skip(1))
}

/// landuse.js `ringArea(r)`: the signed area (m2) of a ring by the shoelace formula.
#[must_use]
pub fn ring_area(r: &[Pt]) -> f64 {
    let mut area = 0.0;
    for (p, q) in edges(r) {
        area += p[0] * q[1] - q[0] * p[1];
    }
    area / 2.0
}

/// landuse.js `ringCentroid(r)`: the area centroid of a ring (the mean of its points when its area is under 1e-6).
#[must_use]
pub fn ring_centroid(r: &[Pt]) -> Pt {
    let (mut area, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for (p, q) in edges(r) {
        let cross = p[0] * q[1] - q[0] * p[1];
        area += cross;
        cx += (p[0] + q[0]) * cross;
        cy += (p[1] + q[1]) * cross;
    }
    if area.abs() < 1e-6 {
        cx = 0.0;
        cy = 0.0;
        for p in r {
            cx += p[0];
            cy += p[1];
        }
        return [cx / usize_to_f64(r.len()), cy / usize_to_f64(r.len())];
    }
    [cx / (3.0 * area), cy / (3.0 * area)]
}

/// landuse.js `inRing(x, y, r)`: whether a point is inside a ring (even-odd rule).
#[must_use]
pub fn in_ring(x: f64, y: f64, r: &[Pt]) -> bool {
    let mut inside = false;
    let Some(&last) = r.last() else {
        return inside;
    };
    // Each point against the one before it, the first against the last (j = n - 1, then j = i)
    let mut prev = last;
    for &cur in r {
        if (cur[1] > y) != (prev[1] > y)
            && x < (prev[0] - cur[0]) * (y - cur[1]) / (prev[1] - cur[1]) + cur[0]
        {
            inside = !inside;
        }
        prev = cur;
    }
    inside
}

fn same(p: &Pt, q: &Pt) -> bool {
    p[0] == q[0] && p[1] == q[1]
}

// The first point is the last (a closed ring). Rings here have two points or more; an empty one counts as closed.
fn is_closed(r: &[Pt]) -> bool {
    match (r.first(), r.last()) {
        (Some(p), Some(q)) => same(p, q),
        _ => true,
    }
}

/// landuse.js `assemble(parts)`: joins open member ways end to end into closed rings (multipolygon relations); keeps rings of three
/// points or more.
#[must_use]
pub fn assemble(parts: Vec<Vec<Pt>>) -> Vec<Vec<Pt>> {
    let mut rings: Vec<Vec<Pt>> = Vec::new();
    let mut open: Vec<Vec<Pt>> = Vec::new();
    for p in parts {
        if p.len() < 2 {
            continue;
        }
        if is_closed(&p) {
            rings.push(p);
        } else {
            open.push(p);
        }
    }
    while !open.is_empty() {
        let mut cur = open.remove(0);
        let mut grew = true;
        while grew && !is_closed(&cur) {
            grew = false;
            // cur is not closed, so it has a last point
            let Some(&end) = cur.last() else {
                break;
            };
            let mut joined: Option<usize> = None;
            for (k, o) in open.iter().enumerate() {
                if o.first().is_some_and(|first| same(&end, first)) {
                    cur.extend_from_slice(o.get(1..).unwrap_or_default());
                } else if let Some((last, rest)) = o.split_last()
                    && same(&end, last)
                {
                    cur.extend(rest.iter().rev());
                } else {
                    continue;
                }
                joined = Some(k);
                break;
            }
            if let Some(k) = joined {
                open.remove(k);
                grew = true;
            }
        }
        rings.push(cur);
    }
    rings.into_iter().filter(|r| r.len() >= 3).collect()
}

/// landuse.js `grid(cell, lim)`: a small uniform grid index over local coordinates. Values are indices into the caller's arrays.
pub struct Grid {
    cell: f64,
    lim: [f64; 4],
    m: HashMap<i64, Vec<usize>>,
}

fn key(i: f64, j: f64) -> i64 {
    f64_to_i64((i + 50000.0) * 100_000.0 + j + 50000.0)
}

impl Grid {
    /// A grid with this cell size (m) over the limits [x0, y0, x1, y1].
    #[must_use]
    pub fn new(cell: f64, lim: [f64; 4]) -> Grid {
        Grid {
            cell,
            lim,
            m: HashMap::new(),
        }
    }
    fn rng(&self, a: f64, b: f64, lo: f64, hi: f64) -> (f64, f64) {
        (
            crate::jsnum::math_max((a / self.cell).floor(), (lo / self.cell).floor()),
            crate::jsnum::math_min((b / self.cell).floor(), (hi / self.cell).floor()),
        )
    }
    /// `add(bb, v)`: files a value under every cell its bounding box [x0, y0, x1, y1] touches (within the limits).
    pub fn add(&mut self, bb: [f64; 4], v: usize) {
        let (i0, i1) = self.rng(bb[0], bb[2], self.lim[0], self.lim[2]);
        let (j0, j1) = self.rng(bb[1], bb[3], self.lim[1], self.lim[3]);
        let mut i = i0;
        while i <= i1 {
            let mut j = j0;
            while j <= j1 {
                self.m.entry(key(i, j)).or_default().push(v);
                j += 1.0;
            }
            i += 1.0;
        }
    }
    /// `at(x, y)`: the values filed under the cell holding the point.
    pub fn at(&self, x: f64, y: f64) -> &[usize] {
        self.m
            .get(&key((x / self.cell).floor(), (y / self.cell).floor()))
            .map_or(&[], Vec::as_slice)
    }
    /// `near(x, y, r)`: the distinct values filed under any cell within r of the point, in the order the cells are visited.
    #[must_use]
    pub fn near(&self, x: f64, y: f64, r: f64) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        let mut seen: HashSet<usize> = HashSet::new();
        let mut ci = ((x - r) / self.cell).floor();
        let i1 = ((x + r) / self.cell).floor();
        while ci <= i1 {
            let mut cj = ((y - r) / self.cell).floor();
            let j1 = ((y + r) / self.cell).floor();
            while cj <= j1 {
                if let Some(vals) = self.m.get(&key(ci, cj)) {
                    for &val in vals {
                        if seen.insert(val) {
                            out.push(val);
                        }
                    }
                }
                cj += 1.0;
            }
            ci += 1.0;
        }
        out
    }
}

/// A building, land-use area or site outline: outer rings, inner rings (holes), bounding box, area (m2) and a point inside.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// Outer rings.
    pub outer: Vec<Vec<Pt>>,
    /// Inner rings.
    pub inner: Vec<Vec<Pt>>,
    /// Bounding box [x0, y0, x1, y1] of the outer rings.
    pub bb: [f64; 4],
    /// Area (m2): the outer rings less the holes.
    pub area: f64,
    /// Centroid of the biggest outer ring (or its mean point when the centroid falls outside it): x.
    pub cx: f64,
    /// The same, y.
    pub cy: f64,
}

impl Shape {
    /// landuse.js `contains(s, x, y)`: inside an outer ring and outside every hole.
    #[must_use]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.bb[0]
            && x <= self.bb[2]
            && y >= self.bb[1]
            && y <= self.bb[3]
            && self.outer.iter().any(|r| in_ring(x, y, r))
            && !self.inner.iter().any(|r| in_ring(x, y, r))
    }
    /// landuse.js `edge(s, x, y)`: the distance (m) from a point to the nearest edge of the outer rings.
    #[must_use]
    pub fn edge(&self, x: f64, y: f64) -> f64 {
        let mut dist = f64::INFINITY;
        for ring in &self.outer {
            for (p, q) in edges(ring) {
                dist = crate::jsnum::math_min(dist, util::seg_dist(x, y, p[0], p[1], q[0], q[1]).d);
            }
        }
        dist
    }
}
