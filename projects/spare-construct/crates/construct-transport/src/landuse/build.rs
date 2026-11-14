//! `ExistingLU.build`: classify each building, then group the activity into base-year zones on a grid (landuse.js "build zones").

use super::StudyBox;
use super::geometry::{Grid, Pt, Shape, assemble, in_ring, ring_area, ring_centroid};
use super::tags::{
    CHILD_A, CHILD_B, EDU_A, EDU_B, IGNORE, IGNORE2, IND_B, MED_A, OTHER_A, OTHER_B, RES_B,
    RETAIL_B, SHOP_BULKY, SHOP_IND, SHOP_YARD, TERT_A, TERT_B, WEIGHT, count_tag, in_set,
    levels_of, lu_class, num, shop_use, students, tag_str, tag_text, tag_truthy, weight,
};
use crate::edit::IdSource;
use crate::graph::{js_sort_by, js_str_lt};
use crate::jsnum::{
    JsNum, Maybe, Num, math_hypot, math_max, math_min, math_round, number_to_js_string, or_default,
    usize_to_f64, value_to_js_string, value_to_number, value_truthy,
};
use crate::project::{LANDUSES, OrderedMap, Zone};
use crate::util::{self, LatLng, fmt, r7};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

const MIN_FOOT: f64 = 30.0;
const HOUSE_MIN: f64 = 60.0;
const HOUSE_MAX: f64 = 400.0;
const M2_PER_DWELLING: f64 = 90.0;
const M2_PER_UNIT: f64 = 200.0;
const M2_PER_ROOM: f64 = 35.0;
const OUTBUILDING_R: f64 = 25.0;
const SHOP_NEAR: f64 = 60.0;
const RURAL_SHOP_NEAR: f64 = 40.0;
const POI_SNAP: f64 = 25.0;
const CAMPUS_R: f64 = 250.0;
const RURAL_GROUP: f64 = 40.0;
const DENSE_R: f64 = 100.0;
const DENSE_N: usize = 6;
const MEDICAL_M2: f64 = 300.0;
const CHILDREN: f64 = 40.0;
const SHOP_M2: f64 = 100.0;
const SUPERMARKET_M2: f64 = 2500.0;
const FASTFOOD_M2: f64 = 200.0;
const TERTIARY: f64 = 1000.0;
const SITE_COVER: f64 = 0.35;
const MERGE_BELOW: f64 = 15.0;

/// A named road for naming zones: its name and shape points ([lat, lng]).
#[derive(Clone, Debug, PartialEq)]
pub struct RoadLine {
    /// Road name.
    pub name: String,
    /// Shape points (at least two).
    pub pts: Vec<LatLng>,
}

/// What `ExistingLU.build(elements, bounds, opts)` is given beyond the elements and the study area.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuildOpts {
    /// Grid cell size (m); at least 50, default 400.
    pub cell: Option<f64>,
    /// The roads that name zones whose buildings have no street address (see [`super::road_lines`]).
    pub links: Vec<RoadLine>,
}

/// A school or campus listed in the statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct CampusStat {
    /// Name ("Unnamed school").
    pub name: String,
    /// Students.
    pub students: f64,
    /// Where it was found: "area", "building" or "point".
    pub src: String,
}

/// The numbers behind a build (`stats`): what was found, what was left out and why, and the assumptions made.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Stats {
    /// Buildings inside the study area.
    pub buildings: f64,
    /// Buildings given a land use.
    pub used: f64,
    /// Buildings left out.
    pub ignored: f64,
    /// Buildings outside the study area.
    pub outside: f64,
    /// Buildings used, by class, in the order first found.
    pub by_class: Vec<(String, f64)>,
    /// Buildings left out, by reason, in the order first found.
    pub ignored_by: Vec<(String, f64)>,
    /// Dwellings (houses and apartments).
    pub dwellings: f64,
    /// Floor area (m2) by land use: retail, office, industrial, and after a run warehouse and fast food.
    pub gfa: Vec<(String, f64)>,
    /// School students.
    pub students: f64,
    /// Early childhood children.
    pub children: f64,
    /// Medical floor area (m2).
    pub medical: f64,
    /// Supermarket floor area (m2).
    pub supermarket: f64,
    /// Tertiary students.
    pub tertiary_students: f64,
    /// Hotel rooms.
    pub hotel_rooms: f64,
    /// Schools found.
    pub schools: Vec<CampusStat>,
    /// Tertiary campuses found.
    pub tertiary: Vec<CampusStat>,
    /// Zones made.
    pub zones: f64,
    /// Grid cells merged into a neighbour for having little activity.
    pub merged: f64,
    /// Every assumption, in words.
    pub assumptions: Vec<String>,
}

impl Stats {
    fn start() -> Stats {
        Stats {
            gfa: vec![
                ("retail".into(), 0.0),
                ("office".into(), 0.0),
                ("industrial".into(), 0.0),
            ],
            ..Default::default()
        }
    }
    /// The statistics as JSON, in the prototype's shape.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let obj = |v: &[(String, f64)]| {
            Value::Object(
                v.iter()
                    .map(|(k, x)| (k.clone(), json!(x)))
                    .collect::<Map<_, _>>(),
            )
        };
        let list = |v: &[CampusStat]| {
            Value::Array(
                v.iter()
                    .map(|c| json!({ "name": c.name, "students": c.students, "src": c.src }))
                    .collect(),
            )
        };
        json!({
            "buildings": self.buildings, "used": self.used, "ignored": self.ignored, "outside": self.outside,
            "byClass": obj(&self.by_class), "ignoredBy": obj(&self.ignored_by), "dwellings": self.dwellings, "gfa": obj(&self.gfa),
            "students": self.students, "children": self.children, "medical": self.medical, "supermarket": self.supermarket,
            "tertiaryStudents": self.tertiary_students, "hotelRooms": self.hotel_rooms, "schools": list(&self.schools), "tertiary": list(&self.tertiary),
            "zones": self.zones, "merged": self.merged, "assumptions": self.assumptions,
        })
    }
}

/// What `ExistingLU.build` returns: the zones and the statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildResult {
    /// Base-year zones ("Existing · Great South Road"), each with its land-use quantities.
    pub zones: Vec<Zone>,
    /// What was found and assumed.
    pub stats: Stats,
}

/// A building.
struct Bld<'a> {
    id: f64,
    t: &'a Value,
    s: Shape,
    b: String,
    x: f64,
    y: f64,
    area: f64,
    inside: bool,
    street: Option<String>,
    pois: Vec<usize>,
    cls: Option<&'static str>,
    k: Option<&'static str>,
    q: Option<f64>,
    why: String,
    own_med: bool,
}

impl<'a> Bld<'a> {
    fn new(id: f64, t: &'a Value, s: Shape) -> Bld<'a> {
        Bld {
            id,
            t,
            s,
            b: tag_text(t, "building"),
            x: 0.0,
            y: 0.0,
            area: 0.0,
            inside: false,
            street: None,
            pois: Vec::new(),
            cls: None,
            k: None,
            q: None,
            why: String::new(),
            own_med: false,
        }
    }
}

/// A land-use area or an amenity / shop site.
struct Area<'a> {
    site: Option<&'static str>,
    lu: Option<&'static str>,
    t: &'a Value,
    s: Shape,
    blds: Vec<usize>,
    campus: Option<usize>,
    has_med: bool,
    hb: Option<Vec<usize>>,
}

impl<'a> Area<'a> {
    fn new(
        site: Option<&'static str>,
        lu: Option<&'static str>,
        t: &'a Value,
        s: Shape,
    ) -> Area<'a> {
        Area {
            site,
            lu,
            t,
            s,
            blds: Vec::new(),
            campus: None,
            has_med: false,
            hb: None,
        }
    }
}

/// A shop, clinic, school or similar mapped as a point.
struct Poi<'a> {
    kind: &'static str,
    x: f64,
    y: f64,
    t: &'a Value,
}

struct Campus<'a> {
    kind: &'static str,
    x: f64,
    y: f64,
    t: &'a Value,
    from: &'static str,
    area: Option<usize>,
}

/// Activity not tied to one building.
struct Item {
    x: f64,
    y: f64,
    k: &'static str,
    q: f64,
    street: Option<String>,
}

/// A grid cell's activity: its column and row, weight, weighted coordinate sums, quantities by land use (in WEIGHT's order)
/// and the weight of each street named in it.
struct Cell {
    i: f64,
    j: f64,
    w: f64,
    sx: f64,
    sy: f64,
    lu: Vec<f64>,
    streets: Vec<(String, f64)>,
}

/// The study area's flat projection: metres east and north of its south-west corner.
struct Frame {
    bx: StudyBox,
    kx: f64,
    ky: f64,
    width: f64,
    height: f64,
}

