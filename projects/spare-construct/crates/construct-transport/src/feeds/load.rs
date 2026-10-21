//! The data hub's loaders: what the downloaded answers of NZTA, Auckland Transport and the crash service are turned into (`loadNZTA`, `loadAT`, `loadCAS`, `loadFutureConnect`, `loadControlled`), and the load of several sources into a project (`loadAll`).
//!
//! The answers come in as JSON the host fetched; [`LoadAll`] says which requests to send and what to do with each answer.

use super::arcgis::{ArcgisPager, Bounds, FetchFailure, HttpRequest, LoadError, ARCGIS_TIMEOUT_MS};
use super::nztime::{date_of, month_year, utc_date};
use crate::datahub::{dir_of, ensure_data, source, Count};
use crate::edit::json::JsonValue;
use crate::jsnum::{json_number, number_to_js_string};
use crate::project::Project;
use crate::safety::{cas_tuple, cas_window, conflate_fc, title, CasWindow, FutureConnect, CAS_FIELDS, CAS_LONG_YEARS, CAS_OUT};
use crate::util::{fmt, r7};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// `NZTA_SITES`: the NZTA state highway traffic monitoring sites layer.
pub const NZTA_SITES: &str = "https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services/Assets_SHTrafficMonitoringSites/FeatureServer/0";
/// `NZTA_CAS`: the NZTA crash analysis system layer.
pub const NZTA_CAS: &str = "https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services/CAS_Data_Public/FeatureServer/0";
/// `AT_COUNTS`: the Auckland Transport traffic counts layer.
pub const AT_COUNTS: &str = "https://services2.arcgis.com/JkPEgZJGxhSjYOo0/arcgis/rest/services/TrafficService/FeatureServer/0";
/// `AT_FC`: the Auckland Transport Future Connect integrated network layer.
pub const AT_FC: &str = "https://services2.arcgis.com/JkPEgZJGxhSjYOo0/arcgis/rest/services/FutureConnect_IntegratedNetwork/FeatureServer/0";
/// `AT_CI`: the Auckland Transport controlled intersections layer (RoadingService layer 2).
pub const AT_CI: &str = "https://services2.arcgis.com/JkPEgZJGxhSjYOo0/arcgis/rest/services/RoadingService/FeatureServer/2";
/// `AT_LICENCE`: the licence text kept with Auckland Transport data.
pub const AT_LICENCE: &str = "Auckland Transport open data, CC BY 4.0 as stated on the AT open data portal (confirm before publishing)";
/// `FC_FIELDS`: the fields asked of the Future Connect layer.
pub const FC_FIELDS: &str = "OBJECTID,street_name,generaltraffic_current,publictransport_current,freight_current,cycle_current,generaltraffic_first_decade,publictransport_first_decade,freight_first_decade,cycle_first_decade";
/// The fields asked of the NZTA sites layer.
pub const NZTA_FIELDS: &str = "OBJECTID,siteref,description,lane,sh,sitetype,type,percentheavy,aadt1yearago,aadt2yearsago,aadt3yearsago,aadt4yearsago,aadt5yearsago,accepteddays";
/// The fields asked of the Auckland Transport counts layer.
pub const AT_COUNT_FIELDS: &str = "OBJECTID,road_name,start_name,end_name,count_date,peak_hour,count_duration,adt,peaktraffic,pcheavy,carr_way_no";
/// The where clause of the Auckland Transport counts layer: only the latest count of each site.
pub const AT_COUNT_WHERE: &str = "latest='Yes'";
/// The fields asked of the controlled intersections layer.
pub const CI_FIELDS: &str = "OBJECTID,intersection_id,intersection_no,intersection_desc,intersection_type,controlled";

fn st(x: &str) -> JsonValue {
    JsonValue::String(x.to_string())
}

fn get<'a>(o: &'a JsonValue, k: &str) -> Option<&'a JsonValue> {
    o.get(k)
}

fn truthy(v: Option<&JsonValue>) -> bool {
    v.is_some_and(JsonValue::truthy)
}

// String(x): "undefined" for a missing value
fn js_str(v: Option<&JsonValue>) -> String {
    v.map_or_else(|| "undefined".to_string(), JsonValue::js_string)
}

