//! Employees, schools, early learning and Unitary Plan zones to zones, and the whole fill (landuse-plus.js `employeesToZones` to `record`).

use super::alloc::{ApplyOpts, CensusReport, Building, census_to_zones, is_existing, land_zone_ids, prep_buildings, prov};
use super::consts::{self, CHOICES, JOB_KEYS, NOT_LAND, std as std_};
use super::geo::{Bx, area_box, bb_in_box, bb_meets, box_key, box_of, in_box, in_poly, lr, n0, rings_area, sample_share};
use super::parse::{AupData, CensusData, EmployeeData, Moe};
use super::requests::{Downloads, Net};
use crate::edit::IdSource;
use crate::edit::history::mark_stale;
use crate::graph::js_sort_by;
use crate::jsmath;
use crate::jsnum::{Maybe, Num, json_number, math_hypot, math_round, value_to_js_string, value_to_number, value_truthy};
use crate::landuse::{self, BuildOpts, StudyBox};
use crate::project::{OrderedMap, Project, Scenario, Zone};
use crate::util::{self, fmt, r1, r3, r7};
use serde_json::{Map, Value, json};

/// A zone with every field missing, for the fields a fill sets one by one.
pub fn new_zone(id: &str) -> Zone {
    Zone {
        id: id.to_string(),
        kind: Maybe::Missing,
        name: Maybe::Missing,
        lat: Maybe::Missing,
        lng: Maybe::Missing,
        lu: Maybe::Missing,
        connectors: Maybe::Missing,
        role: Maybe::Missing,
        year: Maybe::Missing,
        buildout: Maybe::Missing,
        existing: Maybe::Missing,
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

/// A [`Num`] as the JSON it would be saved as (None when missing).
pub fn num_json(n: &Num) -> Option<Value> {
    match n {
        Maybe::Missing => None,
        Maybe::Null => Some(Value::Null),
        Maybe::Set(j) => Some(j.to_value()),
    }
}

fn obj<'a>(m: &'a mut Map<String, Value>, k: &str) -> &'a mut Map<String, Value> {
    let e = m.entry(k.to_string()).or_insert_with(|| Value::Object(Map::new()));
    if !e.is_object() {
        *e = Value::Object(Map::new());
    }
    e.as_object_mut().expect("an object")
}

fn lu_of(z: &mut Zone) -> &mut OrderedMap<Num> {
    if !z.lu.is_set() {
        z.lu = Maybe::Set(OrderedMap::new());
    }
    z.lu.get_mut().expect("land use")
}

fn lu_q(z: &Zone, k: &str) -> f64 {
    z.lu.get().map_or(f64::NAN, |l| l.num_at(k).num())
}

/// `fromMoE(s)`: a provenance record from the Ministry of Education.
pub fn from_moe(s: Option<&Value>) -> bool {
    s.filter(|s| value_truthy(s)).and_then(|s| s.get("source")).filter(|x| value_truthy(x)).map_or(false, |x| value_to_js_string(x).starts_with("Ministry"))
}

/// `luProv(z, k)`: where the quantity of land use k in a zone came from.
pub fn lu_prov(z: &Zone, k: &str) -> Value {
    let s = z.extra.get("luSrc").and_then(|l| l.get(k)).filter(|s| value_truthy(s));
    if let Some(s) = s {
        let q = s.get("q");
        if q.is_none_or(Value::is_null) || value_to_number(q.expect("q")) == crate::jsnum::or_default(lu_q(z, k), 0.0) {
            return s.clone();
        }
        return json!({ "source": "changed in Spare Construct after it was filled", "basis": "user" });
    }
    let name = z.name.str_or("");
    if z.src.get().and_then(Value::as_str) == Some("existing-lu") || name.starts_with("Existing \u{b7} ") {
        return json!({ "source": consts::OSM.name, "licence": consts::OSM.licence, "basis": "estimated" });
    }
    json!({ "source": "entered in Spare Construct", "basis": "user" })
}

/// `replaceLu(z, k, by, when)`: takes land use k out of a zone, keeping what it was (`osm` for OpenStreetMap estimates, `replaced` otherwise).
pub fn replace_lu(z: &mut Zone, k: &str, by: &str, when: &str) -> f64 {
    let q = lu_q(z, k);
    let pr = lu_prov(z, k);
    let basis = pr.get("basis").filter(|x| value_truthy(x)).map(value_to_js_string);
    let source = pr.get("source").filter(|x| value_truthy(x)).map(value_to_js_string);
    if pr.get("basis").and_then(Value::as_str) == Some("estimated") || pr.get("source").and_then(Value::as_str) == Some(consts::OSM.name) {
        obj(&mut z.extra, "osm").insert(k.to_string(), json_number(q));
    } else {
        obj(&mut z.extra, "replaced").insert(
            k.to_string(),
            json!({ "q": json_number(q), "source": source.unwrap_or_else(|| "entered in Spare Construct".into()), "basis": basis.unwrap_or_else(|| "user".into()), "by": by }),
        );
    }
    lu_of(z).remove(k);
    obj(&mut z.extra, "luSrc").insert(k.to_string(), json!({ "source": format!("Replaced by the {by}"), "basis": "derived", "date": when, "q": 0 }));
    q
}

/// One SA2 row of the employees report.
#[derive(Clone, Debug, PartialEq)]
pub struct EmployeeRow {
    /// SA2 code.
    pub code: String,
    /// SA2 name.
    pub name: String,
    /// Employees.
    pub n: f64,
    /// Year of the count.
    pub year: f64,
    /// Share inside the study area.
    pub in_share: f64,
    /// Employees placed in zones.
    pub allocated: f64,
    /// Employees outside the study area.
    pub outside: f64,
    /// Employees inside with no floor area to place them by.
    pub unplaced: f64,
    /// Average annual change (%).
    pub growth: Option<f64>,
    /// Basis text.
    pub basis: String,
}

