//! Placing the downloaded data in zones: Census dwellings and travel, employees, schools and early learning, Unitary Plan zones (landuse-plus.js "allocation to zones").

use super::consts::{self, std as std_};
use super::geo::{Bx, Poly, Ring, bb_in_box, bb_meets, in_box, in_poly, lr, rings_area, sample_share};
use super::parse::CensusData;
use crate::edit::IdSource;
use crate::graph::zone_class;
use crate::jsnum::{Maybe, Num, json_number, math_hypot, math_round, value_to_js_string, value_to_number, value_truthy};
use crate::project::{OrderedMap, Scenario, Zone};
use crate::util::{self, fmt, r1, r3, r7, seg_dist};
use serde_json::{Map, Value, json};

/// `isExisting(z)`: a land-use zone (not a gateway) marked existing whose class is existing.
pub fn is_existing(z: &Zone) -> bool {
    !z.kind.is("gateway") && z.existing.is_true() && zone_class(Some(z)) == "existing"
}

/// `landZones(sc, B)`: the ids of the zones (not gateways) with a finite position, inside the box when there is one.
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
        let lng = r.iter().fold(0.0, |s, p| s + p[0]) / r.len() as f64;
        let lat = r.iter().fold(0.0, |s, p| s + p[1]) / r.len() as f64;
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
pub fn is_census(s: Option<&Value>) -> bool {
    s.is_some_and(|s| {
        value_truthy(s) && {
            let src = s.get("source").filter(|x| value_truthy(x)).map_or(String::new(), value_to_js_string);
            src.starts_with(consts::CENSUS.name)
        }
    })
}

