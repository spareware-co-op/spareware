//! Helpers several report sections share: the regime, the assessed junctions and their summary figures, junction groups,
//! the crash years, speeds and named roads.

use super::consts::{MAX_SITES, PATHWAY_LABELS, hrig, reg};
use super::js;
use super::texts::DISCLAIMERS_VERSION;
use super::{Rep, Sel};
use crate::edit::json::JsonValue;
use crate::edit::net::links_at;
use crate::graph::{js_sort_by, js_str_lt};
use crate::jsnum::{math_max, number_to_js_string};
use crate::project::{Link, Scenario, road_class};
use crate::results::{NodeRes, Res};
use crate::util::fmt;
use std::collections::HashMap;

/// `Kit.junctionName(id, sc)`: the roads (up to three) that meet at a node, or 'Unnamed junction'.
pub(crate) fn kit_junction_name(sc: &Scenario, id: &str) -> String {
    let mut names: Vec<&str> = Vec::new();
    for l in links_at(sc, id) {
        if let Some(n) = l.name.as_str().filter(|n| !n.is_empty())
            && !names.contains(&n)
        {
            names.push(n);
        }
    }
    if names.is_empty() {
        "Unnamed junction".to_string()
    } else {
        names.into_iter().take(3).collect::<Vec<_>>().join(" / ")
    }
}

/// The pathway, lodgement date and regime: one source of truth (`regCtx(P)`).
#[derive(Clone, Debug, PartialEq)]
pub struct RegCtx {
    /// "compliance" or "report": where it came from.
    pub from: &'static str,
    /// Pathway key.
    pub pathway: String,
    /// Pathway name.
    pub label: String,
    /// Lodgement date.
    pub date: String,
    /// The regime in words.
    pub regime: String,
}

/// `regCtx(P)`: the Compliance tab's context wins when it is set; the Report tab's own fields are the fallback.
pub(crate) fn reg_ctx(r: &Rep) -> RegCtx {
    let cx = js::path(Some(r.raw), &["compliance", "ctx"]);
    if let Some(cx) = cx.filter(|c| c.truthy())
        && (js::truthy(js::get(Some(cx), "pathway")) || js::truthy(js::get(Some(cx), "lodgement")))
        && let Some(g) = r.host.regime(r.project)
    {
        return RegCtx {
            from: "compliance",
            label: if g.pathway.is_empty() { String::new() } else { g.pathway_label.clone() },
            pathway: g.pathway,
            date: g.date,
            regime: g.text,
        };
    }
    let d = &r.rs.details;
    let pw = {
        let p = js::text_or_empty(d.get("pathway"));
        if !p.is_empty() {
            p
        } else {
            js::text_or_empty(js::path(Some(r.raw), &["assessment", "type"]))
        }
    };
    let lodgement = d.get("lodgement");
    let regime = if pw == "ftaa" {
        reg::FTAA.to_string()
    } else if js::truthy(lodgement) {
        let before = match lodgement {
            Some(JsonValue::String(s)) => js_str_lt(s, reg::AMENDMENT),
            other => js::num(other) < js::num(Some(&JsonValue::String(reg::AMENDMENT.to_string()))),
        };
        if before { reg::PRE.to_string() } else { reg::POST.to_string() }
    } else {
        "Lodgement date not stated: the regime cannot be set (Planning Act 2026 Sch 1 cl 18\u{2013}21).".to_string()
    };
    let label = if pw.is_empty() {
        String::new()
    } else {
        PATHWAY_LABELS
            .iter()
            .find(|(k, _)| *k == pw)
            .map_or(pw.clone(), |(_, l)| l.to_string())
    };
    RegCtx {
        from: "report",
        pathway: pw,
        label,
        date: js::text_or_empty(lodgement),
        regime,
    }
}

/// `modelVersion(P)`.
pub(crate) fn model_version(r: &Rep) -> String {
    let sv = r.st().sv.or(1.0);
    format!(
        "Spare Construct v2 \u{b7} settings v{} \u{b7} report {}",
        number_to_js_string(sv),
        DISCLAIMERS_VERSION
    )
}

