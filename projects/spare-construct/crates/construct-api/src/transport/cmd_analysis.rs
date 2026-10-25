//! Analysis commands: travel times, catchments, select link, differences, with and without, extent test, development distribution,
//! capital, economics, comparison, the sensitivity pack, model status and metadata.

use super::Session;
use super::cmd_run::{held, pick};
use super::helpers::val;
use super::registry::{Registry, Sink};
use crate::error::{ApiError, ApiResult};
use crate::schema::*;
use construct_transport::analysis::{
    DiffOpts, EconBase, ExtentOpts, SensOpts, dev_distribution, difference, extent_test, msm_check,
    sensitivity, with_without,
};
use construct_transport::assign::assign;
use construct_transport::econ::{capital, econ};
use construct_transport::edit::net;
use construct_transport::jsnum::{Maybe, json_number};
use construct_transport::project::{Project, Scenario, road_class};
use construct_transport::results::{
    Convergence, Res, Results, export_ready, isochrone, lts, model_meta, model_status, select_link,
    travel,
};
use construct_transport::standards::{pcu_f, year_of};
use serde_json::{Map, Value, json};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub(super) fn register(r: &mut Registry) {
    let sc = || string("Scenario id or name (default: the active scenario).");
    let per = || {
        choice(
            &["AM", "PM"],
            "Peak hour (default the project's current period).",
        )
    };
    let base = || {
        string("The base (do-minimum) scenario to compare against (default the project's base).")
    };
    r.add(
        "transport.analysis.travel",
        "The quickest route between two junctions (or zone centroids as z:<zone id>) on the run's congested times, or free-flow times without a run; with the base scenario's route for comparison.",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("period", per()),
                ("from", string("Junction id, or z:<zone id>.")),
                ("to", string("Junction id, or z:<zone id>.")),
            ],
            &["from", "to"],
        ),
        analysis_travel,
    );
    r.add(
        "transport.analysis.isochrone",
        "Travel times from one junction to every junction it reaches: driving on the model's times, or walking (4.8 km/h) or cycling (15 km/h) on every road except motorways and ramps.",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("period", per()),
                ("from", string("Junction id.")),
                ("mode", choice(&["drive", "walk", "cycle"], "How the trip is made (default drive).")),
            ],
            &["from"],
        ),
        analysis_isochrone,
    );
    r.add(
        "transport.analysis.select_link",
        "Where the traffic on a road comes from and goes to, from the run's stored paths (both directions of the road, as the app does, or one arc by key).",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("period", per()),
                ("link", string("Road id: both directions are combined.")),
                ("key", string("One arc key such as <road id>:ab, instead of link.")),
                ("limit", integer("Most origin-destination pairs to list (default 50).")),
            ],
            &[],
        ),
        analysis_select_link,
    );
    r.add(
        "transport.analysis.export_ready",
        "Whether the run is converged well enough for turn volumes to go to SIDRA, and why not.",
        false,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        |s, p| {
            let (i, period) = pick(s, p)?;
            let sid = s.sid(i);
            Ok(val(&export_ready(s.result(&sid, &period))))
        },
    );
    r.add(
        "transport.analysis.lts",
        "Cycling level of traffic stress by road: 1 (all ages) to 4 (confident riders only), 0 where cycling is not allowed.",
        false,
        params(vec![("scenario", sc()), ("period", per()), ("links", strings("Only these road ids."))], &[]),
        analysis_lts,
    );
    r.add(
        "transport.analysis.difference",
        "Per direction change in flow, V/C and junction delay between the base and an option, with the plot's thresholds.",
        false,
        params(
            vec![
                ("base", base()),
                ("scenario", string("The option (default the active scenario).")),
                ("period", per()),
                ("k", number("Bar width per veh/h in pixels (default 0.02).")),
                ("zoom", number("Map zoom (default 16).")),
            ],
            &[],
        ),
        analysis_difference,
    );
    r.add(
        "transport.analysis.with_without",
        "Per junction, the worst degree of saturation, average delay, level of service and longest queue without and with the development, and the changes.",
        false,
        params(
            vec![("without", string("The scenario without the development (default the project's base).")), ("scenario", string("The scenario with it (default the active scenario).")), ("period", per())],
            &[],
        ),
        analysis_with_without,
    );
    r.add(
        "transport.analysis.extent_test",
        "The extent of assessment: which roads and junctions change enough with the development to be included.",
        false,
        params(
            vec![
                ("without", string("The scenario without the development (default the project's base).")),
                ("scenario", string("The scenario with it (default the active scenario).")),
                ("period", per()),
                ("floor", number("Smallest change counted in veh/h.")),
                ("decade", choice(&["first"], "first: the first-decade Future Connect classes.")),
            ],
            &[],
        ),
        analysis_extent_test,
    );
    r.add(
        "transport.analysis.dev_distribution",
        "Per gateway, the share of the development's trips arriving and leaving through it in each period.",
        false,
        params(vec![("scenario", sc())], &[]),
        analysis_dev_distribution,
    );
    r.add(
        "transport.analysis.capital",
        "Indicative capital cost (NZ$m) of the proposed roads and upgraded junctions of a scenario.",
        false,
        params(vec![("scenario", sc())], &[]),
        |s, p| {
            let i = s.pick_scenario(p, "scenario")?;
            Ok(val(&capital(&s.project.scenarios[i], &s.project.settings)))
        },
    );
    r.add(
        "transport.analysis.econ",
        "Indicative economics of an option against its base: benefits from vehicle hours saved, present value, benefit-cost ratio, first-year rate of return, CO2 and the noise check.",
        false,
        params(vec![("base", base()), ("scenario", string("The option (default the active scenario).")), ("period", per())], &[]),
        analysis_econ,
    );
    r.add(
        "transport.analysis.compare",
        "Compare every scenario in a period: the headline numbers with their change against the base, capital cost and economics.",
        false,
        params(vec![("period", per())], &[]),
        analysis_compare,
    );
    r.add_streaming(
        "transport.analysis.sensitivity",
        "Run the sensitivity pack (Table SENS-1) for a scenario: growth, trip rates, demand and capacity tests, each a full model run. Progress is reported.",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("without", string("The scenario without the development, so each test is also run without it.")),
                ("periods", strings("Periods to run (default the scenario's, otherwise AM and PM).")),
                ("tests", strings("Test ids (default T1+, T1-, T2, T3+, T3-, T4+, T4-, T5; also D+, D-, T6).")),
                ("modeShare", number("T6: percent of vehicle trips replaced by another mode.")),
                ("gapTol", number("Gap target for the runs.")),
                ("maxIter", number("Iteration limit for the runs.")),
                ("withEcon", boolean("Add the discount-rate row from the economics against the base (default false).")),
                ("junctionNames", boolean("Name junctions by their site or the roads that meet there, as the Report does (default true); false names them by id.")),
            ],
            &[],
        ),
        analysis_sensitivity,
    );
    r.add(
        "transport.analysis.msm_check",
        "Compare the growth the base and future runs imply, link by link, with the regional model's growth loaded in the project.",
        false,
        params(
            vec![("base", base()), ("scenario", string("The future scenario (default the active scenario).")), ("period", per()), ("pct", number("Flag when growth differs by more than this percent of the regional rate (default 20)."))],
            &[],
        ),
        analysis_msm_check,
    );
    r.add(
        "transport.model.status",
        "The model's status for a period: Uncalibrated, Calibrated or Validated, with the category, standard and date.",
        false,
        params(vec![("period", per())], &[]),
        |s, p| {
            let period = s.period_of(p)?;
            Ok(val(&model_status(&s.project, &period)))
        },
    );
    r.add(
        "transport.model.meta",
        "The metadata every result carries: model status, convergence, data vintages, base and scenario years, growth basis and junction methods.",
        false,
        params(vec![("scenario", sc()), ("period", per())], &[]),
        model_meta_cmd,
    );
}