/// `nearestRoad(sc, lat, lng)`: the name of the road within 800 m that is nearest, if any.
pub fn nearest_road(sc: &Scenario, lat: f64, lng: f64) -> Option<String> {
    let f = util::flat(lat);
    let mut best: Option<String> = None;
    let mut bd = 800.0;
    for l in sc.links.values() {
        let name = l.name.str_or("");
        let pts = l.pts.get();
        if name.is_empty() || pts.is_none_or(|p| p.len() < 2) {
            continue;
        }
        let pts = pts.expect("points");
        for i in 1..pts.len() {
            let (a, b) = (pts[i - 1].0, pts[i].0);
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
                let e = dst.entry(key.clone()).or_insert_with(|| Value::Object(Map::new()));
                if !e.is_object() {
                    *e = Value::Object(Map::new());
                }
                add_counts(e.as_object_mut().expect("object"), m, k);
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
        if let Some(b) = self.buildings {
            o["buildings"] = json!(b);
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

fn json_obj<'a>(m: &'a mut Map<String, Value>, k: &str) -> &'a mut Map<String, Value> {
    let e = m.entry(k.to_string()).or_insert_with(|| Value::Object(Map::new()));
    if !e.is_object() {
        *e = Value::Object(Map::new());
    }
    e.as_object_mut().expect("an object")
}

fn lu_map(z: &mut Zone) -> &mut OrderedMap<Num> {
    if !z.lu.is_set() {
        z.lu = Maybe::Set(OrderedMap::new());
    }
    z.lu.get_mut().expect("lu")
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
    let mut zones: Vec<(String, f64, f64)> = zone_ids.iter().map(|id| { let z = &sc.zones[id]; (id.clone(), z.lat.num(), z.lng.num()) }).collect();
    let f = util::flat((b.s + b.n) / 2.0);
    let dw_ok = !c.missing.iter().any(|m| m == "dw layer");
    let nearest_n = zones.len();
    let nearest = |zones: &[(String, f64, f64)], lat: f64, lng: f64| -> (Option<usize>, f64) {
        let mut best = None;
        let mut bd = f64::INFINITY;
        for (i, z) in zones.iter().take(nearest_n).enumerate() {
            let d = math_hypot(&[(z.2 - lng) * f.kx, (z.1 - lat) * f.ky]);
            if d < bd {
                bd = d;
                best = Some(i);
            }
        }
        (best, bd)
    };
    let mut buckets: Vec<(String, Bucket)> = Vec::new();
    let mut rows: Vec<CensusRow> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let (mut cum, mut placed_sum) = (0.0f64, 0.0f64);
    for a in &c.sa1 {
        if !bb_meets(&a.poly.bb, b) {
            continue;
        }
        let within = bb_in_box(&a.poly.bb, b);
        let smp = if within { None } else { Some(sample_share(&a.poly, b, 24)) };
        let in_share = if within { 1.0 } else { smp.as_ref().expect("sample").share };
        if !(in_share > 0.0) {
            continue;
        }
        let bl: Vec<&Building> = blds.iter().filter(|x| in_poly(&a.poly, x.lng, x.lat)).collect();
        let (pts, basis): (Vec<[f64; 3]>, String) = if !bl.is_empty() {
            (
                bl.iter().map(|x| [x.lat, x.lng, x.w]).collect(),
                if within { "building footprints".to_string() } else { "area overlap at the study-area edge, then building footprints".to_string() },
            )
        } else {
            let sp = smp.unwrap_or_else(|| sample_share(&a.poly, b, 24));
            (sp.pts.iter().map(|p| [p[1], p[0], 1.0]).collect(), "area overlap (no residential buildings mapped)".to_string())
        };
        if pts.is_empty() {
            continue;
        }
        let mut w: Vec<(String, f64)> = Vec::new();
        for [lat, lng, wt] in pts {
            let (zi, d) = nearest(&zones, lat, lng);
            let key = match zi {
                Some(i) if !(opts.create != Some(false) && d > std_::CREATE_M) => zones[i].0.clone(),
                _ => format!("new:{}", a.code),
            };
            let z_of = if key.starts_with("new:") { None } else { zi.map(|i| zones[i].0.clone()) };
            let bi = match buckets.iter().position(|(k, _)| *k == key) {
                Some(i) => i,
                None => {
                    buckets.push((key.clone(), Bucket { z: z_of, res: 0.0, apt: 0.0, sa1: Vec::new(), attrs: Map::new(), sx: 0.0, sy: 0.0, sw: 0.0 }));
                    buckets.len() - 1
                }
            };
            let bk = &mut buckets[bi].1;
            bk.sx += lng * wt;
            bk.sy += lat * wt;
            bk.sw += wt;
            match w.iter_mut().find(|(k, _)| *k == key) {
                Some(e) => e.1 += wt,
                None => w.push((key, wt)),
            }
        }
        let occ_raw = a.v.get("occ");
        let (mut occ, mut occ_basis): (Option<f64>, String) = (occ_raw.and_then(Value::as_f64), "occupied private dwellings".to_string());
        if !dw_ok {
            occ = Some(0.0);
            occ_basis = "dwellings layer not downloaded".to_string();
        }
        if occ.is_none() && a.v.get("hh").is_some_and(|h| !h.is_null()) {
            occ = a.v.get("hh").and_then(Value::as_f64);
            occ_basis = "households (occupied dwellings suppressed)".to_string();
        }
        if occ.is_none() {
            occ = Some(0.0);
            warnings.push(format!("SA1 {}: dwelling and household counts are suppressed, so it adds no dwellings.", a.code));
            occ_basis = "suppressed".to_string();
        }
        let occ = occ.expect("dwellings");
        let target = occ * in_share;
        cum += target;
        let t = crate::jsnum::math_max(0.0, math_round(cum) - placed_sum);
        placed_sum += t;
        let it = |k: &str| nz(a.item(k));
        let typed = it("sep") + it("joined") + it("otherPriv");
        let apt = math_round(t * if typed > 0.0 { it("joined") / typed } else { 0.0 });
        let hs = t - apt;
        let ws: Vec<f64> = w.iter().map(|(_, x)| *x).collect();
        let tot = ws.iter().fold(0.0, |s, x| s + x);
        let hs_parts = lr(hs, &ws);
        let ap_parts = lr(apt, &ws);
        for (i, (k, _)) in w.iter().enumerate() {
            let bk = &mut buckets.iter_mut().find(|(bk, _)| bk == k).expect("bucket").1;
            let sh = ws[i] / tot * in_share;
            bk.res += hs_parts[i];
            bk.apt += ap_parts[i];
            bk.sa1.push((a.code.clone(), r3(sh)));
            add_counts(&mut bk.attrs, &a.v, sh);
        }
        rows.push(CensusRow {
            code: a.code.clone(),
            occ: occ_raw.map(Value::as_f64),
            used: occ,
            occ_basis,
            in_share: r3(in_share),
            target: r1(target),
            placed: t,
            houses: hs,
            apartments: apt,
            basis,
            zones: w.len(),
        });
    }
    // Zones for Census dwellings far from any existing zone (or when there are none)
    for (k, bk) in buckets.iter_mut() {
        if bk.z.is_some() || !(bk.res + bk.apt > 0.0) {
            continue;
        }
        let (lat, lng) = (bk.sy / bk.sw, bk.sx / bk.sw);
        let id = ids.uid("z");
        let road = nearest_road(sc, lat, lng);
        let name = format!("Existing \u{b7} {}", road.unwrap_or_else(|| format!("SA1 {}", k.chars().skip(4).collect::<String>())));
        let mut z = super::fill::new_zone(&id);
        z.kind = Maybe::Set("zone".into());
        z.existing = true.into();
        z.src = Maybe::Set(json!("census"));
        z.name = Maybe::Set(name);
        z.lat = Num::number(r7(lat));
        z.lng = Num::number(r7(lng));
        z.lu = Maybe::Set(OrderedMap::new());
        z.connectors = Maybe::Null;
        let (zl, zg) = (z.lat.num(), z.lng.num());
        sc.zones.insert(id.clone(), z);
        bk.z = Some(id.clone());
        zones.push((id, zl, zg));
    }
    let mut flagged: Vec<Flagged> = Vec::new();
    let when = ids.today();
    let (mut osm_total, mut census_total) = (0.0f64, 0.0f64);
    for (zid, _, _) in zones.iter() {
        let (res, apt, sa1, attrs) = match buckets.iter().find(|(_, bk)| bk.z.as_deref() == Some(zid.as_str())) {
            Some((_, bk)) => (bk.res, bk.apt, bk.sa1.clone(), bk.attrs.clone()),
            None => (0.0, 0.0, Vec::new(), Map::new()),
        };
        let z = sc.zones.get_mut(zid).expect("zone");
        lu_map(z);
        json_obj(&mut z.extra, "luSrc");
        let lu_src_res_is_census = is_census(z.extra.get("luSrc").and_then(|l| l.get("residential")));
        let osm_has = z.extra.get("osm").is_some_and(value_truthy);
        let (osm_res, osm_apt) = if lu_src_res_is_census && osm_has {
            let o = &z.extra["osm"];
            (nz_value(o.get("residential")), nz_value(o.get("apartment")))
        } else {
            (crate::jsnum::or_default(lu_num(z, "residential"), 0.0), crate::jsnum::or_default(lu_num(z, "apartment"), 0.0))
        };
        let o_t = osm_res + osm_apt;
        let c_t = res + apt;
        osm_total += o_t;
        census_total += c_t;
        if dw_ok {
            let newer = o_t > c_t * (1.0 + std_::CENSUS_NEWER) + std_::CENSUS_NEWER_ABS;
            let keep = newer && opts.newer.as_deref() == Some("osm");
            if newer {
                flagged.push(Flagged { id: z.id.clone(), name: z.name.as_str().map_or(Value::Null, |n| json!(n)), osm: o_t, census: c_t, kept: keep });
            }
            if o_t != 0.0 && !z.extra.get("osm").is_some_and(value_truthy) {
                z.extra.insert("osm".into(), json!({ "residential": json_number(osm_res), "apartment": json_number(osm_apt) }));
            }
            if !keep {
                for (k, q) in [("residential", res), ("apartment", apt)] {
                    if q > 0.0 {
                        lu_map(z).insert(k, Num::number(q));
                    } else {
                        lu_map(z).remove(k);
                    }
                    let codes = sa1.iter().map(|s| s.0.clone()).collect::<Vec<_>>().join(", ");
                    let note = format!(
                        "Census occupied dwellings in SA1 {}, placed by {}; random-rounded to base 3",
                        if codes.is_empty() { fmt::DASH.to_string() } else { codes },
                        if sa1.is_empty() { "none" } else { "building footprint or area" }
                    );
                    let p = prov(&consts::CENSUS, &when, vec![("q", json_number(q)), ("vintage", json!(consts::CENSUS.vintage)), ("note", json!(note))]);
                    json_obj(&mut z.extra, "luSrc").insert(k.to_string(), p);
                }
            } else {
                let mut rec = Map::new();
                rec.insert("source".into(), json!(consts::OSM.name));
                rec.insert("licence".into(), json!(consts::OSM.licence));
                rec.insert("basis".into(), json!("estimated"));
                rec.insert("date".into(), json!(when));
                if z.lu.get().is_some_and(|l| l.contains_key("residential")) {
                    rec.insert("q".into(), super::fill::num_json(z.lu.get().expect("lu").num_at("residential")).unwrap_or(Value::Null));
                }
                rec.insert("note".into(), json!("Kept: OSM shows more dwellings than the 2023 Census (likely built since March 2023)"));
                json_obj(&mut z.extra, "luSrc").insert("residential".into(), Value::Object(rec));
            }
        }
        let mut v = attrs;
        round_counts(&mut v);
        let veh = v.get("veh").cloned().unwrap_or_else(|| json!({}));
        let vehicles = [0, 1, 2, 3, 4, 5].iter().fold(0.0, |s, i| s + *i as f64 * crate::jsnum::or_default(veh.get(i.to_string()).map_or(0.0, value_to_number), 0.0));
        let nul = |x: Option<&Value>| x.filter(|x| !x.is_null()).cloned().unwrap_or(Value::Null);
        let obj_or_null = |x: Option<&Value>| x.filter(|x| value_truthy(x)).cloned().unwrap_or(Value::Null);
        z.extra.insert(
            "census".into(),
            json!({
                "at": when, "source": consts::CENSUS.name, "licence": consts::CENSUS.licence, "basis": "derived", "vintage": consts::CENSUS.vintage, "rounding": consts::CENSUS.rounding,
                "sa1": sa1.iter().map(|(code, share)| json!({ "code": code, "share": json_number(*share) })).collect::<Vec<_>>(),
                "dwellings": if dw_ok { json_number(c_t) } else { Value::Null },
                "pop": nul(v.get("pop")), "hh": nul(v.get("hh")), "empWork": nul(v.get("empWork")),
                "vehicles": json_number(r1(vehicles)), "vehStated": nul(veh.get("stated")), "noCar": nul(veh.get("0")),
                "modes": { "r": obj_or_null(v.get("r")), "w": obj_or_null(v.get("w")), "e": obj_or_null(v.get("e")) },
            }),
        );
    }
    // Location-only mode shares for the other land-use zones (development, committed): the SA1 they stand in
    for id in land_zone_ids(sc, Some(b)) {
        let z = &sc.zones[&id];
        if is_existing(z) {
            continue;
        }
        let (lat, lng) = (z.lat.num(), z.lng.num());
        if let Some(a) = c.sa1.iter().find(|a| in_poly(&a.poly, lng, lat)) {
            let m = |k: &str| a.v.get(k).filter(|x| value_truthy(x)).cloned().unwrap_or(Value::Null);
            let rec = json!({ "at": when, "source": consts::CENSUS.name, "licence": consts::CENSUS.licence, "basis": "derived", "vintage": consts::CENSUS.vintage,
                "rounding": consts::CENSUS.rounding, "sa1": a.code, "modes": { "r": m("r"), "w": m("w"), "e": m("e") } });
            sc.zones.get_mut(&id).expect("zone").extra.insert("censusLoc".into(), rec);
        }
    }
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
        census_total,
        osm_total,
        flagged,
        warnings,
        sa1_total,
        buildings: None,
    }
}

// `(x || 0)` of a loose JSON number
fn nz_value(v: Option<&Value>) -> f64 {
    v.map_or(0.0, |x| crate::jsnum::or_default(value_to_number(x), 0.0))
}
