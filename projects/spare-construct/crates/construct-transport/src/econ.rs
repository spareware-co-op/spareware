//! Appraisal from engine.js.
//!
//! Capital cost, MBCM discounting (NZTA General Circular 25/01 schedule and sensitivity
//! rate), analysis period rules, IPM efficiency bands, BCR text, the signal to noise of a
//! scenario difference and the indicative economics of an option against its base. Port of
//! engine.js, "appraisal", "MBCM discounting" and `econ`.
//!
//! Results are passed explicitly: `econ` takes each scenario with its [`Results`] (what
//! engine.js reads as `sc.res`), and the settings it reads as `Store.S`.
//! The standards (mbcm, mbcmUpdate, ipm, noise) come from crate::standards.

// `!(x > 0)` is the JavaScript's own test: unlike `x <= 0` it is true for NaN
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::jsnum::{
    Maybe, Num, js_trim, json_number, math_max, math_min, math_round, number_to_js_string,
    or_default, plain_number, to_fixed, to_int32, value_to_number,
};
use crate::project::{EconSettings, OrderedMap, Scenario, Settings};
use crate::results::{Res, Results, js_sort_by, plain};
use crate::standards::{STANDARDS, year_of};
use crate::util::fmt;
use serde::Serialize;
use serde_json::{Value, json};
use std::borrow::Cow;

/// The default discount schedule as engine.js holds it ([{ to: 30, r: 2 }, ...]).
pub fn mbcm_rates() -> Value {
    Value::Array(
        STANDARDS
            .mbcm
            .rates
            .iter()
            .map(|d| json!({ "to": d.to.map(json_number), "r": json_number(d.r) }))
            .collect(),
    )
}

// Store.S.econ; a settings object without the block reads as an empty one (the JavaScript would throw)
fn econ_settings(s: &Settings) -> Cow<'_, EconSettings> {
    match s.econ.get() {
        Some(e) => Cow::Borrowed(e),
        None => Cow::Owned(serde_json::from_value(json!({})).expect("an empty econ block")),
    }
}

// obj[key] ?? d for a loosely typed settings map, read as a number
fn map_num_or(m: &Maybe<OrderedMap<Num>>, key: &str, d: f64) -> f64 {
    match m.get().and_then(|m| m.get(key)) {
        Some(v) if !v.is_nullish() => v.num(),
        _ => d,
    }
}

// The key `obj[x]` looks up for a text field that may be missing or null
fn key_text(x: &Maybe<String>) -> &str {
    match x {
        Maybe::Set(s) => s,
        Maybe::Null => "null",
        Maybe::Missing => "undefined",
    }
}

/* ---------------- capital cost ---------------- */

/// What `Engine.capital(sc)` returns: capital cost (NZ$m) of the proposed links and upgraded junctions.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Capital {
    /// Links plus junctions.
    #[serde(with = "plain")]
    pub total: f64,
    /// Proposed links: km times lanes times the class's cost per lane-km.
    #[serde(with = "plain")]
    pub links: f64,
    /// Upgraded junctions by control.
    #[serde(with = "plain")]
    pub nodes: f64,
    /// Length of the proposed links (km).
    #[serde(with = "plain")]
    pub km: f64,
}

/// `Engine.capital(sc)` under the settings s: proposed links at `econ.laneKm` per lane-km (3 for a class without one) and upgraded junctions at `econ.node` by control (0 for none).
///
/// A settings object without `econ.laneKm` or `econ.node` reads as empty maps (the JavaScript
/// throws when it needs them). Junction prices are added as numbers; the JavaScript's `+=`
/// joins a price saved as text onto the total as text ("0" + "2" is "02").
pub fn capital(sc: &Scenario, s: &Settings) -> Capital {
    let e = econ_settings(s);
    let (mut links, mut nodes, mut km) = (0.0, 0.0, 0.0);
    for l in sc.links.values() {
        if l.status.is("proposed") && sc.nodes.contains_key(&l.a) && sc.nodes.contains_key(&l.b) {
            let k = sc.link_len(l) / 1000.0;
            km += k;
            let lanes = to_int32(l.lanes_ab.num()) as f64 + to_int32(l.lanes_ba.num()) as f64;
            links += k * lanes * map_num_or(&e.lane_km, key_text(&l.cls), 3.0);
        }
    }
    for n in sc.nodes.values() {
        if n.upgrade.truthy() {
            nodes += map_num_or(&e.node, key_text(&n.control), 0.0);
        }
    }
    Capital {
        total: links + nodes,
        links,
        nodes,
        km,
    }
}

/* ---------------- MBCM discounting ---------------- */

