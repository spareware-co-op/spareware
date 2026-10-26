//! The calibration panel's calculations through the states a project goes through, against the JavaScript's recorded
//! answers (expected/calibrate-panel-*.json, from tools/transport-truth/gen-calibrate.mjs): counts, screenlines, a
//! cordon, journey checks and turning counts as imported through the panel's own importer, then thirteen states
//! (as prepared, held out, roles by hand, after matrix estimation, after trip-end factors, other scenario, core area
//! drawn, changed settings, balanced turning counts, sites removed, few counts, gateway counts). Each state is run
//! from the project text with the real Rust assignment, and everything the panel shows is compared: roles, methods,
//! rows, statistics, screenline rows, journey checks, turning-count rows, network checks, the evidence and the
//! assessment under each standard and category, the criteria tables, the CSV exports and the records the
//! steps make.

mod calibrate_support;
mod support;

use calibrate_support::states::{replay_state, run_both, snapshot};
use construct_transport::calibrate::{CalibStore, import_turn_csv, match_sites};
use construct_transport::edit::IdSource;
use construct_transport::project::Project;
use serde_json::{Value, json};
use support::{assert_close, diff_json, load_json};

const SKIPPED: [&str; 4] = ["name", "setup", "level", "snapshot"];

/// The balancing audit lists in a canonical order (site, then movement).
///
/// The audit follows the order of a count's movements ("W-T" before "W-R" as the file gave them). The project keeps
/// the calibration store as JSON values, whose objects serde_json holds sorted by key, so a project read from text
/// has the movements in key order instead. The entries themselves are identical.
fn sort_audits(v: &mut Value) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                if k == "audit" {
                    match x {
                        Value::Array(a) => sort_entries(a),
                        Value::Object(per) => per.values_mut().for_each(|y| {
                            if let Value::Array(a) = y {
                                sort_entries(a)
                            }
                        }),
                        _ => {}
                    }
                } else {
                    sort_audits(x);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(sort_audits),
        _ => {}
    }
}

fn sort_entries(a: &mut [Value]) {
    a.sort_by_key(|e| (e["siteId"].as_str().unwrap_or("").to_string(), e["movement"].as_str().unwrap_or("").to_string()));
}

fn replay(file: &str, expect_states: usize) {
    let all = load_json(file);
    let text = all["projectText"].as_str().expect("projectText");
    let sc_id = all["scenario"].as_str().expect("scenario").to_string();
    let states = all["states"].as_array().expect("states");
    assert_eq!(states.len(), expect_states, "{file}: states");
    let only = std::env::var("PANEL_STATE").ok();
    let mut failures: Vec<String> = Vec::new();
    let mut check = |got: &Value, want: &Value, path: String| {
        let (mut g, mut w) = (got.clone(), want.clone());
        sort_audits(&mut g);
        sort_audits(&mut w);
        let (diffs, total) = diff_json(&g, &w, &path);
        if total > 0 {
            let mut m = format!("{total} difference(s) under {path}");
            for d in diffs.iter().take(6) {
                m.push_str(&format!("
    {}
        expected {}
        actual   {}", d.path, d.expected, d.actual));
            }
            failures.push(m);
        }
    };
    for st in states {
        let name = st["name"].as_str().expect("name");
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        let mut p = Project::from_json(text).expect("the project text reads");
        run_both(&mut p, &sc_id);
        let out = replay_state(&mut p, name, all["fixture"].as_str().unwrap_or(""), &all["coreArea"]);
        // what the steps recorded
        for (k, want) in st.as_object().expect("state") {
            if SKIPPED.contains(&k.as_str()) {
                continue;
            }
            match out.get(k) {
                Some(got) => check(got, want, format!("{name}: {k}")),
                None => check(&Value::Null, want, format!("{name}: the replay did not record {k}")),
            }
        }
        // everything the panel shows afterwards
        let level = st["level"].as_str().expect("level");
        let sc = p.scenarios[0].id.clone();
        let got = snapshot(&p, &sc, name, level, true);
        check(&got, &st["snapshot"], format!("{name}: snapshot"));
    }
    assert!(failures.is_empty(), "{file}: {} part(s) differ from the JavaScript
{}", failures.len(), failures.join("
"));
}

#[test]
fn corridor_states() {
    replay("expected/calibrate-panel-corridor.json", 13);
}

#[test]
fn corridor_forecast_states() {
    replay("expected/calibrate-panel-corridor-forecast.json", 3);
}

#[test]
fn grid_states() {
    replay("expected/calibrate-panel-grid.json", 13);
}

/// Hands out the ids the JavaScript made, in order.
struct Queue(std::collections::VecDeque<String>);

impl IdSource for Queue {
    fn uid(&mut self, prefix: &str) -> String {
        let id = self.0.pop_front().expect("an id left");
        assert!(id.starts_with(prefix), "{id} does not start with {prefix}");
        id
    }
    fn now_ms(&self) -> f64 {
        1_792_540_800_000.0
    }
}

fn ids_of(v: &Value) -> Queue {
    Queue(v.as_array().expect("ids").iter().map(|x| x.as_str().expect("id").to_string()).collect())
}

/// The panel's own importer (files read, peak hours found, sites matched to junctions) makes the stored sites.
#[test]
fn imported_turning_counts() {
    for f in ["expected/calibrate-panel-corridor.json", "expected/calibrate-panel-grid.json"] {
        let all = load_json(f);
        let sc_id = all["scenario"].as_str().expect("scenario").to_string();
        let mut p = Project::from_json(all["projectText"].as_str().expect("text")).expect("project");
        let want = CalibStore::read(&p).turn_sites;
        assert!(want.len() >= 5, "{f}: {} stored sites", want.len());
        let mut st = CalibStore::read(&p);
        st.turn_sites.clear();
        st.write(&mut p);
        let texts = all["texts"].as_array().expect("texts");
        let imported = all["imported"].as_array().expect("imported");
        assert_eq!(imported.len(), texts.len());
        for (t, rec) in texts.iter().zip(imported) {
            let file = t["file"].as_str().expect("file");
            let r = import_turn_csv(&mut p, t["text"].as_str().expect("text"), file, &mut ids_of(&rec["sites"])).expect("imports");
            assert_close(&json!({ "skipped": r.skipped, "reasons": r.reasons }), &json!({ "skipped": rec["skipped"], "reasons": rec["reasons"] }), file);
        }
        match_sites(&mut p, &sc_id);
        import_turn_csv(&mut p, all["far"].as_str().expect("far"), "far.csv", &mut ids_of(&all["farIds"])).expect("imports");
        match_sites(&mut p, &sc_id);
        let got = serde_json::to_value(&CalibStore::read(&p).turn_sites).expect("sites");
        assert_close(&got, &serde_json::to_value(&want).expect("sites"), f);
    }
}
