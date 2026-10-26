//! Reading the downloaded answers: Census 2023 by SA1, employees by SA2, the Ministry of Education directory and the Unitary Plan zones.

use super::consts::{self, VarSpec, aup_name};
use super::geo::{Poly, Ring, rings_of};
use crate::jsnum::{js_trim, number_to_js_string, value_to_js_string, value_to_number, value_truthy};
use serde_json::{Map, Value, json};

/// One downloaded ArcGIS layer: its field list (from `?f=json`) and its features.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layer {
    /// The layer's fields, each `{ name, alias, domain }`.
    pub fields: Vec<Value>,
    /// The features, each `{ attributes, geometry }`.
    pub features: Vec<Value>,
}

impl Layer {
    /// A layer from the layer description (`meta.fields`) and the features.
    pub fn from_meta(meta: &Value, features: Vec<Value>) -> Layer {
        Layer { fields: meta.get("fields").and_then(Value::as_array).cloned().unwrap_or_default(), features }
    }
}

/// How a Census item was found in a layer: its field name (None when missing) and whether by alias or by code.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    /// The item key ("pop", "r.drivePriv").
    pub key: String,
    /// The field name, or None when the layer has no such field.
    pub name: Option<String>,
    /// "alias", "code" or "missing".
    pub by: &'static str,
}

fn lower(v: Option<&Value>) -> String {
    match v {
        Some(x) if value_truthy(x) => value_to_js_string(x).to_lowercase(),
        _ => String::new(),
    }
}

fn field_name(f: &Value) -> Option<&str> {
    f.get("name").and_then(Value::as_str)
}

/// `resolve(fields, spec, year)`: finds each item's field by its alias text (the layer's year must say "Year: 2023"), else by its known code, else reports it missing.
pub fn resolve(fields: &[Value], spec: &[VarSpec], year: f64) -> Vec<Resolved> {
    let yr = format!("year: {}", number_to_js_string(year));
    spec.iter()
        .map(|v| {
            let want = v.alias.to_lowercase();
            let hit = fields.iter().find(|f| {
                let a = lower(f.get("alias"));
                a.contains(&yr) && a.ends_with(&want)
            });
            match hit {
                Some(f) => Resolved { key: v.key.clone(), name: f.get("name").filter(|x| !x.is_null()).map(value_to_js_string), by: "alias" },
                None if fields.iter().any(|f| field_name(f) == Some(v.code.as_str())) || fields.is_empty() => {
                    Resolved { key: v.key.clone(), name: Some(v.code.clone()), by: "code" }
                }
                None => Resolved { key: v.key.clone(), name: None, by: "missing" },
            }
        })
        .collect()
}

// A code field name such as SA12023_V1_00: the prefix, four digits, "_V", one or more digits, "_00"
fn is_code_field(name: &str, prefix: &str) -> bool {
    let Some(rest) = name.strip_prefix(prefix) else { return false };
    let b = rest.as_bytes();
    if b.len() < 4 || !b[..4].iter().all(u8::is_ascii_digit) {
        return false;
    }
    let Some(rest) = rest[4..].strip_prefix("_V") else { return false };
    let Some(digits) = rest.strip_suffix("_00") else { return false };
    !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit())
}

/// `codeField(fields, re)` for the SA1 code field (`SA1\d{4}_V\d+_00`), or None.
pub fn sa1_code_field(fields: &[Value]) -> Option<String> {
    fields.iter().find_map(|f| field_name(f).filter(|n| is_code_field(n, "SA1")).map(str::to_string))
}

/// `codeField(fields, re)` for the SA2 code field (`SA2\d{4}_V\d+_00`), or None.
pub fn sa2_code_field(fields: &[Value]) -> Option<String> {
    fields.iter().find_map(|f| field_name(f).filter(|n| is_code_field(n, "SA2")).map(str::to_string))
}

/// `val(a, name)`: a count from a feature's attributes; missing, non-numeric and negative cells (suppressed) are None.
pub fn val(a: &Value, name: Option<&str>) -> Option<f64> {
    let name = name?;
    let v = a.get(name)?;
    if v.is_null() {
        return None;
    }
    let x = value_to_number(v);
    if !x.is_finite() || x < 0.0 { None } else { Some(x) }
}

