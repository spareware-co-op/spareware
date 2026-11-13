//! Placing the downloaded data in zones: Census dwellings and travel, employees, schools and early learning, Unitary Plan zones (landuse-plus.js "allocation to zones").

use super::consts::{self, std as std_};
use super::geo::{Bx, Poly, Ring, Sample, bb_in_box, bb_meets, in_box, in_poly, lr, rings_area, sample_share};
use super::parse::{CensusData, Sa1};
use crate::edit::IdSource;
use crate::graph::zone_class;
use crate::jsnum::{Maybe, Num, json_number, math_hypot, math_round, usize_to_f64, value_to_js_string, value_to_number, value_truthy};
use crate::project::{OrderedMap, Scenario, Zone};
use crate::util::{self, fmt, r1, r3, r7, seg_dist};
use serde_json::{Map, Value, json};

/// `isExisting(z)`: a land-use zone (not a gateway) marked existing whose class is existing.
#[must_use]
pub fn is_existing(z: &Zone) -> bool {
    !z.kind.is("gateway") && z.existing.is_true() && zone_class(Some(z)) == "existing"
}

/// `landZones(sc, B)`: the ids of the zones (not gateways) with a finite position, inside the box when there is one.
#[must_use]
pub fn land_zone_ids(sc: &Scenario, b: Option<&Bx>) -> Vec<String> {
    sc.zones
        .values()
        .filter(|z| {
            !z.kind.is("gateway") && z.lat.num().is_finite() && z.lng.num().is_finite() && b.is_none_or(|b| in_box(b, z.lat.num(), z.lng.num()))
        })
        .map(|z| z.id.clone())
        .collect()
}

/// A residential-capable building outline: its centre and its weight (footprint \u{d7} levels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Building {
    /// Latitude of the centre.
    pub lat: f64,
    /// Longitude of the centre.
    pub lng: f64,
    /// Weight.
    pub w: f64,
}

const RES_BLD: [&str; 10] = ["yes", "house", "detached", "semidetached_house", "terrace", "residential", "apartments", "bungalow", "static_caravan", "dormitory"];
const NONRES_LU: [&str; 7] = ["industrial", "commercial", "retail", "education", "farmyard", "cemetery", "military"];

fn lls(g: Option<&Value>) -> Ring {
    g.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|p| value_truthy(p) && value_to_number(p.get("lat").unwrap_or(&Value::Null)).is_finite() && p.get("lat").is_some() && p.get("lon").is_some() && value_to_number(p.get("lon").unwrap_or(&Value::Null)).is_finite())
                .map(|p| [value_to_number(&p["lon"]), value_to_number(&p["lat"])])
                .collect()
        })
        .unwrap_or_default()
}

