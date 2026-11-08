//! Downloading counts and crashes for a study area (data.js `DataHub.loadAll` and the loaders behind it) and the project's data store (`DataHub.D()`).
//!
//! The committed `datahub` module holds the counts and the maths on them; it has no downloader, so the downloads of the three free sources the study area
//! setup and the Drury sample use are here: NZTA state highway count sites, Auckland Transport counts and NZTA CAS crash records. A source that fails is
//! reported in the messages and the others carry on; a cancel (the host's [`Failure::Cancelled`]) stops the whole load.

use super::mapbox::{nz_date, nz_month_year, utc_date};
use crate::census::{Bx, Failure, Net, host_of, query_string};
use crate::edit::IdSource;
use crate::jsnum::{
    Maybe, json_number, math_max, math_round, number_to_js_string, to_fixed, value_to_js_string,
    value_to_number, value_truthy,
};
use crate::project::{OrderedMap, Project, ProjectData};
use crate::util::{fmt, r7};
use serde_json::{Map, Value, json};

const NZTA_SITES: &str = "https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services/Assets_SHTrafficMonitoringSites/FeatureServer/0";
const NZTA_CAS: &str = "https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services/CAS_Data_Public/FeatureServer/0";
const AT_COUNTS: &str = "https://services2.arcgis.com/JkPEgZJGxhSjYOo0/arcgis/rest/services/TrafficService/FeatureServer/0";
/// The time limit of one ArcGIS request (ms).
pub const ARCGIS_TIMEOUT_MS: f64 = 30000.0;
/// The crash fields at fixed positions in a crash tuple (from position 7).
pub const CAS_FIELDS: [&str; 8] = [
    "fatalCount",
    "seriousInjuryCount",
    "minorInjuryCount",
    "light",
    "weatherA",
    "OBJECTID",
    "trafficControl",
    "urban",
];
const CAS_OUT: &str = "OBJECTID,crashSeverity,crashYear,crashLocation1,crashLocation2,speedLimit,pedestrian,bicycle,motorcycle,fatalCount,seriousInjuryCount,minorInjuryCount,light,weatherA,trafficControl,urban";
/// Full years of crashes in the main tables.
pub const CAS_YEARS: f64 = 5.0;
/// Full years in the long window for state highway accesses.
pub const CAS_LONG_YEARS: f64 = 10.0;
const YTD_LABEL: &str = "YTD, incomplete";

/// Why a count or crash download did not give its list.
#[derive(Clone, Debug, PartialEq)]
pub enum DataError {
    /// The user cancelled (the host's message); the whole load stops.
    Cancelled(String),
    /// The source did not answer or answered badly (the prototype's words); the other sources carry on.
    Failed(String),
}

/// What a download is asked to do (the `opts` of `loadAll`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LoadOpts {
    /// Full years of crashes: 5 (the default) or 10; `+x || 5`, at least 1, rounded.
    pub cas_years: Option<f64>,
    /// The time limit of each request in ms (30 s when not above 0).
    pub timeout_ms: Option<f64>,
}

/// `DataHub.D()`: the project's data store with its defaults filled in: `counts`, `crashes`, `loaded`, `jt` and the source switches in `use`.
pub fn data_d(p: &mut Project) -> &mut ProjectData {
    if p.data.get().is_none() {
        p.data = Maybe::Set(ProjectData {
            loaded: Maybe::Missing,
            msm: Maybe::Missing,
            controlled: Maybe::Missing,
            extra: Map::new(),
        });
    }
    let d = p.data.get_mut().expect("data");
    for k in ["counts", "crashes", "jt"] {
        if !d.extra.get(k).is_some_and(value_truthy) {
            d.extra.insert(k.to_string(), json!([]));
        }
    }
    if !d.loaded.get().is_some() {
        d.loaded = Maybe::Set(OrderedMap::new());
    }
    let mut use_ = Map::new();
    for (k, v) in [
        ("nzta", true),
        ("at", true),
        ("user", true),
        ("virtual", false),
        ("ramm", true),
    ] {
        use_.insert(k.to_string(), json!(v));
    }
    if let Some(Value::Object(own)) = d.extra.get("use") {
        for (k, v) in own {
            use_.insert(k.clone(), v.clone());
        }
    }
    d.extra.insert("use".to_string(), Value::Object(use_));
    d
}