/// `nodeKey(rs, id)`: the `res.nodes` key for a node or junction-site id.
fn node_key(rl: &[&Res], id: &str) -> Option<String> {
    for r in rl {
        if r.nodes.contains_key(id) {
            return Some(id.to_string());
        }
        for (k, nr) in r.nodes.iter() {
            if nr.site.as_deref() == Some(id) {
                return Some(k.to_string());
            }
        }
    }
    None
}

/// `siteName(sc, res, id)`: a junction's name from its site, its roads or its site's first node.
pub(crate) fn site_name(sc: &Scenario, res: Option<&Res>, id: &str) -> String {
    if let Some(res) = res {
        if let Some(nr) = res.nodes.get(id)
            && let Some(sid) = &nr.site
            && let Some(s) = res.sites.as_ref().and_then(|m| m.get(sid))
            && !s.name.is_empty()
        {
            return s.name.clone();
        }
        if sc.nodes.contains_key(id) {
            return kit_junction_name(sc, id);
        }
        if let Some(s) = res.sites.as_ref().and_then(|m| m.get(id))
            && !s.name.is_empty()
        {
            return s.name.clone();
        }
    } else if sc.nodes.contains_key(id) {
        return kit_junction_name(sc, id);
    }
    if let Some(st) = sc.sites.get().and_then(|m| m.get(id)) {
        if let Some(n) = st.name.as_str().filter(|n| !n.is_empty()) {
            return n.to_string();
        }
        if let Some(n0) = st.nodes.iter().find(|x| sc.nodes.contains_key(*x)) {
            return kit_junction_name(sc, n0);
        }
    }
    "Unknown junction".to_string()
}

/// The junctions the report assesses (`assessedSites`).
#[derive(Clone, Debug, PartialEq)]
pub struct Assessed {
    /// The `res.nodes` keys.
    pub ids: Vec<String>,
    /// How they were chosen.
    pub basis: String,
}