// String(x || d)
fn or_str(v: Option<&JsonValue>, d: &str) -> String {
    match v {
        Some(v) if v.truthy() => v.js_string(),
        _ => d.to_string(),
    }
}

// obj[k] = v, left out when v is undefined (a missing key)
fn put(o: &mut JsonValue, k: &str, v: Option<&JsonValue>) {
    if let Some(v) = v {
        o.set(k, v.clone());
    }
}

fn num(v: Option<&JsonValue>) -> f64 {
    v.map_or(f64::NAN, JsonValue::to_number)
}

fn geometry_xy(x: &JsonValue) -> (f64, f64) {
    let g = x.get("geometry");
    (r7(num(g.and_then(|g| g.get("y")))), r7(num(g.and_then(|g| g.get("x")))))
}

fn count_of(o: &JsonValue) -> Count {
    serde_json::from_value::<Count>(o.to_value()).expect("a count reads")
}

// /\bramp\b/i.test(text)
fn has_word_ramp(text: &str) -> bool {
    let cs: Vec<char> = text.to_lowercase().chars().collect();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let pat: Vec<char> = "ramp".chars().collect();
    if cs.len() < 4 {
        return false;
    }
    (0..=cs.len() - 4).any(|i| cs[i..i + 4] == pat[..] && (i == 0 || !word(cs[i - 1])) && (i + 4 == cs.len() || !word(cs[i + 4])))
}

/// `loadNZTA(b)` after the download: state highway count sites from the layer's features, as the project holds them.
///
/// Each site with a position and a positive daily volume (last year's, else the year before) becomes a directional count
/// with its five years of history; an error is the text the prototype's error would carry.
pub fn parse_nzta(features: &[JsonValue]) -> Result<Vec<Count>, String> {
    let mut out = Vec::new();
    for x in features.iter().filter(|x| truthy(x.get("geometry"))) {
        let Some(a) = x.get("attributes").filter(|a| !a.is_null()) else {
            return Err("Cannot read properties of undefined (reading 'aadt5yearsago')".to_string());
        };
        let hist = JsonValue::Array(
            ["aadt5yearsago", "aadt4yearsago", "aadt3yearsago", "aadt2yearsago", "aadt1yearago"]
                .iter()
                .map(|k| get(a, k).cloned().unwrap_or(JsonValue::Null))
                .collect(),
        );
        let (lat, lng) = geometry_xy(x);
        let sh = get(a, "sh");
        let description = get(a, "description");
        let sh_text = match sh {
            Some(v) if v.truthy() => match v {
                JsonValue::String(s) => s.clone(),
                _ => return Err("(a.sh || \"\").replace is not a function".to_string()),
            },
            _ => String::new(),
        };
        // 'SH' + sh with a trailing N or S taken off
        let road = {
            let mut t = sh_text.clone();
            if t.ends_with('N') || t.ends_with('S') {
                t.pop();
            }
            format!("SH{t}")
        };
        let virtual_site = get(a, "sitetype").is_some_and(|v| v.strict_eq(&st("Virtual")))
            || get(a, "type").is_some_and(|v| v.strict_eq(&st("Virtual")));
        let sitetype_virtual = get(a, "sitetype").is_some_and(|v| v.strict_eq(&st("Virtual")));
        let adt = [get(a, "aadt1yearago"), get(a, "aadt2yearsago")].into_iter().flatten().find(|v| v.truthy()).cloned().unwrap_or(JsonValue::Null);
        if adt.to_number() <= 0.0 || adt.to_number().is_nan() {
            continue;
        }
        let mut o = JsonValue::object();
        o.set("id", JsonValue::String(format!("nzta:{}", js_str(get(a, "siteref")))));
        o.set("src", st("nzta"));
        put(&mut o, "ref", get(a, "siteref"));
        o.set(
            "name",
            if truthy(description) { description.cloned().unwrap_or(JsonValue::Null) } else { JsonValue::String(format!("SH{} site {}", js_str(sh), js_str(get(a, "siteref")))) },
        );
        o.set("road", JsonValue::String(road));
        o.set("lat", JsonValue::Number(lat));
        o.set("lng", JsonValue::Number(lng));
        o.set("adt", adt);
        put(&mut o, "hv", get(a, "percentheavy"));
        let desc_text = or_str(description, "");
        o.set("dir", dir_of(&desc_text.to_uppercase()).map_or(JsonValue::Null, |d| JsonValue::String(d.to_string())));
        o.set("directional", JsonValue::Bool(true));
        o.set("virtual", JsonValue::Bool(virtual_site));
        o.set("ramp", JsonValue::Bool(has_word_ramp(&desc_text)));
        o.set("hist", hist);
        o.set(
            "note",
            JsonValue::String(if sitetype_virtual {
                "NZTA estimate (virtual site), not a physical count".to_string()
            } else {
                format!(
                    "{} site{}",
                    or_str(get(a, "sitetype"), "Count"),
                    if truthy(get(a, "accepteddays")) { format!(", {} counted days last year", js_str(get(a, "accepteddays"))) } else { String::new() }
                )
            }),
        );
        out.push(count_of(&o));
    }
    Ok(out)
}

