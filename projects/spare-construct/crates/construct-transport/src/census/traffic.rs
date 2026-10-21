//! Development traffic: where it goes by gateway, junctions without and with the development, the "without" scenario, and gateway flows from one chosen count
//! (landuse-plus.js "development traffic" and the gateway section's `useCount`).

use super::consts::{LANDUSES_CARD_USES, PERIODS};
use super::geo::parse_num;
use crate::analysis::{DevDistribution, WithWithout, dev_distribution, with_without};
use crate::calibrate::{junction_name, stored_res};
use crate::datahub::{self, AsText, Count, Data, gateways_from_counts};
use crate::jsnum::{value_to_js_string, value_truthy};
use crate::project::{Project, Scenario, land_use};
use crate::results::Res;
use crate::standards::year_of;
use crate::util::fmt;
use serde_json::{Value, json};
use std::rc::Rc;

/// `gwCounted(sc)`: how many gateways have flows set from counts, and how many there are.
pub fn gw_counted(sc: &Scenario) -> (usize, usize) {
    let g: Vec<_> = sc.zones.values().filter(|z| z.kind.is("gateway")).collect();
    (g.iter().filter(|z| z.gw_src.get().is_some_and(value_truthy)).count(), g.len())
}

/// `distBasis(sc, res, P, S)`: how the distribution of a result was made, in words.
pub fn dist_basis(p: &Project, sc: &Scenario, res: &Res) -> String {
    if res.manual {
        return "manual OD matrix entered by the user".to_string();
    }
    let (n, of) = gw_counted(sc);
    let gw = if of == 0 {
        "no gateways".to_string()
    } else if n == of {
        format!("gateway flows from counts at all {of} gateways")
    } else if n > 0 {
        format!("gateway flows from counts at {n} of {of} gateways (the others estimated from road class and lanes)")
    } else {
        format!("gateway flows estimated from road class and lanes at all {of} gateways (none from counts)")
    };
    let me = p
        .me
        .get()
        .and_then(|m| m.factors.get(&res.period))
        .and_then(|f| f.get())
        .is_some_and(|f| f.len() > 1);
    format!(
        "gravity model (\u{3b2} = {} per minute of free-flow time); {gw}{}",
        p.settings.gravity_beta.js_string(),
        if me { ", with matrix estimation factors fitted to counts" } else { "" }
    )
}

/// The distribution of development traffic over the gateways (`distribution`'s answer).
#[derive(Clone, Debug, PartialEq)]
pub struct Distribution {
    /// Shares and veh/h by gateway and the totals by period.
    pub dist: DevDistribution,
    /// By period, the shares in and out added up (%).
    pub sums: Vec<(String, f64, f64)>,
    /// How each period's distribution was made.
    pub basis: String,
    /// A result is out of date.
    pub stale: bool,
}

/// `distribution(sc)`: where the development traffic goes, from the AM and PM results the scenario has stored that have development trips; None when none do.
///
/// A result read back from a saved project lists its junctions and gateways alphabetically (the saved JSON is read into sorted maps), where the prototype keeps the
/// order the run made them in; the rows are then in the same order unless two have exactly the same traffic. Pass the results of a run to [`distribution_of`]
/// to keep the run's own order.
pub fn distribution(p: &Project, sc: &Scenario) -> Option<Distribution> {
    let res: Vec<Res> = PERIODS.iter().filter_map(|per| stored_res(sc, per)).collect();
    let refs: Vec<&Res> = res.iter().collect();
    distribution_of(p, sc, &refs)
}

/// `distribution(sc)` for results the caller holds (AM then PM): those with development trips are used.
pub fn distribution_of(p: &Project, sc: &Scenario, results: &[&Res]) -> Option<Distribution> {
    let res: Vec<&Res> = results.iter().copied().filter(|r| r.dev.trips > 0.0).collect();
    if res.is_empty() {
        return None;
    }
    let d = dev_distribution(Some(sc), &res);
    let sums = res
        .iter()
        .map(|r| {
            let total = |dir: &str| {
                d.rows.iter().fold(0.0, |s, x| {
                    let v = x.shares.get(&format!("{}{dir}", r.period)).copied().unwrap_or(0.0);
                    s + crate::jsnum::or_default(v, 0.0)
                })
            };
            (r.period.clone(), total("in"), total("out"))
        })
        .collect();
    let basis = res.iter().map(|r| format!("{}: {}", r.period, dist_basis(p, sc, r))).collect::<Vec<_>>().join("; ");
    let stale = res.iter().any(|r| r.stale);
    Some(Distribution { dist: d, sums, basis, stale })
}