/// `assessedSites(P, sel)`: the extent-of-effects sites when the test has been stored, otherwise signals, roundabouts and
/// upgraded junctions, then the busiest give-way junctions (DoS 0.5 or more), up to 20.
pub(crate) fn assessed_sites(r: &Rep, with: Option<usize>) -> Assessed {
    let Some(w) = with else {
        return Assessed {
            ids: Vec::new(),
            basis: "No scenario to assess.".to_string(),
        };
    };
    let sc = r.scenario(w);
    let rl: Vec<&Res> = r
        .rep_periods(w)
        .iter()
        .filter_map(|p| r.results[w].get(p))
        .collect();
    if rl.is_empty() {
        return Assessed {
            ids: Vec::new(),
            basis: "The model has not been run for the assessed scenario.".to_string(),
        };
    }
    let ext: Option<Vec<String>> = sc
        .extent
        .get()
        .and_then(|e| e.get("sites"))
        .and_then(|s| s.as_array())
        .filter(|a| !a.is_empty())
        .map(|a| a.iter().map(|v| crate::jsnum::value_to_js_string(v)).collect());
    if let Some(ext) = ext {
        let mut ids: Vec<String> = Vec::new();
        for id in &ext {
            if let Some(k) = node_key(&rl, id)
                && !ids.contains(&k)
            {
                ids.push(k);
            }
        }
        let n = ids.len();
        ids.truncate(40);
        return Assessed {
            ids,
            basis: format!(
                "The {} junctions inside the extent of effects (AT ITA Guidelines 2025 v2 s4.2.1.1).",
                n
            ),
        };
    }
    let mut score: Vec<(String, f64, String)> = Vec::new();
    for res in &rl {
        for (id, nr) in res.nodes.iter() {
            if nr.r#type == "none" {
                continue;
            }
            let dos = if nr.dos == 0.0 || nr.dos.is_nan() { 0.0 } else { nr.dos };
            match score.iter().position(|(k, _, _)| k == id) {
                Some(p) => {
                    score[p].1 = math_max(score[p].1, dos);
                    score[p].2 = nr.r#type.clone();
                }
                None => score.push((id.to_string(), math_max(0.0, dos), nr.r#type.clone())),
            }
        }
    }
    let sc_of = |id: &str| score.iter().find(|(k, _, _)| k == id).map_or(0.0, |x| x.1);
    let mut must: Vec<String> = score
        .iter()
        .filter(|(id, _, _)| sc.nodes.get(id).is_some_and(|n| n.upgrade.truthy()))
        .map(|(id, _, _)| id.clone())
        .collect();
    js_sort_by(&mut must, |a, b| sc_of(b) - sc_of(a));
    let mut rest: Vec<String> = score
        .iter()
        .filter(|(id, s, t)| !must.contains(id) && (*s >= 0.5 || t.contains("signals") || t.contains("roundabout")))
        .map(|(id, _, _)| id.clone())
        .collect();
    js_sort_by(&mut rest, |a, b| sc_of(b) - sc_of(a));
    let n = MAX_SITES.max(must.len());
    let mut ids: Vec<String> = must.into_iter().chain(rest).collect();
    ids.truncate(n);
    Assessed {
        ids,
        basis: format!(
            "Spare Construct selection: upgraded junctions, then the busiest signals, roundabouts and give-way junctions (give way at DoS 0.5 or more), up to {}. Run the extent of effects test to set the assessed sites by AT ITA s4.2.1.1.",
            MAX_SITES
        ),
    }
}

/// Summary figures of one junction result row (`jSum`).
#[derive(Clone, Debug, PartialEq)]
pub struct JSum {
    /// Degree of saturation.
    pub dos: f64,
    /// Delay (s): average for signals and roundabouts, worst movement otherwise.
    pub delay: f64,
    /// Level of service.
    pub los: String,
    /// Level of service as shown.
    pub los_text: String,
    /// Basis of the level of service or delay.
    pub basis: String,
    /// Longest 95% queue (m).
    pub q95: f64,
    /// Its leg.
    pub leg: String,
    /// Practical degree of saturation.
    pub xp: f64,
    /// Control.
    pub kind: String,
    /// Volume.
    pub vol: f64,
    /// Cycle (s).
    pub c: Option<f64>,
}

/// `jSum(nr)`.
pub(crate) fn j_sum(nr: &NodeRes) -> JSum {
    let sig = nr.r#type == "signals" || nr.r#type == "roundabout";
    let delay = if sig { nr.avg_delay } else { nr.worst_delay };
    let mut q = 0.0f64;
    let mut leg = String::new();
    for a in &nr.appr {
        let a_q = if a.q95 == 0.0 || a.q95.is_nan() { 0.0 } else { a.q95 };
        if a_q > q {
            q = a.q95;
            leg = format!(
                "{}{}",
                if a.approach.is_empty() { String::new() } else { format!("{}: ", a.approach) },
                a.name
            );
        }
    }
    JSum {
        dos: nr.dos,
        delay,
        los: nr.los.clone(),
        los_text: if nr.los_text.is_empty() { nr.los.clone() } else { nr.los_text.clone() },
        basis: if !nr.los_basis.is_empty() { nr.los_basis.clone() } else { nr.delay_basis.clone() },
        q95: q,
        leg,
        xp: nr.xp,
        kind: nr.r#type.clone(),
        vol: nr.vol,
        c: nr.c,
    }
}

/// A row of the junction tables: an assessed junction in one period (`junctionRows`).
#[derive(Clone)]
pub(crate) struct JRow<'a> {
    pub id: String,
    pub p: &'static str,
    pub name: String,
    pub s: Option<JSum>,
    pub nr: Option<&'a NodeRes>,
}

