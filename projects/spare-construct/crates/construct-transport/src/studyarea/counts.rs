//! Downloading counts and crashes for a study area (data.js `DataHub.loadAll` and the loaders behind it) and the project's data store (`DataHub.D()`).
//!
//! The committed `datahub` module holds the counts and the maths on them; it has no downloader, so the downloads of the three free sources the study area
//! setup and the Drury sample use are here: NZTA state highway count sites, Auckland Transport counts and NZTA CAS crash records. A source that fails is
//! reported in the messages and the others carry on; a cancel (the host's [`Failure::Cancelled`]) stops the whole load.

use super::mapbox::{nz_date, nz_month_year, utc_date};
use crate::census::{Bx, Failure, Net, host_of, query_string};
use crate::edit::IdSource;
use crate::jsnum::{Maybe, f64_to_i64, i64_to_f64, json_number, math_max, math_round, number_to_js_string, to_fixed, usize_to_f64, value_to_js_string, value_to_number, value_truthy};
use crate::project::{OrderedMap, Project, ProjectData};
use crate::util::{fmt, r7};
use serde_json::{Map, Value, json};

const NZTA_SITES: &str = "https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services/Assets_SHTrafficMonitoringSites/FeatureServer/0";
const NZTA_CAS: &str = "https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services/CAS_Data_Public/FeatureServer/0";
const AT_COUNTS: &str = "https://services2.arcgis.com/JkPEgZJGxhSjYOo0/arcgis/rest/services/TrafficService/FeatureServer/0";
/// The time limit of one `ArcGIS` request (ms).
pub const ARCGIS_TIMEOUT_MS: f64 = 30000.0;
/// The crash fields at fixed positions in a crash tuple (from position 7).
pub const CAS_FIELDS: [&str; 8] = ["fatalCount", "seriousInjuryCount", "minorInjuryCount", "light", "weatherA", "OBJECTID", "trafficControl", "urban"];
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
///
/// Always `Some`: a missing store is made first (the Option only spares a panic on a branch that cannot run).
pub fn data_d(p: &mut Project) -> Option<&mut ProjectData> {
    let fresh = || ProjectData { loaded: Maybe::Missing, msm: Maybe::Missing, controlled: Maybe::Missing, extra: Map::new() };
    if p.data.get().is_none() {
        p.data = Maybe::Set(fresh());
    }
    let d = p.data.get_mut()?;
    for k in ["counts", "crashes", "jt"] {
        if !d.extra.get(k).is_some_and(value_truthy) {
            d.extra.insert(k.to_string(), json!([]));
        }
    }
    if d.loaded.get().is_none() {
        d.loaded = Maybe::Set(OrderedMap::new());
    }
    let mut use_ = Map::new();
    for (k, v) in [("nzta", true), ("at", true), ("user", true), ("virtual", false), ("ramm", true)] {
        use_.insert(k.to_string(), json!(v));
    }
    if let Some(Value::Object(own)) = d.extra.get("use") {
        for (k, v) in own {
            use_.insert(k.clone(), v.clone());
        }
    }
    d.extra.insert("use".to_string(), Value::Object(use_));
    Some(d)
}

/// The counts of the data store (`D().counts`); always `Some`, as [`data_d`] is.
pub fn counts_of(p: &mut Project) -> Option<&mut Vec<Value>> {
    list_of(data_d(p)?, "counts")
}

/// The crashes of the data store (`D().crashes`); always `Some`, as [`data_d`] is.
pub fn crashes_of(p: &mut Project) -> Option<&mut Vec<Value>> {
    list_of(data_d(p)?, "crashes")
}