/// `Engine.uspwf(i, n)`: present worth of a constant benefit of 1 a year for n years, continuously discounted at rate i.
pub fn uspwf(i: f64, n: f64) -> f64 {
    if !(n > 0.0) {
        return 0.0;
    }
    if !(i > 0.0) {
        return n;
    }
    (1.0 - crate::jsmath::pow(1.0 + i, -n)) / (1.0 + i).ln()
}

/// `Engine.agpwf(i, n)`: present worth of a benefit growing by 1 a year for n years, continuously discounted at rate i.
pub fn agpwf(i: f64, n: f64) -> f64 {
    if !(n > 0.0) {
        return 0.0;
    }
    if !(i > 0.0) {
        return n * n / 2.0;
    }
    let l = (1.0 + i).ln();
    let v = crate::jsmath::pow(1.0 + i, -n);
    1.0 / (l * l) - n * v / l - v / (l * l)
}

/// Present worth factors of a discount schedule (engine.js pwf).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Pwf {
    /// Uniform series factor.
    #[serde(with = "plain")]
    pub uspwf: f64,
    /// Arithmetic gradient factor.
    #[serde(with = "plain")]
    pub agpwf: f64,
}

struct Seg {
    to: f64,
    r: f64,
}

// s.to == null || s.to === '' ? Infinity : +s.to (read off an object; other values have no "to")
fn seg_to(s: &Value) -> f64 {
    match s.get("to") {
        None | Some(Value::Null) => f64::INFINITY,
        Some(Value::String(t)) if t.is_empty() => f64::INFINITY,
        Some(v) => value_to_number(v),
    }
}

// +s.r (undefined reads as NaN)
fn seg_r(s: &Value) -> f64 {
    s.get("r").map_or(f64::NAN, value_to_number)
}

// typeof s === 'object' && s: objects and arrays
fn is_js_object(v: &Value) -> bool {
    v.is_object() || v.is_array()
}

/// `Engine.pwf(rates, n)`: present worth factors over n years of a schedule ([{ to, r }], to null = onwards) or a single rate (%).
pub fn pwf(rates: &Value, n: f64) -> Pwf {
    let mut segs: Vec<Seg> = match rates {
        Value::Array(a) => a
            .iter()
            .filter(|s| is_js_object(s))
            .map(|s| Seg {
                to: seg_to(s),
                r: seg_r(s) / 100.0,
            })
            .collect(),
        // [{ to: n, r: +rates || 0 }]
        other => vec![Seg {
            to: n,
            r: or_default(value_to_number(other), 0.0) / 100.0,
        }],
    };
    segs.retain(|s| s.to > 0.0 && s.r.is_finite());
    js_sort_by(&mut segs, |a, b| a.to - b.to);
    if segs.is_empty() {
        segs.push(Seg { to: n, r: 0.0 });
    }
    let (mut t0, mut d, mut u1, mut a1) = (0.0, 1.0, 0.0, 0.0);
    let last = segs.len() - 1;
    for (k, seg) in segs.iter().enumerate() {
        if !(t0 < n) {
            break;
        }
        let end = if k == last { n } else { math_min(n, seg.to) };
        let len = end - t0;
        let i = seg.r;
        if len <= 0.0 {
            continue;
        }
        let u = uspwf(i, len);
        u1 += d * u;
        a1 += d * (t0 * u + agpwf(i, len));
        d *= crate::jsmath::pow(1.0 + i, -len);
        t0 = end;
    }
    Pwf {
        uspwf: u1,
        agpwf: a1,
    }
}

/// The analysis period engine.js econPeriod settles on.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EconPeriod {
    /// Years used.
    #[serde(with = "plain")]
    pub years: f64,
    /// The period asked for is allowed.
    pub ok: bool,
    /// Why not ("" when ok).
    pub why: String,
    /// A period over 40 years: report a 40-year comparison too (left out otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compare40: Option<bool>,
}

// String(x || '').trim() is not empty
fn has_text(x: &Num) -> bool {
    x.truthy() && !js_trim(&x.js_string()).is_empty()
}

/// `Engine.econPeriod(E)`: 40 years by default; up to 60 with a justification note; more than 60 only with NZTA's written approval.
pub fn econ_period(e: &EconSettings) -> EconPeriod {
    let want = math_round(or_default(e.years.num(), STANDARDS.mbcm.years));
    let (my, ly) = (STANDARDS.mbcm.years, STANDARDS.mbcm.long_years);
    let long_ok = |years| EconPeriod {
        years,
        ok: true,
        why: String::new(),
        compare40: Some(true),
    };
    if want <= my {
        return EconPeriod {
            years: math_max(1.0, want),
            ok: true,
            why: String::new(),
            compare40: None,
        };
    }
    if want <= ly {
        return if has_text(&e.long_life_note) {
            long_ok(want)
        } else {
            EconPeriod {
                years: my,
                ok: false,
                why: format!(
                    "A {}-year period needs a justification note (long-life assets). {} years were used.",
                    number_to_js_string(want),
                    number_to_js_string(my)
                ),
                compare40: None,
            }
        };
    }
    if has_text(&e.approval) {
        return long_ok(want);
    }
    let used = if has_text(&e.long_life_note) { ly } else { my };
    EconPeriod {
        years: used,
        ok: false,
        why: format!(
            "More than {} years needs NZTA's written approval, recorded in the economics settings. {} years were used.",
            number_to_js_string(ly),
            number_to_js_string(used)
        ),
        compare40: None,
    }
}