/// `setP(o, k, v)`: sets an item, "a.b" going into the object a.
fn set_p(o: &mut Map<String, Value>, k: &str, v: Option<f64>) {
    let value = v.map_or(Value::Null, crate::jsnum::json_number);
    match k.split_once('.') {
        None => {
            o.insert(k.to_string(), value);
        }
        Some((p0, p1)) => {
            let e = o.entry(p0.to_string()).or_insert_with(|| Value::Object(Map::new()));
            if !e.is_object() {
                *e = Value::Object(Map::new());
            }
            e.as_object_mut().expect("an object").insert(p1.to_string(), value);
        }
    }
}

/// One Census SA1: its code, outline and the values of each item (null where suppressed).
#[derive(Clone, Debug, PartialEq)]
pub struct Sa1 {
    /// SA1 code.
    pub code: String,
    /// The outline.
    pub poly: Poly,
    /// The items: pop, hh, occ, sep, joined, otherPriv, empWork, r, w, e (mode counts) and veh, each a number or null.
    pub v: Map<String, Value>,
}

impl Sa1 {
    /// A top-level item as a number (None for null or missing).
    pub fn item(&self, k: &str) -> Option<f64> {
        self.v.get(k).and_then(Value::as_f64)
    }
}

/// The parsed Census: the SA1s, how each item was found and what was missing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CensusData {
    /// SA1s with an outline, in the order they first appeared.
    pub sa1: Vec<Sa1>,
    /// How each item was found (key, field and how).
    pub fields: Vec<Resolved>,
    /// Items and layers that were not found.
    pub missing: Vec<String>,
    /// Messages of the layers that did not download.
    pub errors: Vec<String>,
    /// A layer failed, so the result is partial and a retry fetches again.
    pub partial: bool,
}

/// The four Census layers as downloaded; one that failed is None.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CensusLayers {
    /// Population by SA1 (ind0).
    pub ind0: Option<Layer>,
    /// Travel to work and education, workplace industry (ind2).
    pub ind2: Option<Layer>,
    /// Dwellings (dw).
    pub dw: Option<Layer>,
    /// Households and vehicles (hh).
    pub hh: Option<Layer>,
}

impl CensusLayers {
    /// The layer with this key ("ind0", "ind2", "dw", "hh").
    pub fn get(&self, k: &str) -> Option<&Layer> {
        match k {
            "ind0" => self.ind0.as_ref(),
            "ind2" => self.ind2.as_ref(),
            "dw" => self.dw.as_ref(),
            "hh" => self.hh.as_ref(),
            _ => None,
        }
    }
}

/// `parseCensus(layers)`: the SA1s with their values from whichever layers loaded, how each item was found, and what is missing.
pub fn parse_census(layers: &CensusLayers) -> CensusData {
    let mut by_code: Vec<(String, Sa1)> = Vec::new();
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut fields: Vec<Resolved> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for (lk, spec) in consts::census_vars() {
        let Some(l) = layers.get(lk) else {
            missing.push(format!("{lk} layer"));
            continue;
        };
        let cf = sa1_code_field(&l.fields).unwrap_or_else(|| "SA12023_V1_00".to_string());
        let res = resolve(&l.fields, &spec, 2023.0);
        for r in &res {
            match fields.iter_mut().find(|f| f.key == r.key) {
                Some(f) => *f = r.clone(),
                None => fields.push(r.clone()),
            }
            if r.name.is_none() {
                missing.push(r.key.clone());
            }
        }
        for f in &l.features {
            let empty = json!({});
            let a = f.get("attributes").filter(|x| value_truthy(x)).unwrap_or(&empty);
            let code = match a.get(&cf) {
                None | Some(Value::Null) => String::new(),
                Some(x) => value_to_js_string(x),
            };
            if code.is_empty() {
                continue;
            }
            let i = *index.entry(code.clone()).or_insert_with(|| {
                by_code.push((code.clone(), Sa1 { code: code.clone(), poly: Poly { rings: Vec::new(), bb: [f64::NAN; 4] }, v: Map::new() }));
                by_code.len() - 1
            });
            let o = &mut by_code[i].1;
            let rg = rings_of(f.get("geometry"));
            if !rg.is_empty() && o.poly.rings.is_empty() {
                o.poly = Poly::new(rg);
            }
            for r in &res {
                set_p(&mut o.v, &r.key, val(a, r.name.as_deref()));
            }
        }
    }
    CensusData {
        sa1: by_code.into_iter().map(|(_, o)| o).filter(|o| !o.poly.rings.is_empty()).collect(),
        fields,
        missing,
        errors: Vec::new(),
        partial: false,
    }
}

