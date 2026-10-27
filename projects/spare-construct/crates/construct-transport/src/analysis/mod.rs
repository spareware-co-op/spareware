//! Assessment analysis from engine.js.
//!
//! The extent of effects test (AT ITA 4.2.1.1, with the Future Connect classes it reads),
//! difference plots, without and with development per junction, the gateway distribution of
//! development traffic, the MSM consistency check and the sensitivity pack ([`sensitivity`]).
//! Port of engine.js, "extent of effects and difference plots", devDistribution, "sensitivity
//! pack" and "MSM consistency".
//!
//! Each function takes the results it compares (`&Res`) explicitly. The standards (extent,
//! diff, msm) come from crate::standards; the extent test's regular expressions are matched
//! by hand here (see [`extent_class`]).

// `!(x > 0)` is the JavaScript's own test: unlike `x <= 0` it is true for NaN
#![allow(clippy::neg_cmp_op_on_partial_ord)]

pub mod sensitivity;

pub use sensitivity::*;

use crate::jsnum::{
    math_max, math_min, number_to_js_string, or_default, to_fixed, value_to_js_string,
    value_to_number, value_truthy,
};
use crate::project::{Link, OrderedMap, Scenario};
use crate::results::{Res, SELECT_LABEL, js_sort_by, plain};
use crate::standards::STANDARDS;
use crate::util::{clamp, fmt, r1, r3};
use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};
use serde_json::Value;

/* ---------------- Future Connect classes and the extent test ---------------- */

/// A link's Future Connect classes and where they came from (engine.js fcOf).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FcOf {
    /// General traffic class, or null.
    pub gt: Value,
    /// Public transport class, or null.
    pub pt: Value,
    /// Freight class, or null.
    pub freight: Value,
    /// Where the classes came from.
    pub basis: Value,
}

// obj[k] for a JSON value (None when not an object or no such key)
fn key_of<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    v.get(k)
}

// !!obj[k]
fn key_truthy(v: &Value, k: &str) -> bool {
    key_of(v, k).is_some_and(value_truthy)
}

/// `Engine.fcOf(L, decade)`: the link's Future Connect classes (conflated or set by the user, the first-decade ones when decade and given), otherwise from its road class.
pub fn fc_of(l: &Link, decade: bool) -> FcOf {
    if let Some(f) = l.fc.get().filter(|f| value_truthy(f))
        && ["gt", "pt", "freight", "gt10", "pt10", "freight10"]
            .iter()
            .any(|k| key_truthy(f, k))
    {
        let pick = |k: &str| {
            let k10 = format!("{k}10");
            let v = match key_of(f, &k10) {
                Some(x) if decade && !x.is_null() => Some(x),
                _ => key_of(f, k),
            };
            v.filter(|x| value_truthy(x))
                .cloned()
                .unwrap_or(Value::Null)
        };
        let basis = if key_truthy(f, "user") {
            Value::String("set by the user".to_string())
        } else {
            key_of(f, "src")
                .filter(|x| value_truthy(x))
                .cloned()
                .unwrap_or_else(|| Value::String("AT Future Connect".to_string()))
        };
        return FcOf {
            gt: pick("gt"),
            pt: pick("pt"),
            freight: pick("freight"),
            basis,
        };
    }
    let gt = match l.cls.as_str() {
        Some("motorway") | Some("ramp") => "Motorway",
        Some("arterial") => "Arterial (from road class)",
        Some("collector") => "Collector (from road class)",
        Some("local") => "Local (from road class)",
        _ => "",
    };
    FcOf {
        gt: if gt.is_empty() {
            Value::Null
        } else {
            Value::String(gt.to_string())
        },
        pt: Value::Null,
        freight: Value::Null,
        basis: Value::String(
            "road class (Spare Construct assumption; no Future Connect class)".to_string(),
        ),
    }
}

/// A link's extent threshold (engine.js extentClass).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ExtentClass {
    /// Threshold (fraction), or null.
    #[serde(with = "plain")]
    pub p: Option<f64>,
    /// "5%", "10%" or null.
    pub cls: Option<String>,
    /// Why the threshold applies.
    pub why: Vec<String>,
}