/// `junctionRows(P, c, sc, ids)`: the assessed junctions of a scenario by period. For a scenario other than the assessed
/// one the prototype's list of the assessed junctions is empty, so it uses its own selection.
pub(crate) fn junction_rows<'a>(r: &Rep<'a>, sel: &Sel, sc: usize, ids: Option<&[String]>) -> Vec<JRow<'a>> {
    let rl = r.rep_periods(sc);
    let mut out = Vec::new();
    if rl.is_empty() {
        return out;
    }
    let list: Vec<String> = match ids {
        Some(i) => i.to_vec(),
        None => {
            if Some(sc) == sel.with {
                assessed_sites(r, sel.with).ids
            } else {
                Vec::new()
            }
        }
    };
    let mut use_ids = list.clone();
    if ids.is_none() && Some(sc) != sel.with {
        let own = assessed_sites(r, Some(sc)).ids;
        use_ids = if !list.is_empty() {
            list.iter()
                .filter(|id| rl.iter().any(|p| r.results[sc].get(p).is_some_and(|res| res.nodes.contains_key(*id))))
                .cloned()
                .collect()
        } else {
            own.clone()
        };
        if use_ids.is_empty() {
            use_ids = own;
        }
    }
    let scn = r.scenario(sc);
    for id in &use_ids {
        for p in &rl {
            let res = r.results[sc].get(p).expect("a period with results");
            let nr = res.nodes.get(id);
            out.push(JRow {
                id: id.clone(),
                p,
                name: site_name(scn, Some(res), id),
                s: nr.map(j_sum),
                nr,
            });
        }
    }
    out
}

/// A junction made of one or more assessed nodes (`junctionGroups`).
#[derive(Clone, Debug)]
pub(crate) struct JGroup {
    /// First assessed node.
    pub id: String,
    /// Assessed nodes in it.
    pub ids: Vec<String>,
    /// Every member node.
    pub nodes: Vec<String>,
    /// Name, or empty.
    pub name: String,
}

/// `junctionGroups(P, sc, scw, ids)`: groups node ids into junctions: a roundabout or divided junction of several nodes is one
/// site (the data hub's junction of the reference scenario, which needs crashes loaded), else the junction site of the result,
/// else the node alone.
pub(crate) fn junction_groups(r: &Rep, sc: usize, scw: usize, ids: &[String]) -> Vec<JGroup> {
    let has_crashes = !js::items(js::get(r.data(), "crashes")).is_empty();
    let nr: Option<Vec<(String, super::host::NodeRiskInfo)>> = if has_crashes { r.host.node_risk(sc) } else { None };
    let nr = nr.unwrap_or_default();
    let rl: Vec<&Res> = r
        .rep_periods(scw)
        .iter()
        .filter_map(|p| r.results[scw].get(p))
        .collect();
    let scr = r.scenario(sc);
    let mut out: Vec<JGroup> = Vec::new();
    let mut by: Vec<(String, usize)> = Vec::new();
    for id in ids {
        let mut key: Option<String> = None;
        let mut nodes = vec![id.clone()];
        let mut name = String::new();
        let j = if scr.nodes.contains_key(id) { r.host.junction_of(sc, id) } else { None };
        let hit = j.as_ref().and_then(|j| nr.iter().find(|(k, _)| k == j));
        if let (Some(j), Some((_, info))) = (&j, hit) {
            key = Some(format!("j:{j}"));
            nodes = info.nodes.clone();
            name = info.name.clone();
        } else {
            let site = rl
                .iter()
                .filter_map(|res| res.nodes.get(id).and_then(|n| n.site.clone()))
                .find(|s| !s.is_empty());
            if let Some(site) = site {
                key = Some(format!("s:{site}"));
                let res = rl.iter().find(|x| x.sites.as_ref().is_some_and(|m| m.contains_key(&site)));
                name = res
                    .and_then(|x| x.sites.as_ref().and_then(|m| m.get(&site)))
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                nodes = rl.first().map_or(Vec::new(), |r0| {
                    r0.nodes
                        .keys()
                        .filter(|k| {
                            rl.iter()
                                .any(|x| x.nodes.get(*k).is_some_and(|n| n.site.as_deref() == Some(site.as_str())))
                        })
                        .map(str::to_string)
                        .collect()
                });
            }
        }
        let key = key.unwrap_or_else(|| format!("n:{id}"));
        if let Some((_, gi)) = by.iter().find(|(k, _)| *k == key) {
            out[*gi].ids.push(id.clone());
            continue;
        }
        let mut members: Vec<String> = vec![id.clone()];
        for n in nodes {
            if !members.contains(&n) {
                members.push(n);
            }
        }
        by.push((key, out.len()));
        out.push(JGroup {
            id: id.clone(),
            ids: vec![id.clone()],
            nodes: members,
            name: if !name.is_empty() && name != "Unnamed junction" { name } else { String::new() },
        });
    }
    out
}

