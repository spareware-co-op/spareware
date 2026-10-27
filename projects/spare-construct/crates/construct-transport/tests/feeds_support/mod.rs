//! What the data feeds truth tests share: the made-up answers under tools/transport-truth/fixtures/data, the recorded request logs of
//! the prototype's runs, and a stand-in for the network that replays them: every request a flow makes must be the recorded one
//! (same method, address and body), and is answered the way the recorded run was answered.
#![allow(dead_code)]

use construct_transport::edit::json::JsonValue;
use construct_transport::feeds::{Answer, FetchFailure, Flow, HttpRequest};
use construct_transport::project::Project;
use serde_json::Value;

/// The prototype's clock in every recorded run: 2026-10-21T00:00:00Z.
pub const NOW: f64 = 1_792_540_800_000.0;

/// The text of a file under tools/transport-truth/fixtures/data.
pub fn fixture_text(file: &str) -> String {
    let p = crate::support::truth_dir().join("fixtures").join("data").join(file);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// The bytes of a file under tools/transport-truth/fixtures/data.
pub fn fixture_bytes(file: &str) -> Vec<u8> {
    let p = crate::support::truth_dir().join("fixtures").join("data").join(file);
    std::fs::read(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// A fixture file as JSON with its key order kept.
pub fn fixture_json(file: &str) -> JsonValue {
    JsonValue::parse(&fixture_text(file)).unwrap_or_else(|e| panic!("{file} is not JSON: {e}"))
}

/// The features of a saved ArcGIS answer, in order.
pub fn fixture_features(file: &str) -> Vec<JsonValue> {
    match fixture_json(file).get("features") {
        Some(JsonValue::Array(a)) => a.clone(),
        _ => panic!("{file} has no features"),
    }
}

/// expected/feeds-<section>.json.
pub fn expected(section: &str) -> Value {
    crate::support::load_json(&format!("expected/feeds-{section}.json"))
}

/// The test project inputs/<name>.json.
pub fn project(name: &str) -> Project {
    crate::support::load_project(name)
}

/// A project from its saved text.
pub fn project_of_text(text: &str) -> Project {
    Project::from_json(text).expect("project reads")
}

/// A recorded value as the ordered JSON the prototype's code read.
pub fn ordered(v: &Value) -> JsonValue {
    JsonValue::from_value(v)
}

/// The answer a request got in a recorded run, or the failure it met.
pub fn answer_of(desc: &Value) -> Result<Answer, FetchFailure> {
    let kind = desc["kind"].as_str().expect("answer kind");
    let file = || desc["file"].as_str().expect("answer file");
    match kind {
        "features" if desc.get("inline").is_some() => {
            let feats: Vec<JsonValue> = desc["inline"].as_array().unwrap().iter().map(JsonValue::from_value).collect();
            let mut ans = JsonValue::object();
            ans.set("features", JsonValue::Array(feats));
            if desc["exceeded"].as_bool() == Some(true) {
                ans.set("exceededTransferLimit", JsonValue::Bool(true));
            }
            Ok(Answer::Json(ans))
        }
        "features" => {
            let all = fixture_features(file());
            let want: Option<Vec<String>> = desc.get("fields").and_then(Value::as_array).map(|a| a.iter().map(|x| x.as_str().unwrap().to_string()).collect());
            let feats: Vec<JsonValue> = desc["idx"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| {
                    let f = all[i.as_u64().unwrap() as usize].clone();
                    match &want {
                        None => f,
                        Some(w) => {
                            let mut g = f.clone();
                            let attrs = f.get("attributes").and_then(JsonValue::as_object).map(|a| a.iter().filter(|(k, _)| w.contains(&k.to_lowercase())).map(|(k, v)| (k.to_string(), v.clone())).collect::<Vec<_>>()).unwrap_or_default();
                            g.set("attributes", JsonValue::from_pairs(attrs));
                            g
                        }
                    }
                })
                .collect();
            let mut ans = JsonValue::object();
            ans.set("features", JsonValue::Array(feats));
            if desc["exceeded"].as_bool() == Some(true) {
                ans.set("exceededTransferLimit", JsonValue::Bool(true));
            }
            Ok(Answer::Json(ans))
        }
        "json" => Ok(Answer::Json(fixture_json(file()))),
        "literal" => Ok(Answer::Json(JsonValue::from_value(&desc["value"]))),
        "text" => Ok(Answer::Text(fixture_text(file()))),
        "literal-text" => Ok(Answer::Text(desc["value"].as_str().unwrap().to_string())),
        "bytes" => Ok(Answer::Bytes { data: fixture_bytes(file()), last_modified: desc["lastModified"].as_str().map(str::to_string) }),
        "head" => Ok(Answer::Head { last_modified: desc["lastModified"].as_str().map(str::to_string) }),
        "fail" => Err(match desc["failure"].as_str().unwrap() {
            "http" => FetchFailure::Http(desc["status"].as_u64().unwrap() as u16),
            "unreachable" => FetchFailure::Unreachable(desc["message"].as_str().unwrap().to_string()),
            "unreadable" => FetchFailure::Unreadable(desc["message"].as_str().unwrap().to_string()),
            "timeout" => FetchFailure::TimedOut,
            "timeout-reading" => FetchFailure::TimedOutReading,
            "cancel" => FetchFailure::Cancelled,
            other => panic!("unknown failure {other}"),
        }),
        other => panic!("unknown answer kind {other}"),
    }
}

/// Panics unless a request is the recorded one.
pub fn check_request(req: &HttpRequest, rec: &Value, label: &str) {
    assert_eq!(req.method, rec["method"].as_str().unwrap(), "{label}: method of {}", req.url);
    assert_eq!(req.url, rec["url"].as_str().unwrap(), "{label}: address");
    assert_eq!(req.body.as_deref(), rec["body"].as_str(), "{label}: body of {}", req.url);
    if let Some(h) = rec["headers"].as_object() {
        for (k, v) in h {
            let got = req.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str());
            assert_eq!(got, v.as_str(), "{label}: header {k} of {}", req.url);
        }
    }
}

/// Runs a flow against a recorded log starting at an entry: each request it makes must be the next recorded one. Gives the position
/// after the last entry used and how the flow ended (Err with its message when a request failed the flow).
pub fn replay<F: Flow>(flow: &mut F, log: &[Value], from: usize, label: &str) -> (usize, Result<(), String>) {
    let mut k = from;
    while let Some(req) = flow.next_request() {
        let rec = log.get(k).unwrap_or_else(|| panic!("{label}: the flow asks for request {k} ({}) but only {} were recorded", req.url, log.len()));
        check_request(&req, rec, &format!("{label} request {k}"));
        k += 1;
        let ans = answer_of(&rec["answer"]);
        if let Err(e) = flow.answer(ans) {
            return (k, Err(e));
        }
    }
    (k, Ok(()))
}

/// Panics unless every recorded request was used.
pub fn all_used(log: &[Value], used: usize, label: &str) {
    assert_eq!(used, log.len(), "{label}: the flow made {used} requests, the recording has {}", log.len());
}

/// A recorded bounds object as bounds.
pub fn bounds_of(v: &Value) -> construct_transport::feeds::Bounds {
    construct_transport::feeds::Bounds { south: v["south"].as_f64().unwrap(), west: v["west"].as_f64().unwrap(), north: v["north"].as_f64().unwrap(), east: v["east"].as_f64().unwrap() }
}

/// A recorded value with the generator's "NaN" marker read as null: the prototype's JSON writes a number that is not a number as null, as the Rust does.
pub fn nan_as_null(v: &Value) -> Value {
    match v {
        Value::String(s) if s == "NaN" => Value::Null,
        Value::Array(a) => Value::Array(a.iter().map(nan_as_null).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), nan_as_null(x))).collect()),
        other => other.clone(),
    }
}

struct Unused(u32);

impl construct_transport::edit::ids::IdSource for Unused {
    fn uid(&mut self, prefix: &str) -> String {
        self.0 += 1;
        format!("{prefix}x{}", self.0)
    }
    fn now_ms(&self) -> f64 {
        NOW
    }
}

/// A project reopened as the prototype's `reopen` does: read from its text and migrated (years written as text become numbers).
pub fn reopen(text: &str) -> Project {
    construct_transport::edit::migrate::migrate_text(text, &mut Unused(0)).expect("the project migrates")
}

/// A recorded section with its key order kept (for values the prototype read as ordered objects).
pub fn expected_ordered(section: &str) -> JsonValue {
    let p = crate::support::truth_dir().join("expected").join(format!("feeds-{section}.json"));
    JsonValue::parse(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))).expect("recorded JSON")
}