// /^(motorway|strategic( arterial)?|primary arterial|secondary arterial|arterial \(from road class\))$/i
fn gt5(s: &str) -> bool {
    [
        "motorway",
        "strategic",
        "strategic arterial",
        "primary arterial",
        "secondary arterial",
        "arterial (from road class)",
    ]
    .iter()
    .any(|p| s.eq_ignore_ascii_case(p))
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

// /rapid transit|frequent transit|\b(i?rtn|ftn)\b/i
fn pt5(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    if l.contains("rapid transit") || l.contains("frequent transit") {
        return true;
    }
    let b = l.as_bytes();
    (0..b.len()).any(|p| {
        (p == 0 || !is_word(b[p - 1]))
            && ["irtn", "rtn", "ftn"].iter().any(|w| {
                let e = p + w.len();
                b[p..].starts_with(w.as_bytes()) && (e == b.len() || !is_word(b[e]))
            })
    })
}

// /^level (1a|1b|2)$/i
fn freight5(s: &str) -> bool {
    ["level 1a", "level 1b", "level 2"]
        .iter()
        .any(|p| s.eq_ignore_ascii_case(p))
}

// /^collector/i
fn gt10(s: &str) -> bool {
    s.len() >= 9 && s.as_bytes()[..9].eq_ignore_ascii_case(b"collector")
}

/// `Engine.extentClass(fc)`: the AT ITA 4.2.1.1 threshold for a link's classes: 5%, 10% (collectors) or none.
///
/// The patterns of STANDARDS.extent (gt5, pt5, freight5, gt10) are matched as JavaScript's
/// case-insensitive regular expressions match them: ASCII letters fold, word characters for
/// `` are [A-Za-z0-9_].
pub fn extent_class(fc: &FcOf) -> ExtentClass {
    let mut why = Vec::new();
    let text = |v: &Value| value_to_js_string(v);
    if value_truthy(&fc.gt) && gt5(&text(&fc.gt)) {
        why.push(format!("general traffic {}", text(&fc.gt)));
    }
    if value_truthy(&fc.pt) && pt5(&text(&fc.pt)) {
        why.push(format!("public transport {}", text(&fc.pt)));
    }
    if value_truthy(&fc.freight) && freight5(&text(&fc.freight)) {
        why.push(format!(
            "freight {} ({})",
            text(&fc.freight),
            STANDARDS.extent.freight_basis
        ));
    }
    if !why.is_empty() {
        return ExtentClass {
            p: Some(STANDARDS.extent.p5),
            cls: Some("5%".to_string()),
            why,
        };
    }
    if value_truthy(&fc.gt) && gt10(&text(&fc.gt)) {
        return ExtentClass {
            p: Some(STANDARDS.extent.p10),
            cls: Some("10%".to_string()),
            why: vec![format!("general traffic {}", text(&fc.gt))],
        };
    }
    ExtentClass {
        p: None,
        cls: None,
        why: Vec::new(),
    }
}

/// The classes shown in an extent row.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FcClasses {
    /// General traffic.
    pub gt: Value,
    /// Public transport.
    pub pt: Value,
    /// Freight.
    pub freight: Value,
}

/// One directional link in the extent test.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtentRow {
    /// Arc key.
    pub key: String,
    /// Link id.
    pub link: String,
    /// Road name.
    pub name: String,
    /// "ab" or "ba".
    pub dir: String,
    /// Future Connect classes.
    pub fc: FcClasses,
    /// Where the classes came from.
    pub basis: Value,
    /// Flow without (veh/h), 0.1.
    #[serde(with = "plain")]
    pub without: f64,
    /// Flow with (veh/h), 0.1.
    #[serde(with = "plain")]
    pub with: f64,
    /// Change (veh/h), 0.1.
    #[serde(with = "plain")]
    pub d: f64,
    /// Change over the flow without, 0.001; null for a new flow.
    #[serde(with = "plain")]
    pub p: Option<f64>,
    /// "+12.5%", "−3.0%" or "new".
    pub p_text: String,
    /// Threshold, or null.
    #[serde(with = "plain")]
    pub threshold: Option<f64>,
    /// "5%", "10%" or null.
    pub cls: Option<String>,
    /// Why the threshold applies.
    pub why: Vec<String>,
    /// Over the threshold and the floor.
    pub included: bool,
}

/// A junction the extent includes, and the approaches that put it in.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ExtentSite {
    /// Controller node id.
    pub node: String,
    /// Junction site id, or null.
    pub site: Option<String>,
    /// Included approach arc keys.
    pub approaches: Vec<String>,
}