/// One edition's IPM efficiency band of a BCR (a row of engine.js ipmBand).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct IpmRating {
    /// Edition id.
    pub edition: String,
    /// A draft edition.
    pub draft: bool,
    /// "VL", "L", "M" or "H"; null without a BCR.
    pub band: Option<String>,
    /// "Very low" ... or "No BCR".
    pub label: String,
    /// Note ("" when none).
    pub note: String,
    /// Edition name (left out without a BCR).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Edition link (left out without a BCR).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Edition checked against the source (left out without a BCR).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
}

/// `Engine.ipmBand(bcr, edition)`: the IPM band of a BCR in each edition (or only the one named), banded on the unrounded value.
pub fn ipm_band(bcr: Option<f64>, edition: Option<&str>) -> Vec<IpmRating> {
    STANDARDS
        .ipm
        .iter()
        .filter(|e| match edition {
            Some(id) if !id.is_empty() => e.src.id == id,
            _ => true,
        })
        .map(|e| {
            let note = e.note.unwrap_or("").to_string();
            match bcr.filter(|x| x.is_finite()) {
                None => IpmRating {
                    edition: e.src.id.to_string(),
                    draft: e.draft,
                    band: None,
                    label: "No BCR".to_string(),
                    note,
                    source: None,
                    url: None,
                    verified: None,
                },
                Some(x) => {
                    let b = e
                        .bands
                        .iter()
                        .find(|b| x < b.below)
                        .expect("the last band has no upper bound");
                    IpmRating {
                        edition: e.src.id.to_string(),
                        draft: e.draft,
                        band: Some(b.band.to_string()),
                        label: b.label.to_string(),
                        note,
                        source: Some(e.src.name.to_string()),
                        url: e.src.url.map(str::to_string),
                        verified: Some(e.src.verified),
                    }
                }
            }
        })
        .collect()
}

/// `Engine.bcrText(x)`: one decimal below 10, a whole number from 10 (12.4 is "12"), "–" when none.
pub fn bcr_text(x: Option<f64>) -> String {
    match x {
        Some(x) if x.is_finite() => {
            if x.abs() >= 10.0 {
                number_to_js_string(math_round(x))
            } else {
                to_fixed(x, 1)
            }
        }
        _ => fmt::DASH.to_string(),
    }
}

/* ---------------- signal to noise ---------------- */

/// What a re-run with a tighter target would use.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tighter {
    /// Gap target.
    #[serde(with = "plain")]
    pub gap_tol: f64,
    /// Iteration limit.
    #[serde(with = "plain")]
    pub max_iter: f64,
}

/// Signal to noise of a scenario difference (engine.js snr).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Snr {
    /// (|change in VHT| / base VHT) / max(base gap, option gap); Infinity when both gaps are 0.
    #[serde(rename = "R", with = "plain")]
    pub r: f64,
    /// R is at least the minimum.
    pub ok: bool,
    /// The minimum (TAG M3.1).
    #[serde(with = "plain")]
    pub min: f64,
    /// The warning ("" when ok).
    pub text: String,
    /// A tighter target to re-run with.
    pub tighter: Tighter,
}

/// `Engine.snr(bRes, oRes)`: signal to noise of the VHT difference between two runs; None without both results or a base VHT. s is `Store.S` (for the tighter gap target).
pub fn snr(b: Option<&Res>, o: Option<&Res>, s: &Settings) -> Option<Snr> {
    let (b, o) = (b?, o?);
    if !(b.kpi.vht > 0.0) {
        return None;
    }
    let g = math_max(or_default(b.gap, 0.0), or_default(o.gap, 0.0));
    let d = (b.kpi.vht - o.kpi.vht).abs() / b.kpi.vht;
    let r = if g > 0.0 { d / g } else { f64::INFINITY };
    let min = STANDARDS.noise.min;
    let text = if r >= min {
        String::new()
    } else {
        let shown = if r < 10.0 {
            to_fixed(r, 1)
        } else {
            number_to_js_string(math_round(r))
        };
        format!(
            "Difference may be assignment noise (signal-to-noise {}\u{d7}; TAG M3.1 asks for \u{2265}{}\u{d7}). Re-run both with a tighter convergence target.",
            shown,
            number_to_js_string(min)
        )
    };
    // Math.min(Store.S.gapTol || 1, ...)
    let tol = if s.gap_tol.truthy() {
        s.gap_tol.num()
    } else {
        1.0
    };
    Some(Snr {
        r,
        ok: r >= min,
        min,
        text,
        tighter: Tighter {
            gap_tol: math_min(tol, STANDARDS.noise.tighter_gap_tol),
            max_iter: STANDARDS.noise.tighter_max_iter,
        },
    })
}