/// `junctionCount(P, c, ids)`: nodes and junctions among node ids: a roundabout of four nodes counts once.
pub(crate) fn junction_count(r: &Rep, sel: &Sel, ids: &[String]) -> (usize, usize) {
    let sc = sel.existing.or(sel.base).unwrap_or(0);
    let scw = sel.with.unwrap_or(sc);
    (ids.len(), junction_groups(r, sc, scw, ids).len())
}

/// The crash history years (`crashYears`): counts by year, severity and vulnerable user.
#[derive(Clone, Debug, Default)]
pub struct Tally {
    /// Crashes.
    pub n: f64,
    /// Fatal.
    pub f: f64,
    /// Serious.
    pub s: f64,
    /// Minor.
    pub m: f64,
    /// Non-injury.
    pub nn: f64,
    /// Pedestrian.
    pub p: f64,
    /// Cyclist.
    pub c: f64,
    /// Motorcyclist.
    pub mc: f64,
    /// Deaths.
    pub deaths: f64,
    /// Serious injuries.
    pub si: f64,
    /// Dark.
    pub dark: f64,
    /// Wet.
    pub wet: f64,
}

/// What the crash fields hold.
#[derive(Clone, Copy, Debug, Default)]
pub struct CrashHas {
    /// Deaths are recorded.
    pub deaths: bool,
    /// Serious injuries are recorded.
    pub si: bool,
    /// Light is recorded.
    pub light: bool,
    /// Weather is recorded.
    pub weather: bool,
}

/// The crash years of the downloaded CAS area.
#[derive(Clone, Debug)]
pub struct CrashYears {
    /// First full year.
    pub from: f64,
    /// Last full year.
    pub to: f64,
    /// The year to date.
    pub ytd: f64,
    /// Totals by year, in order.
    pub years: Vec<(f64, Tally)>,
    /// The five-year total.
    pub total: Tally,
    /// The year to date.
    pub y: Tally,
    /// What is recorded.
    pub has: CrashHas,
    /// A full year still inside the CAS entry lag.
    pub lag_year: Option<f64>,
    /// The CAS window label.
    pub label: String,
}

/// Positions of the extra CAS fields in a crash row (`CX`).
pub(crate) mod cx {
    /// Fatal count.
    pub const FATAL: usize = 7;
    /// Serious injury count.
    pub const SERIOUS: usize = 8;
    /// Minor injury count.
    pub const MINOR: usize = 9;
    /// Light.
    pub const LIGHT: usize = 10;
    /// Weather.
    pub const WEATHER: usize = 11;
    /// OBJECTID.
    pub const ID: usize = 12;
    /// Traffic control.
    pub const CONTROL: usize = 13;
    /// Urban.
    pub const URBAN: usize = 14;
}

/// `cnum(k, i)`: a numeric crash field, or none.
pub(crate) fn cnum(k: &JsonValue, i: usize) -> Option<f64> {
    let v = js::idx(Some(k), i)?;
    match v {
        JsonValue::Null => None,
        JsonValue::String(s) if s.is_empty() => None,
        _ => {
            let x = v.to_number();
            if x.is_finite() { Some(x) } else { None }
        }
    }
}

/// `ctext(k, i)`: a text crash field, or empty.
pub(crate) fn ctext(k: &JsonValue, i: usize) -> String {
    match js::idx(Some(k), i) {
        Some(v) if !v.is_null() && !v.is_object_like() => v.js_string(),
        _ => String::new(),
    }
}

fn contains_ci(s: &str, words: &[&str]) -> bool {
    let l = s.to_lowercase();
    words.iter().any(|w| l.contains(w))
}

