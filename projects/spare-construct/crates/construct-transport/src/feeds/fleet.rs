//! Fleet mix from the NZTA Motor Vehicle Register (section 8 of feeds.js): passenger cars and vans of a district by motive power.
//!
//! The register's newest monthly layer (`MVR_Oct26` and so on) is found in the NZTA services list, and a statistics query
//! counts the district's cars and vans by motive power (about 20 s). The result can set the CO2 factor of the model:
//! the share of registered cars and vans that are not battery electric (tailpipe CO2 only; hybrids count as fuel-burning).

use super::arcgis::{FetchFailure, HttpRequest};
use super::flow::{get_error, resolve_fields, Answer, ArcQuery, Flow, LayerMeta, MetaCache};
use super::jsregex::Regex;
use super::sources::{FLEET_DAYS, FLEET_TIMEOUT, NZTA};
use super::vintage::{feeds_of, stamp};
use crate::edit::ids::iso_string;
use crate::edit::json::JsonValue;
use crate::graph::js_sort_by;
use crate::jsnum::{json_number, math_max, Maybe};
use crate::project::Project;
use crate::util::{clamp, fmt};
use serde_json::{json, Map, Value};

/// `fleetCacheKey(district)`: the key a host keeps a district's result under ("corridor.feeds.fleet.AUCKLAND").
pub fn fleet_cache_key(district: &str) -> String {
    format!("corridor.feeds.fleet.{district}")
}

/// The district as the register spells it: trimmed, upper case, with quotes taken out.
pub fn fleet_district(text: &str) -> Result<String, String> {
    let d = crate::jsnum::js_trim(text).to_uppercase().replace('\'', "");
    if d.is_empty() {
        return Err("Type a district name as the register spells it, such as AUCKLAND or HAMILTON CITY.".to_string());
    }
    Ok(d)
}

/// A district's fleet by motive power.
#[derive(Clone, Debug, PartialEq)]
pub struct Fleet {
    /// The district as the register spells it.
    pub district: String,
    /// The register layer used ("MVR_Oct26").
    pub service: String,
    /// The layer's address.
    pub url: String,
    /// When it was asked (ISO).
    pub at: String,
    /// Passenger cars and vans in the district.
    pub total: f64,
    /// Each motive power's share of the total, biggest first.
    pub shares: Vec<(String, f64)>,
}

impl Fleet {
    /// The fleet as the prototype saves it. The shares are an object; the project file's objects are sorted by key, so the order biggest first is not kept there.
    pub fn to_value(&self) -> Value {
        json!({
            "district": self.district, "service": self.service, "url": self.url, "at": self.at, "total": json_number(self.total),
            "shares": self.shares.iter().map(|(k, v)| (k.clone(), json_number(*v))).collect::<Map<String, Value>>(),
        })
    }
}

const MON: [(&str, f64); 12] = [("Jan", 1.0), ("Feb", 2.0), ("Mar", 3.0), ("Apr", 4.0), ("May", 5.0), ("Jun", 6.0), ("Jul", 7.0), ("Aug", 8.0), ("Sep", 9.0), ("Oct", 10.0), ("Nov", 11.0), ("Dec", 12.0)];

/// The newest register layer in the NZTA services list: the name ("MVR_Oct26") with a monthly number to compare.
pub fn newest_mvr(list: &JsonValue) -> Option<String> {
    let re = Regex::lit(r"^MVR_([A-Z][a-z]{2})(\d{2})$", "");
    let services = match list.get("services") {
        Some(JsonValue::Array(a)) => a.as_slice(),
        _ => &[],
    };
    let mut found: Vec<(String, f64)> = Vec::new();
    for s in services {
        let full = s.get("name").map_or_else(|| "undefined".to_string(), JsonValue::js_string);
        let n = full.rsplit('/').next().unwrap_or("").to_string();
        let u: Vec<u16> = n.encode_utf16().collect();
        if let Some(m) = re.exec_units(&u, 0) {
            let mon = m.text(&u, 1).unwrap_or_default();
            let yy: f64 = m.text(&u, 2).map_or(f64::NAN, |t| crate::jsnum::string_to_number(&t));
            if let Some((_, k)) = MON.iter().find(|(name, _)| *name == mon) {
                found.push((n, (2000.0 + yy) * 12.0 + k));
            }
        }
    }
    js_sort_by(&mut found, |p, q| q.1 - p.1);
    found.into_iter().next().map(|(n, _)| n)
}