/// A signal to noise with the period it is for (econ's `snr`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PeriodSnr {
    /// Period.
    pub period: String,
    /// The signal to noise.
    #[serde(flatten)]
    pub snr: Snr,
}

/* ---------------- indicative economics ---------------- */

/// A standard's reference in the economics output.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StdRef {
    /// Name.
    pub name: String,
    /// Edition.
    pub edition: String,
    /// Link.
    pub url: String,
    /// Checked against the source.
    pub verified: bool,
}

fn std_ref(src: &crate::standards::Source) -> StdRef {
    StdRef {
        name: src.name.to_string(),
        edition: src.edition.to_string(),
        url: src.url.unwrap_or("").to_string(),
        verified: src.verified,
    }
}

/// What `Engine.econ(baseSc, optSc, period)` returns when both have results.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Econ {
    /// The two scenarios are modelled for different years: no benefits.
    Mismatch(EconMismatch),
    /// Older projects: flat benefits at the saved discount rate.
    Uniform(Box<EconUniform>),
    /// MBCM v1.7.5 with the GC 25/01 schedule.
    Mbcm(Box<EconMbcm>),
}

impl Econ {
    /// "mbcm" or "uniform"; None for a year mismatch.
    pub fn method(&self) -> Option<&str> {
        match self {
            Econ::Mismatch(_) => None,
            Econ::Uniform(u) => Some(&u.method),
            Econ::Mbcm(m) => Some(&m.method),
        }
    }
}

/// Economics between scenarios modelled for different years.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EconMismatch {
    /// "year".
    pub mismatch: String,
    /// The base's year.
    #[serde(with = "plain")]
    pub base_year: f64,
    /// The option's year.
    #[serde(with = "plain")]
    pub opt_year: f64,
    /// null.
    #[serde(with = "plain")]
    pub dvht: Option<f64>,
    /// null.
    #[serde(with = "plain")]
    pub annual: Option<f64>,
    /// null.
    #[serde(with = "plain")]
    pub pv: Option<f64>,
    /// Capital cost of the option over the base (NZ$m).
    #[serde(with = "plain")]
    pub cap: f64,
    /// null.
    #[serde(with = "plain")]
    pub bcr: Option<f64>,
    /// null.
    #[serde(with = "plain")]
    pub dco2: Option<f64>,
    /// Periods with both results.
    #[serde(with = "plain")]
    pub periods: f64,
    /// MBCM label.
    pub label: String,
    /// What to do.
    pub note: String,
}

/// Economics by the older uniform method (flat benefits).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EconUniform {
    /// "uniform".
    pub method: String,
    /// Average VHT saved per modelled peak hour.
    #[serde(with = "plain")]
    pub dvht: f64,
    /// Annual benefit (NZ$m).
    #[serde(with = "plain")]
    pub annual: f64,
    /// Present value at the saved rate (NZ$m).
    #[serde(with = "plain")]
    pub pv: f64,
    /// Present value at the sensitivity rate (NZ$m).
    #[serde(with = "plain")]
    pub pv8: f64,
    /// Capital cost of the option over the base (NZ$m).
    #[serde(with = "plain")]
    pub cap: f64,
    /// Benefit-cost ratio; null without a cost.
    #[serde(with = "plain")]
    pub bcr: Option<f64>,
    /// BCR at the sensitivity rate; null without a cost.
    #[serde(with = "plain")]
    pub bcr8: Option<f64>,
    /// BCR text.
    pub bcr_text: String,
    /// BCR text at the sensitivity rate.
    pub bcr8_text: String,
    /// Sensitivity rate (%).
    #[serde(with = "plain")]
    pub sens_rate: f64,
    /// CO2 saved a year (t).
    #[serde(with = "plain")]
    pub dco2: f64,
    /// Periods with both results.
    #[serde(with = "plain")]
    pub periods: f64,
    /// Years as saved (left out when not saved).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub years: Num,
    /// Discount rate as saved (left out when not saved).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub rate: Num,
    /// The least signal to noise over the periods; null without one.
    pub snr: Option<PeriodSnr>,
    /// MBCM label.
    pub label: String,
    /// Basis text.
    pub basis: String,
    /// GC 25/01.
    pub standard: StdRef,
    /// Warnings.
    pub warnings: Vec<String>,
}

