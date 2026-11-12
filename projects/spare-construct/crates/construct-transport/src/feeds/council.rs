//! Council count layers (section 6 of feeds.js): a generic `ArcGIS` importer for traffic counts.
//!
//! The layer is read first ([`CouncilRead`]), the fields that hold each value are guessed from the field names and the
//! user confirms them, and then the counts in the study area are downloaded and turned into the user's own counts ([`CouncilImport`]).

use super::arcgis::{Bounds, FetchFailure, HttpRequest};
use super::flow::{env_params, resolve_fields, Answer, ArcQuery, Flow, LayerMeta, MetaCache};
use super::gtfs::hash_str;
use super::jsregex::Regex;
use super::nztime::{civil_from_days, today, DAY_MS};
use super::sources::{council, SRC_COUNCIL};
use super::vintage::stamp;
use crate::datahub::Count;
use crate::edit::json::JsonValue;
use crate::jsnum::{f64_to_i64, i64_to_f64, json_number, math_round, number_to_js_string, usize_to_f64};
use crate::project::Project;
use crate::util::{self, r7};
use serde::Serialize;
use serde_json::{json, Map, Value};

/// The fields guessed to hold each value of a layer (`guessFields`): empty text when none looks right.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuessedFields {
    /// Daily volume.
    pub adt: String,
    /// Peak-hour volume.
    pub peak: String,
    /// Peak-hour time.
    pub peak_time: String,
    /// Heavy vehicles.
    pub hcv: String,
    /// Count date.
    pub date: String,
    /// Site name.
    pub name: String,
    /// Direction.
    pub dir: String,
    /// The flag of a site's latest count.
    pub latest: String,
    /// Year columns (`year_2023` ...), newest first.
    pub years: Vec<String>,
}

/// `guessFields(meta)`: which field of a layer holds the daily volume, peak hour and so on, from the field names and aliases.
#[must_use]
pub fn guess_fields(meta: &JsonValue) -> GuessedFields {
    let skip = Regex::lit("oid|geometry|globalid", "i");
    let fields: Vec<&JsonValue> = match meta.get("fields") {
        Some(JsonValue::Array(a)) => a
            .iter()
            .filter(|f| {
                let t = match f.get("type") {
                    Some(t) if t.truthy() => t.js_string(),
                    _ => String::new(),
                };
                !skip.test(&t)
            })
            .collect(),
        _ => Vec::new(),
    };
    let name_of = |f: &JsonValue| f.get("name").map_or_else(|| "undefined".to_string(), JsonValue::js_string);
    let alias_of = |f: &JsonValue| match f.get("alias") {
        Some(a) if a.truthy() => a.js_string(),
        _ => String::new(),
    };
    let best = |res: &[&str]| -> String {
        for re in res {
            let re = Regex::lit(re, "i");
            if let Some(f) = fields.iter().find(|x| re.test(&name_of(x)) || re.test(&alias_of(x))) {
                return name_of(f);
            }
        }
        String::new()
    };
    let year_re = Regex::lit(r"^year_?(\d{4})$", "i");
    let mut years: Vec<(String, f64)> = fields
        .iter()
        .filter_map(|f| {
            let n = name_of(f);
            let u: Vec<u16> = n.encode_utf16().collect();
            year_re.exec_units(&u, 0).map(|m| (n.clone(), m.text(&u, 1).map_or(f64::NAN, |t| crate::jsnum::string_to_number(&t))))
        })
        .collect();
    crate::graph::js_sort_by(&mut years, |p, q| q.1 - p.1);
    GuessedFields {
        adt: best(&["^adt$", "aadt", "average.*daily", "^vpd$"]),
        peak: best(&["peak.*(traffic|vol)", "peak_?hour_?vol"]),
        peak_time: best(&["peak_?hour$", "peak_?time"]),
        hcv: best(&["heavy|hcv|pc_?hv"]),
        date: best(&["count_?date", "^date$", "survey_?date"]),
        name: best(&["road_?name", "^name$", "street", "location", "site_?desc", "description"]),
        dir: best(&["^dir", "direction"]),
        latest: best(&["^latest$"]),
        years: years.into_iter().map(|(n, _)| n).collect(),
    }
}

/// What reading a layer gives: its cleaned address, name, description, the guessed fields and all field names.
#[derive(Clone, Debug, PartialEq)]
pub struct CouncilLayer {
    /// The layer's address (query and trailing slashes taken off).
    pub url: String,
    /// The layer's name, or "Count layer".
    pub name: String,
    /// The layer's description.
    pub meta: JsonValue,
    /// The guessed fields.
    pub map: GuessedFields,
    /// Every field name.
    pub fields: Vec<String>,
}

