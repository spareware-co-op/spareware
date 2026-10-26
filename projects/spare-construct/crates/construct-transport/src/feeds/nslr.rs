//! Legal speed limits from the NZTA National Speed Limit Register (section 1 of feeds.js).
//!
//! Current versions only (a future scenario year uses 1 July of that year, so gazetted future changes apply). Each link
//! takes the modal limit of the polygons under three points along it (a quarter, a half and three quarters), the
//! version that started last where versions overlap, and a variable (school) limit where one covers the link.

use super::arcgis::Bounds;
use super::flow::{env_params, resolve_fields, Answer, ArcQuery, Flow, LayerMeta, MetaCache};
use super::geo::{live_links, rings_box, rings_of, HasBox, PolyIndex, Ring, RingsBox};
use super::jsregex::Regex;
use super::nztime::{nz_year, utc_date};
use super::sources::{src, NSLR_FUTURE_DAY, NSLR_SAMPLES, PERIOD_AM, PERIOD_PM};
use super::vintage::{feeds_of, stamp};
use super::arcgis::{FetchFailure, HttpRequest};
use crate::edit::json::JsonValue;
use crate::jsnum::{math_min_of, number_to_js_string, value_to_number, value_truthy, JsNum, Maybe};
use crate::project::{Link, Project, Scenario};
use crate::standards::year_of;
use crate::util::{self, fmt};
use serde::Serialize;
use serde_json::{json, Map, Value};

/// The fields asked of the register (`NSLR_FIELDS`).
pub const NSLR_FIELDS: [&str; 12] = [
    "OBJECTID",
    "speedLimitZoneValue",
    "speedCategoryName",
    "speedLimitZoneVariableSpeed",
    "speedLimitZoneVarPrdDesc",
    "speedLimitZoneDesc",
    "speedLimitZoneStartDate",
    "speedLimitZoneEndDate",
    "speedLimitZoneLanePurposeName",
    "rcaZoneReferenceName",
    "legalReference",
    "legalInstrumentURL",
];

fn year_text(year: f64) -> String {
    number_to_js_string(year)
}

/// `nslrWhere(year, now)`: the where clause for versions in force now, or on 1 July of a later scenario year.
pub fn nslr_where(year: Option<f64>, now_ms: f64) -> String {
    let t = match year {
        Some(y) if y != 0.0 && y > nz_year(now_ms) as f64 => format!("TIMESTAMP '{}-{} 00:00:00'", year_text(y), NSLR_FUTURE_DAY),
        _ => "CURRENT_TIMESTAMP".to_string(),
    };
    format!("(speedLimitZoneEndDate IS NULL OR speedLimitZoneEndDate > {t}) AND speedLimitZoneStartDate <= {t}")
}

/// `asOfMs(year, now)`: the instant the register is read at: 1 July of a later scenario year, else now.
pub fn as_of_ms(year: Option<f64>, now_ms: f64) -> f64 {
    match year {
        Some(y) if y != 0.0 && y > nz_year(now_ms) as f64 => super::nztime::date_utc(y, 6.0, 1.0),
        _ => now_ms,
    }
}

/// Which peak windows a variable limit's period text covers, and the times it names (`varPeriods`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VarPeriods {
    /// The text covers the AM peak window.
    #[serde(rename = "AM")]
    pub am: bool,
    /// The text covers the PM peak window.
    #[serde(rename = "PM")]
    pub pm: bool,
    /// The times named, as [start, end] hours.
    pub windows: Vec<[f64; 2]>,
}

fn unit_text(u: &[u16], g: Option<(usize, usize)>) -> Option<String> {
    g.map(|(a, b)| String::from_utf16_lossy(&u[a..b]))
}