/// What the statistics answer holds, as the fleet.
pub fn fleet_from_answer(features: &[JsonValue], district: &str, service: &str, url: &str, mp: &str, now_ms: f64) -> Result<Fleet, String> {
    let empty = JsonValue::object();
    let mut counts: Vec<(String, f64)> = Vec::new();
    let mut total = 0.0;
    for f in features {
        let a = f.get("attributes").filter(|a| a.truthy()).unwrap_or(&empty);
        let k = [a.get(mp), a.get(&mp.to_lowercase())].into_iter().flatten().find(|v| !v.is_null()).map_or_else(|| "UNKNOWN".to_string(), JsonValue::js_string);
        let num = |key: &str| a.get(key).map_or(f64::NAN, JsonValue::to_number);
        let n = {
            let x = num("n");
            if x != 0.0 && !x.is_nan() {
                x
            } else {
                let y = num("N");
                if y != 0.0 && !y.is_nan() { y } else { 0.0 }
            }
        };
        match counts.iter_mut().find(|(c, _)| *c == k) {
            Some(e) => e.1 = n,
            None => counts.push((k, n)),
        }
        total += n;
    }
    if total == 0.0 || total.is_nan() {
        return Err(format!("The register has no passenger cars for '{district}'. Check the spelling (for example AUCKLAND, HAMILTON CITY, CHRISTCHURCH CITY)."));
    }
    js_sort_by(&mut counts, |p, q| q.1 - p.1);
    Ok(Fleet {
        district: district.to_string(),
        service: service.to_string(),
        url: url.to_string(),
        at: iso_string(now_ms),
        total,
        shares: counts.into_iter().map(|(k, n)| (k, n / total)).collect(),
    })
}

enum FStage {
    List,
    Meta,
    Query,
    Done,
}

/// `loadFleet(district)`: the download: the services list (to find the newest register layer), the layer's description, then the statistics query.
pub struct FleetLoad {
    district: String,
    now_ms: f64,
    timeout_ms: f64,
    cache: MetaCache,
    stage: FStage,
    service: String,
    url: String,
    meta: Option<LayerMeta>,
    tla: String,
    vt: String,
    mp: String,
    query: Option<ArcQuery>,
    fleet: Option<Fleet>,
}

impl FleetLoad {
    /// A download for a district (as the register spells it: see [`fleet_district`]). `limit_ms` is the settings time limit; the statistics query is allowed at least 90 s.
    pub fn new(district: &str, now_ms: f64, limit_ms: f64, cache: &MetaCache) -> FleetLoad {
        FleetLoad {
            district: district.to_string(),
            now_ms,
            timeout_ms: limit_ms,
            cache: cache.clone(),
            stage: FStage::List,
            service: String::new(),
            url: String::new(),
            meta: None,
            tla: String::new(),
            vt: String::new(),
            mp: String::new(),
            query: None,
            fleet: None,
        }
    }
    /// The fleet, once the download is done.
    pub fn finish(self) -> Option<Fleet> {
        self.fleet
    }
    fn start_query(&mut self) {
        let meta = self.meta.as_ref().and_then(LayerMeta::meta);
        let pick = |w: &str| resolve_fields(meta, &[w]).into_iter().next().unwrap_or_else(|| w.to_string());
        self.tla = pick("TLA");
        self.vt = pick("VEHICLE_TYPE");
        self.mp = pick("MOTIVE_POWER");
        let oid = pick("OBJECTID");
        let params = vec![
            ("where".to_string(), format!("{}='{}' AND {}='PASSENGER CAR/VAN'", self.tla, self.district, self.vt)),
            ("groupByFieldsForStatistics".to_string(), self.mp.clone()),
            ("outStatistics".to_string(), format!("[{{\"statisticType\":\"count\",\"onStatisticField\":{},\"outStatisticFieldName\":\"n\"}}]", serde_json::to_string(&oid).expect("text"))),
        ];
        self.query = Some(ArcQuery::new(&self.url, params, 2, math_max(self.timeout_ms, FLEET_TIMEOUT * 1000.0)));
        self.stage = FStage::Query;
    }
}

