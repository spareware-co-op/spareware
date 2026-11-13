//! Calibration against the JavaScript's recorded answers: matrix estimation (calibrate.js `fit`) on both truth-test
//! projects given traffic counts of every kind, with every branch of the fit (weights, count lists, count groups, limits
//! and smaller steps, nothing to fit, no improvement, earlier factors kept, clearing), the counted sites, statistics,
//! fitting units and parallel check of each prior run, and the validation report before and after.
//! Recorded by tools/transport-truth/gen-calibrate.mjs (expected/calibrate-*.json).

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly. Tests
// may use native arithmetic and casts (Cargo.toml), and as in the kernel's tests a case table may run
// long and name its values with single letters.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "test code; the rules are for the crate's own code"
)]

mod calibrate_support;
mod support;

use calibrate_support::replay_fit;
use construct_transport::datahub::Count;
use support::load_json;

fn replay_file(file: &str, expect_cases: usize) {
    let j = load_json(&format!("expected/{file}.json"));
    let cases = j["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), expect_cases, "{file}: recorded cases");
    let mut done = 0;
    for rec in cases {
        let text = rec
            .get("projectText")
            .or_else(|| j.get("projectText"))
            .and_then(|t| t.as_str())
            .expect("project text");
        if rec["weightKind"] == "caller" {
            continue; // needs the calibration panel's method weights (calibration_truth.rs)
        }
        replay_fit(rec, text, None);
        done += 1;
    }
    assert!(done >= expect_cases - 1, "{file}: replayed {done}");
}

#[test]
fn corridor_fits_as_recorded() {
    replay_file("calibrate-corridor", 12);
}

#[test]
fn grid_fits_as_recorded() {
    replay_file("calibrate-grid", 12);
}

#[test]
fn grid_with_existing_zones_fits_as_recorded() {
    replay_file("calibrate-grid-existing", 12);
}

#[test]
fn special_fits_as_recorded() {
    replay_file("calibrate-special", 21);
}

#[allow(dead_code)]
fn _count(_: &Count) {}
