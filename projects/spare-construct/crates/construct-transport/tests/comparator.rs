//! The truth-test comparator itself (tests/support): tolerances, ignored keys, non-finite numbers and its report.

mod support;

use serde_json::json;
use support::{PROJECTS, assert_close, diff_json, load_expected, load_input};

#[test]
fn equal_within_tolerance() {
    let e = json!({ "a": 1.0, "b": [1e9, 0.0, "x", true, null], "c": { "d": "Infinity", "e": "-Infinity", "f": "NaN" }, "ts": 1, "ms": 2 });
    let a = json!({ "a": 1.0000000001, "b": [1e9 + 0.5, 1e-10, "x", true, null], "c": { "d": "Infinity", "e": "-Infinity", "f": "NaN" }, "ts": 99 });
    assert_close(&a, &e, "$");
    assert_close(&json!(2), &json!(2.0), "$");
}

#[test]
fn reports_every_difference_with_its_path() {
    let e = json!({ "n": 1.0, "s": "a", "b": true, "arr": [1, 2, 3], "o": { "k": 1 }, "inf": "Infinity", "gone": 1, "Main Rd:ab": 5 });
    let a = json!({ "n": 1.001, "s": "b", "b": false, "arr": [1, 2], "o": { "k": "1" }, "inf": 1e308, "extra": 0, "Main Rd:ab": 5.0 });
    let (diffs, total) = diff_json(&a, &e, "$");
    let paths: Vec<&str> = diffs.iter().map(|d| d.path.as_str()).collect();
    assert_eq!(total, 8, "{diffs:#?}");
    for p in [
        "$.n",
        "$.s",
        "$.b",
        "$.arr.length",
        "$.o.k",
        "$.inf",
        "$.gone",
        "$.extra",
    ] {
        assert!(paths.contains(&p), "{p} not reported in {paths:?}");
    }
    let msg = std::panic::catch_unwind(|| assert_close(&a, &e, "$")).expect_err("must fail");
    let msg = msg.downcast_ref::<String>().expect("message");
    assert!(
        msg.starts_with("8 difference(s)") && msg.contains("$.n") && msg.contains("expected 1.0"),
        "{msg}"
    );
}

#[test]
fn reports_at_most_thirty() {
    let e = json!((0..100).collect::<Vec<i32>>());
    let a = json!((0..100).map(|i| i + 1).collect::<Vec<i32>>());
    let (diffs, total) = diff_json(&a, &e, "$");
    assert_eq!((diffs.len(), total), (30, 100));
}

#[test]
fn truth_files_load() {
    for name in PROJECTS {
        let e = load_expected(name);
        assert_eq!(e["project"], name);
        assert!(e["scenarios"].as_object().is_some_and(|s| !s.is_empty()));
        assert!(load_input(name)["scenarios"].is_array());
        assert_close(&e, &e, name);
    }
}