/// The counts of the data store (`D().counts`).
pub fn counts_of(p: &mut Project) -> &mut Vec<Value> {
    let d = data_d(p);
    match d.extra.get_mut("counts") {
        Some(Value::Array(a)) => a,
        _ => unreachable!("data_d made the list"),
    }
}

/// The crashes of the data store (`D().crashes`).
pub fn crashes_of(p: &mut Project) -> &mut Vec<Value> {
    let d = data_d(p);
    match d.extra.get_mut("crashes") {
        Some(Value::Array(a)) => a,
        _ => unreachable!("data_d made the list"),
    }
}

fn plain_str(v: Option<&Value>) -> String {
    match v {
        Some(x) if value_truthy(x) => value_to_js_string(x),
        _ => String::new(),
    }
}

// /\b[a-z]/g over the lower-cased text: a letter at a word edge (ASCII word characters) becomes a capital
fn title(v: Option<&Value>) -> String {
    let s = plain_str(v).to_lowercase();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = String::new();
    let mut prev: Option<char> = None;
    for c in s.chars() {
        let boundary = !prev.is_some_and(word);
        if c.is_ascii_lowercase() && boundary {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c);
        }
        prev = Some(c);
    }
    out
}

// /\b(NB|SB|EB|WB)\b/ in the upper-cased text
fn dir_of(s: &str) -> Option<&'static str> {
    let up: Vec<char> = s.to_uppercase().chars().collect();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    for i in 0..up.len().saturating_sub(1) {
        let pair: String = up[i..i + 2].iter().collect();
        let d = match pair.as_str() {
            "NB" => "NB",
            "SB" => "SB",
            "EB" => "EB",
            "WB" => "WB",
            _ => continue,
        };
        let before = i == 0 || !word(up[i - 1]);
        let after = i + 2 >= up.len() || !word(up[i + 2]);
        if before && after {
            return Some(d);
        }
    }
    None
}

// /\bramp\b/i
fn has_ramp(s: &str) -> bool {
    let low: Vec<char> = s.to_lowercase().chars().collect();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let pat: Vec<char> = "ramp".chars().collect();
    (0..low.len().saturating_sub(3)).any(|i| {
        low[i..i + 4] == pat[..]
            && (i == 0 || !word(low[i - 1]))
            && (i + 4 >= low.len() || !word(low[i + 4]))
    })
}

/// `arcgis(url, b, where, fields, timeout)`: every feature of a layer inside the box, a page at a time (at most 25 pages). Each request has its own time limit.
pub fn arcgis(
    net: &mut dyn Net,
    url: &str,
    b: &Bx,
    where_: &str,
    fields: &str,
    timeout_ms: f64,
) -> Result<Vec<Value>, DataError> {
    let host = host_of(url);
    let secs = number_to_js_string(math_round(timeout_ms / 1000.0));
    let mut out: Vec<Value> = Vec::new();
    let mut offset = 0usize;
    for _ in 0..25 {
        let geom = [b.w, b.s, b.e, b.n]
            .iter()
            .map(|v| to_fixed(*v, 6))
            .collect::<Vec<_>>()
            .join(",");
        let q = query_string(&[
            ("where", where_.to_string()),
            ("geometry", geom),
            ("geometryType", "esriGeometryEnvelope".to_string()),
            ("inSR", "4326".to_string()),
            ("spatialRel", "esriSpatialRelIntersects".to_string()),
            ("outFields", fields.to_string()),
            ("outSR", "4326".to_string()),
            ("f", "json".to_string()),
            ("resultOffset", offset.to_string()),
            ("orderByFields", "OBJECTID".to_string()),
        ]);
        let j = match net.get_json(&format!("{url}/query?{q}")) {
            Ok(j) => j,
            Err(Failure::Cancelled(m)) => return Err(DataError::Cancelled(m)),
            Err(Failure::Timeout) | Err(Failure::ReadTimeout) => {
                return Err(DataError::Failed(format!(
                    "{host} did not answer within {secs} s. Try again later"
                )));
            }
            Err(Failure::Unreachable(m)) => {
                return Err(DataError::Failed(format!(
                    "{host} could not be reached ({m})"
                )));
            }
            Err(Failure::Http(s)) => {
                return Err(DataError::Failed(format!("{host} answered HTTP {s}")));
            }
            Err(Failure::Unreadable(_)) => {
                return Err(DataError::Failed(format!(
                    "{host} sent an answer Spare Construct could not read"
                )));
            }
        };
        if !j.is_object() {
            return Err(DataError::Failed(format!(
                "{host} sent an answer Spare Construct could not read"
            )));
        }
        if let Some(e) = j.get("error").filter(|e| value_truthy(e)) {
            let msg = e
                .get("message")
                .filter(|m| value_truthy(m))
                .map_or_else(|| "query failed".to_string(), value_to_js_string);
            return Err(DataError::Failed(msg));
        }
        let f = j
            .get("features")
            .filter(|x| value_truthy(x))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let n = f.len();
        out.extend(f);
        if !j.get("exceededTransferLimit").is_some_and(value_truthy) || n == 0 {
            break;
        }
        offset += n;
    }
    Ok(out)
}

