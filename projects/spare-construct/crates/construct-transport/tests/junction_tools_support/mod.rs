//! Support for the junction tools truth tests: reads a recorded variant (the saved project, the engine's results for it and the
//! stand-in counted peak hour factors) and counts checks against the prototype's answers.
#![allow(dead_code)] // each test file uses its own part of this module

use construct_transport::edit::json::JsonValue;
use construct_transport::junction_tools::Features;
use construct_transport::project::{Project, Scenario};
use construct_transport::results::Results;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Write as _;

pub use crate::support::{diff_json, read_truth};

/// The text of a file under tools/transport-truth.
pub fn text(rel: &str) -> String {
    read_truth(rel)
}

/// A recorded variant: the project (typed and as ordered JSON), each scenario's results and the stand-in peak hour factors.
pub struct Variant {
    /// Name.
    pub name: String,
    /// The project's JSON with its keys in file order.
    pub raw: JsonValue,
    /// The typed project.
    pub project: Project,
    /// Each scenario's results, in the project's order.
    pub results: Vec<Results>,
    /// The peak hour factors the stand-in Calibration.phf answers with, by "node|period".
    pub phf: HashMap<String, f64>,
}

/// Reads fixtures/data/<prefix>-<name>.json.
pub fn load_variant(prefix: &str, name: &str) -> Variant {
    let file = JsonValue::parse(&text(&format!("fixtures/data/{prefix}-{name}.json"))).expect("fixture JSON");
    let raw = file.get("project").expect("project").clone();
    let project: Project = serde_json::from_str(&raw.stringify()).expect("the project reads");
    let res_json = file.get("results").expect("results");
    let results: Vec<Results> = project
        .scenarios
        .iter()
        .map(|sc| {
            let r = res_json.get(&sc.id).expect("results of the scenario");
            serde_json::from_str(&r.stringify()).expect("results read")
        })
        .collect();
    let phf = file
        .get("phf")
        .and_then(JsonValue::as_object)
        .map(|o| {
            o.iter()
                .map(|(k, v)| (k.to_string(), v.as_f64().expect("number")))
                .collect()
        })
        .unwrap_or_default();
    Variant {
        name: name.to_string(),
        raw,
        project,
        results,
        phf,
    }
}

/// The stand-in for the Calibration feature: answers from a table.
pub struct PhfHost {
    /// Peak hour factors by "node|period".
    pub phf: HashMap<String, f64>,
}

impl Features for PhfHost {
    fn count_phf(&self, _sc: &Scenario, node_id: &str, period: &str) -> Option<f64> {
        self.phf.get(&format!("{node_id}|{period}")).copied()
    }
}

/// Counts checks against the JavaScript's answers and reports every difference at the end.
#[derive(Default)]
pub struct Checker {
    /// Checks run.
    pub run: usize,
    /// Checks that matched.
    pub passed: usize,
    report: String,
}

impl Checker {
    /// Compares one value with the JavaScript's (numbers within the tolerance, text exactly).
    pub fn check(&mut self, path: &str, actual: &Value, expected: &Value) {
        self.run += 1;
        let (diffs, total) = diff_json(actual, expected, path);
        if total == 0 {
            self.passed += 1;
            return;
        }
        let _ = write!(self.report, "\n{total} difference(s) under {path}:");
        for d in diffs.iter().take(6) {
            let _ = write!(
                self.report,
                "\n  {}\n      expected {}\n      actual   {}",
                d.path, d.expected, d.actual
            );
        }
    }
    /// Counts one exact comparison of text.
    pub fn same_text(&mut self, path: &str, actual: &str, expected: &str) {
        self.run += 1;
        if actual == expected {
            self.passed += 1;
        } else {
            let at = actual
                .chars()
                .zip(expected.chars())
                .position(|(a, e)| a != e)
                .unwrap_or_else(|| actual.chars().count().min(expected.chars().count()));
            let ctx = |s: &str| s.chars().skip(at.saturating_sub(60)).take(160).collect::<String>();
            let _ = write!(
                self.report,
                "\n{path}: texts differ at character {at} (lengths {} and {})\n      expected ...{:?}\n      actual   ...{:?}",
                expected.chars().count(),
                actual.chars().count(),
                ctx(expected),
                ctx(actual)
            );
        }
    }
    /// Prints the counts and panics when any check failed.
    pub fn finish(self, what: &str) {
        println!("{what}: {} of {} checks match the JavaScript", self.passed, self.run);
        if self.passed != self.run {
            let mut r = self.report;
            if r.len() > 60000 {
                r.truncate(60000);
                r.push_str("\n...");
            }
            panic!("{what}: {} of {} checks differ from the JavaScript:{r}", self.run - self.passed, self.run);
        }
    }
}