// /^SH\s*\d/i.test(text)
fn is_state_highway_name(text: &str) -> bool {
    let cs: Vec<char> = text.chars().collect();
    if cs.len() < 3 || !(cs[0] == 'S' || cs[0] == 's') || !(cs[1] == 'H' || cs[1] == 'h') {
        return false;
    }
    let mut i = 2;
    while i < cs.len() && crate::jsnum::is_js_space(cs[i]) {
        i += 1;
    }
    i < cs.len() && cs[i].is_ascii_digit()
}

// padStart(4, '0')
fn pad_start4(s: &str) -> String {
    let n = s.encode_utf16().count();
    if n >= 4 { s.to_string() } else { format!("{}{}", "0".repeat(4 - n), s) }
}

fn slice16(s: &str, from: usize, to: Option<usize>) -> String {
    let u: Vec<u16> = s.encode_utf16().collect();
    let to = to.unwrap_or(u.len()).min(u.len());
    if from >= to { String::new() } else { String::from_utf16_lossy(&u[from..to]) }
}

/// `loadAT(b)` after the download: Auckland Transport count sites from the layer's features (state highways left out, NZTA's own counts cover them).
///
/// Sites with a position and a positive ADT become two-way counts with the counted peak hour. An error is the text the prototype's error would carry.
pub fn parse_at(features: &[JsonValue]) -> Result<Vec<Count>, String> {
    let mut out = Vec::new();
    for x in features.iter().filter(|x| truthy(x.get("geometry"))) {
        let Some(a) = x.get("attributes").filter(|a| !a.is_null()) else {
            return Err("Cannot read properties of undefined (reading 'road_name')".to_string());
        };
        if is_state_highway_name(&or_str(get(a, "road_name"), "")) {
            continue;
        }
        let (lat, lng) = geometry_xy(x);
        let ph = pad_start4(&or_str(get(a, "peak_hour"), ""));
        let date = match get(a, "count_date") {
            Some(d) if d.truthy() => Some(date_of(d).ok_or_else(|| "Invalid time value".to_string())?),
            _ => None,
        };
        let adt = get(a, "adt");
        // .filter(c => c.adt > 0)
        if !adt.is_some_and(|v| v.to_number() > 0.0) {
            continue;
        }
        let mut o = JsonValue::object();
        o.set("id", JsonValue::String(format!("at:{}", js_str(get(a, "OBJECTID")))));
        o.set("src", st("at"));
        o.set("ref", JsonValue::String(match get(a, "carr_way_no") {
            Some(v) if v.truthy() => v.js_string(),
            _ => js_str(get(a, "OBJECTID")),
        }));
        o.set(
            "name",
            JsonValue::String(format!(
                "{} \u{b7} {} to {}",
                title(&or_str(get(a, "road_name"), "")),
                title(&or_str(get(a, "start_name"), "")),
                title(&or_str(get(a, "end_name"), ""))
            )),
        );
        put(&mut o, "road", get(a, "road_name"));
        o.set("lat", JsonValue::Number(lat));
        o.set("lng", JsonValue::Number(lng));
        put(&mut o, "adt", adt);
        put(&mut o, "peak", get(a, "peaktraffic"));
        o.set("peakTime", JsonValue::String(format!("{}:{}", slice16(&ph, 0, Some(2)), slice16(&ph, 2, None))));
        put(&mut o, "hv", get(a, "pcheavy"));
        o.set("date", match date {
            Some(ms) => JsonValue::String(utc_date(ms)),
            None => JsonValue::Null,
        });
        put(&mut o, "dur", get(a, "count_duration"));
        o.set("directional", JsonValue::Bool(false));
        o.set(
            "note",
            JsonValue::String(format!(
                "{} count{}",
                or_str(get(a, "count_duration"), ""),
                match date {
                    Some(ms) => format!(", {}", month_year(ms)),
                    None => String::new(),
                }
            )),
        );
        out.push(count_of(&o));
    }
    Ok(out)
}