/// What `Engine.extentTest` returns.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtentTest {
    /// Period of the with result.
    pub period: String,
    /// Directional links with flow, included first, then by change.
    pub rows: Vec<ExtentRow>,
    /// Included arc keys.
    pub links: Vec<String>,
    /// Included junctions (site id, or node id).
    pub sites: Vec<String>,
    /// Included junctions by site or node id.
    pub site_of: OrderedMap<ExtentSite>,
    /// Smallest change counted (veh/h).
    #[serde(with = "plain")]
    pub floor: f64,
    /// "current" or "first decade".
    pub decade: String,
    /// Suggested study area [[south, west], [north, east]], or null.
    #[serde(with = "plain")]
    pub bounds: Option<Vec<[f64; 2]>>,
    /// Buffer (m).
    #[serde(with = "plain")]
    pub buffer: f64,
    /// The rule in words.
    pub label: String,
    /// Without and with ran at the same gap target.
    pub same_gap: bool,
    /// Warning when they did not ("" otherwise).
    pub note: String,
}

/// Options of the extent test (engine.js opts).
#[derive(Clone, Copy, Debug, Default)]
pub struct ExtentOpts<'a> {
    /// Smallest change counted (veh/h); None: STANDARDS.extent.floor.
    pub floor: Option<f64>,
    /// Some("first"): the first-decade Future Connect classes.
    pub decade: Option<&'a str>,
    /// The without scenario, for links that only exist without.
    pub sc_without: Option<&'a Scenario>,
}

// Keys of two maps, the first's then the second's new ones (new Set([...keys(a), ...keys(b)]))
fn union_keys<'a, V, W>(a: &'a OrderedMap<V>, b: &'a OrderedMap<W>) -> Vec<&'a str> {
    let mut seen = std::collections::HashSet::new();
    a.keys()
        .chain(b.keys())
        .filter(|k| seen.insert(*k))
        .collect()
}

// key.slice(0, key.lastIndexOf(':')) and key.slice(-2)
fn arc_link_id(key: &str) -> String {
    match key.rfind(':') {
        Some(i) => key[..i].to_string(),
        None => {
            let n = key.chars().count();
            key.chars().take(n.saturating_sub(1)).collect()
        }
    }
}

fn last_two(key: &str) -> String {
    let n = key.chars().count();
    key.chars().skip(n.saturating_sub(2)).collect()
}