impl Frame {
    fn new(bx: StudyBox) -> Frame {
        let f = util::flat(f64::midpoint(bx.s, bx.n));
        let (kx, ky) = (f.kx, f.ky);
        Frame {
            bx,
            kx,
            ky,
            width: (bx.e - bx.w) * kx,
            height: (bx.n - bx.s) * ky,
        }
    }
    // The limits of the indexes: the study area and 2 km around it
    fn lim(&self) -> [f64; 4] {
        [-2000.0, -2000.0, self.width + 2000.0, self.height + 2000.0]
    }
    fn local(&self, lat: f64, lng: f64) -> Pt {
        [(lng - self.bx.w) * self.kx, (lat - self.bx.s) * self.ky]
    }
    fn inside(&self, x: f64, y: f64) -> bool {
        x >= 0.0 && x <= self.width && y >= 0.0 && y <= self.height
    }
}

/// Outer or inner rings.
type Rings = Vec<Vec<Pt>>;

fn is_finite_value(v: Option<&Value>) -> bool {
    v.is_some_and(|v| value_to_number(v).is_finite())
}

fn word(t: &Value, k: &str, w: &str) -> bool {
    tag_str(t, k) == Some(w)
}

fn bump(v: &mut Vec<(String, f64)>, key: &str) {
    match v.iter_mut().find(|(k, _)| k == key) {
        Some((_, n)) => *n += 1.0,
        None => v.push((key.to_string(), 1.0)),
    }
}

fn kind_of(t: &Value) -> Option<&'static str> {
    let am = tag_str(t, "amenity");
    let tourism = tag_str(t, "tourism");
    let shop = tag_str(t, "shop");
    if in_set(&EDU_A, am) {
        Some("school")
    } else if in_set(&TERT_A, am) {
        Some("tertiary")
    } else if in_set(&CHILD_A, am) {
        Some("childcare")
    } else if in_set(&MED_A, am) {
        Some("medical")
    } else if am == Some("hospital") {
        Some("hospital")
    } else if am == Some("fast_food") {
        Some("fastfood")
    } else if tourism == Some("hotel") || tourism == Some("motel") {
        Some("hotel")
    } else if shop == Some("supermarket") {
        Some("supermarket")
    } else if tag_truthy(t, "shop") {
        Some("shop")
    } else {
        None
    }
}

fn street_of(t: &Value) -> Option<String> {
    if tag_truthy(t, "addr:street") {
        Some(tag_text(t, "addr:street"))
    } else {
        None
    }
}

// Levels or a height are mapped
fn tagged(t: &Value) -> bool {
    num(&tag_text(t, "building:levels")).is_some_and(|l| l >= 1.0)
        || num(&tag_text(t, "height")).is_some_and(|ht| ht >= 4.5)
}

/// `ExistingLU.build(elements, bounds, opts)`: zones for the existing land use in a study area, from an Overpass JSON answer.
///
/// `elements` is the list of elements, or an object with an `elements` list. With no usable `bounds` the box of the elements is used.
/// Buildings are classified from their own tags, shops, clinics and schools mapped inside or at them, the site or land-use area they
/// sit in, and for the rest by size; the activity is grouped on a grid of `opts.cell` metres at its weighted centre, and cells with
/// less than 15 dwelling-equivalents merge into the nearest cell. Zones are named after the street of their addresses, else the
/// nearest named road, else "Area n".
pub fn build(
    elements: &Value,
    bounds: Option<&StudyBox>,
    ids: &mut dyn IdSource,
    opts: &BuildOpts,
) -> BuildResult {
    let cell = math_max(50.0, or_default(opts.cell.unwrap_or(f64::NAN), 400.0));
    let els = sorted_elements(elements);
    let bx = study_box(bounds, &els);
    let mut stats = Stats::start();
    if !bx.s.is_finite() || els.is_empty() {
        return BuildResult {
            zones: Vec::new(),
            stats,
        };
    }
    let fr = Frame::new(bx);
    let empty = json!({});
    let (mut w, cand) = World::gather(&fr, &els, &empty);
    for &bi in &cand {
        classify_building(&mut w, bi);
    }
    rural_outbuildings(&mut w, fr.lim());
    site_items(&mut w, &fr);
    orphan_campuses(&mut w);
    let loose_n = loose_items(&mut w, &fr);
    let kept = kept_campuses(&mut w.campuses, &w.areas);
    campus_items(&mut w, &kept, &fr, &mut stats);

    // Stats over buildings inside the study area
    let extra = std::mem::take(&mut w.extra);
    let items = building_items(&w.blds, extra, &fr, &mut stats);
    totals(&items, &mut stats);

    // Aggregate to grid cells, then merge cells with little activity into the nearest cell
    let mut live = grid_cells(&items, &fr, cell);
    merge_cells(&mut live, &mut stats);

    let roads = named_roads(opts, &fr);
    let zones = make_zones(&live, &roads, &fr, ids);
    stats.zones = usize_to_f64(zones.len());
    assumptions(&mut stats, w.no_area, loose_n, cell);
    BuildResult { zones, stats }
}

/// The elements with a type and an id, each once (the later copy, in the place of the first), ordered by type and then id so the
/// result doesn't depend on download order.
fn sorted_elements(elements: &Value) -> Vec<&Value> {
    let raw: &[Value] = match elements {
        Value::Array(arr) => arr,
        other => other
            .get("elements")
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice),
    };
    let mut by_id: Vec<(String, &Value)> = Vec::new();
    let mut index_of: HashMap<String, usize> = HashMap::new();
    for el in raw {
        if !el.is_object()
            || !el.get("type").is_some_and(value_truthy)
            || el.get("id").is_none_or(Value::is_null)
        {
            continue;
        }
        let key = format!(
            "{}/{}",
            value_to_js_string(&el["type"]),
            value_to_js_string(&el["id"])
        );
        if let Some(&i) = index_of.get(&key) {
            // i is the position the key was given in by_id below, so it is always there
            if let Some(slot) = by_id.get_mut(i) {
                slot.1 = el;
            }
        } else {
            index_of.insert(key.clone(), by_id.len());
            by_id.push((key, el));
        }
    }
    let mut els: Vec<&Value> = by_id.into_iter().map(|(_, el)| el).collect();
    js_sort_by(&mut els, |a, b| {
        let (ta, tb) = (
            value_to_js_string(&a["type"]),
            value_to_js_string(&b["type"]),
        );
        if js_str_lt(&ta, &tb) {
            -1.0
        } else if js_str_lt(&tb, &ta) {
            1.0
        } else {
            value_to_number(&a["id"]) - value_to_number(&b["id"])
        }
    });
    els
}

/// The study area: `bounds` when usable, else the box of every point of the elements.
fn study_box(bounds: Option<&StudyBox>, els: &[&Value]) -> StudyBox {
    if let Some(b) = bounds.copied().filter(StudyBox::is_valid) {
        return b;
    }
    let mut bx = StudyBox {
        s: f64::INFINITY,
        w: f64::INFINITY,
        n: f64::NEG_INFINITY,
        e: f64::NEG_INFINITY,
    };
    for &el in els {
        let points: Vec<&Value> = match el.get("geometry") {
            Some(g) if value_truthy(g) => g.as_array().map_or(Vec::new(), |arr| arr.iter().collect()),
            _ => {
                if el.get("lat").is_some_and(|v| !v.is_null()) {
                    vec![el]
                } else {
                    Vec::new()
                }
            }
        };
        for pt in points {
            if !value_truthy(pt) {
                continue;
            }
            let lat = pt.get("lat").map_or(f64::NAN, value_to_number);
            let lon = pt.get("lon").map_or(f64::NAN, value_to_number);
            bx.s = math_min(bx.s, lat);
            bx.n = math_max(bx.n, lat);
            bx.w = math_min(bx.w, lon);
            bx.e = math_max(bx.e, lon);
        }
    }
    bx
}

fn pts_of(fr: &Frame, g: Option<&Value>) -> Vec<Pt> {
    g.and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter(|pt| {
                    value_truthy(pt)
                        && is_finite_value(pt.get("lat"))
                        && is_finite_value(pt.get("lon"))
                })
                .map(|pt| fr.local(value_to_number(&pt["lat"]), value_to_number(&pt["lon"])))
                .collect()
        })
        .unwrap_or_default()
}

// The outer and inner rings of a multipolygon relation, from its member ways
fn relation_rings(fr: &Frame, el: &Value) -> (Rings, Rings) {
    let ms: Vec<&Value> =
        el.get("members")
            .and_then(Value::as_array)
            .map_or(Vec::new(), |arr| {
                arr.iter()
                    .filter(|mem| {
                        mem.get("type").and_then(Value::as_str) == Some("way")
                            && mem.get("geometry").is_some_and(value_truthy)
                    })
                    .collect()
            });
    let outer = assemble(
        ms.iter()
            .filter(|mem| mem.get("role").and_then(Value::as_str) != Some("inner"))
            .map(|mem| pts_of(fr, mem.get("geometry")))
            .collect(),
    );
    let inner = assemble(
        ms.iter()
            .filter(|mem| mem.get("role").and_then(Value::as_str) == Some("inner"))
            .map(|mem| pts_of(fr, mem.get("geometry")))
            .collect(),
    );
    (outer, inner)
}

