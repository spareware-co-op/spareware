//! What the safety and feeds truth tests share: the made-up crash and Future Connect answers under
//! tools/transport-truth/fixtures/data, and a stand-in for the server that serves them the way the recorded run did.
#![allow(dead_code)]

use construct_transport::edit::json::JsonValue;
use construct_transport::feeds::{Bounds, LoadAll, LoadOptions};
use construct_transport::project::Project;
use serde_json::Value;

/// The text of a file under tools/transport-truth/fixtures/data.
pub fn data_text(file: &str) -> String {
    let p = crate::support::truth_dir()
        .join("fixtures")
        .join("data")
        .join(file);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// The features of a saved ArcGIS answer (fixtures/data/<file>), in order.
pub fn features_of(file: &str) -> Vec<JsonValue> {
    let v = JsonValue::parse(&data_text(file)).expect("fixture is JSON");
    match v.get("features") {
        Some(JsonValue::Array(a)) => a.clone(),
        _ => panic!("{file} has no features"),
    }
}

/// The saved test project with the divided road (fixtures/data/safety-<name>-project.json) as typed project.
pub fn base_project(name: &str) -> Project {
    Project::from_json(&data_text(&format!("safety-{name}-project.json"))).expect("project reads")
}

/// A box recorded as { south, west, north, east }.
pub fn bounds_of(v: &Value) -> Bounds {
    Bounds {
        south: v["south"].as_f64().unwrap(),
        west: v["west"].as_f64().unwrap(),
        north: v["north"].as_f64().unwrap(),
        east: v["east"].as_f64().unwrap(),
    }
}

/// Runs a recorded load against a project: every request must be the recorded one (same address), and is answered with the
/// features the recorded run's server sent. Gives the load's messages and status lines.
pub fn replay_load(p: &mut Project, project: &str, rec: &Value) -> (Vec<String>, Vec<String>) {
    let which: Vec<&str> = rec["which"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    let opts = LoadOptions {
        cas_years: rec["opts"]["casYears"].as_f64(),
        timeout_ms: None,
    };
    let mut load = LoadAll::new(
        bounds_of(&rec["bounds"]),
        &which,
        &opts,
        rec["now"].as_f64().unwrap(),
    );
    let cas = features_of(&format!("cas-{project}.json"));
    let fc = features_of(&format!("fc-{project}.json"));
    for (k, exp) in rec["requests"].as_array().unwrap().iter().enumerate() {
        let req = load
            .next_request(p)
            .expect("no cancel")
            .unwrap_or_else(|| panic!("request {k} is missing"));
        assert_eq!(req.url, exp["url"].as_str().unwrap(), "request {k}");
        let layer = if req.url.contains("CAS_Data_Public") {
            &cas
        } else {
            &fc
        };
        let feats: Vec<JsonValue> = exp["idx"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| layer[i.as_u64().unwrap() as usize].clone())
            .collect();
        let mut ans = JsonValue::object();
        ans.set("features", JsonValue::Array(feats));
        if exp["exceeded"].as_bool() == Some(true) {
            ans.set("exceededTransferLimit", JsonValue::Bool(true));
        }
        load.answer(p, Ok(ans)).expect("answer");
    }
    assert!(
        load.next_request(p).expect("no cancel").is_none(),
        "more requests than recorded"
    );
    (load.messages().to_vec(), load.statuses().to_vec())
}
