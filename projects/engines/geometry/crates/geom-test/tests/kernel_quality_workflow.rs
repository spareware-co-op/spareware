//! `.github/workflows/kernel-quality.yml`, `.cargo/mutants.toml` and `.config/nextest.toml` keep
//! the promises their headers make (piece G21).
//!
//! The gates themselves are tested in `quality_gates.rs` and, with the real tools, in
//! `seeded_gates.rs`; this file checks the configuration that runs them, for what a reader would
//! otherwise have to check by eye. The workflow is read as YAML (yaml-rust2) and the two
//! configuration files as TOML (toml), so a check sees the structure GitHub and the tools see, not
//! a line that looks right:
//!
//! - **Triggers.** Pull requests that touch the kernel, the schedule (weekly) and on demand, never
//!   a push; and the path filters cover everything the gates read.
//! - **Jobs and when they run.** The mutation plan, shards and judge on every event (the plan picks
//!   the diff or the full run), the shards only when the plan has mutants, the judge even when a
//!   shard failed; coverage and the seeded gates on every event; Miri weekly and on demand, and on
//!   a pull request only when it changes how Miri runs.
//! - **Least privilege.** One permission, `contents: read`, and no credentials kept by checkout.
//! - **Pins.** Every action pinned to a full commit, the same commit as `kernel.yml` where both use
//!   an action; one `RUST_TOOLCHAIN`, equal to `rust-toolchain.toml`; the nightly pinned by its
//!   date; cargo-mutants, cargo-llvm-cov and cargo-nextest installed at exact versions with
//!   `--locked`, and nothing installed another way.
//! - **Shards and partitions.** The mutation shards come from the plan, and each shard runs
//!   `--shard k/n` with the plan's count; Miri's partitions are 1 to n for its `PARTITIONS: n`.
//! - **Every gate is judged.** The judge reads the plan's list; cargo-mutants' exit status is passed
//!   on unless it is one the judge decides (2, survivors; 3, timeouts); coverage keeps each run's
//!   log for the judge.
//! - **The tools' configuration.** cargo-mutants mutates only kernel crates' sources and judges
//!   them by integration tests, with `--locked`; nextest's Miri profiles stop a test at its budget
//!   as a pass and never fail fast, and the profile the workflow names exists.
//! - **One aliasing exception.** Miri runs geom-pool, the only crate that names rayon, under Tree
//!   Borrows and every other crate under Stacked Borrows, and only while `Cargo.lock` holds
//!   crossbeam-epoch 0.9.21, whose `IsElement::element_of` Stacked Borrows refuses.

// The crate denies native arithmetic in kernel code (Cargo.toml); tests may use it.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "counts and indices; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use yaml_rust2::{Yaml, YamlLoader};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .to_path_buf()
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap()
}

fn yaml(rel: &str) -> Yaml {
    let mut docs = YamlLoader::load_from_str(&read(rel)).unwrap();
    assert_eq!(docs.len(), 1, "{rel} holds one document");
    docs.remove(0)
}

fn workflow() -> Yaml {
    yaml(".github/workflows/kernel-quality.yml")
}

fn toml_file(rel: &str) -> toml::Table {
    read(rel).parse::<toml::Table>().unwrap()
}

/// A mapping's string keys, in file order.
fn keys(y: &Yaml) -> Vec<String> {
    y.as_hash()
        .unwrap()
        .keys()
        .map(|k| k.as_str().unwrap().to_owned())
        .collect()
}

fn text(y: &Yaml) -> &str {
    y.as_str().unwrap_or_else(|| panic!("not a string: {y:?}"))
}

/// Every step of every job, as (job, step).
fn steps(w: &Yaml) -> Vec<(String, Yaml)> {
    let mut out = Vec::new();
    for job in keys(&w["jobs"]) {
        for step in w["jobs"][job.as_str()]["steps"].as_vec().unwrap() {
            out.push((job.clone(), step.clone()));
        }
    }
    out
}