fn rings_of_el(e: &Value) -> Vec<Ring> {
    match e.get("type").and_then(Value::as_str) {
        Some("way") => vec![lls(e.get("geometry"))],
        Some("relation") => e
            .get("members")
            .and_then(Value::as_array)
            .map(|m| {
                m.iter()
                    .filter(|m| m.get("type").and_then(Value::as_str) == Some("way") && m.get("role").and_then(Value::as_str) != Some("inner"))
                    .map(|m| lls(m.get("geometry")))
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn tag<'a>(t: &'a Value, k: &str) -> Option<&'a Value> {
    t.get(k)
}

/// `prepBuildings(els, B)`: residential-capable building outlines (40 to 1500 m\u{b2}, not in a non-residential land-use area), weighted by footprint \u{d7} levels.
pub fn prep_buildings(els: &[Value], b: &Bx) -> Vec<Building> {
    let mut out = Vec::new();
    if els.is_empty() {
        return out;
    }
    let empty = json!({});
    let mut areas: Vec<Poly> = Vec::new();
    for e in els {
        let t = e.get("tags").filter(|t| value_truthy(t)).unwrap_or(&empty);
        let has_building = tag(t, "building").is_some_and(value_truthy);
        let non_res = tag(t, "landuse").and_then(Value::as_str).is_some_and(|l| NONRES_LU.contains(&l));
        if !has_building && non_res {
            let rg: Vec<Ring> = rings_of_el(e).into_iter().filter(|r| r.len() > 3).collect();
            if !rg.is_empty() {
                areas.push(Poly::new(rg));
            }
        }
    }
    for e in els {
        let t = e.get("tags").filter(|t| value_truthy(t)).unwrap_or(&empty);
        let bld = tag(t, "building");
        if !bld.is_some_and(value_truthy) || !bld.and_then(Value::as_str).is_some_and(|s| RES_BLD.contains(&s)) || e.get("type").and_then(Value::as_str) != Some("way") {
            continue;
        }
        let r = lls(e.get("geometry"));
        if r.len() < 4 {
            continue;
        }
        let a = rings_area(std::slice::from_ref(&r));
        if a < 40.0 || a > 1500.0 {
            continue;
        }
        let lng = r.iter().fold(0.0, |s, p| s + p[0]) / usize_to_f64(r.len());
        let lat = r.iter().fold(0.0, |s, p| s + p[1]) / usize_to_f64(r.len());
        if !in_box(b, lat, lng) || areas.iter().any(|a| in_poly(a, lng, lat)) {
            continue;
        }
        let levels = crate::jsnum::or_default(tag(t, "building:levels").map_or(f64::NAN, value_to_number), if bld.and_then(Value::as_str) == Some("apartments") { 3.0 } else { 1.0 });
        let lv = crate::jsnum::math_max(1.0, crate::jsnum::math_min(30.0, math_round(levels)));
        out.push(Building { lat, lng, w: a * lv });
    }
    out
}

/// `prov(src, extra)`: the provenance record of a derived value: source, licence, date, basis "derived", then the extra members.
#[must_use]
pub fn prov(src: &consts::Source, today: &str, extra: Vec<(&str, Value)>) -> Value {
    let mut o = Map::new();
    o.insert("source".into(), json!(src.name));
    o.insert("licence".into(), json!(src.licence));
    o.insert("date".into(), json!(today));
    o.insert("basis".into(), json!("derived"));
    for (k, v) in extra {
        o.insert(k.to_string(), v);
    }
    Value::Object(o)
}

/// `isCensus(s)`: a provenance record whose source starts with the Census name.
#[must_use]
pub fn is_census(s: Option<&Value>) -> bool {
    s.is_some_and(|s| {
        value_truthy(s) && {
            let src = s.get("source").filter(|x| value_truthy(x)).map_or(String::new(), value_to_js_string);
            src.starts_with(consts::CENSUS.name)
        }
    })
}

/// `nearestRoad(sc, lat, lng)`: the name of the road within 800 m that is nearest, if any.
#[must_use]
pub fn nearest_road(sc: &Scenario, lat: f64, lng: f64) -> Option<String> {
    let f = util::flat(lat);
    let mut best: Option<String> = None;
    let mut bd = 800.0;
    for l in sc.links.values() {
        let name = l.name.str_or("");
        let Some(pts) = l.pts.get() else {
            continue;
        };
        if name.is_empty() || pts.len() < 2 {
            continue;
        }
        // Each segment: point i - 1 to point i
        for pair in pts.windows(2) {
            let [pa, pb] = pair else {
                continue;
            };
            let (a, b) = (pa.0, pb.0);
            let d = seg_dist(lng * f.kx, lat * f.ky, a[1] * f.kx, a[0] * f.ky, b[1] * f.kx, b[0] * f.ky).d;
            if d < bd {
                bd = d;
                best = Some(name.to_string());
            }
        }
    }
    best
}

/// `addCounts(dst, src, k)`: dst += src \u{d7} k for nested count objects (nulls are skipped).
pub fn add_counts(dst: &mut Map<String, Value>, src: &Map<String, Value>, k: f64) {
    for (key, v) in src {
        match v {
            Value::Object(m) => {
                add_counts(json_obj(dst, key), m, k);
            }
            Value::Null => {}
            other => {
                let x = value_to_number(other);
                if x.is_finite() {
                    let cur = dst.get(key).map_or(0.0, |c| crate::jsnum::or_default(value_to_number(c), 0.0));
                    dst.insert(key.clone(), json_number(cur + x * k));
                }
            }
        }
    }
}

/// `roundCounts(o)`: every count rounded to one decimal, nested objects too.
pub fn round_counts(o: &mut Map<String, Value>) {
    for v in o.values_mut() {
        match v {
            Value::Object(m) => round_counts(m),
            other => *other = json_number(r1(value_to_number(other))),
        }
    }
}

/// A per-zone sum of SA1 values: dwellings placed and the SA1s that gave them.
struct Bucket {
    z: Option<String>,
    res: f64,
    apt: f64,
    sa1: Vec<(String, f64)>,
    attrs: Map<String, Value>,
    sx: f64,
    sy: f64,
    sw: f64,
}

/// A row of the Census report: one SA1 and where its dwellings went.
#[derive(Clone, Debug, PartialEq)]
pub struct CensusRow {
    /// SA1 code.
    pub code: String,
    /// Occupied dwellings in the Census (None: the layer did not load; Some(None): suppressed).
    pub occ: Option<Option<f64>>,
    /// Dwellings used.
    pub used: f64,
    /// What the dwellings count is based on.
    pub occ_basis: String,
    /// Share of the SA1 inside the study area (3 decimals).
    pub in_share: f64,
    /// Dwellings expected inside the study area (1 decimal).
    pub target: f64,
    /// Dwellings placed.
    pub placed: f64,
    /// Houses placed.
    pub houses: f64,
    /// Apartments placed.
    pub apartments: f64,
    /// How the dwellings were placed.
    pub basis: String,
    /// Zones that received dwellings.
    pub zones: usize,
}

impl CensusRow {
    /// The row as JSON.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut o = Map::new();
        o.insert("code".into(), json!(self.code));
        match self.occ {
            None => {}
            Some(None) => {
                o.insert("occ".into(), Value::Null);
            }
            Some(Some(x)) => {
                o.insert("occ".into(), json_number(x));
            }
        }
        o.insert("used".into(), json_number(self.used));
        o.insert("occBasis".into(), json!(self.occ_basis));
        o.insert("inShare".into(), json_number(self.in_share));
        o.insert("target".into(), json_number(self.target));
        o.insert("placed".into(), json_number(self.placed));
        o.insert("houses".into(), json_number(self.houses));
        o.insert("apartments".into(), json_number(self.apartments));
        o.insert("basis".into(), json!(self.basis));
        o.insert("zones".into(), json!(self.zones));
        Value::Object(o)
    }
}

/// A zone where OpenStreetMap shows clearly more dwellings than the Census.
#[derive(Clone, Debug, PartialEq)]
pub struct Flagged {
    /// Zone id.
    pub id: String,
    /// Zone name.
    pub name: Value,
    /// Dwellings in OpenStreetMap.
    pub osm: f64,
    /// Dwellings in the Census.
    pub census: f64,
    /// The OpenStreetMap count was kept.
    pub kept: bool,
}

/// The report of placing the Census (`censusToZones`' answer).
#[derive(Clone, Debug, PartialEq)]
pub struct CensusReport {
    /// One row per SA1 that touches the study area.
    pub rows: Vec<CensusRow>,
    /// Existing zones considered.
    pub zones: usize,
    /// Zones made for dwellings far from any existing zone.
    pub created: usize,
    /// Dwellings expected inside the study area.
    pub expected: f64,
    /// Dwellings placed.
    pub placed: f64,
    /// The dwellings layer was downloaded.
    pub dwellings: bool,
    /// Census dwellings over the zones.
    pub census_total: f64,
    /// OpenStreetMap dwellings over the zones.
    pub osm_total: f64,
    /// Zones that look newer than the Census.
    pub flagged: Vec<Flagged>,
    /// Warnings.
    pub warnings: Vec<String>,
    /// Occupied dwellings over all SA1s.
    pub sa1_total: f64,
    /// Residential building outlines used (set by `apply`).
    pub buildings: Option<usize>,
}

impl CensusReport {
    /// The report as JSON, in the prototype's shape.
    pub fn to_value(&self) -> Value {
        let mut o = json!({
            "rows": self.rows.iter().map(CensusRow::to_value).collect::<Vec<_>>(),
            "zones": self.zones, "created": self.created, "expected": json_number(self.expected), "placed": json_number(self.placed), "dwellings": self.dwellings,
            "censusTotal": json_number(self.census_total), "osmTotal": json_number(self.osm_total),
            "flagged": self.flagged.iter().map(|f| json!({ "id": f.id, "name": f.name, "osm": json_number(f.osm), "census": json_number(f.census), "kept": f.kept })).collect::<Vec<_>>(),
            "warnings": self.warnings, "sa1Total": json_number(self.sa1_total),
        });
        // The json! above is an object
        if let Some(b) = self.buildings
            && let Some(m) = o.as_object_mut()
        {
            m.insert("buildings".into(), json!(b));
        }
        o
    }
}

/// What a fill is asked to do (the `opts` of `apply`, `censusToZones` and `schoolsToZones`).
#[derive(Clone, Debug, Default)]
pub struct ApplyOpts {
    /// "osm" keeps the OpenStreetMap dwelling count where it is clearly higher; anything else flags it.
    pub newer: Option<String>,
    /// False stops zones being made for far dwellings (`opts.create === false`).
    pub create: Option<bool>,
    /// School rolls the user entered, by school id.
    pub rolls: Vec<(String, Value)>,
    /// False skips replacing existing school and early learning numbers (`opts.replace === false`).
    pub replace: Option<bool>,
}

// v as an object, made {} first when it is anything else
fn as_object(v: &mut Value) -> &mut Map<String, Value> {
    match v {
        Value::Object(o) => o,
        other => {
            // Now an object, so the second call returns at once
            *other = Value::Object(Map::new());
            as_object(other)
        }
    }
}

/// `m[k]` as an object, made {} first when it is missing or anything else.
pub(super) fn json_obj<'a>(m: &'a mut Map<String, Value>, k: &str) -> &'a mut Map<String, Value> {
    as_object(m.entry(k.to_string()).or_insert_with(|| Value::Object(Map::new())))
}

