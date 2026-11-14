//! Auckland Council AGS23 v1.1 growth (section 4 of feeds.js): household, job and population growth by MSM zone for forecast years.
//!
//! Fetched live for the study area and never saved with the project. A factor for an existing-land-use zone is the growth of
//! the MSM zone it sits in, and the growth beyond each gateway is the households plus jobs of the zones outside the study
//! area, within 10 km of the gateway and within 60 degrees of the direction from the study area's centre to it.

use super::arcgis::{Bounds, FetchFailure, HttpRequest};
use super::flow::{env_params, resolve_fields, Answer, ArcQuery, Flow, LayerMeta, MetaCache};
use super::geo::{bounds_of_box, box_of_bounds, centroid, in_box, in_rings, net_box, pad_box, rings_box, rings_of, Ring, RingsBox};
use super::nztime::today;
use super::sources::{AGS_GATEWAY_DEG, AGS_GATEWAY_KM, AGS_GROUPS, AGS_YEARS, SRC_AGS};
use super::vintage::{feeds_of, stamp};
use crate::edit::json::JsonValue;
use crate::jsnum::{json_number, math_round, to_fixed, value_to_number, JsNum, Maybe, Num};
use crate::project::{land_use, Project, Scenario, Zone};
use crate::standards::year_of;
use crate::util::{ang_diff, bearing, clamp, dist, fmt, LatLng};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// The land-use growth basis text kept on every zone Spare Construct grows (`agsBasisText`).
pub const AGS_BASIS_TEXT: &str = "Land-use growth: AGS23 v1.1 by MSM zone, factor method";

/// `agsYear(y)`: a year clamped to the years AGS23 covers (2022 to 2052); anything that is not a number reads as 2023.
#[must_use]
pub fn ags_year(y: f64) -> f64 {
    let t = if y == 0.0 || y.is_nan() { 2023.0 } else { y };
    clamp(math_round(t), AGS_YEARS[0], AGS_YEARS[1])
}

/// One year's households, jobs and population in a zone.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct AgsYear {
    /// Households.
    pub hh: Option<f64>,
    /// Jobs (employment).
    pub emp: Option<f64>,
    /// Population.
    pub pop: Option<f64>,
}

/// One AGS23 MSM zone: its outline and its households, jobs and population by year.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgsZone {
    /// The MSM zone number.
    pub msm: String,
    /// The local board.
    pub board: String,
    /// The zone's rings, [lng, lat] points.
    pub rings: Vec<Ring>,
    /// The box around the rings.
    pub bb: RingsBox,
    /// The centre as [lat, lng].
    pub c: LatLng,
    /// Values by year.
    pub v: BTreeMap<String, AgsYear>,
}

/// The downloaded zones (`mem.ags`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgsData {
    /// The zones.
    pub zones: Vec<AgsZone>,
    /// The years loaded.
    pub years: Vec<f64>,
}

fn get3(a: &JsonValue, k: &str, y: f64) -> Option<f64> {
    let want = format!("{}_{}", k, crate::jsnum::number_to_js_string(y)).to_lowercase();
    match a {
        JsonValue::Object(o) => o.keys().find(|x| x.to_lowercase() == want).map(|f| o.get(f).map_or(f64::NAN, JsonValue::to_number)),
        _ => None,
    }
}

/// `loadAGS(b, years, base)` after the download: the zones that have outlines, with the values for the years asked.
#[must_use]
pub fn parse_ags(features: &[JsonValue], ys: &[f64]) -> Vec<AgsZone> {
    let empty = JsonValue::object();
    features
        .iter()
        .filter(|x| x.get("geometry").and_then(|g| g.get("rings")).is_some_and(JsonValue::truthy))
        .map(|x| {
            let a = x.get("attributes").filter(|a| a.truthy()).unwrap_or(&empty);
            let mut v = BTreeMap::new();
            for &y in ys {
                v.insert(
                    crate::jsnum::number_to_js_string(y),
                    AgsYear { hh: get3(a, "HH", y), emp: get3(a, "EMP", y), pop: get3(a, "POP", y) },
                );
            }
            let rings = rings_of(x.get("geometry").and_then(|g| g.get("rings")));
            let msm = ["MSM", "msm", "OBJECTID"].iter().find_map(|k| a.get(k).filter(|v| !v.is_null())).map_or_else(|| "undefined".to_string(), JsonValue::js_string);
            AgsZone {
                msm,
                board: match a.get("LocalBoard") {
                    Some(b) if b.truthy() => b.js_string(),
                    _ => String::new(),
                },
                bb: rings_box(&rings),
                c: centroid(&rings),
                rings,
                v,
            }
        })
        .collect()
}