fn shape(fr: &Frame, el: &Value) -> Option<Shape> {
    let (outer, inner): (Rings, Rings) = match el.get("type").and_then(Value::as_str) {
        Some("way") => {
            let ring = pts_of(fr, el.get("geometry"));
            if ring.len() < 3 {
                return None;
            }
            (vec![ring], Vec::new())
        }
        Some("relation") => {
            let (outer, inner) = relation_rings(fr, el);
            if outer.is_empty() {
                return None;
            }
            (outer, inner)
        }
        _ => return None,
    };
    let mut bb = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for ring in &outer {
        for pt in ring {
            bb[0] = math_min(bb[0], pt[0]);
            bb[1] = math_min(bb[1], pt[1]);
            bb[2] = math_max(bb[2], pt[0]);
            bb[3] = math_max(bb[3], pt[1]);
        }
    }
    let areas: Vec<f64> = outer.iter().map(|ring| ring_area(ring).abs()).collect();
    let sum = areas.iter().fold(0.0, |acc, x| acc + x);
    let hole = inner.iter().fold(0.0, |acc, ring| acc + ring_area(ring).abs());
    let area = math_max(0.0, sum - hole);
    let mx = areas.iter().copied().fold(f64::NEG_INFINITY, math_max);
    // The largest area is one of the list, so its ring is found (only a NaN area, which finite points do not give, could miss)
    let big = areas
        .iter()
        .position(|ar| *ar == mx)
        .and_then(|i| outer.get(i))?;
    let mut cen = ring_centroid(big);
    if !in_ring(cen[0], cen[1], big) {
        let count = usize_to_f64(big.len());
        cen = big
            .iter()
            .fold([0.0, 0.0], |acc, pt| [acc[0] + pt[0] / count, acc[1] + pt[1] / count]);
    }
    Some(Shape {
        outer,
        inner,
        bb,
        area,
        cx: cen[0],
        cy: cen[1],
    })
}

/// The buildings, the areas (land use and amenity/shop sites) and the point POIs, in element order.
fn sort_features<'a>(
    fr: &Frame,
    els: &[&'a Value],
    empty: &'a Value,
) -> (Vec<Bld<'a>>, Vec<Area<'a>>, Vec<Poi<'a>>) {
    let mut blds: Vec<Bld> = Vec::new();
    let mut areas: Vec<Area> = Vec::new();
    let mut pois: Vec<Poi> = Vec::new();
    for &el in els {
        let t = el.get("tags").filter(|t| value_truthy(t)).unwrap_or(empty);
        let kind = kind_of(t);
        let ty = el.get("type").and_then(Value::as_str);
        if ty == Some("node") {
            if let Some(kind) = kind
                && is_finite_value(el.get("lat"))
            {
                let [x, y] = fr.local(
                    value_to_number(&el["lat"]),
                    el.get("lon").map_or(f64::NAN, value_to_number),
                );
                pois.push(Poi { kind, x, y, t });
            }
            continue;
        }
        let shp = shape(fr, el);
        if tag_truthy(t, "building") && !word(t, "building", "no") {
            if let Some(shp) = shp {
                blds.push(Bld::new(value_to_number(&el["id"]), t, shp));
            }
            continue;
        }
        let Some(shp) = shp else {
            // open way or broken relation carrying a POI: use the mean of its points
            let pts = pts_of(fr, el.get("geometry"));
            if let Some(kind) = kind
                && !pts.is_empty()
            {
                let count = usize_to_f64(pts.len());
                pois.push(Poi {
                    kind,
                    x: pts.iter().fold(0.0, |acc, pt| acc + pt[0]) / count,
                    y: pts.iter().fold(0.0, |acc, pt| acc + pt[1]) / count,
                    t,
                });
            }
            continue;
        };
        if let Some(site) = kind {
            areas.push(Area::new(Some(site), None, t, shp));
        } else if let Some(lu) = lu_class(tag_str(t, "landuse")) {
            areas.push(Area::new(None, Some(lu), t, shp));
        }
    }
    (blds, areas, pois)
}

/// Building candidates (not ignored by tag or size) and their indexes by outline and by centre.
fn candidates(fr: &Frame, blds: &mut [Bld<'_>], lim: [f64; 4]) -> (Vec<usize>, Grid, Grid) {
    let mut cand: Vec<usize> = Vec::new();
    let mut b_idx = Grid::new(100.0, lim);
    let mut c_idx = Grid::new(100.0, lim);
    for (i, bd) in blds.iter_mut().enumerate() {
        bd.x = bd.s.cx;
        bd.y = bd.s.cy;
        bd.area = bd.s.area;
        bd.inside = fr.inside(bd.x, bd.y);
        bd.street = street_of(bd.t);
        let man_made = tag_text(bd.t, "man_made");
        let structure = matches!(
            man_made.as_str(),
            "storage_tank"
                | "silo"
                | "water_tower"
                | "street_cabinet"
                | "reservoir_covered"
                | "wastewater_plant"
        );
        if IGNORE.contains(&bd.b.as_str())
            || IGNORE2.contains(&bd.b.as_str())
            || structure
            || tag_truthy(bd.t, "aeroway")
        {
            bd.why = "outbuilding or structure".to_string();
        } else if bd.area < MIN_FOOT {
            bd.why = "under 30 m\u{b2}".to_string();
        } else {
            cand.push(i);
            b_idx.add(bd.s.bb, i);
            c_idx.add([bd.x, bd.y, bd.x, bd.y], i);
        }
    }
    (cand, b_idx, c_idx)
}

/// Point POIs: snap to the building they sit in, or the one whose outline is nearest within 25 m (points are often mapped at
/// the entrance, outside the wall of a large building). Returns the POIs left loose.
fn snap_pois(blds: &mut [Bld<'_>], pois: &[Poi<'_>], b_idx: &Grid) -> Vec<usize> {
    let mut loose: Vec<usize> = Vec::new();
    for (pi, p) in pois.iter().enumerate() {
        let mut hit: Option<usize> = b_idx
            .at(p.x, p.y)
            .iter()
            .copied()
            .find(|&i| blds.get(i).is_some_and(|bd| bd.s.contains(p.x, p.y)));
        if hit.is_none() {
            let mut best = POI_SNAP;
            for i in b_idx.near(p.x, p.y, POI_SNAP) {
                // The index holds positions in blds
                let Some(bd) = blds.get(i) else {
                    continue;
                };
                let d = bd.s.edge(p.x, p.y);
                if d < best {
                    best = d;
                    hit = Some(i);
                }
            }
        }
        if let Some(i) = hit
            && let Some(bd) = blds.get_mut(i)
        {
            bd.pois.push(pi);
        } else {
            loose.push(pi);
        }
    }
    loose
}

/// General retail and supermarkets (not workshops or trade suppliers), which make a commercial area retail.
fn shop_grid(pois: &[Poi<'_>], lim: [f64; 4]) -> Grid {
    let mut shops = Grid::new(100.0, lim);
    for (pi, p) in pois.iter().enumerate() {
        if p.kind == "supermarket"
            || (p.kind == "shop" && shop_use(tag_str(p.t, "shop")) == "retail")
        {
            shops.add([p.x, p.y, p.x, p.y], pi);
        }
    }
    shops
}

/// Campuses (schools, tertiary, early childhood) of the areas and the points; each campus area keeps its campus in its `campus` slot.
fn area_campuses<'a>(areas: &mut [Area<'a>], pois: &[Poi<'a>]) -> Vec<Campus<'a>> {
    let mut campuses: Vec<Campus> = Vec::new();
    for (ai, a) in areas.iter_mut().enumerate() {
        if let Some(site) = a.site
            && matches!(site, "school" | "tertiary" | "childcare")
        {
            a.campus = Some(campuses.len());
            campuses.push(Campus {
                kind: site,
                x: a.s.cx,
                y: a.s.cy,
                t: a.t,
                from: "area",
                area: Some(ai),
            });
        }
    }
    for p in pois {
        if matches!(p.kind, "school" | "tertiary" | "childcare") {
            campuses.push(Campus {
                kind: p.kind,
                x: p.x,
                y: p.y,
                t: p.t,
                from: "point",
                area: None,
            });
        }
    }
    campuses
}

/// Everything the classification reads and writes: the features, the campuses and activity found so far, and the indexes.
struct World<'a> {
    blds: Vec<Bld<'a>>,
    areas: Vec<Area<'a>>,
    pois: Vec<Poi<'a>>,
    campuses: Vec<Campus<'a>>,
    extra: Vec<Item>,
    orphans: Vec<usize>,
    rural: Vec<usize>,
    loose: Vec<usize>,
    no_area: usize,
    a_idx: Grid,
    c_idx: Grid,
    shops: Grid,
}

/// One building being classified: its tags, where it is, and the sites and areas it sits in.
struct Here<'a> {
    bi: usize,
    t: &'a Value,
    bt: String,
    am: Option<&'a str>,
    tourism: Option<&'a str>,
    x: f64,
    y: f64,
    area: f64,
    sites_at: Vec<usize>,
    site: Option<usize>,
    site_kind: Option<&'static str>,
    lu_kind: Option<&'static str>,
}