/// `Engine.extentTest(sc, without, withRes, opts)`: per directional link, change = with − without and p = change / without; a link is included when p is over its threshold and the change is at least the floor; a junction is included when any of its approaches is.
///
/// sc is the with scenario (link classes and geometry). None without both results.
pub fn extent_test(
    sc: &Scenario,
    without: Option<&Res>,
    with: Option<&Res>,
    opts: ExtentOpts,
) -> Option<ExtentTest> {
    let floor = opts.floor.unwrap_or(STANDARDS.extent.floor);
    let decade = opts.decade == Some("first");
    let (without, with) = (without?, with?);
    let mut rows = Vec::new();
    for key in union_keys(&without.arcs, &with.arcs) {
        let id = arc_link_id(key);
        let Some(l) = sc
            .links
            .get(&id)
            .or_else(|| opts.sc_without.and_then(|w| w.links.get(&id)))
        else {
            continue;
        };
        let v0 = or_default(without.arcs.get(key).map_or(0.0, |a| a.v), 0.0);
        let v1 = or_default(with.arcs.get(key).map_or(0.0, |a| a.v), 0.0);
        let d = v1 - v0;
        let p = if v0 > 0.0 {
            d / v0
        } else if d > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };
        let fc = fc_of(l, decade);
        let ec = extent_class(&fc);
        let included = ec.p.is_some_and(|t| p > t) && d >= floor;
        if !(v0 > 0.0 || v1 > 0.0) {
            continue;
        }
        rows.push(ExtentRow {
            key: key.to_string(),
            link: id,
            name: l.name.str_or("").to_string(),
            dir: last_two(key),
            fc: FcClasses {
                gt: fc.gt,
                pt: fc.pt,
                freight: fc.freight,
            },
            basis: fc.basis,
            without: r1(v0),
            with: r1(v1),
            d: r1(d),
            p: if p.is_finite() { Some(r3(p)) } else { None },
            p_text: if p.is_finite() {
                format!(
                    "{}{}%",
                    if p >= 0.0 { "+" } else { "\u{2212}" },
                    to_fixed((p * 100.0).abs(), 1)
                )
            } else {
                "new".to_string()
            },
            threshold: ec.p,
            cls: ec.cls,
            why: ec.why,
            included,
        });
    }
    let b2n = |b: bool| if b { 1.0 } else { 0.0 };
    js_sort_by(&mut rows, |a, b| {
        or_default(b2n(b.included) - b2n(a.included), b.d - a.d)
    });
    // new Set(included keys): the row keys are already distinct
    let links: Vec<String> = rows
        .iter()
        .filter(|r| r.included)
        .map(|r| r.key.clone())
        .collect();
    let inc: std::collections::HashSet<&str> = links.iter().map(String::as_str).collect();
    let mut sites = Vec::new();
    let mut site_of = OrderedMap::new();
    for (nid, nr) in with.nodes.iter() {
        let hit: Vec<String> = nr
            .appr
            .iter()
            .filter(|a| inc.contains(a.key.as_str()))
            .map(|a| a.key.clone())
            .collect();
        if !hit.is_empty() {
            let site = nr.site.clone().filter(|s| !s.is_empty());
            let id = site.clone().unwrap_or_else(|| nid.to_string());
            sites.push(id.clone());
            site_of.insert(
                id,
                ExtentSite {
                    node: nid.to_string(),
                    site,
                    approaches: hit,
                },
            );
        }
    }
    // the suggested study area: the included links' extent with a buffer
    let (mut s, mut w, mut n, mut e) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for r in rows.iter().filter(|r| r.included) {
        let Some(l) = sc.links.get(&r.link) else {
            continue;
        };
        if !sc.nodes.contains_key(&l.a) || !sc.nodes.contains_key(&l.b) {
            continue;
        }
        for q in sc.geom(l) {
            s = math_min(s, q[0]);
            n = math_max(n, q[0]);
            w = math_min(w, q[1]);
            e = math_max(e, q[1]);
        }
    }
    let pad = STANDARDS.extent.buffer / 111320.0;
    let pad_l = if s.is_finite() {
        STANDARDS.extent.buffer / (111320.0 * crate::jsmath::cos((s + n) / 2.0 * std::f64::consts::PI / 180.0))
    } else {
        0.0
    };
    let same_gap = without.gap_tol == with.gap_tol;
    Some(ExtentTest {
        period: with.period.clone(),
        rows,
        links,
        sites,
        site_of,
        floor,
        decade: if decade { "first decade" } else { "current" }.to_string(),
        bounds: if s.is_finite() {
            Some(vec![[s - pad, w - pad_l], [n + pad, e + pad_l]])
        } else {
            None
        },
        buffer: STANDARDS.extent.buffer,
        label: format!(
            "AT ITA Guidelines 2025 v2 s4.2.1.1: 5% on motorways, strategic, primary and secondary arterials, RTN and FTN routes and freight levels 1A, 1B and 2; 10% on collectors; changes under {} veh/h left out (Spare Construct assumption). The freight set is a Spare Construct assumption.",
            number_to_js_string(floor)
        ),
        same_gap,
        note: if same_gap {
            String::new()
        } else {
            "Without and with ran at different convergence targets; re-run both at the same target before relying on the extent.".to_string()
        },
    })
}

/* ---------------- difference plots ---------------- */

/// One arc of a difference plot.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DiffArc {
    /// Change in flow (veh/h), 0.1.
    #[serde(with = "plain")]
    pub d: f64,
    /// Change in V/C, 0.001.
    #[serde(with = "plain")]
    pub dvc: f64,
    /// Change in junction delay (s), 0.1.
    #[serde(with = "plain")]
    pub dnd: f64,
    /// The bar is drawn.
    pub show: bool,
    /// The bar is labelled.
    pub label: bool,
    /// Bar width (px), 0.1.
    #[serde(with = "plain")]
    pub w: f64,
    /// "up", "down" or "".
    pub sign: String,
}

/// What `Engine.difference` returns.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Difference {
    /// Arcs by key (the base's keys, then the option's new ones).
    pub arcs: OrderedMap<DiffArc>,
    /// Bars under this change are hidden (veh/h), 0.1.
    #[serde(with = "plain")]
    pub hide: f64,
    /// The larger assignment noise of the two runs (veh/h).
    #[serde(with = "plain")]
    pub noise95: f64,
    /// Labels from this change (veh/h).
    #[serde(with = "plain")]
    pub label_min: f64,
    /// The thresholds in words.
    pub basis: String,
}