/// The report of placing employees.
#[derive(Clone, Debug, PartialEq)]
pub struct EmployeesReport {
    /// One row per SA2 with employees that touches the study area.
    pub rows: Vec<EmployeeRow>,
    /// Year of the count.
    pub year: f64,
    /// Employees placed.
    pub allocated: f64,
    /// Census workplace count over the zones.
    pub census: f64,
    /// Employees over the Census workplace count.
    pub ratio: Option<f64>,
    /// Average annual change weighted by employees (%).
    pub growth: Option<f64>,
    /// The cross-check, in words.
    pub check: String,
}

impl EmployeesReport {
    /// The report as JSON.
    pub fn to_value(&self) -> Value {
        let n = |x: Option<f64>| x.map_or(Value::Null, json_number);
        json!({
            "rows": self.rows.iter().map(|r| json!({ "code": r.code, "name": r.name, "n": json_number(r.n), "year": json_number(r.year), "inShare": json_number(r.in_share),
                "allocated": json_number(r.allocated), "outside": json_number(r.outside), "unplaced": json_number(r.unplaced), "growth": n(r.growth), "basis": r.basis })).collect::<Vec<_>>(),
            "year": json_number(self.year), "allocated": json_number(self.allocated), "census": json_number(self.census), "ratio": n(self.ratio), "growth": n(self.growth), "check": self.check,
        })
    }
}

/// `employeesToZones(sc, E, B)`: each SA2 total is a hard control. The part inside the study area goes to the existing zones in the SA2 by
/// commercial and industrial floor area; the rest is reported as outside, or unplaced when no floor area is mapped.
pub fn employees_to_zones(sc: &mut Scenario, ids: &dyn IdSource, e: &EmployeeData, b: &Bx) -> EmployeesReport {
    let zone_ids: Vec<String> = land_zone_ids(sc, Some(b)).into_iter().filter(|id| sc.zones.get(id).is_some_and(is_existing)).collect();
    let when = ids.today();
    for id in &zone_ids {
        sc.zones.get_mut(id).expect("zone").extra.remove("jobs");
    }
    let gfa = |z: &Zone| JOB_KEYS.iter().fold(0.0, |s, k| s + crate::jsnum::or_default(lu_q(z, k), 0.0));
    let mut rows = Vec::new();
    let (mut allocated, mut growth_w, mut growth_s) = (0.0f64, 0.0f64, 0.0f64);
    for a in &e.sa2 {
        let Some(an) = a.n else { continue };
        if !(an > 0.0) || !bb_meets(&a.poly.bb, b) {
            continue;
        }
        let within = bb_in_box(&a.poly.bb, b);
        let in_share = if within { 1.0 } else { sample_share(&a.poly, b, 24).share };
        if !(in_share > 0.0) {
            continue;
        }
        let in_n = math_round(an * in_share);
        let zs: Vec<&String> = zone_ids.iter().filter(|id| { let z = &sc.zones[*id]; in_poly(&a.poly, z.lng.num(), z.lat.num()) && gfa(z) > 0.0 }).collect();
        let weights: Vec<f64> = zs.iter().map(|id| gfa(&sc.zones[*id])).collect();
        let parts = lr(in_n, &weights);
        let zs: Vec<String> = zs.into_iter().cloned().collect();
        for (i, id) in zs.iter().enumerate() {
            if parts[i] == 0.0 {
                continue;
            }
            let z = sc.zones.get_mut(id).expect("zone");
            if !z.extra.get("jobs").is_some_and(value_truthy) {
                z.extra.insert("jobs".into(), json!({ "n": 0, "by": [], "year": json_number(e.year), "source": consts::EMPLOYEES.name, "licence": consts::EMPLOYEES.licence,
                    "basis": "derived", "date": when, "note": "SA2 employees placed by commercial and industrial floor area" }));
            }
            let jobs = z.extra.get_mut("jobs").and_then(Value::as_object_mut).expect("jobs");
            let n = jobs.get("n").map_or(0.0, value_to_number);
            jobs.insert("n".into(), json_number(n + parts[i]));
            if let Some(by) = jobs.get_mut("by").and_then(Value::as_array_mut) {
                by.push(json!({ "sa2": a.code, "n": json_number(parts[i]) }));
            }
        }
        let placed = if zs.is_empty() { 0.0 } else { in_n };
        allocated += placed;
        if let Some(g) = a.growth {
            growth_w += an * in_share;
            growth_s += an * in_share * g;
        }
        rows.push(EmployeeRow {
            code: a.code.clone(),
            name: a.name.clone(),
            n: an,
            year: e.year,
            in_share: r3(in_share),
            allocated: placed,
            outside: an - in_n,
            unplaced: in_n - placed,
            growth: a.growth,
            basis: if within { "inside the study area".into() } else { "area overlap at the study-area edge".into() },
        });
    }
    let census = zone_ids.iter().fold(0.0, |s, id| {
        let v = sc.zones[id].extra.get("census").and_then(|c| c.get("empWork")).map_or(0.0, |x| crate::jsnum::or_default(value_to_number(x), 0.0));
        s + v
    });
    let ratio = if census > 0.0 { Some(allocated / census) } else { None };
    let band = format!("{}\u{2013}{}", crate::jsnum::number_to_js_string(std_::JOBS_CHECK[0]), crate::jsnum::number_to_js_string(std_::JOBS_CHECK[1]));
    let check = match ratio {
        None => "No Census 2023 workplace counts to compare with.".to_string(),
        Some(r) if r < std_::JOBS_CHECK[0] || r > std_::JOBS_CHECK[1] => format!(
            "Check: {} employees are {} \u{d7} the Census 2023 workplace count (Spare Construct band {band}). The floor area from OSM may be incomplete.",
            crate::jsnum::number_to_js_string(e.year),
            fmt::num(r, 2)
        ),
        Some(r) => format!(
            "{} employees are {} \u{d7} the Census 2023 workplace count, within the Spare Construct band {band}.",
            crate::jsnum::number_to_js_string(e.year),
            fmt::num(r, 2)
        ),
    };
    EmployeesReport {
        rows,
        year: e.year,
        allocated,
        census: r1(census),
        ratio: ratio.map(r3),
        growth: if growth_w > 0.0 { Some(r1(growth_s / growth_w)) } else { None },
        check,
    }
}