/// Every `uses:` of a workflow, as action and pinned revision.
fn action_pins(w: &Yaml) -> BTreeMap<String, Vec<String>> {
    let mut pins: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (_, step) in steps(w) {
        if let Some(uses) = step["uses"].as_str() {
            let (action, rev) = uses.split_once('@').expect("an action without a pin");
            pins.entry(action.to_owned())
                .or_default()
                .push(rev.to_owned());
        }
    }
    pins
}

/// The `run:` scripts of a job, joined.
fn runs(w: &Yaml, job: &str) -> String {
    w["jobs"][job]["steps"]
        .as_vec()
        .unwrap()
        .iter()
        .filter_map(|s| s["run"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn it_runs_on_kernel_pull_requests_weekly_and_on_demand() {
    let w = workflow();
    let on = &w["on"];
    assert_eq!(
        keys(on),
        ["pull_request", "schedule", "workflow_dispatch"],
        "pushes run kernel.yml, not the quality gates"
    );
    let cron = text(&on["schedule"][0]["cron"]);
    let fields: Vec<&str> = cron.split_whitespace().collect();
    assert_eq!(fields.len(), 5, "{cron}");
    assert!(
        fields[2] == "*" && fields[3] == "*" && fields[4] != "*",
        "weekly: {cron}"
    );
    let paths: Vec<&str> = on["pull_request"]["paths"]
        .as_vec()
        .unwrap()
        .iter()
        .map(text)
        .collect();
    for path in [
        "projects/engines/geometry/**",
        ".cargo/**",
        ".config/nextest.toml",
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        ".github/workflows/kernel-quality.yml",
    ] {
        assert!(paths.contains(&path), "no path filter {path}");
    }
    assert!(geom_test::quality_ledger::LEDGER_PATH.starts_with("projects/engines/geometry/"));
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn each_job_runs_on_the_events_its_gate_names() {
    let w = workflow();
    let jobs = &w["jobs"];
    assert_eq!(
        keys(jobs),
        [
            "mutants-plan",
            "mutants",
            "mutants-judge",
            "coverage",
            "gates-seeded",
            "miri",
        ]
    );
    // Every job runs on every event, but the shards only when the plan found mutants, and the
    // judge even when a shard failed, so a lost shard is reported, not skipped.
    for job in ["mutants-plan", "coverage", "gates-seeded", "miri"] {
        assert!(jobs[job]["if"].is_badvalue(), "{job} has a condition");
    }
    assert_eq!(
        text(&jobs["mutants"]["if"]),
        "needs.mutants-plan.outputs.count != '0'"
    );
    assert_eq!(text(&jobs["mutants-judge"]["if"]), "${{ !cancelled() }}");
    let needs: Vec<&str> = jobs["mutants-judge"]["needs"]
        .as_vec()
        .unwrap()
        .iter()
        .map(text)
        .collect();
    assert_eq!(needs, ["mutants-plan", "mutants"]);
    // The judge checks the jobs it reads before it judges.
    let first = text(&jobs["mutants-judge"]["steps"][0]["run"]);
    assert!(first.contains("needs.mutants-plan.result"), "{first}");
    assert!(first.contains("needs.mutants.result"), "{first}");
    // The plan decides between the diff and the full run by the event.
    let plan = runs(&w, "mutants-plan");
    assert!(plan.contains("if [ \"$GITHUB_EVENT_NAME\" = pull_request ]; then"));
    // Miri: every partition weekly and on demand; one partition on a pull request, and only when
    // it changes the Miri run.
    let miri = runs(&w, "miri");
    for watched in [
        ".github/workflows/kernel-quality",
        ".config/nextest",
        "geom-test/src/miri",
    ] {
        assert!(miri.contains(watched), "{watched}");
    }
    for step in jobs["miri"]["steps"].as_vec().unwrap().iter().skip(2) {
        let condition = text(&step["if"]);
        assert!(condition.contains("env.NEEDED == 'true'"), "{condition}");
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn permissions_are_read_only_and_credentials_are_not_kept() {
    let w = workflow();
    assert_eq!(keys(&w["permissions"]), ["contents"]);
    assert_eq!(text(&w["permissions"]["contents"]), "read");
    for job in keys(&w["jobs"]) {
        assert!(
            w["jobs"][job.as_str()]["permissions"].is_badvalue(),
            "{job} sets its own permissions"
        );
    }
    let mut checkouts = 0;
    for (job, step) in steps(&w) {
        if step["uses"]
            .as_str()
            .is_some_and(|u| u.starts_with("actions/checkout@"))
        {
            checkouts += 1;
            assert_eq!(
                step["with"]["persist-credentials"].as_bool(),
                Some(false),
                "{job}"
            );
        }
    }
    assert_eq!(checkouts, keys(&w["jobs"]).len(), "one checkout per job");
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn actions_are_pinned_to_the_commits_kernel_yml_uses() {
    let pins = action_pins(&workflow());
    let kernel = action_pins(&yaml(".github/workflows/kernel.yml"));
    assert!(pins.len() >= 5, "{pins:?}");
    for (action, revs) in &pins {
        for r in revs {
            assert!(
                r.len() == 40 && r.chars().all(|ch| ch.is_ascii_hexdigit()),
                "{action}@{r} is not a full commit"
            );
        }
        if let Some(theirs) = kernel.get(action) {
            assert!(
                revs.iter().all(|r| theirs.contains(r)),
                "{action}: {revs:?} vs {theirs:?}"
            );
        }
    }
    // The artifact actions kernel.yml does not use, pinned to the commits of their tags: v7.0.2
    // and v8.0.2.
    assert!(
        pins["actions/upload-artifact"]
            .iter()
            .all(|r| r == "cf430e030ddbb5b0abf93d22962f4752f3646cd9")
    );
    assert!(
        pins["actions/download-artifact"]
            .iter()
            .all(|r| r == "9000827ccba6bdab643e8b6fd33ac0654aef8333")
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn toolchains_and_tools_are_pinned() {
    let w = workflow();
    let channel = toml_file("rust-toolchain.toml")["toolchain"]["channel"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(text(&w["env"]["RUST_TOOLCHAIN"]), channel);
    for (job, step) in steps(&w) {
        if let Some(t) = step["with"]["toolchain"].as_str() {
            assert_eq!(t, "${{ env.RUST_TOOLCHAIN }}", "{job}");
        }
        // Steps use the nightly only through RUSTUP_TOOLCHAIN, and only that nightly.
        if let Some(t) = step["env"]["RUSTUP_TOOLCHAIN"].as_str() {
            assert_eq!(t, "${{ env.RUST_NIGHTLY }}", "{job}");
        }
        for env in keys_or_empty(&w["jobs"][job.as_str()]["env"]) {
            assert_ne!(
                env, "RUSTUP_TOOLCHAIN",
                "{job} sets the toolchain for every step"
            );
        }
    }
    // The nightly, by date, at least two weeks before the day it was pinned (2026-10-22).
    let nightly = text(&w["env"]["RUST_NIGHTLY"]);
    let date = nightly.strip_prefix("nightly-").expect(nightly);
    let parts: Vec<u32> = date.split('-').map(|p| p.parse().unwrap()).collect();
    assert_eq!(parts.len(), 3, "{nightly}");
    assert!(
        (parts[0], parts[1], parts[2]) <= (2026, 10, 8),
        "{nightly} is too recent"
    );
    let all_runs: String = keys(&w["jobs"])
        .iter()
        .map(|j| runs(&w, j))
        .collect::<Vec<_>>()
        .join("\n");
    for (tool, key) in [
        ("cargo-mutants", "CARGO_MUTANTS_VERSION"),
        ("cargo-llvm-cov", "CARGO_LLVM_COV_VERSION"),
        ("cargo-nextest", "CARGO_NEXTEST_VERSION"),
    ] {
        let version = text(&w["env"][key]);
        let parts: Vec<&str> = version.split('.').collect();
        assert!(
            parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok()),
            "{key} = {version}"
        );
        let install = format!("cargo install --locked {tool} --version \"${key}\"");
        let lines: Vec<&str> = all_runs
            .lines()
            .map(str::trim)
            .filter(|l| l.contains(tool) && l.contains("install"))
            .collect();
        assert!(!lines.is_empty(), "{tool} is never installed");
        for l in lines {
            assert_eq!(l, install, "{tool}");
        }
    }
    // The workflow's nextest is new enough for the configuration it reads.
    let nextest: Vec<u32> = text(&w["env"]["CARGO_NEXTEST_VERSION"])
        .split('.')
        .map(|p| p.parse().unwrap())
        .collect();
    let config = toml_file(".config/nextest.toml");
    let required: Vec<u32> = config["nextest-version"]["required"]
        .as_str()
        .unwrap()
        .split('.')
        .map(|p| p.parse().unwrap())
        .collect();
    assert!(nextest >= required, "{nextest:?} < {required:?}");
    // Nothing installs a tool another way.
    assert_eq!(
        all_runs.matches("cargo install").count(),
        all_runs.matches("cargo install --locked").count()
    );
    assert!(!all_runs.contains("curl ") && !all_runs.contains("wget "));
}

fn keys_or_empty(y: &Yaml) -> Vec<String> {
    if y.is_badvalue() { Vec::new() } else { keys(y) }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn every_shard_and_partition_runs_once() {
    let w = workflow();
    let shards = &w["jobs"]["mutants"];
    assert_eq!(
        text(&shards["strategy"]["matrix"]["shard"]),
        "${{ fromJSON(needs.mutants-plan.outputs.shards) }}"
    );
    assert_eq!(
        text(&shards["env"]["SHARDS"]),
        "${{ needs.mutants-plan.outputs.count }}"
    );
    assert_eq!(shards["strategy"]["fail-fast"].as_bool(), Some(false));
    let run = runs(&w, "mutants");
    assert!(
        run.contains("--shard \"${{ matrix.shard }}/$SHARDS\""),
        "{run}"
    );
    assert!(run.contains("--no-shuffle"), "{run}");
    let plan = &w["jobs"]["mutants-plan"]["outputs"];
    assert_eq!(text(&plan["count"]), "${{ steps.plan.outputs.count }}");
    assert_eq!(text(&plan["shards"]), "${{ steps.plan.outputs.shards }}");
    // Miri: the weekly matrix lists partitions 1 to PARTITIONS, and each runs its own.
    let miri = &w["jobs"]["miri"];
    let n = miri["env"]["PARTITIONS"].as_i64().unwrap();
    let matrix = text(&miri["strategy"]["matrix"]["partition"]);
    let weekly = matrix
        .split("|| '")
        .nth(1)
        .and_then(|rest| rest.split('\'').next())
        .unwrap();
    let listed: Vec<i64> = serde_json::from_str(weekly).unwrap();
    assert_eq!(listed, (1..=n).collect::<Vec<_>>());
    assert!(
        matrix.contains("github.event_name == 'pull_request' && '[1]'"),
        "{matrix}"
    );
    assert_eq!(miri["strategy"]["fail-fast"].as_bool(), Some(false));
    let run = runs(&w, "miri");
    assert!(
        run.contains("--partition \"hash:${{ matrix.partition }}/$PARTITIONS\""),
        "{run}"
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn every_gate_is_judged_and_tool_failures_are_not_swallowed() {
    let w = workflow();
    let plan = runs(&w, "mutants-plan");
    for needle in [
        "cargo mutants --in-diff \"$RUNNER_TEMP/plan/pr.diff\" --list --json > \"$RUNNER_TEMP/plan/list.json\"",
        "cargo mutants --list --json > \"$RUNNER_TEMP/plan/list.json\"",
        "kernel-quality -- mutants-plan",
        "git show \"HEAD^1:projects/engines/geometry/quality-ledger.json\"",
    ] {
        assert!(plan.contains(needle), "the plan does not run {needle:?}");
    }
    let judge = runs(&w, "mutants-judge");
    for needle in [
        "args=(--expected \"$RUNNER_TEMP/plan/list.json\")",
        "args+=(--mode diff)",
        "args+=(--mode full)",
        "args+=(--base-ledger \"$RUNNER_TEMP/plan/base-ledger.json\")",
        "kernel-quality -- mutants \"${args[@]}\" \"${outs[@]}\"",
    ] {
        assert!(judge.contains(needle), "the judge does not run {needle:?}");
    }
    // cargo-mutants' own verdict goes to the judge only for survivors and timeouts.
    let shard = runs(&w, "mutants");
    assert_eq!(shard.matches("cargo mutants ").count(), 1);
    assert!(shard.contains("case \"$code\" in 0|2|3) ;; *) exit \"$code\" ;; esac"));
    assert!(shard.contains("test -f \"$RUNNER_TEMP/out/mutants.out/mutants.json\""));
    // Coverage: measured on the nightly with branches, each run's log kept, then judged.
    let coverage = runs(&w, "coverage");
    for needle in [
        "kernel-quality -- affected $SCOPE",
        "kernel-quality -- coverage $SCOPE",
        "--branch",
        "| tee \"$RUNNER_TEMP/cov/$crate.log\"",
    ] {
        assert!(
            coverage.contains(needle),
            "coverage does not run {needle:?}"
        );
    }
    let measure = w["jobs"]["coverage"]["steps"]
        .as_vec()
        .unwrap()
        .iter()
        .find(|s| s["name"].as_str() == Some("Measure each crate with its own tests"))
        .unwrap();
    assert_eq!(
        text(&measure["env"]["RUSTFLAGS"]),
        "-Z cross-crate-inline-threshold=never -C link-dead-code"
    );
    // The seeded gates run with --ignored, on the coverage nightly.
    let seeded = runs(&w, "gates-seeded");
    assert!(
        seeded.contains("--test seeded_gates -- --ignored"),
        "{seeded}"
    );
    // Miri: every kernel crate's tests under nextest, and its doctests.
    let miri = runs(&w, "miri");
    for needle in [
        "kernel-quality -- miri-packages",
        "cargo miri nextest run --locked $PACKAGES",
        "cargo miri test --locked $PACKAGES --doc",
        "profile=(--profile miri-smoke)",
    ] {
        assert!(miri.contains(needle), "Miri does not run {needle:?}");
    }
}

/// The one crate Miri runs under Tree Borrows, and the crossbeam-epoch release that is why.
const TREE_BORROWS_CRATE: &str = "geom-pool";
const CROSSBEAM_EPOCH_WITH_THE_VIOLATION: &str = "0.9.21";

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn only_geom_pool_runs_under_tree_borrows_and_only_while_crossbeam_needs_it() {
    let w = workflow();
    let steps = w["jobs"]["miri"]["steps"].as_vec().unwrap();
    let with_flags: Vec<(&str, &str)> = steps
        .iter()
        .filter_map(|s| Some((s["env"]["MIRIFLAGS"].as_str()?, s["run"].as_str()?)))
        .collect();
    // Every Miri run but one keeps Stacked Borrows, and that run's crate leaves the main run.
    let tree: Vec<&&str> = with_flags
        .iter()
        .filter(|(flags, _)| flags.contains("-Zmiri-tree-borrows"))
        .map(|(_, run)| run)
        .collect();
    assert_eq!(tree.len(), 1, "one step runs Tree Borrows");
    assert!(
        tree[0].contains(&format!(
            "cargo miri nextest run --locked -p {TREE_BORROWS_CRATE} "
        )),
        "{}",
        tree[0]
    );
    let main = with_flags
        .iter()
        .find(|(_, run)| run.contains("cargo miri nextest run --locked $PACKAGES"))
        .unwrap()
        .1;
    assert!(
        main.contains(&format!("-E 'not package({TREE_BORROWS_CRATE})'")),
        "{main}"
    );
    // The crate is the one that may name rayon, and the exception lasts only while the lock
    // holds the crossbeam-epoch release whose `element_of` Stacked Borrows refuses
    // (crossbeam-rs/crossbeam#871 fixes it upstream, for 0.10). Once the lock moves, run
    // geom-pool under Stacked Borrows again (remove the step and the filter), or record why not.
    let manifest = toml_file(&format!(
        "projects/engines/geometry/crates/{TREE_BORROWS_CRATE}/Cargo.toml"
    ));
    assert!(
        manifest["target"]["cfg(not(target_family = \"wasm\"))"]["dependencies"]
            .get("rayon")
            .is_some(),
        "{TREE_BORROWS_CRATE} no longer names rayon"
    );
    let lock = toml_file("Cargo.lock");
    let epochs: Vec<&str> = lock["package"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["name"].as_str() == Some("crossbeam-epoch"))
        .map(|p| p["version"].as_str().unwrap())
        .collect();
    assert_eq!(
        epochs,
        [CROSSBEAM_EPOCH_WITH_THE_VIOLATION],
        "crossbeam-epoch moved: look at kernel-quality.yml's Tree Borrows step again"
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_tools_are_configured_as_the_gates_need() {
    // cargo-mutants: only the kernel crates' sources, judged by integration tests (28.8: through
    // the public interface), locked, on the release profile, the mutated crate's tests only.
    let m = toml_file(".cargo/mutants.toml");
    let strings = |key: &str| -> Vec<String> {
        m[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        strings("examine_globs"),
        ["projects/engines/geometry/crates/*/src/**/*.rs"]
    );
    let excluded = strings("exclude_globs");
    for dev in geom_test::deps::DEVELOPMENT_CRATES {
        assert!(
            excluded.contains(&format!("projects/engines/geometry/crates/{dev}/**")),
            "{dev} is mutated"
        );
    }
    // No name filter narrows CI's runs: the globs alone say what is mutated, and a local run
    // narrows itself with --re, which the file must leave free (command-line lists are added to
    // the file's).
    for key in ["examine_re", "exclude_re"] {
        assert!(m.get(key).is_none(), "{key} narrows every run");
    }
    assert_eq!(strings("additional_cargo_test_args"), ["--test", "*"]);
    assert_eq!(strings("additional_cargo_args"), ["--locked"]);
    assert_eq!(m["profile"].as_str(), Some("release"));
    assert_eq!(m["test_workspace"].as_bool(), Some(false));
    // nextest: the Miri profiles stop a test at its budget as a pass, never fail fast, and write
    // JUnit; the one the workflow names on a pull request exists.
    let n = toml_file(".config/nextest.toml");
    for profile in ["default-miri", "miri-smoke"] {
        let p = &n["profile"][profile];
        let slow = &p["slow-timeout"];
        assert_eq!(slow["on-timeout"].as_str(), Some("pass"), "{profile}");
        assert!(
            slow["terminate-after"].as_integer().unwrap() >= 1,
            "{profile}"
        );
        assert!(slow["period"].as_str().unwrap().ends_with('s'), "{profile}");
        assert_eq!(p["fail-fast"].as_bool(), Some(false), "{profile}");
        assert_eq!(p["junit"]["path"].as_str(), Some("junit.xml"), "{profile}");
    }
    // The weekly budget: two periods of five minutes, so a partition's worst case fits the job.
    let weekly = &n["profile"]["default-miri"]["slow-timeout"];
    assert_eq!(weekly["period"].as_str(), Some("300s"));
    assert_eq!(weekly["terminate-after"].as_integer(), Some(2));
    assert!(n["profile"].as_table().unwrap().get("default").is_none());
}
