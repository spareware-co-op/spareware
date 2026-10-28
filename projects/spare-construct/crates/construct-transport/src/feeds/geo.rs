//! Geometry helpers of packs/nz/feeds.js: polygons from downloaded layers, point in polygon with a grid of boxes, centroids, and the study area's box.

use crate::edit::json::JsonValue;
use crate::jsmath;
use crate::project::{Link, Scenario};
use crate::util::LatLng;
use std::collections::HashMap;

/// One polygon ring: points as [x, y] (longitude, latitude).
pub type Ring = Vec<[f64; 2]>;

/// A box as [west (x0), south (y0), east (x1), north (y1)] of a polygon's points.
pub type RingsBox = [f64; 4];

/// `ringsBox(rings)`: the box around every point of the rings (infinite and reversed for no points).
pub fn rings_box(rings: &[Ring]) -> RingsBox {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for r in rings {
        for p in r {
            if p[0] < x0 {
                x0 = p[0];
            }
            if p[0] > x1 {
                x1 = p[0];
            }
            if p[1] < y0 {
                y0 = p[1];
            }
            if p[1] > y1 {
                y1 = p[1];
            }
        }
    }
    [x0, y0, x1, y1]
}

/// `inRings(x, y, rings)`: even-odd point in polygon (holes included); x is longitude, y latitude.
pub fn in_rings(x: f64, y: f64, rings: &[Ring]) -> bool {
    let mut inside = false;
    for r in rings {
        let mut j = r.len().wrapping_sub(1);
        for i in 0..r.len() {
            let (xi, yi, xj, yj) = (r[i][0], r[i][1], r[j][0], r[j][1]);
            if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                inside = !inside;
            }
            j = i;
        }
    }
    inside
}

/// `centroid(rings)`: the area centroid of the biggest ring as [lat, lng] (the middle of the box when the area is zero).
pub fn centroid(rings: &[Ring]) -> LatLng {
    let empty: Ring = Vec::new();
    let mut r = rings.first().unwrap_or(&empty);
    for q in rings {
        if q.len() > r.len() {
            r = q;
        }
    }
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    let mut j = r.len().wrapping_sub(1);
    for i in 0..r.len() {
        let f = r[j][0] * r[i][1] - r[i][0] * r[j][1];
        a += f;
        cx += (r[j][0] + r[i][0]) * f;
        cy += (r[j][1] + r[i][1]) * f;
        j = i;
    }
    if a == 0.0 || a.is_nan() {
        let bb = rings_box(rings);
        return [(bb[1] + bb[3]) / 2.0, (bb[0] + bb[2]) / 2.0];
    }
    [cy / (3.0 * a), cx / (3.0 * a)]
}

/// A polygon with its box, for [`PolyIndex`].
pub trait HasBox {
    /// The polygon's rings.
    fn rings(&self) -> &[Ring];
    /// The polygon's box.
    fn bb(&self) -> RingsBox;
}

/// `polyIndex(polys, cell)`: a grid of polygon boxes for fast point lookups (cells in degrees, 0.005 by default).
#[derive(Clone, Debug)]
pub struct PolyIndex {
    cell: f64,
    grid: HashMap<i64, Vec<usize>>,
}

fn key(i: f64, j: f64) -> i64 {
    (i * 1e6 + j) as i64
}

impl PolyIndex {
    /// Indexes polygons by the cells their boxes cover.
    pub fn new<T: HasBox>(polys: &[T], cell: f64) -> PolyIndex {
        let mut grid: HashMap<i64, Vec<usize>> = HashMap::new();
        for (n, p) in polys.iter().enumerate() {
            let [x0, y0, x1, y1] = p.bb();
            let mut i = (x0 / cell).floor();
            while i <= (x1 / cell).floor() {
                let mut j = (y0 / cell).floor();
                while j <= (y1 / cell).floor() {
                    grid.entry(key(i, j)).or_default().push(n);
                    j += 1.0;
                }
                i += 1.0;
            }
        }
        PolyIndex { cell, grid }
    }
    /// The polygons (by position in the list given to [`PolyIndex::new`]) that hold the point, in the order they were indexed.
    pub fn find<T: HasBox>(&self, polys: &[T], x: f64, y: f64) -> Vec<usize> {
        self.grid
            .get(&key((x / self.cell).floor(), (y / self.cell).floor()))
            .map(|v| {
                v.iter()
                    .copied()
                    .filter(|&n| {
                        let p = &polys[n];
                        let bb = p.bb();
                        x >= bb[0] && x <= bb[2] && y >= bb[1] && y <= bb[3] && in_rings(x, y, p.rings())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Rings as downloaded (`geometry.rings`): lists of [x, y, ...] points.
pub fn rings_of(v: Option<&JsonValue>) -> Vec<Ring> {
    match v {
        Some(JsonValue::Array(rs)) => rs
            .iter()
            .map(|r| match r {
                JsonValue::Array(pts) => pts
                    .iter()
                    .map(|p| match p {
                        JsonValue::Array(c) => [c.first().map_or(f64::NAN, JsonValue::to_number), c.get(1).map_or(f64::NAN, JsonValue::to_number)],
                        _ => [f64::NAN, f64::NAN],
                    })
                    .collect(),
                _ => Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// `liveLinks(sc)`: the links that are not closed and have both end junctions.
pub fn live_links(sc: &Scenario) -> Vec<&Link> {
    sc.links.values().filter(|l| !l.status.is("closed") && sc.nodes.contains_key(&l.a) && sc.nodes.contains_key(&l.b)).collect()
}

/// A box of latitudes and longitudes: south, north, west, east.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatLngBox {
    /// South edge.
    pub s: f64,
    /// North edge.
    pub n: f64,
    /// West edge.
    pub w: f64,
    /// East edge.
    pub e: f64,
}

/// `netBox(sc)`: the box around the scenario's junctions; None for none.
pub fn net_box(sc: &Scenario) -> Option<LatLngBox> {
    if sc.nodes.is_empty() {
        return None;
    }
    let lat: Vec<f64> = sc.nodes.values().map(|n| n.lat.num()).collect();
    let lng: Vec<f64> = sc.nodes.values().map(|n| n.lng.num()).collect();
    Some(LatLngBox {
        s: crate::jsnum::math_min_of(&lat),
        n: crate::jsnum::math_max_of(&lat),
        w: crate::jsnum::math_min_of(&lng),
        e: crate::jsnum::math_max_of(&lng),
    })
}

/// `inBox(B, ll)`: the point is inside the box (edges included).
pub fn in_box(b: &LatLngBox, ll: &LatLng) -> bool {
    ll[0] >= b.s && ll[0] <= b.n && ll[1] >= b.w && ll[1] <= b.e
}

/// `padBox(b, m)`: the box moved out by m metres on every side.
pub fn pad_box(b: &LatLngBox, m: f64) -> LatLngBox {
    let dy = m / 110574.0;
    let dx = m / (111320.0 * jsmath::cos((b.s + b.n) / 2.0 * std::f64::consts::PI / 180.0));
    LatLngBox { s: b.s - dy, w: b.w - dx, n: b.n + dy, e: b.e + dx }
}

/// The Leaflet-style bounds of a box.
pub fn bounds_of_box(b: &LatLngBox) -> crate::importers::Bounds {
    crate::importers::Bounds { south: b.s, west: b.w, north: b.n, east: b.e }
}

/// A box from Leaflet-style bounds.
pub fn box_of_bounds(b: &crate::importers::Bounds) -> LatLngBox {
    LatLngBox { s: b.south, w: b.west, n: b.north, e: b.east }
}