/// `censusFields(lk)(fields)`: the field names to ask a Census layer for: the SA1 code field and every item that was found.
pub fn census_fields(lk: &str, fields: &[Value]) -> Vec<String> {
    let mut out = vec![sa1_code_field(fields).unwrap_or_else(|| "SA12023_V1_00".to_string())];
    out.extend(resolve(fields, &consts::census_vars_of(lk), 2023.0).into_iter().filter_map(|r| r.name));
    out
}

/// One SA2 with its employee count.
#[derive(Clone, Debug, PartialEq)]
pub struct Sa2 {
    /// SA2 code.
    pub code: String,
    /// SA2 name (the code when the layer has no name).
    pub name: String,
    /// Employees (None when suppressed).
    pub n: Option<f64>,
    /// Year of the count.
    pub year: f64,
    /// Average annual change (%), if the layer has it.
    pub growth: Option<f64>,
    /// The outline.
    pub poly: Poly,
}

/// The parsed employee counts.
#[derive(Clone, Debug, PartialEq)]
pub struct EmployeeData {
    /// SA2s with an outline.
    pub sa2: Vec<Sa2>,
    /// Year of the latest count in the layer.
    pub year: f64,
    /// The count field ("ec2025").
    pub field: String,
}

// /^ec(\d{4})$/
fn ec_year(name: &str) -> Option<f64> {
    let d = name.strip_prefix("ec")?;
    if d.len() == 4 && d.bytes().all(|c| c.is_ascii_digit()) { d.parse().ok() } else { None }
}

/// `parseEmployees(L)`: SA2 employee counts, using the latest `ecYYYY` field in the layer.
pub fn parse_employees(l: &Layer) -> EmployeeData {
    let fl = &l.fields;
    let cf = sa2_code_field(fl).unwrap_or_else(|| "SA22023_V1_00".to_string());
    let nf = format!("{cf}_NAME");
    let nf = fl.iter().any(|f| field_name(f) == Some(nf.as_str())).then_some(nf);
    let years: Vec<f64> = fl.iter().filter_map(|f| field_name(f).and_then(ec_year)).collect();
    let year = if years.is_empty() { 2025.0 } else { crate::jsnum::math_max_of(&years) };
    let ef = format!("ec{}", number_to_js_string(year));
    let gf = (fl.iter().any(|f| field_name(f) == Some("ec_avperinc")) || fl.is_empty()).then_some("ec_avperinc");
    let sa2 = l
        .features
        .iter()
        .filter_map(|f| {
            let empty = json!({});
            let a = f.get("attributes").filter(|x| value_truthy(x)).unwrap_or(&empty);
            let rg = rings_of(f.get("geometry"));
            let code = match a.get(&cf) {
                None | Some(Value::Null) => String::new(),
                Some(x) => value_to_js_string(x),
            };
            let name = match nf.as_deref().and_then(|n| a.get(n)) {
                Some(x) if value_truthy(x) => value_to_js_string(x),
                _ => code.clone(),
            };
            let growth = gf.and_then(|g| a.get(g)).filter(|x| !x.is_null() && value_to_number(x).is_finite()).map(value_to_number);
            if code.is_empty() || rg.is_empty() {
                return None;
            }
            Some(Sa2 { code, name, n: val(a, Some(&ef)), year, growth, poly: Poly::new(rg) })
        })
        .collect();
    EmployeeData { sa2, year, field: ef }
}