// A list of the data store: data_d made it when it was missing or falsy; a saved value that is truthy but not a list (which the
// prototype cannot use as one) is replaced by an empty list.
fn list_of<'a>(d: &'a mut ProjectData, key: &str) -> Option<&'a mut Vec<Value>> {
    if !d.extra.get(key).is_some_and(Value::is_array) {
        d.extra.insert(key.to_string(), json!([]));
    }
    d.extra.get_mut(key).and_then(Value::as_array_mut)
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
fn dir_of(text: &str) -> Option<&'static str> {
    let up: Vec<char> = text.to_uppercase().chars().collect();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    for (i, pair) in up.windows(2).enumerate() {
        let dir = match pair {
            ['N', 'B'] => "NB",
            ['S', 'B'] => "SB",
            ['E', 'B'] => "EB",
            ['W', 'B'] => "WB",
            _ => continue,
        };
        // a word edge: the start or end of the text, or a character that is not a word character
        let before = i.checked_sub(1).and_then(|j| up.get(j)).is_none_or(|c| !word(*c));
        let after = up.get(i.saturating_add(2)).is_none_or(|c| !word(*c));
        if before && after {
            return Some(dir);
        }
    }
    None
}

// /\bramp\b/i
fn has_ramp(text: &str) -> bool {
    let low: Vec<char> = text.to_lowercase().chars().collect();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let pat: Vec<char> = "ramp".chars().collect();
    low.windows(4).enumerate().any(|(i, win)| {
        win == pat.as_slice() && i.checked_sub(1).and_then(|j| low.get(j)).is_none_or(|c| !word(*c)) && low.get(i.saturating_add(4)).is_none_or(|c| !word(*c))
    })
}

