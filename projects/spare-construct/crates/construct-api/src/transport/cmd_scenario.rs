//! Scenario commands: list, select, copy, create, assessment set, rename, delete, base, year, describe and check.

use super::Session;
use super::cmd_network::{FieldKind, apply_fields};
use super::helpers::val;
use super::registry::Registry;
use crate::error::{ApiError, ApiResult};
use crate::schema::*;
use construct_transport::edit::scenarios::{self, CreateSetOpts};
use construct_transport::edit::{self};
use construct_transport::jsnum::{Maybe, math_round};
use construct_transport::standards::year_of;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

pub(super) fn register(r: &mut Registry) {
    let sc = || string("Scenario id or name (default: the active scenario).");
    r.add(
        "transport.scenario.list",
        "List the scenarios with their role, year, size and which results they hold.",
        false,
        none(),
        |s, _| {
            let list: Vec<Value> = (0..s.project.scenarios.len()).map(|i| s.scenario_brief(i)).collect();
            Ok(json!({ "active": s.project.active.as_str(), "base": s.project.base.as_str(), "scenarios": list }))
        },
    );
    r.add(
        "transport.scenario.select",
        "Make a scenario the active one, as picking it in the top bar does.",
        true,
        params(
            vec![("scenario", string("Scenario id or name."))],
            &["scenario"],
        ),
        scenario_select,
    );
    r.add(
        "transport.scenario.copy",
        "Duplicate a scenario (network, zones, routes and manual matrices, no results) as Scenario N and make the copy the active one.",
        true,
        params(vec![("scenario", sc()), ("name", string("Name for the copy (default Scenario N)."))], &[]),
        scenario_copy,
    );
    r.add(
        "transport.scenario.add",
        "Add a new empty scenario and make it the active one.",
        true,
        params(vec![("name", string("Name (default Scenario N)."))], &[]),
        scenario_add,
    );
    r.add(
        "transport.scenario.create_set",
        "Create the assessment set: existing (count year), then for the opening and design years the future baseline, with the proposal and with mitigation.",
        true,
        params(
            vec![
                ("pathway", choice(&["consent", "planchange", "nor", "ftaa"], "Consenting pathway (default consent).")),
                ("countYear", number("Year of the traffic counts (default the base year).")),
                ("opening", number("Opening year (default the count year).")),
                ("design", number("Design year (default the pathway's years after opening).")),
                ("from", string("Id of the scenario to copy from (default the active one).")),
                ("mitigation", boolean("False leaves out the mitigation scenarios (default true).")),
                ("periods", strings("Periods to assess (default AM and PM).")),
                ("buildout", free_object("Percent of the development zones built, by year: { \"2036\": 60 }.")),
            ],
            &[],
        ),
        scenario_create_set,
    );
    r.add(
        "transport.scenario.sensitivity_copy",
        "Add a sensitivity copy of a scenario, with the unfunded projects left out or, with unfunded, put back in.",
        true,
        params(vec![("scenario", sc()), ("unfunded", boolean("Put the unfunded projects back in."))], &[]),
        scenario_sensitivity_copy,
    );
    r.add(
        "transport.scenario.rename",
        "Rename a scenario.",
        true,
        params(
            vec![("scenario", sc()), ("name", string("The new name."))],
            &["name"],
        ),
        scenario_rename,
    );
    r.add(
        "transport.scenario.delete",
        "Delete a scenario with its roads, zones and results (Undo cannot bring it back). The project keeps at least one.",
        true,
        params(vec![("scenario", string("Scenario id or name."))], &["scenario"]),
        scenario_delete,
    );
    r.add(
        "transport.scenario.set_base",
        "Use a scenario as the base (do-minimum) for comparisons.",
        true,
        params(vec![("scenario", sc())], &[]),
        scenario_set_base,
    );
    r.add(
        "transport.scenario.set_year",
        "Set a scenario's forecast year (1900 to 2200); results go out of date.",
        true,
        params(
            vec![("scenario", sc()), ("year", integer("The forecast year."))],
            &["year"],
        ),
        scenario_set_year,
    );
    r.add(
        "transport.scenario.set",
        "Change scenario fields (name, notes, role, year, buildout, periods) as the panel's fields do, one undo step.",
        true,
        params(vec![("scenario", sc()), ("values", free_object("Fields to set: { \"role\": \"proposal\", \"notes\": \"...\" }."))], &["values"]),
        scenario_set,
    );
    r.add(
        "transport.scenario.clear_results",
        "Forget the results of a scenario.",
        true,
        params(vec![("scenario", sc())], &[]),
        scenario_clear_results,
    );
    r.add(
        "transport.scenario.describe",
        "Describe what a scenario contains in words: role, year, land uses, network stages, periods and growth.",
        false,
        params(vec![("scenario", sc())], &[]),
        |s, p| {
            let i = s.pick_scenario(p, "scenario")?;
            Ok(val(&scenarios::describe(&s.project.scenarios[i], &s.project)))
        },
    );
    r.add(
        "transport.scenario.table",
        "The assessment scenarios table (by year, then without and with the proposal) and the scenarios grouped by year.",
        false,
        none(),
        |s, _| Ok(json!({ "table": val(&scenarios::table(&s.project)), "groups": val(&scenarios::groups(&s.project)) })),
    );
    r.add(
        "transport.register.table",
        "The committed-development register: each row with its type, status, likelihood, funding and whether it belongs in the future baseline, and why.",
        false,
        none(),
        |s, _| Ok(json!({ "rows": edit::register::table(&s.project) })),
    );
    r.add(
        "transport.register.presets",
        "The preset register rows the app offers (committed network projects and precincts), as they stand on a date.",
        false,
        params(vec![("asAt", string("The date the statuses are as at (default the engine's preset date)."))], &[]),
        |_, p| {
            let as_at = p.opt_str("asAt")?.unwrap_or(edit::register::PRESET_AS_AT).to_string();
            Ok(json!({ "asAt": as_at, "rows": val(&edit::register::presets(&as_at)) }))
        },
    );
    r.add(
        "transport.scenario.check",
        "Check a scenario for unfunded projects, programmed projects and development in a baseline.",
        false,
        params(vec![("scenario", sc())], &[]),
        |s, p| {
            let i = s.pick_scenario(p, "scenario")?;
            let c = scenarios::check(&s.project.scenarios[i], &s.project);
            Ok(json!({ "scenario": s.sid(i), "blocks": c.iter().filter(|x| x.level == "block").count(), "findings": val(&c) }))
        },
    );
}