/// Options of a difference plot: k px per veh/h (0.02) and the map zoom (16).
#[derive(Clone, Copy, Debug, Default)]
pub struct DiffOpts {
    /// Bar width per veh/h (px); None: 0.02.
    pub k: Option<f64>,
    /// Map zoom; None: 16.
    pub zoom: Option<f64>,
}

/// `Engine.difference(base, opt, o)`: per arc change in flow, V/C and junction delay; bars hidden under max(20, 2 × noise95), labels from 10 veh/h (50 below zoom 15). None without both results.
pub fn difference(base: Option<&Res>, opt: Option<&Res>, o: DiffOpts) -> Option<Difference> {
    let (base, opt) = (base?, opt?);
    let n95 = math_max(
        or_default(base.noise95.unwrap_or(0.0), 0.0),
        or_default(opt.noise95.unwrap_or(0.0), 0.0),
    );
    let hide = math_max(STANDARDS.diff.floor, STANDARDS.diff.noise_k * n95);
    let k = o.k.unwrap_or(0.02);
    let zoom = o.zoom.unwrap_or(16.0);
    let label_min = if zoom < STANDARDS.diff.low_zoom {
        STANDARDS.diff.label_low_zoom
    } else {
        STANDARDS.diff.label
    };
    let mut arcs = OrderedMap::new();
    for key in union_keys(&base.arcs, &opt.arcs) {
        let a = base.arcs.get(key);
        let b = opt.arcs.get(key);
        let get = |x: Option<&crate::results::ArcRes>, f: fn(&crate::results::ArcRes) -> f64| {
            or_default(x.map_or(0.0, f), 0.0)
        };
        let d = get(b, |x| x.v) - get(a, |x| x.v);
        let show = d.abs() >= hide;
        arcs.insert(
            key,
            DiffArc {
                d: r1(d),
                dvc: r3(get(b, |x| x.vc) - get(a, |x| x.vc)),
                dnd: r1(get(b, |x| x.nd) - get(a, |x| x.nd)),
                show,
                label: show && d.abs() >= label_min,
                w: r1(clamp(
                    k * d.abs(),
                    STANDARDS.diff.w_min,
                    STANDARDS.diff.w_max,
                )),
                sign: if d > 0.0 {
                    "up"
                } else if d < 0.0 {
                    "down"
                } else {
                    ""
                }
                .to_string(),
            },
        );
    }
    Some(Difference {
        arcs,
        hide: r1(hide),
        noise95: n95,
        label_min,
        basis: format!(
            "Bars under {} veh/h are hidden (the larger of {} veh/h and twice the assignment noise, {} veh/h).",
            fmt::int(hide),
            number_to_js_string(STANDARDS.diff.floor),
            number_to_js_string(r1(n95))
        ),
    })
}

/* ---------------- without and with development ---------------- */

/// One side of a without / with junction row.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WwSide {
    /// Largest degree of saturation.
    #[serde(with = "plain")]
    pub dos: f64,
    /// Average delay (s).
    #[serde(with = "plain")]
    pub delay: f64,
    /// Level of service.
    pub los: String,
    /// Level of service text.
    pub los_text: String,
    /// Longest back of queue (m).
    #[serde(with = "plain")]
    pub q95: f64,
}

/// The changes from without to with.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WwDelta {
    /// Change in DoS, 0.001.
    #[serde(with = "plain")]
    pub dos: f64,
    /// "+0.12" or "–".
    pub dos_text: String,
    /// Change in delay (s), 0.1.
    #[serde(with = "plain")]
    pub delay: f64,
    /// "+12.2 s" or "–".
    pub delay_text: String,
    /// "B to D" or "–".
    pub los: String,
    /// Change in the longest queue (m).
    #[serde(with = "plain")]
    pub q95: f64,
    /// "+35 m" or "–".
    pub q95_text: String,
}

/// One junction without and with development.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WwRow {
    /// Controller node id.
    pub id: String,
    /// Junction site id, or null.
    pub site: Option<String>,
    /// Junction name.
    pub name: String,
    /// Period of the with result.
    pub period: String,
    /// Without, or null when the junction is not there.
    pub without: Option<WwSide>,
    /// With, or null when the junction is not there.
    pub with: Option<WwSide>,
    /// Changes (left out unless both are there).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<WwDelta>,
}