/// `varPeriods(desc)`: the peak windows a variable (school) limit applies in: '08:30 \u{2013} 9:15am and 2:45 \u{2013} 3:30pm' gives AM true and PM false.
///
/// Minutes are optional when am or pm is given ('8am \u{2013} 9.15am'); bare numbers ('40 - 50') are not times. None when no time range is named.
pub fn var_periods(desc: &str) -> Option<VarPeriods> {
    let re = Regex::lit(r"(\d{1,2})(?:[:.](\d{2}))?\s*(am|pm)?\s*(?:–|-|to)\s*(\d{1,2})(?:[:.](\d{2}))?\s*(am|pm)?", "gi");
    let u: Vec<u16> = desc.encode_utf16().collect();
    let mut out: Vec<[f64; 2]> = Vec::new();
    let mut at = 0;
    while at <= u.len() {
        let Some(m) = re.exec_units(&u, at) else { break };
        let g = |i: usize| unit_text(&u, m.groups[i]);
        let (m1, m2, m3, m4, m5, m6) = (g(0), g(1), g(2), g(3), g(4), g(5));
        let truthy = |s: &Option<String>| s.as_deref().is_some_and(|x| !x.is_empty());
        let num = |s: &Option<String>| s.as_deref().map_or(f64::NAN, crate::jsnum::string_to_number);
        if !((truthy(&m2) || truthy(&m3) || truthy(&m6)) && (truthy(&m5) || truthy(&m6))) || num(&m1) > 24.0 || num(&m4) > 24.0 {
            at = m.start + 1;
            continue;
        }
        at = m.end;
        // h(hh, ap) = (+hh % 12) + (ap is pm ? 12 : 0) + (no ap and hh is 12 ? 12 : 0)
        let h = |hh: &Option<String>, ap: Option<&str>| -> f64 {
            let n = num(hh);
            (n % 12.0) + if ap.is_some_and(|a| a.to_lowercase() == "pm") { 12.0 } else { 0.0 } + if ap.is_none() && n == 12.0 { 12.0 } else { 0.0 }
        };
        let e_ap = if truthy(&m6) { m6.clone() } else { None };
        let s_ap = if truthy(&m3) { m3.clone() } else { e_ap.clone() };
        let mi = |s: &Option<String>| {
            let n = num(s);
            (if n == 0.0 || n.is_nan() { 0.0 } else { n }) / 60.0
        };
        let (min1, min2) = (mi(&m2), mi(&m5));
        let mut s = h(&m1, s_ap.as_deref()) + min1;
        let e = h(&m4, e_ap.as_deref()) + min2;
        if s > e && !truthy(&m3) {
            s = h(&m1, Some("am")) + min1;
        }
        out.push([s, e]);
    }
    if out.is_empty() {
        return None;
    }
    let ov = |w: [f64; 2]| out.iter().any(|[s, e]| *s < w[1] && *e > w[0]);
    Some(VarPeriods { am: ov(PERIOD_AM), pm: ov(PERIOD_PM), windows: out.clone() })
}

/// One polygon of the register: a speed limit zone version.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NslrPoly {
    /// The polygon's rings, [lng, lat] points.
    pub rings: Vec<Ring>,
    /// The box around the rings: [west, south, east, north].
    pub bb: RingsBox,
    /// The limit (km/h).
    pub v: f64,
    /// Speed category ("Permanent", "Variable", ...).
    pub cat: String,
    /// A variable (school) limit.
    pub variable: bool,
    /// The variable limit (km/h) of a variable zone.
    pub var_v: Option<f64>,
    /// When the variable limit applies, as the register words it.
    pub var_desc: String,
    /// The road controlling authority.
    pub rca: String,
    /// The legal reference.
    #[serde(rename = "ref")]
    pub reference: String,
    /// The legal instrument's address.
    pub inst: String,
    /// When this version started (ms since 1970), as the register held it (left out when the register has none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<Value>,
}

impl HasBox for NslrPoly {
    fn rings(&self) -> &[Ring] {
        &self.rings
    }
    fn bb(&self) -> RingsBox {
        self.bb
    }
}

/// How many versions were dropped, and why.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct NslrDrop {
    /// Ended (or not yet started) at the as-at date.
    pub ended: usize,
    /// For a lane purpose other than general traffic.
    pub lane: usize,
    /// With no limit value.
    pub value: usize,
}