/// The report of placing schools and early learning services.
#[derive(Clone, Debug, PartialEq)]
pub struct SchoolsReport {
    /// Zones made.
    pub created: f64,
    /// Zones whose school was updated.
    pub updated: f64,
    /// Schools the directory has no roll for.
    pub needs_roll: Vec<Value>,
    /// Students replaced in existing zones.
    pub replaced_school: f64,
    /// Child places replaced in existing zones.
    pub replaced_childcare: f64,
    /// The schools added.
    pub schools: Vec<Value>,
    /// Early learning services added.
    pub ece: f64,
    /// Services left out and why.
    pub skipped: Vec<String>,
    /// The school directory was downloaded.
    pub loaded_schools: bool,
    /// The early learning directory was downloaded.
    pub loaded_ece: bool,
}

impl SchoolsReport {
    /// The report as JSON.
    pub fn to_value(&self) -> Value {
        json!({ "created": json_number(self.created), "updated": json_number(self.updated), "needsRoll": self.needs_roll,
            "replaced": { "school": json_number(self.replaced_school), "childcare": json_number(self.replaced_childcare) },
            "schools": self.schools, "ece": json_number(self.ece), "skipped": self.skipped,
            "loaded": { "schools": self.loaded_schools, "ece": self.loaded_ece } })
    }
}

// /home.?based/i
fn home_based(s: &str) -> bool {
    let low: Vec<char> = s.to_lowercase().chars().collect();
    let at = |i: usize, w: &str| low.len() >= i + w.chars().count() && low[i..i + w.chars().count()].iter().copied().eq(w.chars());
    (0..low.len()).any(|i| {
        at(i, "home")
            && (at(i + 4, "based") || (i + 4 < low.len() && !matches!(low[i + 4], '\n' | '\r' | '\u{2028}' | '\u{2029}') && at(i + 5, "based")))
    })
}

fn moe_zone_id(sc: &Scenario, id: &str) -> Option<String> {
    sc.zones.values().find(|z| z.extra.get("moe").and_then(|m| m.get("id")).and_then(Value::as_str) == Some(id)).map(|z| z.id.clone())
}

