//! The ledger of this workspace: the committed `projects/engines/geometry/PROVENANCE.md` and
//! `NOTICE` are what the headers, the licence texts and the method notes give, and the places
//! that describe the process name what the code checks. CI runs the same check as its own step
//! (`cargo run -p geom-test --bin provenance -- check`, in `.github/workflows/kernel-dev.yml`);
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

use std::fs;
use std::path::PathBuf;

use geom_test::ledger::methods::{FIELDS, SECTIONS};
use geom_test::ledger::notices::KEYS;
use geom_test::ledger::{NOTICES_DIR, repo};
use geom_test::map::provenance::METHODS_FOLDER;
use geom_test::map::repo::workspace_root;

fn root() -> PathBuf {
    workspace_root().unwrap()
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel))
        .unwrap()
        .replace("\r\n", "\n")
}

#[test]
fn the_committed_ledger_is_current() {
    let problems = repo::check(&root()).unwrap();
    let report: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert!(
        problems.is_empty(),
        "{} problem(s); fix them, then run `cargo run -p geom-test --bin provenance -- write`:\n{}",
        problems.len(),
        report.join("\n")
    );
}

#[test]
fn the_folders_describe_every_file_they_hold() {
    for (folder, skip) in [
        (METHODS_FOLDER, ["README.md", "TEMPLATE.md"]),
        (NOTICES_DIR, ["README.md", "README.md"]),
    ] {
        let readme = read(&format!("{folder}README.md"));
        let files = repo::files_in(&root(), folder).unwrap();
        assert!(!files.is_empty(), "{folder} is empty");
        for f in files {
            let name = f.path.rsplit('/').next().unwrap();
            if skip.contains(&name) {
                continue;
            }
            assert!(
                readme.contains(&format!("[{name}]({name})")),
                "{folder}README.md does not list {name}"
            );
        }
    }
}

#[test]
fn the_readmes_name_what_the_checks_read() {
    let methods = read(&format!("{METHODS_FOLDER}README.md"));
    let template = read(&format!("{METHODS_FOLDER}TEMPLATE.md"));
    for field in FIELDS {
        assert!(
            template.contains(&format!("| {field} | <")),
            "TEMPLATE.md lacks the field {field}"
        );
    }
    for section in SECTIONS {
        assert!(
            template.contains(&format!("## {section}\n")),
            "TEMPLATE.md lacks the section {section}"
        );
    }
    for words in ["method only", "owner only", "full commit", "code block", "Method note:"] {
        assert!(methods.contains(words), "methods/README.md does not say {words:?}");
    }
    let notices = read(&format!("{NOTICES_DIR}README.md"));
    for key in KEYS {
        assert!(
            notices.contains(&format!("{key}: <")),
            "notices/README.md does not show the key {key}"
        );
    }
    assert!(notices.contains("Licence text:\n<") && notices.contains("Credit:\n<"));
}

#[test]
fn ci_regenerates_and_compares_the_ledger() {
    let workflow = read(".github/workflows/kernel-dev.yml");
    assert!(
        workflow.contains("run: cargo run --locked -p geom-test --bin provenance -- check"),
        "kernel-dev.yml does not run the ledger's check"
    );
    for path in [
        "\"projects/engines/geometry/**\"",
        "\"projects/engines/crates/**\"",
    ] {
        assert!(workflow.contains(path), "kernel-dev.yml does not run on {path}");
    }
    let grammar = read("projects/engines/geometry/HEADERS.md");
    assert!(
        grammar.contains("cargo run -p geom-test --bin provenance -- write"),
        "HEADERS.md does not say how the ledger is written from the records"
    );
}

#[test]
fn the_pull_request_template_asks_for_outside_sources_and_sessions() {
    let template = read(".github/pull_request_template.md");
    assert!(
        template.contains(
            "| Source | Pinned link | Licence | Class (29.1) | Files affected | What was taken |"
        ),
        "the template has no outside-sources table in chapter 29.20 rule 5's columns"
    );
    for words in [
        "## Session attestation",
        "method note",
        "was not in its context",
        "second reviewer",
        "copied expression",
        "source-scan -- check",
        "provenance -- check",
    ] {
        assert!(template.contains(words), "the template does not say {words:?}");
    }
}