/// The years `loadAGS` asks for: the base year and every forecast year, clamped and without repeats.
#[must_use]
pub fn ags_years(years: &[f64], base: f64) -> Vec<f64> {
    let mut ys = vec![ags_year(base)];
    for y in years {
        let k = ags_year(*y);
        if !ys.contains(&k) {
            ys.push(k);
        }
    }
    ys
}

/// `loadAGS(b, years, base)`: the download of AGS23 zones for the study area padded by 10 km: the layer's description (optional), then every zone.
pub struct AgsLoad {
    meta: LayerMeta,
    bounds: Bounds,
    ys: Vec<f64>,
    timeout_ms: f64,
    query: Option<ArcQuery>,
}

impl AgsLoad {
    /// A download for an area, the forecast years (`years`) and the base year.
    #[must_use]
    pub fn new(bounds: Bounds, years: &[f64], base: f64, timeout_ms: f64, cache: &MetaCache) -> AgsLoad {
        AgsLoad { meta: LayerMeta::new(SRC_AGS.url, cache, timeout_ms), bounds, ys: ags_years(years, base), timeout_ms, query: None }
    }
    fn start_query(&mut self) {
        let mut want: Vec<String> = vec!["OBJECTID".into(), "MSM".into(), "LocalBoard".into()];
        for y in &self.ys {
            let t = crate::jsnum::number_to_js_string(*y);
            want.extend([format!("HH_{t}"), format!("EMP_{t}"), format!("POP_{t}")]);
        }
        let want_refs: Vec<&str> = want.iter().map(String::as_str).collect();
        let fields = resolve_fields(self.meta.meta(), &want_refs);
        let padded = bounds_of_box(&pad_box(&box_of_bounds(&self.bounds), AGS_GATEWAY_KM * 1000.0));
        let mut p: Vec<(String, String)> = vec![("where".into(), "1=1".into()), ("outFields".into(), fields.join(","))];
        p.extend(env_params(&padded));
        p.push(("orderByFields".into(), "OBJECTID".into()));
        self.query = Some(ArcQuery::new(SRC_AGS.url, p, 40, self.timeout_ms));
    }
    /// The zones, once the download is done.
    pub fn finish(self) -> AgsData {
        let feats = self.query.map(ArcQuery::into_features).unwrap_or_default();
        AgsData { zones: parse_ags(&feats, &self.ys), years: self.ys }
    }
}

impl Flow for AgsLoad {
    fn next_request(&mut self) -> Option<HttpRequest> {
        if self.query.is_none() {
            if let Some(r) = self.meta.next_request() {
                return Some(r);
            }
            self.start_query();
        }
        self.query.as_mut().and_then(Flow::next_request)
    }
    fn answer(&mut self, a: Result<Answer, FetchFailure>) -> Result<(), String> {
        if let Some(q) = self.query.as_mut() {
            return q.answer(a);
        }
        let _ = self.meta.answer(a);
        self.start_query();
        Ok(())
    }
    fn is_done(&self) -> bool {
        self.query.as_ref().is_some_and(Flow::is_done)
    }
}