/// `schoolsToZones(sc, schools, ece, B, opts)`: schools become zones of their own with the Ministry roll (rolls the directory lacks come
/// from `opts.rolls`); early learning places go to the nearest existing zone within 300 m, else a zone of their own. The school and early
/// learning numbers in existing zones are replaced; development and committed zones are never touched.
pub fn schools_to_zones(sc: &mut Scenario, ids: &mut dyn IdSource, schools: &[Moe], ece: &[Moe], b: &Bx, opts: &ApplyOpts, schools_loaded: bool, ece_loaded: bool) -> SchoolsReport {
    let when = ids.today();
    let zone_ids: Vec<String> = land_zone_ids(sc, Some(b)).into_iter().filter(|id| sc.zones.get(id).is_some_and(is_existing)).collect();
    let replace = opts.replace != Some(false);
    let by = "Ministry of Education directory";
    let mut out = SchoolsReport {
        created: 0.0,
        updated: 0.0,
        needs_roll: Vec::new(),
        replaced_school: 0.0,
        replaced_childcare: 0.0,
        schools: Vec::new(),
        ece: 0.0,
        skipped: Vec::new(),
        loaded_schools: schools_loaded || !schools.is_empty(),
        loaded_ece: ece_loaded || !ece.is_empty(),
    };
    if replace && !schools.is_empty() {
        for id in &zone_ids {
            let z = sc.zones.get_mut(id).expect("zone");
            let has_school = z.lu.get().is_some() && lu_q(z, "school") > 0.0;
            if !z.extra.get("moe").is_some_and(value_truthy) && has_school && !from_moe(z.extra.get("luSrc").and_then(|l| l.get("school"))) {
                out.replaced_school += replace_lu(z, "school", by, &when);
            }
        }
    }
    for s in schools {
        if !in_box(b, s.lat, s.lng) {
            continue;
        }
        let roll: Option<f64> = match s.roll {
            Some(r) => Some(r),
            None => opts.rolls.iter().find(|(k, _)| *k == s.id).and_then(|(_, v)| (!v.is_null() && value_to_number(v).is_finite()).then(|| value_to_number(v))),
        };
        let opens = s.opens.filter(|o| crate::jsnum::truthy_number(*o));
        let Some(roll) = roll else {
            out.needs_roll.push(json!({ "id": s.id, "name": s.name, "type": s.kind_of_org, "status": s.status, "opens": opens.map_or(Value::Null, json_number),
                "rollDate": s.roll_date.as_ref().filter(|d| !d.is_empty()), "lat": json_number(s.lat), "lng": json_number(s.lng) }));
            continue;
        };
        let zid = moe_zone_id(sc, &s.id);
        let p = prov(
            &consts::SCHOOLS,
            &when,
            vec![
                ("basis", json!(if s.roll.is_some() { "measured" } else { "user" })),
                ("q", json_number(roll)),
                ("vintage", json!(match s.roll_date.as_deref() { Some(d) if !d.is_empty() => format!("roll at {d}"), _ => "no roll date".to_string() })),
                ("note", json!(if s.roll.is_some() { format!("MoE roll, {}", s.kind_of_org) } else { "Roll entered by the user (the MoE directory has none)".to_string() })),
            ],
        );
        let id = match zid {
            Some(id) => {
                out.updated += 1.0;
                id
            }
            None => {
                let id = ids.uid("z");
                let mut z = new_zone(&id);
                z.kind = Maybe::Set("zone".into());
                z.name = Maybe::Set(s.name.clone());
                z.lat = Num::number(r7(s.lat));
                z.lng = Num::number(r7(s.lng));
                z.lu = Maybe::Set(OrderedMap::new());
                z.connectors = Maybe::Null;
                z.src = Maybe::Set(json!("moe"));
                sc.zones.insert(id.clone(), z);
                out.created += 1.0;
                id
            }
        };
        let z = sc.zones.get_mut(&id).expect("zone");
        lu_of(z).insert("school", Num::number(roll));
        obj(&mut z.extra, "luSrc").insert("school".into(), p);
        z.extra.insert(
            "moe".into(),
            json!({ "id": s.id, "type": s.kind_of_org, "rollDate": s.roll_date, "status": s.status, "source": consts::SCHOOLS.name, "licence": consts::SCHOOLS.licence, "date": when }),
        );
        if let Some(o) = opens {
            z.year = Num::number(o);
            z.role = Maybe::Set("committed".into());
            z.existing = false.into();
        } else {
            z.existing = true.into();
        }
        out.schools.push(json!({ "id": s.id, "name": s.name, "type": s.kind_of_org, "roll": json_number(roll), "rollDate": s.roll_date, "opens": opens.map_or(Value::Null, json_number), "entered": s.roll.is_none() }));
    }
    if !ece.is_empty() {
        // places from an earlier fill come out first (unless changed by hand since)
        let all: Vec<String> = sc.zones.keys().map(str::to_string).collect();
        for id in &all {
            let z = sc.zones.get_mut(id).expect("zone");
            if z.extra.get("moeEce").is_some_and(value_truthy) {
                z.extra.remove("moeEce");
                if z.lu.get().is_some() && from_moe(Some(&lu_prov(z, "childcare"))) {
                    lu_of(z).remove("childcare");
                    obj(&mut z.extra, "luSrc").remove("childcare");
                }
            }
        }
        if replace {
            for id in &zone_ids {
                let z = sc.zones.get_mut(id).expect("zone");
                if z.lu.get().is_some() && lu_q(z, "childcare") > 0.0 && !from_moe(Some(&lu_prov(z, "childcare"))) {
                    out.replaced_childcare += replace_lu(z, "childcare", by, &when);
                }
            }
        }
        let f = util::flat((b.s + b.n) / 2.0);
        let mut cands: Vec<String> = sc.zones.values().filter(|z| is_existing(z) && !z.extra.get("moe").is_some_and(value_truthy)).map(|z| z.id.clone()).collect();
        for c in ece {
            if !in_box(b, c.lat, c.lng) {
                continue;
            }
            if home_based(&c.kind_of_org) {
                out.skipped.push(format!("{} (home-based network)", c.name));
                continue;
            }
            let q = if c.places.is_some() { c.places } else { c.roll };
            let q = match q {
                Some(q) if q > 0.0 => q,
                _ => {
                    out.skipped.push(format!("{} (no places or roll in the directory)", c.name));
                    continue;
                }
            };
            let mut zid: Option<String> = None;
            let mut bd = 300.0;
            for x in &cands {
                let z = &sc.zones[x];
                let d = math_hypot(&[(z.lng.num() - c.lng) * f.kx, (z.lat.num() - c.lat) * f.ky]);
                if d < bd {
                    bd = d;
                    zid = Some(x.clone());
                }
            }
            let zid = match zid {
                Some(z) => z,
                None => {
                    let id = ids.uid("z");
                    let mut z = new_zone(&id);
                    z.kind = Maybe::Set("zone".into());
                    z.existing = true.into();
                    z.src = Maybe::Set(json!("moe"));
                    z.name = Maybe::Set(c.name.clone());
                    z.lat = Num::number(r7(c.lat));
                    z.lng = Num::number(r7(c.lng));
                    z.lu = Maybe::Set(OrderedMap::new());
                    z.connectors = Maybe::Null;
                    sc.zones.insert(id.clone(), z);
                    cands.push(id.clone());
                    out.created += 1.0;
                    id
                }
            };
            let z = sc.zones.get_mut(&zid).expect("zone");
            let cur = crate::jsnum::or_default(lu_q(z, "childcare"), 0.0);
            let total = cur + q;
            lu_of(z).insert("childcare", Num::number(total));
            let me = z.extra.entry("moeEce".to_string()).or_insert_with(|| json!([]));
            if let Some(a) = me.as_array_mut() {
                a.push(json!(c.id));
            }
            let n_ece = z.extra.get("moeEce").and_then(Value::as_array).map_or(0, Vec::len);
            let p = prov(
                &consts::ECE,
                &when,
                vec![
                    ("basis", json!("measured")),
                    ("q", json_number(total)),
                    ("vintage", json!(match c.roll_date.as_deref() { Some(d) if !d.is_empty() => format!("roll at {d}"), _ => String::new() })),
                    ("note", json!(format!("MoE licensed places ({n_ece} service{})", if n_ece > 1 { "s" } else { "" }))),
                ],
            );
            obj(&mut z.extra, "luSrc").insert("childcare".into(), p);
            out.ece += 1.0;
        }
    }
    out
}