/// `councilRead(url)`: the address cleaned up (`/query...`, a query string and trailing slashes taken off), or why it is not a layer.
///
/// # Errors
///
/// The prototype's message when the cleaned address is not one `ArcGIS` layer.
pub fn council_url(url: &str) -> Result<String, String> {
    let t = crate::jsnum::js_trim(url);
    let q1 = Regex::lit(r"\/query.*$", "i");
    let q2 = Regex::lit(r"\?.*$", "");
    let q3 = Regex::lit(r"\/+$", "");
    let u = q3.replace(&q2.replace(&q1.replace(t, |_, _| String::new()), |_, _| String::new()), |_, _| String::new());
    if !Regex::lit(r"^https:\/\/[^/]+\/.+\/(FeatureServer|MapServer)\/\d+$", "i").test(&u) {
        return Err("Paste the address of one ArcGIS layer, ending in /FeatureServer/0 (or another layer number).".to_string());
    }
    Ok(u)
}

/// `councilRead(url)`: reads a layer's description and guesses its fields.
pub struct CouncilRead {
    url: String,
    meta: LayerMeta,
}

impl CouncilRead {
    /// Reads a layer; Err when the text is not the address of a layer.
    ///
    /// # Errors
    ///
    /// As [`council_url`].
    pub fn new(url: &str, timeout_ms: f64, cache: &MetaCache) -> Result<CouncilRead, String> {
        let u = council_url(url)?;
        Ok(CouncilRead { meta: LayerMeta::new(&u, cache, timeout_ms), url: u })
    }
    /// The layer, once its description was read; Err when it has no fields.
    ///
    /// # Errors
    ///
    /// A message when the description was not read or the layer has no fields.
    pub fn finish(self) -> Result<CouncilLayer, String> {
        let meta = self.meta.meta().cloned().ok_or("The layer was not read.")?;
        let fields: Vec<String> = match meta.get("fields") {
            Some(JsonValue::Array(a)) => a.iter().map(|f| f.get("name").map_or_else(|| "undefined".to_string(), JsonValue::js_string)).collect(),
            _ => return Err("That address is not a feature layer with fields.".to_string()),
        };
        let name = match meta.get("name") {
            Some(n) if n.truthy() => n.js_string(),
            _ => "Count layer".to_string(),
        };
        Ok(CouncilLayer { url: self.url, name, map: guess_fields(&meta), meta, fields })
    }
}

impl Flow for CouncilRead {
    fn next_request(&mut self) -> Option<HttpRequest> {
        self.meta.next_request()
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        self.meta.answer(a)
    }
    fn is_done(&self) -> bool {
        self.meta.is_done()
    }
}

/// `yearOfVal(v)`: the year of a count date: the UTC year of a timestamp in ms, else the first four digits of the text; None when there is none.
#[must_use]
pub fn year_of_val(v: Option<&JsonValue>) -> Option<f64> {
    let v = v?;
    match v {
        JsonValue::Null => return None,
        JsonValue::String(s) if s.is_empty() => return None,
        JsonValue::Number(n) if *n > 1e10 => {
            let ms = super::nztime::time_clip(*n)?;
            return Some(i64_to_f64(civil_from_days(f64_to_i64((ms / DAY_MS).floor())).0));
        }
        _ => {}
    }
    let s = v.js_string();
    let re = Regex::lit(r"(\d{4})", "");
    let u: Vec<u16> = s.encode_utf16().collect();
    re.exec_units(&u, 0).and_then(|m| m.text(&u, 1)).map(|t| crate::jsnum::string_to_number(&t))
}

/// The fields the user confirmed: an ordered object of field names (adt, peak, peakTime, hcv, date, name, dir, latest) and the year columns (`years`, a list).
pub type CouncilMap = JsonValue;

/// `councilImport(read, map, b, opts)` after the download: the counts in the layer, as the user's own counts.
///
/// A site's daily volume is the chosen field, else the newest year column with a volume; a count older than the preset's
/// `maxAgeYears` is left out. Counts of this layer already in the project are replaced. `now_ms` dates the import.
pub struct CouncilImport {
    layer: CouncilLayer,
    map: CouncilMap,
    preset: Option<String>,
    now_ms: f64,
    query: ArcQuery,
}

fn map_str(map: &CouncilMap, k: &str) -> String {
    match map.get(k) {
        Some(v) if v.truthy() => v.js_string(),
        _ => String::new(),
    }
}