/// After `loadAGS`: stamps the vintage table (the zones are not saved with the project).
pub fn apply_ags_load(p: &mut Project, data: &AgsData, now_ms: f64) {
    let mut extra = Map::new();
    extra.insert("n".into(), json!(data.zones.len()));
    extra.insert("note".into(), json!(format!("{}; factor method by MSM zone. Not saved with the project.", SRC_AGS.attribution.unwrap_or(""))));
    stamp(p, "ags", &extra, now_ms);
}

/// The record the Get growth factors action keeps in `P.feeds.ags`: when, how many zones, the base year and how many gateways got growth.
pub fn record_ags(p: &mut Project, zones: usize, gateways: usize, now_ms: f64) {
    let base = p.settings.base_year.clone();
    feeds_of(p).insert(
        "ags".to_string(),
        json!({ "at": crate::edit::ids::iso_string(now_ms), "zones": zones, "base": base.get().map_or(Value::Null, JsNum::to_value), "gateways": gateways }),
    );
}

fn ratio(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(a), Some(b)) if a > 0.0 => Some(b / a),
        _ => None,
    }
}

/// How AGS23 grows one zone: the factors for households, jobs and population from the base year to the year.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgsFactor {
    /// The MSM zone the point sits in.
    pub msm: String,
    /// The local board.
    pub board: String,
    /// The base year used (clamped to 2022 to 2052).
    pub base: f64,
    /// The forecast year used (clamped).
    pub year: f64,
    /// Household growth factor.
    pub hh: Option<f64>,
    /// Job growth factor.
    pub emp: Option<f64>,
    /// Population growth factor.
    pub pop: Option<f64>,
}

/// `agsRangeText(base, year, f)`: the sentence that says a year outside 2022 to 2052 used the nearest AGS23 year.
#[must_use]
pub fn ags_range_text(base: &str, year: &str, f_base: f64, f_year: f64) -> String {
    format!(
        "AGS23 covers {}\u{2013}{}, so this uses growth from {} to {} (asked: {} to {})",
        crate::jsnum::number_to_js_string(AGS_YEARS[0]),
        crate::jsnum::number_to_js_string(AGS_YEARS[1]),
        crate::jsnum::number_to_js_string(f_base),
        crate::jsnum::number_to_js_string(f_year),
        base,
        year
    )
}

/// `agsZoneFactor(ll, base, year, data)`: the factors of the MSM zone that holds a point ([lat, lng]); None outside every zone or without values for the years.
#[must_use]
pub fn ags_zone_factor(ll: LatLng, base: f64, year: f64, data: Option<&AgsData>) -> Option<AgsFactor> {
    let data = data?;
    let (b, y) = (ags_year(base), ags_year(year));
    let z = data.zones.iter().find(|m| ll[1] >= m.bb[0] && ll[1] <= m.bb[2] && ll[0] >= m.bb[1] && ll[0] <= m.bb[3] && in_rings(ll[1], ll[0], &m.rings))?;
    let (vb, vy) = (z.v.get(&crate::jsnum::number_to_js_string(b))?, z.v.get(&crate::jsnum::number_to_js_string(y))?);
    Some(AgsFactor {
        msm: z.msm.clone(),
        board: z.board.clone(),
        base: b,
        year: y,
        hh: ratio(vb.hh, vy.hh),
        emp: ratio(vb.emp, vy.emp),
        pop: ratio(vb.pop, vy.pop),
    })
}

/// `existingZone(z)`: a zone that stands for existing land use.
#[must_use]
pub fn existing_zone(z: &Zone) -> bool {
    z.kind.is("zone") && (z.existing.is_true() || z.src.get().is_some_and(|s| s.as_str() == Some("existing-lu")))
}

/// A proposed growth of one existing-land-use zone in a forecast scenario.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgsRow {
    /// Row id, "<scenario>|<zone>".
    pub id: String,
    /// Scenario id.
    pub sc: String,
    /// Scenario name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sc_name: Option<String>,
    /// Zone id.
    pub zone: String,
    /// Zone name, or "Zone".
    pub name: String,
    /// The scenario's year.
    pub year: f64,
    /// The factors (null outside AGS23).
    pub f: Option<AgsFactor>,
    /// Land uses that change, each [from, to].
    pub lu: BTreeMap<String, (Value, f64)>,
    /// The user changed this zone's land use after a factor was applied.
    pub user: bool,
    /// The zone sits outside every AGS23 zone.
    pub outside: bool,
    /// A sentence when the year or base year was outside 2022 to 2052.
    pub clamped: String,
}

