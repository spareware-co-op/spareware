//! The naming and reference fixtures (piece G141, chapter 28.3): every stored reference of every
//! parametric model resolves as expected after each upstream edit, and the names are the same on
//! every target of the determinism matrix.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "tests: a failure stops the test"
)]

use geom_test::golden::{self, hex, sha256};
use geom_test::naming::{fixtures, misses, report, scenarios};

/// The golden of every fixture's every resolution (28.13).
const GOLDEN: &str = "geom-test/naming-fixtures";

#[test]
fn every_reference_resolves_as_expected_after_every_edit() {
    let all = fixtures().unwrap();
    assert!(all.len() >= 12, "{} fixtures", all.len());
    let mut failed = Vec::new();
    for fixture in &all {
        assert!(geom_test::fixture::is_id(fixture.id), "{}", fixture.id);
        assert!(!fixture.edits.is_empty(), "{}", fixture.id);
        failed.extend(misses(fixture).unwrap());
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn the_names_are_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let text = report(&fixtures().unwrap()).unwrap();
        (sha256(text.as_bytes()), text)
    });
    println!(
        "naming fixtures: {} lines, golden hash {}",
        checked.value.lines().count(),
        hex(sha256(checked.value.as_bytes()))
    );
}

/// SolidCraft's naming scenarios in this kernel's terms (take map item 15): every reference
/// resolves as the chapter says after each edit.
#[test]
fn the_solidcraft_scenarios_resolve_as_expected() {
    let all = scenarios().unwrap();
    assert!(all.len() >= 10, "{} scenarios", all.len());
    let mut failed = Vec::new();
    for fixture in &all {
        assert!(geom_test::fixture::is_id(fixture.id), "{}", fixture.id);
        failed.extend(misses(fixture).unwrap());
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// Built twice, the scenarios resolve alike, and none shares an id with the catalogue.
#[test]
fn the_scenarios_resolve_alike_every_time() {
    let first = report(&scenarios().unwrap()).unwrap();
    let second = report(&scenarios().unwrap()).unwrap();
    assert_eq!(first, second);
    let catalogue = fixtures().unwrap();
    for scenario in scenarios().unwrap() {
        assert!(
            catalogue.iter().all(|f| f.id != scenario.id),
            "{}",
            scenario.id
        );
    }
}