/// What `Engine.withWithout` returns.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WithWithout {
    /// Junctions, largest increase in delay first.
    pub rows: Vec<WwRow>,
    /// Warning when the runs had different gap targets ("" otherwise).
    pub note: String,
    /// SELECT_LABEL.
    pub label: String,
}

fn ww_pick(nr: Option<&crate::results::NodeRes>) -> Option<WwSide> {
    let nr = nr?;
    let q95 = nr
        .appr
        .iter()
        .fold(0.0, |m, a| math_max(m, or_default(a.q95, 0.0)));
    Some(WwSide {
        dos: nr.dos,
        delay: nr.avg_delay,
        los: nr.los.clone(),
        los_text: if nr.los_text.is_empty() {
            nr.los.clone()
        } else {
            nr.los_text.clone()
        },
        q95,
    })
}

// '–' under half a unit of the last decimal, otherwise '+12.2 s' / '−3.0 s'
fn ww_sgn(x: f64, u: &str, dp: usize) -> String {
    let half = [1.0, 0.1, 0.01][dp] / 2.0;
    if x.abs() < half {
        return fmt::DASH.to_string();
    }
    format!(
        "{}{}{}",
        if x > 0.0 { "+" } else { "\u{2212}" },
        to_fixed(x.abs(), dp),
        u
    )
}

/// `Engine.withWithout(without, withRes, opts)`: per junction, max DoS, average delay, LoS and longest Q95 without and with development, and the changes. name(id) names a junction (default: its site's name, otherwise the id). None without both results.
pub fn with_without(
    without: Option<&Res>,
    with: Option<&Res>,
    name: Option<&dyn Fn(&str) -> String>,
) -> Option<WithWithout> {
    let (without, with) = (without?, with?);
    let default_name = |id: &str| -> String {
        let site = with.nodes.get(id).and_then(|n| n.site.as_deref());
        let site_key = site.unwrap_or("undefined");
        match with.sites.as_ref().and_then(|s| s.get(site_key)) {
            Some(s) if !s.name.is_empty() => s.name.clone(),
            _ => id.to_string(),
        }
    };
    let mut rows = Vec::new();
    for id in union_keys(&without.nodes, &with.nodes) {
        let a = ww_pick(without.nodes.get(id));
        let b = ww_pick(with.nodes.get(id));
        if a.is_none() && b.is_none() {
            continue;
        }
        let site = with
            .nodes
            .get(id)
            .or_else(|| without.nodes.get(id))
            .and_then(|n| n.site.clone())
            .filter(|s| !s.is_empty());
        let delta = match (&a, &b) {
            (Some(a), Some(b)) => Some(WwDelta {
                dos: r3(b.dos - a.dos),
                dos_text: ww_sgn(b.dos - a.dos, "", 2),
                delay: r1(b.delay - a.delay),
                delay_text: ww_sgn(b.delay - a.delay, " s", 1),
                los: if a.los == b.los {
                    fmt::DASH.to_string()
                } else {
                    format!("{} to {}", a.los, b.los)
                },
                q95: b.q95 - a.q95,
                q95_text: ww_sgn(b.q95 - a.q95, " m", 0),
            }),
            _ => None,
        };
        rows.push(WwRow {
            id: id.to_string(),
            site,
            name: match name {
                Some(f) => f(id),
                None => default_name(id),
            },
            period: with.period.clone(),
            without: a,
            with: b,
            delta,
        });
    }
    let delay_of = |r: &WwRow| or_default(r.delta.as_ref().map_or(0.0, |d| d.delay), 0.0);
    js_sort_by(&mut rows, |p, q| delay_of(q) - delay_of(p));
    let note = if without.gap_tol != with.gap_tol {
        format!(
            "Without (gap target {}) and with ({}) ran at different convergence targets. Re-run both at the same target.",
            number_to_js_string(without.gap_tol),
            number_to_js_string(with.gap_tol)
        )
    } else {
        String::new()
    };
    Some(WithWithout {
        rows,
        note,
        label: SELECT_LABEL.to_string(),
    })
}

/* ---------------- gateway distribution of development traffic ---------------- */

/// Development trips crossing the boundary in one period.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevPeriod {
    /// Arriving through gateways (veh/h).
    #[serde(rename = "in", with = "plain")]
    pub r#in: f64,
    /// Leaving through gateways (veh/h).
    #[serde(with = "plain")]
    pub out: f64,
    /// Development trips (veh/h).
    #[serde(with = "plain")]
    pub trips: f64,
    /// Share of development trips that stay inside (%), 0.1; null without trips.
    #[serde(with = "plain")]
    pub internal_pct: Option<f64>,
}

