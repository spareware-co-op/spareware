//! The naming and reference fixtures (piece G141, chapter 28.3): every stored reference of every
//! parametric model resolves as expected after each upstream edit, and the names are the same on
//! every target of the determinism matrix.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "tests: a failure stops the test"
)]

use geom_test::golden::{self, hex, sha256};
use geom_test::naming::{fixtures, misses, report};

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