// A land use map, made empty first when it is missing or null
fn lu_set(lu: &mut Maybe<OrderedMap<Num>>) -> &mut OrderedMap<Num> {
    match lu {
        Maybe::Set(m) => m,
        other => {
            // Now set, so the second call returns at once
            *other = Maybe::Set(OrderedMap::new());
            lu_set(other)
        }
    }
}

/// A zone's land use, made an empty map first when it is missing or null.
pub(super) fn lu_map(z: &mut Zone) -> &mut OrderedMap<Num> {
    lu_set(&mut z.lu)
}

fn lu_num(z: &Zone, k: &str) -> f64 {
    z.lu.get().map_or(f64::NAN, |l| l.num_at(k).num())
}

fn nz(x: Option<f64>) -> f64 {
    x.unwrap_or(0.0)
}

/// `censusToZones(sc, C, blds, B, opts)`: Census dwellings and attributes to the existing zones.
///
/// Inside an SA1 the dwellings follow residential building footprints (each building goes to its nearest existing zone); an SA1 that
/// crosses the study-area edge is first split by area overlap; an SA1 with no buildings mapped is split by area. Dwellings far from any
/// zone get a zone of their own.
pub fn census_to_zones(sc: &mut Scenario, ids: &mut dyn IdSource, c: &CensusData, blds: &[Building], b: &Bx, opts: &ApplyOpts) -> CensusReport {
    let zone_ids: Vec<String> = land_zone_ids(sc, Some(b)).into_iter().filter(|id| sc.zones.get(id).is_some_and(is_existing)).collect();
    // (id, lat, lng) of each existing zone: nearest() looks only at these, in this order
    let mut zones: Vec<ZoneAt> = zone_ids.iter().filter_map(|id| sc.zones.get(id).map(|z| (id.clone(), z.lat.num(), z.lng.num()))).collect();
    let dw_ok = !c.missing.iter().any(|m| m == "dw layer");
    let Placed { rows, mut buckets, mut warnings, total: placed_sum } = place_sa1s(c, blds, b, opts, &zones, dw_ok);
    make_far_zones(sc, ids, &mut buckets, &mut zones);
    let when = ids.today();
    let mut totals = Totals { osm: 0.0, census: 0.0, flagged: Vec::new() };
    for (zid, _, _) in &zones {
        let bucket = buckets.iter().find(|(_, bk)| bk.z.as_deref() == Some(zid.as_str())).map(|(_, bk)| bk);
        // Every id in zones is one of this scenario's zones (found above or made by make_far_zones)
        let Some(z) = sc.zones.get_mut(zid) else {
            continue;
        };
        write_zone(z, bucket, dw_ok, opts, &when, &mut totals);
    }
    census_loc(sc, c, b, &when);
    let expected = rows.iter().fold(0.0, |s, r| s + r.used * r.in_share);
    if !dw_ok {
        warnings.push("The Census dwellings layer did not download, so dwellings are left as they were; people, households, vehicles and travel were filled from the layers that loaded.".to_string());
    }
    let created = buckets.iter().filter(|(k, bk)| k.starts_with("new:") && bk.z.is_some()).count();
    let sa1_total = rows.iter().fold(0.0, |s, r| s + crate::jsnum::or_default(r.occ.flatten().unwrap_or(0.0), 0.0));
    CensusReport {
        rows,
        zones: zones.len(),
        created,
        expected: r1(expected),
        placed: placed_sum,
        dwellings: dw_ok,
        census_total: totals.census,
        osm_total: totals.osm,
        flagged: totals.flagged,
        warnings,
        sa1_total,
        buildings: None,
    }
}

