//! Trip generation (AT ITA Guidelines 2025 v2 Appendix 1): person and vehicle trips per zone and activity, the adjustments in sequence,
//! how a zone's table reaches the assignment, the tables T1 to T4b data, the RR 422 check and `trip_generation.csv` (landuse-plus.js "trip generation").

use super::consts::{self, DEFAULT_RATE_SOURCE, MODES, PERIODS, group_of, ita, mode_src, rr422, std as std_};
use super::fill::num_json;
use super::geo::r2;
use crate::demand::stage_of;
use crate::edit::json::{JsonValue, deep_merge};
use crate::graph::zone_class;
use crate::jsnum::{JsNum, Maybe, Num, js_trim, json_number, math_max, math_min, math_round, number_to_js_string, or_default, truthy_number, value_to_js_string, value_to_number, value_truthy};
use crate::project::{LANDUSES, LandUse, OrderedMap, Project, Scenario, Settings, Zone};
use crate::standards::year_of;
use crate::util::{self, clamp, fmt, r1, r3};
use serde_json::{Map, Value, json};
use std::fmt::Write as _;

/// The land use settings (`conf(S)`): the default mode split, students per car and the source and year of each trip rate, merged with the project's own.
#[must_use]
pub fn conf(s: &Settings) -> Value {
    let defaults = json!({ "split": { "driver": std_::SPLIT_DRIVER, "passenger": std_::SPLIT_PASSENGER, "pt": std_::SPLIT_PT, "walk": std_::SPLIT_WALK, "cycle": std_::SPLIT_CYCLE },
        "eduOcc": std_::EDU_OCC, "rateMeta": {} });
    let own = s.extra.get("landuse").filter(|x| value_truthy(x)).cloned().unwrap_or_else(|| json!({}));
    deep_merge(&JsonValue::from_value(&defaults), &JsonValue::from_value(&own)).to_value()
}

fn cfg_split(cfg: &Value, m: &str) -> f64 {
    math_max(0.0, or_default(cfg.get("split").and_then(|s| s.get(m)).map_or(f64::NAN, value_to_number), 0.0))
}

/// The share of each travel mode and where it came from (the answer of `splitFor`).
#[derive(Clone, Debug, PartialEq)]
pub struct Split {
    /// Car driver (%), car passenger, public transport, walk, cycle, in the order of [`MODES`].
    pub modes: [f64; 5],
    /// "user", "derived" or "assumed".
    pub basis: &'static str,
    /// Where the shares came from.
    pub source: String,
    /// What the entered shares add up to (user and assumed).
    pub sum: Option<f64>,
    /// People in the Census sample (derived).
    pub n: Option<f64>,
    /// The Census specification's car share, when it can be worked out (derived; None is null).
    pub car_spec: Option<Option<f64>>,
}

impl Split {
    /// The share of a mode ("driver", "passenger", "pt", "walk" or "cycle").
    #[must_use]
    pub fn get(&self, m: &str) -> f64 {
        MODES.iter().position(|x| *x == m).and_then(|i| self.modes.get(i).copied()).unwrap_or(f64::NAN)
    }
    /// The answer as JSON.
    pub fn to_value(&self) -> Value {
        let mut o = Map::new();
        for (m, x) in MODES.iter().zip(self.modes) {
            o.insert(m.to_string(), json_number(x));
        }
        if let Some(n) = self.n {
            o.insert("n".into(), json_number(n));
        }
        if let Some(c) = self.car_spec {
            o.insert("carSpec".into(), c.map_or(Value::Null, json_number));
        }
        o.insert("basis".into(), json!(self.basis));
        o.insert("source".into(), json!(self.source));
        if let Some(s) = self.sum {
            o.insert("sum".into(), json_number(s));
        }
        Value::Object(o)
    }
}

// `c[k] || 0` of a loose count
fn cnt(c: &Value, k: &str) -> f64 {
    or_default(c.get(k).map_or(0.0, value_to_number), 0.0)
}

// `+m || 0`
fn plus0(x: f64) -> f64 {
    or_default(x, 0.0)
}

/// What `sharesOf` finds: mode shares of a Census mode count object.
#[derive(Clone, Debug, PartialEq)]
pub struct Shares {
    /// Car driver, passenger, public transport, walk, cycle (%).
    pub modes: [f64; 5],
    /// People counted.
    pub n: f64,
    /// The Census specification's car share, or None.
    pub car_spec: Option<f64>,
}

/// `sharesOf(c, g)`: mode shares from the Census mode counts of a travel topic (g is "r", "w" or "e"); None when nobody is counted.
pub fn shares_of(c: &Value, g: &str) -> Option<Shares> {
    if !value_truthy(c) {
        return None;
    }
    let raw = |k: &str| c.get(k).map_or(f64::NAN, value_to_number);
    let m: [f64; 5] = if g == "e" {
        [raw("drive"), raw("pass"), cnt(c, "schoolBus") + cnt(c, "bus") + cnt(c, "train") + cnt(c, "ferry"), raw("walk"), raw("bike")]
    } else {
        [cnt(c, "drivePriv") + cnt(c, "driveCo"), raw("pass"), cnt(c, "bus") + cnt(c, "train") + cnt(c, "ferry"), raw("walk"), raw("bike")]
    };
    let n = m.iter().fold(0.0, |s, x| s + plus0(*x));
    // !(n > 0), NaN included
    if n <= 0.0 || n.is_nan() {
        return None;
    }
    let stated = c.get("stated").map_or(f64::NAN, value_to_number);
    let car_spec = if g != "e" && stated > cnt(c, "home") {
        Some(100.0 * (cnt(c, "drivePriv") + cnt(c, "driveCo") + cnt(c, "pass")) / (stated - cnt(c, "home")))
    } else {
        None
    };
    let modes = m.map(|x| 100.0 * plus0(x) / n);
    Some(Shares { modes, n: math_round(n), car_spec })
}