fn aup_at<'a>(a: &'a AupData, lat: f64, lng: f64) -> Option<(&'a super::parse::AupZone, f64)> {
    let at = |la: f64, ln: f64| a.zones.iter().find(|f| in_poly(&f.poly, ln, la));
    let here = at(lat, lng);
    if let Some(h) = here
        && !NOT_LAND.iter().any(|c| f64::from(*c) == h.code)
    {
        return Some((h, 0.0));
    }
    let k = util::flat(lat);
    for m in [20.0, 40.0, 80.0] {
        for i in 0..8 {
            let bearing = i as f64 * std::f64::consts::PI / 4.0;
            if let Some(f) = at(lat + m * jsmath::cos(bearing) / k.ky, lng + m * jsmath::sin(bearing) / k.kx)
                && !NOT_LAND.iter().any(|c| f64::from(*c) == f.code)
            {
                return Some((f, m));
            }
        }
    }
    here.map(|h| (h, 0.0))
}

/// The report of matching zones to Unitary Plan zones.
#[derive(Clone, Debug, PartialEq)]
pub struct AupReport {
    /// Zones matched to a Unitary Plan zone.
    pub zones: usize,
    /// Zoned land inside the study area by zone: code, name, group, hectares and polygon count.
    pub summary: Vec<Value>,
}

/// `aupToZones(sc, A, B)`: each zone gets the Unitary Plan zone at its centre (the nearest land-use zone within 80 m when it stands on road, water or coast), and the zoned land in the study area is totalled.
pub fn aup_to_zones(sc: &mut Scenario, ids: &dyn IdSource, a: &AupData, b: &Bx) -> AupReport {
    let when = ids.today();
    let mut n = 0usize;
    for id in land_zone_ids(sc, Some(b)) {
        let z = sc.zones.get_mut(&id).expect("zone");
        if let Some((f, near)) = aup_at(a, z.lat.num(), z.lng.num()) {
            let mut o = Map::new();
            o.insert("code".into(), json_number(f.code));
            o.insert("name".into(), json!(f.name));
            o.insert("group".into(), json!(f.group));
            o.insert("status".into(), json!(f.status));
            o.insert("date".into(), json!(when));
            o.insert("source".into(), json!(consts::AUP.name));
            o.insert("licence".into(), json!(consts::AUP.licence));
            o.insert("basis".into(), json!("derived"));
            if crate::jsnum::truthy_number(near) {
                o.insert("near".into(), json_number(near));
                o.insert("note".into(), json!(format!("Nearest zone within {} m: the zone centre is on road reserve or water", crate::jsnum::number_to_js_string(near))));
            }
            z.extra.insert("aup".into(), Value::Object(o));
            n += 1;
        }
    }
    struct Row {
        code: f64,
        name: String,
        group: String,
        ha: f64,
        n: f64,
    }
    let mut by: Vec<Row> = Vec::new();
    for f in &a.zones {
        if !bb_meets(&f.poly.bb, b) {
            continue;
        }
        let sh = if bb_in_box(&f.poly.bb, b) { 1.0 } else { sample_share(&f.poly, b, 16).share };
        if !(sh > 0.0) {
            continue;
        }
        let i = match by.iter().position(|r| r.code == f.code) {
            Some(i) => i,
            None => {
                by.push(Row { code: f.code, name: f.name.clone(), group: f.group.clone(), ha: 0.0, n: 0.0 });
                by.len() - 1
            }
        };
        by[i].ha += rings_area(&f.poly.rings) * sh / 1e4;
        by[i].n += 1.0;
    }
    // the hectares are rounded first, then the areas sorted by them (a stable sort)
    let mut rows: Vec<Row> = by.into_iter().map(|r| Row { ha: r1(r.ha), ..r }).collect();
    js_sort_by(&mut rows, |x, y| y.ha - x.ha);
    AupReport {
        zones: n,
        summary: rows.iter().map(|r| json!({ "code": json_number(r.code), "name": r.name, "group": r.group, "ha": json_number(r.ha), "n": json_number(r.n) })).collect(),
    }
}

/// What was downloaded for a fill (`data`): each source that answered.
#[derive(Clone, Debug, Default)]
pub struct FillData {
    /// OpenStreetMap elements (buildings and land-use areas).
    pub elements: Option<Vec<Value>>,
    /// Census SA1s.
    pub census: Option<CensusData>,
    /// Employees by SA2.
    pub employees: Option<EmployeeData>,
    /// Schools.
    pub schools: Option<Vec<Moe>>,
    /// Early learning services.
    pub ece: Option<Vec<Moe>>,
    /// Unitary Plan zones.
    pub aup: Option<AupData>,
}

/// The report of a fill (`apply`'s answer and what `fillAll` adds to it).
#[derive(Clone, Debug, PartialEq)]
pub struct FillReport {
    /// The date of the fill.
    pub at: String,
    /// The study area filled.
    pub bounds: Bx,
    /// Warnings.
    pub warnings: Vec<String>,
    /// Census report.
    pub census: Option<CensusReport>,
    /// Employees report.
    pub employees: Option<EmployeesReport>,
    /// Schools report.
    pub schools: Option<SchoolsReport>,
    /// Unitary Plan report.
    pub aup: Option<AupReport>,
    /// Which source was used for each item.
    pub choices: Vec<Value>,
    /// Zones first built from OpenStreetMap (a fill from `fill_all`).
    pub built_from_osm: Option<usize>,
    /// Scenarios the fill covered (a fill from `fill_all`).
    pub scenarios: Option<usize>,
}