// An existing zone as the placing sees it: (id, lat, lng)
type ZoneAt = (String, f64, f64);

// What placing the SA1s gives: the report rows, the per-zone buckets, the warnings and the dwellings placed
struct Placed {
    rows: Vec<CensusRow>,
    buckets: Vec<(String, Bucket)>,
    warnings: Vec<String>,
    total: f64,
}

// Each SA1's dwellings go to the buckets of the existing zones nearest its points (or to a new zone's bucket when the nearest is too far)
fn place_sa1s(c: &CensusData, blds: &[Building], b: &Bx, opts: &ApplyOpts, zones: &[ZoneAt], dw_ok: bool) -> Placed {
    let fl = util::flat(f64::midpoint(b.s, b.n));
    let nearest = |lat: f64, lng: f64| -> (Option<usize>, f64) {
        let mut best = None;
        let mut bd = f64::INFINITY;
        for (i, zone) in zones.iter().enumerate() {
            let dist = math_hypot(&[(zone.2 - lng) * fl.kx, (zone.1 - lat) * fl.ky]);
            if dist < bd {
                bd = dist;
                best = Some(i);
            }
        }
        (best, bd)
    };
    let mut out = Placed { rows: Vec::new(), buckets: Vec::new(), warnings: Vec::new(), total: 0.0 };
    let mut cum = 0.0f64;
    for sa1 in &c.sa1 {
        if !bb_meets(&sa1.poly.bb, b) {
            continue;
        }
        let within = bb_in_box(&sa1.poly.bb, b);
        let smp = if within { None } else { Some(sample_share(&sa1.poly, b, 24)) };
        // smp is None just when the SA1 lies wholly inside the box
        let in_share = smp.as_ref().map_or(1.0, |sm| sm.share);
        // !(in_share > 0), NaN included
        if in_share <= 0.0 || in_share.is_nan() {
            continue;
        }
        let (pts, basis) = sa1_points(sa1, blds, b, within, smp);
        if pts.is_empty() {
            continue;
        }
        // The weight of each bucket key, in the order the keys first appear
        let mut weights: Vec<(String, f64)> = Vec::new();
        for [lat, lng, wt] in pts {
            let (zi, dist) = nearest(lat, lng);
            let near_id = zi.and_then(|i| zones.get(i)).map(|zone| zone.0.clone());
            let key = match &near_id {
                Some(id) if !(opts.create != Some(false) && dist > std_::CREATE_M) => id.clone(),
                _ => format!("new:{}", sa1.code),
            };
            let z_of = if key.starts_with("new:") { None } else { near_id };
            let slot = if let Some(i) = out.buckets.iter().position(|(k, _)| *k == key) {
                out.buckets.get_mut(i)
            } else {
                out.buckets.push((key.clone(), Bucket { z: z_of, res: 0.0, apt: 0.0, sa1: Vec::new(), attrs: Map::new(), sx: 0.0, sy: 0.0, sw: 0.0 }));
                out.buckets.last_mut()
            };
            if let Some((_, bk)) = slot {
                bk.sx += lng * wt;
                bk.sy += lat * wt;
                bk.sw += wt;
            }
            match weights.iter_mut().find(|(k, _)| *k == key) {
                Some(e) => e.1 += wt,
                None => weights.push((key, wt)),
            }
        }
        let occ_raw = sa1.v.get("occ");
        let (occ, occ_basis) = sa1_occ(sa1, dw_ok, &mut out.warnings);
        let target = occ * in_share;
        cum += target;
        let placed = crate::jsnum::math_max(0.0, math_round(cum) - out.total);
        out.total += placed;
        let it = |k: &str| nz(sa1.item(k));
        let typed = it("sep") + it("joined") + it("otherPriv");
        let apt = math_round(placed * if typed > 0.0 { it("joined") / typed } else { 0.0 });
        let hs = placed - apt;
        let ws: Vec<f64> = weights.iter().map(|(_, x)| *x).collect();
        let tot = ws.iter().fold(0.0, |acc, x| acc + x);
        let hs_parts = lr(hs, &ws);
        let ap_parts = lr(apt, &ws);
        // lr gives one part per weight, and every key in weights has its bucket
        for (((k, x), hp), ap) in weights.iter().zip(&hs_parts).zip(&ap_parts) {
            let Some((_, bk)) = out.buckets.iter_mut().find(|(bkey, _)| bkey == k) else {
                continue;
            };
            let sh = *x / tot * in_share;
            bk.res += *hp;
            bk.apt += *ap;
            bk.sa1.push((sa1.code.clone(), r3(sh)));
            add_counts(&mut bk.attrs, &sa1.v, sh);
        }
        out.rows.push(CensusRow {
            code: sa1.code.clone(),
            occ: occ_raw.map(Value::as_f64),
            used: occ,
            occ_basis,
            in_share: r3(in_share),
            target: r1(target),
            placed,
            houses: hs,
            apartments: apt,
            basis,
            zones: weights.len(),
        });
    }
    out
}