/// A downloaded register for an area and a date.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NslrSet {
    /// The zone versions in force.
    pub polys: Vec<NslrPoly>,
    /// The where clause asked.
    #[serde(rename = "where")]
    pub where_: String,
    /// The future year the limits are for, or null for now.
    pub year: Option<f64>,
    /// The as-at date, YYYY-MM-DD.
    pub as_of: String,
    /// Versions left out.
    pub drop: NslrDrop,
    /// Every page was downloaded.
    pub complete: bool,
    /// Features the server sent.
    pub n: usize,
}

/// `parseInt(x, 10)` of a downloaded value: the leading whole number of its text, else NaN.
pub fn parse_int10(v: Option<&JsonValue>) -> f64 {
    let s = match v {
        Some(v) => v.js_string(),
        None => "undefined".to_string(),
    };
    let t = crate::jsnum::js_trim(&s);
    let (neg, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return f64::NAN;
    }
    let n: f64 = digits.parse().unwrap_or(f64::NAN);
    if neg { -n } else { n }
}

fn text_of(v: Option<&JsonValue>) -> String {
    match v {
        Some(v) if v.truthy() => v.js_string(),
        _ => String::new(),
    }
}

/// `loadNSLR(b, year, now)` after the download: the register's polygons in force at the as-at date.
///
/// Never trusts the server's filter alone: a version that has ended (or not started) by the as-at date is dropped,
/// a lane purpose other than general or all is dropped, and a version with no positive limit is dropped.
pub fn parse_nslr(features: &[JsonValue], complete: bool, where_: &str, year: Option<f64>, now_ms: f64) -> NslrSet {
    let at = as_of_ms(year, now_ms);
    let mut drop = NslrDrop::default();
    let mut polys = Vec::new();
    let lane_ok = Regex::lit("general|all", "i");
    let variable_re = Regex::lit("variable", "i");
    let empty = JsonValue::object();
    for x in features {
        let a = x.get("attributes").filter(|a| a.truthy()).unwrap_or(&empty);
        let Some(g) = x.get("geometry").filter(|g| g.truthy()) else { continue };
        let rings = rings_of(g.get("rings"));
        if !g.get("rings").is_some_and(JsonValue::truthy) || rings.is_empty() {
            continue;
        }
        let start = a.get("speedLimitZoneStartDate");
        let end = a.get("speedLimitZoneEndDate");
        let nn = |v: Option<&JsonValue>| v.is_some_and(|v| !v.is_null());
        if (nn(end) && end.unwrap().to_number() <= at) || (nn(start) && start.unwrap().to_number() > at) {
            drop.ended += 1;
            continue;
        }
        if let Some(lane) = a.get("speedLimitZoneLanePurposeName").filter(|l| l.truthy())
            && !lane_ok.test(&lane.js_string())
        {
            drop.lane += 1;
            continue;
        }
        let v = parse_int10(a.get("speedLimitZoneValue"));
        if !(v > 0.0) {
            drop.value += 1;
            continue;
        }
        let cat = text_of(a.get("speedCategoryName"));
        let variable = variable_re.test(&cat);
        let vv = parse_int10(a.get("speedLimitZoneVariableSpeed"));
        polys.push(NslrPoly {
            bb: rings_box(&rings),
            rings,
            v,
            cat,
            variable,
            var_v: if variable && vv > 0.0 { Some(vv) } else { None },
            var_desc: text_of(a.get("speedLimitZoneVarPrdDesc")),
            rca: text_of(a.get("rcaZoneReferenceName")),
            reference: text_of(a.get("legalReference")),
            inst: text_of(a.get("legalInstrumentURL")),
            start: start.map(JsonValue::to_value),
        });
    }
    NslrSet {
        polys,
        where_: where_.to_string(),
        year: match year {
            Some(y) if y != 0.0 && y > nz_year(now_ms) as f64 => Some(y),
            _ => None,
        },
        as_of: utc_date(at),
        drop,
        complete,
        n: features.len(),
    }
}