impl CouncilImport {
    /// A download of the layer's counts in an area; `preset` is "hamilton" or "qldc" (or None).
    pub fn new(layer: CouncilLayer, map: CouncilMap, b: &Bounds, preset: Option<&str>, now_ms: f64, timeout_ms: f64) -> CouncilImport {
        let mut fields: Vec<String> = vec!["OBJECTID".to_string()];
        if let Some(o) = map.as_object() {
            for (_, v) in o.iter() {
                match v {
                    JsonValue::Array(a) => fields.extend(a.iter().filter(|x| x.truthy()).map(JsonValue::js_string)),
                    x if x.truthy() => fields.push(x.js_string()),
                    _ => {}
                }
            }
        }
        let mut uniq: Vec<String> = Vec::new();
        for f in fields {
            if !f.is_empty() && !uniq.contains(&f) {
                uniq.push(f);
            }
        }
        let latest = map_str(&map, "latest");
        let where_ = if latest.is_empty() { "1=1".to_string() } else { format!("{latest} IN ('Y','Yes','YES','y')") };
        let want: Vec<&str> = uniq.iter().map(String::as_str).collect();
        let resolved = resolve_fields(Some(&layer.meta), &want).join(",");
        let mut params: Vec<(String, String)> = vec![("where".into(), where_), ("outFields".into(), if resolved.is_empty() { "*".to_string() } else { resolved })];
        params.extend(env_params(b));
        let query = ArcQuery::new(&layer.url, params, 40, timeout_ms);
        CouncilImport { layer, map, preset: preset.map(str::to_string), now_ms, query }
    }
    /// How many features have been downloaded (for a progress line).
    #[must_use]
    pub fn count(&self) -> usize {
        self.query.features().len()
    }
    /// The counts, after the download: replaces this layer's counts in the project, stamps the vintage table and says how many were imported.
    pub fn finish(self, p: &mut Project) -> CouncilImported {
        let feats = self.query.features().to_vec();
        let complete = self.query.complete();
        let n_features = feats.len();
        let preset = self.preset.as_deref().and_then(council);
        let max_age = preset.and_then(|pr| pr.max_age_years).filter(|age| *age != 0.0);
        let key = hash_str(&self.layer.url);
        let label = preset.map_or_else(|| self.layer.name.clone(), |pr| pr.label.to_string());
        let reader = FeatureReader {
            cols: Columns::of(&self.map),
            layer: &self.layer,
            key: &key,
            label: &label,
            now_ms: self.now_ms,
            y_now: i64_to_f64(super::nztime::nz_year(self.now_ms)),
            max_age,
            dir_re: Regex::lit(r"\b(NB|SB|EB|WB)\b", "i"),
            digits_re: Regex::lit(r"\D", "g"),
        };
        let mut list: Vec<Count> = Vec::new();
        for feat in &feats {
            if let Some(count) = reader.count(feat, list.len()) {
                list.push(count);
            }
        }
        replace_counts(p, &self.layer.url, &list);
        let note = fields_note(&self.map, max_age, complete);
        let mut extra = Map::new();
        extra.insert("key".into(), json!(format!("council:{key}")));
        extra.insert("url".into(), json!(self.layer.url));
        extra.insert("source".into(), json!(label));
        extra.insert("n".into(), json!(list.len()));
        extra.insert("note".into(), json!(note));
        stamp(p, "council", &extra, self.now_ms);
        CouncilImported { n: list.len(), of_: n_features, complete }
    }
    /// The layer being imported.
    #[must_use]
    pub fn layer(&self) -> &CouncilLayer {
        &self.layer
    }
}

// The fields of a layer the user confirmed, as each feature is read with them (empty text for a field not chosen).
struct Columns {
    adt: String,
    date: String,
    peak: String,
    peak_time: String,
    hcv: String,
    name: String,
    dir: String,
    years: Vec<String>,
}

impl Columns {
    fn of(map: &CouncilMap) -> Columns {
        Columns {
            adt: map_str(map, "adt"),
            date: map_str(map, "date"),
            peak: map_str(map, "peak"),
            peak_time: map_str(map, "peakTime"),
            hcv: map_str(map, "hcv"),
            name: map_str(map, "name"),
            dir: map_str(map, "dir"),
            years: match map.get("years") {
                Some(JsonValue::Array(arr)) => arr.iter().map(JsonValue::js_string).collect(),
                _ => Vec::new(),
            },
        }
    }
}

// What `CouncilImport::finish` reads each feature with.
struct FeatureReader<'a> {
    cols: Columns,
    layer: &'a CouncilLayer,
    key: &'a str,
    label: &'a str,
    now_ms: f64,
    y_now: f64,
    max_age: Option<f64>,
    dir_re: Regex,
    digits_re: Regex,
}