/// `loadCAS(b)` after the download: the crash tuples of the features that have a position.
pub fn parse_cas(features: &[JsonValue]) -> Vec<Vec<Value>> {
    features.iter().filter(|x| truthy(x.get("geometry"))).map(cas_tuple).collect()
}

/// `loadFutureConnect(b)` after the download: Future Connect sections that have polylines, with their classes now and in the first decade.
pub fn parse_future_connect(features: &[JsonValue]) -> Vec<FutureConnect> {
    let empty = JsonValue::object();
    features
        .iter()
        .filter(|x| matches!(x.get("geometry").and_then(|g| g.get("paths")), Some(JsonValue::Array(_))))
        .map(|x| {
            let a = x.get("attributes").filter(|a| a.truthy()).unwrap_or(&empty);
            let cls = |k: &str| match get(a, k) {
                Some(v) if v.truthy() => Some(v.js_string()),
                _ => None,
            };
            let paths = match x.get("geometry").and_then(|g| g.get("paths")) {
                Some(JsonValue::Array(ps)) => ps
                    .iter()
                    .map(|p| match p {
                        JsonValue::Array(pts) => pts
                            .iter()
                            .map(|q| match q {
                                JsonValue::Array(c) => [r7(num(c.get(1))), r7(num(c.first()))],
                                _ => [f64::NAN, f64::NAN],
                            })
                            .collect(),
                        _ => Vec::new(),
                    })
                    .collect(),
                _ => Vec::new(),
            };
            FutureConnect {
                id: get(a, "OBJECTID").map_or(Value::Null, JsonValue::to_value),
                name: or_str(get(a, "street_name"), ""),
                gt: cls("generaltraffic_current"),
                pt: cls("publictransport_current"),
                freight: cls("freight_current"),
                cycle: cls("cycle_current"),
                gt10: cls("generaltraffic_first_decade"),
                pt10: cls("publictransport_first_decade"),
                freight10: cls("freight_first_decade"),
                cycle10: cls("cycle_first_decade"),
                paths,
            }
        })
        .collect()
}

/// One AT controlled intersection (`loadControlled` entry).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlledSite {
    /// The intersection id (its OBJECTID when it has none).
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub id: Value,
    /// AT's site number, or null.
    pub no: Value,
    /// Description without the leading "(number) ".
    pub name: String,
    /// Intersection type.
    #[serde(rename = "type")]
    pub kind: String,
    /// Traffic signals are in control.
    pub controlled: bool,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lng: f64,
}

// String.replace(/^\(\d+\)\s*/, '')
pub(crate) fn strip_site_prefix(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    if cs.first() == Some(&'(') {
        let mut i = 1;
        while i < cs.len() && cs[i].is_ascii_digit() {
            i += 1;
        }
        if i > 1 && i < cs.len() && cs[i] == ')' {
            i += 1;
            while i < cs.len() && crate::jsnum::is_js_space(cs[i]) {
                i += 1;
            }
            return cs[i..].iter().collect();
        }
    }
    s.to_string()
}