fn num_text(n: &Num) -> String {
    n.js_string()
}

// a === b for a settings value and a number
fn strict_eq(n: &Num, x: f64) -> bool {
    matches!(n, Maybe::Set(JsNum::Number(a)) if *a == x)
}

/// `agsProposals(P, data)`: growth for every existing-land-use zone of every forecast-year scenario (those not in the base year).
pub fn ags_proposals(p: &Project, data: Option<&AgsData>) -> Vec<AgsRow> {
    let settings = &p.settings;
    let mut rows = Vec::new();
    for sc in &p.scenarios {
        let yr = year_of(Some(sc), settings);
        if strict_eq(&settings.base_year, yr) {
            continue;
        }
        for zone in sc.zones.values() {
            if !existing_zone(zone) {
                continue;
            }
            let factor = ags_zone_factor([zone.lat.num(), zone.lng.num()], settings.base_year.num(), yr, data);
            // the base land use: luBase, else lu
            let base: Vec<(String, Value)> = match zone.extra.get("luBase") {
                Some(Value::Object(o)) => o.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                _ => zone.lu.get().map(|l| l.iter().map(|(k, v)| (k.to_string(), v.get().map_or(Value::Null, JsNum::to_value))).collect()).unwrap_or_default(),
            };
            let current = |k: &str| zone.lu.get().and_then(|l| l.get(k)).map_or(Value::Null, |v| v.get().map_or(Value::Null, JsNum::to_value));
            let mut lu: BTreeMap<String, (Value, f64)> = BTreeMap::new();
            if let Some(fac) = &factor {
                for (k, qty) in &base {
                    let group = land_use(k).map(|l| l.group).and_then(|grp| AGS_GROUPS.iter().find(|(gk, _)| *gk == grp)).map_or("emp", |(_, v)| v);
                    let mult = match group {
                        "hh" => fac.hh,
                        "pop" => fac.pop,
                        _ => fac.emp,
                    };
                    let qn = value_to_number(qty);
                    if let Some(mult) = mult
                        && qn != 0.0
                        && !qn.is_nan()
                    {
                        lu.insert(k.clone(), (current(k), math_round(qn * mult * 10.0) / 10.0));
                    }
                }
            }
            // A land use the user edited after Spare Construct applied a factor is the user's
            let user = match zone.extra.get("ags").and_then(|rec| rec.get("lu")) {
                Some(Value::Object(o)) => o.iter().any(|(k, qty)| {
                    let have = zone.lu.get().and_then(|l| l.get(k)).is_some();
                    !(have && JsonValue::from_value(&current(k)).strict_eq(&JsonValue::from_value(qty)))
                }),
                _ => false,
            };
            // AGS23 runs 2022 to 2052: a year outside that uses the nearest AGS23 year, and the row says so
            let clamped = match &factor {
                Some(fac) if fac.year != yr || !strict_eq(&settings.base_year, fac.base) => {
                    ags_range_text(&num_text(&settings.base_year), &crate::jsnum::number_to_js_string(yr), fac.base, fac.year)
                }
                _ => String::new(),
            };
            rows.push(AgsRow {
                id: format!("{}|{}", sc.id, zone.id),
                sc: sc.id.clone(),
                sc_name: sc.name.as_str().map(str::to_string),
                zone: zone.id.clone(),
                name: zone.name.str_or("Zone").to_string(),
                year: yr,
                outside: factor.is_none(),
                f: factor,
                lu,
                user,
                clamped,
            });
        }
    }
    rows
}