// The points an SA1's dwellings follow ([lat, lng, weight]) and how: its residential buildings, else the sample points inside both it and the box
fn sa1_points(sa1: &Sa1, blds: &[Building], b: &Bx, within: bool, smp: Option<Sample>) -> (Vec<[f64; 3]>, String) {
    let bl: Vec<&Building> = blds.iter().filter(|x| in_poly(&sa1.poly, x.lng, x.lat)).collect();
    if bl.is_empty() {
        let sp = smp.unwrap_or_else(|| sample_share(&sa1.poly, b, 24));
        (sp.pts.iter().map(|p| [p[1], p[0], 1.0]).collect(), "area overlap (no residential buildings mapped)".to_string())
    } else {
        (
            bl.iter().map(|x| [x.lat, x.lng, x.w]).collect(),
            if within { "building footprints".to_string() } else { "area overlap at the study-area edge, then building footprints".to_string() },
        )
    }
}

// The dwellings an SA1 counts and what the count is based on; a warning when its dwelling and household counts are both suppressed
fn sa1_occ(sa1: &Sa1, dw_ok: bool, warnings: &mut Vec<String>) -> (f64, String) {
    let (mut occ, mut occ_basis): (Option<f64>, String) = (sa1.v.get("occ").and_then(Value::as_f64), "occupied private dwellings".to_string());
    if !dw_ok {
        occ = Some(0.0);
        occ_basis = "dwellings layer not downloaded".to_string();
    }
    if occ.is_none() && sa1.v.get("hh").is_some_and(|h| !h.is_null()) {
        occ = sa1.v.get("hh").and_then(Value::as_f64);
        occ_basis = "households (occupied dwellings suppressed)".to_string();
    }
    if occ.is_none() {
        occ = Some(0.0);
        warnings.push(format!("SA1 {}: dwelling and household counts are suppressed, so it adds no dwellings.", sa1.code));
        occ_basis = "suppressed".to_string();
    }
    // Set to 0 just above when it was still None
    (occ.unwrap_or(0.0), occ_basis)
}