/// `loadControlled(b)` after the download: the controlled intersections that have a position.
pub fn parse_controlled(features: &[JsonValue]) -> Vec<ControlledSite> {
    let empty = JsonValue::object();
    features
        .iter()
        .filter(|x| truthy(x.get("geometry")))
        .map(|x| {
            let a = x.get("attributes").filter(|a| a.truthy()).unwrap_or(&empty);
            let (lat, lng) = geometry_xy(x);
            // a ?? b: the first that is neither null nor undefined
            let nn = |v: Option<&JsonValue>| v.filter(|v| !v.is_null()).map_or(Value::Null, JsonValue::to_value);
            let id = match get(a, "intersection_id") {
                Some(v) if !v.is_null() => v.to_value(),
                _ => nn(get(a, "OBJECTID")),
            };
            ControlledSite {
                id,
                no: nn(get(a, "intersection_no")),
                name: strip_site_prefix(&or_str(get(a, "intersection_desc"), "")),
                kind: or_str(get(a, "intersection_type"), ""),
                controlled: get(a, "controlled").is_some_and(|v| v.strict_eq(&st("Yes"))),
                lat,
                lng,
            }
        })
        .collect()
}

/// What `loadAll` takes besides the area and the sources.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadOptions {
    /// Full years of crashes, 5 (default) or 10.
    pub cas_years: Option<f64>,
    /// The time limit of each request in ms (30 s when unset or not positive).
    pub timeout_ms: Option<f64>,
}

struct Current {
    w: String,
    pager: ArcgisPager,
    cw: Option<CasWindow>,
}

/// `DataHub.loadAll(b, which, status, opts)`: loads several sources into a project, a request at a time.
///
/// Sources are "nzta" and "at" (count sites), "cas" (five years of crashes), "cas10" (ten years for state highway
/// accesses), "fc" (Future Connect road classes, put on the active scenario's links) and "controlled" (AT controlled
/// intersections). The caller sends each [`next_request`](LoadAll::next_request) and gives the answer to
/// [`answer`](LoadAll::answer); a source that fails is reported in the messages and the next one goes on, a cancel stops everything.
pub struct LoadAll {
    bounds: Bounds,
    which: Vec<String>,
    idx: usize,
    timeout_ms: f64,
    cas_years: Option<f64>,
    now_ms: f64,
    cancelled: bool,
    started: bool,
    current: Option<Current>,
    msgs: Vec<String>,
    statuses: Vec<String>,
}

