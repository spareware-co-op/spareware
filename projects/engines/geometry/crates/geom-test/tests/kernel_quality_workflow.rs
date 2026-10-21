//! `.github/workflows/kernel-quality.yml` keeps the promises its header makes (piece G21).
//!
//! The gates themselves are tested in `quality_gates.rs`; this file checks the workflow that runs
//! them, line by line, for what a reader would otherwise have to check by eye:
//!
//! - **Triggers.** The diff mutation run and coverage on pull requests that touch the kernel, the
//!   full sharded mutation run, coverage and Miri weekly and on demand; and the path filters cover
//!   everything the gates read (the kernel's folder with the ledger, `.cargo/` with
//!   `mutants.toml`, the workspace manifests, the toolchain file and the workflow itself).
//! - **Least privilege.** One permission, `contents: read`, and no credentials kept by checkout.
//! - **Pins.** Every action pinned to a full commit, the same commit as `kernel.yml` where both use
//!   an action; one `RUST_TOOLCHAIN`, equal to `rust-toolchain.toml`; the nightly pinned by its
//!   date; cargo-mutants and cargo-llvm-cov installed at exact versions with `--locked`.
//! - **Shards.** Each sharded job's matrix lists exactly shards 0 to n − 1 for its `SHARDS: n`, so no
//!   shard is lost or run twice.
//! - **Every gate is judged.** The judges run in their own steps, the diff and full runs with the
//!   list of every expected mutant, and cargo-mutants' exit status is passed on unless it is one the
//!   judge decides (2, survivors; 3, timeouts).
//!
//! Method: a line scan of the workflow, as the scaffold check reads the kernel workflows'
//! toolchain pins (`projects/engines/crates/engine-geometry/tests/scaffold/toolchain.rs`).

// The crate denies native arithmetic in kernel code (Cargo.toml); tests may use it.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "indentation counts; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .to_path_buf()
}

/// A repository file's text, with Windows line ends (a checkout with `core.autocrlf`) read as
/// plain newlines, so the line scans below see the file as Git stores it.
fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel))
        .unwrap()
        .replace("\r\n", "\n")
}

fn workflow() -> String {
    read(".github/workflows/kernel-quality.yml")
}

/// Every `uses:` line's action and the commit it is pinned to.
fn action_pins(yml: &str) -> BTreeMap<String, Vec<String>> {
    let mut pins: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in yml.lines().map(str::trim) {
        let Some(rest) = line
            .strip_prefix("- uses:")
            .or_else(|| line.strip_prefix("uses:"))
        else {
            continue;
        };
        let spec = rest.split('#').next().unwrap().trim();
        let (action, commit) = spec.split_once('@').expect("an action without a pin");
        pins.entry(action.to_owned())
            .or_default()
            .push(commit.to_owned());
    }
    pins
}

/// The value of the first `key:` line in `text`, unquoted.
fn value(text: &str, key: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix(&format!("{key}:")))
        .map(|v| {
            v.split('#')
                .next()
                .unwrap()
                .trim()
                .trim_matches('"')
                .to_owned()
        })
}