// Zones for Census dwellings far from any existing zone (or when there are none)
fn make_far_zones(sc: &mut Scenario, ids: &mut dyn IdSource, buckets: &mut [(String, Bucket)], zones: &mut Vec<ZoneAt>) {
    for (key, bk) in buckets.iter_mut() {
        let dwellings = bk.res + bk.apt;
        // !(res + apt > 0), NaN included
        if bk.z.is_some() || dwellings <= 0.0 || dwellings.is_nan() {
            continue;
        }
        let (lat, lng) = (bk.sy / bk.sw, bk.sx / bk.sw);
        let id = ids.uid("z");
        let road = nearest_road(sc, lat, lng);
        let name = format!("Existing \u{b7} {}", road.unwrap_or_else(|| format!("SA1 {}", key.chars().skip(4).collect::<String>())));
        let mut zone = super::fill::new_zone(&id);
        zone.kind = Maybe::Set("zone".into());
        zone.existing = true.into();
        zone.src = Maybe::Set(json!("census"));
        zone.name = Maybe::Set(name);
        zone.lat = Num::number(r7(lat));
        zone.lng = Num::number(r7(lng));
        zone.lu = Maybe::Set(OrderedMap::new());
        zone.connectors = Maybe::Null;
        let (zl, zg) = (zone.lat.num(), zone.lng.num());
        sc.zones.insert(id.clone(), zone);
        bk.z = Some(id.clone());
        zones.push((id, zl, zg));
    }
}