// ------------------------------------------------------------------ helpers

fn need_results(s: &Session, i: usize, period: &str) -> ApiResult<()> {
    held(s, i, period).map(|_| ())
}

fn node_exists(sc: &Scenario, id: &str) -> bool {
    match id.strip_prefix("z:") {
        Some(z) => sc.zones.contains_key(z),
        None => sc.nodes.contains_key(id),
    }
}

// ------------------------------------------------------------------ travel and catchments

/// `pickPath` of ui.js: the route, the base scenario's route and whether the times are congested.
fn analysis_travel(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let (from, to) = (p.str("from")?, p.str("to")?);
    let sc = &s.project.scenarios[i];
    for (k, id) in [("from", from), ("to", to)] {
        if !node_exists(sc, id) {
            return Err(ApiError::not_found(format!(
                "`{k}` is not a junction or zone of this scenario: {id}."
            )));
        }
    }
    let sid = sc.id.clone();
    let res = s.result(&sid, &period);
    let r = travel(sc, &s.project.settings, res, from, to);
    let mut out = match r.map(|t| val(&t)) {
        Some(Value::Object(m)) if !m.contains_key("none") => Value::Object(m),
        _ => return Ok(json!({ "none": true, "message": "No route between those two points." })),
    };
    let pr = &s.project;
    let base = pr
        .base
        .as_str()
        .and_then(|id| pr.scenarios.iter().find(|x| x.id == id))
        .or(pr.scenarios.first());
    let mut base_route = Value::Null;
    if let Some(b) = base
        && b.id != sid
        && s.result(&b.id, &period).is_some()
        && node_exists(b, from)
        && node_exists(b, to)
        && let Some(bt) = travel(b, &s.project.settings, s.result(&b.id, &period), from, to)
    {
        let v = val(&bt);
        if v.get("none").is_none() {
            base_route = v;
        }
    }
    out["base"] = base_route;
    out["congested"] = json!(res.is_some());
    Ok(out)
}