impl FillReport {
    /// The report as JSON, in the prototype's shape.
    pub fn to_value(&self) -> Value {
        let mut o = Map::new();
        o.insert("at".into(), json!(self.at));
        o.insert("bounds".into(), self.bounds.to_value());
        o.insert("warnings".into(), json!(self.warnings));
        o.insert("census".into(), self.census.as_ref().map_or(Value::Null, CensusReport::to_value));
        o.insert("employees".into(), self.employees.as_ref().map_or(Value::Null, EmployeesReport::to_value));
        o.insert("schools".into(), self.schools.as_ref().map_or(Value::Null, SchoolsReport::to_value));
        o.insert("aup".into(), self.aup.as_ref().map_or(Value::Null, |a| json!({ "zones": a.zones, "summary": a.summary })));
        o.insert("choices".into(), Value::Array(self.choices.clone()));
        if let Some(b) = self.built_from_osm {
            o.insert("builtFromOSM".into(), json!(b));
        }
        if let Some(s) = self.scenarios {
            o.insert("scenarios".into(), json!(s));
        }
        Value::Object(o)
    }
}

/// `choices(o)`: which source was used for each item, with what it gave.
pub fn choices(c: Option<&CensusReport>, e: Option<&EmployeesReport>, s: Option<&SchoolsReport>, aup: Option<&AupReport>) -> Vec<Value> {
    let val = |id: &str| -> Option<String> {
        match id {
            "dwellings" => c.filter(|c| c.dwellings).map(|c| format!("Census {} (OSM {})", n0(Some(c.placed)), n0(Some(c.osm_total)))),
            "people" => c.map(|c| format!("{} SA1s", n0(Some(c.rows.len() as f64)))),
            "jobs" => e.map(|e| format!("{} employees (Census workplace {})", n0(Some(e.allocated)), n0(Some(e.census)))),
            "schools" => s.filter(|s| s.loaded_schools).map(|s| format!("{} schools (rolls replaced in existing zones: {} students)", s.schools.len(), n0(Some(s.replaced_school)))),
            "ece" => s.filter(|s| s.loaded_ece).map(|s| format!("{} services (places replaced in existing zones: {})", crate::jsnum::number_to_js_string(s.ece), n0(Some(s.replaced_childcare)))),
            "gfa" => Some("OSM".to_string()),
            "zoning" => aup.map(|a| format!("{} zones matched", a.zones)),
            _ => None,
        }
    };
    CHOICES
        .iter()
        .map(|ch| {
            let v = val(ch.id);
            json!({ "id": ch.id, "item": ch.item, "use": ch.used, "instead": ch.instead, "why": ch.why, "used": v.is_some(), "value": v })
        })
        .collect()
}

/// `apply(sc, data, opts)`: puts downloaded data in a scenario's zones. The study area is `bounds` when given, else the area of the project and scenario.
pub fn apply(p: &mut Project, sc_idx: usize, ids: &mut dyn IdSource, data: &FillData, opts: &ApplyOpts, bounds: Option<&Bx>) -> Result<FillReport, String> {
    let b = match bounds {
        Some(g) => Bx { basis: Some(g.basis.clone().filter(|b| !b.is_empty()).unwrap_or_else(|| "the area given".to_string())), ..g.clone() },
        None => match area_box(p, &p.scenarios[sc_idx]) {
            Some(b) => b,
            None => return Err("Add roads or zones first, so Spare Construct knows the study area.".to_string()),
        },
    };
    let sc = &mut p.scenarios[sc_idx];
    let mut out = FillReport {
        at: ids.today(),
        bounds: b.clone(),
        warnings: Vec::new(),
        census: None,
        employees: None,
        schools: None,
        aup: None,
        choices: Vec::new(),
        built_from_osm: None,
        scenarios: None,
    };
    let blds: Vec<Building> = prep_buildings(data.elements.as_deref().unwrap_or(&[]), &b);
    if let Some(c) = &data.census {
        let mut r = census_to_zones(sc, ids, c, &blds, &b, opts);
        r.buildings = Some(blds.len());
        out.warnings.extend(r.warnings.iter().cloned());
        if !c.missing.is_empty() {
            out.warnings.push(format!("Census fields not found: {}. Those values are left blank.", c.missing.join(", ")));
        }
        out.census = Some(r);
    }
    if let Some(e) = &data.employees {
        out.employees = Some(employees_to_zones(sc, ids, e, &b));
    }
    if data.schools.is_some() || data.ece.is_some() {
        out.schools = Some(schools_to_zones(sc, ids, data.schools.as_deref().unwrap_or(&[]), data.ece.as_deref().unwrap_or(&[]), &b, opts, data.schools.is_some(), data.ece.is_some()));
    }
    if let Some(a) = &data.aup {
        out.aup = Some(aup_to_zones(sc, ids, a, &b));
    }
    out.choices = choices(out.census.as_ref(), out.employees.as_ref(), out.schools.as_ref(), out.aup.as_ref());
    Ok(out)
}

/// `ExistingLU.build` on a scenario's roads for a study area, as `fillAll` and the study-area setup do it.
fn build_existing(sc: &mut Scenario, ids: &mut dyn IdSource, elements: &[Value], b: &Bx, cell: Option<f64>) -> usize {
    let sbox = StudyBox { s: b.s, w: b.w, n: b.n, e: b.e };
    let r = landuse::build(&Value::Array(elements.to_vec()), Some(&sbox), ids, &BuildOpts { cell, links: landuse::road_lines(sc) });
    let n = r.zones.len();
    for mut z in r.zones {
        z.src = Maybe::Set(json!("existing-lu"));
        sc.zones.insert(z.id.clone(), z);
    }
    n
}

/// What `fillAll` is asked to do.
#[derive(Clone, Debug, Default)]
pub struct FillOpts {
    /// The study area (else the project's own).
    pub bounds: Option<Bx>,
    /// Which sources to use; a source left out is not downloaded. None means all.
    pub want_census: Option<bool>,
    /// Use employees (default true).
    pub want_employees: Option<bool>,
    /// Use schools and early learning (default true).
    pub want_schools: Option<bool>,
    /// Use Unitary Plan zones (default true).
    pub want_aup: Option<bool>,
    /// Use OpenStreetMap buildings (default true).
    pub want_osm: Option<bool>,
    /// OpenStreetMap elements already downloaded, so they are not fetched twice.
    pub elements: Option<Vec<Value>>,
    /// Fill every scenario, not only the one given.
    pub all: bool,
    /// How the fill places things.
    pub apply: ApplyOpts,
}