/// The Ministry of Education fields asked for, by directory ("schools" or "ece").
pub fn moe_fields(kind: &str) -> &'static [&'static str] {
    if kind == "ece" {
        &["ECE_Id", "Org_Name", "Org_Type", "All_Children", "Total", "Roll_Date", "Latitude", "Longitude"]
    } else {
        &["School_Id", "Org_Name", "Org_Type", "Total", "Roll_Date", "Latitude", "Longitude", "Status"]
    }
}

/// One school or early learning service from the Ministry of Education directory.
#[derive(Clone, Debug, PartialEq)]
pub struct Moe {
    /// Directory id (or the name when there is none).
    pub id: String,
    /// "schools" or "ece".
    pub kind: String,
    /// Name without any proposed opening date.
    pub name: String,
    /// Organisation type.
    pub kind_of_org: String,
    /// Roll (students).
    pub roll: Option<f64>,
    /// Licensed places (early learning only).
    pub places: Option<f64>,
    /// Roll date (YYYY-MM-DD).
    pub roll_date: Option<String>,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lng: f64,
    /// Status ("Open", "Proposed", ...).
    pub status: String,
    /// Opening year of a proposed school.
    pub opens: Option<f64>,
}

impl Moe {
    /// The record as JSON, in the prototype's shape (`type` is the organisation type).
    pub fn to_value(&self) -> Value {
        let n = |x: Option<f64>| x.map_or(Value::Null, crate::jsnum::json_number);
        json!({ "id": self.id, "kind": self.kind, "name": self.name, "type": self.kind_of_org, "roll": n(self.roll), "places": n(self.places),
            "rollDate": self.roll_date, "lat": crate::jsnum::json_number(self.lat), "lng": crate::jsnum::json_number(self.lng), "status": self.status, "opens": n(self.opens) })
    }
}

// First case-insensitive (ASCII) occurrence of an ASCII needle, as a byte index
fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    (0..=h.len() - n.len()).find(|&i| h[i..i + n.len()].eq_ignore_ascii_case(n))
}

// /\(Proposed opening date:\s*(\d{4})-(\d\d)-(\d\d)\)/i: the year of the first match
fn proposed_year(name: &str) -> Option<f64> {
    const LIT: &str = "(proposed opening date:";
    let mut from = 0;
    while let Some(i) = find_ci(&name[from..], LIT) {
        let at = from + i + LIT.len();
        let rest = name[at..].trim_start_matches(crate::jsnum::is_js_space);
        let b = rest.as_bytes();
        let ok = b.len() >= 11
            && b[..4].iter().all(u8::is_ascii_digit)
            && b[4] == b'-'
            && b[5..7].iter().all(u8::is_ascii_digit)
            && b[7] == b'-'
            && b[8..10].iter().all(u8::is_ascii_digit)
            && b[10] == b')';
        if ok {
            return rest[..4].parse().ok();
        }
        from = from + i + 1;
    }
    None
}

// name.replace(/\s*\(Proposed opening date:[^)]*\)/i, '')
fn strip_proposed(name: &str) -> String {
    const LIT: &str = "(proposed opening date:";
    let mut from = 0;
    while let Some(i) = find_ci(&name[from..], LIT) {
        let start = from + i;
        let after = start + LIT.len();
        if let Some(close) = name[after..].find(')') {
            let end = after + close + 1;
            let head = name[..start].trim_end_matches(crate::jsnum::is_js_space);
            return format!("{head}{}", &name[end..]);
        }
        from = start + 1;
    }
    name.to_string()
}

fn plain_text(v: Option<&Value>) -> String {
    match v {
        Some(x) if value_truthy(x) => value_to_js_string(x),
        _ => String::new(),
    }
}