impl FeatureReader<'_> {
    // The count of one feature, or None when it has no point or no volume or is older than the preset allows;
    // `before` is how many counts were kept before it.
    fn count(&self, feat: &JsonValue, before: usize) -> Option<Count> {
        let empty = JsonValue::object();
        let attrs = feat.get("attributes").filter(|v| v.truthy()).unwrap_or(&empty);
        let geom = feat.get("geometry").filter(|v| v.truthy())?;
        let ll = feature_point(geom)?;
        let at = |field: &str| attrs.get(field);
        let num = |field: &str| at(field).map_or(f64::NAN, JsonValue::to_number);
        let cols = &self.cols;
        let oid = at("OBJECTID").filter(|v| !v.is_null());
        let mut adt: Option<f64> = if cols.adt.is_empty() { None } else { Some(num(&cols.adt)) };
        let mut year: Option<f64> = if cols.date.is_empty() { None } else { year_of_val(at(&cols.date)) };
        if !adt.is_some_and(|v| v > 0.0) && !cols.years.is_empty() {
            for field in &cols.years {
                let vol = num(field);
                if vol > 0.0 {
                    adt = Some(vol);
                    year = year_of_val(Some(&JsonValue::String(field.clone())));
                    break;
                }
            }
        }
        let adt = adt.filter(|v| *v > 0.0)?;
        if let Some(ma) = self.max_age
            && let Some(yr) = year.filter(|v| *v != 0.0)
            && self.y_now - yr > ma
        {
            return None;
        }
        let dir: Option<String> = if cols.dir.is_empty() {
            None
        } else {
            let text = match at(&cols.dir) {
                Some(v) if v.truthy() => v.js_string(),
                _ => String::new(),
            }
            .to_uppercase();
            let units: Vec<u16> = text.encode_utf16().collect();
            self.dir_re.exec_units(&units, 0).and_then(|found| found.text(&units, 1)).filter(|txt| !txt.is_empty())
        };
        let peak: Option<f64> = if cols.peak.is_empty() {
            None
        } else {
            let vol = num(&cols.peak);
            if vol == 0.0 || vol.is_nan() { None } else { Some(vol) }
        };
        let hour = if cols.peak_time.is_empty() { None } else { peak_hour(at(&cols.peak_time), &self.digits_re) };
        let oid_text = oid.map_or_else(|| before.to_string(), JsonValue::js_string);
        let site_no = match oid {
            Some(v) => v.js_string(),
            None => number_to_js_string(usize_to_f64(before) + 1.0),
        };
        let name = match at(&cols.name).filter(|_| !cols.name.is_empty()).filter(|v| v.truthy()) {
            Some(v) => v.js_string(),
            None => format!("{} site {}", self.layer.name, site_no),
        };
        let hv = if !cols.hcv.is_empty() && at(&cols.hcv).is_some_and(|v| !v.is_null()) && num(&cols.hcv).is_finite() {
            json_number(num(&cols.hcv))
        } else {
            Value::Null
        };
        let yr_text = year.filter(|v| *v != 0.0).map(number_to_js_string);
        let am = peak.filter(|_| hour.is_some_and(|hr| (6.0..10.0).contains(&hr)));
        let pm = peak.filter(|_| hour.is_some_and(|hr| (14.5..19.0).contains(&hr)));
        let mut obj = Map::new();
        obj.insert("id".into(), json!(format!("feeds:{}:{oid_text}", self.key)));
        obj.insert("src".into(), json!("user"));
        obj.insert("name".into(), json!(name));
        obj.insert("lat".into(), json_number(r7(ll[0])));
        obj.insert("lng".into(), json_number(r7(ll[1])));
        obj.insert("adt".into(), json_number(math_round(adt)));
        obj.insert("hv".into(), hv);
        obj.insert("dir".into(), dir.as_ref().map_or(Value::Null, |txt| json!(txt)));
        obj.insert("directional".into(), json!(dir.is_some()));
        obj.insert("date".into(), yr_text.as_ref().map_or(Value::Null, |txt| json!(txt)));
        obj.insert("am".into(), am.map_or(Value::Null, json_number));
        obj.insert("pm".into(), pm.map_or(Value::Null, json_number));
        obj.insert("note".into(), json!(format!("{}{}", self.label, yr_text.as_ref().map_or(String::new(), |txt| format!(", {txt}")))));
        obj.insert("obsMethod".into(), json!("council count"));
        obj.insert("feed".into(), json!({ "source": self.label, "url": self.layer.url, "date": today(self.now_ms), "licence": SRC_COUNCIL.licence }));
        // Every field above has the type a Count reads, so this does not fail
        serde_json::from_value::<Count>(Value::Object(obj)).ok()
    }
}