fn scenario_select(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.scenario_index(p.str("scenario")?)?;
    let id = s.sid(i);
    s.project.active = Maybe::Set(id.clone());
    s.touch();
    Ok(json!({ "active": id, "name": s.project.scenarios[i].name.as_str() }))
}

fn name_set(s: &Session) -> HashSet<String> {
    s.project
        .scenarios
        .iter()
        .map(|x| x.name.as_str().unwrap_or("").to_string())
        .collect()
}

fn next_name(s: &Session) -> String {
    let names = name_set(s);
    let mut n = s.project.scenarios.len() + 1;
    while names.contains(&format!("Scenario {n}")) {
        n += 1;
    }
    format!("Scenario {n}")
}

/// `duplicateScenario()` of ui.js: a copy of the active scenario without results, named "Scenario N", made active.
fn scenario_copy(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let mut c = s.project.scenarios[i].clone();
    let name = match p.opt_str("name")? {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => next_name(s),
    };
    c.res = Maybe::Set(Value::Object(Map::new()));
    c.id = edit::IdSource::uid(&mut s.ids, "s");
    c.name = Maybe::Set(name.clone());
    let id = c.id.clone();
    let from = s.project.scenarios[i]
        .name
        .as_str()
        .unwrap_or("")
        .to_string();
    s.project.scenarios.push(c);
    s.project.active = Maybe::Set(id.clone());
    s.touch();
    Ok(json!({
        "id": id, "name": name, "copiedFrom": from,
        "message": format!("Created {name} as a copy of {from}. Change it, then run the model to compare.")
    }))
}

fn scenario_add(s: &mut Session, p: &Params) -> ApiResult {
    let name = match p.opt_str("name")? {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => format!("Scenario {}", s.project.scenarios.len() + 1),
    };
    let sc = edit::new_scenario(&mut s.ids, &name);
    let id = sc.id.clone();
    s.project.scenarios.push(sc);
    s.project.active = Maybe::Set(id.clone());
    s.touch();
    Ok(json!({ "id": id, "name": name }))
}

