//! The pre-lodgement check that reduces further-information requests (packs/nz/compliance.js section 11, the linter).
//!
//! The response template and the statutory clocks of the same section live in [`super::clocks`] (`rfi_template`, `clocks`).

use super::data::m as model;
use super::engine;
use super::jv::{Jx, V, jobj, ns, pad_start};
use super::net;
use super::record::{active_sc, assumption, periods_of, state};
use super::rx::rx;
use crate::graph::js_sort_by;
use crate::project::default_settings;

/// `out.push({ id, text, fix })`.
fn add(out: &mut Vec<V>, id: &str, text: String, fix: &str) {
    out.push(jobj! { "id" => id, "text" => text, "fix" => fix });
}

/// `n === 1 ? '' : 's'`.
fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// `new Set(...).add(v)`: adds a value unless an identical one is there.
fn set_add(set: &mut Vec<V>, v: &V) {
    if !set.iter().any(|x| x.strict_eq(v)) {
        set.push(v.clone());
    }
}

/// The property names every JavaScript object has: `LANDUSES[k]` finds a function there, with no `label`.
pub(crate) fn is_object_key(k: &str) -> bool {
    matches!(
        k,
        "constructor"
            | "__defineGetter__"
            | "__defineSetter__"
            | "hasOwnProperty"
            | "__lookupGetter__"
            | "__lookupSetter__"
            | "isPrototypeOf"
            | "propertyIsEnumerable"
            | "toString"
            | "valueOf"
            | "__proto__"
            | "toLocaleString"
    )
}

/// `(LANDUSES[k] || { label: k }).label`: None where JavaScript gives undefined (an object property name has no label).
pub(crate) fn landuse_label(k: &str) -> Option<String> {
    let e = model("LANDUSES").g(k);
    if e.t() {
        if e.has("label") {
            Some(e.g("label").sn())
        } else {
            None
        }
    } else if is_object_key(k) {
        None
    } else {
        Some(k.to_string())
    }
}

/// The project as the engine reads it. A trip rate can carry notes the checks read (a source, a survey year, whether it was
/// reviewed) next to its peak periods; the engine models none of them and cannot hold them, so the copy it reads leaves them out.
pub(crate) fn engine_view(p: &V) -> std::borrow::Cow<'_, V> {
    let plain =
        |r: &V| r.is_null() || r.is_obj() && r.vals().iter().all(|x| x.is_obj() || x.is_null());
    let rates = p.g("settings").g("rates");
    if !rates.is_obj() || rates.vals().iter().all(|r| plain(r)) {
        return std::borrow::Cow::Borrowed(p);
    }
    let mut q = p.clone();
    if let Some(rs) = q.get_mut("settings").and_then(|s| s.get_mut("rates")) {
        let keys: Vec<String> = rs.keys().iter().map(|k| k.to_string()).collect();
        for k in keys {
            let r = rs.g(&k).clone();
            if r.is_obj() {
                let mut kept = V::object();
                for (f, v) in r.entries() {
                    if v.is_obj() || v.is_null() {
                        kept.set(f, v.clone());
                    }
                }
                rs.set(k, kept);
            }
        }
    }
    std::borrow::Cow::Owned(q)
}

/// The rate entry of a land use (`(S.rates || {})[k] || {}`).
pub(crate) fn rate_of(settings: &V, k: &str) -> V {
    let r = settings.g("rates").g(k);
    if r.t() { r.clone() } else { V::object() }
}

