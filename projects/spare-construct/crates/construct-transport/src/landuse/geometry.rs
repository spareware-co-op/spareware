//! Shapes in local metres for the land-use build (landuse.js "geometry"): ring area and centroid, point in ring, joining the
//! member ways of a multipolygon into rings, and the small uniform grid that finds shapes near a point.

use crate::util::{self};
use std::collections::{HashMap, HashSet};

/// A point in local metres (x east, y north).
pub type Pt = [f64; 2];

/// landuse.js `ringArea(r)`: the signed area (m2) of a ring by the shoelace formula.
pub fn ring_area(r: &[Pt]) -> f64 {
    let mut a = 0.0;
    let n = r.len();
    for i in 0..n {
        let p = r[i];
        let q = r[(i + 1) % n];
        a += p[0] * q[1] - q[0] * p[1];
    }
    a / 2.0
}

/// landuse.js `ringCentroid(r)`: the area centroid of a ring (the mean of its points when its area is under 1e-6).
pub fn ring_centroid(r: &[Pt]) -> Pt {
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    let n = r.len();
    for i in 0..n {
        let p = r[i];
        let q = r[(i + 1) % n];
        let c = p[0] * q[1] - q[0] * p[1];
        a += c;
        cx += (p[0] + q[0]) * c;
        cy += (p[1] + q[1]) * c;
    }
    if a.abs() < 1e-6 {
        cx = 0.0;
        cy = 0.0;
        for p in r {
            cx += p[0];
            cy += p[1];
        }
        return [cx / r.len() as f64, cy / r.len() as f64];
    }
    [cx / (3.0 * a), cy / (3.0 * a)]
}

/// landuse.js `inRing(x, y, r)`: whether a point is inside a ring (even-odd rule).
pub fn in_ring(x: f64, y: f64, r: &[Pt]) -> bool {
    let mut c = false;
    if r.is_empty() {
        return c;
    }
    let mut j = r.len() - 1;
    for i in 0..r.len() {
        let a = r[i];
        let b = r[j];
        if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
        j = i;
    }
    c
}

fn same(p: &Pt, q: &Pt) -> bool {
    p[0] == q[0] && p[1] == q[1]
}

/// landuse.js `assemble(parts)`: joins open member ways end to end into closed rings (multipolygon relations); keeps rings of three
/// points or more.
pub fn assemble(parts: Vec<Vec<Pt>>) -> Vec<Vec<Pt>> {
    let mut rings: Vec<Vec<Pt>> = Vec::new();
    let mut open: Vec<Vec<Pt>> = Vec::new();
    for p in parts {
        if p.len() < 2 {
            continue;
        }
        if same(&p[0], &p[p.len() - 1]) {
            rings.push(p);
        } else {
            open.push(p);
        }
    }
    while !open.is_empty() {
        let mut cur = open.remove(0);
        let mut grew = true;
        while grew && !same(&cur[0], &cur[cur.len() - 1]) {
            grew = false;
            for k in 0..open.len() {
                let end = cur[cur.len() - 1];
                let o = &open[k];
                if same(&end, &o[0]) {
                    cur.extend_from_slice(&o[1..]);
                } else if same(&end, &o[o.len() - 1]) {
                    let mut rev: Vec<Pt> = o[..o.len() - 1].to_vec();
                    rev.reverse();
                    cur.extend(rev);
                } else {
                    continue;
                }
                open.remove(k);
                grew = true;
                break;
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
    ((i + 50000.0) * 100000.0 + j + 50000.0) as i64
}

impl Grid {
    /// A grid with this cell size (m) over the limits [x0, y0, x1, y1].
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
    pub fn near(&self, x: f64, y: f64, r: f64) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        let mut seen: HashSet<usize> = HashSet::new();
        let mut i = ((x - r) / self.cell).floor();
        let i1 = ((x + r) / self.cell).floor();
        while i <= i1 {
            let mut j = ((y - r) / self.cell).floor();
            let j1 = ((y + r) / self.cell).floor();
            while j <= j1 {
                if let Some(v) = self.m.get(&key(i, j)) {
                    for &x in v {
                        if seen.insert(x) {
                            out.push(x);
                        }
                    }
                }
                j += 1.0;
            }
            i += 1.0;
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
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.bb[0]
            && x <= self.bb[2]
            && y >= self.bb[1]
            && y <= self.bb[3]
            && self.outer.iter().any(|r| in_ring(x, y, r))
            && !self.inner.iter().any(|r| in_ring(x, y, r))
    }
    /// landuse.js `edge(s, x, y)`: the distance (m) from a point to the nearest edge of the outer rings.
    pub fn edge(&self, x: f64, y: f64) -> f64 {
        let mut d = f64::INFINITY;
        for r in &self.outer {
            let n = r.len();
            for i in 0..n {
                let p = r[i];
                let q = r[(i + 1) % n];
                d = crate::jsnum::math_min(d, util::seg_dist(x, y, p[0], p[1], q[0], q[1]).d);
            }
        }
        d
    }
}
