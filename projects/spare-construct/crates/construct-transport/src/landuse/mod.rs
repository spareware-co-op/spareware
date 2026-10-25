//! Existing land use from OpenStreetMap (landuse.js `ExistingLU`, contract C4).
//!
//! Takes the downloaded building outlines, land-use areas, schools, early childhood centres, clinics, hotels and shops
//! (an Overpass JSON answer, `out tags geom`), classifies each building, and groups the result into base-year zones on a
//! grid. NZ OSM buildings come from the LINZ import and are almost all untagged (`building=yes`), so the land-use area a
//! building sits in and its footprint size do most of the work. Every number here is indicative and listed in
//! `stats.assumptions`, so the interface and the report can show it.
//!
//! The downloading itself (HTTP requests to an Overpass server) stays outside: [`download_jobs`] splits the study area into the
//! tiles and queries the prototype sends, and [`overpass_elements`] and [`merge_elements`] read and combine the answers.
//! [`build`] then takes the elements and returns the zones.

mod build;
mod geometry;
mod tags;

pub use build::{BuildOpts, BuildResult, CampusStat, RoadLine, Stats, build, constants};
pub use geometry::{Grid, Pt, Shape, assemble, in_ring, ring_area, ring_centroid};
pub use tags::{WEIGHT, count, levels_of, num, shop_use, students, weight};

use crate::jsnum::{math_max, to_fixed, value_to_js_string, value_to_number, value_truthy};
use crate::project::Scenario;
use crate::util::{self, LatLng, fmt};
use serde_json::Value;

/// landuse.js TILE: the edge (m) of a building query tile.
pub const TILE: f64 = 3000.0;
/// landuse.js LU_TILE: the edge (m) of a land-use, amenity and shop query tile.
pub const LU_TILE: f64 = 9000.0;
/// landuse.js MAX_AREA: the largest study area (m2); anything bigger than 20 by 20 km is refused.
pub const MAX_AREA: f64 = 400e6;

/// A study area: south and north latitudes, west and east longitudes (degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StudyBox {
    /// South edge (latitude).
    pub s: f64,
    /// West edge (longitude).
    pub w: f64,
    /// North edge (latitude).
    pub n: f64,
    /// East edge (longitude).
    pub e: f64,
}

impl StudyBox {
    /// The box of two corners [lat, lng] in any order (`[[lat, lng], [lat, lng]]`).
    pub fn from_corners(a: LatLng, b: LatLng) -> StudyBox {
        StudyBox {
            s: crate::jsnum::math_min(a[0], b[0]),
            w: crate::jsnum::math_min(a[1], b[1]),
            n: math_max(a[0], b[0]),
            e: math_max(a[1], b[1]),
        }
    }
    /// True when all four edges are finite numbers and the box has a positive size.
    pub fn is_valid(&self) -> bool {
        [self.s, self.w, self.n, self.e]
            .iter()
            .all(|v| v.is_finite())
            && !(self.n <= self.s)
            && !(self.e <= self.w)
    }
    /// landuse.js `size(B)`: the width and height in metres, on a flat projection at the box's middle latitude.
    pub fn size_m(&self) -> (f64, f64) {
        let f = util::flat((self.s + self.n) / 2.0);
        ((self.e - self.w) * f.kx, (self.n - self.s) * f.ky)
    }
}

/// landuse.js `tiles(b, edge)`: splits a box into near-square tiles no larger than `edge` metres, south to north, west to east.
pub fn tiles(b: &StudyBox, edge: f64) -> Vec<StudyBox> {
    let (w, h) = b.size_m();
    let nx = math_max(1.0, (w / edge - 1e-9).ceil());
    let ny = math_max(1.0, (h / edge - 1e-9).ceil());
    let mut out = Vec::new();
    let mut j = 0.0;
    while j < ny {
        let mut i = 0.0;
        while i < nx {
            out.push(StudyBox {
                s: b.s + (b.n - b.s) * j / ny,
                n: b.s + (b.n - b.s) * (j + 1.0) / ny,
                w: b.w + (b.e - b.w) * i / nx,
                e: b.w + (b.e - b.w) * (i + 1.0) / nx,
            });
            i += 1.0;
        }
        j += 1.0;
    }
    out
}

fn bbox_str(t: &StudyBox) -> String {
    [t.s, t.w, t.n, t.e]
        .iter()
        .map(|v| to_fixed(*v, 6))
        .collect::<Vec<_>>()
        .join(",")
}

/// landuse.js `qBuildings(t)`: the Overpass query for the buildings in a tile (ways and multipolygon relations).
pub fn query_buildings(t: &StudyBox) -> String {
    let bb = bbox_str(t);
    format!(
        "[out:json][timeout:90];(way[\"building\"]({bb});relation[\"building\"][\"type\"=\"multipolygon\"]({bb}););out tags geom;"
    )
}