/// Present worth factors of the central and sensitivity schedules.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EconFactors {
    /// Uniform series factor.
    #[serde(with = "plain")]
    pub uspwf: f64,
    /// Arithmetic gradient factor.
    #[serde(with = "plain")]
    pub agpwf: f64,
    /// Uniform series factor at the sensitivity rate.
    #[serde(with = "plain")]
    pub uspwf8: f64,
    /// Arithmetic gradient factor at the sensitivity rate.
    #[serde(with = "plain")]
    pub agpwf8: f64,
}

/// The 40-year comparison of a longer analysis period.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Compare40 {
    /// 40.
    #[serde(with = "plain")]
    pub years: f64,
    /// Present value over 40 years (NZ$m).
    #[serde(with = "plain")]
    pub pv: f64,
    /// BCR over 40 years; null without a cost.
    #[serde(with = "plain")]
    pub bcr: Option<f64>,
}

/// Economics by the MBCM method.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EconMbcm {
    /// "mbcm".
    pub method: String,
    /// Average VHT saved per modelled peak hour.
    #[serde(with = "plain")]
    pub dvht: f64,
    /// VHT saved by period.
    #[serde(with = "plain")]
    pub dvht_by: OrderedMap<f64>,
    /// Year-one benefit (NZ$m).
    #[serde(with = "plain")]
    pub annual: f64,
    /// Present value at the central schedule (NZ$m).
    #[serde(with = "plain")]
    pub pv: f64,
    /// Present value at the sensitivity rate (NZ$m).
    #[serde(with = "plain")]
    pub pv8: f64,
    /// Capital cost of the option over the base (NZ$m).
    #[serde(with = "plain")]
    pub cap: f64,
    /// Benefit-cost ratio; null without a cost.
    #[serde(with = "plain")]
    pub bcr: Option<f64>,
    /// BCR at the sensitivity rate; null without a cost.
    #[serde(with = "plain")]
    pub bcr8: Option<f64>,
    /// Net present value (NZ$m).
    #[serde(with = "plain")]
    pub npv: f64,
    /// Net present value at the sensitivity rate (NZ$m).
    #[serde(with = "plain")]
    pub npv8: f64,
    /// First-year rate of return; null without a cost.
    #[serde(with = "plain")]
    pub fyrr: Option<f64>,
    /// CO2 saved a year (t).
    #[serde(with = "plain")]
    pub dco2: f64,
    /// Periods with both results.
    #[serde(with = "plain")]
    pub periods: f64,
    /// Analysis period used (years).
    #[serde(with = "plain")]
    pub years: f64,
    /// Discount schedule used.
    pub rates: Value,
    /// Sensitivity rate (%).
    #[serde(with = "plain")]
    pub sens_rate: f64,
    /// "commercial" or "non-commercial".
    pub activity: String,
    /// Arithmetic benefit growth (fraction a year).
    #[serde(rename = "R", with = "plain")]
    pub r: f64,
    /// Value of time used (NZ$/veh h).
    #[serde(with = "plain")]
    pub vot: f64,
    /// The value of time was updated by the MBCM factor.
    pub vot_update: bool,
    /// Equivalent hours a year by period.
    pub hours: OrderedMap<Num>,
    /// Present worth factors.
    pub factors: EconFactors,
    /// IPM bands of the BCR.
    pub ipm: Vec<IpmRating>,
    /// IPM bands of the BCR at the sensitivity rate.
    pub ipm8: Vec<IpmRating>,
    /// BCR text.
    pub bcr_text: String,
    /// BCR text at the sensitivity rate.
    pub bcr8_text: String,
    /// The least signal to noise over the periods; null without one.
    pub snr: Option<PeriodSnr>,
    /// Periods with hours that were not modelled.
    pub incomplete: Vec<String>,
    /// The rating is provisional (L*).
    pub lstar: bool,
    /// MBCM label.
    pub label: String,
    /// MBCM disclaimer.
    pub disclaimer: String,
    /// MBCM.
    pub standard: StdRef,
    /// GC 25/01.
    pub discount: StdRef,
    /// Basis text.
    pub basis: String,
    /// Warnings.
    pub warnings: Vec<String>,
    /// The 40-year comparison of a longer period (left out otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compare40: Option<Compare40>,
    /// Value of time note when it was not updated (left out otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vot_note: Option<String>,
}