/// The download of the register for an area and a year: the layer's description, then its polygons (up to 30 pages).
pub struct NslrLoad {
    meta: LayerMeta,
    bounds: Bounds,
    year: Option<f64>,
    now_ms: f64,
    timeout_ms: f64,
    where_: String,
    query: Option<ArcQuery>,
}

impl NslrLoad {
    /// Loads the limits in force now (`year` None) or on 1 July of a later scenario year, for an area.
    pub fn new(bounds: Bounds, year: Option<f64>, now_ms: f64, timeout_ms: f64, cache: &MetaCache) -> NslrLoad {
        NslrLoad {
            meta: LayerMeta::new(src("nslr").expect("source").url, cache, timeout_ms),
            bounds,
            year,
            now_ms,
            timeout_ms,
            where_: nslr_where(year, now_ms),
            query: None,
        }
    }
    fn start_query(&mut self) {
        let fields = resolve_fields(self.meta.meta(), &NSLR_FIELDS);
        let mut p: Vec<(String, String)> = vec![("where".into(), self.where_.clone()), ("outFields".into(), fields.join(","))];
        p.extend(env_params(&self.bounds));
        p.push(("geometryPrecision".into(), "6".into()));
        p.push(("maxAllowableOffset".into(), "0.00002".into()));
        p.push(("orderByFields".into(), "OBJECTID".into()));
        self.query = Some(ArcQuery::new(src("nslr").expect("source").url, p, 30, self.timeout_ms));
    }
    /// The register's polygons once the download is done.
    pub fn finish(self) -> NslrSet {
        let q = self.query.expect("the download finished");
        let complete = q.complete();
        parse_nslr(q.features(), complete, &self.where_, self.year, self.now_ms)
    }
}

impl Flow for NslrLoad {
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
        if self.query.is_none() {
            // the description is optional: field names are taken as asked when it cannot be read
            let _ = self.meta.answer(a);
            self.start_query();
            return Ok(());
        }
        self.query.as_mut().expect("query").answer(a)
    }
    fn is_done(&self) -> bool {
        self.query.as_ref().is_some_and(Flow::is_done)
    }
}

/// What `assignNSLR` counted.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NslrStats {
    /// Existing, open links looked at.
    pub links: usize,
    /// Links that took a legal limit.
    pub nslr: usize,
    /// Of those, links with a variable (school) limit too.
    pub variable: usize,
    /// Links with no limit in the register here.
    pub missing: usize,
    /// nslr / links.
    pub share: f64,
}

fn extra_f64(l: &Link, k: &str) -> f64 {
    l.extra.get(k).map_or(f64::NAN, value_to_number)
}

// The version that started last, then the lower value (a version with no start counts as 0)
fn latest<'a>(hs: &[&'a NslrPoly]) -> Option<&'a NslrPoly> {
    let start = |h: &NslrPoly| match &h.start {
        Some(v) if value_truthy(v) => value_to_number(v),
        _ => 0.0,
    };
    let mut it = hs.iter().copied();
    let mut m = it.next()?;
    for h in it {
        if start(h) > start(m) || (start(h) == start(m) && h.v < m.v) {
            m = h;
        }
    }
    Some(m)
}