/// Minimum-time search by a heap of (time, node), for the walk and cycle catchments.
#[derive(PartialEq)]
struct Item(f64, usize);
impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> Ordering {
        // a smaller time comes out first
        o.0.partial_cmp(&self.0)
            .unwrap_or(Ordering::Equal)
            .then(o.1.cmp(&self.1))
    }
}

/// `Kit.catchment(sc, fromId, mode)`: walking at 4.8 km/h (both ways) or cycling at 15 km/h (one-way streets obeyed) on every road except motorways and ramps.
fn catchment(sc: &Scenario, from: &str, mode: &str) -> Value {
    let (kmh, oneway, label) = if mode == "cycle" {
        (15.0, true, "cycle")
    } else {
        (4.8, false, "walk")
    };
    let v = kmh / 3.6;
    let ids: Vec<&str> = sc.nodes.keys().collect();
    let ix: std::collections::HashMap<&str, usize> =
        ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); ids.len()];
    for l in sc.links.values() {
        let c = road_class(l.cls.as_str());
        let (a, b) = (ix.get(l.a.as_str()), ix.get(l.b.as_str()));
        let (Some(&a), Some(&b)) = (a, b) else {
            continue;
        };
        if c.rank >= 5.0 || l.status.is("closed") {
            continue;
        }
        let t = net::len(sc, l) / v;
        if !oneway || l.lanes_ab.num() > 0.0 {
            adj[a].push((b, t));
        }
        if !oneway || l.lanes_ba.num() > 0.0 {
            adj[b].push((a, t));
        }
    }
    let mut best = vec![f64::INFINITY; ids.len()];
    let mut heap = BinaryHeap::new();
    if let Some(&s0) = ix.get(from) {
        best[s0] = 0.0;
        heap.push(Item(0.0, s0));
    }
    while let Some(Item(t, i)) = heap.pop() {
        if t > best[i] {
            continue;
        }
        for &(j, w) in &adj[i] {
            if t + w < best[j] {
                best[j] = t + w;
                heap.push(Item(t + w, j));
            }
        }
    }
    let mut times = Map::new();
    for (i, t) in best.iter().enumerate() {
        if t.is_finite() {
            times.insert(ids[i].to_string(), json_number(*t));
        }
    }
    json!({
        "from": from, "times": times, "mode": mode,
        "basis": format!(
            "Spare Construct assumption: {label} at {} km/h on every road except motorways and ramps{}; no footpath, crossing or gradient data.",
            construct_transport::jsnum::number_to_js_string(kmh),
            if oneway { ", obeying one-way streets" } else { "" }
        ),
    })
}