/// `crashYears(P)`: CR1 rows over the five full years plus the year to date; None with no crashes.
pub(crate) fn crash_years(r: &Rep) -> Option<CrashYears> {
    let cr = js::items(js::get(r.data(), "crashes"));
    if cr.is_empty() {
        return None;
    }
    let cas = js::get(r.loaded(), "cas");
    let yr = |v: Option<&JsonValue>| -> Option<f64> {
        let x = js::num(v);
        if x > 1900.0 && x < 3000.0 { Some(x.floor()) } else { None }
    };
    let ys: Vec<f64> = cr
        .iter()
        .map(|c| js::num(js::idx(Some(c), 3)))
        .filter(|y| *y > 0.0 && *y < 3000.0)
        .collect();
    if ys.is_empty() && yr(js::get(cas, "from")).is_none() {
        return None;
    }
    let min_y = ys.iter().copied().fold(f64::INFINITY, f64::min);
    let max_y = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let from = yr(js::get(cas, "from")).unwrap_or(min_y);
    let to = yr(js::get(cas, "to")).unwrap_or(max_y - if js::truthy(js::get(cas, "ytd")) { 0.0 } else { 1.0 });
    let ytd = yr(js::get(cas, "ytd")).unwrap_or(to + 1.0);
    let mut years: Vec<(f64, Tally)> = Vec::new();
    let mut k = from;
    while k <= to {
        years.push((k, Tally::default()));
        k += 1.0;
    }
    let mut total = Tally::default();
    let mut y = Tally::default();
    let mut has = CrashHas::default();
    for c in cr {
        let yy = js::num(js::idx(Some(c), 3));
        let sev = js::idx(Some(c), 2);
        let sev_s = sev.map(|v| v.js_string()).unwrap_or_default();
        let in_years = yy >= from && yy <= to;
        // the target: a full year, or the year to date
        let target_is_years = in_years;
        let target_is_ytd = !in_years && yy == ytd;
        if !(target_is_years || target_is_ytd) || sev_s.chars().count() != 1 || !"FSMN".contains(sev_s.as_str()) {
            continue;
        }
        let df = cnum(c, cx::FATAL);
        let ds = cnum(c, cx::SERIOUS);
        let lt = ctext(c, cx::LIGHT);
        let wt = ctext(c, cx::WEATHER);
        if df.is_some() {
            has.deaths = true;
        }
        if ds.is_some() {
            has.si = true;
        }
        if !lt.is_empty() {
            has.light = true;
        }
        if !wt.is_empty() {
            has.weather = true;
        }
        let flags = js::text_or_empty(js::idx(Some(c), 5));
        let mut add = |o: &mut Tally| {
            o.n += 1.0;
            match sev_s.as_str() {
                "F" => o.f += 1.0,
                "S" => o.s += 1.0,
                "M" => o.m += 1.0,
                _ => o.nn += 1.0,
            }
            for fl in ['p', 'c', 'm'] {
                if flags.contains(fl) && "FSM".contains(sev_s.as_str()) {
                    match fl {
                        'p' => o.p += 1.0,
                        'c' => o.c += 1.0,
                        _ => o.mc += 1.0,
                    }
                }
            }
            o.deaths += df.unwrap_or(0.0);
            o.si += ds.unwrap_or(0.0);
            if contains_ci(&lt, &["dark"]) {
                o.dark += 1.0;
            }
            if contains_ci(&wt, &["rain", "snow", "hail", "sleet", "wet"]) {
                o.wet += 1.0;
            }
        };
        if target_is_ytd {
            add(&mut y);
        } else if let Some(slot) = years.iter_mut().find(|(k, _)| *k == yy) {
            add(&mut slot.1);
            if yy != ytd {
                add(&mut total);
            }
        }
    }
    let lag = yr(js::get(cas, "lagYear"));
    let label = match js::get(cas, "label") {
        Some(l) if l.truthy() => l.js_string(),
        _ => format!(
            "{}\u{2013}{} plus {} to date",
            number_to_js_string(from),
            number_to_js_string(to),
            number_to_js_string(ytd)
        ),
    };
    Some(CrashYears {
        from,
        to,
        ytd,
        years,
        total,
        y,
        has,
        lag_year: lag,
        label,
    })
}

