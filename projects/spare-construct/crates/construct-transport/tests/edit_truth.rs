//! Editing truth tests: network edits, junction sites, assessment scenarios, the register, undo, project upgrade (migrate), ids,
//! number text, dates and name order against the prototype.
//!
//! tools/transport-truth/gen-edit.mjs ran the prototype's model.js functions (edit-ops.mjs and edit-scripts.mjs hold the
//! operation lists, edit-migrate-cases.mjs the saved projects) and wrote expected/edit-*.json; these tests replay the same
//! operations with the same seeded id generator and compare every answer and every project state.

mod edit_ops;
mod edit_support;
mod support;

use construct_transport::edit::collate::locale_compare;
use construct_transport::edit::ids::{iso_string, number_to_radix36};
use construct_transport::edit::json::JsonValue;
use construct_transport::edit::migrate::{
    adopt_defaults, keep_settings, migrate_json, migrate_text,
};
use construct_transport::edit::{IdSource, Ids};
use edit_ops::{apply, ctx_for};
use edit_support::{Checker, load_json};
use engine_sim::{Mulberry32, Rng};
use serde_json::{Value, json};
use std::cmp::Ordering;

/// Replays one recorded operation list and compares every answer and project state.
fn replay(file: &str, ck: &mut Checker) -> usize {
    let ex = load_json(&format!("expected/{file}"));
    replay_case(file, &ex, ck)
}

/// The same for a case already loaded.
fn replay_case(file: &str, ex: &Value, ck: &mut Checker) -> usize {
    let mut ctx = ctx_for(ex);
    let ops = ex["ops"].as_array().expect("ops");
    let steps = ex["steps"].as_array().expect("steps");
    assert_eq!(ops.len(), steps.len(), "{file}: one step per operation");
    for (k, (o, step)) in ops.iter().zip(steps).enumerate() {
        let at = format!("{file}[{k}]({})", o["op"].as_str().unwrap_or("?"));
        let (ret, state) = apply(&mut ctx, o);
        ck.check(&format!("{at}.ret"), &ret, &step["ret"]);
        match (state, step.get("state")) {
            (Some(s), Some(e)) => ck.check(&format!("{at}.state"), &s, e),
            (None, None) => {}
            (Some(_), None) => ck.same(&format!("{at}: state recorded"), &true, &false),
            (None, Some(_)) => ck.same(&format!("{at}: state recorded"), &false, &true),
        }
    }
    ops.len()
}

#[test]
fn a_network_built_and_edited_node_by_node_matches_the_prototype() {
    let mut ck = Checker::default();
    let n = replay("edit-build.json", &mut ck);
    println!("operations replayed: {n}");
    ck.finish("network edits");
}

#[test]
fn random_networks_match_the_prototype() {
    let cases = load_json("expected/edit-fuzz.json");
    let mut ck = Checker::default();
    let mut n = 0;
    for (k, case) in cases.as_array().expect("cases").iter().enumerate() {
        n += replay_case(&format!("edit-fuzz[{k}]"), case, &mut ck);
    }
    println!("operations replayed: {n}");
    ck.finish("random networks");
}

#[test]
fn junction_sites_match_the_prototype() {
    let mut ck = Checker::default();
    let n = replay("edit-sites.json", &mut ck);
    println!("operations replayed: {n}");
    ck.finish("junction sites");
}

#[test]
fn scenarios_the_register_and_checks_match_the_prototype() {
    let mut ck = Checker::default();
    let n = replay("edit-scenarios.json", &mut ck);
    println!("operations replayed: {n}");
    ck.finish("scenarios");
}

#[test]
fn undo_and_redo_match_the_prototype() {
    let mut ck = Checker::default();
    let n = replay("edit-history.json", &mut ck);
    println!("operations replayed: {n}");
    ck.finish("undo and redo");
}

fn ids_from(seed: u64, skip: u64, seq0: u64, now: f64) -> Ids<Mulberry32> {
    let mut rng = Mulberry32::new(seed as u32);
    for _ in 0..skip {
        rng.next_f64();
    }
    Ids::new(rng, now).with_seq(seq0)
}