fn put(o: &mut Map<String, Value>, k: &str, v: Option<&Value>) {
    if let Some(x) = v {
        o.insert(k.to_string(), x.clone());
    }
}

fn geo_xy(x: &Value) -> Option<(f64, f64)> {
    let g = x.get("geometry").filter(|g| value_truthy(g))?;
    Some((
        g.get("y").map_or(f64::NAN, value_to_number),
        g.get("x").map_or(f64::NAN, value_to_number),
    ))
}

fn or_chain(a: Option<&Value>, b: Option<&Value>) -> Value {
    match a {
        Some(v) if value_truthy(v) => v.clone(),
        _ => match b {
            Some(v) if value_truthy(v) => v.clone(),
            _ => Value::Null,
        },
    }
}

/// `loadNZTA(b)`: the NZTA state highway count sites in the box that have a daily count.
pub fn load_nzta(net: &mut dyn Net, b: &Bx, timeout_ms: f64) -> Result<Vec<Value>, DataError> {
    let f = arcgis(
        net,
        NZTA_SITES,
        b,
        "1=1",
        "OBJECTID,siteref,description,lane,sh,sitetype,type,percentheavy,aadt1yearago,aadt2yearsago,aadt3yearsago,aadt4yearsago,aadt5yearsago,accepteddays",
        timeout_ms,
    )?;
    let mut out = Vec::new();
    for x in f
        .iter()
        .filter(|x| x.get("geometry").is_some_and(value_truthy))
    {
        let empty = json!({});
        let a = x
            .get("attributes")
            .filter(|a| value_truthy(a))
            .unwrap_or(&empty);
        let (y, xx) = geo_xy(x).expect("geometry");
        let siteref = a.get("siteref");
        let desc = a.get("description");
        let hist: Vec<Value> = [
            "aadt5yearsago",
            "aadt4yearsago",
            "aadt3yearsago",
            "aadt2yearsago",
            "aadt1yearago",
        ]
        .iter()
        .map(|k| a.get(*k).cloned().unwrap_or(Value::Null))
        .collect();
        let adt = or_chain(a.get("aadt1yearago"), a.get("aadt2yearsago"));
        // 'SH' + (a.sh || '').replace(/[NS]$/, '')
        let sh = plain_str(a.get("sh"));
        let sh_road = if sh.ends_with('N') || sh.ends_with('S') {
            sh[..sh.len() - 1].to_string()
        } else {
            sh.clone()
        };
        let sh_raw = a
            .get("sh")
            .map_or("undefined".to_string(), value_to_js_string);
        let sitetype = a.get("sitetype");
        let virtual_ = sitetype.and_then(Value::as_str) == Some("Virtual")
            || a.get("type").and_then(Value::as_str) == Some("Virtual");
        let mut c = Map::new();
        c.insert(
            "id".into(),
            json!(format!(
                "nzta:{}",
                siteref.map_or("undefined".to_string(), value_to_js_string)
            )),
        );
        c.insert("src".into(), json!("nzta"));
        put(&mut c, "ref", siteref);
        let name = match desc {
            Some(d) if value_truthy(d) => value_to_js_string(d),
            _ => format!(
                "SH{} site {}",
                sh_raw,
                siteref.map_or("undefined".to_string(), value_to_js_string)
            ),
        };
        c.insert("name".into(), json!(name));
        c.insert("road".into(), json!(format!("SH{sh_road}")));
        c.insert("lat".into(), json_number(r7(y)));
        c.insert("lng".into(), json_number(r7(xx)));
        c.insert("adt".into(), adt.clone());
        put(&mut c, "hv", a.get("percentheavy"));
        c.insert(
            "dir".into(),
            dir_of(&plain_str(desc)).map_or(Value::Null, |d| json!(d)),
        );
        c.insert("directional".into(), json!(true));
        c.insert("virtual".into(), json!(virtual_));
        c.insert("ramp".into(), json!(has_ramp(&plain_str(desc))));
        c.insert("hist".into(), Value::Array(hist));
        let note = if sitetype.and_then(Value::as_str) == Some("Virtual") {
            "NZTA estimate (virtual site), not a physical count".to_string()
        } else {
            let days = a
                .get("accepteddays")
                .filter(|d| value_truthy(d))
                .map(value_to_js_string);
            format!(
                "{} site{}",
                match sitetype {
                    Some(s) if value_truthy(s) => value_to_js_string(s),
                    _ => "Count".to_string(),
                },
                days.map_or(String::new(), |d| format!(", {d} counted days last year"))
            )
        };
        c.insert("note".into(), json!(note));
        // .filter(c => c.adt > 0)
        if value_to_number(&adt) > 0.0 {
            out.push(Value::Object(c));
        }
    }
    Ok(out)
}