fn tg_on(z: &Zone) -> Option<&Value> {
    z.tg.get().filter(|t| t.get("on").is_some_and(value_truthy))
}

// `x != null && x !== ''`
fn given(v: Option<&Value>) -> bool {
    v.is_some_and(|x| !x.is_null() && x.as_str() != Some(""))
}

/// `splitFor(z, k, S)`: the mode split for an activity in a zone: the zone's own entry, else the Census for its SA1s, else the default.
#[must_use]
pub fn split_for(z: &Zone, k: &str, s: &Settings) -> Split {
    let tg = tg_on(z);
    let cfg = conf(s);
    if let Some(own) = tg.and_then(|table| table.get("split")).filter(|x| value_truthy(x))
        && MODES.iter().any(|mode| given(own.get(*mode)))
    {
        let raw: [f64; 5] = MODES.map(|mode| math_max(0.0, plus0(own.get(mode).map_or(f64::NAN, value_to_number))));
        let sum = or_default(raw.iter().fold(0.0, |acc, x| acc + x), 1.0);
        let modes = raw.map(|x| 100.0 * x / sum);
        return Split { modes, basis: "user", source: "entered for this zone".into(), sum: Some(sum), n: None, car_spec: None };
    }
    let topic = group_of(k);
    let census = z.extra.get("census").filter(|rec| rec.get("modes").and_then(|ms| ms.get(topic)).is_some_and(value_truthy));
    let src = census.or_else(|| z.extra.get("censusLoc").filter(|x| value_truthy(x)));
    let sh = src.and_then(|rec| rec.get("modes")).filter(|x| value_truthy(x)).and_then(|ms| ms.get(topic)).and_then(|counts| shares_of(counts, topic));
    // sh is only Some when src is
    if let Some(sh) = &sh
        && sh.n >= std_::CENSUS_MIN_N
        && let Some(src) = src
    {
        let codes: Vec<String> = match src.get("sa1") {
            Some(Value::Array(list)) => list.iter().map(|rec| rec.get("code").map_or("undefined".to_string(), value_to_js_string)).collect(),
            Some(other) => vec![value_to_js_string(other)],
            None => vec!["undefined".to_string()],
        };
        let shown = codes.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
        return Split {
            modes: sh.modes,
            basis: "derived",
            source: format!("{}, SA1 {}{} ({} people)", mode_src(topic), shown, if codes.len() > 4 { " \u{2026}" } else { "" }, number_to_js_string(sh.n)),
            sum: None,
            n: Some(sh.n),
            car_spec: Some(sh.car_spec),
        };
    }
    let raw: [f64; 5] = MODES.map(|mode| cfg_split(&cfg, mode));
    let sum = raw.iter().fold(0.0, |acc, x| acc + x);
    let split: [f64; 5] = if sum > 0.0 {
        raw.map(|x| 100.0 * x / sum)
    } else {
        [std_::SPLIT_DRIVER, std_::SPLIT_PASSENGER, std_::SPLIT_PT, std_::SPLIT_WALK, std_::SPLIT_CYCLE]
    };
    let mut source = std_::SPLIT_LABEL.to_string();
    // Writing to a String cannot fail
    if let Some(sh) = &sh {
        let _ = write!(source, " (the Census sample is only {} people)", number_to_js_string(sh.n));
    }
    if (sum - 100.0).abs() > 0.5 {
        let _ = write!(source, " (entered shares add up to {}%, rescaled to 100%)", fmt::num(sum, 0));
    }
    Split { modes: split, basis: "assumed", source, sum: Some(sum), n: None, car_spec: None }
}

/// People per car, and where it came from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Occ {
    /// People per car.
    pub v: f64,
    /// "user", "assumed" or "derived".
    pub basis: &'static str,
}

/// `occFor(z, k, sp, cfg)`: people per car: the zone's own, the education default, or from the mode split.
pub fn occ_for(z: &Zone, k: &str, sp: &Split, cfg: &Value) -> Occ {
    let occ = tg_on(z).and_then(|t| t.get("occupancy")).map_or(f64::NAN, value_to_number);
    if occ > 0.0 {
        return Occ { v: occ, basis: "user" };
    }
    if group_of(k) == "e" {
        return Occ { v: cfg.get("eduOcc").map_or(f64::NAN, value_to_number), basis: "assumed" };
    }
    let (d, p) = (sp.modes[0], sp.modes[1]);
    Occ { v: if d > 0.0 { (d + p) / d } else { 1.0 }, basis: "derived" }
}

/// A rate review flag: level "warn" or "review" and its text.
#[derive(Clone, Debug, PartialEq)]
pub struct RateFlag {
    /// "warn" or "review".
    pub level: &'static str,
    /// What to do.
    pub text: String,
}

/// `rateFlag(year)`: a note on a rate's survey year: none for a rate surveyed after 2022.
#[must_use]
pub fn rate_flag(year: f64) -> Option<RateFlag> {
    let y = year;
    // !(y > 1900), NaN included
    if y <= 1900.0 || y.is_nan() {
        return Some(RateFlag { level: "warn", text: "No survey year: an indicative default. Replace it with a surveyed or published rate (AT ITA Guidelines App 1).".into() });
    }
    if y < 2020.0 {
        return Some(RateFlag { level: "review", text: format!("Rate from {}, before 2020: {}", number_to_js_string(y), ita::REVIEW) });
    }
    if y <= ita::REVIEW_TO {
        return Some(RateFlag { level: "review", text: format!("Rate from {}, surveyed 2020\u{2013}2022: {}", number_to_js_string(y), ita::REVIEW) });
    }
    None
}

/// `unitOne(L)`: the unit of a trip rate ("dwelling", "100 m\u{b2} GFA").
#[must_use]
pub fn unit_one(l: &LandUse) -> String {
    if l.per == 100.0 {
        return "100 m\u{b2} GFA".to_string();
    }
    let mut u = l.unit.to_string();
    if u.ends_with('s') {
        u.pop();
    }
    if let Some(stem) = u.strip_suffix("places") {
        u = format!("{stem}place");
    }
    u
}