#[test]
fn ids_match_the_prototype() {
    let ex = load_json("expected/edit-ids.json");
    let mut ck = Checker::default();
    let mut one = |ids: &Value, prefixes: &dyn Fn(usize) -> String| {
        let mut g = ids_from(
            ids["seed"].as_u64().expect("seed"),
            ids["rngSkip"].as_u64().expect("skip"),
            ids["seq0"].as_u64().expect("seq0"),
            ids["now"].as_f64().expect("now"),
        );
        for (i, want) in ids["ids"].as_array().expect("ids").iter().enumerate() {
            let got = g.uid(&prefixes(i));
            ck.same(
                &format!("seed {} id {i}", ids["seed"]),
                &got,
                &want.as_str().expect("text").to_string(),
            );
        }
    };
    for case in ex["ids"].as_array().expect("cases") {
        let prefixes: Vec<String> = case["prefixes"]
            .as_array()
            .expect("prefixes")
            .iter()
            .map(|p| p.as_str().expect("text").to_string())
            .collect();
        one(case, &|i| prefixes[i % prefixes.len()].clone());
    }
    // A different clock changes only the middle of an id
    one(&ex["later"], &|i| format!("x{i}"));
    ck.finish("ids");
}

#[test]
fn number_text_in_base_36_matches_the_prototype() {
    let ex = load_json("expected/edit-ids.json");
    let mut ck = Checker::default();
    for (i, row) in ex["radix"].as_array().expect("radix").iter().enumerate() {
        let x = row[0].as_f64().expect("number");
        ck.same(
            &format!("radix[{i}] {x:e}"),
            &number_to_radix36(x),
            &row[1].as_str().expect("text").to_string(),
        );
    }
    ck.finish("Number.toString(36)");
}

#[test]
fn dates_match_the_prototype() {
    let ex = load_json("expected/edit-ids.json");
    let mut ck = Checker::default();
    for (i, row) in ex["dates"].as_array().expect("dates").iter().enumerate() {
        ck.same(
            &format!("dates[{i}]"),
            &iso_string(row[0].as_f64().expect("ms")),
            &row[1].as_str().expect("text").to_string(),
        );
    }
    ck.finish("Date.toISOString");
}

#[test]
fn name_order_matches_localecompare() {
    let ex = load_json("expected/edit-ids.json");
    let mut ck = Checker::default();
    for (i, row) in ex["collate"].as_array().expect("pairs").iter().enumerate() {
        let (a, b) = (
            row[0].as_str().expect("text"),
            row[1].as_str().expect("text"),
        );
        let got = match locale_compare(a, b) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        };
        ck.same(
            &format!("collate[{i}] {a:?} vs {b:?}"),
            &got,
            &row[2].as_i64().expect("sign"),
        );
    }
    ck.finish("localeCompare");
}