/// `applyAGS(P, rows, ids, when)`: grows the chosen zones' land use (keeping the base-year land use in `luBase`) and records the factors in `zone.ags`; gives how many zones changed.
pub fn apply_ags(p: &mut Project, rows: &[AgsRow], pick: &[String], when: &str) -> usize {
    let mut changed: usize = 0;
    for row in rows {
        if !pick.contains(&row.id) {
            continue;
        }
        let Some(fac) = &row.f else { continue };
        let Some(sc) = p.scenarios.iter_mut().find(|s| s.id == row.sc) else { continue };
        let Some(zone) = sc.zones.get_mut(&row.zone) else { continue };
        if !zone.extra.get("luBase").is_some_and(crate::jsnum::value_truthy) {
            let lu_now: Map<String, Value> = zone.lu.get().map(|l| l.iter().map(|(k, v)| (k.to_string(), v.get().map_or(Value::Null, JsNum::to_value))).collect()).unwrap_or_default();
            zone.extra.insert("luBase".to_string(), Value::Object(lu_now));
        }
        // A new object, never an edit in place
        let mut lu = zone.lu.get().cloned().unwrap_or_default();
        for (k, (_, to)) in &row.lu {
            lu.insert(k.clone(), Maybe::number(*to));
        }
        zone.lu = Maybe::Set(lu);
        let mut rec = Map::new();
        rec.insert("msm".into(), json!(fac.msm));
        rec.insert("base".into(), json_number(fac.base));
        rec.insert("year".into(), json_number(fac.year));
        rec.insert("f".into(), json!({ "hh": fac.hh, "emp": fac.emp, "pop": fac.pop }));
        rec.insert("lu".into(), Value::Object(row.lu.iter().map(|(k, (_, to))| (k.clone(), json_number(*to))).collect()));
        rec.insert("at".into(), json!(when));
        rec.insert("src".into(), json!(AGS_BASIS_TEXT));
        if !row.clamped.is_empty() {
            rec.insert("note".into(), json!(row.clamped));
        }
        zone.extra.insert("ags".to_string(), Value::Object(rec));
        // Bound: at most one for each row
        changed = changed.saturating_add(1);
    }
    changed
}

/// What `agsGateway` finds: the growth beyond a gateway as an equivalent linear percentage a year.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgsGateway {
    /// Base year used.
    pub base: f64,
    /// Forecast year used.
    pub year: f64,
    /// Growth factor (null with no zones beyond the gateway).
    pub factor: Option<f64>,
    /// Linear growth, percent a year.
    pub pct: Option<f64>,
    /// MSM zones counted.
    pub n: usize,
    /// The first 20 zone numbers.
    pub msm: Vec<String>,
    /// What the figure is.
    pub basis: String,
}

