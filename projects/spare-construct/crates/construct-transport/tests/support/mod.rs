//! Truth-test support: loads the prototype's recorded answers and compares JSON with a tolerance.
//!
//! tools/transport-truth holds inputs/<name>.json (saved projects after the prototype's
//! migrate()), expected/<name>.json (everything the JavaScript engine returned for them, from
//! gen-expected.mjs) and expected/util-cases.json (the helpers' answers, from
//! gen-util-cases.mjs). Use [`assert_close`] to check a port's JSON output against them.
#![allow(dead_code)] // each test file uses its own part of this module

use construct_transport::project::Project;
use serde_json::Value;
use std::fmt::Write as _;
use std::path::PathBuf;

/// The truth-test projects in tools/transport-truth/inputs.
pub const PROJECTS: [&str; 2] = ["corridor", "grid"];

/// Relative tolerance for numbers.
pub const REL_TOL: f64 = 1e-6;
/// Absolute tolerance for numbers (for values near zero).
pub const ABS_TOL: f64 = 1e-9;
/// Keys never compared: wall-clock time stamps and run times.
pub const IGNORED_KEYS: [&str; 2] = ["ts", "ms"];
/// Differences listed in a failure message.
pub const MAX_REPORTED: usize = 30;

/// tools/transport-truth, from this crate's manifest directory.
pub fn truth_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../tools/transport-truth")
}

/// Text of a file under tools/transport-truth.
pub fn read_truth(rel: &str) -> String {
    let p = truth_dir().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// JSON of a file under tools/transport-truth.
pub fn load_json(rel: &str) -> Value {
    serde_json::from_str(&read_truth(rel)).unwrap_or_else(|e| panic!("{rel} is not JSON: {e}"))
}

/// Text of inputs/<name>.json.
pub fn input_text(name: &str) -> String {
    read_truth(&format!("inputs/{name}.json"))
}

/// inputs/<name>.json as JSON.
pub fn load_input(name: &str) -> Value {
    load_json(&format!("inputs/{name}.json"))
}

/// inputs/<name>.json as the typed project.
pub fn load_project(name: &str) -> Project {
    Project::from_json(&input_text(name))
        .unwrap_or_else(|e| panic!("inputs/{name}.json does not read as a Project: {e}"))
}

/// expected/<name>.json (what the JavaScript engine returned).
pub fn load_expected(name: &str) -> Value {
    load_json(&format!("expected/{name}.json"))
}

/// One difference between actual and expected JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct Diff {
    /// Where, as a JSON path ($.scenarios["s-dm"].periods.AM.res.arcs[...]).
    pub path: String,
    /// What was expected.
    pub expected: String,
    /// What the port gave.
    pub actual: String,
}

/// A number in JSON, or the strings gen-expected.mjs writes for non-finite numbers.
pub fn as_number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => match s.as_str() {
            "NaN" => Some(f64::NAN),
            "Infinity" => Some(f64::INFINITY),
            "-Infinity" => Some(f64::NEG_INFINITY),
            _ => None,
        },
        _ => None,
    }
}

/// True when two numbers agree within [`REL_TOL`] or [`ABS_TOL`]; NaN equals NaN, infinities only themselves.
pub fn numbers_close(a: f64, e: f64) -> bool {
    if a.is_nan() || e.is_nan() {
        return a.is_nan() && e.is_nan();
    }
    if a.is_infinite() || e.is_infinite() {
        return a == e;
    }
    let d = (a - e).abs();
    d <= ABS_TOL || d <= REL_TOL * a.abs().max(e.abs())
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 160 {
        format!("{}...", s.chars().take(160).collect::<String>())
    } else {
        s
    }
}

fn key_path(path: &str, k: &str) -> String {
    let ident = !k.is_empty()
        && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !k.chars().next().unwrap().is_ascii_digit();
    if ident {
        format!("{path}.{k}")
    } else {
        format!("{path}[{k:?}]")
    }
}

struct Collector {
    diffs: Vec<Diff>,
    total: usize,
}

impl Collector {
    fn push(&mut self, path: &str, expected: String, actual: String) {
        self.total += 1;
        if self.diffs.len() < MAX_REPORTED {
            self.diffs.push(Diff {
                path: path.to_string(),
                expected,
                actual,
            });
        }
    }
    fn walk(&mut self, a: &Value, e: &Value, path: &str) {
        if let (Some(x), Some(y)) = (as_number(a), as_number(e))
            && (a.is_number() || e.is_number())
        {
            if !numbers_close(x, y) {
                self.push(path, short(e), short(a));
            }
            return;
        }
        match (a, e) {
            (Value::Object(am), Value::Object(em)) => {
                for (k, ev) in em {
                    if IGNORED_KEYS.contains(&k.as_str()) {
                        continue;
                    }
                    let p = key_path(path, k);
                    match am.get(k) {
                        Some(av) => self.walk(av, ev, &p),
                        None => self.push(&p, short(ev), "(missing)".to_string()),
                    }
                }
                for (k, av) in am {
                    if !IGNORED_KEYS.contains(&k.as_str()) && !em.contains_key(k) {
                        self.push(&key_path(path, k), "(missing)".to_string(), short(av));
                    }
                }
            }
            (Value::Array(aa), Value::Array(ea)) => {
                if aa.len() != ea.len() {
                    self.push(
                        &format!("{path}.length"),
                        ea.len().to_string(),
                        aa.len().to_string(),
                    );
                }
                for (i, (av, ev)) in aa.iter().zip(ea).enumerate() {
                    self.walk(av, ev, &format!("{path}[{i}]"));
                }
            }
            _ => {
                if a != e {
                    self.push(path, short(e), short(a));
                }
            }
        }
    }
}

/// Every difference between actual and expected (the first [`MAX_REPORTED`] in full) and the total count.
pub fn diff_json(actual: &Value, expected: &Value, path: &str) -> (Vec<Diff>, usize) {
    let mut c = Collector {
        diffs: Vec::new(),
        total: 0,
    };
    c.walk(actual, expected, path);
    (c.diffs, c.total)
}

/// Panics unless actual matches expected: numbers within [`REL_TOL`] / [`ABS_TOL`], text and booleans exactly,
/// arrays item by item, objects with the same keys (ignoring [`IGNORED_KEYS`]); "NaN", "Infinity" and
/// "-Infinity" in either equal the matching number. The message lists up to [`MAX_REPORTED`] differences.
pub fn assert_close(actual: &Value, expected: &Value, path: &str) {
    let (diffs, total) = diff_json(actual, expected, path);
    if total == 0 {
        return;
    }
    let mut msg = format!("{total} difference(s) from the JavaScript under {path}");
    if total > diffs.len() {
        let _ = write!(msg, " (first {} shown)", diffs.len());
    }
    msg.push(':');
    for d in &diffs {
        let _ = write!(
            msg,
            "\n  {}\n      expected {}\n      actual   {}",
            d.path, d.expected, d.actual
        );
    }
    panic!("{msg}");
}