// /^SH\s*\d/i
fn is_state_highway(name: &str) -> bool {
    let up: Vec<char> = name.to_uppercase().chars().collect();
    if up.len() < 3 || up[0] != 'S' || up[1] != 'H' {
        return false;
    }
    let mut i = 2;
    while i < up.len() && crate::jsnum::is_js_space(up[i]) {
        i += 1;
    }
    i < up.len() && up[i].is_ascii_digit()
}

/// `loadAT(b)`: Auckland Transport's latest counts in the box (state highway sections are left to NZTA's own counts) that have a daily count.
pub fn load_at(net: &mut dyn Net, b: &Bx, timeout_ms: f64) -> Result<Vec<Value>, DataError> {
    let f = arcgis(
        net,
        AT_COUNTS,
        b,
        "latest='Yes'",
        "OBJECTID,road_name,start_name,end_name,count_date,peak_hour,count_duration,adt,peaktraffic,pcheavy,carr_way_no",
        timeout_ms,
    )?;
    let mut out = Vec::new();
    for x in f
        .iter()
        .filter(|x| x.get("geometry").is_some_and(value_truthy))
    {
        let empty = json!({});
        let a = x
            .get("attributes")
            .filter(|a| value_truthy(a))
            .unwrap_or(&empty);
        if is_state_highway(&plain_str(a.get("road_name"))) {
            continue;
        }
        let (y, xx) = geo_xy(x).expect("geometry");
        let ph = {
            let s = plain_str(a.get("peak_hour"));
            format!(
                "{}{}",
                "0".repeat(4usize.saturating_sub(s.chars().count())),
                s
            )
        };
        let date_ms = a
            .get("count_date")
            .filter(|d| value_truthy(d))
            .map(value_to_number);
        if date_ms.is_some_and(|ms| !ms.is_finite()) {
            // new Date(x).toISOString() throws for a date that is not a date
            return Err(DataError::Failed("Invalid time value".to_string()));
        }
        let oid = a.get("OBJECTID");
        let mut c = Map::new();
        c.insert(
            "id".into(),
            json!(format!(
                "at:{}",
                oid.map_or("undefined".to_string(), value_to_js_string)
            )),
        );
        c.insert("src".into(), json!("at"));
        let rf = match a.get("carr_way_no") {
            Some(v) if value_truthy(v) => value_to_js_string(v),
            _ => oid.map_or("undefined".to_string(), value_to_js_string),
        };
        c.insert("ref".into(), json!(rf));
        c.insert(
            "name".into(),
            json!(format!(
                "{} \u{b7} {} to {}",
                title(a.get("road_name")),
                title(a.get("start_name")),
                title(a.get("end_name"))
            )),
        );
        put(&mut c, "road", a.get("road_name"));
        c.insert("lat".into(), json_number(r7(y)));
        c.insert("lng".into(), json_number(r7(xx)));
        put(&mut c, "adt", a.get("adt"));
        put(&mut c, "peak", a.get("peaktraffic"));
        let chars: Vec<char> = ph.chars().collect();
        c.insert(
            "peakTime".into(),
            json!(format!(
                "{}:{}",
                chars.iter().take(2).collect::<String>(),
                chars.iter().skip(2).collect::<String>()
            )),
        );
        put(&mut c, "hv", a.get("pcheavy"));
        c.insert(
            "date".into(),
            date_ms.map_or(Value::Null, |ms| json!(utc_date(ms))),
        );
        put(&mut c, "dur", a.get("count_duration"));
        c.insert("directional".into(), json!(false));
        c.insert(
            "note".into(),
            json!(format!(
                "{} count{}",
                plain_str(a.get("count_duration")),
                date_ms.map_or(String::new(), |ms| format!(", {}", nz_month_year(ms)))
            )),
        );
        let adt = a.get("adt").map_or(f64::NAN, value_to_number);
        if adt > 0.0 {
            out.push(Value::Object(c));
        }
    }
    Ok(out)
}