impl<'a> World<'a> {
    /// The features sorted and indexed, and the building candidates.
    fn gather(fr: &Frame, els: &[&'a Value], empty: &'a Value) -> (World<'a>, Vec<usize>) {
        let lim = fr.lim();
        let (mut blds, mut areas, pois) = sort_features(fr, els, empty);
        let mut a_idx = Grid::new(250.0, lim);
        for (i, a) in areas.iter().enumerate() {
            a_idx.add(a.s.bb, i);
        }
        let (cand, b_idx, c_idx) = candidates(fr, &mut blds, lim);
        let loose = snap_pois(&mut blds, &pois, &b_idx);
        let shops = shop_grid(&pois, lim);
        let campuses = area_campuses(&mut areas, &pois);
        let world = World {
            blds,
            areas,
            pois,
            campuses,
            extra: Vec::new(),
            orphans: Vec::new(),
            rural: Vec::new(),
            loose,
            no_area: 0,
            a_idx,
            c_idx,
            shops,
        };
        (world, cand)
    }

    /// The areas containing a point, smallest first.
    fn areas_at(&self, x: f64, y: f64) -> Vec<usize> {
        let mut found: Vec<usize> = self
            .a_idx
            .at(x, y)
            .iter()
            .copied()
            .filter(|&i| self.areas.get(i).is_some_and(|ar| ar.s.contains(x, y)))
            .collect();
        let size = |i: usize| self.areas.get(i).map_or(f64::NAN, |ar| ar.s.area);
        js_sort_by(&mut found, |&one, &two| size(one) - size(two));
        found
    }

    /// The building's details for classifying it; counts it when it is in no mapped area, and files it with the shop sites it is in.
    fn here(&mut self, bi: usize) -> Option<Here<'a>> {
        let bd = self.blds.get(bi)?;
        let (t, bt, x, y, area, b_in) = (bd.t, bd.b.clone(), bd.x, bd.y, bd.area, bd.inside);
        let sites_at = self.areas_at(x, y);
        let site = sites_at
            .iter()
            .copied()
            .find(|&ai| self.areas.get(ai).is_some_and(|ar| ar.site.is_some()));
        let lu = sites_at
            .iter()
            .copied()
            .find(|&ai| self.areas.get(ai).is_some_and(|ar| ar.lu.is_some()));
        if b_in && site.is_none() && lu.is_none() {
            // Bound: at most one per building
            self.no_area = self.no_area.saturating_add(1);
        }
        // A shop site's own buildings carry its floor area, so the site adds nothing on top
        for &ai in &sites_at {
            if let Some(ar) = self.areas.get_mut(ai)
                && matches!(ar.site, Some("shop" | "supermarket"))
            {
                ar.blds.push(bi);
            }
        }
        let site_kind = site.and_then(|si| self.areas.get(si)).and_then(|ar| ar.site);
        let lu_kind = lu.and_then(|li| self.areas.get(li)).and_then(|ar| ar.lu);
        Some(Here {
            bi,
            t,
            bt,
            am: tag_str(t, "amenity"),
            tourism: tag_str(t, "tourism"),
            x,
            y,
            area,
            sites_at,
            site,
            site_kind,
            lu_kind,
        })
    }

    fn set(&mut self, bi: usize, cls: &'static str, kq: Option<(&'static str, f64)>) {
        if let Some(bd) = self.blds.get_mut(bi) {
            set_fn(bd, cls, kq);
        }
    }

    fn ign(&mut self, bi: usize, why: &str) {
        if let Some(bd) = self.blds.get_mut(bi) {
            ign_fn(bd, why);
        }
    }

    // Floor area: footprint times floors
    fn floor(&self, bi: usize, apt: bool) -> f64 {
        self.blds
            .get(bi)
            .map_or(f64::NAN, |bd| bd.area * levels_of(bd.t, apt))
    }

    // The building's floor area as the land use k
    fn floor_use(&mut self, bi: usize, k: &'static str) {
        let q = self.floor(bi, false);
        self.set(bi, k, Some((k, q)));
    }

    // The first point POI of this kind mapped in or at the building
    fn poi_of(&self, bi: usize, kind: &str) -> Option<usize> {
        self.blds
            .get(bi)?
            .pois
            .iter()
            .copied()
            .find(|&p| self.pois.get(p).is_some_and(|pt| pt.kind == kind))
    }

    fn has(&self, bi: usize, kind: &str) -> bool {
        self.poi_of(bi, kind).is_some()
    }

    fn rooms(&self, bi: usize, t: &Value) -> f64 {
        match count_tag(t, "rooms") {
            Some(c) if c != 0.0 => c,
            _ => math_max(1.0, math_round(self.floor(bi, false) / M2_PER_ROOM)),
        }
    }

    // A house-sized or larger building other than bi within 25 m of the point
    fn big_near(&self, bi: usize, x: f64, y: f64) -> bool {
        self.c_idx.near(x, y, OUTBUILDING_R).iter().any(|&o| {
            o != bi
                && self.blds.get(o).is_some_and(|ob| {
                    ob.area >= HOUSE_MIN && math_hypot(&[ob.x - x, ob.y - y]) <= OUTBUILDING_R
                })
        })
    }

    fn shop_near(&self, x: f64, y: f64, r: f64) -> bool {
        self.shops.near(x, y, r).iter().any(|&pi| {
            self.pois
                .get(pi)
                .is_some_and(|pt| math_hypot(&[pt.x - x, pt.y - y]) <= r)
        })
    }

    fn residential(&mut self, h: &Here<'a>) {
        let bi = h.bi;
        if h.area < HOUSE_MIN && self.big_near(bi, h.x, h.y) {
            self.ign(bi, "outbuilding next to a larger building");
            return;
        }
        // Large untagged footprints are not assumed to be apartment blocks: one dwelling per 200 m², one floor
        if h.area > HOUSE_MAX {
            if tagged(h.t) {
                let q = self.floor(bi, false) / M2_PER_DWELLING;
                self.set(bi, "apartment", Some(("apartment", q)));
            } else {
                self.set(
                    bi,
                    "large residential",
                    Some((
                        "residential",
                        math_max(1.0, math_round(h.area / M2_PER_UNIT)),
                    )),
                );
            }
            return;
        }
        self.set(bi, "residential", Some(("residential", 1.0)));
    }

    // Retail where shops are mapped within 60 m, else office
    fn commercial(&mut self, h: &Here<'a>) {
        let k = if self.shop_near(h.x, h.y, SHOP_NEAR) {
            "retail"
        } else {
            "office"
        };
        self.floor_use(h.bi, k);
    }

    fn building_campus(&mut self, h: &Here<'a>, kind: &'static str) {
        self.campuses.push(Campus {
            kind,
            x: h.x,
            y: h.y,
            t: h.t,
            from: "building",
            area: None,
        });
    }

    // A building with shops mapped in it: general retail if any shop is, else trade/bulky goods, else a workshop
    fn shop_points_use(&self, bi: usize) -> &'static str {
        let u: Vec<&str> = self.blds.get(bi).map_or(Vec::new(), |bd| {
            bd.pois
                .iter()
                .filter_map(|&p| self.pois.get(p))
                .filter(|pt| pt.kind == "shop")
                .map(|pt| shop_use(tag_str(pt.t, "shop")))
                .collect()
        });
        if u.contains(&"retail") {
            "retail"
        } else if u.contains(&"warehouse") {
            "warehouse"
        } else {
            "industrial"
        }
    }

    // Clinics mapped as points add 300 m² each, unless the building or its site is already the clinic
    fn point_clinics(&mut self, h: &Here<'a>) {
        let Some(bd) = self.blds.get(h.bi) else {
            return;
        };
        let in_clinic_site = bd.cls == Some("medical") && h.site_kind == Some("medical");
        if bd.own_med || in_clinic_site {
            return;
        }
        for pt in bd.pois.iter().filter_map(|&p| self.pois.get(p)) {
            if pt.kind == "medical" {
                self.extra.push(Item {
                    x: h.x,
                    y: h.y,
                    k: "medical",
                    q: MEDICAL_M2,
                    street: street_of(pt.t),
                });
            }
        }
    }

    // A medical site contains the point
    fn in_med_site(&self, p: &Poi<'_>) -> bool {
        self.areas_at(p.x, p.y)
            .iter()
            .any(|&a| self.areas.get(a).is_some_and(|ar| ar.site == Some("medical")))
    }
}

/// Classifies one candidate building: (1) its own tags, (2) shops, clinics and schools mapped as points in or at it, (3) the site or
/// land-use area it sits in, (4) outside any mapped area, by size; then adds the clinics mapped in it.
fn classify_building(w: &mut World<'_>, bi: usize) {
    let Some(h) = w.here(bi) else {
        return;
    };
    if !(by_use_tags(w, &h) || by_shop_tags(w, &h) || by_points(w, &h) || by_site(w, &h)) {
        by_size(w, &h);
    }
    w.point_clinics(&h);
}

