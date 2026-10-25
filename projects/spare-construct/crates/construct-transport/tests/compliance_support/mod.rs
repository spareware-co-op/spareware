//! Support for the compliance truth tests (tests/compliance_*.rs): the project fixtures and expected answers that
//! tools/transport-truth/gen-compliance*.mjs write, project variants built from a fixture and a list of sets, and the
//! conversion of the port's loose JSON to the plain form the comparator reads.
//!
//! A case in an expected file names a fixture and a map of dotted paths to values (`sets`); the JavaScript applied the same sets to
//! the same project before it called the function, so the test does too ([`variant`]). The fixtures `corridor` and `grid` are the
//! saved test projects with the engine's results (fixtures/data/compliance-base-corridor.json and -base-grid.json); any other name is
//! fixtures/data/compliance-<name>.json.
#![allow(dead_code)] // each test file uses its own part of this module

use crate::support::{assert_close, load_json, read_truth};
use construct_transport::compliance::jv::V;
use serde_json::Value;

/// A project fixture (with results) as loose JSON.
pub fn fixture(name: &str) -> V {
    let file = match name {
        "corridor" | "grid" => format!("fixtures/data/compliance-base-{name}.json"),
        n => format!("fixtures/data/compliance-{n}.json"),
    };
    V::parse(&read_truth(&file)).unwrap_or_else(|e| panic!("{file}: {e}"))
}

/// `o[path] = v` for a dotted path: a step that is not there becomes an object, a step on an array is an index.
pub fn set_path(root: &mut V, path: &str, value: V) {
    let steps: Vec<&str> = path.split('.').collect();
    fn go(cur: &mut V, steps: &[&str], value: V) {
        let k = steps[0];
        if steps.len() == 1 {
            put(cur, k, value);
            return;
        }
        let need_obj = !matches!(get(cur, k), Some(x) if x.is_object_like());
        if need_obj {
            put(cur, k, V::object());
        }
        let next = get_mut(cur, k).expect("step exists");
        go(next, &steps[1..], value);
    }
    fn get<'a>(v: &'a V, k: &str) -> Option<&'a V> {
        match v {
            V::Array(a) => k.parse::<usize>().ok().and_then(|i| a.get(i)),
            _ => v.get(k),
        }
    }
    fn get_mut<'a>(v: &'a mut V, k: &str) -> Option<&'a mut V> {
        match v {
            V::Array(a) => k.parse::<usize>().ok().and_then(move |i| a.get_mut(i)),
            _ => v.get_mut(k),
        }
    }
    fn put(v: &mut V, k: &str, value: V) {
        match v {
            V::Array(a) => {
                if let Ok(i) = k.parse::<usize>() {
                    while a.len() <= i {
                        a.push(V::Null);
                    }
                    a[i] = value;
                }
            }
            _ => v.set(k, value),
        }
    }
    go(root, &steps, value);
}

/// A fixture with a case's sets applied (`sets` is a JSON object of dotted path to value, or null).
pub fn variant(fixture_name: &str, sets: &Value) -> V {
    let mut p = fixture(fixture_name);
    apply_sets(&mut p, sets);
    p
}

/// Applies a case's sets to a project.
pub fn apply_sets(p: &mut V, sets: &Value) {
    if let Value::Object(m) = sets {
        for (k, v) in m {
            set_path(p, k, V::from_value(v));
        }
    }
}

/// The port's loose JSON in the plain form the comparator reads: non-finite numbers as "NaN", "Infinity" and "-Infinity".
pub fn plain(v: &V) -> Value {
    match v {
        V::Null => Value::Null,
        V::Bool(b) => Value::Bool(*b),
        V::Number(x) => {
            if x.is_finite() {
                construct_transport::jsnum::json_number(*x)
            } else {
                Value::String(construct_transport::jsnum::number_to_js_string(*x))
            }
        }
        V::String(s) => Value::String(s.clone()),
        V::Array(a) => Value::Array(a.iter().map(plain).collect()),
        V::Object(m) => Value::Object(m.iter().map(|(k, x)| (k.to_string(), plain(x))).collect()),
    }
}

/// Panics unless the port's answer matches the recorded one (numbers within the comparator's tolerance, text exactly).
pub fn check(path: &str, actual: &V, expected: &Value) {
    assert_close(&plain(actual), expected, path);
}

/// An expected file (expected/compliance-<name>.json).
pub fn expected(name: &str) -> Value {
    load_json(&format!("expected/compliance-{name}.json"))
}

/// The `ok` answer of a recorded case; panics with the recorded error when the prototype threw (use [`case_error`] for those).
pub fn case_ok<'a>(case: &'a Value) -> &'a Value {
    case.get("ok").unwrap_or_else(|| panic!("case {} has no ok answer: {}", case["id"], case))
}

/// The error text of a recorded case the prototype refused (None when it answered).
pub fn case_error(case: &Value) -> Option<&str> {
    case.get("error").and_then(Value::as_str)
}