impl LoadAll {
    /// A load of these sources (the prototype's default is nzta, at and cas) for an area, at a time (ms since 1970).
    pub fn new(bounds: Bounds, which: &[&str], opts: &LoadOptions, now_ms: f64) -> LoadAll {
        LoadAll {
            bounds,
            which: which.iter().map(|s| s.to_string()).collect(),
            idx: 0,
            timeout_ms: match opts.timeout_ms {
                Some(t) if t > 0.0 => t,
                _ => ARCGIS_TIMEOUT_MS,
            },
            cas_years: opts.cas_years,
            now_ms,
            cancelled: false,
            started: false,
            current: None,
            msgs: Vec::new(),
            statuses: Vec::new(),
        }
    }
    /// The user cancelled: the next call to [`next_request`](LoadAll::next_request) stops the load (a cancel while a request waits is reported by the host as [`FetchFailure::Cancelled`]).
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
    /// What the load reports so far, one line per source.
    pub fn messages(&self) -> &[String] {
        &self.msgs
    }
    /// The progress lines the prototype passes to its `status` callback, in order.
    pub fn statuses(&self) -> &[String] {
        &self.statuses
    }
    /// The next request to send, or None when every source is done. The first call makes sure the project has its data store (`D()`).
    pub fn next_request(&mut self, p: &mut Project) -> Result<Option<HttpRequest>, LoadError> {
        if !self.started {
            self.started = true;
            ensure_data(p);
        }
        loop {
            if let Some(cur) = &self.current {
                if let Some(r) = cur.pager.next_request() {
                    return Ok(Some(r));
                }
                self.current = None;
            }
            if self.cancelled {
                return Err(LoadError::Cancelled);
            }
            if self.idx >= self.which.len() {
                return Ok(None);
            }
            let w = self.which[self.idx].clone();
            self.idx += 1;
            let b = self.bounds;
            let t = self.timeout_ms;
            let (pager, cw) = match w.as_str() {
                "cas" => {
                    self.statuses.push("Downloading crash records (NZTA CAS)\u{2026}".to_string());
                    let cw = cas_window(self.now_ms, self.cas_years);
                    (ArcgisPager::new(NZTA_CAS, &b, &cas_window(self.now_ms, Some(cw.years)).where_, CAS_OUT, t), Some(cw))
                }
                "cas10" => {
                    self.statuses.push("Downloading 10 years of crash records for state highway accesses (NZTA CAS)\u{2026}".to_string());
                    let cw = cas_window(self.now_ms, Some(CAS_LONG_YEARS));
                    (ArcgisPager::new(NZTA_CAS, &b, &cas_window(self.now_ms, Some(cw.years)).where_, CAS_OUT, t), Some(cw))
                }
                "fc" => {
                    self.statuses.push("Downloading AT Future Connect road classes\u{2026}".to_string());
                    (ArcgisPager::new(AT_FC, &b, "1=1", FC_FIELDS, t), None)
                }
                "controlled" => {
                    self.statuses.push("Downloading AT controlled intersections\u{2026}".to_string());
                    (ArcgisPager::new(AT_CI, &b, "1=1", CI_FIELDS, t), None)
                }
                "nzta" | "at" => {
                    let label = source(&w).map_or("", |s| s.label);
                    self.statuses.push(format!("Downloading {label}\u{2026}"));
                    if w == "nzta" {
                        (ArcgisPager::new(NZTA_SITES, &b, "1=1", NZTA_FIELDS, t), None)
                    } else {
                        (ArcgisPager::new(AT_COUNTS, &b, AT_COUNT_WHERE, AT_COUNT_FIELDS, t), None)
                    }
                }
                _ => {
                    self.msgs.push(format!("{} is not a source Spare Construct downloads here", w.to_uppercase()));
                    continue;
                }
            };
            self.current = Some(Current { w, pager, cw });
        }
    }
    /// Takes the answer to the last request. A failed source is added to the messages and the load goes on with the next; only a cancel is an error.
    pub fn answer(&mut self, p: &mut Project, a: Result<JsonValue, FetchFailure>) -> Result<(), LoadError> {
        let Some(cur) = self.current.as_mut() else { return Ok(()) };
        match cur.pager.answer(a) {
            Err(LoadError::Cancelled) => {
                self.current = None;
                return Err(LoadError::Cancelled);
            }
            Err(LoadError::Failed(m)) => {
                self.msgs.push(format!("{} failed ({})", cur.w.to_uppercase(), m));
                self.current = None;
                return Ok(());
            }
            Ok(()) => {}
        }
        if !cur.pager.is_done() {
            return Ok(());
        }
        let cur = self.current.take().expect("current source");
        let features = cur.pager.into_features();
        match self.finish_source(p, &cur.w, cur.cw.as_ref(), &features) {
            Ok(m) => self.msgs.push(m),
            Err(e) => self.msgs.push(format!("{} failed ({})", cur.w.to_uppercase(), e)),
        }
        Ok(())
    }