/// `agsGateway(sc, z, base, year, data)`: households plus jobs of MSM zones whose centre lies outside the study area, within 10 km of the gateway and within 60 degrees of the direction from the study-area centre to the gateway.
///
/// `base` and `year` are the settings' values as given (the sentence quotes them when they were clamped). None without data, a network, or with the same year twice.
#[must_use]
pub fn ags_gateway(sc: &Scenario, z: &Zone, base: &Num, year: f64, data: Option<&AgsData>) -> Option<AgsGateway> {
    let data = data?;
    let (yb, yf) = (ags_year(base.num()), ags_year(year));
    let area = net_box(sc)?;
    if yf == yb {
        return None;
    }
    let centre: LatLng = [f64::midpoint(area.s, area.n), f64::midpoint(area.w, area.e)];
    let gate: LatLng = [z.lat.num(), z.lng.num()];
    let dir = bearing(&centre, &gate);
    let (mut b0, mut b1) = (0.0, 0.0);
    let mut used: Vec<String> = Vec::new();
    let (kb, ky) = (crate::jsnum::number_to_js_string(yb), crate::jsnum::number_to_js_string(yf));
    for mz in &data.zones {
        if in_box(&area, &mz.c) || dist(&gate, &mz.c) > AGS_GATEWAY_KM * 1000.0 || ang_diff(bearing(&centre, &mz.c), dir) > AGS_GATEWAY_DEG {
            continue;
        }
        let (Some(v0), Some(v1)) = (mz.v.get(&kb), mz.v.get(&ky)) else { continue };
        let or0 = |val: Option<f64>| match val {
            Some(v) if v != 0.0 && !v.is_nan() => v,
            _ => 0.0,
        };
        let x0 = or0(v0.hh) + or0(v0.emp);
        let x1 = or0(v1.hh) + or0(v1.emp);
        if x0 == 0.0 || x0.is_nan() {
            continue;
        }
        b0 += x0;
        b1 += x1;
        used.push(mz.msm.clone());
    }
    if b0 == 0.0 {
        return Some(AgsGateway {
            base: yb,
            year: yf,
            factor: None,
            pct: None,
            n: 0,
            msm: Vec::new(),
            basis: format!("No AGS23 zones lie beyond this gateway within {} km.", crate::jsnum::number_to_js_string(AGS_GATEWAY_KM)),
        });
    }
    let factor = b1 / b0;
    let pct = (factor - 1.0) / (yf - yb) * 100.0;
    let cl = if yf != year || yb != base.num() {
        format!(" {}.", ags_range_text(&num_text(base), &crate::jsnum::number_to_js_string(year), yb, yf))
    } else {
        String::new()
    };
    Some(AgsGateway {
        base: yb,
        year: yf,
        factor: Some(factor),
        pct: Some(pct),
        n: used.len(),
        msm: used.iter().take(20).cloned().collect(),
        basis: format!(
            "Households + jobs in {} MSM zone{} beyond the gateway (within {} km, \u{b1}{}\u{b0}): {} in {} \u{2192} {} in {}, \u{d7}{} = {}% a year, linear.{}",
            used.len(),
            if used.len() == 1 { "" } else { "s" },
            crate::jsnum::number_to_js_string(AGS_GATEWAY_KM),
            crate::jsnum::number_to_js_string(AGS_GATEWAY_DEG),
            fmt::int(b0),
            kb,
            fmt::int(b1),
            ky,
            to_fixed(factor, 3),
            to_fixed(pct, 2),
            cl
        ),
    })
}

/// One gateway's growth, as `agsGateways` lists it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgsGatewayRow {
    /// Scenario id.
    pub sc: String,
    /// Gateway zone id.
    pub zone: String,
    /// Gateway name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The growth.
    #[serde(flatten)]
    pub g: AgsGateway,
}

/// `agsGateways(P, data)`: the growth beyond every gateway of every scenario, also kept on each gateway zone as `agsGrowth` (with the date and the AGS23 attribution).
pub fn ags_gateways(p: &mut Project, data: Option<&AgsData>, now_ms: f64) -> Vec<AgsGatewayRow> {
    let settings = p.settings.clone();
    let day_text = today(now_ms);
    let mut out = Vec::new();
    for sc in &mut p.scenarios {
        let yr = year_of(Some(sc), &settings);
        let ids: Vec<String> = sc.zones.values().filter(|z| z.kind.is("gateway")).map(|z| z.id.clone()).collect();
        for id in ids {
            let Some(zone) = sc.zones.get(id.as_str()) else {
                continue;
            };
            let growth = ags_gateway(sc, zone, &settings.base_year, yr, data);
            if let Some(growth) = growth {
                // The id was read from this scenario's zones just above
                let Some(zone) = sc.zones.get_mut(id.as_str()) else { continue };
                // AgsGateway holds only numbers, text and lists, which always write as JSON
                let mut rec = serde_json::to_value(&growth).unwrap_or_default();
                if let Value::Object(obj) = &mut rec {
                    obj.insert("at".into(), json!(day_text));
                    obj.insert("src".into(), json!(SRC_AGS.attribution.unwrap_or("")));
                }
                zone.extra.insert("agsGrowth".to_string(), rec);
                out.push(AgsGatewayRow { sc: sc.id.clone(), zone: id, name: zone.name.as_str().map(str::to_string), g: growth });
            }
        }
    }
    out
}