// segText: the discount schedule in words ("2% years 1–30, then 1.5% to year 100, then 1% from year 101")
fn seg_text(rates: &Value) -> String {
    let list: &[Value] = rates.as_array().map_or(&[], Vec::as_slice);
    let mut s: Vec<Seg> = list
        .iter()
        .filter(|x| is_js_object(x))
        .map(|x| Seg {
            to: seg_to(x),
            r: seg_r(x),
        })
        .collect();
    js_sort_by(&mut s, |a, b| a.to - b.to);
    let mut from = 1.0;
    let mut parts = Vec::new();
    for (k, x) in s.iter().enumerate() {
        let r = number_to_js_string(x.r);
        let t = if !x.to.is_finite() {
            if k > 0 {
                format!("{}% from year {}", r, number_to_js_string(from))
            } else {
                format!("{}% for all years", r)
            }
        } else if k > 0 {
            format!("{}% to year {}", r, number_to_js_string(x.to))
        } else {
            format!("{}% years 1\u{2013}{}", r, number_to_js_string(x.to))
        };
        from = x.to + 1.0;
        parts.push(t);
    }
    parts.join(", then ")
}

// A period's equivalent hours a year (+H[p] || 0)
fn hours_of(h: &OrderedMap<Num>, p: &str) -> f64 {
    h.num_at(p).or(0.0)
}

