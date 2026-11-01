//! The trigger finder against the JavaScript's recorded answers (expected/compliance-f1-trigger.json and, for the ANOP criterion,
//! compliance-f1-trigger-anop.json, from tools/transport-truth/gen-compliance-f1.mjs). Each case steps the build-out and runs the assignment,
//! so these are the slowest tests of the module.

mod compliance_f1_support;
mod compliance_support;
mod support;

use compliance_f1_support::{args, obj, ok_of};
use compliance_support::{check, variant};
use construct_transport::compliance::dates::set_clock;
use construct_transport::compliance::extra::{ctx, mitigation};
use construct_transport::compliance::jv::{Jx, V};

/// Runs every case of a recorded file: the finder's answer and whether it was kept in the project's record.
fn run_file(name: &str, at_least: usize) {
    set_clock(Some(1792540800000.0));
    let all = compliance_support::expected(name);
    let cases = all["cases"].as_array().expect("cases");
    assert!(cases.len() >= at_least, "{} cases", cases.len());
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert_eq!(case["call"].as_str(), Some("triggerFinder"), "{id}");
        let mut p = variant(case["fixture"].as_str().unwrap(), &case["sets"]);
        let opts = args(case).g("opts").clone();
        let r = mitigation::trigger_finder(&mut p, &opts);
        let kept = ctx::data(&p).g("triggers").clone();
        let state = if kept.nullish() {
            "none"
        } else if kept.stringify() == r.stringify() {
            "same"
        } else {
            "other"
        };
        check(
            id,
            &obj(vec![("r", r), ("saved", V::String(state.to_string()))]),
            ok_of(case),
        );
    }
}

#[test]
fn trigger_finder() {
    run_file("f1-trigger", 30);
}

#[test]
fn trigger_finder_anop_criterion() {
    run_file("f1-trigger-anop", 10);
}