/// `assignNSLR(sc, set)`: puts the legal limit on every open link of a scenario: the modal value of the polygons under 25, 50 and 75% of the link.
///
/// Sets `speedLimit`, `speedSource` ("NSLR"), `nslr` (limit, category, authority, legal reference, as-at date, year and any
/// variable limit) and `varSpeed`. A link with nothing in the register now loses what an earlier load put on it and is marked "OSM or class default".
pub fn assign_nslr(sc: &mut Scenario, set: &NslrSet) -> NslrStats {
    let find = PolyIndex::new(&set.polys, 0.005);
    let mut st = NslrStats { links: 0, nslr: 0, variable: 0, missing: 0, share: 0.0 };
    let ids: Vec<String> = live_links(sc).iter().map(|l| l.id.clone()).collect();
    for id in ids {
        st.links += 1;
        let l = &sc.links[id.as_str()];
        let g = sc.geom(l);
        let cum = util::cum_len(&g);
        let len = cum[cum.len() - 1];
        let mut picks: Vec<Option<&NslrPoly>> = Vec::new();
        let mut vars: Vec<&NslrPoly> = Vec::new();
        for fr in NSLR_SAMPLES {
            let p = util::along(&g, &cum, len * fr).p;
            let hit: Vec<&NslrPoly> = find.find(&set.polys, p[1], p[0]).into_iter().map(|n| &set.polys[n]).collect();
            let perm: Vec<&NslrPoly> = hit.iter().copied().filter(|h| !h.variable).collect();
            picks.push(latest(if !perm.is_empty() { &perm } else { &hit }));
            let vz_list: Vec<&NslrPoly> = hit.iter().copied().filter(|h| h.variable && h.var_v.is_some()).collect();
            if let Some(vz) = latest(&vz_list) {
                vars.push(vz);
            }
        }
        // votes by limit, in the order limits were first seen
        let mut votes: Vec<(f64, usize)> = Vec::new();
        for h in picks.iter().flatten() {
            match votes.iter_mut().find(|(v, _)| *v == h.v) {
                Some(e) => e.1 += 1,
                None => votes.push((h.v, 1)),
            }
        }
        let l = sc.links.get_mut(id.as_str()).expect("link");
        if votes.is_empty() {
            if l.extra.get("speedSource").and_then(Value::as_str) == Some("NSLR") {
                l.extra.remove("speedLimit");
                l.extra.remove("nslr");
                l.var_speed = Maybe::Missing;
            }
            l.extra.insert("speedSource".to_string(), json!("OSM or class default"));
            st.missing += 1;
            continue;
        }
        let top = votes.iter().map(|(_, n)| *n).max().expect("votes");
        let cands: Vec<f64> = votes.iter().filter(|(_, n)| *n == top).map(|(v, _)| *v).collect();
        let v = if cands.len() == 1 {
            cands[0]
        } else if let Some(Some(p1)) = picks.get(1).copied().map(|p| p.filter(|p| cands.contains(&p.v))) {
            p1.v
        } else {
            math_min_of(&cands)
        };
        let src_poly = picks.iter().flatten().find(|h| h.v == v).expect("a pick holds the limit");
        let vz = vars.first().copied();
        l.extra.insert("speedLimit".to_string(), crate::jsnum::json_number(v));
        l.extra.insert("speedSource".to_string(), json!("NSLR"));
        let mut n = Map::new();
        n.insert("v".to_string(), crate::jsnum::json_number(v));
        n.insert("cat".to_string(), json!(src_poly.cat));
        n.insert("rca".to_string(), json!(src_poly.rca));
        n.insert("ref".to_string(), json!(src_poly.reference));
        n.insert("inst".to_string(), json!(src_poly.inst));
        n.insert("at".to_string(), json!(set.as_of));
        n.insert("year".to_string(), set.year.map_or(Value::Null, crate::jsnum::json_number));
        if let Some(vz) = vz {
            let mut var = Map::new();
            var.insert("v".to_string(), vz.var_v.map_or(Value::Null, crate::jsnum::json_number));
            var.insert("desc".to_string(), json!(vz.var_desc));
            let vp = var_periods(&vz.var_desc);
            if let Some(vp) = &vp {
                var.insert("AM".to_string(), json!(vp.am));
                var.insert("PM".to_string(), json!(vp.pm));
            }
            n.insert("var".to_string(), Value::Object(var));
            l.var_speed = Maybe::Set(json!({
                "v": vz.var_v.map_or(Value::Null, crate::jsnum::json_number),
                "desc": vz.var_desc,
                "AM": vp.as_ref().is_some_and(|p| p.am),
                "PM": vp.as_ref().is_some_and(|p| p.pm),
            }));
            st.variable += 1;
        } else {
            l.var_speed = Maybe::Missing;
        }
        l.extra.insert("nslr".to_string(), Value::Object(n));
        st.nslr += 1;
    }
    st.share = if st.links > 0 { st.nslr as f64 / st.links as f64 } else { 0.0 };
    st
}

