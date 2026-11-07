//! The downloads of the land use feature: the requests it makes, how it pages and how it words a failure (landuse-plus.js "fetching").
//!
//! This crate does no HTTP. The host provides [`Net`] (one GET or POST that gives back JSON, or a [`Failure`]); the functions here
//! build the same requests the prototype sends, read the answers the same way and give the same messages when something goes wrong.

use super::consts::{self, CKAN, TIMEOUT_MS, census_layer};
use super::geo::{Bx, box_key};
use super::parse::{
    AupData, CensusData, CensusLayers, EmployeeData, Layer, Moe, census_fields, moe_fields,
    parse_aup, parse_census, parse_employees, parse_moe, sa2_code_field,
};
use crate::importers::OVERPASS;
use crate::jsnum::{to_fixed, value_to_js_string, value_to_number, value_truthy};
use crate::landuse::{self, StudyBox};
use crate::util::fmt;
use serde_json::Value;
use std::collections::HashMap;

/// Why a download gave no JSON answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// The server did not answer within the time limit.
    Timeout,
    /// The server could not be reached; the text is the network error.
    Unreachable(String),
    /// The server answered with this HTTP status (not 2xx).
    Http(u16),
    /// The answer stalled while being read.
    ReadTimeout,
    /// The answer could not be read as JSON; the text is the parser's message.
    Unreadable(String),
    /// The user cancelled the download; the text is the host's message. The land use downloads word this like a timeout, the count and crash downloads stop.
    Cancelled(String),
}

impl Failure {
    /// The text a failed `fetch` carries in the prototype ("host answered HTTP 429", the network error, the parser error).
    pub fn message(&self, url: &str) -> String {
        match self {
            Failure::Timeout => "The operation was aborted".to_string(),
            Failure::Unreachable(m) | Failure::Unreadable(m) | Failure::Cancelled(m) => m.clone(),
            Failure::Http(s) => format!("{} answered HTTP {s}", host_of(url)),
            Failure::ReadTimeout => "The operation was aborted".to_string(),
        }
    }
}

/// The host's way to download: one GET or one form POST, giving the answer as JSON.
pub trait Net {
    /// GETs a web address and reads the answer as JSON.
    fn get_json(&mut self, url: &str) -> Result<Value, Failure>;
    /// POSTs a form (`Content-Type: application/x-www-form-urlencoded`, an Overpass query as `data=...`) and reads the answer as JSON.
    fn post_form(&mut self, url: &str, body: &str) -> Result<Value, Failure>;
}

/// `new URL(url).host`: the host name (and port) of a web address.
pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let auth = &rest[..end];
    auth.rsplit_once('@').map_or(auth, |(_, h)| h).to_string()
}

fn pct(out: &mut String, b: u8) {
    out.push('%');
    out.push_str(&format!("{b:02X}"));
}

/// `URLSearchParams` encoding of a name or value: letters, digits and `*-._` stay, a space is `+`, the rest is percent-encoded by UTF-8 byte.
pub fn form_encode(s: &str) -> String {
    let mut out = String::new();
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'*' | b'-' | b'.' | b'_') {
            out.push(b as char);
        } else if b == b' ' {
            out.push('+');
        } else {
            pct(&mut out, b);
        }
    }
    out
}

/// `encodeURIComponent(s)`: letters, digits and `-_.!~*'()` stay, the rest is percent-encoded by UTF-8 byte.
pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            )
        {
            out.push(b as char);
        } else {
            pct(&mut out, b);
        }
    }
    out
}