/// (1) the building's own tags: community uses, hotels, schools, clinics and hospitals. False when none applies.
fn by_use_tags<'a>(w: &mut World<'a>, h: &Here<'a>) -> bool {
    let (bi, tags, bt, am) = (h.bi, h.t, h.bt.as_str(), h.am);
    if OTHER_B.contains(&bt) || am.is_some_and(|amenity| OTHER_A.contains(&amenity)) {
        w.ign(bi, "community or other use");
    } else if bt == "hotel" || h.tourism == Some("hotel") || h.tourism == Some("motel") {
        let rooms = w.rooms(bi, tags);
        w.set(bi, "hotel", Some(("hotel", rooms)));
    } else if in_set(&EDU_A, am) {
        w.set(bi, "education", None);
        w.building_campus(h, "school");
    } else if in_set(&TERT_A, am) {
        w.set(bi, "tertiary", None);
        w.building_campus(h, "tertiary");
    } else if in_set(&CHILD_A, am) {
        w.set(bi, "childcare", None);
        w.building_campus(h, "childcare");
    } else if in_set(&MED_A, am) {
        w.set(bi, "medical", None);
        if let Some(bd) = w.blds.get_mut(bi) {
            bd.own_med = true;
        }
        w.extra.push(Item {
            x: h.x,
            y: h.y,
            k: "medical",
            q: MEDICAL_M2,
            street: street_of(tags),
        });
        for &ai in &h.sites_at {
            if let Some(ar) = w.areas.get_mut(ai)
                && ar.site == Some("medical")
            {
                ar.has_med = true; // the site is this clinic
            }
        }
    } else if am == Some("hospital") || bt == "hospital" {
        w.floor_use(bi, "medical");
    } else {
        return false;
    }
    true
}

/// (1) the building's own tags, continued: shops, offices, industry, schools, houses and commercial buildings. False when none applies.
fn by_shop_tags<'a>(w: &mut World<'a>, h: &Here<'a>) -> bool {
    let (bi, tags, bt) = (h.bi, h.t, h.bt.as_str());
    if h.am == Some("fast_food") {
        w.floor_use(bi, "fastfood");
    } else if bt == "supermarket"
        || tag_str(tags, "shop") == Some("supermarket")
        || w.has(bi, "supermarket")
    {
        w.floor_use(bi, "supermarket");
    } else if RETAIL_B.contains(&bt) || tag_truthy(tags, "shop") {
        let k = if tag_truthy(tags, "shop") {
            shop_use(tag_str(tags, "shop"))
        } else {
            "retail"
        };
        w.floor_use(bi, k);
    } else if bt == "office" || tag_truthy(tags, "office") {
        w.floor_use(bi, "office");
    } else if bt == "warehouse" {
        w.floor_use(bi, "warehouse");
    } else if IND_B.contains(&bt)
        || tag_truthy(tags, "industrial")
        || tag_truthy(tags, "craft")
        || word(tags, "landuse", "industrial")
    {
        w.floor_use(bi, "industrial");
    } else if in_set(&EDU_B, Some(bt)) || in_set(&TERT_B, Some(bt)) || in_set(&CHILD_B, Some(bt))
    {
        let cls = if in_set(&CHILD_B, Some(bt)) {
            "childcare"
        } else if in_set(&TERT_B, Some(bt)) {
            "tertiary"
        } else {
            "education"
        };
        w.set(bi, cls, None);
        if h.site
            .is_none_or(|si| w.areas.get(si).is_none_or(|ar| ar.campus.is_none()))
        {
            w.orphans.push(bi);
        }
    } else if RES_B.contains(&bt) {
        w.set(bi, "residential", Some(("residential", 1.0)));
    } else if bt == "apartments" {
        let qty = math_max(1.0, w.floor(bi, true) / M2_PER_DWELLING);
        w.set(bi, "apartment", Some(("apartment", qty)));
    } else if bt == "terrace" {
        let qty = math_max(1.0, math_round(w.floor(bi, false) / M2_PER_DWELLING));
        w.set(bi, "residential", Some(("residential", qty)));
    } else if bt == "commercial" {
        w.commercial(h);
    } else {
        return false;
    }
    true
}

/// (2) shops, clinics, schools mapped as points in or at the building. False when none applies.
fn by_points<'a>(w: &mut World<'a>, h: &Here<'a>) -> bool {
    let bi = h.bi;
    if w.has(bi, "hotel") {
        // has() has just found the hotel point
        let tags = w
            .poi_of(bi, "hotel")
            .and_then(|pi| w.pois.get(pi))
            .map_or(h.t, |pt| pt.t);
        let rooms = w.rooms(bi, tags);
        w.set(bi, "hotel", Some(("hotel", rooms)));
    } else if w.has(bi, "fastfood") {
        w.floor_use(bi, "fastfood");
    } else if w.has(bi, "shop") {
        let k = w.shop_points_use(bi);
        w.floor_use(bi, k);
    } else if w.has(bi, "medical") {
        w.set(bi, "medical", None);
    } else if w.has(bi, "hospital") {
        w.floor_use(bi, "medical");
    } else if w.has(bi, "childcare") {
        w.set(bi, "childcare", None);
    } else if w.has(bi, "school") {
        w.set(bi, "education", None);
    } else if w.has(bi, "tertiary") {
        w.set(bi, "tertiary", None);
    } else {
        return false;
    }
    true
}

/// (3) the site or land-use area the building sits in. False when it is in neither.
fn by_site<'a>(w: &mut World<'a>, h: &Here<'a>) -> bool {
    let bi = h.bi;
    let sk = h.site_kind;
    if sk == Some("school") {
        w.set(bi, "education", None);
    } else if sk == Some("tertiary") {
        w.set(bi, "tertiary", None);
    } else if sk == Some("childcare") {
        w.set(bi, "childcare", None);
    } else if sk == Some("medical") {
        w.set(bi, "medical", None);
    } else if sk == Some("hospital") {
        w.floor_use(bi, "medical");
    } else if sk == Some("hotel") {
        w.set(bi, "hotel", None);
        // site_kind is set only when site is
        if let Some(si) = h.site
            && let Some(ar) = w.areas.get_mut(si)
        {
            ar.hb.get_or_insert_with(Vec::new).push(bi);
        }
    } else if sk == Some("fastfood") {
        w.floor_use(bi, "fastfood");
    } else if let Some(sa) = h.site {
        let k = if sk == Some("supermarket") {
            "supermarket"
        } else {
            shop_use(w.areas.get(sa).and_then(|ar| tag_str(ar.t, "shop")))
        };
        w.floor_use(bi, k);
    } else {
        return by_land_use(w, h);
    }
    true
}

/// (3) continued: the land-use area the building sits in. False when it is in none.
fn by_land_use<'a>(w: &mut World<'a>, h: &Here<'a>) -> bool {
    match h.lu_kind {
        Some("residential") => w.residential(h),
        Some("commercial") => w.commercial(h),
        Some("retail") => w.floor_use(h.bi, "retail"),
        Some("industrial") => w.floor_use(h.bi, "industrial"),
        Some("education") => {
            w.set(h.bi, "education", None);
            w.orphans.push(h.bi);
        }
        _ => return false,
    }
    true
}

/// (4) outside any mapped area: rural houses by size, shops by proximity.
fn by_size<'a>(w: &mut World<'a>, h: &Here<'a>) {
    if h.bt == "residential" || (HOUSE_MIN..=HOUSE_MAX).contains(&h.area) {
        w.residential(h);
        if w.blds
            .get(h.bi)
            .is_some_and(|bd| bd.cls == Some("residential"))
        {
            w.rural.push(h.bi);
        }
    } else if w.shop_near(h.x, h.y, RURAL_SHOP_NEAR) {
        w.floor_use(h.bi, "retail");
    } else {
        w.ign(
            h.bi,
            if h.area < HOUSE_MIN {
                "small rural building"
            } else {
                "large rural building (farm or yard)"
            },
        );
    }
}