/// `withoutScenario(sc, P)`: the index of the scenario without the development: the user's choice (`P.landuse.without`), else a same-year baseline, else the
/// base scenario if it is the same year, else another same-year scenario that is not a proposal or mitigation. A scenario of another year is never picked on its own.
pub fn without_scenario(p: &Project, sc_idx: usize) -> Option<usize> {
    let others: Vec<usize> = (0..p.scenarios.len()).filter(|i| *i != sc_idx).collect();
    let chosen = p.extra.get("landuse").and_then(|l| l.get("without")).filter(|x| value_truthy(x)).map(value_to_js_string);
    if let Some(c) = chosen
        && let Some(i) = others.iter().find(|i| p.scenarios[**i].id == c)
    {
        return Some(*i);
    }
    let yr = |i: usize| year_of(Some(&p.scenarios[i]), &p.settings);
    let me = yr(sc_idx);
    let same: Vec<usize> = others.into_iter().filter(|i| yr(*i) == me).collect();
    let role = |i: usize| p.scenarios[i].role.as_str().map(str::to_string);
    same.iter()
        .find(|i| role(**i).as_deref() == Some("baseline"))
        .or_else(|| same.iter().find(|i| p.base.is(&p.scenarios[**i].id)))
        .or_else(|| same.iter().find(|i| !matches!(role(**i).as_deref(), Some("proposal") | Some("mitigation"))))
        .copied()
}

/// Junctions without and with the development in one period (`withWithout`'s answer).
#[derive(Clone, Debug, PartialEq)]
pub struct JunctionComparison {
    /// The comparison, largest increase in delay first.
    pub ww: WithWithout,
    /// A result is out of date.
    pub stale: bool,
    /// The period.
    pub period: String,
}

/// `withWithout(sc, woSc, period)`: junctions without and with the development in a period, from the results the two scenarios have stored; None unless both have one
/// for it. (A result read back from a saved project lists its junctions alphabetically; see [`distribution`].)
pub fn with_without_period(sc: &Scenario, wo: &Scenario, period: &str) -> Option<JunctionComparison> {
    let a = stored_res(wo, period)?;
    let b = stored_res(sc, period)?;
    with_without_res(sc, &a, &b, period)
}

/// `withWithout(sc, woSc, period)` for results the caller holds: `without` is the scenario without the development, `with` the scenario `sc`.
pub fn with_without_res(sc: &Scenario, a: &Res, b: &Res, period: &str) -> Option<JunctionComparison> {
    let name = |id: &str| -> String {
        let nr = b.nodes.get(id).or_else(|| a.nodes.get(id));
        let site = nr.and_then(|n| n.site.clone()).filter(|s| !s.is_empty());
        let s = site.and_then(|s| b.sites.as_ref().and_then(|m| m.get(&s)));
        match s {
            Some(s) if !s.name.is_empty() => s.name.clone(),
            _ => junction_name(sc, id),
        }
    };
    let ww = with_without(Some(a), Some(b), Some(&name))?;
    Some(JunctionComparison { ww, stale: a.stale || b.stale, period: period.to_string() })
}

