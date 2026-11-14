//! The calculation engine as the compliance checks call it. The checks hold a project as loose JSON ([`V`]); these functions
//! read the typed project the engine works on out of that JSON, call the engine (assignment, model status, extent test,
//! economics, trip ends, graph searches, scenario checks, the register) and give the answer back as loose JSON in the
//! prototype's shape.
//!
//! Crash analysis (`DataHub.safetySummary`, `DataHub.linkRisk`) and the climate hazard feed (`Feeds`) are not part of this
//! module: [`safety_summary`] and [`link_risk`] answer "not loaded" until the safety module supplies them, and the checks that
//! need them read the same fields from the project's data when it holds them.

use super::jv::{Jx, V};
use crate::calibrate;
use crate::demand;
use crate::econ;
use crate::edit::{register, scenarios};
use crate::graph::{self, Graph};
use crate::project::{Link, Project, Scenario, Settings, Zone};
use crate::results::{self, AssignOpts, Res, Results};
use crate::standards;
use crate::turns;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// A typed value out of loose JSON, None when the JSON is not what the type reads (a project the prototype saved always is).
#[must_use]
pub fn typed<T: DeserializeOwned>(v: &V) -> Option<T> {
    serde_json::from_str(&v.stringify()).ok()
}

/// Loose JSON out of a typed value (numbers that are not finite come out as the text "NaN", "Infinity" or "-Infinity", as results hold them).
pub fn loose<T: Serialize>(t: &T) -> V {
    // The engine's types serialise to plain JSON (text keys only), which always parses back; null is only the total fallback
    serde_json::to_string(t).ok().and_then(|text| V::parse(&text).ok()).unwrap_or(V::Null)
}

/// The typed project (`Store.P`); a new empty project when the JSON is not a project (which a saved project always is).
#[must_use]
pub fn project(p: &V) -> Project {
    typed(p).unwrap_or_else(|| crate::project::new_project("", "", ""))
}

/// A typed scenario out of a scenario value; a new empty scenario when the JSON is not one (which a saved scenario always is).
#[must_use]
pub fn scenario(sc: &V) -> Scenario {
    typed(sc).unwrap_or_else(|| crate::project::new_scenario("", ""))
}

/// The scenario's results by period, typed.
#[must_use]
pub fn results_of(sc: &V) -> Results {
    if sc.g("res").is_obj() { typed(sc.g("res")).unwrap_or_else(Results::new) } else { Results::new() }
}

/// One result value, typed; None when the JSON is not a result (which a saved result always is).
#[must_use]
pub fn res_typed(r: &V) -> Option<Res> {
    typed(r)
}

/// `Engine.assign(sc, opts)`: the equilibrium assignment of a scenario for a period, as loose JSON.
///
/// `p` is the open project, `settings` the settings to run with (None: the project's), `opts` the engine's options:
/// `{ period, gapTol, maxIter, demandFactor, select, arcPenalty }`.
pub fn assign(p: &V, sc: &V, settings: Option<&V>, opts: &V) -> V {
    let proj = project(p);
    let s: Settings = settings.and_then(typed).unwrap_or_else(|| proj.settings.clone());
    let o: AssignOpts = typed(opts).unwrap_or_default();
    loose(&crate::assign::assign(&proj, &scenario(sc), &s, &o, None))
}

/// `Engine.modelStatus(P, period)`.
#[must_use]
pub fn model_status(p: &V, period: &str) -> V {
    loose(&results::model_status(&project(p), period))
}

/// `Engine.yearOf(sc, S)` with the project's settings.
#[must_use]
pub fn year_of(p: &V, sc: &V) -> f64 {
    standards::year_of(Some(&scenario(sc)), &project(p).settings)
}

/// `Engine.stageOf(z, sc)`: the share of a zone built out in the scenario.
#[must_use]
pub fn stage_of(z: &V, sc: &V) -> f64 {
    let zone: Option<Zone> = typed(z);
    demand::stage_of(zone.as_ref(), Some(&scenario(sc)))
}

/// `Engine.zoneClass(z)`: "development", "committed" or "existing".
#[must_use]
pub fn zone_class(z: &V) -> String {
    let zone: Option<Zone> = typed(z);
    graph::zone_class(zone.as_ref()).to_string()
}