/// `isoFrom(nid)` of ui.js.
fn analysis_isochrone(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let from = p.str("from")?;
    let sc = &s.project.scenarios[i];
    if !node_exists(sc, from) {
        return Err(ApiError::not_found(format!(
            "There is no junction {from} in this scenario."
        )));
    }
    let mode = p.opt_str("mode")?.unwrap_or("drive");
    if mode == "walk" || mode == "cycle" {
        return Ok(catchment(sc, from, mode));
    }
    let sid = sc.id.clone();
    let res = s.result(&sid, &period);
    let r = isochrone(sc, &s.project.settings, res, from).ok_or_else(|| {
        ApiError::not_found(format!("There is no junction {from} in this scenario."))
    })?;
    let mut out = val(&r);
    out["mode"] = json!("drive");
    out["basis"] = json!(if res.is_some() {
        "Driving, using congested peak-hour travel times from the model."
    } else {
        "Driving at free-flow speeds; run the model for congested times."
    });
    Ok(out)
}

/// The 'sla' action of the road panel: both directions of a road combined, as the app does.
fn analysis_select_link(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let res = held(s, i, &period)?;
    let sc = &s.project.scenarios[i];
    let limit = p.opt_f64("limit")?.map_or(50, |x| x.max(0.0) as usize);
    let none = || {
        ApiError::invalid_state(
            "Run the model again to enable select-link analysis (this result has no stored paths).",
        )
    };
    let mut a = match (p.opt_str("key")?, p.opt_str("link")?) {
        (Some(k), _) => select_link(Some(res), k).ok_or_else(none)?,
        (None, Some(id)) => {
            let l = sc
                .links
                .get(id)
                .ok_or_else(|| ApiError::not_found(format!("There is no road {id}.")))?;
            let key = if l.lanes_ab.num() > 0.0 {
                format!("{id}:ab")
            } else {
                format!("{id}:ba")
            };
            let mut a = select_link(Some(res), &key).ok_or_else(none)?;
            if l.lanes_ab.num() > 0.0
                && l.lanes_ba.num() > 0.0
                && let Some(b2) = select_link(Some(res), &format!("{id}:ba"))
            {
                for (k, f) in b2.flows.iter() {
                    let cur = a
                        .flows
                        .get(k)
                        .copied()
                        .map_or(0.0, |x| if x.is_nan() { 0.0 } else { x });
                    a.flows.insert(k, cur + f);
                }
                a.ods.extend(b2.ods);
                a.tot += b2.tot;
                construct_transport::results::js_sort_by(&mut a.ods, |x, y| y.q - x.q);
            }
            a
        }
        (None, None) => {
            return Err(ApiError::bad_params(
                "Give a `link` (road id) or a `key` (arc key).",
            ));
        }
    };
    let pf = pcu_f(&s.project.settings);
    let total_ods = a.ods.len();
    a.ods.truncate(limit);
    let name = |id: &str| {
        sc.zones
            .get(id)
            .and_then(|z| z.name.as_str())
            .unwrap_or(id)
            .to_string()
    };
    let ods: Vec<Value> = a.ods.iter().map(|o| json!({ "from": o.o, "fromName": name(&o.o), "to": o.d, "toName": name(&o.d), "q": val(&o.q), "vehPerHour": val(&(o.q / pf)) })).collect();
    Ok(json!({
        "key": a.key, "totalPcu": val(&a.tot), "vehPerHour": val(&(a.tot / pf)), "odPairs": total_ods, "ods": ods, "flows": val(&a.flows),
        "message": format!("{} veh/h use this road.", construct_transport::util::fmt::int(a.tot / pf)),
    }))
}