/// `gwSrcText(z)`: the count source of a gateway as "NZTA 01N00478 \u{b7} SH1 Drury \u{b7} 61,200/day \u{b7} NB": each count named in the gateway's `gwSrc` with its direction, or "both directions" for a two-way count.
pub fn gw_src_text(sc: &Scenario, gw_id: &str, counts: &[Rc<Count>]) -> String {
    let Some(z) = sc.zones.get(gw_id) else { return String::new() };
    let Some(src) = z.gw_src.get().filter(|v| value_truthy(v)) else { return String::new() };
    let short = |c: &Count| -> String {
        match c.src.as_text().and_then(datahub::source) {
            Some(s) => s.short.to_string(),
            None => c.src.js_string(),
        }
    };
    let label = |c: &Count| -> String {
        format!(
            "{}{} {} {}{}",
            short(c),
            if c.virtual_.truthy() { " estimate" } else { "" },
            if c.reference.truthy() { c.reference.js_string() } else { String::new() },
            c.name.js_string(),
            if c.adt.truthy() { format!(" ({}/day)", fmt::int(c.adt.num())) } else { String::new() }
        )
    };
    value_to_js_string(src)
        .split("; ")
        .map(|seg| match counts.iter().find(|c| label(c) == seg) {
            Some(c) => [
                format!("{}{}{}", short(c), if c.virtual_.truthy() { " estimate" } else { "" }, if c.reference.truthy() { format!(" {}", c.reference.js_string()) } else { String::new() }),
                c.name.js_string(),
                if c.adt.truthy() { format!("{}/day", fmt::int(c.adt.num())) } else { String::new() },
                if c.directional.truthy() && c.dir.truthy() { c.dir.js_string() } else { "both directions".to_string() },
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" \u{b7} "),
            None => seg.to_string(),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// What `useCount` gives: how many gateways were set and the source, or why none was.
#[derive(Clone, Debug, PartialEq)]
pub struct UseCount {
    /// Gateways set.
    pub n: usize,
    /// The count source text of the gateway.
    pub src: Option<String>,
    /// Why nothing was set.
    pub error: Option<String>,
}

/// `useCount(sc, gwId, countId)`: sets a gateway's flows from one chosen count (every other count is set aside for the call). The count must be on the
/// gateway's road and class within 3 km, as for the nearest counts.
pub fn use_count(p: &mut Project, sc_idx: usize, gw_id: &str, count_id: &str) -> UseCount {
    let fail = |m: String| UseCount { n: 0, src: None, error: Some(m) };
    if !p.scenarios[sc_idx].zones.get(gw_id).is_some_and(|z| z.kind.is("gateway")) {
        return fail("That gateway is no longer in this scenario.".to_string());
    }
    let data = Data::read(p);
    let Some(c) = data.counts.iter().find(|x| x.id == count_id).cloned() else {
        return fail("That count is no longer loaded.".to_string());
    };
    let c_name = if c.name.truthy() { c.name.js_string() } else { "That count".to_string() };
    if c.off.truthy() {
        return fail(format!("{c_name} is switched off in Traffic data. Switch it on to use it."));
    }
    // every other count is set aside for the call (switched off in a copy of the list, which is put back afterwards)
    let original = p.data.get().and_then(|d| d.extra.get("counts")).cloned();
    let mut aside = original.clone().unwrap_or_else(|| json!([]));
    if let Some(list) = aside.as_array_mut() {
        for x in list.iter_mut() {
            if x.get("id").and_then(Value::as_str) != Some(c.id.as_str())
                && let Some(o) = x.as_object_mut()
            {
                o.insert("off".into(), json!(true));
            }
        }
    }
    if let Some(d) = p.data.get_mut() {
        d.extra.insert("counts".into(), aside);
    }
    let only = Data::read(p);
    if let Some(d) = p.data.get_mut() {
        match original {
            Some(o) => {
                d.extra.insert("counts".into(), o);
            }
            None => {
                d.extra.remove("counts");
            }
        }
    }
    let s = p.settings.clone();
    let mut tmp = p.scenarios[sc_idx].clone();
    let z = tmp.zones.get(gw_id).cloned().expect("gateway");
    tmp.zones = crate::project::OrderedMap::new();
    tmp.zones.insert(gw_id.to_string(), z);
    let n = gateways_from_counts(&mut tmp, &s, &only);
    let z = tmp.zones.get(gw_id).cloned().expect("gateway");
    p.scenarios[sc_idx].zones.insert(gw_id.to_string(), z);
    if n > 0 {
        let counts = Data::read(p).counts;
        return UseCount { n, src: Some(gw_src_text(&p.scenarios[sc_idx], gw_id, &counts)), error: None };
    }
    let zname = p.scenarios[sc_idx].zones[gw_id].name.str_or("this gateway").to_string();
    fail(format!(
        "{c_name} cannot set {zname}'s flows: it must be on the gateway's road, of the same class, within 3 km (and its source switched on). Pick another count, or enter the flows by hand."
    ))
}

/// `cardCheck(k, raw)`: the answer of the zone card, checked: the land use and quantity, or an error message.
pub fn card_check(k: &str, raw: &Value) -> Result<(String, f64), String> {
    if k.is_empty() || land_use(k).is_none() {
        return Err("Choose a land use.".to_string());
    }
    match parse_num(raw) {
        Some(q) if q > 0.0 => Ok((k.to_string(), q)),
        _ => Err("Enter a number.".to_string()),
    }
}

/// The land uses the zone card offers first.
pub const CARD_USES: [&str; 6] = LANDUSES_CARD_USES;

/// The `{ k, q }` answer of [`card_check`] as JSON.
pub fn card_json(r: &Result<(String, f64), String>) -> Value {
    match r {
        Ok((k, q)) => json!({ "k": k, "q": crate::jsnum::json_number(*q) }),
        Err(e) => json!({ "error": e }),
    }
}