/// What a fill remembers between calls, so rolls can be entered after it (`lastData`).
#[derive(Clone, Debug, Default)]
pub struct LastFill {
    /// The data of the last fill.
    pub data: FillData,
    /// The study area of the last fill.
    pub bounds: Option<Bx>,
}

/// `fillAll(sc, opts, onStatus)`: downloads and applies everything for a scenario (or every scenario with `opts.all`). Each source fails soft on its own.
/// Every changed scenario's results are marked out of date.
#[allow(clippy::too_many_arguments)]
pub fn fill_all(
    p: &mut Project,
    sc_idx: usize,
    ids: &mut dyn IdSource,
    net: &mut dyn Net,
    dl: &mut Downloads,
    last: &mut LastFill,
    opts: &FillOpts,
    status: &mut dyn FnMut(&str),
) -> Result<FillReport, String> {
    let b = match &opts.bounds {
        Some(g) => Bx { basis: Some("the area given".to_string()), ..g.clone() },
        None => area_box(p, &p.scenarios[sc_idx]).ok_or_else(|| "Add roads or zones first, so Spare Construct knows the study area.".to_string())?,
    };
    let want = |o: Option<bool>| o.unwrap_or(true);
    let mut data = FillData::default();
    let mut warnings: Vec<String> = Vec::new();
    if let Some(el) = &opts.elements {
        data.elements = Some(el.clone());
    } else if want(opts.want_osm) {
        match dl.existing_land_use(net, &b, status) {
            Ok(e) => data.elements = Some(e),
            Err(e) => warnings.push(e),
        }
    }
    if want(opts.want_census) {
        match dl.census(net, &b, status) {
            Ok(c) => data.census = Some(c),
            Err(e) => warnings.push(e),
        }
    }
    if let Some(c) = &data.census {
        warnings.extend(c.errors.iter().cloned());
    }
    if want(opts.want_employees) {
        match dl.employees(net, &b, status) {
            Ok(e) => data.employees = Some(e),
            Err(e) => warnings.push(e),
        }
    }
    if want(opts.want_schools) {
        match dl.moe(net, "schools", &b, status) {
            Ok(s) => data.schools = Some(s),
            Err(e) => warnings.push(e),
        }
        match dl.moe(net, "ece", &b, status) {
            Ok(s) => data.ece = Some(s),
            Err(e) => warnings.push(e),
        }
    }
    if want(opts.want_aup) {
        match dl.aup(net, &b, status) {
            Ok(a) => data.aup = Some(a),
            Err(e) => warnings.push(e),
        }
    }
    let targets: Vec<usize> = if opts.all { (0..p.scenarios.len()).collect() } else { vec![sc_idx] };
    let mut out: Option<FillReport> = None;
    let mut built = 0usize;
    for &si in &targets {
        if let Some(el) = &data.elements {
            let has_existing = land_zone_ids(&p.scenarios[si], Some(&b)).iter().any(|id| p.scenarios[si].zones.get(id).is_some_and(is_existing));
            if !has_existing {
                let n = build_existing(&mut p.scenarios[si], ids, el, &b, None);
                if si == sc_idx {
                    built = n;
                }
            }
        }
        let o = apply(p, si, ids, &data, &opts.apply, Some(&b))?;
        mark_stale(&mut p.scenarios[si]);
        if si == sc_idx {
            out = Some(o);
        }
    }
    let mut out = out.expect("the scenario filled");
    let mut w = warnings;
    w.extend(out.warnings.drain(..));
    out.warnings = w;
    out.built_from_osm = Some(built);
    out.scenarios = Some(targets.len());
    last.data = data;
    last.bounds = Some(b);
    record(p, &out);
    Ok(out)
}

/// `record(out)`: saves a compact summary (no polygons) with provenance in the project (`P.landuse`).
pub fn record(p: &mut Project, out: &FillReport) {
    let l = obj(&mut p.extra, "landuse");
    l.insert("bounds".into(), out.bounds.to_value());
    let last = json!({
        "at": out.at, "bounds": out.bounds.to_value(), "census": out.census.as_ref().map(CensusReport::to_value), "employees": out.employees.as_ref().map(EmployeesReport::to_value),
        "schools": out.schools.as_ref().map(SchoolsReport::to_value), "aup": out.aup.as_ref().map(|a| json!({ "zones": a.zones })), "choices": out.choices,
        "warnings": out.warnings, "builtFromOSM": out.built_from_osm.unwrap_or(0), "scenarios": out.scenarios.unwrap_or(1),
    });
    l.insert("last".into(), last);
    if let Some(a) = &out.aup {
        l.insert("aupSummary".into(), Value::Array(a.summary.clone()));
    }
    let src = obj(l, "sources");
    if let Some(c) = &out.census {
        src.insert("census".into(), json!({ "name": consts::CENSUS.name, "vintage": consts::CENSUS.vintage, "licence": consts::CENSUS.licence, "at": out.at, "n": c.rows.len() }));
    }
    if let Some(e) = &out.employees {
        src.insert("employees".into(), json!({ "name": consts::EMPLOYEES.name, "vintage": format!("February {}", crate::jsnum::number_to_js_string(e.year)), "licence": consts::EMPLOYEES.licence, "at": out.at, "n": e.rows.len() }));
    }
    if let Some(s) = &out.schools {
        src.insert("moe".into(), json!({ "name": "Ministry of Education, Directory of Educational Institutions", "licence": consts::SCHOOLS.licence, "at": out.at, "n": json_number(s.schools.len() as f64 + s.ece) }));
    }
    if let Some(a) = &out.aup {
        src.insert("aup".into(), json!({ "name": consts::AUP.name, "licence": consts::AUP.licence, "at": out.at, "n": a.summary.len() }));
    }
}

