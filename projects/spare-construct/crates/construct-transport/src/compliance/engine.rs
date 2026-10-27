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

/// A typed value out of loose JSON (panics when the JSON is not what the type reads, which a project the prototype saved always is).
pub fn typed<T: DeserializeOwned>(v: &V) -> T {
    serde_json::from_str(&v.stringify()).unwrap_or_else(|e| panic!("compliance: cannot read {} as {}: {e}", short(v), std::any::type_name::<T>()))
}

fn short(v: &V) -> String {
    let t = v.stringify();
    if t.chars().count() > 80 { t.chars().take(80).collect::<String>() + "..." } else { t }
}

/// Loose JSON out of a typed value (numbers that are not finite come out as the text "NaN", "Infinity" or "-Infinity", as results hold them).
pub fn loose<T: Serialize>(t: &T) -> V {
    V::parse(&serde_json::to_string(t).expect("a result serialises")).expect("serialised JSON parses")
}

/// The typed project (`Store.P`).
pub fn project(p: &V) -> Project {
    typed(p)
}

/// A typed scenario out of a scenario value.
pub fn scenario(sc: &V) -> Scenario {
    typed(sc)
}

/// The scenario's results by period, typed.
pub fn results_of(sc: &V) -> Results {
    if sc.g("res").is_obj() { typed(sc.g("res")) } else { Results::new() }
}

/// One result value, typed.
pub fn res_typed(r: &V) -> Res {
    typed(r)
}

/// `Engine.assign(sc, opts)`: the equilibrium assignment of a scenario for a period, as loose JSON.
///
/// `p` is the open project, `settings` the settings to run with (None: the project's), `opts` the engine's options:
/// `{ period, gapTol, maxIter, demandFactor, select, arcPenalty }`.
pub fn assign(p: &V, sc: &V, settings: Option<&V>, opts: &V) -> V {
    let proj = project(p);
    let s: Settings = settings.map_or_else(|| proj.settings.clone(), typed);
    let o: AssignOpts = typed(opts);
    loose(&crate::assign::assign(&proj, &scenario(sc), &s, &o, None))
}

/// `Engine.modelStatus(P, period)`.
pub fn model_status(p: &V, period: &str) -> V {
    loose(&results::model_status(&project(p), period))
}

/// `Engine.yearOf(sc, S)` with the project's settings.
pub fn year_of(p: &V, sc: &V) -> f64 {
    standards::year_of(Some(&scenario(sc)), &project(p).settings)
}

/// `Engine.stageOf(z, sc)`: the share of a zone built out in the scenario.
pub fn stage_of(z: &V, sc: &V) -> f64 {
    let zone: Zone = typed(z);
    demand::stage_of(Some(&zone), Some(&scenario(sc)))
}

/// `Engine.zoneClass(z)`: "development", "committed" or "existing".
pub fn zone_class(z: &V) -> String {
    let zone: Zone = typed(z);
    graph::zone_class(Some(&zone)).to_string()
}

/// `Engine.tripEnds(z, S, period, sc)`: the zone's trips (`O` leaving, `D` arriving).
pub fn trip_ends(p: &V, z: &V, period: &str, sc: &V) -> (f64, f64) {
    let proj = project(p);
    let zone: Zone = typed(z);
    let e = demand::trip_ends(&proj, &zone, &proj.settings, period, &scenario(sc));
    (e.o, e.d)
}

/// `Engine.extentTest(sc, without, with, { scWithout })`: the extent of effects of the with result against the without one, or null.
pub fn extent_test(sc: &V, without: &V, with: &V, sc_without: Option<&V>) -> V {
    let s = scenario(sc);
    let sw = sc_without.map(scenario);
    let (a, b): (Option<Res>, Option<Res>) = (without.t().then(|| res_typed(without)), with.t().then(|| res_typed(with)));
    let opts = crate::analysis::ExtentOpts { floor: None, decade: None, sc_without: sw.as_ref() };
    crate::analysis::extent_test(&s, a.as_ref(), b.as_ref(), opts).map_or(V::Null, |x| loose(&x))
}

/// `Engine.econ(base, opt, period)`: the indicative economics of an option against a base scenario, or null without results for both.
pub fn econ(p: &V, base: &V, opt: &V, period: &str) -> V {
    let proj = project(p);
    let (bs, os) = (scenario(base), scenario(opt));
    econ::econ(&proj.settings, &bs, &results_of(base), &os, &results_of(opt), period).map_or(V::Null, |e| loose(&e))
}

/// `Engine.capital(sc)`: capital cost of the proposed roads and upgraded junctions (NZ$m), as the engine reports it.
pub fn capital(p: &V, sc: &V) -> V {
    loose(&econ::capital(&scenario(sc), &project(p).settings))
}

/// `Engine.pwf(rates, n)`: present worth factors of a discount schedule.
pub fn pwf(rates: &V, n: f64) -> V {
    loose(&econ::pwf(&rates.to_value(), n))
}

