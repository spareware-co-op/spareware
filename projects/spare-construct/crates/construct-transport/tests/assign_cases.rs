//! The assignment on the variants of the truth-test projects that gen-assign-cases.mjs makes
//! (expected/assign-cases.json): the options, select-link, select-turn and select-zone, charges and tolls, the
//! convergence rules, signal optimisation, junction and capacity methods, vehicle mix, turn penalties and bans, development
//! zones, bus routes and lanes, closed links, matrix estimation, junction sites of each control, unserved trips, more than
//! 300 zones and a settings object that is not the project's. Every case's whole result must equal the prototype's
//! (only the time stamp and run time left out), and so must the calls it makes to its progress callback.

mod recorded;
mod support;

use construct_transport::assign::assign;
use construct_transport::jsnum::plain_number;
use construct_transport::project::{Project, Settings};
use construct_transport::results::AssignOpts;
use recorded::json;
use serde_json::{Value, json};

use support::{diff_json, load_json};

#[test]
fn assignment_on_the_variants() {
    let cases = load_json("expected/assign-cases.json");
    let cases = cases.as_array().expect("cases");
    assert_eq!(cases.len(), 85);
    let mut failures: Vec<String> = Vec::new();
    for case in cases {
        let name = case["name"].as_str().expect("name");
        let p = Project::from_json(case["projectText"].as_str().expect("project text"))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let sc = p
            .scenario(case["scenario"].as_str().expect("scenario"))
            .expect("scenario");
        let s: Settings = match case["settingsText"].as_str() {
            Some(t) => serde_json::from_str(t).unwrap_or_else(|e| panic!("{name}: settings: {e}")),
            None => p.settings.clone(),
        };
        let opts: AssignOpts = serde_json::from_value(case["opts"].clone())
            .unwrap_or_else(|e| panic!("{name}: opts: {e}"));
        let mut calls: Vec<Value> = Vec::new();
        let mut on_progress = |it: u32, gap: f64, msg: &str| {
            calls.push(json!([
                it,
                plain_number(gap),
                if msg.is_empty() {
                    Value::Null
                } else {
                    json!(msg)
                }
            ]));
        };
        let mut got = json(&assign(&p, sc, &s, &opts, Some(&mut on_progress)));
        let (calls_diffs, calls_total) = diff_json(
            &Value::Array(calls),
            &case["progress"],
            &format!("{name}.progress"),
        );
        if calls_total > 0 {
            failures.push(format!(
                "{calls_total} difference(s) in {name}.progress, first: {} expected {} actual {}",
                calls_diffs[0].path, calls_diffs[0].expected, calls_diffs[0].actual
            ));
        }
        for k in case["omit"].as_array().expect("omit") {
            got.as_object_mut()
                .expect("result")
                .remove(k.as_str().expect("key"));
        }
        let path = format!("{name}.res");
        let (diffs, total) = diff_json(&got, &case["res"], &path);
        if total > 0 {
            let mut msg = format!("{total} difference(s) under {path}:");
            for d in diffs.iter().take(6) {
                msg.push_str(&format!(
                    "\n  {}\n      expected {}\n      actual   {}",
                    d.path, d.expected, d.actual
                ));
            }
            failures.push(msg);
        }
    }
    assert!(
        failures.is_empty(),
        "{} case(s) differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