/// `speedOf(L)`: the legal speed limit of a link, else the coded (modelled) speed.
pub(crate) struct Speed {
    /// The speed (km/h), none when there is none.
    pub v: Option<f64>,
    /// Its source.
    pub src: String,
}

/// `speedOf(L)`.
pub(crate) fn speed_of(l: &Link) -> Speed {
    let ex = |k: &str| l.extra.get(k);
    let lv = |v: Option<&serde_json::Value>| v.map_or(f64::NAN, crate::jsnum::value_to_number);
    let n = ex("nslr").filter(|v| crate::jsnum::value_truthy(v));
    let sl = lv(ex("speedLimit"));
    let lim = if sl > 0.0 {
        sl
    } else if let Some(n) = n {
        if n.is_object() {
            let v = n.get("v").filter(|x| crate::jsnum::value_truthy(x)).or_else(|| n.get("limit"));
            lv(v)
        } else {
            crate::jsnum::value_to_number(n)
        }
    } else {
        lv(ex("nslr"))
    };
    let source = ex("speedSource").filter(|v| crate::jsnum::value_truthy(v));
    let is_nslr = source.is_some_and(|v| v.as_str() == Some("NSLR"));
    if lim > 0.0 && (is_nslr || n.is_some()) {
        return Speed {
            v: Some(lim),
            src: "NSLR".to_string(),
        };
    }
    let sp = l.speed.num();
    Speed {
        v: if sp == 0.0 || sp.is_nan() { None } else { Some(sp) },
        src: match source {
            Some(s) if s.as_str() != Some("NSLR") => format!("{} (not NSLR)", crate::jsnum::value_to_js_string(s)),
            _ => match ex("speedSrc").filter(|v| crate::jsnum::value_truthy(v)) {
                Some(s) => crate::jsnum::value_to_js_string(s),
                None => "coded (not NSLR)".to_string(),
            },
        },
    }
}

/// A named road of a scenario: its links, class and length.
pub(crate) struct RoadGroup<'a> {
    pub name: String,
    pub cls: String,
    pub links: Vec<&'a Link>,
    pub km: f64,
}

/// `roadGroups(sc)`: named roads of a scenario, grouped (arterial and above, plus collectors), longest first within a class.
pub(crate) fn road_groups(sc: &Scenario) -> Vec<RoadGroup<'_>> {
    let mut g: Vec<RoadGroup> = Vec::new();
    for l in sc.links.values() {
        if l.status.is("closed") {
            continue;
        }
        let Some(name) = l.name.as_str().filter(|n| !n.is_empty()) else {
            continue;
        };
        let c = road_class(l.cls.as_str());
        if c.rank < 3.0 {
            continue;
        }
        let cls = l.cls.as_str().unwrap_or("").to_string();
        let k = format!("{name}|{cls}");
        let i = match g.iter().position(|x| format!("{}|{}", x.name, x.cls) == k) {
            Some(i) => i,
            None => {
                g.push(RoadGroup {
                    name: name.to_string(),
                    cls: cls.clone(),
                    links: Vec::new(),
                    km: 0.0,
                });
                g.len() - 1
            }
        };
        g[i].links.push(l);
        g[i].km += sc.link_len(l) / 1000.0;
    }
    js_sort_by(&mut g, |a, b| {
        let ra = road_class(Some(a.cls.as_str())).rank;
        let rb = road_class(Some(b.cls.as_str())).rank;
        let d = rb - ra;
        if d != 0.0 && !d.is_nan() { d } else { b.km - a.km }
    });
    g
}

/// `R.num(x, d)` for a count of things: whole numbers with grouping.
pub(crate) fn plural(n: f64, one: &str, many: &str) -> String {
    format!("{} {}", fmt::int(n), if n == 1.0 { one } else { many })
}

/// The collective risk constants re-exported for the crash tables.
pub(crate) const HRIG_RADIUS: f64 = hrig::RADIUS;

/// Index of scenario ids by id (for lookups by the report's figure requests).
pub(crate) fn scenario_ids(r: &Rep) -> HashMap<String, usize> {
    r.project
        .scenarios
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.clone(), i))
        .collect()
}