/// The crash window (`casWindow`): the last full years and the year to date, by New Zealand's calendar.
#[derive(Clone, Debug, PartialEq)]
pub struct CasWindow {
    /// First full year.
    pub from: i64,
    /// Last full year.
    pub to: i64,
    /// The year to date.
    pub ytd: i64,
    /// Full years.
    pub years: i64,
    /// The last full year when it may be incomplete (before 1 April the entry lag can leave it so).
    pub lag_year: Option<i64>,
    /// The window in words.
    pub label: String,
    /// The ArcGIS where clause.
    pub where_: String,
}

/// `casWindow(now, years)`: the window of crash years at an instant (ms since 1970).
pub fn cas_window(now_ms: f64, years: Option<f64>) -> CasWindow {
    let t = nz_date(now_ms);
    let y = t.y;
    let n = math_max(
        1.0,
        math_round(crate::jsnum::or_default(
            years.unwrap_or(CAS_YEARS),
            CAS_YEARS,
        )),
    ) as i64;
    let (from, to) = (y - n, y - 1);
    let lag_year = if t.m < 4 { Some(to) } else { None };
    CasWindow {
        from,
        to,
        ytd: y,
        years: n,
        lag_year,
        label: format!("{from}\u{2013}{to} ({n} full years), plus {y} to date ({YTD_LABEL})"),
        where_: format!("crashYear>={from}"),
    }
}