fn analysis_lts(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let sid = s.sid(i);
    let res = s.result(&sid, &period);
    let only: Option<std::collections::HashSet<String>> =
        p.opt_strings("links")?.map(|v| v.into_iter().collect());
    let sc = &s.project.scenarios[i];
    let mut m = Map::new();
    for l in sc
        .links
        .values()
        .filter(|l| only.as_ref().is_none_or(|o| o.contains(&l.id)))
    {
        m.insert(l.id.clone(), json!(lts(l, res, &s.project.settings)));
    }
    Ok(json!({ "scenario": sid, "period": period, "withResults": res.is_some(), "lts": m }))
}

// ------------------------------------------------------------------ with and without

fn pair(s: &Session, p: &Params, base_key: &str) -> ApiResult<(usize, usize, String)> {
    let b = match p.opt_str(base_key)? {
        Some(k) => s.scenario_index(k)?,
        None => {
            let pr = &s.project;
            pr.base
                .as_str()
                .and_then(|id| pr.scenarios.iter().position(|x| x.id == id))
                .unwrap_or(0)
        }
    };
    let o = s.pick_scenario(p, "scenario")?;
    Ok((b, o, s.period_of(p)?))
}

fn both(s: &Session, b: usize, o: usize, period: &str) -> ApiResult<()> {
    need_results(s, b, period)?;
    need_results(s, o, period)
}

fn analysis_difference(s: &mut Session, p: &Params) -> ApiResult {
    let (b, o, period) = pair(s, p, "base")?;
    both(s, b, o, &period)?;
    let (bid, oid) = (s.sid(b), s.sid(o));
    let opts = DiffOpts {
        k: p.opt_f64("k")?,
        zoom: p.opt_f64("zoom")?,
    };
    let d = difference(s.result(&bid, &period), s.result(&oid, &period), opts)
        .ok_or_else(|| ApiError::invalid_state("Run both scenarios first."))?;
    Ok(json!({ "base": bid, "scenario": oid, "period": period, "difference": val(&d) }))
}

fn analysis_with_without(s: &mut Session, p: &Params) -> ApiResult {
    let (b, o, period) = pair(s, p, "without")?;
    both(s, b, o, &period)?;
    let (bid, oid) = (s.sid(b), s.sid(o));
    let w = with_without(s.result(&bid, &period), s.result(&oid, &period), None)
        .ok_or_else(|| ApiError::invalid_state("Run both scenarios first."))?;
    Ok(json!({ "without": bid, "scenario": oid, "period": period, "withWithout": val(&w) }))
}

fn analysis_extent_test(s: &mut Session, p: &Params) -> ApiResult {
    let (b, o, period) = pair(s, p, "without")?;
    both(s, b, o, &period)?;
    let (bid, oid) = (s.sid(b), s.sid(o));
    let opts = ExtentOpts {
        floor: p.opt_f64("floor")?,
        decade: p.opt_str("decade")?,
        sc_without: Some(&s.project.scenarios[b]),
    };
    let t = extent_test(
        &s.project.scenarios[o],
        s.result(&bid, &period),
        s.result(&oid, &period),
        opts,
    )
    .ok_or_else(|| ApiError::invalid_state("Run both scenarios first."))?;
    Ok(json!({ "without": bid, "scenario": oid, "period": period, "extentTest": val(&t) }))
}

fn analysis_dev_distribution(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let sid = s.sid(i);
    let list: Vec<&Res> = ["AM", "PM"]
        .iter()
        .filter_map(|per| s.result(&sid, per))
        .collect();
    if list.is_empty() {
        return Err(ApiError::invalid_state(
            "Run the model first (transport.run).",
        ));
    }
    Ok(
        json!({ "scenario": sid, "distribution": val(&dev_distribution(Some(&s.project.scenarios[i]), &list)) }),
    )
}