/// `Compliance.lint(P)`: what to fix before lodging: `[{ id, text, fix }]` (counts without a date or source, an unmodelled observed peak, the default heavy vehicle share, altered junctions without pedestrian volumes, an uncalibrated base model, no sensitivity or cumulative scenario, trip rates without a source or year, unevidenced trip discounts, unfunded projects in an option).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // a number that is not a number fails `x > 0` and passes `!(x > 0)`, as in the prototype
pub fn lint(p: &V) -> V {
    let c = state(p);
    let s = p.g("settings");
    let sc = active_sc(p);
    let mut out: Vec<V> = Vec::new();
    let counts = p.g("data").g("counts").items();
    let undated: Vec<&V> = counts
        .iter()
        .filter(|c| !c.g("date").t() && !c.g("src").eq_s("nzta"))
        .collect();
    if !undated.is_empty() {
        add(
            &mut out,
            "count-date",
            format!(
                "{} count{} without a date{} (count without date)",
                undated.len(),
                plural(undated.len()),
                if undated.iter().any(|c| !c.g("src").t()) {
                    " or source"
                } else {
                    ""
                }
            ),
            "Record the survey date and source for each count (AT ITA v2 s4.2).",
        );
    }
    if counts.iter().any(|c| !c.g("src").t()) {
        add(
            &mut out,
            "count-source",
            "Counts without a source".to_string(),
            "Record where each count came from.",
        );
    }
    let re_hr = rx(r"^(\d{2}):", "");
    let hrs: Vec<i64> = counts
        .iter()
        .filter_map(|c| {
            let t = c.g("peakTime");
            re_hr.exec(&if t.t() { t.s() } else { String::new() })
        })
        .map(|m| m.get(1).parse::<i64>().unwrap_or(0))
        .collect();
    if hrs.len() >= 3 {
        // the hour that most counts peak in (a Map in insertion order, sorted by count, most first)
        let mut tally: Vec<(i64, f64)> = Vec::new();
        for h in &hrs {
            match tally.iter_mut().find(|e| e.0 == *h) {
                Some(e) => e.1 += 1.0,
                None => tally.push((*h, 1.0)),
            }
        }
        js_sort_by(&mut tally, |a, b| b.1 - a.1);
        let mode = tally[0].0;
        let mut modelled: Vec<&str> = Vec::new();
        for sc in p.g("scenarios").items() {
            for per in periods_of(sc) {
                if !modelled.contains(&per) {
                    modelled.push(per);
                }
            }
        }
        let in_am = (6..=9).contains(&mode);
        let in_pm = (15..=18).contains(&mode);
        if !((in_am && (modelled.contains(&"AM") || modelled.is_empty()))
            || (in_pm && (modelled.contains(&"PM") || modelled.is_empty())))
        {
            add(
                &mut out,
                "peak",
                format!(
                    "The observed peak in the counts is around {}:00 ({} counts), which is not one of the modelled periods (AM 06\u{2013}09, PM 15\u{2013}18; {})",
                    pad_start(&mode.to_string(), 2, '0'),
                    hrs.len(),
                    assumption()
                ),
                "Assess the development and network peaks (AT ITA v2 s3.6.1(vi)); consider an inter-peak or weekend period.",
            );
        }
    }
    if s.g("hvPct").n() == default_settings().hv_pct.num()
        && !counts.iter().any(|c| c.g("hv").n() > 0.0)
    {
        add(
            &mut out,
            "hv",
            format!(
                "Heavy vehicle share is the Spare Construct default ({}%) and no count gives one",
                s.g("hvPct").s()
            ),
            "Use classified counts for the HV share.",
        );
    }
    let mut altered: Vec<V> = Vec::new();
    for n in sc.g("nodes").vals() {
        if n.g("upgrade").t() {
            set_add(&mut altered, n.g("id"));
        }
    }
    for l in sc.g("links").vals() {
        if l.g("status").eq_s("proposed") {
            for id in [l.g("a"), l.g("b")] {
                if net::road_links(sc, &id.s()).len() >= 3 {
                    set_add(&mut altered, id);
                }
            }
        }
    }
    let no_peds = altered
        .iter()
        .filter(|id| !(c.g("peds").g(&id.s()).n() > 0.0))
        .count();
    if no_peds > 0 {
        add(
            &mut out,
            "peds",
            format!(
                "{} altered junction{} without pedestrian volumes",
                no_peds,
                plural(no_peds)
            ),
            "Enter a pedestrian count for each altered junction (select the junction \u{203a} Compliance).",
        );
    }
    let ev = engine_view(p);
    let ms = engine::model_status(&ev, "AM");
    if !ms.t() || ms.g("status").eq_s("uncalibrated") {
        add(
            &mut out,
            "base",
            "The base model is uncalibrated".to_string(),
            "Fit the matrix to counts and validate against independent counts (TMDG 2019).",
        );
    }
    let scenarios = p.g("scenarios").items();
    if !scenarios.iter().any(|s| s.g("role").eq_s("sensitivity")) {
        add(
            &mut out,
            "sens",
            "No sensitivity scenario".to_string(),
            "Run the sensitivity pack or add a sensitivity scenario (Practice Note s9.3(a)(x); AT ITA v2 s3.6.1(v)).",
        );
    }
    if !scenarios.iter().any(|s| {
        s.g("zones")
            .vals()
            .iter()
            .any(|z| engine::zone_role(z) == "committed")
    }) && p.g("register").jlen() == 0
    {
        add(
            &mut out,
            "cumulative",
            "No cumulative (committed development) scenario".to_string(),
            "Add committed developments to the register and a future baseline (AT ITA v2 s3.6.1).",
        );
    }
    let mut used: Vec<String> = Vec::new();
    for z in net::dev_zones(sc) {
        for (k, v) in z.g("lu").entries() {
            if v.n() > 0.0 && !used.iter().any(|u| u == k) {
                used.push(k.to_string());
            }
        }
    }
    let no_src: Vec<&String> = used
        .iter()
        .filter(|k| {
            let r = rate_of(s, k);
            !(r.g("src").t() || r.g("source").t()) || !(r.g("year").n() > 0.0)
        })
        .collect();
    if !no_src.is_empty() {
        add(
            &mut out,
            "rates",
            format!(
                "Trip rates without a source or year: {}",
                no_src
                    .iter()
                    .map(|k| landuse_label(k).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            "Record the source and survey year of each rate used (AT ITA v2 Appendix 1).",
        );
    }
    let old: Vec<&String> = used
        .iter()
        .filter(|k| {
            let r = rate_of(s, k);
            r.g("year").n() > 0.0 && r.g("year").n() <= 2022.0 && !r.g("reviewed").t()
        })
        .collect();
    if !old.is_empty() {
        add(
            &mut out,
            "rates-old",
            format!(
                "Pre-2020 or 2020\u{2013}22 trip rates not reviewed: {}",
                old.iter()
                    .map(|k| landuse_label(k).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            "Review older rates against post-2022 travel patterns (AT ITA v2 s3.6.1).",
        );
    }
    let tdm = net::dev_zones(sc)
        .iter()
        .filter(|z| z.g("reduction").n() > 0.0 && !z.g("reductionSrc").t())
        .count();
    if tdm > 0 {
        add(
            &mut out,
            "tdm",
            format!(
                "{} zone{} with a WFH, TDM or mode-shift discount without evidence",
                tdm,
                plural(tdm)
            ),
            "Give evidence for each trip discount (AT ITA v2 s3.6.1(iv)).",
        );
    }
    let mut blocks = 0usize;
    for s in scenarios
        .iter()
        .filter(|s| !s.g("role").eq_s("sensitivity"))
    {
        blocks += engine::scenario_check(&ev, s)
            .items()
            .iter()
            .filter(|x| x.g("level").eq_s("block") && x.g("code").eq_s("unfunded"))
            .count();
    }
    if blocks > 0 {
        add(
            &mut out,
            "unfunded",
            format!(
                "Unfunded projects in a do-minimum or option scenario ({})",
                ns(blocks as f64)
            ),
            "Unfunded projects belong in sensitivity scenarios only (AT ITA s3.6.1).",
        );
    }
    V::Array(out)
}