// A crash as a tuple: [lat, lng, severity, year, location, flags, speed limit, then the fixed extra fields with trailing nulls left off]
fn cas_tuple(x: &Value) -> Value {
    let empty = json!({});
    let a = x
        .get("attributes")
        .filter(|a| value_truthy(a))
        .unwrap_or(&empty);
    let (y, xx) = geo_xy(x).expect("geometry");
    let sev = {
        let s = match a.get("crashSeverity") {
            Some(v) if value_truthy(v) => value_to_js_string(v),
            _ => "N".to_string(),
        };
        s.chars()
            .next()
            .map(|c| c.to_uppercase().collect::<String>())
            .unwrap_or_default()
    };
    let sev = if "FSMN".contains(&sev) {
        sev
    } else {
        "N".to_string()
    };
    let loc: Vec<String> = ["crashLocation1", "crashLocation2"]
        .iter()
        .filter_map(|k| a.get(*k).filter(|v| value_truthy(v)))
        .map(|v| title(Some(v)))
        .collect();
    let flags = format!(
        "{}{}{}",
        if a.get("pedestrian").is_some_and(value_truthy) {
            "p"
        } else {
            ""
        },
        if a.get("bicycle").is_some_and(value_truthy) {
            "c"
        } else {
            ""
        },
        if a.get("motorcycle").is_some_and(value_truthy) {
            "m"
        } else {
            ""
        }
    );
    let mut t: Vec<Value> = vec![
        json_number(r7(y)),
        json_number(r7(xx)),
        json!(sev),
        a.get("crashYear").cloned().unwrap_or(Value::Null),
        json!(loc.join(" / ")),
        json!(flags),
        a.get("speedLimit").cloned().unwrap_or(Value::Null),
    ];
    let num = |v: Option<&Value>| -> Value {
        match v {
            None => Value::Null,
            Some(x) if x.is_null() || x.as_str() == Some("") => Value::Null,
            Some(x) => {
                let n = value_to_number(x);
                if n.is_finite() {
                    json_number(n)
                } else {
                    Value::Null
                }
            }
        }
    };
    let txt = |v: Option<&Value>| -> Value {
        match v {
            None => Value::Null,
            Some(x) if x.is_null() || x.as_str() == Some("") => Value::Null,
            Some(x) => json!(value_to_js_string(x)),
        }
    };
    let mut extra: Vec<Value> = CAS_FIELDS
        .iter()
        .map(|k| {
            if k.ends_with("Count") || *k == "OBJECTID" {
                num(a.get(*k))
            } else {
                txt(a.get(*k))
            }
        })
        .collect();
    while extra.last().is_some_and(Value::is_null) {
        extra.pop();
    }
    t.extend(extra);
    Value::Array(t)
}

/// `loadCAS(b, years, now)`: the crash records of the window in the box, as tuples.
pub fn load_cas(
    net: &mut dyn Net,
    b: &Bx,
    years: f64,
    now_ms: f64,
    timeout_ms: f64,
) -> Result<Vec<Value>, DataError> {
    let w = cas_window(now_ms, Some(years));
    let f = arcgis(net, NZTA_CAS, b, &w.where_, CAS_OUT, timeout_ms)?;
    Ok(f.iter()
        .filter(|x| x.get("geometry").is_some_and(value_truthy))
        .map(cas_tuple)
        .collect())
}

fn timeout_of(o: &LoadOpts) -> f64 {
    match o.timeout_ms {
        Some(t) if t > 0.0 => t,
        _ => ARCGIS_TIMEOUT_MS,
    }
}