// 6 or more buildings of house size or larger within 100 m of the building
fn dense(blds: &[Bld<'_>], c_idx: &Grid, bi: usize) -> bool {
    let Some(me) = blds.get(bi) else {
        return false;
    };
    let near = c_idx
        .near(me.x, me.y, DENSE_R)
        .iter()
        .filter(|&&o| {
            blds.get(o).is_some_and(|ob| {
                ob.area >= HOUSE_MIN && math_hypot(&[ob.x - me.x, ob.y - me.y]) <= DENSE_R
            })
        })
        .count();
    near >= DENSE_N
}

/* Rural houses: a farmhouse and its sheds sit together, so a house-sized building within 40 m of a house (measured from that house,
never chained from shed to shed) is its outbuilding. Where 6 or more buildings of house size or larger stand within 100 m it is a
settlement (a street with no mapped residential area), and each building is a dwelling. */
fn rural_outbuildings(w: &mut World<'_>, lim: [f64; 4]) {
    let farm: Vec<usize> = w
        .rural
        .iter()
        .copied()
        .filter(|&bi| !dense(&w.blds, &w.c_idx, bi))
        .collect();
    let mut taken: HashSet<usize> = HashSet::new();
    let mut r_idx = Grid::new(100.0, lim);
    for &bi in &farm {
        if let Some(bd) = w.blds.get(bi) {
            r_idx.add([bd.x, bd.y, bd.x, bd.y], bi);
        }
    }
    // The building closest to 150 m² is the house and claims the house-sized sheds around it
    let mut order = farm;
    let area = |i: usize| w.blds.get(i).map_or(f64::NAN, |bd| bd.area);
    let id = |i: usize| w.blds.get(i).map_or(f64::NAN, |bd| bd.id);
    js_sort_by(&mut order, |&p, &q| {
        let diff = (area(p) - 150.0).abs() - (area(q) - 150.0).abs();
        if diff != 0.0 && !diff.is_nan() {
            diff
        } else {
            id(p) - id(q)
        }
    });
    for house in order {
        if taken.contains(&house) {
            continue;
        }
        taken.insert(house);
        let Some((hx, hy)) = w.blds.get(house).map(|bd| (bd.x, bd.y)) else {
            continue;
        };
        for other in r_idx.near(hx, hy, RURAL_GROUP) {
            if !taken.contains(&other)
                && w.blds
                    .get(other)
                    .is_some_and(|ob| math_hypot(&[ob.x - hx, ob.y - hy]) <= RURAL_GROUP)
            {
                taken.insert(other);
                if let Some(ob) = w.blds.get_mut(other) {
                    ign_fn(ob, "rural outbuilding");
                }
            }
        }
    }
}

// Clinic, hotel and shop sites. A clinic site is 300 m² unless a clinic building in it already counts;
// shop sites mapped without buildings use a building cover of their land area (yards excluded)
fn site_items(w: &mut World<'_>, fr: &Frame) {
    for ar in &w.areas {
        if !fr.inside(ar.s.cx, ar.s.cy) {
            continue;
        }
        if ar.site == Some("medical") && !ar.has_med {
            w.extra.push(Item {
                x: ar.s.cx,
                y: ar.s.cy,
                k: "medical",
                q: MEDICAL_M2,
                street: street_of(ar.t),
            });
        }
        if ar.site == Some("hotel") {
            let q = match count_tag(ar.t, "rooms") {
                Some(rooms) if rooms != 0.0 => rooms,
                _ => match &ar.hb {
                    Some(hb) => math_round(
                        hb.iter().fold(0.0, |sum, &bj| {
                            sum + w
                                .blds
                                .get(bj)
                                .map_or(f64::NAN, |bd| bd.area * levels_of(bd.t, false))
                        }) / M2_PER_ROOM,
                    ),
                    None => 0.0,
                },
            };
            if q > 0.0 {
                w.extra.push(Item {
                    x: ar.s.cx,
                    y: ar.s.cy,
                    k: "hotel",
                    q,
                    street: street_of(ar.t),
                });
            }
        }
        if matches!(ar.site, Some("shop" | "supermarket"))
            && ar.blds.is_empty()
            && !tag_str(ar.t, "shop").is_some_and(|s| SHOP_YARD.contains(&s))
        {
            let k = if ar.site == Some("supermarket") {
                "supermarket"
            } else {
                shop_use(tag_str(ar.t, "shop"))
            };
            w.extra.push(Item {
                x: ar.s.cx,
                y: ar.s.cy,
                k,
                q: math_min(10000.0, ar.s.area * SITE_COVER),
                street: street_of(ar.t),
            });
        }
    }
}

// Education buildings with no mapped campus: attach to one within 250 m, else start one
fn orphan_campuses(w: &mut World<'_>) {
    for &bi in &w.orphans {
        let Some(bd) = w.blds.get(bi) else {
            continue;
        };
        let kind = if bd.cls == Some("childcare") {
            "childcare"
        } else if bd.cls == Some("tertiary") {
            "tertiary"
        } else {
            "school"
        };
        if !w
            .campuses
            .iter()
            .any(|c| c.kind == kind && math_hypot(&[c.x - bd.x, c.y - bd.y]) <= CAMPUS_R)
        {
            w.campuses.push(Campus {
                kind,
                x: bd.x,
                y: bd.y,
                t: bd.t,
                from: "building",
                area: None,
            });
        }
    }
}

// Shops, supermarkets, fast food, hotels with rooms and clinics mapped as points in no building; returns how many are in the study area
fn loose_items(w: &mut World<'_>, fr: &Frame) -> usize {
    let mut loose_n = 0usize;
    for &pi in &w.loose {
        let Some(p) = w.pois.get(pi) else {
            continue;
        };
        let (k, q) = match p.kind {
            "shop" => (shop_use(tag_str(p.t, "shop")), SHOP_M2),
            "supermarket" => ("supermarket", SUPERMARKET_M2),
            "fastfood" => ("fastfood", FASTFOOD_M2),
            "hotel" => match count_tag(p.t, "rooms") {
                Some(rooms) if rooms != 0.0 => ("hotel", rooms),
                _ => continue,
            },
            "medical" if !w.in_med_site(p) => ("medical", MEDICAL_M2),
            _ => continue,
        };
        w.extra.push(Item {
            x: p.x,
            y: p.y,
            k,
            q,
            street: street_of(p.t),
        });
        if fr.inside(p.x, p.y) {
            // Bound: at most one per POI
            loose_n = loose_n.saturating_add(1);
        }
    }
    loose_n
}

// One campus each: a point or tagged building inside a mapped campus area, very close to another, with the same name nearby, or
// unnamed within 250 m of another of its kind is the same campus. Returns the positions of the campuses kept.
fn kept_campuses(campuses: &mut [Campus<'_>], areas: &[Area<'_>]) -> Vec<usize> {
    let rank = |from: &str| match from {
        "area" => 0.0,
        "building" => 1.0,
        _ => 2.0,
    };
    js_sort_by(campuses, |one, two| rank(one.from) - rank(two.from));
    let mut kept: Vec<usize> = Vec::new();
    for (ci, cur) in campuses.iter().enumerate() {
        let dup = kept.iter().any(|&ki| {
            // kept holds positions in campuses
            let Some(prev) = campuses.get(ki) else {
                return false;
            };
            if prev.kind != cur.kind {
                return false;
            }
            let dist = math_hypot(&[prev.x - cur.x, prev.y - cur.y]);
            let in_area = prev
                .area
                .is_some_and(|ai| areas.get(ai).is_some_and(|ar| ar.s.contains(cur.x, cur.y)));
            let cname = tag_truthy(cur.t, "name");
            in_area
                || dist < 50.0
                || (cname && tag_text(cur.t, "name") == tag_text(prev.t, "name") && dist <= CAMPUS_R)
                || (!cname && cur.from != "area" && dist <= CAMPUS_R)
        });
        if !dup {
            kept.push(ci);
        }
    }
    kept
}

// The students and children of the campuses kept, in the study area
fn campus_items(w: &mut World<'_>, kept: &[usize], fr: &Frame, stats: &mut Stats) {
    for &ci in kept {
        let Some(c) = w.campuses.get(ci) else {
            continue;
        };
        if !fr.inside(c.x, c.y) {
            continue;
        }
        let name_or = |dflt: &str| {
            if tag_truthy(c.t, "name") {
                tag_text(c.t, "name")
            } else {
                dflt.to_string()
            }
        };
        if c.kind == "school" {
            let n = students(c.t);
            w.extra.push(Item {
                x: c.x,
                y: c.y,
                k: "school",
                q: n,
                street: street_of(c.t),
            });
            stats.schools.push(CampusStat {
                name: name_or("Unnamed school"),
                students: n,
                src: c.from.to_string(),
            });
        } else if c.kind == "tertiary" {
            let cap = count_tag(c.t, "capacity")
                .filter(|v| *v != 0.0)
                .or_else(|| count_tag(c.t, "students").filter(|v| *v != 0.0))
                .unwrap_or(TERTIARY);
            let n = math_round(cap);
            w.extra.push(Item {
                x: c.x,
                y: c.y,
                k: "tertiary",
                q: n,
                street: street_of(c.t),
            });
            stats.tertiary.push(CampusStat {
                name: name_or("Unnamed campus"),
                students: n,
                src: c.from.to_string(),
            });
        } else {
            let cap = count_tag(c.t, "capacity")
                .filter(|v| *v != 0.0)
                .unwrap_or(CHILDREN);
            w.extra.push(Item {
                x: c.x,
                y: c.y,
                k: "childcare",
                q: math_round(cap),
                street: street_of(c.t),
            });
        }
    }
}

// Counts the buildings inside the study area; returns the activity of those used, then the other activity in the study area
fn building_items(blds: &[Bld<'_>], extra: Vec<Item>, fr: &Frame, stats: &mut Stats) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    for bd in blds {
        if !bd.inside {
            stats.outside += 1.0;
            continue;
        }
        stats.buildings += 1.0;
        if let Some(cls) = bd.cls {
            stats.used += 1.0;
            bump(&mut stats.by_class, cls);
            if let (Some(k), Some(q)) = (bd.k, bd.q)
                && q > 0.0
            {
                items.push(Item {
                    x: bd.x,
                    y: bd.y,
                    k,
                    q,
                    street: bd.street.clone(),
                });
            }
        } else {
            stats.ignored += 1.0;
            bump(&mut stats.ignored_by, &bd.why);
        }
    }
    for it in extra {
        if fr.inside(it.x, it.y) && it.q > 0.0 {
            items.push(it);
        }
    }
    items
}

// The totals by land use in the statistics
fn totals(items: &[Item], stats: &mut Stats) {
    let mut tot: HashMap<&str, f64> = WEIGHT.iter().map(|(k, _)| (*k, 0.0)).collect();
    for it in items {
        // Every item's key is one of WEIGHT's
        if let Some(v) = tot.get_mut(it.k) {
            *v += it.q;
        }
    }
    let tt = |k: &str| tot.get(k).copied().unwrap_or(f64::NAN);
    stats.dwellings = math_round(tt("residential") + tt("apartment"));
    stats.gfa = ["retail", "office", "industrial", "warehouse", "fastfood"]
        .iter()
        .map(|k| (k.to_string(), math_round(tt(k))))
        .collect();
    stats.students = math_round(tt("school"));
    stats.children = math_round(tt("childcare"));
    stats.medical = math_round(tt("medical"));
    stats.supermarket = math_round(tt("supermarket"));
    stats.tertiary_students = math_round(tt("tertiary"));
    stats.hotel_rooms = math_round(tt("hotel"));
}

// The position of a land-use key in WEIGHT
fn kix(k: &str) -> Option<usize> {
    WEIGHT.iter().position(|(key, _)| *key == k)
}

// The activity summed into grid cells; the cells with any, north to south and west to east
fn grid_cells(items: &[Item], fr: &Frame, cell: f64) -> Vec<Cell> {
    let nx = math_max(1.0, (fr.width / cell).ceil());
    let ny = math_max(1.0, (fr.height / cell).ceil());
    let mut cells: Vec<Cell> = Vec::new();
    let mut cell_ix: HashMap<u64, usize> = HashMap::new();
    for it in items {
        let ci = math_min(nx - 1.0, (it.x / cell).floor());
        let cj = math_min(ny - 1.0, (it.y / cell).floor());
        let key = cj * nx + ci;
        let at = if let Some(&at) = cell_ix.get(&key.to_bits()) {
            at
        } else {
            let at = cells.len();
            cell_ix.insert(key.to_bits(), at);
            cells.push(Cell {
                i: ci,
                j: cj,
                w: 0.0,
                sx: 0.0,
                sy: 0.0,
                lu: vec![0.0; WEIGHT.len()],
                streets: Vec::new(),
            });
            at
        };
        // at is a position in cells, given just above or earlier
        let Some(c) = cells.get_mut(at) else {
            continue;
        };
        let wt = it.q * weight(it.k);
        c.w += wt;
        c.sx += it.x * wt;
        c.sy += it.y * wt;
        if let Some(ki) = kix(it.k)
            && let Some(slot) = c.lu.get_mut(ki)
        {
            *slot += it.q;
        }
        if let Some(st) = &it.street {
            match c.streets.iter_mut().find(|(name, _)| name == st) {
                Some((_, v)) => *v += wt,
                None => c.streets.push((st.clone(), wt)),
            }
        }
    }
    let mut live: Vec<Cell> = cells.into_iter().filter(|c| c.w > 0.0).collect();
    js_sort_by(&mut live, |one, two| {
        let d = two.j - one.j;
        if d != 0.0 && !d.is_nan() {
            d
        } else {
            one.i - two.i
        }
    });
    live
}

// Merge cells with little activity into the nearest cell (by activity-weighted centre)
fn merge_cells(live: &mut Vec<Cell>, stats: &mut Stats) {
    loop {
        let mut small: Option<usize> = None;
        for (ci, c) in live.iter().enumerate() {
            if c.w < MERGE_BELOW && small.and_then(|s| live.get(s)).is_none_or(|sc| c.w < sc.w) {
                small = Some(ci);
            }
        }
        let Some(small) = small.filter(|_| live.len() >= 2) else {
            break;
        };
        let Some((sx, sy)) = live.get(small).map(|sc| (sc.sx / sc.w, sc.sy / sc.w)) else {
            break;
        };
        let mut best: Option<usize> = None;
        let mut bd = f64::INFINITY;
        for (ci, c) in live.iter().enumerate() {
            if ci != small {
                let d = math_hypot(&[c.sx / c.w - sx, c.sy / c.w - sy]);
                if d < bd {
                    bd = d;
                    best = Some(ci);
                }
            }
        }
        // Cells with activity have finite centres, so another cell is always nearer than infinity; stop if not
        let Some(best) = best else {
            break;
        };
        let sm = live.remove(small);
        let best = if best > small {
            best.saturating_sub(1)
        } else {
            best
        };
        let Some(bc) = live.get_mut(best) else {
            break;
        };
        bc.w += sm.w;
        bc.sx += sm.sx;
        bc.sy += sm.sy;
        // Both hold one quantity per land use, in WEIGHT's order
        for (slot, v) in bc.lu.iter_mut().zip(&sm.lu) {
            *slot += *v;
        }
        for (street, wv) in sm.streets {
            match bc.streets.iter_mut().find(|(name, _)| *name == street) {
                Some((_, v)) => *v += wv,
                None => bc.streets.push((street, wv)),
            }
        }
        stats.merged += 1.0;
    }
}

// Named roads for zones with no addresses, in local metres
fn named_roads<'o>(opts: &'o BuildOpts, fr: &Frame) -> Vec<(&'o str, Vec<Pt>)> {
    opts.links
        .iter()
        .filter(|l| !l.name.is_empty() && l.pts.len() > 1)
        .map(|l| {
            (
                l.name.as_str(),
                l.pts.iter().map(|p| fr.local(p[0], p[1])).collect(),
            )
        })
        .collect()
}