#[test]
fn migrate_matches_the_prototype() {
    let ex = load_json("expected/edit-migrate.json");
    let mut ck = Checker::default();
    let mut typed = 0usize;
    for case in ex["cases"].as_array().expect("cases") {
        let name = case["name"].as_str().expect("name");
        let text = case["text"].as_str().expect("text");
        let new_ids = || {
            ids_from(
                case["seed"].as_u64().expect("seed"),
                case["rngSkip"].as_u64().expect("skip"),
                case["seq0"].as_u64().expect("seq0"),
                ex["now"].as_f64().expect("now"),
            )
        };
        let at = format!("migrate[{name}]");
        // The upgrade on ordered JSON, the way the JavaScript runs it on the parsed file
        let m =
            migrate_json(JsonValue::parse(text).expect("JSON"), &mut new_ids()).expect("upgrades");
        ck.check(&format!("{at}.project"), &m.to_value(), &case["expected"]);
        // The order of the keys that matter: settings, rates, and each scenario's ids
        let o = &case["orders"];
        let keys = |v: Option<&JsonValue>| -> Vec<String> {
            v.and_then(JsonValue::as_object)
                .map(|m| m.keys().map(str::to_string).collect())
                .unwrap_or_default()
        };
        let want = |v: &Value| -> Vec<String> {
            v.as_array().map_or(Vec::new(), |a| {
                a.iter()
                    .map(|x| x.as_str().expect("text").to_string())
                    .collect()
            })
        };
        ck.same(
            &format!("{at}.order.settings"),
            &keys(m.get("settings")),
            &want(&o["settings"]),
        );
        ck.same(
            &format!("{at}.order.rates"),
            &keys(m.get("settings").and_then(|s| s.get("rates"))),
            &want(&o["rates"]),
        );
        let scs = m
            .get("scenarios")
            .and_then(JsonValue::as_array)
            .expect("scenarios");
        for (k, s) in scs.iter().enumerate() {
            let e = &o["scenarios"][k];
            ck.same(
                &format!("{at}.order.scenarios[{k}].id"),
                &s.get("id").map(|v| v.js_string()),
                &e["id"].as_str().map(str::to_string),
            );
            for key in ["nodes", "links", "zones", "routes", "sites", "res"] {
                ck.same(
                    &format!("{at}.order.scenarios[{k}].{key}"),
                    &keys(s.get(key)),
                    &want(&e[key]),
                );
            }
        }
        // As a typed project (a project with a register row that is not an object does not read as one: the JSON check above covers it)
        match migrate_text(text, &mut new_ids()) {
            Ok(p) => {
                typed += 1;
                ck.check(&format!("{at}.typed"), &p.to_value(), &case["expected"]);
                for (k, sc) in p.scenarios.iter().enumerate() {
                    let e = &o["scenarios"][k];
                    ck.same(
                        &format!("{at}.typed.order[{k}].nodes"),
                        &sc.nodes.keys().map(str::to_string).collect::<Vec<_>>(),
                        &want(&e["nodes"]),
                    );
                    ck.same(
                        &format!("{at}.typed.order[{k}].links"),
                        &sc.links.keys().map(str::to_string).collect::<Vec<_>>(),
                        &want(&e["links"]),
                    );
                }
                ck.same(
                    &format!("{at}.typed.order.rates"),
                    &p.settings
                        .rates
                        .get()
                        .map(|r| r.keys().map(str::to_string).collect::<Vec<_>>())
                        .unwrap_or_default(),
                    &want(&o["rates"]),
                );
            }
            Err(e) => {
                // Projects the prototype keeps as they are but a typed project cannot hold: a register row that is not an object, a matrix estimation that is an array
                assert!(
                    name.starts_with("register presets") || name.starts_with("me array"),
                    "{at}: the upgraded project does not read as a project: {e}"
                );
            }
        }
    }
    println!(
        "saved projects upgraded: {} ({typed} also as typed projects)",
        ex["cases"].as_array().expect("cases").len()
    );
    ck.finish("project upgrade");
}

#[test]
fn adopting_and_keeping_the_new_defaults_match_the_prototype() {
    let ex = load_json("expected/edit-migrate.json");
    let mut ck = Checker::default();
    for case in ex["adopt"].as_array().expect("cases") {
        let name = case["name"].as_str().expect("name");
        let mut ids = ids_from(
            case["seed"].as_u64().expect("seed"),
            case["rngSkip"].as_u64().expect("skip"),
            case["seq0"].as_u64().expect("seq0"),
            ex["now"].as_f64().expect("now"),
        );
        let mut p =
            migrate_text(case["text"].as_str().expect("text"), &mut ids).expect("a project");
        let mut rets: Vec<Value> = Vec::new();
        let mut act = |p: &mut construct_transport::project::Project, a: &Value| {
            if let Some(adopt) = a.get("adopt") {
                let keys: Option<Vec<String>> = adopt.as_array().map(|l| {
                    l.iter()
                        .map(|k| k.as_str().expect("key").to_string())
                        .collect()
                });
                rets.push(json!(adopt_defaults(p, keys.as_deref()).expect("adopts")));
            } else {
                rets.push(json!(keep_settings(p)));
            }
        };
        act(&mut p, &case["action"]);
        if let Some(again) = case["action"].get("again") {
            act(&mut p, again);
        }
        let at = format!("adopt[{name}]");
        ck.check(&format!("{at}.returns"), &Value::Array(rets), &case["rets"]);
        ck.check(&format!("{at}.project"), &p.to_value(), &case["expected"]);
    }
    ck.finish("adopt and keep");
}