/// `parseMoE(records, kind)`: directory records with a location that are not closed.
pub fn parse_moe(records: &[Value], kind: &str) -> Vec<Moe> {
    let num = |v: Option<&Value>| -> Option<f64> {
        let v = v?;
        if v.is_null() || v.as_str() == Some("") {
            return None;
        }
        let x = value_to_number(v);
        x.is_finite().then_some(x)
    };
    records
        .iter()
        .filter_map(|r| {
            let name = js_trim(&plain_text(r.get("Org_Name"))).to_string();
            let m = proposed_year(&name);
            let id = {
                let a = plain_text(r.get("School_Id"));
                if !a.is_empty() {
                    a
                } else {
                    let b = plain_text(r.get("ECE_Id"));
                    if b.is_empty() { name.clone() } else { b }
                }
            };
            let roll_date: String = plain_text(r.get("Roll_Date")).chars().take(10).collect();
            let status = match r.get("Status") {
                Some(x) if value_truthy(x) => value_to_js_string(x),
                _ => if m.is_some() { "Proposed".to_string() } else { "Open".to_string() },
            };
            let (lat, lng) = (num(r.get("Latitude")), num(r.get("Longitude")));
            let rec = Moe {
                id,
                kind: kind.to_string(),
                name: js_trim(&strip_proposed(&name)).to_string(),
                kind_of_org: plain_text(r.get("Org_Type")),
                roll: num(r.get("Total")),
                places: if kind == "ece" { num(r.get("All_Children")) } else { None },
                roll_date: if roll_date.is_empty() { None } else { Some(roll_date) },
                lat: lat.unwrap_or(f64::NAN),
                lng: lng.unwrap_or(f64::NAN),
                status,
                opens: m,
            };
            (lat.is_some() && lng.is_some() && !rec.status.eq_ignore_ascii_case("closed")).then_some(rec)
        })
        .collect()
}

/// One Unitary Plan base zone polygon.
#[derive(Clone, Debug, PartialEq)]
pub struct AupZone {
    /// Zone code.
    pub code: f64,
    /// Zone name.
    pub name: String,
    /// Zone group ("Residential", "Business", ...).
    pub group: String,
    /// Version status ("Operative in part", ...).
    pub status: String,
    /// The outline.
    pub poly: Poly,
}

/// The parsed Unitary Plan zones.
#[derive(Clone, Debug, PartialEq)]
pub struct AupData {
    /// The zone polygons.
    pub zones: Vec<AupZone>,
    /// The zone names the layer's coded domain gives, by code.
    pub names: Vec<(String, String)>,
}

fn domain(fl: &[Value], name: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let coded = fl
        .iter()
        .find(|f| field_name(f) == Some(name))
        .and_then(|f| f.get("domain"))
        .and_then(|d| d.get("codedValues"))
        .and_then(Value::as_array);
    for c in coded.into_iter().flatten() {
        let k = c.get("code").map_or("undefined".to_string(), value_to_js_string);
        let v = c.get("name").map_or(String::new(), |x| if value_truthy(x) { value_to_js_string(x) } else { String::new() });
        match out.iter_mut().find(|(ek, _)| *ek == k) {
            Some(e) => e.1 = v,
            None => out.push((k, v)),
        }
    }
    out
}

fn dom_get(d: &[(String, String)], key: &str) -> String {
    d.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or_default()
}

/// `parseAUP(L)`: the zone polygons with their names, groups and status from the layer's coded domains.
pub fn parse_aup(l: &Layer) -> AupData {
    let fl = &l.fields;
    let (zn, gn, vs) = (domain(fl, "ZONE"), domain(fl, "GROUPZONE"), domain(fl, "VERSIONSTATUS"));
    let zones = l
        .features
        .iter()
        .filter_map(|f| {
            let empty = json!({});
            let a = f.get("attributes").filter(|x| value_truthy(x)).unwrap_or(&empty);
            let rg: Vec<Ring> = rings_of(f.get("geometry"));
            let code = a.get("ZONE").map_or(f64::NAN, value_to_number);
            let key = |k: &str| a.get(k).map_or("undefined".to_string(), value_to_js_string);
            let mut name = dom_get(&zn, &number_to_js_string(code));
            if name.is_empty() {
                name = aup_name(code).map_or_else(|| format!("AUP zone {}", number_to_js_string(code)), str::to_string);
            }
            let group = dom_get(&gn, &key("GROUPZONE"));
            let status = dom_get(&vs, &key("VERSIONSTATUS"));
            (!rg.is_empty() && code.is_finite()).then(|| AupZone { code, name, group, status, poly: Poly::new(rg) })
        })
        .collect();
    AupData { zones, names: zn }
}