/// `arcgis(url, b, where, fields, timeout)`: every feature of a layer inside the box, a page at a time (at most 25 pages). Each request has its own time limit.
///
/// # Errors
///
/// [`DataError::Cancelled`] when the user cancels; [`DataError::Failed`] (in the prototype's words) when the server does not answer in
/// time, cannot be reached, answers with an HTTP error or something that is not JSON, or reports an error of its own.
pub fn arcgis(net: &mut dyn Net, url: &str, b: &Bx, where_: &str, fields: &str, timeout_ms: f64) -> Result<Vec<Value>, DataError> {
    let host = host_of(url);
    let secs = number_to_js_string(math_round(timeout_ms / 1000.0));
    let mut out: Vec<Value> = Vec::new();
    let mut offset = 0usize;
    for _ in 0..25 {
        let geom = [b.w, b.s, b.e, b.n].iter().map(|v| to_fixed(*v, 6)).collect::<Vec<_>>().join(",");
        let query = query_string(&[
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
        let reply = match net.get_json(&format!("{url}/query?{query}")) {
            Ok(reply) => reply,
            Err(Failure::Cancelled(msg)) => return Err(DataError::Cancelled(msg)),
            Err(Failure::Timeout | Failure::ReadTimeout) => return Err(DataError::Failed(format!("{host} did not answer within {secs} s. Try again later"))),
            Err(Failure::Unreachable(msg)) => return Err(DataError::Failed(format!("{host} could not be reached ({msg})"))),
            Err(Failure::Http(code)) => return Err(DataError::Failed(format!("{host} answered HTTP {code}"))),
            Err(Failure::Unreadable(_)) => return Err(DataError::Failed(format!("{host} sent an answer Spare Construct could not read"))),
        };
        if !reply.is_object() {
            return Err(DataError::Failed(format!("{host} sent an answer Spare Construct could not read")));
        }
        if let Some(err) = reply.get("error").filter(|e| value_truthy(e)) {
            let msg = err.get("message").filter(|m| value_truthy(m)).map_or_else(|| "query failed".to_string(), value_to_js_string);
            return Err(DataError::Failed(msg));
        }
        let feats = reply.get("features").filter(|x| value_truthy(x)).and_then(Value::as_array).cloned().unwrap_or_default();
        let got = feats.len();
        out.extend(feats);
        if !reply.get("exceededTransferLimit").is_some_and(value_truthy) || got == 0 {
            break;
        }
        // Bound: a count of features held in memory, far below usize::MAX
        offset = offset.saturating_add(got);
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
    Some((g.get("y").map_or(f64::NAN, value_to_number), g.get("x").map_or(f64::NAN, value_to_number)))
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
///
/// # Errors
///
/// As [`arcgis`].
pub fn load_nzta(net: &mut dyn Net, b: &Bx, timeout_ms: f64) -> Result<Vec<Value>, DataError> {
    let feats = arcgis(
        net,
        NZTA_SITES,
        b,
        "1=1",
        "OBJECTID,siteref,description,lane,sh,sitetype,type,percentheavy,aadt1yearago,aadt2yearsago,aadt3yearsago,aadt4yearsago,aadt5yearsago,accepteddays",
        timeout_ms,
    )?;
    let mut out = Vec::new();
    for feat in feats.iter().filter(|x| x.get("geometry").is_some_and(value_truthy)) {
        let empty = json!({});
        let attrs = feat.get("attributes").filter(|a| value_truthy(a)).unwrap_or(&empty);
        // The filter above kept only features with a geometry, so this never skips one
        let Some((lat, lng)) = geo_xy(feat) else { continue };
        let siteref = attrs.get("siteref");
        let desc = attrs.get("description");
        let hist: Vec<Value> = ["aadt5yearsago", "aadt4yearsago", "aadt3yearsago", "aadt2yearsago", "aadt1yearago"].iter().map(|k| attrs.get(*k).cloned().unwrap_or(Value::Null)).collect();
        let adt = or_chain(attrs.get("aadt1yearago"), attrs.get("aadt2yearsago"));
        // 'SH' + (a.sh || '').replace(/[NS]$/, '')
        let sh = plain_str(attrs.get("sh"));
        let sh_road = sh.strip_suffix(['N', 'S']).unwrap_or(sh.as_str()).to_string();
        let sh_raw = attrs.get("sh").map_or("undefined".to_string(), value_to_js_string);
        let sitetype = attrs.get("sitetype");
        let virtual_ = sitetype.and_then(Value::as_str) == Some("Virtual") || attrs.get("type").and_then(Value::as_str) == Some("Virtual");
        let mut site = Map::new();
        site.insert("id".into(), json!(format!("nzta:{}", siteref.map_or("undefined".to_string(), value_to_js_string))));
        site.insert("src".into(), json!("nzta"));
        put(&mut site, "ref", siteref);
        let name = match desc {
            Some(text) if value_truthy(text) => value_to_js_string(text),
            _ => format!("SH{} site {}", sh_raw, siteref.map_or("undefined".to_string(), value_to_js_string)),
        };
        site.insert("name".into(), json!(name));
        site.insert("road".into(), json!(format!("SH{sh_road}")));
        site.insert("lat".into(), json_number(r7(lat)));
        site.insert("lng".into(), json_number(r7(lng)));
        site.insert("adt".into(), adt.clone());
        put(&mut site, "hv", attrs.get("percentheavy"));
        site.insert("dir".into(), dir_of(&plain_str(desc)).map_or(Value::Null, |d| json!(d)));
        site.insert("directional".into(), json!(true));
        site.insert("virtual".into(), json!(virtual_));
        site.insert("ramp".into(), json!(has_ramp(&plain_str(desc))));
        site.insert("hist".into(), Value::Array(hist));
        let note = if sitetype.and_then(Value::as_str) == Some("Virtual") {
            "NZTA estimate (virtual site), not a physical count".to_string()
        } else {
            let days = attrs.get("accepteddays").filter(|d| value_truthy(d)).map(value_to_js_string);
            format!(
                "{} site{}",
                match sitetype {
                    Some(s) if value_truthy(s) => value_to_js_string(s),
                    _ => "Count".to_string(),
                },
                days.map_or(String::new(), |d| format!(", {d} counted days last year"))
            )
        };
        site.insert("note".into(), json!(note));
        // .filter(c => c.adt > 0)
        if value_to_number(&adt) > 0.0 {
            out.push(Value::Object(site));
        }
    }
    Ok(out)
}

// /^SH\s*\d/i
fn is_state_highway(name: &str) -> bool {
    let up: Vec<char> = name.to_uppercase().chars().collect();
    let ['S', 'H', rest @ ..] = up.as_slice() else {
        return false;
    };
    // the first character after "SH" and any spaces must be a digit (none at all, as for "SH" alone, is false)
    rest.iter().find(|c| !crate::jsnum::is_js_space(**c)).is_some_and(char::is_ascii_digit)
}

/// `loadAT(b)`: Auckland Transport's latest counts in the box (state highway sections are left to NZTA's own counts) that have a daily count.
///
/// # Errors
///
/// As [`arcgis`], and [`DataError::Failed`] ("Invalid time value") for a count date that is not a number.
pub fn load_at(net: &mut dyn Net, b: &Bx, timeout_ms: f64) -> Result<Vec<Value>, DataError> {
    let feats = arcgis(net, AT_COUNTS, b, "latest='Yes'", "OBJECTID,road_name,start_name,end_name,count_date,peak_hour,count_duration,adt,peaktraffic,pcheavy,carr_way_no", timeout_ms)?;
    let mut out = Vec::new();
    for feat in feats.iter().filter(|x| x.get("geometry").is_some_and(value_truthy)) {
        let empty = json!({});
        let attrs = feat.get("attributes").filter(|a| value_truthy(a)).unwrap_or(&empty);
        if is_state_highway(&plain_str(attrs.get("road_name"))) {
            continue;
        }
        // The filter above kept only features with a geometry, so this never skips one
        let Some((lat, lng)) = geo_xy(feat) else { continue };
        let ph = {
            let raw = plain_str(attrs.get("peak_hour"));
            format!("{}{}", "0".repeat(4usize.saturating_sub(raw.chars().count())), raw)
        };
        let date_ms = attrs.get("count_date").filter(|d| value_truthy(d)).map(value_to_number);
        if date_ms.is_some_and(|ms| !ms.is_finite()) {
            // new Date(x).toISOString() throws for a date that is not a date
            return Err(DataError::Failed("Invalid time value".to_string()));
        }
        let oid = attrs.get("OBJECTID");
        let mut site = Map::new();
        site.insert("id".into(), json!(format!("at:{}", oid.map_or("undefined".to_string(), value_to_js_string))));
        site.insert("src".into(), json!("at"));
        let rf = match attrs.get("carr_way_no") {
            Some(v) if value_truthy(v) => value_to_js_string(v),
            _ => oid.map_or("undefined".to_string(), value_to_js_string),
        };
        site.insert("ref".into(), json!(rf));
        site.insert("name".into(), json!(format!("{} \u{b7} {} to {}", title(attrs.get("road_name")), title(attrs.get("start_name")), title(attrs.get("end_name")))));
        put(&mut site, "road", attrs.get("road_name"));
        site.insert("lat".into(), json_number(r7(lat)));
        site.insert("lng".into(), json_number(r7(lng)));
        put(&mut site, "adt", attrs.get("adt"));
        put(&mut site, "peak", attrs.get("peaktraffic"));
        let chars: Vec<char> = ph.chars().collect();
        site.insert("peakTime".into(), json!(format!("{}:{}", chars.iter().take(2).collect::<String>(), chars.iter().skip(2).collect::<String>())));
        put(&mut site, "hv", attrs.get("pcheavy"));
        site.insert("date".into(), date_ms.map_or(Value::Null, |ms| json!(utc_date(ms))));
        put(&mut site, "dur", attrs.get("count_duration"));
        site.insert("directional".into(), json!(false));
        site.insert(
            "note".into(),
            json!(format!("{} count{}", plain_str(attrs.get("count_duration")), date_ms.map_or(String::new(), |ms| format!(", {}", nz_month_year(ms))))),
        );
        let adt = attrs.get("adt").map_or(f64::NAN, value_to_number);
        if adt > 0.0 {
            out.push(Value::Object(site));
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
    /// The `ArcGIS` where clause.
    pub where_: String,
}

/// `casWindow(now, years)`: the window of crash years at an instant (ms since 1970).
#[must_use]
pub fn cas_window(now_ms: f64, years: Option<f64>) -> CasWindow {
    let t = nz_date(now_ms);
    let y = t.y;
    let n = f64_to_i64(math_max(1.0, math_round(crate::jsnum::or_default(years.unwrap_or(CAS_YEARS), CAS_YEARS))));
    // Bound: a calendar year and a count of years, far inside i64
    let (from, to) = (y.saturating_sub(n), y.saturating_sub(1));
    let lag_year = if t.m < 4 { Some(to) } else { None };
    CasWindow { from, to, ytd: y, years: n, lag_year, label: format!("{from}\u{2013}{to} ({n} full years), plus {y} to date ({YTD_LABEL})"), where_: format!("crashYear>={from}") }
}

// A crash as a tuple: [lat, lng, severity, year, location, flags, speed limit, then the fixed extra fields with trailing nulls left off];
// None for a feature without a geometry (callers have already left those out)
fn cas_tuple(feat: &Value) -> Option<Value> {
    let empty = json!({});
    let attrs = feat.get("attributes").filter(|a| value_truthy(a)).unwrap_or(&empty);
    let (lat, lng) = geo_xy(feat)?;
    let sev = {
        let text = match attrs.get("crashSeverity") {
            Some(v) if value_truthy(v) => value_to_js_string(v),
            _ => "N".to_string(),
        };
        text.chars().next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default()
    };
    let sev = if "FSMN".contains(&sev) { sev } else { "N".to_string() };
    let loc: Vec<String> = ["crashLocation1", "crashLocation2"].iter().filter_map(|k| attrs.get(*k).filter(|v| value_truthy(v))).map(|v| title(Some(v))).collect();
    let flags = format!(
        "{}{}{}",
        if attrs.get("pedestrian").is_some_and(value_truthy) { "p" } else { "" },
        if attrs.get("bicycle").is_some_and(value_truthy) { "c" } else { "" },
        if attrs.get("motorcycle").is_some_and(value_truthy) { "m" } else { "" }
    );
    let mut tuple: Vec<Value> = vec![json_number(r7(lat)), json_number(r7(lng)), json!(sev), attrs.get("crashYear").cloned().unwrap_or(Value::Null), json!(loc.join(" / ")), json!(flags), attrs.get("speedLimit").cloned().unwrap_or(Value::Null)];
    let num = |v: Option<&Value>| -> Value {
        match v {
            None => Value::Null,
            Some(raw) if raw.is_null() || raw.as_str() == Some("") => Value::Null,
            Some(raw) => {
                let n = value_to_number(raw);
                if n.is_finite() { json_number(n) } else { Value::Null }
            }
        }
    };
    let txt = |v: Option<&Value>| -> Value {
        match v {
            None => Value::Null,
            Some(raw) if raw.is_null() || raw.as_str() == Some("") => Value::Null,
            Some(raw) => json!(value_to_js_string(raw)),
        }
    };
    let mut extra: Vec<Value> = CAS_FIELDS.iter().map(|k| if k.ends_with("Count") || *k == "OBJECTID" { num(attrs.get(*k)) } else { txt(attrs.get(*k)) }).collect();
    while extra.last().is_some_and(Value::is_null) {
        extra.pop();
    }
    tuple.extend(extra);
    Some(Value::Array(tuple))
}

/// `loadCAS(b, years, now)`: the crash records of the window in the box, as tuples.
///
/// # Errors
///
/// As [`arcgis`].
pub fn load_cas(net: &mut dyn Net, b: &Bx, years: f64, now_ms: f64, timeout_ms: f64) -> Result<Vec<Value>, DataError> {
    let w = cas_window(now_ms, Some(years));
    let f = arcgis(net, NZTA_CAS, b, &w.where_, CAS_OUT, timeout_ms)?;
    // cas_tuple gives None only without a geometry, which the filter has already left out
    Ok(f.iter().filter(|x| x.get("geometry").is_some_and(value_truthy)).filter_map(cas_tuple).collect())
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
///
/// # Errors
///
/// The host's message when the user cancels a download (the sources not yet loaded are left as they were).
pub fn load_all(p: &mut Project, ids: &dyn IdSource, net: &mut dyn Net, b: &Bx, which: &[&str], status: &mut dyn FnMut(&str), opts: &LoadOpts) -> Result<Vec<String>, String> {
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
                    let list = load_cas(net, b, i64_to_f64(cw.years), now, timeout)?;
                    let n_ytd = list.iter().filter(|c| c.get(3).is_some_and(|y| value_to_number(y) == i64_to_f64(cw.ytd) && y.is_number())).count();
                    // data_d always gives the store
                    let Some(d) = data_d(p) else {
                        return Ok(());
                    };
                    msgs.push(format!("{} crashes", fmt::int(usize_to_f64(list.len()))));
                    set_loaded(d, 
                        "cas".to_string(),
                        json!({ "at": json_number(now), "n": list.len(), "from": cw.from, "to": cw.to, "ytd": cw.ytd, "years": cw.years, "nYTD": n_ytd, "lagYear": cw.lag_year, "label": cw.label, "fields": CAS_FIELDS }),
                    );
                    d.extra.insert("crashes".to_string(), Value::Array(list));
                }
                "cas10" => {
                    status("Downloading 10 years of crash records for state highway accesses (NZTA CAS)\u{2026}");
                    let cw = cas_window(now, Some(CAS_LONG_YEARS));
                    let list = load_cas(net, b, i64_to_f64(cw.years), now, timeout)?;
                    // data_d always gives the store
                    let Some(d) = data_d(p) else {
                        return Ok(());
                    };
                    msgs.push(format!("{} crashes over {} years (state highway accesses)", fmt::int(usize_to_f64(list.len())), cw.years));
                    set_loaded(d, 
                        "casLong".to_string(),
                        json!({ "at": json_number(now), "n": list.len(), "from": cw.from, "to": cw.to, "ytd": cw.ytd, "years": cw.years, "lagYear": cw.lag_year, "label": cw.label,
                            "purpose": "state highway accesses", "bounds": [[json_number(r7(b.s)), json_number(r7(b.w))], [json_number(r7(b.n)), json_number(r7(b.e))]], "fields": CAS_FIELDS }),
                    );
                    d.extra.insert("crashesLong".to_string(), Value::Array(list));
                }
                "nzta" | "at" => {
                    let (label, short) = if *w == "nzta" { ("NZTA state highway counts", "NZTA") } else { ("Auckland Transport counts", "AT") };
                    status(&format!("Downloading {label}\u{2026}"));
                    let list = if *w == "nzta" { load_nzta(net, b, timeout)? } else { load_at(net, b, timeout)? };
                    // data_d always gives the store
                    let Some(d) = data_d(p) else {
                        return Ok(());
                    };
                    let old: Vec<Value> = d.extra.get("counts").and_then(Value::as_array).cloned().unwrap_or_default();
                    // the switches of counts already loaded are kept
                    let keep: Vec<(String, Option<Value>)> = old.iter().filter(|c| c.get("src").and_then(Value::as_str) == Some(w)).map(|c| (value_to_js_string(c.get("id").unwrap_or(&Value::Null)), c.get("off").cloned())).collect();
                    let mut counts: Vec<Value> = old.into_iter().filter(|c| c.get("src").and_then(Value::as_str) != Some(w)).collect();
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
                    set_loaded(d, w.to_string(), json!({ "at": json_number(now), "n": list.len() }));
                    msgs.push(format!("{} {short} count sites", list.len()));
                }
                other => msgs.push(format!("{} is not a source Spare Construct downloads here", other.to_uppercase())),
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

// loaded[key] = value, making the record when it is missing
fn set_loaded(d: &mut ProjectData, key: String, value: Value) {
    if let Some(loaded) = d.loaded.get_mut() {
        loaded.insert(key, value);
    } else {
        let mut loaded = OrderedMap::new();
        loaded.insert(key, value);
        d.loaded = Maybe::Set(loaded);
    }
}