/// `Engine.tripEnds(z, S, period, sc)`: the zone's trips (`O` leaving, `D` arriving).
#[must_use]
pub fn trip_ends(p: &V, z: &V, period: &str, sc: &V) -> (f64, f64) {
    let proj = project(p);
    let Some(zone) = typed::<Zone>(z) else {
        // A saved zone always reads; an unreadable one has no trips to give
        return (f64::NAN, f64::NAN);
    };
    let e = demand::trip_ends(&proj, &zone, &proj.settings, period, &scenario(sc));
    (e.o, e.d)
}

/// `Engine.extentTest(sc, without, with, { scWithout })`: the extent of effects of the with result against the without one, or null.
pub fn extent_test(sc: &V, without: &V, with: &V, sc_without: Option<&V>) -> V {
    let s = scenario(sc);
    let sw = sc_without.map(scenario);
    let (a, b): (Option<Res>, Option<Res>) = (without.t().then(|| res_typed(without)).flatten(), with.t().then(|| res_typed(with)).flatten());
    let opts = crate::analysis::ExtentOpts { floor: None, decade: None, sc_without: sw.as_ref() };
    crate::analysis::extent_test(&s, a.as_ref(), b.as_ref(), opts).map_or(V::Null, |x| loose(&x))
}

/// `Engine.econ(base, opt, period)`: the indicative economics of an option against a base scenario, or null without results for both.
#[must_use]
pub fn econ(p: &V, base: &V, opt: &V, period: &str) -> V {
    let proj = project(p);
    let (bs, os) = (scenario(base), scenario(opt));
    econ::econ(&proj.settings, &bs, &results_of(base), &os, &results_of(opt), period).map_or(V::Null, |e| loose(&e))
}

/// `Engine.capital(sc)`: capital cost of the proposed roads and upgraded junctions (NZ$m), as the engine reports it.
#[must_use]
pub fn capital(p: &V, sc: &V) -> V {
    loose(&econ::capital(&scenario(sc), &project(p).settings))
}

/// `Engine.pwf(rates, n)`: present worth factors of a discount schedule.
#[must_use]
pub fn pwf(rates: &V, n: f64) -> V {
    loose(&econ::pwf(&rates.to_value(), n))
}

/// `Engine.ef(v, S)`: the `CO2e` emission factor at an average speed (g per veh-km).
#[must_use]
pub fn ef(p: &V, v: f64) -> f64 {
    standards::ef(v, &project(p).settings)
}

/// `Engine.STANDARDS` (the standards table as loose JSON).
#[must_use]
pub fn standards_table() -> V {
    V::from_value(&standards::STANDARDS.to_value())
}

/// `Engine.buildGraph(sc, S)`: the routing graph of a scenario (for searches).
#[must_use]
pub fn build_graph(p: &V, sc: &V, period: Option<&str>) -> Graph {
    graph::build_graph(&scenario(sc), &project(p).settings, period)
}

/// `Engine.arcTime(a, x, S)`: the travel time on an arc of a graph carrying x pcu/h.
#[must_use]
pub fn arc_time(p: &V, a: &graph::Arc, x: f64) -> f64 {
    let perf = crate::junction::PerfSet::new(&project(p).settings);
    crate::junction::arc_time(a, x, &perf)
}

/// `Scenarios.check(sc, P)`: the checks on a scenario (blocks, warnings), as an array of `{ level, code, id, text }`.
#[must_use]
pub fn scenario_check(p: &V, sc: &V) -> V {
    loose(&scenarios::check(&scenario(sc), &project(p)))
}

/// `Scenarios.describe(sc, P)`: what a scenario contains, in words.
#[must_use]
pub fn scenario_describe(p: &V, sc: &V) -> V {
    loose(&scenarios::describe(&scenario(sc), &project(p)))
}