/// What `rollsReady` found: the schools waiting for a roll and the study area, or why there are none.
#[derive(Clone, Debug, PartialEq)]
pub struct RollsReady {
    /// Schools to ask a roll for (their `roll` is None).
    pub list: Vec<Moe>,
    /// The study area of the last fill.
    pub bounds: Option<Bx>,
    /// Why the list is empty, when it is.
    pub why: Option<String>,
}

fn moe_from_saved(s: &Value, lat: f64, lng: f64) -> Moe {
    let text = |k: &str| s.get(k).filter(|x| value_truthy(x)).map(value_to_js_string);
    Moe {
        id: s.get("id").map_or("undefined".to_string(), value_to_js_string),
        kind: "schools".into(),
        name: s.get("name").map_or(String::new(), |x| if x.is_null() { String::new() } else { value_to_js_string(x) }),
        kind_of_org: s.get("type").map_or(String::new(), |x| if x.is_null() { String::new() } else { value_to_js_string(x) }),
        roll: None,
        places: None,
        roll_date: text("rollDate"),
        lat,
        lng,
        status: s.get("status").map_or(String::new(), |x| if x.is_null() { String::new() } else { value_to_js_string(x) }),
        opens: s.get("opens").filter(|x| !x.is_null()).map(value_to_number),
    }
}

/// `rollsReady(rolls, P)`: the schools the last fill had no roll for (saved with the project, with their location), or why there are none.
pub fn rolls_ready(p: &Project, last: &LastFill, rolls: Option<&[(String, Value)]>) -> RollsReady {
    let l = p.extra.get("landuse");
    let saved: Vec<Value> = l.and_then(|l| l.get("last")).and_then(|x| x.get("schools")).and_then(|x| x.get("needsRoll")).and_then(Value::as_array).cloned().unwrap_or_default();
    let b = l.and_then(|l| l.get("bounds")).and_then(box_of);
    let run: Vec<&Moe> = match (&last.data.schools, &b, &last.bounds) {
        (Some(s), Some(b), Some(lb)) if box_key(lb) == box_key(b) => s.iter().collect(),
        _ => Vec::new(),
    };
    let mut list: Vec<Moe> = Vec::new();
    for s in saved.iter().filter(|s| value_truthy(s)) {
        let id = s.get("id").map_or("undefined".to_string(), value_to_js_string);
        if let Some(r) = rolls
            && !r.iter().any(|(k, v)| *k == id && !v.is_null())
        {
            continue;
        }
        let (lat, lng) = (s.get("lat"), s.get("lng"));
        let ok = lat.is_some_and(|x| !x.is_null() && value_to_number(x).is_finite()) && lng.is_some_and(|x| !x.is_null() && value_to_number(x).is_finite());
        if ok {
            list.push(moe_from_saved(s, value_to_number(lat.expect("lat")), value_to_number(lng.expect("lng"))));
        } else if let Some(m) = run.iter().rev().find(|m| m.id == id) {
            let mut m = (*m).clone();
            m.roll = None;
            list.push(m);
        }
    }
    if !list.is_empty() && b.is_some() {
        return RollsReady { list, bounds: b, why: None };
    }
    RollsReady {
        list: Vec::new(),
        bounds: b,
        why: Some(if saved.is_empty() {
            "No schools are waiting for a roll. Fill existing land use again if the directory has changed.".to_string()
        } else {
            "This project was filled before school locations were saved with it. Fill existing land use again, then enter the rolls.".to_string()
        }),
    }
}

/// `applyRolls(sc, rolls)`: adds the schools the directory had no roll for, with the user's numbers, to the given scenarios. None when there is nothing to apply.
pub fn apply_rolls(p: &mut Project, last: &LastFill, ids: &mut dyn IdSource, targets: &[usize], rolls: &[(String, Value)]) -> Option<SchoolsReport> {
    let r = rolls_ready(p, last, Some(rolls));
    if r.list.is_empty() || targets.is_empty() {
        return None;
    }
    let b = r.bounds.expect("bounds");
    let mut first: Option<SchoolsReport> = None;
    for &si in targets {
        let o = schools_to_zones(&mut p.scenarios[si], ids, &r.list, &[], &b, &ApplyOpts { rolls: rolls.to_vec(), replace: Some(false), ..Default::default() }, false, false);
        if first.is_none() {
            first = Some(o);
        }
        mark_stale(&mut p.scenarios[si]);
    }
    let r = first?;
    if let Some(s) = p.extra.get_mut("landuse").and_then(|l| l.get_mut("last")).and_then(|l| l.get_mut("schools")).filter(|s| value_truthy(s)) {
        let done: Vec<String> = r.schools.iter().map(|y| y.get("id").map_or("undefined".to_string(), value_to_js_string)).collect();
        let need: Vec<Value> = s.get("needsRoll").and_then(Value::as_array).cloned().unwrap_or_default().into_iter().filter(|x| !done.contains(&x.get("id").map_or("undefined".to_string(), value_to_js_string))).collect();
        s["needsRoll"] = Value::Array(need);
        if !s.get("schools").is_some_and(value_truthy) {
            s["schools"] = json!([]);
        }
        if let Some(a) = s["schools"].as_array_mut() {
            a.extend(r.schools.iter().cloned());
        }
        let created = s.get("created").map_or(0.0, |c| crate::jsnum::or_default(value_to_number(c), 0.0));
        s["created"] = json_number(created + r.created);
    }
    Some(r)
}