// `pv(o, p)`: o[p] as a number when it is given and finite, else None
fn pv(o: Option<&Value>, p: &str) -> Option<f64> {
    let o = o.filter(|x| value_truthy(x))?;
    let v = o.get(p)?;
    if v.is_null() || v.as_str() == Some("") {
        return None;
    }
    let x = value_to_number(v);
    x.is_finite().then_some(x)
}

/// An inbound and outbound pair of vehicle trips (veh/h).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Dir {
    /// Arriving.
    pub r#in: f64,
    /// Leaving.
    pub out: f64,
}

impl Dir {
    fn sub(self, b: Dir) -> Dir {
        Dir { r#in: self.r#in - b.r#in, out: self.out - b.out }
    }
    fn mul(self, f: f64) -> Dir {
        Dir { r#in: self.r#in * f, out: self.out * f }
    }
    /// The pair as JSON.
    #[must_use]
    pub fn to_value(self) -> Value {
        json!({ "in": json_number(self.r#in), "out": json_number(self.out) })
    }
}

/// What each adjustment took off one activity (the `adj` of a row).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Adj {
    /// Existing site trips.
    pub existing: f64,
    /// Internal trips.
    pub internal: f64,
    /// Pass-by trips.
    pub passby: f64,
    /// Travel demand management.
    pub tdm: f64,
}

/// One activity of a zone in a period (a row of the trip generation tables).
#[derive(Clone, Debug, PartialEq)]
pub struct TripRow {
    /// Land use key, or "other".
    pub k: String,
    /// Display name.
    pub label: String,
    /// Display group.
    pub group: String,
    /// Quantity (None for other trips).
    pub q: Option<f64>,
    /// Unit of the quantity.
    pub unit: String,
    /// "vehicle" or "person".
    pub basis: &'static str,
    /// The rate (None for other trips).
    pub rate: Option<f64>,
    /// What the rate is per.
    pub rate_unit: String,
    /// Where the rate came from.
    pub source: String,
    /// Survey year of the rate.
    pub year: Option<f64>,
    /// Review flag of the rate.
    pub flag: Option<RateFlag>,
    /// Person trips.
    pub person: Option<f64>,
    /// Mode split (None for other trips).
    pub split: Option<Split>,
    /// People per car (None for other trips).
    pub occ: Option<f64>,
    /// Where the occupancy came from.
    pub occ_basis: Option<&'static str>,
    /// Gross vehicle trips.
    pub veh: f64,
    /// Share arriving (%).
    pub in_pct: f64,
    /// Arriving vehicle trips.
    pub vin: f64,
    /// Leaving vehicle trips.
    pub vout: f64,
    /// Freight and service trips.
    pub freight: f64,
    /// Freight rate.
    pub freight_rate: f64,
    /// Heavy vehicle share (%).
    pub hv_pct: f64,
    /// Opening hours.
    pub hours: String,
    /// The activity has a drive-through.
    pub drive_through: bool,
    /// The rate was entered for this zone.
    pub own_rate: bool,
    /// What each adjustment took off.
    pub adj: Adj,
    /// Net arriving.
    pub net_in: f64,
    /// Net leaving.
    pub net_out: f64,
    /// Net vehicle trips.
    pub net: f64,
    /// Heavy vehicle trips.
    pub hv: f64,
    /// The rate as the settings held it, when the row used the settings' rate (it can be text there); the answer shows it as it was.
    pub rate_raw: Option<Value>,
    /// The share arriving as the settings held it, when the row used the settings' share.
    pub in_pct_raw: Option<Value>,
    /// The heavy vehicle share as the settings held it, when the row used the settings' share.
    pub hv_pct_raw: Option<Value>,
    /// People per car as the settings held it (students per car on education trips).
    pub occ_raw: Option<Value>,
}

impl TripRow {
    /// The row as JSON, in the prototype's shape.
    pub fn to_value(&self) -> Value {
        let n = |x: Option<f64>| x.map_or(Value::Null, json_number);
        let mut o = Map::new();
        o.insert("k".into(), json!(self.k));
        o.insert("label".into(), json!(self.label));
        o.insert("group".into(), json!(self.group));
        o.insert("q".into(), n(self.q));
        o.insert("unit".into(), json!(self.unit));
        o.insert("basis".into(), json!(self.basis));
        o.insert("rate".into(), self.rate_raw.clone().unwrap_or_else(|| n(self.rate)));
        o.insert("rateUnit".into(), json!(self.rate_unit));
        o.insert("source".into(), json!(self.source));
        o.insert("year".into(), n(self.year));
        o.insert("flag".into(), self.flag.as_ref().map_or(Value::Null, |f| json!({ "level": f.level, "text": f.text })));
        o.insert("person".into(), n(self.person));
        o.insert("split".into(), self.split.as_ref().map_or(Value::Null, Split::to_value));
        o.insert("occ".into(), self.occ_raw.clone().unwrap_or_else(|| n(self.occ)));
        if let Some(b) = self.occ_basis {
            o.insert("occBasis".into(), json!(b));
        }
        o.insert("veh".into(), json_number(self.veh));
        o.insert("inPct".into(), self.in_pct_raw.clone().unwrap_or_else(|| json_number(self.in_pct)));
        o.insert("vin".into(), json_number(self.vin));
        o.insert("vout".into(), json_number(self.vout));
        o.insert("freight".into(), json_number(self.freight));
        o.insert("freightRate".into(), json_number(self.freight_rate));
        o.insert("hvPct".into(), self.hv_pct_raw.clone().unwrap_or_else(|| json_number(self.hv_pct)));
        o.insert("hours".into(), json!(self.hours));
        o.insert("driveThrough".into(), json!(self.drive_through));
        o.insert("ownRate".into(), json!(self.own_rate));
        o.insert("adj".into(), json!({ "existing": json_number(self.adj.existing), "internal": json_number(self.adj.internal), "passby": json_number(self.adj.passby), "tdm": json_number(self.adj.tdm) }));
        o.insert("netIn".into(), json_number(self.net_in));
        o.insert("netOut".into(), json_number(self.net_out));
        o.insert("net".into(), json_number(self.net));
        o.insert("hv".into(), json_number(self.hv));
        Value::Object(o)
    }
}

/// A zone's trips in one period: rows per activity and the totals from gross to net in the ITA order (the answer of `tripGen`).
#[derive(Clone, Debug, PartialEq)]
pub struct TripGen {
    /// Zone id.
    pub zone: String,
    /// Zone name.
    pub name: Value,
    /// "existing", "committed" or "development".
    pub cls: &'static str,
    /// "AM" or "PM".
    pub period: String,
    /// The zone's own table is switched on.
    pub on: bool,
    /// One row per activity (then other trips).
    pub rows: Vec<TripRow>,
    /// Gross vehicle trips.
    pub gross: Dir,
    /// Existing site trips taken off (never more than the gross).
    pub existing: Dir,
    /// Existing site trips as entered.
    pub existing_raw: Dir,
    /// Internal trips.
    pub internal: Dir,
    /// Pass-by trips.
    pub passby: Dir,
    /// Trips after pass-by.
    pub after_p: Dir,
    /// Travel demand management.
    pub tdm: Dir,
    /// Net vehicle trips.
    pub net: Dir,
    /// Freight and service trips (kept separate).
    pub freight: Dir,
    /// The TDM reduction is 100% so freight cannot be loaded.
    pub lost: bool,
    /// The zone is open in the scenario's year.
    pub open: bool,
    /// Share built in the scenario.
    pub stage: f64,
    /// What the assignment loads: (net + freight) times the build-out, 0 before opening.
    pub loaded: Dir,
    /// Internal, pass-by and TDM as percentages.
    pub pct: (f64, f64, f64),
}

impl TripGen {
    /// The existing site trips are larger than the new trips (as entered, by more than 1e-9 veh/h either way).
    #[must_use]
    pub fn exceeds(&self) -> bool {
        self.existing_raw.r#in > self.gross.r#in + 1e-9 || self.existing_raw.out > self.gross.out + 1e-9
    }
    /// The answer as JSON, in the prototype's shape.
    pub fn to_value(&self) -> Value {
        json!({
            "zone": self.zone, "name": self.name, "cls": self.cls, "period": self.period, "on": self.on,
            "rows": self.rows.iter().map(TripRow::to_value).collect::<Vec<_>>(),
            "gross": self.gross.to_value(), "existing": self.existing.to_value(), "existingRaw": self.existing_raw.to_value(), "exceeds": self.exceeds(),
            "internal": self.internal.to_value(), "passby": self.passby.to_value(), "afterP": self.after_p.to_value(), "tdm": self.tdm.to_value(), "net": self.net.to_value(),
            "freight": self.freight.to_value(), "lost": self.lost, "open": self.open, "stage": json_number(self.stage), "loaded": self.loaded.to_value(),
            "pct": { "internal": json_number(self.pct.0), "passby": json_number(self.pct.1), "tdm": json_number(self.pct.2) },
        })
    }
}

/// `syncState(z)`: how a switched-on table reaches the engine: "tgLoad", "extra" or None.
#[must_use]
pub fn sync_state(z: &Zone) -> Option<String> {
    let tg = z.tg.get().filter(|t| value_truthy(t))?;
    match tg.get("synced").filter(|x| value_truthy(x)) {
        Some(s) => Some(value_to_js_string(s)),
        None => tg.get("other").map(|_| "extra".to_string()),
    }
}

// The trips entered directly: `otherOf(z)[period + 'in']` and `+ 'out'`
fn other_trips(z: &Zone, period: &str) -> (f64, f64) {
    let (kin, kout) = (format!("{period}in"), format!("{period}out"));
    if tg_on(z).is_some() && sync_state(z).as_deref() == Some("extra") {
        let o = z.tg.get().and_then(|t| t.get("other"));
        let g = |k: &str| or_default(o.and_then(|o| o.get(k)).map_or(f64::NAN, value_to_number), 0.0);
        (g(&kin), g(&kout))
    } else {
        let e = z.extra_trips.get();
        let g = |k: &str| or_default(e.map_or(f64::NAN, |e| e.num_at(k).num()), 0.0);
        (g(&kin), g(&kout))
    }
}

// Where a row's rate came from and its survey year: the zone's own entry, else (for a rate not entered for the zone) the settings'
fn rate_source(spec: &Value, meta: &Value, custom: bool) -> (String, Option<f64>) {
    let source = match spec.get("source") {
        Some(x) if value_truthy(x) => value_to_js_string(x),
        _ => {
            if custom {
                "entered for this zone (no source given)".to_string()
            } else {
                match meta.get("source") {
                    Some(x) if value_truthy(x) => value_to_js_string(x),
                    _ => DEFAULT_RATE_SOURCE.to_string(),
                }
            }
        }
    };
    let year: Option<f64> = if given(spec.get("year")) {
        Some(value_to_number(&spec["year"]))
    } else if custom {
        None
    } else if given(meta.get("year")) {
        Some(value_to_number(&meta["year"]))
    } else {
        None
    };
    (source, year)
}

// One land use's row of a zone in a period (the loop over LANDUSES in `tripGen`): None when the zone has none of it or the
// settings have no rate for it. tg_val is the zone's table when it is switched on, else {} (so spec is then {}).
fn land_use_row(z: &Zone, s: &Settings, period: &str, cfg: &Value, tg_val: &Value, lu_def: &LandUse) -> Option<TripRow> {
    let key = lu_def.key;
    let qty = or_default(z.lu.get().map_or(f64::NAN, |lu| lu.num_at(key).num()), 0.0);
    if qty == 0.0 {
        return None;
    }
    let rates = s.rates.get().and_then(|m| m.get(key)).and_then(|m| m.get()).and_then(|m| m.get(period)).and_then(|m| m.get())?;
    let empty = json!({});
    let spec = tg_val.get("rows").filter(|x| value_truthy(x)).and_then(|rw| rw.get(key)).filter(|x| value_truthy(x)).unwrap_or(&empty);
    let units = qty / lu_def.per;
    let meta = cfg.get("rateMeta").and_then(|m| m.get(key)).filter(|x| value_truthy(x)).unwrap_or(&empty);
    // hours and the drive-through flag describe the activity (T1); they never switch the table on
    let desc = z.tg.get().and_then(|table| table.get("rows")).filter(|x| value_truthy(x)).and_then(|rw| rw.get(key)).filter(|x| value_truthy(x)).unwrap_or(&empty);
    let hours = match desc.get("hours") {
        Some(h) if !h.is_null() => js_trim(&value_to_js_string(h)).to_string(),
        _ => String::new(),
    };
    let drive = desc.get("driveThrough") == Some(&Value::Bool(true));
    let sp = split_for(z, key, s);
    let car = (sp.modes[0] + sp.modes[1]) / 100.0;
    let oc = occ_for(z, key, &sp, cfg);
    let occ = oc.v;
    let (basis, rate, person, veh);
    let mut rate_raw: Option<Value> = None;
    if spec.get("basis").and_then(Value::as_str) == Some("person")
        && let Some(rt) = pv(spec.get("person"), period)
    {
        basis = "person";
        rate = rt;
        let trips = units * rt;
        person = Some(trips);
        veh = if occ > 0.0 { trips * car / occ } else { 0.0 };
    } else {
        basis = "vehicle";
        let rt = pv(spec.get("veh"), period).unwrap_or_else(|| rates.rate.num());
        if pv(spec.get("veh"), period).is_none() {
            rate_raw = num_json(&rates.rate);
        }
        rate = rt;
        veh = units * rt;
        person = if car > 0.0 { Some(veh * occ / car) } else { None };
    }
    let custom = basis == "person" || pv(spec.get("veh"), period).is_some();
    let (source, year) = rate_source(spec, meta, custom);
    let in_pct = pv(spec.get("inPct"), period).unwrap_or_else(|| rates.in_pct.num());
    let in_pct_raw = if pv(spec.get("inPct"), period).is_none() { num_json(&rates.in_pct) } else { None };
    let fr = or_default(pv(spec.get("freight"), period).unwrap_or(f64::NAN), 0.0);
    let own_hv = spec.get("hvPct").filter(|h| given(Some(h)) && value_to_number(h).is_finite());
    let hv_pct = own_hv.map_or_else(|| s.hv_pct.num(), value_to_number);
    let hv_pct_raw = if own_hv.is_none() { num_json(&s.hv_pct) } else { None };
    Some(TripRow {
        k: key.to_string(),
        label: lu_def.label.to_string(),
        group: lu_def.group.to_string(),
        q: Some(qty),
        unit: lu_def.unit.to_string(),
        basis,
        rate: Some(rate),
        rate_unit: format!("{} per {} per hour", if basis == "person" { "person trips" } else { "veh" }, unit_one(lu_def)),
        source,
        year,
        flag: rate_flag(year.unwrap_or(0.0)),
        person,
        split: Some(sp),
        occ: Some(occ),
        occ_basis: Some(oc.basis),
        veh,
        in_pct,
        vin: veh * in_pct / 100.0,
        vout: veh * (1.0 - in_pct / 100.0),
        freight: units * fr,
        freight_rate: fr,
        hv_pct,
        hours,
        drive_through: drive,
        own_rate: custom,
        adj: Adj::default(),
        net_in: 0.0,
        net_out: 0.0,
        net: 0.0,
        hv: 0.0,
        rate_raw,
        in_pct_raw,
        hv_pct_raw,
        occ_raw: if oc.basis == "assumed" { cfg.get("eduOcc").cloned() } else { None },
    })
}

// Trips entered directly (the user's z.extra; while the engine loads the table through z.extra, they wait in tg.other); None when there are none
fn other_row(z: &Zone, s: &Settings, period: &str) -> Option<TripRow> {
    let (xin, xout) = other_trips(z, period);
    if !(truthy_number(xin) || truthy_number(xout)) {
        return None;
    }
    Some(TripRow {
        k: "other".into(),
        label: "Other trips (entered directly)".into(),
        group: "Other".into(),
        q: None,
        unit: String::new(),
        basis: "vehicle",
        rate: None,
        rate_unit: "veh/h".into(),
        source: "entered directly".into(),
        year: None,
        flag: None,
        person: None,
        split: None,
        occ: None,
        occ_basis: None,
        veh: xin + xout,
        in_pct: if xin + xout > 0.0 { 100.0 * xin / (xin + xout) } else { 50.0 },
        vin: xin,
        vout: xout,
        freight: 0.0,
        freight_rate: 0.0,
        hv_pct: s.hv_pct.num(),
        hours: String::new(),
        drive_through: false,
        own_rate: false,
        adj: Adj::default(),
        net_in: 0.0,
        net_out: 0.0,
        net: 0.0,
        hv: 0.0,
        rate_raw: None,
        in_pct_raw: None,
        hv_pct_raw: num_json(&s.hv_pct),
        occ_raw: None,
    })
}

/// `tripGen(z, S, period, sc)`: trip generation for one zone and period.
pub fn trip_gen(z: &Zone, s: &Settings, period: &str, sc: &Scenario) -> TripGen {
    let on = tg_on(z).is_some();
    let tg_val = if on { z.tg.get().cloned().unwrap_or_else(|| json!({})) } else { json!({}) };
    let cfg = conf(s);
    let mut rows: Vec<TripRow> = LANDUSES.iter().filter_map(|lu_def| land_use_row(z, s, period, &cfg, &tg_val, lu_def)).collect();
    rows.extend(other_row(z, s, period));
    let tot = |get: &dyn Fn(&TripRow) -> f64| rows.iter().fold(0.0, |acc, row| acc + get(row));
    let gross = Dir { r#in: tot(&|row| row.vin), out: tot(&|row| row.vout) };
    let eg = if on { tg_val.get("existing").filter(|x| value_truthy(x)).cloned().unwrap_or_else(|| json!({})) } else { json!({}) };
    let (ekin, ekout) = (format!("{period}in"), format!("{period}out"));
    let existing_raw = Dir {
        r#in: math_max(0.0, plus0(eg.get(&ekin).map_or(f64::NAN, value_to_number))),
        out: math_max(0.0, plus0(eg.get(&ekout).map_or(f64::NAN, value_to_number))),
    };
    let existing = Dir { r#in: math_min(gross.r#in, existing_raw.r#in), out: math_min(gross.out, existing_raw.out) };
    let pct = |share: f64| clamp(plus0(share), 0.0, 100.0) / 100.0;
    let i_p = if on { pct(tg_val.get("internal").map_or(f64::NAN, value_to_number)) } else { 0.0 };
    let p_p = if on { pct(tg_val.get("passby").map_or(f64::NAN, value_to_number)) } else { 0.0 };
    let t_p = pct(z.reduction.num());
    let after_e = gross.sub(existing);
    let internal = after_e.mul(i_p);
    let after_i = after_e.sub(internal);
    let passby = after_i.mul(p_p);
    let after_p = after_i.sub(passby);
    let tdm = after_p.mul(t_p);
    let net = after_p.sub(tdm);
    let fr = tot(&|row| row.freight);
    let freight = Dir { r#in: fr / 2.0, out: fr / 2.0 };
    let lost = t_p >= 1.0 && fr > 0.0;
    let open = !(z.year.num() > 0.0 && year_of(Some(sc), s) < z.year.num());
    let stage = stage_of(Some(z), Some(sc));
    let kf = if open { stage } else { 0.0 };
    let loaded = Dir { r#in: (net.r#in + if lost { 0.0 } else { freight.r#in }) * kf, out: (net.out + if lost { 0.0 } else { freight.out }) * kf };
    for row in &mut rows {
        let si = if gross.r#in > 0.0 { row.vin / gross.r#in } else { 0.0 };
        let so = if gross.out > 0.0 { row.vout / gross.out } else { 0.0 };
        let both = |dir: Dir| dir.r#in * si + dir.out * so;
        row.adj = Adj { existing: both(existing), internal: both(internal), passby: both(passby), tdm: both(tdm) };
        row.net_in = net.r#in * si;
        row.net_out = net.out * so;
        row.net = row.net_in + row.net_out;
        row.hv = row.net * row.hv_pct / 100.0 + row.freight;
    }
    TripGen {
        zone: z.id.clone(),
        name: z.name.as_str().map_or(Value::Null, |name| json!(name)),
        cls: zone_class(Some(z)),
        period: period.to_string(),
        on,
        rows,
        gross,
        existing,
        existing_raw,
        internal,
        passby,
        after_p,
        tdm,
        net,
        freight,
        lost,
        open,
        stage,
        loaded,
        pct: (i_p * 100.0, p_p * 100.0, t_p * 100.0),
    }
}

fn trips_text(m: Option<&OrderedMap<Num>>) -> String {
    serde_json::to_string(&m.map_or(Value::Null, |m| serde_json::to_value(m).unwrap_or(Value::Null))).unwrap_or_default()
}

fn state_text(z: &Zone) -> String {
    format!("[{},{}]", trips_text(z.extra_trips.get()), trips_text(z.tg_load.get()))
}

fn load_map(pairs: &[(String, f64)]) -> OrderedMap<Num> {
    pairs.iter().map(|(k, v)| (k.clone(), Num::number(*v))).collect()
}

/// `tgNative()`: whether the engine reads a zone's `tgLoad` in place of land use times rates plus `extra`. The current engine does.
pub const TG_NATIVE: bool = true;

// z.tg as an object, made {} first when it is anything else
fn tg_obj(z: &mut Zone) -> &mut Map<String, Value> {
    object_of(&mut z.tg)
}

fn object_of(tg: &mut Maybe<Value>) -> &mut Map<String, Value> {
    match tg {
        Maybe::Set(Value::Object(o)) => o,
        other => {
            // Now an object, so the second call returns at once
            *other = Maybe::Set(Value::Object(Map::new()));
            object_of(other)
        }
    }
}

// `{ ...z.tg.other }` as a trips map
fn other_as_map(other: &Value) -> OrderedMap<Num> {
    let mut m = OrderedMap::new();
    if let Some(o) = other.as_object() {
        for (k, v) in o {
            m.insert(k.clone(), Maybe::Set(JsNum::from(v.clone())));
        }
    }
    m
}

/// `sync(z, S, sc)`: loads a zone's table into the model. Returns whether the zone's `extra` or `tgLoad` changed.
///
/// `load[p] = T4 after pass-by + freight / (1 - r)`, so the engine's `load * (1 - r) * build-out` is `(net + freight) * build-out`.
pub fn sync(z: &mut Zone, s: &Settings, sc: &Scenario) -> bool {
    if z.kind.is("gateway") || tg_on(z).is_none() {
        return false;
    }
    let native = TG_NATIVE;
    let state = sync_state(z);
    let before = state_text(z);
    // move the user's own trips to where this route keeps them before the table is worked out
    if native && state.as_deref() != Some("tgLoad") {
        if state.as_deref() == Some("extra") {
            let other = z.tg.get().and_then(|table| table.get("other")).filter(|own| value_truthy(own)).cloned();
            match other {
                Some(own) => z.extra_trips = Maybe::Set(other_as_map(&own)),
                None => z.extra_trips = Maybe::Missing,
            }
        }
        tg_obj(z).remove("other");
    }
    if !native && state.as_deref() != Some("extra") {
        // The prototype's `Object.keys(e).length || true` keeps every map, empty or not
        let other = z.extra_trips.get().map_or(Value::Null, |trips| serde_json::to_value(trips).unwrap_or(Value::Null));
        tg_obj(z).insert("other".into(), other);
    }
    tg_obj(z).insert("synced".into(), json!(if native { "tgLoad" } else { "extra" }));
    let mut load: Vec<(String, f64)> = Vec::new();
    let mut ex: Vec<(String, f64)> = Vec::new();
    let red = clamp(plus0(z.reduction.num()), 0.0, 100.0) / 100.0;
    let fk = if red < 1.0 { 1.0 / (1.0 - red) } else { 0.0 };
    for per in PERIODS {
        let tg = trip_gen(z, s, per, sc);
        let (mut lu_in, mut lu_out) = (0.0f64, 0.0f64);
        for lu_def in &LANDUSES {
            let qty = or_default(z.lu.get().map_or(f64::NAN, |lu| lu.num_at(lu_def.key).num()), 0.0);
            if qty == 0.0 {
                continue;
            }
            let Some(rates) = s.rates.get().and_then(|m| m.get(lu_def.key)).and_then(|m| m.get()).and_then(|m| m.get(per)).and_then(|m| m.get()) else { continue };
            let tt = qty / lu_def.per * rates.rate.num();
            lu_in += tt * rates.in_pct.num() / 100.0;
            lu_out += tt * (1.0 - rates.in_pct.num() / 100.0);
        }
        let (lin, lout) = (tg.after_p.r#in + tg.freight.r#in * fk, tg.after_p.out + tg.freight.out * fk);
        load.push((format!("{per}in"), lin));
        load.push((format!("{per}out"), lout));
        ex.push((format!("{per}in"), lin - lu_in));
        ex.push((format!("{per}out"), lout - lu_out));
    }
    if native {
        z.tg_load = Maybe::Set(load_map(&load));
    } else {
        z.tg_load = Maybe::Missing;
        z.extra_trips = Maybe::Set(load_map(&ex));
    }
    state_text(z) != before
}

/// `syncAll()`: syncs every zone with its table on in every scenario; returns how many changed.
pub fn sync_all(p: &mut Project) -> usize {
    let s = p.settings.clone();
    let mut n = 0usize;
    for sc in &mut p.scenarios {
        let ids: Vec<String> = sc.zones.iter().filter(|(_, z)| tg_on(z).is_some()).map(|(k, _)| k.to_string()).collect();
        for id in ids {
            // The ids were read from these zones just above
            let Some(mut z) = sc.zones.get(&id).cloned() else { continue };
            if sync(&mut z, &s, sc) {
                // Bound: a count of zones held in memory, far below usize::MAX
                n = n.saturating_add(1);
            }
            sc.zones.insert(id, z);
        }
    }
    n
}

/// `setTable(z, on, S, sc)`: switches a zone's table on (the user's `extra` stays as "other trips") or off (`extra` is the user's again).
pub fn set_table(z: &mut Zone, on: bool, s: &Settings, sc: &Scenario) {
    if !z.tg.get().is_some_and(value_truthy) {
        z.tg = Maybe::Set(json!({}));
    }
    let is_on = tg_on(z).is_some();
    if on && !is_on {
        let t = tg_obj(z);
        t.remove("other");
        t.remove("synced");
        t.insert("on".into(), json!(true));
        sync(z, s, sc);
    } else if !on && is_on {
        if sync_state(z).as_deref() == Some("extra") {
            let other = z.tg.get().and_then(|t| t.get("other")).filter(|o| value_truthy(o)).cloned();
            match other {
                Some(o) => z.extra_trips = Maybe::Set(other_as_map(&o)),
                None => z.extra_trips = Maybe::Missing,
            }
        }
        let t = tg_obj(z);
        t.insert("on".into(), json!(false));
        t.remove("other");
        t.remove("synced");
        z.tg_load = Maybe::Missing;
    }
}

/// `tableZones(sc, scope)`: the ids of the zones the tables cover: development zones (scope "dev"), or every land-use zone (scope "all").
#[must_use]
pub fn table_zones(sc: &Scenario, scope: &str) -> Vec<String> {
    sc.zones.values().filter(|z| !z.kind.is("gateway") && (scope == "all" || zone_class(Some(z)) == "development")).map(|z| z.id.clone()).collect()
}

/// A rate flag with the zone and activity it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct ZoneFlag {
    /// Zone name.
    pub zone: Value,
    /// Activity name.
    pub activity: String,
    /// "warn" or "review".
    pub level: &'static str,
    /// What to do.
    pub text: String,
}

/// The RR 422 check for a period: residential peak-hour vehicle trips per dwelling.
#[derive(Clone, Debug, PartialEq)]
pub struct Sanity {
    /// "AM" or "PM".
    pub period: &'static str,
    /// Dwellings (houses and apartments).
    pub dwellings: f64,
    /// Net vehicle trips (1 decimal).
    pub veh: f64,
    /// Trips per dwelling (3 decimals), None when there are no dwellings.
    pub rate: Option<f64>,
    /// Within the band, None when there are no dwellings.
    pub ok: Option<bool>,
}

/// One zone's trip generation in both periods.
#[derive(Clone, Debug, PartialEq)]
pub struct ZoneTrips {
    /// The AM trips.
    pub am: TripGen,
    /// The PM trips.
    pub pm: TripGen,
}

impl ZoneTrips {
    /// The trips of a period.
    #[must_use]
    pub fn of(&self, period: &str) -> &TripGen {
        if period == "AM" { &self.am } else { &self.pm }
    }
}

/// The data of the trip generation tables (`tables`): per zone and period, the rate flags and the RR 422 check.
#[derive(Clone, Debug, PartialEq)]
pub struct Tables {
    /// Ids of the zones covered.
    pub zones: Vec<String>,
    /// Each zone's trips.
    pub tg: Vec<ZoneTrips>,
    /// Rate flags, once per zone and activity.
    pub flags: Vec<ZoneFlag>,
    /// The RR 422 check for AM and PM.
    pub sanity: Vec<Sanity>,
}

impl Tables {
    /// The tables as JSON (zone ids, trips, flags and the check).
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!({
            "zones": self.zones,
            "tg": self.tg.iter().map(|t| json!({ "AM": t.am.to_value(), "PM": t.pm.to_value() })).collect::<Vec<_>>(),
            "flags": self.flags.iter().map(|f| json!({ "zone": f.zone, "activity": f.activity, "level": f.level, "text": f.text })).collect::<Vec<_>>(),
            "sanity": self.sanity.iter().map(|s| json!({ "period": s.period, "dwellings": json_number(s.dwellings), "veh": json_number(s.veh), "rate": s.rate.map_or(Value::Null, json_number), "ok": s.ok })).collect::<Vec<_>>(),
        })
    }
}

/// What the tables cover: some zones by id, or a scope ("dev" or "all"; None reads the project's `landuse.scope`, else "dev").
#[derive(Clone, Debug, Default)]
pub struct TablesOpts {
    /// The zones to cover.
    pub zones: Option<Vec<String>>,
    /// "dev" or "all".
    pub scope: Option<String>,
}

/// The tables' scope of a project: `P.landuse.scope`, else "dev".
pub fn project_scope(p: &Project) -> String {
    p.extra.get("landuse").and_then(|l| l.get("scope")).filter(|x| value_truthy(x)).map_or("dev".to_string(), value_to_js_string)
}

/// `tables(sc, S, opts)`: the trip generation for the development zones (or every land-use zone) in both periods, the rate flags and the RR 422 check.
#[must_use]
pub fn tables(p: &Project, sc: &Scenario, s: &Settings, opts: &TablesOpts) -> Tables {
    let zones = opts.zones.clone().unwrap_or_else(|| table_zones(sc, &opts.scope.clone().unwrap_or_else(|| project_scope(p))));
    // A zone id the scenario does not hold is left out of both lists, so they stay in step
    let zones: Vec<String> = zones.into_iter().filter(|id| sc.zones.contains_key(id)).collect();
    let tg: Vec<ZoneTrips> = zones
        .iter()
        .filter_map(|id| sc.zones.get(id))
        .map(|z| ZoneTrips { am: trip_gen(z, s, "AM", sc), pm: trip_gen(z, s, "PM", sc) })
        .collect();
    let mut flags: Vec<ZoneFlag> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for t in &tg {
        for r in &t.am.rows {
            let key = format!("{}{}", t.am.zone, r.k);
            if let Some(f) = &r.flag
                && !seen.contains(&key)
            {
                seen.push(key);
                flags.push(ZoneFlag { zone: t.am.name.clone(), activity: r.label.clone(), level: f.level, text: f.text.clone() });
            }
        }
    }
    // RR 422 sanity: residential peak-hour vehicle trips per dwelling (houses and apartments)
    let sanity = PERIODS
        .iter()
        .map(|per| {
            let (mut veh, mut dw) = (0.0f64, 0.0f64);
            for t in &tg {
                for r in &t.of(per).rows {
                    if r.k == "residential" || r.k == "apartment" {
                        veh += r.net;
                        dw += r.q.unwrap_or(f64::NAN);
                    }
                }
            }
            let rate = if dw > 0.0 { Some(veh / dw) } else { None };
            Sanity { period: per, dwellings: dw, veh: r1(veh), rate: rate.map(r3), ok: rate.map(|r| (rr422::BAND[0]..=rr422::BAND[1]).contains(&r)) }
        })
        .collect();
    Tables { zones, tg, flags, sanity }
}

/// `csv(sc, S, opts)`: `trip_generation.csv`, one row per zone, period and activity.
pub fn csv(p: &Project, sc: &Scenario, s: &Settings, opts: &TablesOpts) -> String {
    let t = tables(p, sc, s, opts);
    let mut out: Vec<Vec<Value>> = vec![consts::CSV_HEAD.iter().map(|h| json!(h)).collect()];
    let f = |x: Option<f64>| -> Value {
        match x {
            Some(v) if v.is_finite() => json_number(r2(v)),
            _ => json!(""),
        }
    };
    for zt in &t.tg {
        for per in PERIODS {
            for r in &zt.of(per).rows {
                let mut row: Vec<Value> = vec![
                    json!(zt.am.zone),
                    json!(r.k),
                    r.q.map_or(Value::Null, json_number),
                    json!(r.unit),
                    json!(per),
                    f(r.rate),
                    json!(r.rate_unit),
                    json!(r.source),
                    r.year.map_or(json!(""), json_number),
                    f(r.person),
                ];
                for m in MODES {
                    row.push(match &r.split {
                        Some(sp) => f(Some(sp.get(m))),
                        None => json!(""),
                    });
                }
                row.extend([f(r.occ), f(Some(r.veh)), f(Some(r.adj.existing)), f(Some(r.adj.internal)), f(Some(r.adj.passby)), f(Some(r.adj.tdm)), f(Some(r.net)), f(Some(r.in_pct)), f(Some(r.net_in)), f(Some(r.net_out)), f(Some(r.hv))]);
                out.push(row);
            }
        }
    }
    util::csv(&out)
}

/// A mode as a share of a split, for callers that read it by key.
#[must_use]
pub fn mode_share(sp: &Split, m: &str) -> f64 {
    sp.get(m)
}