/// `Scenarios.copy(sc, { links })`: a copy of a scenario without the links `drop` names (junctions left unused are pruned; no results).
#[must_use]
pub fn scenario_copy_without_links<H: std::hash::BuildHasher>(sc: &V, drop: &std::collections::HashSet<String, H>) -> V {
    struct Ids(u32);
    impl crate::edit::IdSource for Ids {
        fn uid(&mut self, prefix: &str) -> String {
            // Bound: one id per copied item, far below u32::MAX
            self.0 = self.0.saturating_add(1);
            format!("{prefix}{}", self.0)
        }
        fn now_ms(&self) -> f64 {
            crate::compliance::dates::now_ms()
        }
    }
    let src = scenario(sc);
    let keep = |l: &Link| !drop.contains(&l.id);
    let f = scenarios::CopyFilter { links: Some(&keep), zones: None };
    loose(&scenarios::copy(&src, &mut Ids(0), &f))
}

/// `Scenarios.zoneRole(z)`: "development", "committed" or "existing".
#[must_use]
pub fn zone_role(z: &V) -> String {
    // A saved zone always reads; the loose reading is the same rule for one that does not
    typed::<Zone>(z).map_or_else(|| super::net::zone_role(z).to_string(), |zone| scenarios::zone_role(&zone).to_string())
}

/// `Scenarios.linkStage(L)`.
#[must_use]
pub fn link_stage(l: &V) -> String {
    // A saved link always reads; the loose reading is the same rule for one that does not
    typed::<Link>(l).map_or_else(|| super::net::link_stage(l).to_string(), |link| crate::project::link_stage(&link).to_string())
}

/// `Scenarios.unfunded(L)`.
#[must_use]
pub fn link_unfunded(l: &V) -> bool {
    typed::<Link>(l).map_or_else(|| super::net::link_unfunded(l), |link| crate::project::link_unfunded(&link))
}

/// `Register.table(P)`: the committed-development register with each row's inclusion and reason.
pub fn register_table(p: &V) -> V {
    V::Array(register::table(&project(p)).iter().map(V::from_value).collect())
}

/// `Register.inBaseline(row)`: `{ included, reason }`.
#[must_use]
pub fn in_baseline(p: &V, row: &V) -> V {
    let proj = project(p);
    let found = row.g("id").as_str().and_then(|id| proj.register_row(id));
    let typed_row: Option<crate::project::RegisterRow>;
    let r = if let Some(r) = found { Some(r) } else {
        typed_row = typed(row);
        typed_row.as_ref()
    };
    let (included, reason) = crate::project::in_baseline(r);
    super::jv::jobj! { "included" => included, "reason" => reason }
}

/// `Calibrate.report(sc, period)`: the calibration report of a scenario in a period (for the MBCM checks worksheet).
#[must_use]
pub fn calibrate_report(p: &V, sc: &V, period: &str) -> V {
    let proj = project(p);
    let s = scenario(sc);
    let res = sc.g("res").g(period);
    let r = res.t().then(|| res_typed(res)).flatten();
    loose(&calibrate::report(&proj, &s, period, r.as_ref()))
}

/// `Turns.sidraExport(sc, res, null, {})`: the SIDRA volume export of a scenario for the results given; `{ ok, why, csv }`.
#[must_use]
pub fn sidra_export(p: &V, sc: &V, res: &[&V]) -> V {
    let proj = project(p);
    let s = scenario(sc);
    let typed_res: Vec<Res> = res.iter().filter_map(|r| res_typed(r)).collect();
    let borrowed: Vec<&Res> = typed_res.iter().collect();
    let e = turns::sidra_export(Some(&s), &borrowed, &proj.settings, None, &turns::SidraOpts::default());
    super::jv::jobj! { "ok" => e.ok, "why" => e.why, "csv" => e.csv }
}

/// `DataHub.safetySummary(sc)`: the crash analysis of a scenario, or None (the crash analysis lives in the safety module).
#[must_use]
pub fn safety_summary(_p: &V, _sc: &V) -> Option<V> {
    None
}

/// `DataHub.linkRisk(sc)`: crash risk by link id, or None (the crash analysis lives in the safety module).
#[must_use]
pub fn link_risk(_p: &V, _sc: &V) -> Option<V> {
    None
}

/// `DataHub.norm(s)`: a road name normalised for matching.
#[must_use]
pub fn norm(s: &str) -> String {
    crate::datahub::norm(s)
}