// The road with a segment nearest the point, within 1 km
fn nearest_road<'r>(roads: &[(&'r str, Vec<Pt>)], x: f64, y: f64) -> Option<&'r str> {
    let mut best: Option<&'r str> = None;
    let mut bd = 1000.0;
    for (name, xy) in roads {
        for seg in xy.windows(2) {
            let [p0, p1] = seg else {
                continue;
            };
            let d = util::seg_dist(x, y, p0[0], p0[1], p1[0], p1[1]).d;
            if d < bd {
                bd = d;
                best = Some(*name);
            }
        }
    }
    best
}

// A cell's quantities as a zone's land use: rounded (to tens for quantities not counted one by one), zeros left out
fn zone_land_use(c: &Cell) -> OrderedMap<Num> {
    let mut lu: OrderedMap<Num> = OrderedMap::new();
    // The cell holds one quantity per land use, in WEIGHT's order
    for ((k, _), &q) in WEIGHT.iter().zip(&c.lu) {
        let per1 = LANDUSES
            .iter()
            .find(|entry| entry.key == *k)
            .is_some_and(|entry| entry.per == 1.0);
        let v = if per1 {
            math_round(q)
        } else {
            math_round(q / 10.0) * 10.0
        };
        if v > 0.0 {
            lu.insert(*k, Num::number(v));
        }
    }
    lu
}

// The street with the most activity in a cell (ties by name)
fn zone_street(c: &Cell) -> Option<String> {
    let mut sorted: Vec<&(String, f64)> = c.streets.iter().collect();
    js_sort_by(&mut sorted, |one, two| {
        let d = two.1 - one.1;
        if d != 0.0 && !d.is_nan() {
            d
        } else if js_str_lt(&one.0, &two.0) {
            -1.0
        } else {
            1.0
        }
    });
    sorted.first().map(|st| st.0.clone())
}

fn new_zone(id: String, name: String, lat: f64, lng: f64, lu: OrderedMap<Num>) -> Zone {
    Zone {
        id,
        kind: Maybe::Set("zone".to_string()),
        name: Maybe::Set(name),
        lat: Num::number(lat),
        lng: Num::number(lng),
        lu: Maybe::Set(lu),
        connectors: Maybe::Null,
        role: Maybe::Missing,
        year: Maybe::Missing,
        buildout: Maybe::Missing,
        existing: Maybe::Set(JsNum::Bool(true)),
        src: Maybe::Missing,
        node: Maybe::Missing,
        gw: Maybe::Missing,
        gw_src: Maybe::Missing,
        tg: Maybe::Missing,
        tg_load: Maybe::Missing,
        extra_trips: Maybe::Missing,
        reduction: Maybe::Missing,
        reg: Maybe::Missing,
        extra: Map::new(),
    }
}

// A zone for each cell with land use, named after its street, else the nearest road, else "Area n" (numbered when a name repeats)
fn make_zones(
    live: &[Cell],
    roads: &[(&str, Vec<Pt>)],
    fr: &Frame,
    ids: &mut dyn IdSource,
) -> Vec<Zone> {
    let mut zones: Vec<Zone> = Vec::new();
    let mut used: Vec<(String, f64)> = Vec::new();
    let mut area_n = 0.0;
    for c in live {
        let lu = zone_land_use(c);
        if lu.is_empty() {
            continue;
        }
        let x = c.sx / c.w;
        let y = c.sy / c.w;
        let mut base: String = match zone_street(c) {
            Some(st) => st,
            None => nearest_road(roads, x, y)
                .map(str::to_string)
                .unwrap_or_default(),
        };
        if base.is_empty() {
            area_n += 1.0;
            base = format!("Area {}", number_to_js_string(area_n));
        }
        let nth = if let Some((_, seen)) = used.iter_mut().find(|(key, _)| *key == base) {
            *seen += 1.0;
            *seen
        } else {
            used.push((base.clone(), 1.0));
            1.0
        };
        let name = format!(
            "Existing \u{b7} {}{}",
            base,
            if nth > 1.0 {
                format!(" {}", number_to_js_string(nth))
            } else {
                String::new()
            }
        );
        let id = ids.uid("z");
        zones.push(new_zone(
            id,
            name,
            r7(fr.bx.s + y / fr.ky),
            r7(fr.bx.w + x / fr.kx),
            lu,
        ));
    }
    zones
}