/// One gateway's share of development traffic: id, name, veh/h by period, then "<period>in" and "<period>out" (%).
#[derive(Clone, Debug, PartialEq)]
pub struct DevRow {
    /// Gateway id.
    pub id: String,
    /// Gateway name.
    pub name: String,
    /// Development traffic through it by period (veh/h), 0.1.
    pub vph: OrderedMap<f64>,
    /// "AMin", "AMout", "PMin", "PMout" ...: % of the period's development trips in and out.
    pub shares: OrderedMap<f64>,
}

impl Serialize for DevRow {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(3 + self.shares.len()))?;
        m.serialize_entry("id", &self.id)?;
        m.serialize_entry("name", &self.name)?;
        m.serialize_entry("vph", &plain::Ser(&self.vph))?;
        for (k, v) in self.shares.iter() {
            m.serialize_entry(k, &plain::Ser(v))?;
        }
        m.end()
    }
}

/// What `Engine.devDistribution` returns.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DevDistribution {
    /// Gateways, most development traffic first.
    pub rows: Vec<DevRow>,
    /// Totals by period.
    pub periods: OrderedMap<DevPeriod>,
    /// What the shares are of.
    pub label: String,
}

/// `Engine.devDistribution(sc, resList)`: per gateway, the % of development trips arriving and leaving through it in each period, and the veh/h.
pub fn dev_distribution(sc: Option<&Scenario>, res_list: &[&Res]) -> DevDistribution {
    let mut ids: Vec<String> = Vec::new();
    let mut periods: OrderedMap<DevPeriod> = OrderedMap::new();
    for r in res_list {
        let g = &r.dev.gateways;
        let tin = g.values().fold(0.0, |s, x| s + x.r#in);
        let tout = g.values().fold(0.0, |s, x| s + x.out);
        let trips = r.dev.trips;
        periods.insert(
            r.period.clone(),
            DevPeriod {
                r#in: tin,
                out: tout,
                trips,
                internal_pct: if trips > 0.0 {
                    Some(r1(100.0 * (1.0 - (tin + tout) / trips)))
                } else {
                    None
                },
            },
        );
        for k in g.keys() {
            if !ids.iter().any(|x| x == k) {
                ids.push(k.to_string());
            }
        }
    }
    let mut rows: Vec<DevRow> = ids
        .into_iter()
        .map(|id| {
            let name = sc
                .and_then(|sc| sc.zones.get(&id))
                .and_then(|z| z.name.as_str())
                .filter(|n| !n.is_empty())
                .map_or_else(|| id.clone(), str::to_string);
            let mut row = DevRow {
                id: id.clone(),
                name,
                vph: OrderedMap::new(),
                shares: OrderedMap::new(),
            };
            for r in res_list {
                let (xin, xout) = r
                    .dev
                    .gateways
                    .get(&id)
                    .map_or((0.0, 0.0), |x| (x.r#in, x.out));
                let p = &periods[r.period.as_str()];
                row.shares.insert(
                    format!("{}in", r.period),
                    if p.r#in > 0.0 {
                        100.0 * xin / p.r#in
                    } else {
                        0.0
                    },
                );
                row.shares.insert(
                    format!("{}out", r.period),
                    if p.out > 0.0 {
                        100.0 * xout / p.out
                    } else {
                        0.0
                    },
                );
                row.vph.insert(r.period.clone(), r1(xin + xout));
            }
            row
        })
        .collect();
    let total = |r: &DevRow| r.vph.values().fold(0.0, |s, v| s + v);
    js_sort_by(&mut rows, |a, b| total(b) - total(a));
    DevDistribution {
        rows,
        periods,
        label: "Shares of the development trips that cross the study-area boundary; trips that stay inside are counted separately (internal).".to_string(),
    }
}

/* ---------------- MSM consistency ---------------- */

/// One link compared with MSM growth.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MsmRow {
    /// Link id.
    pub link: String,
    /// Implied linear growth (% a year), 0.001.
    #[serde(with = "plain")]
    pub corridor: f64,
    /// MSM growth (% a year).
    #[serde(with = "plain")]
    pub msm: f64,
    /// Difference as % of the MSM rate, 0.1; null when infinite.
    #[serde(with = "plain")]
    pub diff_pct: Option<f64>,
    /// Over the threshold.
    pub flagged: bool,
}

/// What `Engine.msmCheck` returns.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum MsmCheck {
    /// Not checked, and why.
    NotChecked {
        /// false.
        checked: bool,
        /// Why.
        text: String,
        /// Empty.
        rows: Vec<MsmRow>,
    },
    /// Checked.
    Checked {
        /// true.
        checked: bool,
        /// Threshold (% of the MSM rate).
        #[serde(with = "plain")]
        pct: f64,
        /// STANDARDS.msm.edition.
        basis: String,
        /// Links with MSM growth and flow in the base.
        rows: Vec<MsmRow>,
        /// Links flagged.
        #[serde(with = "plain")]
        flagged: f64,
        /// Summary.
        text: String,
    },
}