    fn finish_source(&self, p: &mut Project, w: &str, cw: Option<&CasWindow>, features: &[JsonValue]) -> Result<String, String> {
        ensure_data(p);
        let now = json_number(self.now_ms);
        let b = self.bounds;
        match w {
            "cas" | "cas10" => {
                let cw = cw.expect("a crash window");
                let tuples = parse_cas(features);
                let n = tuples.len();
                let arr = Value::Array(tuples.iter().map(|t| Value::Array(t.clone())).collect());
                let d = p.data.get_mut().expect("data");
                let lag = cw.lag_year.map_or(Value::Null, json_number);
                if w == "cas" {
                    let n_ytd = tuples.iter().filter(|c| c.get(3).and_then(Value::as_f64) == Some(cw.ytd)).count();
                    d.extra.insert("crashes".to_string(), arr);
                    d.loaded.get_mut().expect("loaded").insert(
                        "cas".to_string(),
                        json!({ "at": now, "n": n, "from": json_number(cw.from), "to": json_number(cw.to), "ytd": json_number(cw.ytd), "years": json_number(cw.years), "nYTD": n_ytd, "lagYear": lag, "label": cw.label, "fields": CAS_FIELDS }),
                    );
                    Ok(format!("{} crashes", fmt::int(n as f64)))
                } else {
                    d.extra.insert("crashesLong".to_string(), arr);
                    d.loaded.get_mut().expect("loaded").insert(
                        "casLong".to_string(),
                        json!({
                            "at": now, "n": n, "from": json_number(cw.from), "to": json_number(cw.to), "ytd": json_number(cw.ytd), "years": json_number(cw.years), "lagYear": lag, "label": cw.label,
                            "purpose": "state highway accesses",
                            "bounds": [[json_number(r7(b.south)), json_number(r7(b.west))], [json_number(r7(b.north)), json_number(r7(b.east))]],
                            "fields": CAS_FIELDS,
                        }),
                    );
                    Ok(format!("{} crashes over {} years (state highway accesses)", fmt::int(n as f64), number_to_js_string(cw.years)))
                }
            }
            "fc" => {
                let fc = parse_future_connect(features);
                let n = fc.len();
                let idx = {
                    let active = p.active.as_str().and_then(|id| p.scenarios.iter().position(|s| s.id == id));
                    active.or(if p.scenarios.is_empty() { None } else { Some(0) })
                };
                let Some(idx) = idx else { return Err("Cannot read properties of undefined (reading 'nodes')".to_string()) };
                let m = conflate_fc(&mut p.scenarios[idx], &fc, None, None, self.now_ms);
                let d = p.data.get_mut().expect("data");
                d.extra.insert("fc".to_string(), serde_json::to_value(&fc).expect("fc"));
                d.loaded.get_mut().expect("loaded").insert(
                    "fc".to_string(),
                    json!({ "at": now, "n": n, "source": "AT Future Connect (integrated network)", "url": AT_FC, "licence": AT_LICENCE, "matched": m.matched, "links": m.links, "arterialShare": m.arterial_share.map_or(Value::Null, json_number) }),
                );
                Ok(format!("{} Future Connect sections ({} of {} links classified)", n, m.matched, m.links))
            }
            "controlled" => {
                let list = parse_controlled(features);
                let n = list.len();
                let d = p.data.get_mut().expect("data");
                d.controlled = crate::jsnum::Maybe::Set(serde_json::to_value(&list).expect("sites"));
                d.loaded.get_mut().expect("loaded").insert(
                    "controlled".to_string(),
                    json!({ "at": now, "n": n, "source": "AT controlled intersections (RoadingService layer 2)", "url": AT_CI, "licence": AT_LICENCE }),
                );
                Ok(format!("{n} AT controlled intersections"))
            }
            _ => {
                let list = if w == "nzta" { parse_nzta(features)? } else { parse_at(features)? };
                let d = p.data.get_mut().expect("data");
                let old: Vec<Value> = match d.extra.get("counts") {
                    Some(Value::Array(a)) => a.clone(),
                    _ => Vec::new(),
                };
                let src_is = |c: &Value| c.get("src").and_then(Value::as_str) == Some(w);
                let keep: Vec<&Value> = old.iter().filter(|c| src_is(c)).collect();
                let mut counts: Vec<Value> = old.iter().filter(|c| !src_is(c)).cloned().collect();
                for c in &list {
                    let mut v = serde_json::to_value(c).expect("count");
                    let prior = keep.iter().find(|k| k.get("id").and_then(Value::as_str) == Some(c.id.as_str()));
                    // off: the earlier record's switch where there is one, else false
                    match prior {
                        Some(k) => {
                            if let Some(off) = k.get("off") {
                                v["off"] = off.clone();
                            }
                        }
                        None => v["off"] = Value::Bool(false),
                    }
                    counts.push(v);
                }
                d.extra.insert("counts".to_string(), Value::Array(counts));
                d.loaded.get_mut().expect("loaded").insert(w.to_string(), json!({ "at": now, "n": list.len() }));
                Ok(format!("{} {} count sites", list.len(), source(w).map_or("", |s| s.short)))
            }
        }
    }
}
