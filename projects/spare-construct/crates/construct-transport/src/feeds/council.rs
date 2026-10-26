//! Council count layers (section 6 of feeds.js): a generic ArcGIS importer for traffic counts.
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
use crate::jsnum::{json_number, math_round, number_to_js_string};
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
    /// Year columns (year_2023 ...), newest first.
    pub years: Vec<String>,
}

/// `guessFields(meta)`: which field of a layer holds the daily volume, peak hour and so on, from the field names and aliases.
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
    pub fn new(url: &str, timeout_ms: f64, cache: &MetaCache) -> Result<CouncilRead, String> {
        let u = council_url(url)?;
        Ok(CouncilRead { meta: LayerMeta::new(&u, cache, timeout_ms), url: u })
    }
    /// The layer, once its description was read; Err when it has no fields.
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
pub fn year_of_val(v: Option<&JsonValue>) -> Option<f64> {
    let v = v?;
    match v {
        JsonValue::Null => return None,
        JsonValue::String(s) if s.is_empty() => return None,
        JsonValue::Number(n) if *n > 1e10 => {
            let ms = super::nztime::time_clip(*n)?;
            return Some(civil_from_days((ms / DAY_MS).floor() as i64).0 as f64);
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
        let where_ = if !latest.is_empty() { format!("{latest} IN ('Y','Yes','YES','y')") } else { "1=1".to_string() };
        let want: Vec<&str> = uniq.iter().map(String::as_str).collect();
        let resolved = resolve_fields(Some(&layer.meta), &want).join(",");
        let mut params: Vec<(String, String)> = vec![("where".into(), where_), ("outFields".into(), if resolved.is_empty() { "*".to_string() } else { resolved })];
        params.extend(env_params(b));
        let query = ArcQuery::new(&layer.url, params, 40, timeout_ms);
        CouncilImport { layer, map, preset: preset.map(str::to_string), now_ms, query }
    }
    /// How many features have been downloaded (for a progress line).
    pub fn count(&self) -> usize {
        self.query.features().len()
    }
    /// The counts, after the download: replaces this layer's counts in the project, stamps the vintage table and says how many were imported.
    pub fn finish(self, p: &mut Project) -> CouncilImported {
        let feats = self.query.features().to_vec();
        let complete = self.query.complete();
        let n_features = feats.len();
        let preset = self.preset.as_deref().and_then(council);
        let y_now = super::nztime::nz_year(self.now_ms) as f64;
        let max_age = preset.and_then(|pr| pr.max_age_years).filter(|a| *a != 0.0);
        let key = hash_str(&self.layer.url);
        let label = preset.map_or_else(|| self.layer.name.clone(), |pr| pr.label.to_string());
        let (m_adt, m_date, m_peak, m_ptime, m_hcv, m_name, m_dir) =
            (map_str(&self.map, "adt"), map_str(&self.map, "date"), map_str(&self.map, "peak"), map_str(&self.map, "peakTime"), map_str(&self.map, "hcv"), map_str(&self.map, "name"), map_str(&self.map, "dir"));
        let year_cols: Vec<String> = match self.map.get("years") {
            Some(JsonValue::Array(a)) => a.iter().map(JsonValue::js_string).collect(),
            _ => Vec::new(),
        };
        let dir_re = Regex::lit(r"\b(NB|SB|EB|WB)\b", "i");
        let digits_re = Regex::lit(r"\D", "g");
        let empty = JsonValue::object();
        let mut list: Vec<Count> = Vec::new();
        for x in &feats {
            let a = x.get("attributes").filter(|a| a.truthy()).unwrap_or(&empty);
            let Some(g) = x.get("geometry").filter(|g| g.truthy()) else { continue };
            let at = |k: &str| a.get(k);
            let ll: Option<[f64; 2]> = if g.get("x").is_some_and(|v| !v.is_null()) {
                Some([g.get("y").map_or(f64::NAN, JsonValue::to_number), g.get("x").map_or(f64::NAN, JsonValue::to_number)])
            } else if let Some(JsonValue::Array(paths)) = g.get("paths").filter(|p| p.truthy()) {
                match paths.first() {
                    Some(JsonValue::Array(p0)) => {
                        let pa: Vec<[f64; 2]> = p0
                            .iter()
                            .map(|q| match q {
                                JsonValue::Array(c) => [c.get(1).map_or(f64::NAN, JsonValue::to_number), c.first().map_or(f64::NAN, JsonValue::to_number)],
                                _ => [f64::NAN, f64::NAN],
                            })
                            .collect();
                        let cum = util::cum_len(&pa);
                        Some(util::along(&pa, &cum, cum[cum.len() - 1] / 2.0).p)
                    }
                    _ => None,
                }
            } else if let Some(JsonValue::Array(pts)) = g.get("points").filter(|p| p.truthy()) {
                match pts.first() {
                    Some(JsonValue::Array(c)) => Some([c.get(1).map_or(f64::NAN, JsonValue::to_number), c.first().map_or(f64::NAN, JsonValue::to_number)]),
                    _ => None,
                }
            } else {
                None
            };
            let Some(ll) = ll else { continue };
            let oid = at("OBJECTID").filter(|v| !v.is_null());
            let mut adt: Option<f64> = if !m_adt.is_empty() { Some(at(&m_adt).map_or(f64::NAN, JsonValue::to_number)) } else { None };
            let mut year: Option<f64> = if !m_date.is_empty() { year_of_val(at(&m_date)) } else { None };
            if !adt.is_some_and(|v| v > 0.0) && !year_cols.is_empty() {
                for f in &year_cols {
                    let v = at(f).map_or(f64::NAN, JsonValue::to_number);
                    if v > 0.0 {
                        adt = Some(v);
                        year = year_of_val(Some(&JsonValue::String(f.clone())));
                        break;
                    }
                }
            }
            let Some(adt) = adt.filter(|v| *v > 0.0) else { continue };
            if let Some(ma) = max_age
                && let Some(yr) = year.filter(|y| *y != 0.0)
                && y_now - yr > ma
            {
                continue;
            }
            let dir: Option<String> = if !m_dir.is_empty() {
                let t = match at(&m_dir) {
                    Some(v) if v.truthy() => v.js_string(),
                    _ => String::new(),
                }
                .to_uppercase();
                let u: Vec<u16> = t.encode_utf16().collect();
                dir_re.exec_units(&u, 0).and_then(|m| m.text(&u, 1)).filter(|s| !s.is_empty())
            } else {
                None
            };
            let peak: Option<f64> = if !m_peak.is_empty() {
                let v = at(&m_peak).map_or(f64::NAN, JsonValue::to_number);
                if v == 0.0 || v.is_nan() { None } else { Some(v) }
            } else {
                None
            };
            let pt: String = if !m_ptime.is_empty() {
                let t = match at(&m_ptime) {
                    Some(v) if !v.is_null() => v.js_string(),
                    _ => String::new(),
                };
                let digits = digits_re.replace(&t, |_, _| String::new());
                let n = digits.encode_utf16().count();
                if n >= 4 { digits } else { format!("{}{}", "0".repeat(4 - n), digits) }
            } else {
                String::new()
            };
            let h: Option<f64> = if pt.is_empty() {
                None
            } else {
                let u: Vec<u16> = pt.encode_utf16().collect();
                let part = |a: usize, b: usize| crate::jsnum::string_to_number(&String::from_utf16_lossy(&u[a.min(u.len())..b.min(u.len())]));
                Some(part(0, 2) + part(2, 4) / 60.0)
            };
            let oid_text = oid.map_or_else(|| list.len().to_string(), JsonValue::js_string);
            let site_no = match oid {
                Some(v) => v.js_string(),
                None => number_to_js_string((list.len() + 1) as f64),
            };
            let name = match at(&m_name).filter(|_| !m_name.is_empty()).filter(|v| v.truthy()) {
                Some(v) => v.js_string(),
                None => format!("{} site {}", self.layer.name, site_no),
            };
            let hv = if !m_hcv.is_empty() && at(&m_hcv).is_some_and(|v| !v.is_null()) && at(&m_hcv).map_or(f64::NAN, JsonValue::to_number).is_finite() {
                json_number(at(&m_hcv).map_or(f64::NAN, JsonValue::to_number))
            } else {
                Value::Null
            };
            let yr_text = year.filter(|y| *y != 0.0).map(number_to_js_string);
            let in_am = peak.is_some() && h.is_some_and(|h| h >= 6.0 && h < 10.0);
            let in_pm = peak.is_some() && h.is_some_and(|h| h >= 14.5 && h < 19.0);
            let mut o = Map::new();
            o.insert("id".into(), json!(format!("feeds:{key}:{oid_text}")));
            o.insert("src".into(), json!("user"));
            o.insert("name".into(), json!(name));
            o.insert("lat".into(), json_number(r7(ll[0])));
            o.insert("lng".into(), json_number(r7(ll[1])));
            o.insert("adt".into(), json_number(math_round(adt)));
            o.insert("hv".into(), hv);
            o.insert("dir".into(), dir.as_ref().map_or(Value::Null, |d| json!(d)));
            o.insert("directional".into(), json!(dir.is_some()));
            o.insert("date".into(), yr_text.as_ref().map_or(Value::Null, |y| json!(y)));
            o.insert("am".into(), if in_am { json_number(peak.expect("peak")) } else { Value::Null });
            o.insert("pm".into(), if in_pm { json_number(peak.expect("peak")) } else { Value::Null });
            o.insert("note".into(), json!(format!("{}{}", label, yr_text.as_ref().map_or(String::new(), |y| format!(", {y}")))));
            o.insert("obsMethod".into(), json!("council count"));
            o.insert("feed".into(), json!({ "source": label, "url": self.layer.url, "date": today(self.now_ms), "licence": SRC_COUNCIL.licence }));
            list.push(serde_json::from_value::<Count>(Value::Object(o)).expect("a count reads"));
        }
        crate::datahub::ensure_data(p);
        {
            let d = p.data.get_mut().expect("data");
            let old: Vec<Value> = match d.extra.get("counts") {
                Some(Value::Array(a)) => a.clone(),
                _ => Vec::new(),
            };
            let mut counts: Vec<Value> = old.into_iter().filter(|c| c.get("feed").and_then(|f| f.get("url")).and_then(Value::as_str) != Some(self.layer.url.as_str())).collect();
            counts.extend(list.iter().map(|c| serde_json::to_value(c).expect("count")));
            d.extra.insert("counts".to_string(), Value::Array(counts));
        }
        // the fields used, as the prototype words them
        let mut used: Vec<String> = Vec::new();
        if let Some(o) = self.map.as_object() {
            for (k, v) in o.iter() {
                let items: Vec<String> = match v {
                    JsonValue::Array(a) if !a.is_empty() => a.iter().take(3).map(JsonValue::js_string).collect(),
                    JsonValue::String(s) if !s.is_empty() => vec![s.clone()],
                    _ => continue,
                };
                used.push(format!("{} = {}", k, items.join(", ")));
            }
        }
        let note = format!(
            "Fields used: {}.{}{}",
            used.join("; "),
            match max_age {
                Some(a) => format!(" Counts older than {} years left out.", number_to_js_string(a)),
                None => String::new(),
            },
            if complete { "" } else { " Not every page downloaded." }
        );
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
    pub fn layer(&self) -> &CouncilLayer {
        &self.layer
    }
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