/// `Engine.ef(v, S)`: the CO2e emission factor at an average speed (g per veh-km).
pub fn ef(p: &V, v: f64) -> f64 {
    standards::ef(v, &project(p).settings)
}

/// `Engine.STANDARDS` (the standards table as loose JSON).
pub fn standards_table() -> V {
    V::from_value(&standards::STANDARDS.to_value())
}

/// `Engine.buildGraph(sc, S)`: the routing graph of a scenario (for searches).
pub fn build_graph(p: &V, sc: &V, period: Option<&str>) -> Graph {
    graph::build_graph(&scenario(sc), &project(p).settings, period)
}

/// `Engine.arcTime(a, x, S)`: the travel time on an arc of a graph carrying x pcu/h.
pub fn arc_time(p: &V, a: &graph::Arc, x: f64) -> f64 {
    let perf = crate::junction::PerfSet::new(&project(p).settings);
    crate::junction::arc_time(a, x, &perf)
}

/// `Scenarios.check(sc, P)`: the checks on a scenario (blocks, warnings), as an array of `{ level, code, id, text }`.
pub fn scenario_check(p: &V, sc: &V) -> V {
    loose(&scenarios::check(&scenario(sc), &project(p)))
}

/// `Scenarios.describe(sc, P)`: what a scenario contains, in words.
pub fn scenario_describe(p: &V, sc: &V) -> V {
    loose(&scenarios::describe(&scenario(sc), &project(p)))
}

/// `Scenarios.copy(sc, { links })`: a copy of a scenario without the links `drop` names (junctions left unused are pruned; no results).
pub fn scenario_copy_without_links(sc: &V, drop: &std::collections::HashSet<String>) -> V {
    struct Ids(u32);
    impl crate::edit::IdSource for Ids {
        fn uid(&mut self, prefix: &str) -> String {
            self.0 += 1;
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
pub fn zone_role(z: &V) -> String {
    let zone: Zone = typed(z);
    scenarios::zone_role(&zone).to_string()
}

/// `Scenarios.linkStage(L)`.
pub fn link_stage(l: &V) -> String {
    let link: Link = typed(l);
    crate::project::link_stage(&link).to_string()
}

/// `Scenarios.unfunded(L)`.
pub fn link_unfunded(l: &V) -> bool {
    let link: Link = typed(l);
    crate::project::link_unfunded(&link)
}

/// `Register.table(P)`: the committed-development register with each row's inclusion and reason.
pub fn register_table(p: &V) -> V {
    V::Array(register::table(&project(p)).iter().map(V::from_value).collect())
}

/// `Register.inBaseline(row)`: `{ included, reason }`.
pub fn in_baseline(p: &V, row: &V) -> V {
    let proj = project(p);
    let found = row.g("id").as_str().and_then(|id| proj.register_row(id));
    let typed_row;
    let r = match found {
        Some(r) => Some(r),
        None => {
            typed_row = typed::<crate::project::RegisterRow>(row);
            Some(&typed_row)
        }
    };
    let (included, reason) = crate::project::in_baseline(r);
    super::jv::jobj! { "included" => included, "reason" => reason }
}

/// `Calibrate.report(sc, period)`: the calibration report of a scenario in a period (for the MBCM checks worksheet).
pub fn calibrate_report(p: &V, sc: &V, period: &str) -> V {
    let proj = project(p);
    let s = scenario(sc);
    let res = sc.g("res").g(period);
    let r = res.t().then(|| res_typed(res));
    loose(&calibrate::report(&proj, &s, period, r.as_ref()))
}

/// `Turns.sidraExport(sc, res, null, {})`: the SIDRA volume export of a scenario for the results given; `{ ok, why, csv }`.
pub fn sidra_export(p: &V, sc: &V, res: &[&V]) -> V {
    let proj = project(p);
    let s = scenario(sc);
    let typed_res: Vec<Res> = res.iter().map(|r| res_typed(r)).collect();
    let refs: Vec<&Res> = typed_res.iter().collect();
    let e = turns::sidra_export(Some(&s), &refs, &proj.settings, None, &turns::SidraOpts::default());
    super::jv::jobj! { "ok" => e.ok, "why" => e.why, "csv" => e.csv }
}

/// `DataHub.safetySummary(sc)`: the crash analysis of a scenario, or None (the crash analysis lives in the safety module).
pub fn safety_summary(_p: &V, _sc: &V) -> Option<V> {
    None
}

/// `DataHub.linkRisk(sc)`: crash risk by link id, or None (the crash analysis lives in the safety module).
pub fn link_risk(_p: &V, _sc: &V) -> Option<V> {
    None
}

/// `DataHub.norm(s)`: a road name normalised for matching.
pub fn norm(s: &str) -> String {
    crate::datahub::norm(s)
}