/// A row of the speed review: a road whose modelled speed differs from its legal limit.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SpeedProposal {
    /// Link id.
    pub id: String,
    /// Name, or "Unnamed road".
    pub name: String,
    /// Road class.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cls: Option<String>,
    /// The modelled speed now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Value>,
    /// The legal limit.
    pub to: f64,
    /// The user changed the modelled speed after the legal limit was applied.
    pub user: bool,
    /// A variable (school) limit applies too.
    pub variable: bool,
}

/// `userSpeed(L)`: a modelled speed the user changed after the legal limit was applied.
pub fn user_speed(l: &Link) -> bool {
    match l.extra.get("speedApplied") {
        None | Some(Value::Null) => false,
        Some(a) => !strict_eq_num(&l.speed, a),
    }
}

// L.speed === x for a saved speed and a value read from the link's extra keys
fn strict_eq_num(speed: &crate::jsnum::Num, x: &Value) -> bool {
    match (speed, x) {
        (Maybe::Set(JsNum::Number(a)), Value::Number(b)) => b.as_f64() == Some(*a),
        _ => false,
    }
}

/// `speedProposals(sc)`: existing roads that have a legal limit different from their modelled speed.
pub fn speed_proposals(sc: &Scenario) -> Vec<SpeedProposal> {
    live_links(sc)
        .into_iter()
        .filter(|l| l.status.is("existing") && extra_f64(l, "speedLimit") > 0.0 && !strict_eq_num(&l.speed, l.extra.get("speedLimit").unwrap_or(&Value::Null)))
        .map(|l| SpeedProposal {
            id: l.id.clone(),
            name: l.name.str_or("Unnamed road").to_string(),
            cls: l.cls.as_str().map(str::to_string),
            from: match &l.speed {
                Maybe::Set(v) => Some(v.to_value()),
                Maybe::Null => Some(Value::Null),
                Maybe::Missing => None,
            },
            to: extra_f64(l, "speedLimit"),
            user: user_speed(l),
            variable: l.var_speed.get().is_some_and(value_truthy),
        })
        .collect()
}

/// `applySpeeds(sc, ids)`: copies the legal limit into the modelled (free-flow) speed of the chosen roads (keeping the old speed as `speedOSM` the first time); gives how many changed.
pub fn apply_speeds(sc: &mut Scenario, ids: &[String]) -> usize {
    let mut n = 0;
    for id in ids {
        let Some(l) = sc.links.get_mut(id.as_str()) else { continue };
        if !(extra_f64(l, "speedLimit") > 0.0) {
            continue;
        }
        let none = matches!(l.extra.get("speedOSM"), None | Some(Value::Null));
        if none {
            let old = match &l.speed {
                Maybe::Set(v) => v.to_value(),
                _ => Value::Null,
            };
            if !(matches!(l.speed, Maybe::Missing)) {
                l.extra.insert("speedOSM".to_string(), old);
            }
        }
        let limit = l.extra.get("speedLimit").cloned().unwrap_or(Value::Null);
        l.speed = Maybe::Set(JsNum::from(limit.clone()));
        l.extra.insert("speedApplied".to_string(), limit);
        n += 1;
    }
    n
}

/// The scenario years a speed limit download covers (`nslrAll`): 0 for the limits in force now, or a later scenario's year (when the project
/// settings ask for gazetted future limits), once each, in the order the scenarios list them.
pub fn nslr_years(p: &Project, now_ms: f64, nslr_future: bool) -> Vec<f64> {
    let y_now = nz_year(now_ms) as f64;
    let mut years: Vec<f64> = Vec::new();
    for sc in &p.scenarios {
        let y = year_of(Some(sc), &p.settings);
        let k = if nslr_future && y > y_now { y } else { 0.0 };
        if !years.contains(&k) {
            years.push(k);
        }
    }
    years
}