/// landuse.js `qLanduse(t)`: the Overpass query for land-use areas, schools, clinics, hotels, fast food and shops in a tile, including
/// the areas that enclose the whole tile (asked through the tile's centre and corners).
pub fn query_landuse(t: &StudyBox) -> String {
    let bb = bbox_str(t);
    let lu = "\"landuse\"~\"^(residential|commercial|retail|industrial|education)$\"";
    let pts = [
        [(t.s + t.n) / 2.0, (t.w + t.e) / 2.0],
        [t.s, t.w],
        [t.s, t.e],
        [t.n, t.w],
        [t.n, t.e],
    ];
    let is_in: String = pts
        .iter()
        .enumerate()
        .map(|(i, p)| format!("is_in({},{})->.a{i};", to_fixed(p[0], 6), to_fixed(p[1], 6)))
        .collect();
    let enc: String = (0..pts.len())
        .map(|i| format!("way(pivot.a{i})[{lu}];rel(pivot.a{i})[{lu}];"))
        .collect();
    format!(
        "[out:json][timeout:90];{is_in}(way[{lu}]({bb});relation[{lu}]({bb});{enc}nwr[\"amenity\"~\"^(school|college|university|kindergarten|childcare|doctors|clinic|dentist|hospital|fast_food)$\"]({bb});nwr[\"shop\"]({bb});nwr[\"tourism\"~\"^(hotel|motel)$\"]({bb}););out tags geom;"
    )
}

/// One request of the land-use download: the query to post and the label to show while it runs.
#[derive(Clone, Debug, PartialEq)]
pub struct DownloadJob {
    /// Overpass QL.
    pub query: String,
    /// "Downloading buildings 2/6".
    pub label: String,
}

/// The requests `ExistingLU.fetch(bounds)` sends, in order: the buildings by 3 km tile, then the land-use areas, schools and shops by
/// 9 km tile. Fails with the prototype's message when the area is not usable or is bigger than about 20 by 20 km.
pub fn download_jobs(bounds: &StudyBox) -> Result<Vec<DownloadJob>, String> {
    if !bounds.is_valid() {
        return Err(
            "Existing land use needs a study area: draw or search for one first.".to_string(),
        );
    }
    let (w, h) = bounds.size_m();
    if w * h > MAX_AREA {
        return Err(format!(
            "The study area is {} \u{d7} {} km. Existing land use works up to about 20 \u{d7} 20 km: zoom in or draw a smaller area.",
            fmt::num(w / 1000.0, 1),
            fmt::num(h / 1000.0, 1)
        ));
    }
    let mut jobs = Vec::new();
    let bt = tiles(bounds, TILE);
    for (i, t) in bt.iter().enumerate() {
        jobs.push(DownloadJob {
            query: query_buildings(t),
            label: format!(
                "Downloading buildings{}",
                if bt.len() > 1 {
                    format!(" {}/{}", i + 1, bt.len())
                } else {
                    String::new()
                }
            ),
        });
    }
    let lt = tiles(bounds, LU_TILE);
    for (i, t) in lt.iter().enumerate() {
        jobs.push(DownloadJob {
            query: query_landuse(t),
            label: format!(
                "Downloading land-use areas, schools and shops{}",
                if lt.len() > 1 {
                    format!(" {}/{}", i + 1, lt.len())
                } else {
                    String::new()
                }
            ),
        });
    }
    Ok(jobs)
}

/// The elements of one Overpass answer (`j.elements || []`), or the server's complaint: an answer whose `remark` says error, timed out
/// or out of memory is a failure even though it came back with status 200.
pub fn overpass_elements(answer: &Value, host: &str) -> Result<Vec<Value>, String> {
    if let Some(remark) = answer.get("remark").filter(|r| value_truthy(r)) {
        let text = value_to_js_string(remark);
        let low = text.to_lowercase();
        if low.contains("error") || low.contains("timed out") || low.contains("out of memory") {
            return Err(format!("{host}: {text}"));
        }
    }
    Ok(answer
        .get("elements")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// The features of several answers as one list: a feature that came back in two tiles is kept once (the later copy, in the place of the first).
pub fn merge_elements(batches: &[Vec<Value>]) -> Vec<Value> {
    let mut order: Vec<String> = Vec::new();
    let mut seen: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    for batch in batches {
        for e in batch {
            let key = format!(
                "{}/{}",
                e.get("type")
                    .map_or("undefined".to_string(), value_to_js_string),
                e.get("id")
                    .map_or("undefined".to_string(), value_to_js_string)
            );
            if !seen.contains_key(&key) {
                order.push(key.clone());
            }
            seen.insert(key, e.clone());
        }
    }
    order
        .into_iter()
        .map(|k| seen.remove(&k).expect("a key"))
        .collect()
}

/// The named roads `build` may use to name zones: the scenario's links that have a name and more than one shape point (`L.pts`).
pub fn road_lines(sc: &Scenario) -> Vec<RoadLine> {
    sc.links
        .values()
        .filter_map(|l| {
            let name = l.name.as_str().filter(|n| !n.is_empty())?;
            let pts = l.pts.get().filter(|p| p.len() > 1)?;
            Some(RoadLine {
                name: name.to_string(),
                pts: pts.iter().map(|p| p.0).collect(),
            })
        })
        .collect()
}

/// `+v`, for callers reading a study area from loose JSON.
pub fn number_of(v: Option<&Value>) -> f64 {
    v.map_or(f64::NAN, value_to_number)
}