// Object.entries(x) for a JSON value: object entries, array items by index, characters of text
fn js_entries(v: &Value) -> Vec<(String, Value)> {
    match v {
        Value::Object(m) => m.iter().map(|(k, x)| (k.clone(), x.clone())).collect(),
        Value::Array(a) => a
            .iter()
            .enumerate()
            .map(|(i, x)| (i.to_string(), x.clone()))
            .collect(),
        Value::String(s) => s
            .chars()
            .enumerate()
            .map(|(i, c)| (i.to_string(), Value::String(c.to_string())))
            .collect(),
        _ => Vec::new(),
    }
}

/// `Engine.msmCheck(baseRes, futRes, msm, opts)`: implied linear growth per link ((future / base − 1) / years × 100) against MSM's; flagged when they differ by more than pct % of the MSM rate (20 by default).
///
/// msm is `P.data.msm` ({ links: { linkId: { growth } } }). Its links are read in the order
/// serde_json's Value keeps (sorted), where JavaScript keeps the file's order. A missing base
/// result with a later future result (the JavaScript throws) reads as not checked.
pub fn msm_check(
    base: Option<&Res>,
    fut: Option<&Res>,
    msm_data: Option<&Value>,
    pct: Option<f64>,
) -> MsmCheck {
    let pct = pct.unwrap_or(STANDARDS.msm.pct);
    let links = msm_data
        .filter(|m| value_truthy(m))
        .and_then(|m| m.get("links"))
        .filter(|l| value_truthy(l));
    let entries = links.map(js_entries).unwrap_or_default();
    if entries.is_empty() {
        return MsmCheck::NotChecked {
            checked: false,
            text: STANDARDS.msm.not_checked.to_string(),
            rows: Vec::new(),
        };
    }
    let dy = fut.map_or(0.0, |r| r.year) - base.map_or(0.0, |r| r.year);
    let needs = || MsmCheck::NotChecked {
        checked: false,
        text: "MSM consistency needs a base-year result and a later-year result.".to_string(),
        rows: Vec::new(),
    };
    if !(dy > 0.0) {
        return needs();
    }
    let (Some(base), Some(fut)) = (base, fut) else {
        return needs();
    };
    let flow = |r: &Res, id: &str| {
        ["ab", "ba"].iter().fold(0.0, |s, d| {
            s + or_default(r.arcs.get(&format!("{id}:{d}")).map_or(0.0, |a| a.v), 0.0)
        })
    };
    let mut rows = Vec::new();
    for (id, m) in entries {
        let v0 = flow(base, &id);
        let v1 = flow(fut, &id);
        let g = m.get("growth").map_or(f64::NAN, value_to_number);
        if !(v0 > 0.0) || !g.is_finite() {
            continue;
        }
        let c = (v1 / v0 - 1.0) / dy * 100.0;
        let diff = if g != 0.0 {
            (c - g).abs() / g.abs() * 100.0
        } else if c.abs() > 0.5 {
            f64::INFINITY
        } else {
            0.0
        };
        rows.push(MsmRow {
            link: id,
            corridor: r3(c),
            msm: g,
            diff_pct: if diff.is_finite() {
                Some(r1(diff))
            } else {
                None
            },
            flagged: diff > pct,
        });
    }
    let n = rows.iter().filter(|r| r.flagged).count();
    MsmCheck::Checked {
        checked: true,
        pct,
        basis: STANDARDS.msm.src.edition.to_string(),
        text: format!(
            "{} of {} links differ from MSM growth by more than {}% (Spare Construct assumption).",
            n,
            rows.len(),
            number_to_js_string(pct)
        ),
        rows,
        flagged: n as f64,
    }
}