fn scenario_create_set(s: &mut Session, p: &Params) -> ApiResult {
    let mut o = CreateSetOpts {
        pathway: p.opt_str("pathway")?.map(str::to_string),
        count_year: p.opt_f64("countYear")?,
        opening: p.opt_f64("opening")?,
        design: p.opt_f64("design")?,
        from: match p.opt_str("from")? {
            Some(k) => Some(s.sid(s.scenario_index(k)?)),
            None => None,
        },
        mitigation: p.opt_bool("mitigation")?,
        periods: p.opt_strings("periods")?,
        buildout: HashMap::new(),
    };
    if let Some(b) = p.opt_object("buildout")? {
        for (k, v) in b {
            let x = v
                .as_f64()
                .ok_or_else(|| ApiError::bad_params(format!("`buildout.{k}` must be a number")))?;
            o.buildout.insert(k.clone(), x);
        }
    }
    let made = scenarios::create_set(&mut s.project, &mut s.ids, &o)?;
    s.touch();
    let list: Vec<Value> = made
        .scenarios
        .iter()
        .filter_map(|id| s.project.scenarios.iter().position(|x| &x.id == id))
        .map(|i| s.scenario_brief(i))
        .collect();
    Ok(
        json!({ "scenarios": list, "excluded": made.excluded, "warnings": made.warnings, "set": made.set }),
    )
}

fn scenario_sensitivity_copy(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let id = s.sid(i);
    let unfunded = p.flag("unfunded")?;
    let made = scenarios::sensitivity_of(&mut s.project, &mut s.ids, &id, unfunded)
        .ok_or_else(|| ApiError::not_found("There is no such scenario."))?;
    s.touch();
    let at = s
        .project
        .scenarios
        .iter()
        .position(|x| x.id == made)
        .unwrap_or(0);
    Ok(s.scenario_brief(at))
}

fn scenario_rename(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let name = p.str("name")?;
    if name.is_empty() {
        return Err(ApiError::bad_params("The name cannot be blank."));
    }
    s.project.scenarios[i].name = Maybe::Set(name.to_string());
    s.touch();
    Ok(json!({ "id": s.sid(i), "name": name }))
}

fn scenario_delete(s: &mut Session, p: &Params) -> ApiResult {
    if s.project.scenarios.len() < 2 {
        return Err(ApiError::invalid_state(
            "A project needs at least one scenario.",
        ));
    }
    let i = s.scenario_index(p.str("scenario")?)?;
    // the app deletes the active scenario, so the one named becomes active first
    let removed = s.project.scenarios.remove(i);
    let first = s.project.scenarios[0].id.clone();
    s.project.active = Maybe::Set(first.clone());
    if s.project.base.is(&removed.id) {
        s.project.base = Maybe::Set(first.clone());
    }
    s.forget_scenario_steps(&removed.id);
    s.results.remove(&removed.id);
    if s.sim.as_ref().is_some_and(|x| x.scenario == removed.id) {
        s.sim = None;
    }
    s.touch();
    Ok(json!({
        "deleted": removed.id, "name": removed.name.as_str(), "active": first
    }))
}

fn scenario_set_base(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let id = s.sid(i);
    s.project.base = Maybe::Set(id.clone());
    s.touch();
    let name = s.project.scenarios[i]
        .name
        .as_str()
        .unwrap_or("")
        .to_string();
    Ok(json!({ "base": id, "message": format!("{name} is now the base (do-minimum).") }))
}

fn scenario_set_year(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let y = math_round(p.f64("year")?);
    if !(1900.0..=2200.0).contains(&y) {
        return Err(ApiError::bad_params(
            "Enter a forecast year between 1900 and 2200.",
        ));
    }
    let cur = year_of(Some(&s.project.scenarios[i]), &s.project.settings);
    if y == cur && !s.project.scenarios[i].year.is_nullish() {
        return Ok(json!({ "year": y, "changed": false }));
    }
    s.snap(&format!(
        "forecast year {}",
        construct_transport::jsnum::number_to_js_string(y)
    ));
    s.project.scenarios[i].year = construct_transport::jsnum::Num::number(y);
    s.edited();
    Ok(json!({ "year": y, "changed": true }))
}

fn scenario_set(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let id = s.sid(i);
    let values = p.object("values")?;
    apply_fields(s, FieldKind::Scenario, &id, values)
}

fn scenario_clear_results(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    s.clear_res(i);
    Ok(json!({ "scenario": s.sid(i), "cleared": true }))
}