/// The workflow's jobs, as (name, text of the job).
fn jobs(yml: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut in_jobs = false;
    for line in yml.lines() {
        if line == "jobs:" {
            in_jobs = true;
            continue;
        }
        if !in_jobs {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent == 2 && line.trim_end().ends_with(':') && !line.trim().starts_with('#') {
            out.push((line.trim().trim_end_matches(':').to_owned(), String::new()));
        } else if let Some((_, text)) = out.last_mut() {
            text.push_str(line);
            text.push('\n');
        }
    }
    out
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn it_runs_on_kernel_pull_requests_weekly_and_on_demand() {
    let yml = workflow();
    let on = yml.split("\npermissions:").next().unwrap();
    assert!(on.contains("\n  pull_request:\n    paths:\n"), "{on}");
    assert!(on.contains("\n  schedule:\n"), "{on}");
    assert!(on.contains("\n  workflow_dispatch:"), "{on}");
    assert!(
        !on.contains("\n  push:"),
        "pushes run kernel.yml, not the quality gates"
    );
    let cron = value(on, "- cron").unwrap();
    let fields: Vec<&str> = cron.split_whitespace().collect();
    assert_eq!(fields.len(), 5, "{cron}");
    assert!(
        fields[2] == "*" && fields[3] == "*" && fields[4] != "*",
        "weekly: {cron}"
    );
    for path in [
        "\"projects/engines/geometry/**\"",
        "\".cargo/**\"",
        "\"Cargo.toml\"",
        "\"Cargo.lock\"",
        "\"rust-toolchain.toml\"",
        "\".github/workflows/kernel-quality.yml\"",
    ] {
        assert!(
            on.contains(&format!("      - {path}\n")),
            "no path filter {path}"
        );
    }
    assert!(geom_test::ledger::LEDGER_PATH.starts_with("projects/engines/geometry/"));
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn each_job_runs_on_the_events_its_gate_names() {
    let jobs = jobs(&workflow());
    let names: Vec<&str> = jobs.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "mutants-diff",
            "mutants-diff-judge",
            "mutants-full",
            "mutants-full-judge",
            "coverage",
            "miri-plan",
            "miri",
        ]
    );
    let condition = |job: &str| {
        let text = &jobs.iter().find(|(n, _)| n == job).unwrap().1;
        // The job's own condition, at the job's indentation, not a step's.
        text.lines()
            .find_map(|l| l.strip_prefix("    if: "))
            .map(str::to_owned)
    };
    assert_eq!(
        condition("mutants-diff").unwrap(),
        "github.event_name == 'pull_request'"
    );
    assert!(
        condition("mutants-diff-judge")
            .unwrap()
            .contains("github.event_name == 'pull_request'")
    );
    for job in ["mutants-full", "miri-plan", "miri"] {
        assert_eq!(
            condition(job).unwrap(),
            "github.event_name != 'pull_request'",
            "{job}"
        );
    }
    assert!(
        condition("mutants-full-judge")
            .unwrap()
            .contains("github.event_name != 'pull_request'")
    );
    // Coverage runs on every event: pull requests, the schedule and on demand.
    assert_eq!(condition("coverage"), None);
    // The judges run even when a shard failed, so a lost shard is reported, not skipped.
    for judge in ["mutants-diff-judge", "mutants-full-judge"] {
        assert!(
            condition(judge).unwrap().starts_with("${{ !cancelled() &&"),
            "{judge}"
        );
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn permissions_are_read_only_and_credentials_are_not_kept() {
    let yml = workflow();
    let block: Vec<&str> = yml
        .split("\npermissions:\n")
        .nth(1)
        .unwrap()
        .lines()
        .take_while(|l| l.starts_with("  "))
        .collect();
    assert_eq!(block, ["  contents: read"]);
    assert_eq!(yml.matches("permissions:").count(), 1);
    assert!(!yml.contains(": write"), "no job may write");
    let checkouts = yml.matches("uses: actions/checkout@").count();
    assert_eq!(yml.matches("persist-credentials: false").count(), checkouts);
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn actions_are_pinned_to_the_commits_kernel_yml_uses() {
    let pins = action_pins(&workflow());
    let kernel = action_pins(&read(".github/workflows/kernel.yml"));
    assert!(pins.len() >= 5, "{pins:?}");
    for (action, commits) in &pins {
        for c in commits {
            assert!(
                c.len() == 40 && c.chars().all(|ch| ch.is_ascii_hexdigit()),
                "{action}@{c} is not a full commit"
            );
        }
        if let Some(theirs) = kernel.get(action) {
            assert!(
                commits.iter().all(|c| theirs.contains(c)),
                "{action}: {commits:?} vs {theirs:?}"
            );
        }
    }
    // The artifact actions kernel.yml does not use, pinned to the commits of the tags beside them.
    assert_eq!(
        pins["actions/upload-artifact"][0],
        "cf430e030ddbb5b0abf93d22962f4752f3646cd9"
    );
    assert_eq!(
        pins["actions/download-artifact"][0],
        "9000827ccba6bdab643e8b6fd33ac0654aef8333"
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn toolchains_and_tools_are_pinned() {
    let yml = workflow();
    let toolchain_file = read("rust-toolchain.toml");
    let channel = toolchain_file
        .lines()
        .find_map(|l| l.trim().strip_prefix("channel = "))
        .unwrap()
        .trim_matches('"')
        .to_owned();
    let pins: Vec<&str> = yml
        .lines()
        .filter(|l| l.trim_start().starts_with("RUST_TOOLCHAIN:"))
        .collect();
    assert_eq!(pins.len(), 1, "{pins:?}");
    assert_eq!(value(&yml, "RUST_TOOLCHAIN").unwrap(), channel);
    for line in yml
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("toolchain:"))
    {
        assert_eq!(line, "toolchain: ${{ env.RUST_TOOLCHAIN }}");
    }
    // The nightly, by date, at least two weeks before the day it was pinned (2026-10-22).
    let nightly = value(&yml, "RUST_NIGHTLY").unwrap();
    let date = nightly.strip_prefix("nightly-").expect(&nightly);
    let parts: Vec<u32> = date.split('-').map(|p| p.parse().unwrap()).collect();
    assert_eq!(parts.len(), 3, "{nightly}");
    assert!(
        (parts[0], parts[1], parts[2]) <= (2026, 10, 8),
        "{nightly} is too recent"
    );
    // Steps use the nightly only through RUSTUP_TOOLCHAIN, and only that nightly or the pin.
    for line in yml
        .lines()
        .map(str::trim)
        .filter(|l| l.contains("RUSTUP_TOOLCHAIN") && !l.starts_with('#'))
    {
        assert!(
            line == "RUSTUP_TOOLCHAIN: ${{ env.RUST_NIGHTLY }}"
                || line.starts_with("RUSTUP_TOOLCHAIN=\"$RUST_TOOLCHAIN\" "),
            "{line}"
        );
    }
    for (tool, key) in [
        ("cargo-mutants", "CARGO_MUTANTS_VERSION"),
        ("cargo-llvm-cov", "CARGO_LLVM_COV_VERSION"),
    ] {
        let version = value(&yml, key).unwrap();
        let parts: Vec<&str> = version.split('.').collect();
        assert!(
            parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok()),
            "{key} = {version}"
        );
        let install = format!("cargo install --locked {tool} --version \"${key}\"");
        let installs: Vec<&str> = yml
            .lines()
            .map(str::trim)
            .filter(|l| {
                l.contains(&format!("cargo install --locked {tool}"))
                    || l.contains(&format!("install {tool}"))
            })
            .collect();
        assert!(!installs.is_empty(), "{tool} is never installed");
        for l in installs {
            assert_eq!(l, format!("run: {install}"), "{tool}");
        }
    }
    // Nothing installs a tool another way.
    assert_eq!(
        yml.matches("cargo install").count(),
        yml.matches("cargo install --locked").count()
    );
    assert!(!yml.contains("curl ") && !yml.contains("wget "));
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn every_shard_runs_once() {
    for (name, text) in jobs(&workflow()) {
        let Some(shards) = value(&text, "SHARDS") else {
            continue;
        };
        let n: usize = shards.parse().unwrap();
        let matrix = value(&text, "shard").unwrap();
        let listed: Vec<usize> = matrix
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect();
        assert_eq!(listed, (0..n).collect::<Vec<_>>(), "{name}");
        assert!(
            text.contains("--shard \"${{ matrix.shard }}/$SHARDS\""),
            "{name}"
        );
        assert!(text.contains("--no-shuffle"), "{name}");
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn every_gate_is_judged_and_tool_failures_are_not_swallowed() {
    let yml = workflow();
    for needle in [
        "kernel-quality -- mutants --mode diff \\\n            --expected \"$RUNNER_TEMP/expected.json\"",
        "kernel-quality -- mutants --mode full \\\n            --expected \"$RUNNER_TEMP/expected.json\"",
        "cargo mutants --in-diff \"$RUNNER_TEMP/pr.diff\" --list --json > \"$RUNNER_TEMP/expected.json\"",
        "cargo mutants --list --json > \"$RUNNER_TEMP/expected.json\"",
        "kernel-quality -- coverage $SCOPE",
        "kernel-quality -- affected $SCOPE",
        "kernel-quality -- miri-plan",
        "--branch",
        "-Z cross-crate-inline-threshold=never",
        "cargo miri test --locked",
    ] {
        assert!(yml.contains(needle), "the workflow does not run {needle:?}");
    }
    // cargo-mutants' own verdict goes to the judge only for survivors and timeouts.
    let runs = yml.matches("cargo mutants --").count();
    let tolerated = yml
        .matches("case \"$code\" in 0|2|3) ;; *) exit \"$code\" ;; esac")
        .count();
    assert_eq!(runs, 4, "two mutation runs and two lists");
    assert_eq!(
        tolerated, 2,
        "each mutation run passes on any other exit status"
    );
    // The coverage of each crate is written with the log the judge checks.
    assert!(yml.contains("| tee \"$RUNNER_TEMP/cov/$crate.log\""));
}