/// The progress line for one year of the download.
pub fn nslr_status(year: f64) -> String {
    if year != 0.0 {
        format!("Downloading speed limits in force on 1 July {}\u{2026}", year_text(year))
    } else {
        "Downloading current speed limits (NZTA register)\u{2026}".to_string()
    }
}

/// What `nslrAll` reports: per scenario how many links took a legal limit, the versions dropped and whether every page came.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NslrAllResult {
    /// One entry per scenario.
    pub scenarios: Vec<NslrScenarioResult>,
    /// Versions left out, over every download.
    pub drop: NslrDrop,
    /// Every page of every download came.
    pub complete: bool,
}

/// One scenario's part of [`NslrAllResult`].
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NslrScenarioResult {
    /// Scenario id.
    pub id: String,
    /// Scenario name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The future year the limits are for, or null.
    pub year: Option<f64>,
    /// How many links were looked at, took a limit, and so on.
    #[serde(flatten)]
    pub stats: NslrStats,
}

/// `nslrAll` after the downloads: assigns each scenario the set for its year, records the result in `P.feeds.nslr` and the data vintage table.
///
/// `sets` holds one downloaded set per year of [`nslr_years`] (0 for the limits in force now). The active scenario's share is quoted in the vintage note.
pub fn apply_nslr_all(p: &mut Project, sets: &[(f64, NslrSet)], now_ms: f64, nslr_future: bool) -> NslrAllResult {
    let y_now = nz_year(now_ms) as f64;
    let mut out = NslrAllResult { scenarios: Vec::new(), drop: NslrDrop::default(), complete: true };
    for (_, s) in sets {
        out.drop.ended += s.drop.ended;
        out.drop.lane += s.drop.lane;
        out.drop.value += s.drop.value;
        out.complete = out.complete && s.complete;
    }
    let settings = p.settings.clone();
    for sc in p.scenarios.iter_mut() {
        let y = year_of(Some(sc), &settings);
        let k = if nslr_future && y > y_now { y } else { 0.0 };
        let set = &sets.iter().find(|(yy, _)| *yy == k).expect("a set for the scenario's year").1;
        let stats = assign_nslr(sc, set);
        out.scenarios.push(NslrScenarioResult { id: sc.id.clone(), name: sc.name.as_str().map(str::to_string), year: set.year, stats });
    }
    let active = p.active.as_str().map(str::to_string);
    let cur = out.scenarios.iter().find(|s| Some(&s.id) == active.as_ref()).or(out.scenarios.first());
    let cur_text = match cur {
        Some(c) => format!("{} of roads in {} matched.", fmt::pct(c.stats.share, 0), c.name.clone().unwrap_or_else(|| "undefined".to_string())),
        None => String::new(),
    };
    let any_future = sets.iter().any(|(y, _)| *y != 0.0);
    let note = format!(
        "Current versions only{}. {}{}",
        if any_future { "; future scenario years use the limits gazetted for 1 July of that year" } else { "" },
        cur_text,
        if out.complete { "" } else { " Not every page downloaded: zoom to a smaller area." }
    );
    let record = json!({
        "at": crate::edit::ids::iso_string(now_ms),
        "scenarios": out.scenarios.iter().map(|s| json!({ "id": s.id, "year": s.year, "links": s.stats.links, "nslr": s.stats.nslr, "variable": s.stats.variable })).collect::<Vec<_>>(),
        "drop": serde_json::to_value(&out.drop).expect("drop"),
    });
    feeds_of(p).insert("nslr".to_string(), record);
    let n: usize = sets.iter().map(|(_, s)| s.polys.len()).sum();
    let mut extra = Map::new();
    extra.insert("n".to_string(), json!(n));
    extra.insert("note".to_string(), json!(note));
    stamp(p, "nslr", &extra, now_ms);
    out
}