fn analysis_econ(s: &mut Session, p: &Params) -> ApiResult {
    let (b, o, period) = pair(s, p, "base")?;
    let (bid, oid) = (s.sid(b), s.sid(o));
    let empty = Results::new();
    let rb = s.results.get(&bid).unwrap_or(&empty);
    let ro = s.results.get(&oid).unwrap_or(&empty);
    let e = econ(
        &s.project.settings,
        &s.project.scenarios[b],
        rb,
        &s.project.scenarios[o],
        ro,
        &period,
    )
    .ok_or_else(|| {
        ApiError::invalid_state(format!(
            "Run both scenarios in the {period} peak first (transport.run)."
        ))
    })?;
    Ok(json!({ "base": bid, "scenario": oid, "period": period, "econ": val(&e) }))
}

const KPI_ROWS: [(&str, &str, bool); 7] = [
    ("trips", "Vehicle trips (veh/h)", false),
    ("vkt", "Vehicle km (veh.km/h)", true),
    ("vht", "Vehicle hours (veh.h/h)", true),
    ("spd", "Average speed (km/h)", false),
    ("dly", "Delay (veh.h/h)", true),
    ("co2", "CO2 (t/h)", true),
    ("over", "Roads over capacity", true),
];

/// The Compare scenarios tab: a column per scenario with the headline numbers, their change against the base, capital and economics.
fn analysis_compare(s: &mut Session, p: &Params) -> ApiResult {
    let period = s.period_of(p)?;
    let pr = &s.project;
    let base_i = pr
        .base
        .as_str()
        .and_then(|id| pr.scenarios.iter().position(|x| x.id == id))
        .unwrap_or(0);
    let base_id = pr.scenarios[base_i].id.clone();
    let rb = s.result(&base_id, &period);
    let empty = Results::new();
    let mut cols: Vec<Value> = Vec::new();
    for (i, sc) in pr.scenarios.iter().enumerate() {
        let r = s.result(&sc.id, &period);
        let mut kpi = Map::new();
        if let Some(r) = r {
            let k = val(&r.kpi);
            for (key, label, lower) in KPI_ROWS {
                let v = k.get(key).and_then(Value::as_f64);
                let b = rb
                    .filter(|_| i != base_i)
                    .and_then(|b| val(&b.kpi).get(key).and_then(Value::as_f64));
                let dv = match (v, b) {
                    (Some(v), Some(b)) => Some(v - b),
                    _ => None,
                };
                let dir = match dv {
                    Some(d) if d.abs() >= 1e-6 => {
                        Some(if (d < 0.0) == lower { "down" } else { "up" })
                    }
                    _ => None,
                };
                kpi.insert(
                    key.to_string(),
                    json!({ "label": label, "value": v, "change": dv, "direction": dir }),
                );
            }
        }
        let e = if i == base_i {
            Value::Null
        } else {
            let ro = s.results.get(&sc.id).unwrap_or(&empty);
            let rbase = s.results.get(&base_id).unwrap_or(&empty);
            econ(&pr.settings, &pr.scenarios[base_i], rbase, sc, ro, &period)
                .map_or(Value::Null, |x| val(&x))
        };
        cols.push(json!({
            "scenario": sc.id, "name": sc.name.as_str(), "base": i == base_i,
            "year": r.map_or_else(|| year_of(Some(sc), &pr.settings), |r| r.year), "run": r.is_some(), "stale": r.is_some_and(|r| r.stale),
            "kpi": kpi, "capital": val(&capital(sc, &pr.settings)), "econ": e,
        }));
    }
    Ok(json!({ "period": period, "base": base_id, "scenarios": cols }))
}