// The point of a feature: the point itself, the middle of a line's first path, or the first of several points.
fn feature_point(geom: &JsonValue) -> Option<[f64; 2]> {
    let num = |v: Option<&JsonValue>| v.map_or(f64::NAN, JsonValue::to_number);
    if geom.get("x").is_some_and(|v| !v.is_null()) {
        Some([num(geom.get("y")), num(geom.get("x"))])
    } else if let Some(JsonValue::Array(paths)) = geom.get("paths").filter(|v| v.truthy()) {
        match paths.first() {
            Some(JsonValue::Array(p0)) => {
                let pa: Vec<[f64; 2]> = p0
                    .iter()
                    .map(|q| match q {
                        JsonValue::Array(c) => [num(c.get(1)), num(c.first())],
                        _ => [f64::NAN, f64::NAN],
                    })
                    .collect();
                let cum = util::cum_len(&pa);
                // cum_len gives a length for every point; for none, cum[cum.length - 1] is undefined (NaN)
                Some(util::along(&pa, &cum, cum.last().map_or(f64::NAN, |len| len / 2.0)).p)
            }
            _ => None,
        }
    } else if let Some(JsonValue::Array(pts)) = geom.get("points").filter(|v| v.truthy()) {
        match pts.first() {
            Some(JsonValue::Array(c)) => Some([num(c.get(1)), num(c.first())]),
            _ => None,
        }
    } else {
        None
    }
}

// The peak-hour time as hours: the digits of the text padded to four in front ("730" is 7.5), None for no digits.
fn peak_hour(v: Option<&JsonValue>, digits_re: &Regex) -> Option<f64> {
    let text = match v {
        Some(v) if !v.is_null() => v.js_string(),
        _ => String::new(),
    };
    let digits = digits_re.replace(&text, |_, _| String::new());
    let len = digits.encode_utf16().count();
    let pt = if len >= 4 { digits } else { format!("{}{}", "0".repeat(4_usize.saturating_sub(len)), digits) };
    if pt.is_empty() {
        return None;
    }
    let units: Vec<u16> = pt.encode_utf16().collect();
    let part = |from: usize, to: usize| {
        crate::jsnum::string_to_number(&String::from_utf16_lossy(units.get(from.min(units.len())..to.min(units.len())).unwrap_or(&[])))
    };
    Some(part(0, 2) + part(2, 4) / 60.0)
}

// Puts a layer's counts in the project's data in place of the ones it gave before.
fn replace_counts(p: &mut Project, url: &str, list: &[Count]) {
    crate::datahub::ensure_data(p);
    // ensure_data has just set the data record
    let Some(data) = p.data.get_mut() else { return };
    let old: Vec<Value> = match data.extra.get("counts") {
        Some(Value::Array(arr)) => arr.clone(),
        _ => Vec::new(),
    };
    let mut counts: Vec<Value> = old.into_iter().filter(|c| c.get("feed").and_then(|f| f.get("url")).and_then(Value::as_str) != Some(url)).collect();
    // A Count always writes as JSON
    counts.extend(list.iter().filter_map(|c| serde_json::to_value(c).ok()));
    data.extra.insert("counts".to_string(), Value::Array(counts));
}

// The fields used, and what was left out, as the prototype words them.
fn fields_note(map: &CouncilMap, max_age: Option<f64>, complete: bool) -> String {
    let mut used: Vec<String> = Vec::new();
    if let Some(obj) = map.as_object() {
        for (k, v) in obj.iter() {
            let items: Vec<String> = match v {
                JsonValue::Array(arr) if !arr.is_empty() => arr.iter().take(3).map(JsonValue::js_string).collect(),
                JsonValue::String(txt) if !txt.is_empty() => vec![txt.clone()],
                _ => continue,
            };
            used.push(format!("{} = {}", k, items.join(", ")));
        }
    }
    format!(
        "Fields used: {}.{}{}",
        used.join("; "),
        match max_age {
            Some(age) => format!(" Counts older than {} years left out.", number_to_js_string(age)),
            None => String::new(),
        },
        if complete { "" } else { " Not every page downloaded." }
    )
}

impl Flow for CouncilImport {
    fn next_request(&mut self) -> Option<HttpRequest> {
        self.query.next_request()
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        self.query.answer(a)
    }
    fn is_done(&self) -> bool {
        self.query.is_done()
    }
}

/// What `councilImport` reports.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CouncilImported {
    /// Counts imported.
    pub n: usize,
    /// Features the layer sent.
    #[serde(rename = "of")]
    pub of_: usize,
    /// Every page was downloaded.
    pub complete: bool,
}
