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
    value_to_js_string, value_to_number, value_truthy,
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
    let raw: &[Value] = match elements {
        Value::Array(a) => a,
        other => other
            .get("elements")
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice),
    };
    // De-duplicate and order by id so the result doesn't depend on download order
    let mut by_id: Vec<(String, &Value)> = Vec::new();
    let mut by_ix: HashMap<String, usize> = HashMap::new();
    for e in raw {
        if !e.is_object()
            || !e.get("type").is_some_and(value_truthy)
            || e.get("id").is_none_or(Value::is_null)
        {
            continue;
        }
        let key = format!(
            "{}/{}",
            value_to_js_string(&e["type"]),
            value_to_js_string(&e["id"])
        );
        match by_ix.get(&key) {
            Some(&i) => by_id[i].1 = e,
            None => {
                by_ix.insert(key.clone(), by_id.len());
                by_id.push((key, e));
            }
        }
    }
    let mut els: Vec<&Value> = by_id.into_iter().map(|(_, e)| e).collect();
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
    let mut b = bounds.copied().filter(StudyBox::is_valid);
    if b.is_none() {
        let mut bx = StudyBox {
            s: f64::INFINITY,
            w: f64::INFINITY,
            n: f64::NEG_INFINITY,
            e: f64::NEG_INFINITY,
        };
        for e in &els {
            let points: Vec<&Value> = match e.get("geometry") {
                Some(g) if value_truthy(g) => {
                    g.as_array().map_or(Vec::new(), |a| a.iter().collect())
                }
                _ => {
                    if e.get("lat").is_some_and(|v| !v.is_null()) {
                        vec![*e]
                    } else {
                        Vec::new()
                    }
                }
            };
            for p in points {
                if !value_truthy(p) {
                    continue;
                }
                let lat = p.get("lat").map_or(f64::NAN, value_to_number);
                let lon = p.get("lon").map_or(f64::NAN, value_to_number);
                bx.s = math_min(bx.s, lat);
                bx.n = math_max(bx.n, lat);
                bx.w = math_min(bx.w, lon);
                bx.e = math_max(bx.e, lon);
            }
        }
        b = Some(bx);
    }
    let bx = b.expect("a box");
    let mut stats = Stats::start();
    if !bx.s.is_finite() || els.is_empty() {
        return BuildResult {
            zones: Vec::new(),
            stats,
        };
    }
    let f = util::flat((bx.s + bx.n) / 2.0);
    let (kx, ky) = (f.kx, f.ky);
    let w = (bx.e - bx.w) * kx;
    let h = (bx.n - bx.s) * ky;
    let lim = [-2000.0, -2000.0, w + 2000.0, h + 2000.0];
    let to_xy = |lat: f64, lng: f64| -> Pt { [(lng - bx.w) * kx, (lat - bx.s) * ky] };
    let inside = |x: f64, y: f64| x >= 0.0 && x <= w && y >= 0.0 && y <= h;
    let pts_of = |g: Option<&Value>| -> Vec<Pt> {
        g.and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter(|p| {
                        value_truthy(p)
                            && is_finite_value(p.get("lat"))
                            && is_finite_value(p.get("lon"))
                    })
                    .map(|p| to_xy(value_to_number(&p["lat"]), value_to_number(&p["lon"])))
                    .collect()
            })
            .unwrap_or_default()
    };
    let shape = |e: &Value| -> Option<Shape> {
        let (outer, inner): (Vec<Vec<Pt>>, Vec<Vec<Pt>>);
        match e.get("type").and_then(Value::as_str) {
            Some("way") => {
                let r = pts_of(e.get("geometry"));
                if r.len() < 3 {
                    return None;
                }
                outer = vec![r];
                inner = Vec::new();
            }
            Some("relation") => {
                let ms: Vec<&Value> =
                    e.get("members")
                        .and_then(Value::as_array)
                        .map_or(Vec::new(), |a| {
                            a.iter()
                                .filter(|m| {
                                    m.get("type").and_then(Value::as_str) == Some("way")
                                        && m.get("geometry").is_some_and(value_truthy)
                                })
                                .collect()
                        });
                outer = assemble(
                    ms.iter()
                        .filter(|m| m.get("role").and_then(Value::as_str) != Some("inner"))
                        .map(|m| pts_of(m.get("geometry")))
                        .collect(),
                );
                inner = assemble(
                    ms.iter()
                        .filter(|m| m.get("role").and_then(Value::as_str) == Some("inner"))
                        .map(|m| pts_of(m.get("geometry")))
                        .collect(),
                );
                if outer.is_empty() {
                    return None;
                }
            }
            _ => return None,
        }
        let mut bb = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for r in &outer {
            for p in r {
                bb[0] = math_min(bb[0], p[0]);
                bb[1] = math_min(bb[1], p[1]);
                bb[2] = math_max(bb[2], p[0]);
                bb[3] = math_max(bb[3], p[1]);
            }
        }
        let areas: Vec<f64> = outer.iter().map(|r| ring_area(r).abs()).collect();
        let sum = areas.iter().fold(0.0, |a, b| a + b);
        let hole = inner.iter().fold(0.0, |a, r| a + ring_area(r).abs());
        let area = math_max(0.0, sum - hole);
        let mx = areas.iter().copied().fold(f64::NEG_INFINITY, math_max);
        let big = &outer[areas.iter().position(|a| *a == mx).expect("a biggest ring")];
        let mut c = ring_centroid(big);
        if !in_ring(c[0], c[1], big) {
            let n = big.len() as f64;
            c = big
                .iter()
                .fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
        }
        Some(Shape {
            outer,
            inner,
            bb,
            area,
            cx: c[0],
            cy: c[1],
        })
    };

    // Sort features into buildings, areas (land use + amenity/shop sites) and point POIs
    let empty = json!({});
    let mut blds: Vec<Bld> = Vec::new();
    let mut areas: Vec<Area> = Vec::new();
    let mut pois: Vec<Poi> = Vec::new();
    for e in &els {
        let t = e.get("tags").filter(|t| value_truthy(t)).unwrap_or(&empty);
        let kind = kind_of(t);
        let ty = e.get("type").and_then(Value::as_str);
        if ty == Some("node") {
            if let Some(kind) = kind
                && is_finite_value(e.get("lat"))
            {
                let [x, y] = to_xy(
                    value_to_number(&e["lat"]),
                    e.get("lon").map_or(f64::NAN, value_to_number),
                );
                pois.push(Poi { kind, x, y, t });
            }
            continue;
        }
        let s = shape(e);
        if tag_truthy(t, "building") && !word(t, "building", "no") {
            if let Some(s) = s {
                blds.push(Bld {
                    id: value_to_number(&e["id"]),
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
                });
            }
            continue;
        }
        let Some(s) = s else {
            // open way or broken relation carrying a POI: use the mean of its points
            let p = pts_of(e.get("geometry"));
            if let Some(kind) = kind
                && !p.is_empty()
            {
                let n = p.len() as f64;
                pois.push(Poi {
                    kind,
                    x: p.iter().fold(0.0, |a, q| a + q[0]) / n,
                    y: p.iter().fold(0.0, |a, q| a + q[1]) / n,
                    t,
                });
            }
            continue;
        };
        if let Some(site) = kind {
            areas.push(Area {
                site: Some(site),
                lu: None,
                t,
                s,
                blds: Vec::new(),
                campus: None,
                has_med: false,
                hb: None,
            });
        } else if let Some(lu) = lu_class(tag_str(t, "landuse")) {
            areas.push(Area {
                site: None,
                lu: Some(lu),
                t,
                s,
                blds: Vec::new(),
                campus: None,
                has_med: false,
                hb: None,
            });
        }
    }
    let mut a_idx = Grid::new(250.0, lim);
    for (i, a) in areas.iter().enumerate() {
        a_idx.add(a.s.bb, i);
    }
    let areas_at = |areas: &[Area], x: f64, y: f64| -> Vec<usize> {
        let mut v: Vec<usize> = a_idx
            .at(x, y)
            .iter()
            .copied()
            .filter(|&i| areas[i].s.contains(x, y))
            .collect();
        js_sort_by(&mut v, |&p, &q| areas[p].s.area - areas[q].s.area);
        v
    };

    // Building candidates (not ignored by tag or size) and their indexes
    let mut cand: Vec<usize> = Vec::new();
    let mut b_idx = Grid::new(100.0, lim);
    let mut c_idx = Grid::new(100.0, lim);
    for (i, bd) in blds.iter_mut().enumerate() {
        bd.x = bd.s.cx;
        bd.y = bd.s.cy;
        bd.area = bd.s.area;
        bd.inside = inside(bd.x, bd.y);
        bd.street = if tag_truthy(bd.t, "addr:street") {
            Some(tag_text(bd.t, "addr:street"))
        } else {
            None
        };
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
    // Point POIs: snap to the building they sit in, or the one whose outline is nearest within 25 m
    // (points are often mapped at the entrance, outside the wall of a large building)
    let mut loose: Vec<usize> = Vec::new();
    for (pi, p) in pois.iter().enumerate() {
        let mut hit: Option<usize> = b_idx
            .at(p.x, p.y)
            .iter()
            .copied()
            .find(|&i| blds[i].s.contains(p.x, p.y));
        if hit.is_none() {
            let mut best = POI_SNAP;
            for i in b_idx.near(p.x, p.y, POI_SNAP) {
                let d = blds[i].s.edge(p.x, p.y);
                if d < best {
                    best = d;
                    hit = Some(i);
                }
            }
        }
        match hit {
            Some(i) => blds[i].pois.push(pi),
            None => loose.push(pi),
        }
    }
    // General retail and supermarkets (not workshops or trade suppliers) make a commercial area retail
    let mut shops = Grid::new(100.0, lim);
    for (pi, p) in pois.iter().enumerate() {
        if p.kind == "supermarket"
            || (p.kind == "shop" && shop_use(tag_str(p.t, "shop")) == "retail")
        {
            shops.add([p.x, p.y, p.x, p.y], pi);
        }
    }
    let shop_near = |x: f64, y: f64, r: f64| {
        shops
            .near(x, y, r)
            .iter()
            .any(|&pi| math_hypot(&[pois[pi].x - x, pois[pi].y - y]) <= r)
    };

    // Campuses (schools, tertiary, early childhood) and medical/hospital/hotel/shop sites
    let mut campuses: Vec<Campus> = Vec::new();
    for (ai, a) in areas.iter().enumerate() {
        if let Some(site) = a.site
            && matches!(site, "school" | "tertiary" | "childcare")
        {
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
    // the campus made for each area, in the area's `campus` slot
    {
        let mut next = 0usize;
        for (ai, a) in areas.iter_mut().enumerate() {
            if a.site
                .is_some_and(|s| matches!(s, "school" | "tertiary" | "childcare"))
            {
                debug_assert_eq!(campuses[next].area, Some(ai));
                a.campus = Some(next);
                next += 1;
            }
        }
    }
    for p in &pois {
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
    let mut extra: Vec<Item> = Vec::new();
    let street_of = |t: &Value| {
        if tag_truthy(t, "addr:street") {
            Some(tag_text(t, "addr:street"))
        } else {
            None
        }
    };

    let mut orphans: Vec<usize> = Vec::new();
    let mut rural: Vec<usize> = Vec::new();
    let mut no_area = 0usize;
    for &bi in &cand {
        let t = blds[bi].t;
        let bt = blds[bi].b.clone();
        let am = tag_str(t, "amenity");
        let (bx_, by_, barea, b_in) = (blds[bi].x, blds[bi].y, blds[bi].area, blds[bi].inside);
        let has =
            |blds: &[Bld], pois: &[Poi], k: &str| blds[bi].pois.iter().any(|&p| pois[p].kind == k);
        let poi_of = |blds: &[Bld], pois: &[Poi], k: &str| {
            blds[bi].pois.iter().copied().find(|&p| pois[p].kind == k)
        };
        let sites_at = areas_at(&areas, bx_, by_);
        let site = sites_at.iter().copied().find(|&a| areas[a].site.is_some());
        let lu = sites_at.iter().copied().find(|&a| areas[a].lu.is_some());
        if b_in && site.is_none() && lu.is_none() {
            no_area += 1;
        }
        // A shop site's own buildings carry its floor area, so the site adds nothing on top
        for &a in &sites_at {
            if matches!(areas[a].site, Some("shop" | "supermarket")) {
                areas[a].blds.push(bi);
            }
        }
        let floor = |blds: &[Bld], bj: usize, apt: bool| blds[bj].area * levels_of(blds[bj].t, apt);
        let tagged = |t: &Value| {
            num(&tag_text(t, "building:levels")).is_some_and(|l| l >= 1.0)
                || num(&tag_text(t, "height")).is_some_and(|h| h >= 4.5)
        };
        let rooms = |blds: &[Bld], t: &Value| -> f64 {
            match count_tag(t, "rooms") {
                Some(c) if c != 0.0 => c,
                _ => math_max(1.0, math_round(floor(blds, bi, false) / M2_PER_ROOM)),
            }
        };
        macro_rules! set_use {
            ($cls:expr) => {{
                blds[bi].cls = Some($cls);
                blds[bi].k = None;
                blds[bi].q = None;
            }};
            ($cls:expr, $k:expr, $q:expr) => {{
                let q: f64 = $q;
                blds[bi].cls = Some($cls);
                blds[bi].k = Some($k);
                blds[bi].q = Some(q);
            }};
        }
        macro_rules! ign {
            ($why:expr) => {{
                blds[bi].cls = None;
                blds[bi].k = None;
                blds[bi].why = $why.to_string();
            }};
        }
        macro_rules! shop_as {
            ($k:expr) => {{
                let k: &'static str = $k;
                let q = floor(&blds, bi, false);
                set_use!(k, k, q)
            }};
        }
        // (1) the building's own tags
        let tourism = tag_str(t, "tourism");
        let big_near = |blds: &[Bld]| {
            c_idx.near(bx_, by_, OUTBUILDING_R).iter().any(|&o| {
                o != bi
                    && blds[o].area >= HOUSE_MIN
                    && math_hypot(&[blds[o].x - bx_, blds[o].y - by_]) <= OUTBUILDING_R
            })
        };
        let residential = |blds: &mut Vec<Bld>| {
            if barea < HOUSE_MIN && big_near(blds) {
                ign_fn(&mut blds[bi], "outbuilding next to a larger building");
                return;
            }
            // Large untagged footprints are not assumed to be apartment blocks: one dwelling per 200 m², one floor
            if barea > HOUSE_MAX {
                if tagged(t) {
                    let q = blds[bi].area * levels_of(t, false) / M2_PER_DWELLING;
                    set_fn(&mut blds[bi], "apartment", Some(("apartment", q)));
                } else {
                    set_fn(
                        &mut blds[bi],
                        "large residential",
                        Some((
                            "residential",
                            math_max(1.0, math_round(barea / M2_PER_UNIT)),
                        )),
                    );
                }
                return;
            }
            set_fn(&mut blds[bi], "residential", Some(("residential", 1.0)));
        };
        let site_kind = site.and_then(|s| areas[s].site);
        let lu_kind = lu.and_then(|l| areas[l].lu);
        if OTHER_B.contains(&bt.as_str()) || am.is_some_and(|a| OTHER_A.contains(&a)) {
            ign!("community or other use");
        } else if bt == "hotel" || tourism == Some("hotel") || tourism == Some("motel") {
            let q = rooms(&blds, t);
            set_use!("hotel", "hotel", q);
        } else if in_set(&EDU_A, am) {
            set_use!("education");
            campuses.push(Campus {
                kind: "school",
                x: bx_,
                y: by_,
                t,
                from: "building",
                area: None,
            });
        } else if in_set(&TERT_A, am) {
            set_use!("tertiary");
            campuses.push(Campus {
                kind: "tertiary",
                x: bx_,
                y: by_,
                t,
                from: "building",
                area: None,
            });
        } else if in_set(&CHILD_A, am) {
            set_use!("childcare");
            campuses.push(Campus {
                kind: "childcare",
                x: bx_,
                y: by_,
                t,
                from: "building",
                area: None,
            });
        } else if in_set(&MED_A, am) {
            set_use!("medical");
            blds[bi].own_med = true;
            extra.push(Item {
                x: bx_,
                y: by_,
                k: "medical",
                q: MEDICAL_M2,
                street: street_of(t),
            });
            for &a in &sites_at {
                if areas[a].site == Some("medical") {
                    areas[a].has_med = true; // the site is this clinic
                }
            }
        } else if am == Some("hospital") || bt == "hospital" {
            let q = floor(&blds, bi, false);
            set_use!("medical", "medical", q);
        } else if am == Some("fast_food") {
            shop_as!("fastfood");
        } else if bt == "supermarket"
            || tag_str(t, "shop") == Some("supermarket")
            || has(&blds, &pois, "supermarket")
        {
            shop_as!("supermarket");
        } else if RETAIL_B.contains(&bt.as_str()) || tag_truthy(t, "shop") {
            let k = if tag_truthy(t, "shop") {
                shop_use(tag_str(t, "shop"))
            } else {
                "retail"
            };
            shop_as!(k);
        } else if bt == "office" || tag_truthy(t, "office") {
            let q = floor(&blds, bi, false);
            set_use!("office", "office", q);
        } else if bt == "warehouse" {
            shop_as!("warehouse");
        } else if IND_B.contains(&bt.as_str())
            || tag_truthy(t, "industrial")
            || tag_truthy(t, "craft")
            || word(t, "landuse", "industrial")
        {
            let q = floor(&blds, bi, false);
            set_use!("industrial", "industrial", q);
        } else if in_set(&EDU_B, Some(&bt))
            || in_set(&TERT_B, Some(&bt))
            || in_set(&CHILD_B, Some(&bt))
        {
            let cls = if in_set(&CHILD_B, Some(&bt)) {
                "childcare"
            } else if in_set(&TERT_B, Some(&bt)) {
                "tertiary"
            } else {
                "education"
            };
            set_use!(cls);
            if site.is_none_or(|s| areas[s].campus.is_none()) {
                orphans.push(bi);
            }
        } else if RES_B.contains(&bt.as_str()) {
            set_use!("residential", "residential", 1.0);
        } else if bt == "apartments" {
            let q = math_max(1.0, floor(&blds, bi, true) / M2_PER_DWELLING);
            set_use!("apartment", "apartment", q);
        } else if bt == "terrace" {
            let q = math_max(1.0, math_round(floor(&blds, bi, false) / M2_PER_DWELLING));
            set_use!("residential", "residential", q);
        } else if bt == "commercial" {
            if shop_near(bx_, by_, SHOP_NEAR) {
                let q = floor(&blds, bi, false);
                set_use!("retail", "retail", q);
            } else {
                let q = floor(&blds, bi, false);
                set_use!("office", "office", q);
            }
        // (2) shops, clinics, schools mapped as points in or at it
        } else if has(&blds, &pois, "hotel") {
            let pt = poi_of(&blds, &pois, "hotel").expect("a hotel point");
            let q = rooms(&blds, pois[pt].t);
            set_use!("hotel", "hotel", q);
        } else if has(&blds, &pois, "fastfood") {
            shop_as!("fastfood");
        } else if has(&blds, &pois, "shop") {
            // A building with shops mapped in it: general retail if any shop is, else trade/bulky goods, else a workshop
            let u: Vec<&str> = blds[bi]
                .pois
                .iter()
                .filter(|&&p| pois[p].kind == "shop")
                .map(|&p| shop_use(tag_str(pois[p].t, "shop")))
                .collect();
            let k = if u.contains(&"retail") {
                "retail"
            } else if u.contains(&"warehouse") {
                "warehouse"
            } else {
                "industrial"
            };
            shop_as!(k);
        } else if has(&blds, &pois, "medical") {
            set_use!("medical");
        } else if has(&blds, &pois, "hospital") {
            let q = floor(&blds, bi, false);
            set_use!("medical", "medical", q);
        } else if has(&blds, &pois, "childcare") {
            set_use!("childcare");
        } else if has(&blds, &pois, "school") {
            set_use!("education");
        } else if has(&blds, &pois, "tertiary") {
            set_use!("tertiary");
        // (3) the site or land-use area it sits in
        } else if site_kind == Some("school") {
            set_use!("education");
        } else if site_kind == Some("tertiary") {
            set_use!("tertiary");
        } else if site_kind == Some("childcare") {
            set_use!("childcare");
        } else if site_kind == Some("medical") {
            set_use!("medical");
        } else if site_kind == Some("hospital") {
            let q = floor(&blds, bi, false);
            set_use!("medical", "medical", q);
        } else if site_kind == Some("hotel") {
            set_use!("hotel");
            areas[site.expect("a site")]
                .hb
                .get_or_insert_with(Vec::new)
                .push(bi);
        } else if site_kind == Some("fastfood") {
            shop_as!("fastfood");
        } else if let Some(sa) = site {
            let k = if site_kind == Some("supermarket") {
                "supermarket"
            } else {
                shop_use(tag_str(areas[sa].t, "shop"))
            };
            shop_as!(k);
        } else if lu_kind == Some("residential") {
            residential(&mut blds);
        } else if lu_kind == Some("commercial") {
            if shop_near(bx_, by_, SHOP_NEAR) {
                let q = floor(&blds, bi, false);
                set_use!("retail", "retail", q);
            } else {
                let q = floor(&blds, bi, false);
                set_use!("office", "office", q);
            }
        } else if lu_kind == Some("retail") {
            let q = floor(&blds, bi, false);
            set_use!("retail", "retail", q);
        } else if lu_kind == Some("industrial") {
            let q = floor(&blds, bi, false);
            set_use!("industrial", "industrial", q);
        } else if lu_kind == Some("education") {
            set_use!("education");
            orphans.push(bi);
        // (4) outside any mapped area: rural houses by size, shops by proximity
        } else if bt == "residential" || (barea >= HOUSE_MIN && barea <= HOUSE_MAX) {
            residential(&mut blds);
            if blds[bi].cls == Some("residential") {
                rural.push(bi);
            }
        } else if shop_near(bx_, by_, RURAL_SHOP_NEAR) {
            let q = floor(&blds, bi, false);
            set_use!("retail", "retail", q);
        } else {
            ign!(if barea < HOUSE_MIN {
                "small rural building"
            } else {
                "large rural building (farm or yard)"
            });
        }
        // Clinics mapped as points add 300 m² each, unless the building or its site is already the clinic
        let in_clinic_site = blds[bi].cls == Some("medical") && site_kind == Some("medical");
        if !blds[bi].own_med && !in_clinic_site {
            for &p in &blds[bi].pois {
                if pois[p].kind == "medical" {
                    extra.push(Item {
                        x: bx_,
                        y: by_,
                        k: "medical",
                        q: MEDICAL_M2,
                        street: street_of(pois[p].t),
                    });
                }
            }
        }
    }
    /* Rural houses: a farmhouse and its sheds sit together, so a house-sized building within 40 m of a house (measured from that house,
    never chained from shed to shed) is its outbuilding. Where 6 or more buildings of house size or larger stand within 100 m it is a
    settlement (a street with no mapped residential area), and each building is a dwelling. */
    let dense = |blds: &[Bld], b: usize| -> bool {
        let n = c_idx
            .near(blds[b].x, blds[b].y, DENSE_R)
            .iter()
            .filter(|&&o| {
                blds[o].area >= HOUSE_MIN
                    && math_hypot(&[blds[o].x - blds[b].x, blds[o].y - blds[b].y]) <= DENSE_R
            })
            .count();
        n >= DENSE_N
    };
    let farm: Vec<usize> = rural
        .iter()
        .copied()
        .filter(|&b| !dense(&blds, b))
        .collect();
    let mut taken: HashSet<usize> = HashSet::new();
    let mut r_idx = Grid::new(100.0, lim);
    for &b in &farm {
        r_idx.add([blds[b].x, blds[b].y, blds[b].x, blds[b].y], b);
    }
    // The building closest to 150 m² is the house and claims the house-sized sheds around it
    let mut order = farm.clone();
    js_sort_by(&mut order, |&p, &q| {
        let d = (blds[p].area - 150.0).abs() - (blds[q].area - 150.0).abs();
        if d != 0.0 && !d.is_nan() {
            d
        } else {
            blds[p].id - blds[q].id
        }
    });
    for b in order {
        if taken.contains(&b) {
            continue;
        }
        taken.insert(b);
        for o in r_idx.near(blds[b].x, blds[b].y, RURAL_GROUP) {
            if !taken.contains(&o)
                && math_hypot(&[blds[o].x - blds[b].x, blds[o].y - blds[b].y]) <= RURAL_GROUP
            {
                taken.insert(o);
                ign_fn(&mut blds[o], "rural outbuilding");
            }
        }
    }
    // Clinic, hotel and shop sites. A clinic site is 300 m² unless a clinic building in it already counts;
    // shop sites mapped without buildings use a building cover of their land area (yards excluded)
    for a in &areas {
        if !inside(a.s.cx, a.s.cy) {
            continue;
        }
        if a.site == Some("medical") && !a.has_med {
            extra.push(Item {
                x: a.s.cx,
                y: a.s.cy,
                k: "medical",
                q: MEDICAL_M2,
                street: street_of(a.t),
            });
        }
        if a.site == Some("hotel") {
            let q = match count_tag(a.t, "rooms") {
                Some(c) if c != 0.0 => c,
                _ => match &a.hb {
                    Some(hb) => math_round(
                        hb.iter()
                            .fold(0.0, |s, &b| s + blds[b].area * levels_of(blds[b].t, false))
                            / M2_PER_ROOM,
                    ),
                    None => 0.0,
                },
            };
            if q > 0.0 {
                extra.push(Item {
                    x: a.s.cx,
                    y: a.s.cy,
                    k: "hotel",
                    q,
                    street: street_of(a.t),
                });
            }
        }
        if matches!(a.site, Some("shop" | "supermarket"))
            && a.blds.is_empty()
            && !tag_str(a.t, "shop").is_some_and(|s| SHOP_YARD.contains(&s))
        {
            let k = if a.site == Some("supermarket") {
                "supermarket"
            } else {
                shop_use(tag_str(a.t, "shop"))
            };
            extra.push(Item {
                x: a.s.cx,
                y: a.s.cy,
                k,
                q: math_min(10000.0, a.s.area * SITE_COVER),
                street: street_of(a.t),
            });
        }
    }
    // Education buildings with no mapped campus: attach to one within 250 m, else start one
    for &b in &orphans {
        let kind = if blds[b].cls == Some("childcare") {
            "childcare"
        } else if blds[b].cls == Some("tertiary") {
            "tertiary"
        } else {
            "school"
        };
        if !campuses
            .iter()
            .any(|c| c.kind == kind && math_hypot(&[c.x - blds[b].x, c.y - blds[b].y]) <= CAMPUS_R)
        {
            campuses.push(Campus {
                kind,
                x: blds[b].x,
                y: blds[b].y,
                t: blds[b].t,
                from: "building",
                area: None,
            });
        }
    }
    let mut loose_n = 0usize;
    let in_med_site = |areas: &[Area], p: &Poi| {
        areas_at(areas, p.x, p.y)
            .iter()
            .any(|&a| areas[a].site == Some("medical"))
    };
    for &pi in &loose {
        let p = &pois[pi];
        match p.kind {
            "shop" => extra.push(Item {
                x: p.x,
                y: p.y,
                k: shop_use(tag_str(p.t, "shop")),
                q: SHOP_M2,
                street: street_of(p.t),
            }),
            "supermarket" => extra.push(Item {
                x: p.x,
                y: p.y,
                k: "supermarket",
                q: SUPERMARKET_M2,
                street: street_of(p.t),
            }),
            "fastfood" => extra.push(Item {
                x: p.x,
                y: p.y,
                k: "fastfood",
                q: FASTFOOD_M2,
                street: street_of(p.t),
            }),
            "hotel" if count_tag(p.t, "rooms").is_some_and(|c| c != 0.0) => extra.push(Item {
                x: p.x,
                y: p.y,
                k: "hotel",
                q: count_tag(p.t, "rooms").expect("rooms"),
                street: street_of(p.t),
            }),
            "medical" if !in_med_site(&areas, p) => extra.push(Item {
                x: p.x,
                y: p.y,
                k: "medical",
                q: MEDICAL_M2,
                street: street_of(p.t),
            }),
            _ => continue,
        }
        if inside(p.x, p.y) {
            loose_n += 1;
        }
    }
    // One campus each: a point or tagged building inside a mapped campus area, very close to another, with the same name nearby, or
    // unnamed within 250 m of another of its kind is the same campus
    let rank = |from: &str| match from {
        "area" => 0.0,
        "building" => 1.0,
        _ => 2.0,
    };
    js_sort_by(&mut campuses, |a, b| rank(a.from) - rank(b.from));
    let mut kept: Vec<usize> = Vec::new();
    for ci in 0..campuses.len() {
        let c = &campuses[ci];
        let dup = kept.iter().any(|&ki| {
            let k = &campuses[ki];
            if k.kind != c.kind {
                return false;
            }
            let d = math_hypot(&[k.x - c.x, k.y - c.y]);
            let in_area = k.area.is_some_and(|a| areas[a].s.contains(c.x, c.y));
            let cname = tag_truthy(c.t, "name");
            in_area
                || d < 50.0
                || (cname && tag_text(c.t, "name") == tag_text(k.t, "name") && d <= CAMPUS_R)
                || (!cname && c.from != "area" && d <= CAMPUS_R)
        });
        if !dup {
            kept.push(ci);
        }
    }
    for &ci in &kept {
        let c = &campuses[ci];
        if !inside(c.x, c.y) {
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
            extra.push(Item {
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
            extra.push(Item {
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
            extra.push(Item {
                x: c.x,
                y: c.y,
                k: "childcare",
                q: math_round(cap),
                street: street_of(c.t),
            });
        }
    }

    // Stats over buildings inside the study area
    let mut items: Vec<Item> = Vec::new();
    for bd in &blds {
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
        if inside(it.x, it.y) && it.q > 0.0 {
            items.push(it);
        }
    }
    let mut tot: HashMap<&str, f64> = WEIGHT.iter().map(|(k, _)| (*k, 0.0)).collect();
    for it in &items {
        *tot.get_mut(it.k).expect("a land-use key") += it.q;
    }
    let tt = |k: &str| tot[k];
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

    // Aggregate to grid cells
    struct Cell {
        i: f64,
        j: f64,
        w: f64,
        sx: f64,
        sy: f64,
        lu: Vec<f64>,
        streets: Vec<(String, f64)>,
    }
    let nx = math_max(1.0, (w / cell).ceil());
    let ny = math_max(1.0, (h / cell).ceil());
    let mut cells: Vec<(f64, Cell)> = Vec::new();
    let mut cell_ix: HashMap<u64, usize> = HashMap::new();
    let nk = WEIGHT.len();
    let kix = |k: &str| {
        WEIGHT
            .iter()
            .position(|(key, _)| *key == k)
            .expect("a land-use key")
    };
    for it in &items {
        let i = math_min(nx - 1.0, (it.x / cell).floor());
        let j = math_min(ny - 1.0, (it.y / cell).floor());
        let key = j * nx + i;
        let at = cell_ix.get(&key.to_bits()).copied();
        let at = match at {
            Some(a) => a,
            None => {
                cell_ix.insert(key.to_bits(), cells.len());
                cells.push((
                    key,
                    Cell {
                        i,
                        j,
                        w: 0.0,
                        sx: 0.0,
                        sy: 0.0,
                        lu: vec![0.0; nk],
                        streets: Vec::new(),
                    },
                ));
                cells.len() - 1
            }
        };
        let c = &mut cells[at].1;
        let wt = it.q * weight(it.k);
        c.w += wt;
        c.sx += it.x * wt;
        c.sy += it.y * wt;
        c.lu[kix(it.k)] += it.q;
        if let Some(st) = &it.street {
            match c.streets.iter_mut().find(|(s, _)| s == st) {
                Some((_, v)) => *v += wt,
                None => c.streets.push((st.clone(), wt)),
            }
        }
    }
    // Merge cells with little activity into the nearest cell (by activity-weighted centre)
    let mut live: Vec<Cell> = cells
        .into_iter()
        .map(|(_, c)| c)
        .filter(|c| c.w > 0.0)
        .collect();
    js_sort_by(&mut live, |a, b| {
        let d = b.j - a.j;
        if d != 0.0 && !d.is_nan() {
            d
        } else {
            a.i - b.i
        }
    });
    loop {
        let mut small: Option<usize> = None;
        for (ci, c) in live.iter().enumerate() {
            if c.w < MERGE_BELOW && small.is_none_or(|s| c.w < live[s].w) {
                small = Some(ci);
            }
        }
        let Some(small) = small.filter(|_| live.len() >= 2) else {
            break;
        };
        let sx = live[small].sx / live[small].w;
        let sy = live[small].sy / live[small].w;
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
        let best = best.expect("a nearest cell");
        let sm = live.remove(small);
        let best = if best > small { best - 1 } else { best };
        let bc = &mut live[best];
        bc.w += sm.w;
        bc.sx += sm.sx;
        bc.sy += sm.sy;
        for k in 0..nk {
            bc.lu[k] += sm.lu[k];
        }
        for (s, wv) in sm.streets {
            match bc.streets.iter_mut().find(|(n, _)| *n == s) {
                Some((_, v)) => *v += wv,
                None => bc.streets.push((s, wv)),
            }
        }
        stats.merged += 1.0;
    }

    // Named roads for zones with no addresses
    let roads: Vec<(&str, Vec<Pt>)> = opts
        .links
        .iter()
        .filter(|l| !l.name.is_empty() && l.pts.len() > 1)
        .map(|l| {
            (
                l.name.as_str(),
                l.pts.iter().map(|p| to_xy(p[0], p[1])).collect(),
            )
        })
        .collect();
    let nearest_road = |x: f64, y: f64| -> Option<&str> {
        let mut best: Option<&str> = None;
        let mut bd = 1000.0;
        for (name, xy) in &roads {
            for i in 1..xy.len() {
                let d = util::seg_dist(x, y, xy[i - 1][0], xy[i - 1][1], xy[i][0], xy[i][1]).d;
                if d < bd {
                    bd = d;
                    best = Some(name);
                }
            }
        }
        best
    };

    let mut zones: Vec<Zone> = Vec::new();
    let mut used: Vec<(String, f64)> = Vec::new();
    let mut area_n = 0.0;
    for c in &live {
        let mut lu: OrderedMap<Num> = OrderedMap::new();
        for (ki, (k, _)) in WEIGHT.iter().enumerate() {
            let q = c.lu[ki];
            let per1 = LANDUSES
                .iter()
                .find(|l| l.key == *k)
                .is_some_and(|l| l.per == 1.0);
            let v = if per1 {
                math_round(q)
            } else {
                math_round(q / 10.0) * 10.0
            };
            if v > 0.0 {
                lu.insert(*k, Num::number(v));
            }
        }
        if lu.is_empty() {
            continue;
        }
        let x = c.sx / c.w;
        let y = c.sy / c.w;
        let mut sorted: Vec<&(String, f64)> = c.streets.iter().collect();
        js_sort_by(&mut sorted, |a, b| {
            let d = b.1 - a.1;
            if d != 0.0 && !d.is_nan() {
                d
            } else if js_str_lt(&a.0, &b.0) {
                -1.0
            } else {
                1.0
            }
        });
        let mut base: String = match sorted.first() {
            Some(st) => st.0.clone(),
            None => nearest_road(x, y).map(str::to_string).unwrap_or_default(),
        };
        if base.is_empty() {
            area_n += 1.0;
            base = format!("Area {}", number_to_js_string(area_n));
        }
        let n = match used.iter_mut().find(|(k, _)| *k == base) {
            Some((_, n)) => {
                *n += 1.0;
                *n
            }
            None => {
                used.push((base.clone(), 1.0));
                1.0
            }
        };
        let name = format!(
            "Existing \u{b7} {}{}",
            base,
            if n > 1.0 {
                format!(" {}", number_to_js_string(n))
            } else {
                String::new()
            }
        );
        let id = ids.uid("z");
        zones.push(Zone {
            id: id.clone(),
            kind: Maybe::Set("zone".to_string()),
            name: Maybe::Set(name),
            lat: Num::number(r7(bx.s + y / ky)),
            lng: Num::number(r7(bx.w + x / kx)),
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
        });
    }
    stats.zones = zones.len() as f64;

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
    if stats.buildings != 0.0 && no_area as f64 > stats.buildings / 2.0 {
        a.push(format!("Warning: {} of {} buildings are outside any mapped land-use area, so they were classed by size alone. Check the totals, or map the residential and industrial areas in OpenStreetMap.", fmt::int(no_area as f64), fmt::int(stats.buildings)));
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
    BuildResult { zones, stats }
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
    match kq {
        Some((k, q)) => {
            b.k = Some(k);
            b.q = Some(q);
        }
        None => {
            b.k = None;
            b.q = None;
        }
    }
}

/// Every constant of the build: the numbers and the tag sets, as JSON (sets sorted), for checking against the prototype's source.
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
        ("DENSE_N", DENSE_N as f64),
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