/// The sensitivity pack the way the Report tab runs it: no discount-rate row unless asked for.
fn analysis_sensitivity(s: &mut Session, p: &Params, sink: Sink<'_>) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let wo_i = match p.opt_str("without")? {
        Some(k) => Some(s.scenario_index(k)?),
        None => None,
    };
    let with_econ = p.flag("withEcon")?;
    let names = p.opt_bool("junctionNames")?.unwrap_or(true);
    let (sid, sc_name) = (
        s.sid(i),
        s.project.scenarios[i]
            .name
            .as_str()
            .unwrap_or("")
            .to_string(),
    );
    let base_i = {
        let pr = &s.project;
        pr.base
            .as_str()
            .and_then(|id| pr.scenarios.iter().position(|x| x.id == id))
            .unwrap_or(0)
    };
    let empty = Results::new();
    let Session {
        project, results, ..
    } = &mut *s;
    let project: &Project = project;
    let sc = &project.scenarios[i];
    let name_of = |id: &str| -> String {
        let r = results.get(&sc.id).and_then(|m| m.values().next());
        match r {
            Some(r) => match r
                .nodes
                .get(id)
                .and_then(|n| n.site.as_deref())
                .and_then(|k| r.sites.as_ref().and_then(|m| m.get(k)))
            {
                Some(site) if !site.name.is_empty() => site.name.clone(),
                _ => construct_transport::turns::junction_name(sc, id, None),
            },
            None => construct_transport::turns::junction_name(sc, id, None),
        }
    };
    let mut on_progress = |k: usize, n: usize, label: &str| {
        sink(&json!({ "event": "progress", "run": k, "runs": n, "message": label }));
    };
    let econ_base = if with_econ {
        Some(EconBase {
            sc: &project.scenarios[base_i],
            res: results.get(&project.scenarios[base_i].id).unwrap_or(&empty),
            opt_res: results.get(&sid).unwrap_or(&empty),
        })
    } else {
        None
    };
    let opts = SensOpts {
        periods: p.opt_strings("periods")?,
        tests: p.opt_strings("tests")?,
        without: wo_i.map(|w| &project.scenarios[w]),
        econ_base,
        mode_share: match p.opt_f64("modeShare")? {
            Some(x) => construct_transport::jsnum::Num::number(x),
            None => Maybe::Missing,
        },
        name: if names { Some(&name_of) } else { None },
        gap_tol: p
            .opt_f64("gapTol")?
            .map_or(Maybe::Missing, construct_transport::jsnum::Num::number),
        max_iter: p
            .opt_f64("maxIter")?
            .map_or(Maybe::Missing, construct_transport::jsnum::Num::number),
        on_progress: Some(&mut on_progress),
        now: None,
    };
    let table = sensitivity(project, sc, opts, |s1, st, ao| {
        assign(project, s1, st, ao, None)
    })
    .map_err(|e| ApiError::bad_params(e.error))?;
    Ok(json!({ "scenario": sid, "name": sc_name, "sensitivity": val(&table) }))
}

fn analysis_msm_check(s: &mut Session, p: &Params) -> ApiResult {
    let (b, o, period) = pair(s, p, "base")?;
    let (bid, oid) = (s.sid(b), s.sid(o));
    let msm = s.project.data.get().and_then(|d| d.msm.get());
    let m = msm_check(
        s.result(&bid, &period),
        s.result(&oid, &period),
        msm,
        p.opt_f64("pct")?,
    );
    Ok(json!({ "base": bid, "scenario": oid, "period": period, "msmCheck": val(&m) }))
}

fn model_meta_cmd(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let sid = s.sid(i);
    let c = match s.result(&sid, &period) {
        Some(r) => Convergence {
            converged: r.converged,
            hit_max: r.hit_max,
            gap: r.gap,
            gap_tol: r.gap_tol,
            it: r.it,
            note: r.conv_note.clone(),
        },
        None => Convergence {
            converged: false,
            hit_max: false,
            gap: f64::NAN,
            gap_tol: f64::NAN,
            it: 0.0,
            note: String::new(),
        },
    };
    let m = model_meta(
        &s.project,
        Some(&s.project.scenarios[i]),
        &period,
        &s.project.settings,
        c,
    );
    Ok(json!({ "scenario": sid, "period": period, "meta": val(&m) }))
}