/// `new URLSearchParams(pairs).toString()`.
pub fn query_string(pairs: &[(&str, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", form_encode(k), form_encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

const REJECTED: &str = "the service rejected the query; its fields may have changed. Enter these values by hand for now";

/// `getJSON(url, label, timeout)`: what a download gave, as the answer or the prototype's message.
///
/// A non-2xx status from 400 to 499 says the service rejected the query; other statuses say try again later; an answer with an
/// `error` member is a failure worded with the service's own message.
pub fn checked_answer(
    url: &str,
    label: &str,
    timeout_ms: f64,
    got: Result<Value, Failure>,
) -> Result<Value, String> {
    let host = host_of(url);
    let secs = format!("{} s", crate::jsnum::math_round(timeout_ms / 1000.0));
    let j = match got {
        Ok(j) => j,
        Err(Failure::Timeout) | Err(Failure::Cancelled(_)) => {
            return Err(format!(
                "{label}: {host} did not answer within {secs}. Try again later."
            ));
        }
        Err(Failure::Unreachable(m)) => {
            return Err(format!(
                "{label}: {host} could not be reached ({m}). Check your connection and try again."
            ));
        }
        Err(Failure::Http(s)) if (400..500).contains(&s) => {
            return Err(format!("{label}: {host} answered HTTP {s}: {REJECTED}."));
        }
        Err(Failure::Http(s)) => {
            return Err(format!(
                "{label}: {host} answered HTTP {s}. Try again later."
            ));
        }
        Err(Failure::ReadTimeout) => {
            return Err(format!(
                "{label}: {host} did not finish answering within {secs}. Try again later."
            ));
        }
        Err(Failure::Unreadable(_)) => {
            return Err(format!(
                "{label}: {host} sent an answer Spare Construct could not read. Try again later."
            ));
        }
    };
    if let Some(e) = j.get("error").filter(|e| value_truthy(e)) {
        let code = e.get("code").map_or(f64::NAN, value_to_number);
        let msg = match e.get("message") {
            Some(m) if value_truthy(m) => value_to_js_string(m),
            _ => serde_json::to_string(e).unwrap_or_default(),
        };
        return Err(if code >= 400.0 && code < 500.0 {
            format!("{label}: {msg}: {REJECTED}.")
        } else {
            format!("{label}: {msg}")
        });
    }
    Ok(j)
}

/// `getJSON` through the host: the request is made and its answer checked.
pub fn get_json(net: &mut dyn Net, url: &str, label: &str) -> Result<Value, String> {
    checked_answer(url, label, TIMEOUT_MS, net.get_json(url))
}

/// The address of one page of `arcQuery`: every feature of a layer inside the box, from this offset.
pub fn arc_query_url(
    url: &str,
    b: &Bx,
    out_fields: &[String],
    geometry: bool,
    offset: usize,
) -> String {
    let geom = [b.w, b.s, b.e, b.n]
        .iter()
        .map(|v| to_fixed(*v, 6))
        .collect::<Vec<_>>()
        .join(",");
    let q = query_string(&[
        ("where", "1=1".to_string()),
        ("geometry", geom),
        ("geometryType", "esriGeometryEnvelope".to_string()),
        ("inSR", "4326".to_string()),
        ("spatialRel", "esriSpatialRelIntersects".to_string()),
        ("outFields", out_fields.join(",")),
        ("returnGeometry", geometry.to_string()),
        ("outSR", "4326".to_string()),
        ("geometryPrecision", "6".to_string()),
        ("f", "json".to_string()),
        ("resultOffset", offset.to_string()),
    ]);
    format!("{url}/query?{q}")
}

/// The address of a layer's description (its fields): `url?f=json`.
pub fn arc_layer_url(url: &str) -> String {
    format!("{url}?f=json")
}

/// `arcQuery(url, B, outFields, label, geometry)`: all the features, a page at a time (at most 30 pages).
pub fn arc_query(
    net: &mut dyn Net,
    url: &str,
    b: &Bx,
    out_fields: &[String],
    label: &str,
    geometry: bool,
) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    for _ in 0..30 {
        let j = get_json(
            net,
            &arc_query_url(url, b, out_fields, geometry, offset),
            label,
        )?;
        if j.is_null() {
            return Err("Cannot read properties of null (reading 'features')".to_string());
        }
        let f = j
            .get("features")
            .filter(|x| value_truthy(x))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let more = j.get("exceededTransferLimit").is_some_and(value_truthy);
        let n = f.len();
        out.extend(f);
        if !more || n == 0 {
            break;
        }
        offset += n;
    }
    Ok(out)
}

/// `arcLayer(url, B, pick, label, geometry)`: the layer's description, then its features with only the fields `pick` asks for.
pub fn arc_layer(
    net: &mut dyn Net,
    url: &str,
    b: &Bx,
    pick: &dyn Fn(&[Value]) -> Vec<String>,
    label: &str,
    geometry: bool,
) -> Result<Layer, String> {
    let meta = get_json(net, &arc_layer_url(url), label)?;
    if meta.is_null() {
        return Err("Cannot read properties of null (reading 'fields')".to_string());
    }
    let fields = meta
        .get("fields")
        .filter(|x| value_truthy(x))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let want = pick(&fields);
    let features = arc_query(net, url, b, &want, label, geometry)?;
    Ok(Layer { fields, features })
}

/// What a download is called while it runs ("Downloading Census 2023 dwellings for each SA1\u{2026}") for each Census layer, in the order they are fetched.
pub const CENSUS_FETCH: [(&str, &str); 4] = [
    ("dw", "dwellings"),
    ("hh", "households"),
    ("ind0", "population"),
    ("ind2", "travel to work and education"),
];

/// `fetchCensus(bounds)` without the cache: each layer fails soft on its own. The SA1 outlines come with the first of dwellings or
/// households that loads; with neither, the whole download fails with the layers' messages.
pub fn fetch_census(
    net: &mut dyn Net,
    b: &Bx,
    status: &mut dyn FnMut(&str),
) -> Result<CensusData, String> {
    let mut layers = CensusLayers::default();
    let mut errors: Vec<String> = Vec::new();
    let mut geo = false;
    for (k, label) in CENSUS_FETCH {
        status(&format!(
            "Downloading Census 2023 {label} for each SA1\u{2026}"
        ));
        let want_geo = !geo && (k == "dw" || k == "hh");
        let pick = move |fl: &[Value]| census_fields(k, fl);
        match arc_layer(
            net,
            &census_layer(k),
            b,
            &pick,
            &format!("Census 2023 {label}"),
            want_geo,
        ) {
            Ok(l) => {
                if want_geo {
                    geo = true;
                }
                match k {
                    "dw" => layers.dw = Some(l),
                    "hh" => layers.hh = Some(l),
                    "ind0" => layers.ind0 = Some(l),
                    _ => layers.ind2 = Some(l),
                }
            }
            Err(e) => errors.push(e),
        }
    }
    if !geo {
        return Err(errors.join(" "));
    }
    let mut c = parse_census(&layers);
    c.partial = !errors.is_empty();
    c.errors = errors;
    Ok(c)
}

/// `fetchEmployees(bounds)` without the cache.
pub fn fetch_employees(
    net: &mut dyn Net,
    b: &Bx,
    status: &mut dyn FnMut(&str),
) -> Result<EmployeeData, String> {
    status("Downloading Stats NZ employee counts for each SA2\u{2026}");
    let pick = |fl: &[Value]| -> Vec<String> {
        let cf = sa2_code_field(fl).unwrap_or_else(|| "SA22023_V1_00".to_string());
        let named = |n: &str| {
            fl.iter()
                .any(|f| f.get("name").and_then(Value::as_str) == Some(n))
        };
        let ec: Vec<String> = fl
            .iter()
            .filter_map(|f| f.get("name").and_then(Value::as_str))
            .filter(|n| {
                (n.len() == 6 && n.starts_with("ec") && n[2..].bytes().all(|c| c.is_ascii_digit()))
                    || *n == "ec_avperinc"
            })
            .map(str::to_string)
            .collect();
        let mut list = vec![cf.clone(), format!("{cf}_NAME")];
        if ec.is_empty() {
            list.extend(["ec2025".to_string(), "ec_avperinc".to_string()]);
        } else {
            list.extend(ec);
        }
        let mut out: Vec<String> = Vec::new();
        for x in list {
            if !out.contains(&x) && (named(&x) || fl.is_empty()) {
                out.push(x);
            }
        }
        out
    };
    let l = arc_layer(
        net,
        consts::EMPLOYEES.url,
        b,
        &pick,
        "Stats NZ employees",
        true,
    )?;
    Ok(parse_employees(&l))
}

/// The address of the Ministry of Education directory search for a box (`kind` is "schools" or "ece").
pub fn moe_url(kind: &str, b: &Bx) -> String {
    let s = if kind == "ece" {
        &consts::ECE
    } else {
        &consts::SCHOOLS
    };
    let fields = moe_fields(kind)
        .iter()
        .map(|f| format!("\"{f}\""))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT {fields} FROM \"{}\" WHERE \"Latitude\" BETWEEN {} AND {} AND \"Longitude\" BETWEEN {} AND {}",
        s.resource,
        to_fixed(b.s, 6),
        to_fixed(b.n, 6),
        to_fixed(b.w, 6),
        to_fixed(b.e, 6)
    );
    format!("{CKAN}?sql={}", encode_uri_component(&sql))
}

/// `fetchMoE(kind, bounds)` without the cache.
pub fn fetch_moe(
    net: &mut dyn Net,
    kind: &str,
    b: &Bx,
    status: &mut dyn FnMut(&str),
) -> Result<Vec<Moe>, String> {
    status(&format!(
        "Downloading the Ministry of Education {} directory\u{2026}",
        if kind == "ece" {
            "early learning"
        } else {
            "school"
        }
    ));
    let j = get_json(net, &moe_url(kind, b), "Ministry of Education directory")?;
    let fail = || {
        "Ministry of Education directory: the search failed. Its fields may have changed; try again later.".to_string()
    };
    if j.is_null() {
        return Err("Cannot read properties of null (reading 'success')".to_string());
    }
    if !j.get("success").is_some_and(value_truthy) || !j.get("result").is_some_and(value_truthy) {
        return Err(fail());
    }
    let recs = j["result"]
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(parse_moe(&recs, kind))
}

/// `fetchAUP(bounds)` without the cache.
pub fn fetch_aup(
    net: &mut dyn Net,
    b: &Bx,
    status: &mut dyn FnMut(&str),
) -> Result<AupData, String> {
    status("Downloading Auckland Unitary Plan zones\u{2026}");
    let pick = |fl: &[Value]| -> Vec<String> {
        ["ZONE", "GROUPZONE", "VERSIONSTATUS"]
            .iter()
            .filter(|n| {
                fl.iter()
                    .any(|f| f.get("name").and_then(Value::as_str) == Some(**n))
                    || fl.is_empty()
            })
            .map(|n| n.to_string())
            .collect()
    };
    let l = arc_layer(
        net,
        consts::AUP.url,
        b,
        &pick,
        "Auckland Unitary Plan zones",
        true,
    )?;
    Ok(parse_aup(&l))
}

/// The downloads already made this session, by study area (`cache`): a failure or a partial answer is not kept, so a retry asks again.
#[derive(Clone, Debug, Default)]
pub struct Downloads {
    /// Census by study area.
    pub census: HashMap<String, CensusData>,
    /// Employees by study area.
    pub employees: HashMap<String, EmployeeData>,
    /// Ministry of Education directories by kind and study area.
    pub moe: HashMap<String, Vec<Moe>>,
    /// Unitary Plan zones by study area.
    pub aup: HashMap<String, AupData>,
    /// OpenStreetMap elements by study area.
    pub osm: HashMap<String, Vec<Value>>,
    /// Index of the last Overpass server that answered (the next request starts there).
    pub good: usize,
}

impl Downloads {
    /// `clearCache()`.
    pub fn clear(&mut self) {
        *self = Downloads::default();
    }
    /// `fetchCensus(bounds, onStatus)`: from the cache, else downloaded (and kept unless partial).
    pub fn census(
        &mut self,
        net: &mut dyn Net,
        b: &Bx,
        status: &mut dyn FnMut(&str),
    ) -> Result<CensusData, String> {
        let k = box_key(b);
        if let Some(c) = self.census.get(&k) {
            return Ok(c.clone());
        }
        let c = fetch_census(net, b, status)?;
        if !c.partial {
            self.census.insert(k, c.clone());
        }
        Ok(c)
    }
    /// `fetchEmployees(bounds, onStatus)`.
    pub fn employees(
        &mut self,
        net: &mut dyn Net,
        b: &Bx,
        status: &mut dyn FnMut(&str),
    ) -> Result<EmployeeData, String> {
        let k = box_key(b);
        if let Some(c) = self.employees.get(&k) {
            return Ok(c.clone());
        }
        let c = fetch_employees(net, b, status)?;
        self.employees.insert(k, c.clone());
        Ok(c)
    }
    /// `fetchMoE(kind, bounds, onStatus)`.
    pub fn moe(
        &mut self,
        net: &mut dyn Net,
        kind: &str,
        b: &Bx,
        status: &mut dyn FnMut(&str),
    ) -> Result<Vec<Moe>, String> {
        let k = format!("{kind}|{}", box_key(b));
        if let Some(c) = self.moe.get(&k) {
            return Ok(c.clone());
        }
        let c = fetch_moe(net, kind, b, status)?;
        self.moe.insert(k, c.clone());
        Ok(c)
    }
    /// `cached('osm', B, () => ExistingLU.fetch(B, onStatus))`: the buildings and land-use areas of the study area, from the cache or downloaded.
    pub fn existing_land_use(
        &mut self,
        net: &mut dyn Net,
        b: &Bx,
        status: &mut dyn FnMut(&str),
    ) -> Result<Vec<Value>, String> {
        let k = box_key(b);
        if let Some(c) = self.osm.get(&k) {
            return Ok(c.clone());
        }
        let c = existing_land_use_download(net, &mut self.good, b, status)?;
        self.osm.insert(k, c.clone());
        Ok(c)
    }
    /// `fetchAUP(bounds, onStatus)`.
    pub fn aup(
        &mut self,
        net: &mut dyn Net,
        b: &Bx,
        status: &mut dyn FnMut(&str),
    ) -> Result<AupData, String> {
        let k = box_key(b);
        if let Some(c) = self.aup.get(&k) {
            return Ok(c.clone());
        }
        let c = fetch_aup(net, b, status)?;
        self.aup.insert(k, c.clone());
        Ok(c)
    }
}

/// `ExistingLU.fetch(bounds, onStatus)`: downloads the buildings, land-use areas, schools and shops of a study area, a tile at a time, trying
/// the Overpass servers in turn starting with the one that answered last (`good`). The features are merged (one that came back in two tiles is kept once).
pub fn existing_land_use_download(
    net: &mut dyn Net,
    good: &mut usize,
    b: &Bx,
    status: &mut dyn FnMut(&str),
) -> Result<Vec<Value>, String> {
    let sbox = StudyBox {
        s: b.s,
        w: b.w,
        n: b.n,
        e: b.e,
    };
    let jobs = landuse::download_jobs(&sbox)?;
    let mut batches: Vec<Vec<Value>> = Vec::new();
    for job in &jobs {
        let mut last_err: Option<String> = None;
        let mut got: Option<Vec<Value>> = None;
        for k in 0..OVERPASS.len() {
            let i = (*good + k) % OVERPASS.len();
            let url = OVERPASS[i];
            let host = host_of(url);
            status(&format!("{} from {host}\u{2026}", job.label));
            let body = format!("data={}", encode_uri_component(&job.query));
            match net.post_form(url, &body) {
                Err(f) => last_err = Some(f.message(url)),
                Ok(j) if j.is_null() => {
                    last_err = Some("Cannot read properties of null (reading 'remark')".to_string())
                }
                Ok(j) => match landuse::overpass_elements(&j, &host) {
                    Err(e) => last_err = Some(e),
                    Ok(els) => {
                        *good = i;
                        got = Some(els);
                        break;
                    }
                },
            }
        }
        match got {
            Some(els) => batches.push(els),
            None => {
                return Err(format!(
                    "Couldn't download existing land use: {}. Try again in a minute or use a smaller area.",
                    last_err.unwrap_or_else(|| "no OpenStreetMap server answered".to_string())
                ));
            }
        }
    }
    let merged = landuse::merge_elements(&batches);
    status(&format!(
        "Downloaded {} OpenStreetMap features",
        fmt::int(merged.len() as f64)
    ));
    Ok(merged)
}