// Totals over the zones as they are written: OpenStreetMap and Census dwellings, and the zones that look newer than the Census
struct Totals {
    osm: f64,
    census: f64,
    flagged: Vec<Flagged>,
}

// One existing zone's dwellings (unless the OpenStreetMap count is kept) and its Census record, from its bucket
fn write_zone(z: &mut Zone, bucket: Option<&Bucket>, dw_ok: bool, opts: &ApplyOpts, when: &str, totals: &mut Totals) {
    let (res, apt, sa1, attrs) = match bucket {
        Some(bk) => (bk.res, bk.apt, bk.sa1.clone(), bk.attrs.clone()),
        None => (0.0, 0.0, Vec::new(), Map::new()),
    };
    lu_map(z);
    json_obj(&mut z.extra, "luSrc");
    let lu_src_res_is_census = is_census(z.extra.get("luSrc").and_then(|l| l.get("residential")));
    let osm_has = z.extra.get("osm").is_some_and(value_truthy);
    let (osm_res, osm_apt) = if lu_src_res_is_census && osm_has {
        let osm = z.extra.get("osm");
        (nz_value(osm.and_then(|o| o.get("residential"))), nz_value(osm.and_then(|o| o.get("apartment"))))
    } else {
        (crate::jsnum::or_default(lu_num(z, "residential"), 0.0), crate::jsnum::or_default(lu_num(z, "apartment"), 0.0))
    };
    let o_t = osm_res + osm_apt;
    let c_t = res + apt;
    totals.osm += o_t;
    totals.census += c_t;
    if dw_ok {
        let newer = o_t > c_t * (1.0 + std_::CENSUS_NEWER) + std_::CENSUS_NEWER_ABS;
        let keep = newer && opts.newer.as_deref() == Some("osm");
        if newer {
            totals.flagged.push(Flagged { id: z.id.clone(), name: z.name.as_str().map_or(Value::Null, |n| json!(n)), osm: o_t, census: c_t, kept: keep });
        }
        if o_t != 0.0 && !z.extra.get("osm").is_some_and(value_truthy) {
            z.extra.insert("osm".into(), json!({ "residential": json_number(osm_res), "apartment": json_number(osm_apt) }));
        }
        if keep {
            let mut rec = Map::new();
            rec.insert("source".into(), json!(consts::OSM.name));
            rec.insert("licence".into(), json!(consts::OSM.licence));
            rec.insert("basis".into(), json!("estimated"));
            rec.insert("date".into(), json!(when));
            if let Some(lu) = z.lu.get().filter(|l| l.contains_key("residential")) {
                rec.insert("q".into(), super::fill::num_json(lu.num_at("residential")).unwrap_or(Value::Null));
            }
            rec.insert("note".into(), json!("Kept: OSM shows more dwellings than the 2023 Census (likely built since March 2023)"));
            json_obj(&mut z.extra, "luSrc").insert("residential".into(), Value::Object(rec));
        } else {
            for (key, qty) in [("residential", res), ("apartment", apt)] {
                if qty > 0.0 {
                    lu_map(z).insert(key, Num::number(qty));
                } else {
                    lu_map(z).remove(key);
                }
                let codes = sa1.iter().map(|s| s.0.clone()).collect::<Vec<_>>().join(", ");
                let note = format!(
                    "Census occupied dwellings in SA1 {}, placed by {}; random-rounded to base 3",
                    if codes.is_empty() { fmt::DASH.to_string() } else { codes },
                    if sa1.is_empty() { "none" } else { "building footprint or area" }
                );
                let pr = prov(&consts::CENSUS, when, vec![("q", json_number(qty)), ("vintage", json!(consts::CENSUS.vintage)), ("note", json!(note))]);
                json_obj(&mut z.extra, "luSrc").insert(key.to_string(), pr);
            }
        }
    }
    let mut counts = attrs;
    round_counts(&mut counts);
    z.extra.insert("census".into(), census_record(&sa1, &counts, &(if dw_ok { json_number(c_t) } else { Value::Null }), when));
}