/// `DataHub.loadAll(b, which, status, opts)`: downloads the listed sources ("nzta", "at", "cas", "cas10") for a box into the project's data store. A source
/// that fails is a message in the answer ("NZTA failed (...)") and the others carry on. Gives the messages ("12 NZTA count sites", "340 crashes"), or the
/// host's message when the user cancelled.
pub fn load_all(
    p: &mut Project,
    ids: &dyn IdSource,
    net: &mut dyn Net,
    b: &Bx,
    which: &[&str],
    status: &mut dyn FnMut(&str),
    opts: &LoadOpts,
) -> Result<Vec<String>, String> {
    data_d(p);
    let mut msgs: Vec<String> = Vec::new();
    let timeout = timeout_of(opts);
    let now = ids.now_ms();
    for w in which {
        let r: Result<(), DataError> = (|| {
            match *w {
                "cas" => {
                    status("Downloading crash records (NZTA CAS)\u{2026}");
                    let cw = cas_window(now, opts.cas_years);
                    let list = load_cas(net, b, cw.years as f64, now, timeout)?;
                    let n_ytd = list
                        .iter()
                        .filter(|c| {
                            c.get(3).is_some_and(|y| {
                                value_to_number(y) == cw.ytd as f64 && y.is_number()
                            })
                        })
                        .count();
                    let d = data_d(p);
                    msgs.push(format!("{} crashes", fmt::int(list.len() as f64)));
                    loaded_mut(d).insert(
                        "cas".to_string(),
                        json!({ "at": json_number(now), "n": list.len(), "from": cw.from, "to": cw.to, "ytd": cw.ytd, "years": cw.years, "nYTD": n_ytd, "lagYear": cw.lag_year, "label": cw.label, "fields": CAS_FIELDS }),
                    );
                    d.extra.insert("crashes".to_string(), Value::Array(list));
                }
                "cas10" => {
                    status(
                        "Downloading 10 years of crash records for state highway accesses (NZTA CAS)\u{2026}",
                    );
                    let cw = cas_window(now, Some(CAS_LONG_YEARS));
                    let list = load_cas(net, b, cw.years as f64, now, timeout)?;
                    let d = data_d(p);
                    msgs.push(format!(
                        "{} crashes over {} years (state highway accesses)",
                        fmt::int(list.len() as f64),
                        cw.years
                    ));
                    loaded_mut(d).insert(
                        "casLong".to_string(),
                        json!({ "at": json_number(now), "n": list.len(), "from": cw.from, "to": cw.to, "ytd": cw.ytd, "years": cw.years, "lagYear": cw.lag_year, "label": cw.label,
                            "purpose": "state highway accesses", "bounds": [[json_number(r7(b.s)), json_number(r7(b.w))], [json_number(r7(b.n)), json_number(r7(b.e))]], "fields": CAS_FIELDS }),
                    );
                    d.extra
                        .insert("crashesLong".to_string(), Value::Array(list));
                }
                "nzta" | "at" => {
                    let (label, short) = if *w == "nzta" {
                        ("NZTA state highway counts", "NZTA")
                    } else {
                        ("Auckland Transport counts", "AT")
                    };
                    status(&format!("Downloading {label}\u{2026}"));
                    let list = if *w == "nzta" {
                        load_nzta(net, b, timeout)?
                    } else {
                        load_at(net, b, timeout)?
                    };
                    let d = data_d(p);
                    let old: Vec<Value> = d
                        .extra
                        .get("counts")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    // the switches of counts already loaded are kept
                    let keep: Vec<(String, Option<Value>)> = old
                        .iter()
                        .filter(|c| c.get("src").and_then(Value::as_str) == Some(w))
                        .map(|c| {
                            (
                                value_to_js_string(c.get("id").unwrap_or(&Value::Null)),
                                c.get("off").cloned(),
                            )
                        })
                        .collect();
                    let mut counts: Vec<Value> = old
                        .into_iter()
                        .filter(|c| c.get("src").and_then(Value::as_str) != Some(w))
                        .collect();
                    for mut c in list.clone() {
                        let id = value_to_js_string(c.get("id").unwrap_or(&Value::Null));
                        let off = match keep.iter().find(|(k, _)| *k == id) {
                            Some((_, off)) => off.clone(),
                            None => Some(json!(false)),
                        };
                        if let (Some(o), Some(m)) = (off, c.as_object_mut()) {
                            m.insert("off".into(), o);
                        }
                        counts.push(c);
                    }
                    d.extra.insert("counts".to_string(), Value::Array(counts));
                    loaded_mut(d).insert(
                        w.to_string(),
                        json!({ "at": json_number(now), "n": list.len() }),
                    );
                    msgs.push(format!("{} {short} count sites", list.len()));
                }
                other => msgs.push(format!(
                    "{} is not a source Spare Construct downloads here",
                    other.to_uppercase()
                )),
            }
            Ok(())
        })();
        match r {
            Ok(()) => {}
            Err(DataError::Cancelled(m)) => return Err(m),
            Err(DataError::Failed(m)) => msgs.push(format!("{} failed ({m})", w.to_uppercase())),
        }
    }
    Ok(msgs)
}

fn loaded_mut(d: &mut ProjectData) -> &mut OrderedMap<Value> {
    if d.loaded.get().is_none() {
        d.loaded = Maybe::Set(OrderedMap::new());
    }
    d.loaded.get_mut().expect("loaded")
}
