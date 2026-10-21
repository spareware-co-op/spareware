//! The map of this workspace: every crate and module header under
//! `projects/engines/geometry/crates` (and the facade's) is complete, and the committed
//! `MAP.json` is current. CI runs the same check as its own step (`cargo run -p geom-test --bin
//! map -- check`, in `.github/workflows/kernel-dev.yml`); this test makes `cargo test` catch it
//! too. It also holds the written grammar, `projects/engines/geometry/HEADERS.md`, to the code:
//! every rule, label, placeholder, licence class, family, code extension, code host, limit, known
//! gap and pending folder the code knows is named there, so the page cannot drift from the check.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![cfg(not(target_family = "wasm"))]

use geom_test::map::classes::{ALLOWED, GPL, LGPL, NEVER, READABLE};
use geom_test::map::header::{
    DEVELOPMENT_SUMMARY, FACADE_SUMMARY, FAMILIES, FAMILY_CRATES, GUARANTEES_SUBJECTS,
    INVARIANTS_LABELS, JOB_LABELS, KERNEL_SUMMARY, LAST_CHAPTER, LAST_LAYER, METHOD_LABELS,
    PLACEHOLDERS, RECORD_PREFIXES, SECTIONS_LABELS,
};
use geom_test::map::links::{
    CODE_EXTENSIONS, FORGE_HOST_PREFIXES, FORGE_HOST_SUFFIXES, FORGE_HOSTS, REPO_LINK_ROOTS, VIEWS,
};
use geom_test::map::provenance::{CHANGED, COPYRIGHT_OPENINGS, METHOD_NOTE, METHODS_FOLDER};
use geom_test::map::repo::MAX_DEPTH;
use geom_test::map::{GAPS, GRAMMAR_FILE, PENDING, Rule, repo};

#[test]
fn every_header_is_complete_and_the_map_is_current() {
    let root = repo::workspace_root().unwrap();
    let problems = repo::check(&root, GAPS).unwrap();
    let report: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert!(
        problems.is_empty(),
        "{} problem(s); fix the headers, then run `cargo run -p geom-test --bin map -- write`:\n{}",
        problems.len(),
        report.join("\n")
    );
    let input = repo::read(&root).unwrap();
    let files = input
        .crates
        .iter()
        .fold(0_usize, |n, c| n.saturating_add(c.files.len()));
    println!(
        "{} crates and {files} source files mapped; {} known gap(s)",
        input.crates.len(),
        GAPS.len()
    );
    assert!(input.crates.len() > 40, "the scan found too few crates");
    assert!(
        input.crates.iter().any(|c| c.name == "engine-geometry"),
        "the scan did not find the facade"
    );
}

fn grammar() -> String {
    let root = repo::workspace_root().unwrap();
    std::fs::read_to_string(root.join(GRAMMAR_FILE))
        .unwrap()
        .replace("\r\n", "\n")
}

#[test]
fn the_grammar_names_every_rule_label_and_family() {
    let grammar = grammar();
    let mut missing = Vec::new();
    let mut quoted = |text: &str| {
        let q = format!("`{text}`");
        if !grammar.contains(&q) {
            missing.push(q);
        }
    };
    for label in JOB_LABELS.iter().chain(&SECTIONS_LABELS) {
        quoted(label);
    }
    for family in FAMILIES {
        quoted(family);
    }
    for label in INVARIANTS_LABELS.iter().chain(&METHOD_LABELS) {
        quoted(label);
    }
    for subject in GUARANTEES_SUBJECTS {
        quoted(&format!("What {subject} guarantees"));
    }
    for word in PLACEHOLDERS {
        quoted(word);
    }
    for prefix in
        RECORD_PREFIXES
            .iter()
            .chain(&[KERNEL_SUMMARY, DEVELOPMENT_SUMMARY, FACADE_SUMMARY])
    {
        quoted(prefix);
    }
    for text in [METHOD_NOTE, METHODS_FOLDER, CHANGED] {
        quoted(text);
    }
    for rule in Rule::ALL {
        let row = format!("| `{}` |", rule.label());
        if !grammar.contains(&row) {
            missing.push(row);
        }
    }
    assert!(missing.is_empty(), "HEADERS.md does not name: {missing:?}");
}

#[test]
fn the_grammar_names_every_table_and_limit_the_code_uses() {
    let grammar = grammar();
    let lower = grammar.to_lowercase();
    let mut missing = Vec::new();
    let mut quoted = |text: &str| {
        let q = format!("`{text}`");
        if !grammar.contains(&q) {
            missing.push(q);
        }
    };
    for id in ALLOWED
        .iter()
        .chain(&READABLE)
        .chain(&LGPL)
        .chain(&GPL)
        .chain(&NEVER)
    {
        quoted(id);
    }
    for ext in CODE_EXTENSIONS {
        quoted(&format!(".{ext}"));
    }
    for host in FORGE_HOSTS
        .iter()
        .chain(&FORGE_HOST_PREFIXES)
        .chain(&FORGE_HOST_SUFFIXES)
        .chain(&VIEWS)
        .chain(&REPO_LINK_ROOTS)
    {
        quoted(host);
    }
    for gap in GAPS {
        quoted(gap.file);
    }
    for pending in PENDING {
        quoted(pending.folder);
    }
    for opening in COPYRIGHT_OPENINGS {
        if !lower.contains(&format!("`{opening}`")) {
            missing.push(opening.to_owned());
        }
    }
    for (krate, families) in FAMILY_CRATES {
        let row = format!("| `{krate}` | {} |", families.join(" and "));
        if !grammar.contains(&row) {
            missing.push(row);
        }
    }
    for limit in [
        format!("from 1 to {LAST_LAYER}"),
        format!("from 1 to {LAST_CHAPTER}"),
        format!("more than {MAX_DEPTH} folders below"),
    ] {
        if !grammar.contains(&limit) {
            missing.push(limit);
        }
    }
    assert!(missing.is_empty(), "HEADERS.md does not name: {missing:?}");
}
