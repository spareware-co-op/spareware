//! Support for the f4 truth tests (tests/compliance_f4_*.rs): the recorded cases with their objects in the order the file lists them
//! (the cases hold inputs whose key order the checks read), the project a case ran on, and the comparison of an answer with the
//! recorded one.
#![allow(dead_code)] // each test file uses its own part of this module

use crate::compliance_support::{check, fixture, set_path};
use crate::support::read_truth;
use construct_transport::compliance::jv::{Jx, V};

/// An expected file (expected/compliance-<name>.json), read with ordered objects.
pub fn load(name: &str) -> V {
    V::parse(&read_truth(&format!("expected/compliance-{name}.json"))).unwrap_or_else(|e| panic!("compliance-{name}: {e}"))
}

/// Applies a case's sets (dotted path to value, in the order the case lists them) to a project.
pub fn apply(p: &mut V, sets: &V) {
    for (k, v) in sets.entries() {
        set_path(p, k, v.clone());
    }
}

/// The project a case ran on: its fixture with its sets applied.
pub fn project(case: &V) -> V {
    let mut p = fixture(&case.g("fixture").s());
    apply(&mut p, case.g("sets"));
    p
}

/// The clock the prototype's harness fixed: 21 October 2026, UTC midnight.
pub fn set_clock() {
    construct_transport::compliance::dates::set_clock(Some(1792540800000.0));
}

/// Panics unless an answer matches the case's recorded `ok` (numbers within the comparator's tolerance, text exactly).
pub fn check_ok(case: &V, got: &V) {
    let id = case.g("id").s();
    assert!(case.has("ok"), "case {id} has no recorded answer: {}", case.g("error").s());
    check(&id, got, &case.g("ok").to_value());
}

/// Panics unless a refusal matches the case's recorded error text.
pub fn check_error(case: &V, got: &str) {
    assert_eq!(got, case.g("error").s(), "case {}", case.g("id").s());
}

/// A recorded moment or day as the checks take it: null stays null (the clock), text and numbers as given.
pub fn moment(v: &V) -> V {
    v.clone()
}