impl Flow for FleetLoad {
    fn next_request(&mut self) -> Option<HttpRequest> {
        loop {
            match self.stage {
                FStage::List => return Some(HttpRequest::get_json(format!("{NZTA}?f=json"), self.timeout_ms)),
                FStage::Meta => match self.meta.as_mut().and_then(Flow::next_request) {
                    Some(r) => return Some(r),
                    None => self.start_query(),
                },
                FStage::Query => return self.query.as_mut().and_then(Flow::next_request),
                FStage::Done => return None,
            }
        }
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        match self.stage {
            FStage::List => {
                let list = match a {
                    Ok(Answer::Json(j)) => j,
                    Ok(_) => return Err(get_error(NZTA, self.timeout_ms, &FetchFailure::Unreadable("not JSON".into()))),
                    Err(f) => return Err(get_error(NZTA, self.timeout_ms, &f)),
                };
                let Some(n) = newest_mvr(&list) else {
                    return Err("The NZTA services list has no Motor Vehicle Register layer (MVR_\u{2026}) at the moment. Try again later.".to_string());
                };
                self.url = format!("{NZTA}/{n}/FeatureServer/0");
                self.service = n;
                self.meta = Some(LayerMeta::new(&self.url, &self.cache, self.timeout_ms));
                self.stage = FStage::Meta;
                if self.meta.as_ref().is_some_and(Flow::is_done) {
                    self.start_query();
                }
                Ok(())
            }
            FStage::Meta => {
                self.meta.as_mut().expect("meta").answer(a)?;
                self.start_query();
                Ok(())
            }
            FStage::Query => {
                let q = self.query.as_mut().expect("query");
                q.answer(a)?;
                if q.is_done() {
                    let q = self.query.take().expect("query");
                    let fleet = fleet_from_answer(q.features(), &self.district, &self.service, &self.url, &self.mp, self.now_ms)?;
                    self.fleet = Some(fleet);
                    self.stage = FStage::Done;
                }
                Ok(())
            }
            FStage::Done => Ok(()),
        }
    }
    fn is_done(&self) -> bool {
        matches!(self.stage, FStage::Done)
    }
}

/// After the download: keeps the fleet in `P.feeds.fleet` and stamps the vintage table.
pub fn apply_fleet(p: &mut Project, fleet: &Fleet, now_ms: f64) {
    feeds_of(p).insert("fleet".to_string(), fleet.to_value());
    let mut e = Map::new();
    e.insert("url".into(), json!(fleet.url));
    e.insert("n".into(), json_number(fleet.total));
    e.insert("note".into(), json!(format!("{}: passenger cars and vans registered in {}, by motive power.", fleet.service, fleet.district)));
    stamp(p, "mvr", &e, now_ms);
}

/// Uses a result a host kept from an earlier query (perhaps another project) when it is under 90 days old: it is put in `P.feeds.fleet` and the vintage table names it. Gives the result, or None when it is too old or not a result.
pub fn use_cached_fleet(p: &mut Project, cached: &Value, now_ms: f64) -> Option<Value> {
    let at = cached.get("at").and_then(Value::as_str)?;
    let t = super::nztime::parse_date(at)?;
    if !(now_ms - t < FLEET_DAYS * super::nztime::DAY_MS) {
        return None;
    }
    feeds_of(p).insert("fleet".to_string(), cached.clone());
    let g = |k: &str| cached.get(k).cloned().unwrap_or(Value::Null);
    let text = |k: &str| crate::jsnum::value_to_js_string(&g(k));
    let mut e = Map::new();
    e.insert("url".into(), g("url"));
    e.insert("n".into(), g("total"));
    e.insert("at".into(), g("at"));
    e.insert(
        "note".into(),
        json!(format!(
            "{}: passenger cars and vans registered in {}, by motive power. Kept from the query of {} (results are kept for {} days).",
            text("service"),
            text("district"),
            super::nztime::date_text(Some(&JsonValue::String(at.to_string()))),
            crate::jsnum::number_to_js_string(FLEET_DAYS)
        )),
    );
    stamp(p, "mvr", &e, now_ms);
    Some(cached.clone())
}

/// "Use for CO2": the factor and its source text from the fleet: the share of cars and vans that are not battery electric, to three decimals.
///
/// `shares` is the fleet's share by motive power in any order. Gives (factor, source text).
pub fn fleet_co2(district: &str, service: &str, shares: &[(String, f64)]) -> (f64, String) {
    let electric = Regex::lit("^ELECTRIC$", "i");
    let bev = shares.iter().filter(|(k, _)| electric.test(crate::jsnum::js_trim(k))).fold(0.0, |a, (_, v)| a + if *v == 0.0 || v.is_nan() { 0.0 } else { *v });
    let k = crate::jsnum::math_round(clamp(1.0 - bev, 0.0, 1.0) * 1000.0) / 1000.0;
    (k, format!("{} fleet, {}: {} battery electric (tailpipe CO\u{2082} only; hybrids counted as fuel-burning, Spare Construct assumption)", district, service, fmt::pct(bev, 1)))
}

/// Sets the model's CO2 factor and where it came from (`settings.co2Factor`, `settings.co2FactorSrc`).
pub fn apply_fleet_co2(p: &mut Project, factor: f64, src_text: &str) {
    p.settings.co2_factor = Maybe::number(factor);
    p.settings.extra.insert("co2FactorSrc".to_string(), json!(src_text));
}

/// Goes back to the generic fleet: removes the CO2 factor and its source.
pub fn clear_fleet_co2(p: &mut Project) {
    p.settings.co2_factor = Maybe::Missing;
    p.settings.extra.remove("co2FactorSrc");
}