// A zone's `census` record: the SA1s its dwellings came from and the rounded counts
fn census_record(sa1: &[(String, f64)], counts: &Map<String, Value>, dwellings: &Value, when: &str) -> Value {
    let veh = counts.get("veh").cloned().unwrap_or_else(|| json!({}));
    let vehicles = [0, 1, 2, 3, 4, 5].iter().fold(0.0, |s, i| s + f64::from(*i) * crate::jsnum::or_default(veh.get(i.to_string()).map_or(0.0, value_to_number), 0.0));
    let nul = |x: Option<&Value>| x.filter(|x| !x.is_null()).cloned().unwrap_or(Value::Null);
    let obj_or_null = |x: Option<&Value>| x.filter(|x| value_truthy(x)).cloned().unwrap_or(Value::Null);
    json!({
        "at": when, "source": consts::CENSUS.name, "licence": consts::CENSUS.licence, "basis": "derived", "vintage": consts::CENSUS.vintage, "rounding": consts::CENSUS.rounding,
        "sa1": sa1.iter().map(|(code, share)| json!({ "code": code, "share": json_number(*share) })).collect::<Vec<_>>(),
        "dwellings": dwellings,
        "pop": nul(counts.get("pop")), "hh": nul(counts.get("hh")), "empWork": nul(counts.get("empWork")),
        "vehicles": json_number(r1(vehicles)), "vehStated": nul(veh.get("stated")), "noCar": nul(veh.get("0")),
        "modes": { "r": obj_or_null(counts.get("r")), "w": obj_or_null(counts.get("w")), "e": obj_or_null(counts.get("e")) },
    })
}

// Location-only mode shares for the other land-use zones (development, committed): the SA1 they stand in
fn census_loc(sc: &mut Scenario, c: &CensusData, b: &Bx, when: &str) {
    for id in land_zone_ids(sc, Some(b)) {
        let Some(zone) = sc.zones.get(&id) else {
            continue;
        };
        if is_existing(zone) {
            continue;
        }
        let (lat, lng) = (zone.lat.num(), zone.lng.num());
        if let Some(sa1) = c.sa1.iter().find(|s| in_poly(&s.poly, lng, lat)) {
            let modes_of = |key: &str| sa1.v.get(key).filter(|x| value_truthy(x)).cloned().unwrap_or(Value::Null);
            let rec = json!({ "at": when, "source": consts::CENSUS.name, "licence": consts::CENSUS.licence, "basis": "derived", "vintage": consts::CENSUS.vintage,
                "rounding": consts::CENSUS.rounding, "sa1": sa1.code, "modes": { "r": modes_of("r"), "w": modes_of("w"), "e": modes_of("e") } });
            // The id was read from this scenario's zones just above
            if let Some(zm) = sc.zones.get_mut(&id) {
                zm.extra.insert("censusLoc".into(), rec);
            }
        }
    }
}

// `(x || 0)` of a loose JSON number
fn nz_value(v: Option<&Value>) -> f64 {
    v.map_or(0.0, |x| crate::jsnum::or_default(value_to_number(x), 0.0))
}