// Every assumption, in words, in the statistics
fn assumptions(stats: &mut Stats, no_area: usize, loose_n: usize, cell: f64) {
    let a = &mut stats.assumptions;
    a.push("Indicative only: building outlines from OpenStreetMap (LINZ import). Check dwelling and floor-area totals against council or Stats NZ data before relying on them.".to_string());
    a.push("Untagged buildings are classed by the land-use area they sit in (residential, commercial, retail, industrial, education); clinics, shops and schools mapped inside a building take precedence.".to_string());
    a.push(format!("Garages, sheds, glasshouses, roofs, tanks and similar are ignored, as are footprints under {} m\u{b2} and buildings under {} m\u{b2} within {} m of a larger one (outbuildings).", n0(MIN_FOOT), n0(HOUSE_MIN), n0(OUTBUILDING_R)));
    a.push(format!("Houses: 1 dwelling each. Residential buildings over {} m\u{b2} footprint with mapped levels or height count as apartments at {} m\u{b2} gross floor area per dwelling; without them, one dwelling per {} m\u{b2} of footprint.", n0(HOUSE_MAX), n0(M2_PER_DWELLING), n0(M2_PER_UNIT)));
    a.push("Floors from building:levels, else height \u{f7} 3 m, else 1 (buildings tagged apartments 3). Retail, office, industrial and warehouse GFA = footprint \u{d7} floors.".to_string());
    a.push("Shops that are workshops (car repair, tyres) count as industrial; trade and bulky-goods shops (hardware, car yards, furniture, garden centres and similar) count as warehouse; other shops as retail.".to_string());
    a.push(format!("Hotels and motels from the rooms tag, else one room per {} m\u{b2} of floor area; universities and colleges are tertiary campuses ({} students unless tagged); fast food {} m\u{b2} for a point with no building.", n0(M2_PER_ROOM), fmt::int(TERTIARY), n0(FASTFOOD_M2)));
    a.push(format!(
        "Commercial areas count as retail where shops are mapped within {} m, otherwise office.",
        n0(SHOP_NEAR)
    ));
    a.push(format!("Outside mapped land-use areas, buildings of {}\u{2013}{} m\u{b2} count as rural houses; house-sized buildings within {} m of a house are its sheds, unless {} or more buildings stand within {} m (a street), when each is a dwelling. Others are ignored as farm or yard buildings unless shops are mapped nearby.", n0(HOUSE_MIN), n0(HOUSE_MAX), n0(RURAL_GROUP), DENSE_N, n0(DENSE_R)));
    a.push(format!("Shop sites mapped without buildings: {}% of the site area as floor area (up to 10,000 m\u{b2}); car yards, garden centres and trade yards add none.", n0(math_round(SITE_COVER * 100.0))));
    if stats.buildings != 0.0 && usize_to_f64(no_area) > stats.buildings / 2.0 {
        a.push(format!("Warning: {} of {} buildings are outside any mapped land-use area, so they were classed by size alone. Check the totals, or map the residential and industrial areas in OpenStreetMap.", fmt::int(usize_to_f64(no_area)), fmt::int(stats.buildings)));
    }
    a.push("School rolls from the capacity tag, else 350 (years 1\u{2013}8 or primary), 800 (College/High or secondary years), else 400 students.".to_string());
    a.push(format!(
        "Early childhood centres {} children; doctors, clinics and dentists {} m\u{b2} GFA each.",
        n0(CHILDREN),
        n0(MEDICAL_M2)
    ));
    if loose_n != 0 {
        a.push(format!("{} shop or clinic point{} with no building given a default floor area (shop {} m\u{b2}, supermarket {} m\u{b2}, clinic {} m\u{b2}).", loose_n, if loose_n > 1 { "s" } else { "" }, n0(SHOP_M2), fmt::int(SUPERMARKET_M2), n0(MEDICAL_M2)));
    }
    if let Some((_, n)) = stats
        .ignored_by
        .iter()
        .find(|(k, _)| k == "community or other use")
    {
        let n = *n;
        a.push(format!("{} churches, halls, clubrooms and similar have no Spare Construct land use and are left out.", number_to_js_string(n)));
    }
    a.push(format!("Grouped on a {} m grid at activity-weighted centres; cells under {} dwelling-equivalents merged into the nearest cell.", number_to_js_string(cell), n0(MERGE_BELOW)));
}

fn n0(x: f64) -> String {
    number_to_js_string(x)
}

fn ign_fn(b: &mut Bld, why: &str) {
    b.cls = None;
    b.k = None;
    b.why = why.to_string();
}

fn set_fn(b: &mut Bld, cls: &'static str, kq: Option<(&'static str, f64)>) {
    b.cls = Some(cls);
    if let Some((k, q)) = kq {
        b.k = Some(k);
        b.q = Some(q);
    } else {
        b.k = None;
        b.q = None;
    }
}

/// Every constant of the build: the numbers and the tag sets, as JSON (sets sorted), for checking against the prototype's source.
#[must_use]
pub fn constants() -> Value {
    let mut m = Map::new();
    let numbers: [(&str, f64); 25] = [
        ("TILE", super::TILE),
        ("LU_TILE", super::LU_TILE),
        ("MAX_AREA", super::MAX_AREA),
        ("MIN_FOOT", MIN_FOOT),
        ("HOUSE_MIN", HOUSE_MIN),
        ("HOUSE_MAX", HOUSE_MAX),
        ("M2_PER_DWELLING", M2_PER_DWELLING),
        ("M2_PER_UNIT", M2_PER_UNIT),
        ("M2_PER_ROOM", M2_PER_ROOM),
        ("OUTBUILDING_R", OUTBUILDING_R),
        ("SHOP_NEAR", SHOP_NEAR),
        ("RURAL_SHOP_NEAR", RURAL_SHOP_NEAR),
        ("POI_SNAP", POI_SNAP),
        ("CAMPUS_R", CAMPUS_R),
        ("RURAL_GROUP", RURAL_GROUP),
        ("DENSE_R", DENSE_R),
        ("DENSE_N", usize_to_f64(DENSE_N)),
        ("MEDICAL_M2", MEDICAL_M2),
        ("CHILDREN", CHILDREN),
        ("SHOP_M2", SHOP_M2),
        ("SUPERMARKET_M2", SUPERMARKET_M2),
        ("FASTFOOD_M2", FASTFOOD_M2),
        ("TERTIARY", TERTIARY),
        ("SITE_COVER", SITE_COVER),
        ("MERGE_BELOW", MERGE_BELOW),
    ];
    for (k, v) in numbers {
        m.insert(k.to_string(), json!(v));
    }
    m.insert(
        "WEIGHT".to_string(),
        Value::Object(
            WEIGHT
                .iter()
                .map(|(k, w)| (k.to_string(), json!(w)))
                .collect(),
        ),
    );
    m.insert(
        "KEYS".to_string(),
        json!(WEIGHT.iter().map(|(k, _)| *k).collect::<Vec<_>>()),
    );
    let set = |a: &[&str]| {
        let mut v: Vec<&str> = a.to_vec();
        v.sort_unstable();
        json!(v)
    };
    let ignore: Vec<&str> = IGNORE.iter().chain(IGNORE2.iter()).copied().collect();
    let sets: [(&str, Vec<&str>); 17] = [
        ("IGNORE", ignore),
        ("OTHER_B", OTHER_B.to_vec()),
        ("OTHER_A", OTHER_A.to_vec()),
        ("RES_B", RES_B.to_vec()),
        ("RETAIL_B", RETAIL_B.to_vec()),
        ("IND_B", IND_B.to_vec()),
        ("EDU_B", EDU_B.to_vec()),
        ("TERT_B", TERT_B.to_vec()),
        ("CHILD_B", CHILD_B.to_vec()),
        ("EDU_A", EDU_A.to_vec()),
        ("TERT_A", TERT_A.to_vec()),
        ("CHILD_A", CHILD_A.to_vec()),
        ("MED_A", MED_A.to_vec()),
        ("SHOP_IND", SHOP_IND.to_vec()),
        ("SHOP_BULKY", SHOP_BULKY.to_vec()),
        ("SHOP_YARD", SHOP_YARD.to_vec()),
        (
            "LU_CLASS",
            vec![
                "residential",
                "commercial",
                "retail",
                "industrial",
                "education",
            ],
        ),
    ];
    for (k, v) in sets {
        m.insert(k.to_string(), set(&v));
    }
    Value::Object(m)
}
