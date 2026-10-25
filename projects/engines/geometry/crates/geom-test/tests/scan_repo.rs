//! The banned-source scan of this workspace: no marker in any file of the kernel's crates or the
//! facade's, outside the scanner's own named files. CI runs the same check as its own step
//! (`cargo run -p geom-test --bin source-scan -- check`, in `.github/workflows/kernel-dev.yml`);
//! this test makes `cargo test` catch it too.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![cfg(not(target_family = "wasm"))]

use std::collections::BTreeSet;

use geom_test::map::repo::workspace_root;
use geom_test::scan::markers;
use geom_test::scan::repo::{self, EXEMPT};

#[test]
fn no_banned_source_marker_in_the_kernel() {
    let root = workspace_root().unwrap();
    let findings = repo::check(&root, EXEMPT).unwrap();
    let report: Vec<String> = findings.iter().map(ToString::to_string).collect();
    assert!(
        findings.is_empty(),
        "{} finding(s):\n{}",
        findings.len(),
        report.join("\n")
    );
    let files = repo::files(&root).unwrap();
    let crates: BTreeSet<&str> = files
        .iter()
        .filter_map(|(path, _)| path.split('/').nth(4))
        .collect();
    assert!(
        crates.len() > 40,
        "the scan found too few crates: {crates:?}"
    );
    for wanted in [
        "projects/engines/geometry/crates/geom-num/src/math/exp.rs",
        "projects/engines/geometry/crates/geom-num/Cargo.toml",
        "projects/engines/geometry/crates/geom-test/tests/scan/cash_karp.rs",
        "projects/engines/crates/engine-geometry/src/lib.rs",
    ] {
        assert!(
            files.iter().any(|(path, _)| path == wanted),
            "the scan does not read {wanted}"
        );
    }
    println!("{} files scanned, {} exempt", files.len(), EXEMPT.len());
}

#[test]
fn the_exemptions_are_the_scanner_s_own_files() {
    for e in EXEMPT {
        assert!(
            e.file
                .starts_with("projects/engines/geometry/crates/geom-test/"),
            "{} is not one of the scanner's own files",
            e.file
        );
        assert_ne!(e.reason, "");
    }
    let unique: BTreeSet<&str> = EXEMPT.iter().map(|e| e.file).collect();
    assert_eq!(unique.len(), EXEMPT.len(), "an exemption is listed twice");
}

#[test]
fn the_marker_tables_hold_no_duplicates_and_no_blank() {
    let mut all: Vec<String> = Vec::new();
    all.extend(markers::NR_ROUTINES.iter().map(|m| m.to_lowercase()));
    all.extend(markers::NR_MACROS.iter().map(|m| (*m).to_owned()));
    all.extend(markers::NR_VARIABLES.iter().map(|m| (*m).to_owned()));
    all.extend(markers::ACAD_PATTERNS.iter().map(|m| m.to_lowercase()));
    let unique: BTreeSet<&String> = all.iter().collect();
    assert_eq!(unique.len(), all.len(), "a marker is listed twice");
    assert!(all.iter().all(|m| !m.trim().is_empty()));
    for phrase in markers::NR_PHRASES
        .iter()
        .chain(&markers::COPYLEFT_NOTICES)
        .chain(&markers::TAKING)
        .chain(&markers::COPYLEFT_SOURCES)
    {
        assert_ne!(phrase.len(), 0);
        for word in *phrase {
            assert_eq!(
                *word,
                word.to_lowercase(),
                "phrases are matched lower-cased, so each word is written in lower case"
            );
            assert!(
                word.chars().all(|c| c.is_alphanumeric() || c == '_'),
                "a phrase word is one word: {word:?}"
            );
        }
    }
    for phrase in markers::TAKING {
        assert!(
            (1..=2).contains(&phrase.len()),
            "a taking phrase is one or two words: {phrase:?}"
        );
    }
}

#[test]
fn ci_runs_the_scan() {
    let root = workspace_root().unwrap();
    let workflow = std::fs::read_to_string(root.join(".github/workflows/kernel-dev.yml")).unwrap();
    assert!(
        workflow.contains("run: cargo run --locked -p geom-test --bin source-scan -- check"),
        "kernel-dev.yml does not run the banned-source scan"
    );
}