/// `Engine.econ(baseSc, optSc, period)`: indicative economics of an option against its base, from
/// each scenario's results (base_res, opt_res: `sc.res`) under the settings s (`Store.S`).
///
/// None unless both have a result for the period. Benefits are the VHT saved in the modelled
/// peaks (AM and PM), valued at the value of time over each period's equivalent hours a year,
/// discounted by MBCM (method "mbcm") or flat at the saved rate (older projects).
pub fn econ(
    s: &Settings,
    base_sc: &Scenario,
    base_res: &Results,
    opt_sc: &Scenario,
    opt_res: &Results,
    period: &str,
) -> Option<Econ> {
    let e = econ_settings(s);
    let pairs: Vec<(&Res, &Res, &str)> = ["AM", "PM"]
        .into_iter()
        .filter_map(|p| Some((base_res.get(p)?, opt_res.get(p)?, p)))
        .collect();
    if pairs.is_empty() || base_res.get(period).is_none() || opt_res.get(period).is_none() {
        return None;
    }
    // Benefits only make sense between scenarios modelled for the same year
    let yb = |r: &Res| if r.year > 0.0 { Some(r.year) } else { None };
    let base_year_of = |r: &Res| yb(r).unwrap_or_else(|| year_of(Some(base_sc), s));
    let opt_year_of = |r: &Res| yb(r).unwrap_or_else(|| year_of(Some(opt_sc), s));
    let odd = pairs
        .iter()
        .find(|(b, o, _)| base_year_of(b) != opt_year_of(o));
    let cap = capital(opt_sc, s).total - capital(base_sc, s).total;
    let mut noises: Vec<PeriodSnr> = pairs
        .iter()
        .filter_map(|(b, o, p)| {
            snr(Some(b), Some(o), s).map(|x| PeriodSnr {
                period: p.to_string(),
                snr: x,
            })
        })
        .collect();
    js_sort_by(&mut noises, |a, b| a.snr.r - b.snr.r);
    let noise = noises.into_iter().next();
    let n_pairs = pairs.len() as f64;
    if let Some((b, o, _)) = odd {
        let base_year = base_year_of(b);
        let opt_year = opt_year_of(o);
        return Some(Econ::Mismatch(EconMismatch {
            mismatch: "year".to_string(),
            base_year,
            opt_year,
            dvht: None,
            annual: None,
            pv: None,
            cap,
            bcr: None,
            dco2: None,
            periods: n_pairs,
            label: STANDARDS.mbcm.label.to_string(),
            note: format!(
                "The base is modelled for {} and the option for {}. Set both to the same year to work out benefits.",
                number_to_js_string(base_year),
                number_to_js_string(opt_year)
            ),
        }));
    }
    // (s + f(b) - f(o)) summed over the pairs, then averaged
    let avg = |f: fn(&crate::results::Kpi) -> f64| {
        pairs
            .iter()
            .fold(0.0, |acc, (b, o, _)| acc + f(&b.kpi) - f(&o.kpi))
            / n_pairs
    };
    let dvht = avg(|k| k.vht);
    let bcr_of = |pv: f64| if cap > 0.001 { Some(pv / cap) } else { None };
    let commercial = e.activity.is("commercial");
    if !e.method.is("mbcm") {
        // Older projects keep their saved flat-benefit discounting, with the sensitivity rate beside it
        let annual = dvht * e.annual_hours.num() * e.vot.num() / 1e6;
        let years = e.years.num();
        let flat = |rr: f64| {
            let r = rr / 100.0;
            if r > 0.0 {
                (1.0 - crate::jsmath::pow(1.0 + r, -years)) / r
            } else {
                years
            }
        };
        let pv = annual * flat(e.discount.num());
        let sens_rate = if commercial {
            STANDARDS.mbcm.commercial_sens_rate
        } else if e.sens_rate.num() > 0.0 {
            e.sens_rate.num()
        } else {
            STANDARDS.mbcm.sens_rate
        };
        let pv8 = annual * flat(sens_rate);
        let warnings = match &noise {
            Some(n) if !n.snr.ok => vec![n.snr.text.clone()],
            _ => Vec::new(),
        };
        return Some(Econ::Uniform(Box::new(EconUniform {
            method: "uniform".to_string(),
            dvht,
            annual,
            pv,
            pv8,
            cap,
            bcr: bcr_of(pv),
            bcr8: bcr_of(pv8),
            bcr_text: bcr_text(bcr_of(pv)),
            bcr8_text: bcr_text(bcr_of(pv8)),
            sens_rate,
            dco2: avg(|k| k.co2) * e.annual_hours.num(),
            periods: n_pairs,
            years: e.years.clone(),
            rate: e.discount.clone(),
            snr: noise,
            label: STANDARDS.mbcm.label.to_string(),
            basis: format!(
                "{}% a year over {} years, flat benefits (the project's saved settings, not MBCM v1.7.5); {}% sensitivity ({})",
                e.discount.js_string(),
                e.years.js_string(),
                number_to_js_string(sens_rate),
                STANDARDS.mbcm.discount.src.name
            ),
            standard: std_ref(&STANDARDS.mbcm.discount.src),
            warnings,
        })));
    }
    // { AM: 600, PM: 600, IP: 0, ...periodHours }
    let mut hours: OrderedMap<Num> = [("AM", 600.0), ("PM", 600.0), ("IP", 0.0)]
        .into_iter()
        .map(|(k, v)| (k.to_string(), Num::number(v)))
        .collect();
    if let Some(ph) = e.period_hours.get() {
        for (k, v) in ph.iter() {
            hours.insert(k, v.clone());
        }
    }
    let mut warnings: Vec<String> = Vec::new();
    let vot = e.vot.or(0.0)
        * if e.vot_update.truthy() {
            STANDARDS.mbcm_update.travel_time
        } else {
            1.0
        };
    let mut dvht_by: OrderedMap<f64> = OrderedMap::new();
    let (mut annual, mut dco2) = (0.0, 0.0);
    for (b, o, p) in &pairs {
        let d = b.kpi.vht - o.kpi.vht;
        dvht_by.insert(*p, d);
        let h = hours_of(&hours, p);
        annual += d * vot * h / 1e6;
        dco2 += (b.kpi.co2 - o.kpi.co2) * h;
    }
    let missing: Vec<String> = hours
        .iter()
        .filter(|(p, v)| v.num() > 0.0 && !pairs.iter().any(|x| x.2 == *p))
        .map(|(p, _)| p.to_string())
        .collect();
    if !missing.is_empty() {
        let many = missing.len() > 1;
        let are = if many { "are" } else { "is" };
        let hours_text = missing
            .iter()
            .map(|p| format!("{} h a year ({})", fmt::int(hours.num_at(p).num()), p))
            .collect::<Vec<_>>()
            .join(" and ");
        warnings.push(format!(
            "{} {} not modelled, so {} {} left out of the benefits. Run {} or set its hours to 0.",
            missing.join(" and "),
            are,
            hours_text,
            are,
            if many { "those periods" } else { "that period" }
        ));
    }
    let per = econ_period(&e);
    if !per.ok {
        warnings.push(per.why.clone());
    }
    // Benefits grow arithmetically with background traffic (or the user's own benefit growth rate)
    let bg = &e.benefit_growth;
    let bg_blank = bg.is_nullish() || bg.get().and_then(|v| v.as_str()) == Some("");
    let r = if !bg_blank && bg.num().is_finite() {
        bg.num() / 100.0
    } else {
        s.growth.or(0.0) / 100.0
    };
    if s.growth_mode.is("compound") && bg_blank {
        warnings.push("Traffic growth is set to compound; MBCM benefits grow arithmetically, so its rate is used as the arithmetic rate.".to_string());
    }
    // NZTA GC 25/01: non-commercial 2% / 1.5% / 1% with 8% sensitivity; commercial 8% with 2% sensitivity
    let rates = if commercial {
        json!([{ "to": null, "r": json_number(STANDARDS.mbcm.commercial_rate) }])
    } else {
        match e.rates.get() {
            Some(Value::Array(a)) if !a.is_empty() => Value::Array(a.clone()),
            _ => mbcm_rates(),
        }
    };
    let sens = if commercial {
        STANDARDS.mbcm.commercial_sens_rate
    } else if e.sens_rate.num() > 0.0 {
        e.sens_rate.num()
    } else {
        STANDARDS.mbcm.sens_rate
    };
    let f = pwf(&rates, per.years);
    let f8 = pwf(&json!([{ "to": null, "r": plain_number(sens) }]), per.years);
    let pv = annual * (f.uspwf + r * f.agpwf);
    let pv8 = annual * (f8.uspwf + r * f8.agpwf);
    let bcr = bcr_of(pv);
    let bcr8 = bcr_of(pv8);
    let ipm = ipm_band(bcr, None);
    let ipm8 = ipm_band(bcr8, None);
    let lstar = !missing.is_empty() || !(cap > 0.001);
    let basis = format!(
        "MBCM {}: {}{}; {}% sensitivity ({}); {} years from {}; benefits growing {}% of year-one benefit a year (arithmetic)",
        STANDARDS.mbcm.src.edition,
        if commercial {
            "commercial activity, "
        } else {
            ""
        },
        seg_text(&rates),
        number_to_js_string(sens),
        STANDARDS.mbcm.discount.src.name,
        number_to_js_string(per.years),
        STANDARDS.mbcm.time_zero,
        to_fixed(r * 100.0, 1)
    );
    let compare40 = if per.compare40 == Some(true) {
        let f40 = pwf(&rates, STANDARDS.mbcm.years);
        let pv40 = annual * (f40.uspwf + r * f40.agpwf);
        Some(Compare40 {
            years: STANDARDS.mbcm.years,
            pv: pv40,
            bcr: bcr_of(pv40),
        })
    } else {
        None
    };
    // A band boundary between the central and sensitivity results makes the band uncertain
    for (b, x) in ipm.iter().zip(&ipm8) {
        if let (Some(bb), Some(xb)) = (&b.band, &x.band)
            && bb != xb
        {
            warnings.push(format!(
                "The {}{} band changes from {} to {} at the {}% sensitivity rate.",
                b.edition,
                if b.draft { " (draft)" } else { "" },
                b.label,
                x.label,
                number_to_js_string(sens)
            ));
        }
    }
    if cap >= STANDARDS.mbcm.p95_from && !e.p95.truthy() {
        warnings.push(format!(
            "The cost is {}, at least ${}m: an MBCM evaluation needs a P95 cost estimate.",
            fmt::money(cap),
            number_to_js_string(STANDARDS.mbcm.p95_from)
        ));
    }
    if lstar {
        warnings.push(
            "L*: the inputs are incomplete, so the efficiency rating is provisional.".to_string(),
        );
    }
    if let Some(n) = &noise
        && !n.snr.ok
    {
        warnings.push(n.snr.text.clone());
    }
    let vot_note = if e.vot_update.truthy() {
        None
    } else {
        Some(format!(
            "Value of time ${}/veh\u{b7}h as entered. Use MBCM values for the evaluation year (update factor to {}: \u{d7}{}).",
            e.vot.js_string(),
            STANDARDS.mbcm_update.src.edition,
            number_to_js_string(STANDARDS.mbcm_update.travel_time)
        ))
    };
    Some(Econ::Mbcm(Box::new(EconMbcm {
        method: "mbcm".to_string(),
        dvht,
        dvht_by,
        annual,
        pv,
        pv8,
        cap,
        bcr,
        bcr8,
        npv: pv - cap,
        npv8: pv8 - cap,
        fyrr: if cap > 0.001 {
            Some(annual / cap)
        } else {
            None
        },
        dco2,
        periods: n_pairs,
        years: per.years,
        rates,
        sens_rate: sens,
        activity: if commercial {
            "commercial"
        } else {
            "non-commercial"
        }
        .to_string(),
        r,
        vot,
        vot_update: e.vot_update.truthy(),
        hours,
        factors: EconFactors {
            uspwf: f.uspwf,
            agpwf: f.agpwf,
            uspwf8: f8.uspwf,
            agpwf8: f8.agpwf,
        },
        ipm,
        ipm8,
        bcr_text: bcr_text(bcr),
        bcr8_text: bcr_text(bcr8),
        snr: noise,
        incomplete: missing,
        lstar,
        label: STANDARDS.mbcm.label.to_string(),
        disclaimer: STANDARDS.mbcm.disclaimer.to_string(),
        standard: std_ref(&STANDARDS.mbcm.src),
        discount: std_ref(&STANDARDS.mbcm.discount.src),
        basis,
        warnings,
        compare40,
        vot_note,
    })))
}
