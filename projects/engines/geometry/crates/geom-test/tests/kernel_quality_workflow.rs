//! `.github/workflows/kernel-quality.yml`, `.cargo/mutants.toml` and `.config/nextest.toml` keep
//! the promises their headers make (piece G21).
//!
//! The gates themselves are tested in `quality_gates.rs` and, with the real tools, in
//! `seeded_gates.rs`; this file checks the configuration that runs them, for what a reader would
//! otherwise have to check by eye. The workflow is read as YAML (yaml-rust2) and the two
//! configuration files as TOML (toml), so a check sees the structure GitHub and the tools see, not
//! a line that looks right:
//!
//! - **Triggers.** Every pull request (once it is ready for review), GitHub's merge queue, a push
//!   to main, on demand, and when kernel-release.yml calls it on a release tag, never on a timer;
//!   and the files the job `when` watches cover everything the gates read.
//! - **Jobs and when they run.** The job `when` decides each gate; the mutation plan and judge run
//!   when it chose mutation (the plan picks the diff or the full run by its mode), each shard job
//!   only when the plan gives it shards, the judge even when a shard failed; a full run narrowed to
//!   some files, on demand or on a push to main, has its plan check the files cargo-mutants
//!   selected and its judge count their ledger entries; coverage and the seeded gates when it chose
//!   coverage; Miri when it chose Miri, on a pull request only when it changes how Miri runs. A
//!   draft runs none of them. The last job, `done`, fails unless every gate chosen ran and passed.
//! - **Least privilege.** One permission, `contents: read`, and no credentials kept by checkout.
//! - **Pins.** Every action pinned to a full commit, the same commit as `kernel.yml` where both use
//!   an action; one `RUST_TOOLCHAIN`, equal to `rust-toolchain.toml`; the nightly pinned by its
//!   date; cargo-mutants, cargo-llvm-cov and cargo-nextest installed at exact versions with
//!   `--locked`, and nothing installed another way.
//! - **Shards and partitions.** The mutation shards come from the plan, in as many jobs as
//!   `geom_test::mutants::SHARD_JOBS`, each with the same steps, and each shard runs `--shard k/n`
//!   with the plan's count of the whole run; Miri's partitions are its plan's.
//! - **Every gate is judged.** The judge reads the plan's list; cargo-mutants' exit status is passed
//!   on unless it is one the judge decides (2, survivors; 3, timeouts); coverage keeps each run's
//!   log for the judge.
//! - **The tools' configuration.** cargo-mutants mutates only kernel crates' sources and judges
//!   them by integration tests, with `--locked`; nextest's Miri profiles stop a test at its budget
//!   as a pass and never fail fast, and the profile the workflow names exists.
//! - **One aliasing exception.** Miri runs geom-pool, the only crate that names rayon, under Tree
//!   Borrows and every other crate under Stacked Borrows, and only while `Cargo.lock` holds
//!   crossbeam-epoch 0.9.21, whose `IsElement::element_of` Stacked Borrows refuses.
//! - **Miri's jobs run from one plan, each within 15 minutes.** One plan job lists the tests under
//!   Miri, fails on a test with no row, and hands every partition and geom-pool's job its
//!   filterset, the crates and the cargo-nextest it built, for one day; no other Miri job lists
//!   every test or builds a tool, and each checks the binary's version. The partitions are the
//!   plan's, each ending within what is left of 15 minutes on a runner 1.6 times slower after the
//!   measured set-up (`geom_test::miri::PART_SECONDS`), and geom-pool's tests run by the table
//!   like any other's. Each shown to be checked by a planted change (a partition that lists every
//!   test or builds cargo-nextest, a matrix not the plan's, the hand-over kept a month or not
//!   checked, geom-pool run whole).
//! - **A partition's report is never lost silently.** Its upload is two tries of the same
//!   artifact, each bounded to a few minutes, so a stalled upload cannot hold the job to its limit;
//!   the first's failure only starts the second, and the second's fails the job. Each shown by a
//!   planted change (a try unbounded, the second run every time or hidden, another name, none).
//! - **A test nobody saw finish is not a pass (32.68).** Each Miri gate job reads its report,
//!   right after its tests and whatever they did, with the plan's kernel-quality, names in its
//!   summary every test stopped with no verdict, and fails on one not on the list of tests that may
//!   stop; the list is what the tool writes from the table, its rows over 375 s, and may only
//!   shrink. Shown by planted changes to the workflow (no reading, read only on success, its
//!   failure hidden, another stop) and by the tool on planted reports (a test off the list
//!   stopped fails its job; one on the list does not, and is named as stopped; a missing report
//!   fails it, named in the summary).
//! - **A partition that overruns keeps its log.** A partition's tests' step, and geom-pool's
//!   (the plan holds its tests to a partition's seconds), has its own limit,
//!   `geom_test::miri::TESTS_STEP_MINUTES`, over what the plan gives a partition and under the
//!   job's limit by the set-up and the first upload's bound, so an overrun fails in that step and
//!   the reading of its report and the upload still run; its tests are stopped with SIGTERM a
//!   minute before that limit (`TESTS_RUN_SECONDS`), so nextest writes the report they read.
//!   Shown by planted changes (no limit, the job's limit, a limit under the plan's, an unbounded
//!   upload, no SIGTERM or a later one).
//! - **A later push judges what it can reach.** `geom_test::increment` judges again every file of
//!   the change whose crate's tests what changed since the last clean head reaches, held here to
//!   planted changes that must be judged (a changed line, a changed test, a removed ledger entry,
//!   a change on the base's side, a file in no crate) and ones that need not (nothing, a new test
//!   target, an added ledger entry); and the workflow finds that head in GitHub's record of the
//!   pull request's runs and narrows the plan and coverage by it, each piece shown to be checked
//!   by a planted removal.
//! - **Miri's measuring run leaves every other run as it was.** It runs only on a dispatch whose
//!   input asks for it, and then runs no gate; its jobs save no cache and write no table; each part
//!   runs the measuring profile at the budget, uploads its report for a few days, and fails on a
//!   test that failed; the second part job is the first word for word. Each shown to be checked by
//!   a planted change (the plan running without the input, a measuring run that runs the gates, a
//!   cache saved, the table written, a part with no report upload, a stop other than the budget,
//!   geom-pool measured under Stacked Borrows).

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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use geom_test::increment::{self, Since};
use geom_test::miri;
use geom_test::quality_ledger::LEDGER_PATH;
use geom_test::shared::Changes;
use geom_test::test_scope::Scope;
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
        // A job that calls a reusable workflow (`cheap`) has no steps of its own.
        for step in w["jobs"][job.as_str()]["steps"]
            .as_vec()
            .into_iter()
            .flatten()
        {
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
    // A job that calls a reusable workflow (`cheap`) has no steps of its own.
    w["jobs"][job]["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .filter_map(|s| s["run"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn it_runs_once_per_change_and_never_on_a_timer() {
    let w = workflow();
    let on = &w["on"];
    assert_eq!(
        keys(on),
        [
            "pull_request",
            "merge_group",
            "push",
            "workflow_dispatch",
            "workflow_call"
        ],
        "a change's gates run when it is ready, in the queue and on main; never on a timer"
    );
    let types: Vec<&str> = on["pull_request"]["types"]
        .as_vec()
        .unwrap()
        .iter()
        .map(text)
        .collect();
    assert!(types.contains(&"ready_for_review"), "{types:?}");
    assert_eq!(
        on["push"]["branches"]
            .as_vec()
            .unwrap()
            .iter()
            .map(text)
            .collect::<Vec<_>>(),
        ["main"]
    );
    // No path filter, so the required check `done` reports on every change; `when` reads the
    // kernel's paths instead.
    assert!(on["pull_request"]["paths"].is_badvalue());
    let paths: Vec<&str> = text(&w["env"]["KERNEL_PATHS"]).lines().collect();
    for path in [
        "projects/engines/geometry/**",
        ".cargo/**",
        ".config/**",
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        ".github/**",
    ] {
        assert!(paths.contains(&path), "no path {path}");
    }
    let decide = runs(&w, "when");
    assert!(decide.contains("done <<< \"$KERNEL_PATHS\""), "{decide}");
    // A draft runs no gate.
    let draft = decide.find("if [ \"$DRAFT\" = true ]; then").unwrap();
    assert!(
        decide[draft..]
            .lines()
            .nth(1)
            .is_some_and(|l| l.trim() == "mutants=false coverage=false miri=false"),
        "{decide}"
    );
    assert!(geom_test::quality_ledger::LEDGER_PATH.starts_with("projects/engines/geometry/"));
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_coverage_judge_reads_every_part() {
    let w = workflow();
    let jobs = &w["jobs"];
    // The coverage judge judges even when a part failed, so it names every crate left unmeasured;
    // it reads every part, and `done` fails on any part that did not pass.
    assert_eq!(
        text(&jobs["coverage"]["if"]),
        "${{ !cancelled() && needs.when.outputs.coverage == 'true' && needs.coverage-plan.result == 'success' }}"
    );
    let needs: Vec<&str> = jobs["coverage"]["needs"]
        .as_vec()
        .unwrap()
        .iter()
        .map(text)
        .collect();
    assert_eq!(needs, ["when", "cheap", "coverage-plan", "coverage-parts"]);
    assert!(
        runs(&w, "done")
            .contains("must=\"$must coverage-plan coverage-parts coverage gates-seeded\""),
        "done does not require every coverage job"
    );
    let steps = jobs["coverage"]["steps"].as_vec().unwrap();
    assert!(
        steps.iter().any(|s| {
            s["with"]["pattern"].as_str() == Some("coverage-part-*")
                && s["with"]["merge-multiple"].as_bool() == Some(true)
        }),
        "the judge does not read every part's files"
    );
}

/// The workflow's jobs, in file order.
const JOBS: [&str; 17] = [
    "when",
    "cheap",
    "mutants-plan",
    "mutants",
    "mutants-2",
    "mutants-judge",
    "coverage-plan",
    "coverage-parts",
    "coverage",
    "gates-seeded",
    "miri-plan",
    "miri",
    "miri-pool",
    "miri-measure-plan",
    "miri-measure",
    "miri-measure-2",
    "done",
];

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn each_job_runs_on_the_events_its_gate_names() {
    let w = workflow();
    let jobs = &w["jobs"];
    assert_eq!(keys(jobs), JOBS);
    // Each gate runs when `when` chose it; each shard job only when the plan gives it shards, and
    // the judge even when a shard failed, so a lost shard is reported, not skipped.
    for (job, gate) in [
        ("mutants-plan", "mutants"),
        ("coverage-plan", "coverage"),
        ("coverage-parts", "coverage"),
        ("gates-seeded", "coverage"),
        ("miri-plan", "miri"),
        ("miri", "miri"),
        ("miri-pool", "miri"),
    ] {
        assert_eq!(
            text(&jobs[job]["if"]),
            format!("needs.when.outputs.{gate} == 'true'"),
            "{job}"
        );
    }
    assert_eq!(
        text(&jobs["mutants"]["if"]),
        "needs.mutants-plan.outputs.count != '0'"
    );
    assert_eq!(
        text(&jobs["mutants-2"]["if"]),
        "needs.mutants-plan.outputs.count_2 != '0'"
    );
    assert_eq!(
        text(&jobs["mutants-judge"]["if"]),
        "${{ !cancelled() && needs.when.outputs.mutants == 'true' }}"
    );
    let needs: Vec<&str> = jobs["mutants-judge"]["needs"]
        .as_vec()
        .unwrap()
        .iter()
        .map(text)
        .collect();
    assert_eq!(needs, ["when", "mutants-plan", "mutants", "mutants-2"]);
    // The judge checks the jobs it reads before it judges: a shard job may be skipped only when
    // the plan gave it no shard.
    let first = text(&jobs["mutants-judge"]["steps"][0]["run"]);
    assert!(first.contains("needs.mutants-plan.result"), "{first}");
    for (job, count) in [("mutants", "count"), ("mutants-2", "count_2")] {
        assert!(
            first.contains(&format!(
                "case \"${{{{ needs.{job}.result }}}}\" in success) ;; skipped) test \
                 \"${{{{ needs.mutants-plan.outputs.{count} }}}}\" = 0 ;; *) exit 1 ;; esac"
            )),
            "{first}"
        );
    }
    // The plan decides between the diff and the full run by the mode `when` chose.
    let plan = runs(&w, "mutants-plan");
    assert!(plan.contains("if [ \"$MODE\" = diff ]; then"));
    // Miri: one partition on a pull request, and only when it changes the Miri run; never in the
    // merge queue.
    let decide = runs(&w, "when");
    for watched in [
        ".github/workflows/kernel-quality",
        ".config/nextest",
        "geom-test/src/miri",
    ] {
        assert!(decide.contains(watched), "{watched}");
    }
    assert!(decide.contains("miri=$miri_files"), "{decide}");
    assert!(
        decide.contains("if [ \"$EVENT\" = merge_group ]; then miri=false; fi"),
        "{decide}"
    );
    // A push to main mutates the files it changed and those its ledger entries tie to a moved
    // function, and Miri runs the crates its tests reach.
    assert!(
        decide.contains("kernel-quality -- mutants-files"),
        "{decide}"
    );
    assert!(
        decide.contains("kernel-quality -- miri-packages"),
        "{decide}"
    );
    // The last job judges every other.
    let done_needs: Vec<&str> = jobs["done"]["needs"]
        .as_vec()
        .unwrap()
        .iter()
        .map(text)
        .collect();
    let others: Vec<String> = keys(jobs).into_iter().filter(|j| j != "done").collect();
    assert_eq!(done_needs, others);
    assert_eq!(text(&jobs["done"]["if"]), "${{ always() }}");
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
    // Every job that has steps (the job `cheap` only calls cheap-checks.yml, a workflow of this
    // repository at the same commit, which runs no repository code; `done` reads only the other
    // jobs' results).
    let with_steps = keys(&w["jobs"])
        .iter()
        .filter(|job| *job != "done" && !w["jobs"][job.as_str()]["steps"].is_badvalue())
        .count();
    assert_eq!(checkouts, with_steps, "one checkout per job");
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
    // The artifact actions, pinned to the commits of their tags (v7.0.2 and v8.0.2), as kernel.yml
    // pins them too.
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
        assert!(
            lines.contains(&install.as_str()),
            "{tool} is never installed"
        );
        for l in lines {
            // Miri's jobs put the plan's own build in place, and check its version
            // (`miris_jobs_run_from_one_plan_each_within_fifteen_minutes`).
            let handed = tool == "cargo-nextest" && l == HANDOVER_INSTALL;
            assert!(l == install || handed, "{tool}: {l}");
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
    // Nothing installs a tool another way, and only Miri's jobs that run from the plan take its
    // cargo-nextest.
    assert_eq!(
        all_runs.matches("cargo install").count(),
        all_runs.matches("cargo install --locked").count()
    );
    let taking: Vec<String> = keys(&w["jobs"])
        .into_iter()
        .filter(|j| runs(&w, j).contains("install -m"))
        .collect();
    assert_eq!(taking, FROM_THE_PLAN);
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
fn only_a_dispatch_narrows_the_full_run() {
    let w = workflow();
    // An optional input of a dispatch, empty unless given ...
    let files = &w["on"]["workflow_dispatch"]["inputs"]["files"];
    assert_eq!(files["required"].as_bool(), Some(false));
    assert_eq!(text(&files["default"]), "");
    assert_eq!(text(&files["type"]), "string");
    // (`gate` asks for Miri's gate alone: `a_dispatch_alone_can_run_miris_gate_alone`; the other
    // two ask for Miri's measuring run instead of the gates:
    // `a_measuring_run_runs_only_when_asked_and_leaves_nothing_behind`.)
    assert_eq!(
        keys(&w["on"]["workflow_dispatch"]["inputs"]),
        ["files", "gate", "miri_measure", "miri_measure_tests"]
    );
    // ... that no caller can set (kernel-release.yml's run is whole) ...
    assert!(w["on"]["workflow_call"].is_null() || w["on"]["workflow_call"]["inputs"].is_badvalue());
    // ... with a concurrency group of its own, so it never queues behind a whole run (the last two
    // terms are empty on every run but a measuring one and one of Miri's gate alone, so every
    // other run's group is the files') ...
    assert_eq!(
        text(&w["concurrency"]["group"]),
        "kernel-quality-${{ github.ref }}-${{ github.event_name }}-${{ inputs.files }}${{ \
         (inputs.miri_measure == 'new' || inputs.miri_measure == 'all') && \
         format('-miri-measure-{0}', inputs.miri_measure) || '' }}${{ inputs.gate == 'miri' && \
         '-miri-alone' || '' }}"
    );
    // ... reaching the steps only through the environment, never pasted into a script: `when`
    // reads it, and every job that narrows reads `when`'s choice.
    let decide = w["jobs"]["when"]["steps"]
        .as_vec()
        .unwrap()
        .iter()
        .find(|s| s["id"].as_str() == Some("when"))
        .unwrap();
    assert_eq!(text(&decide["env"]["INPUT_FILES"]), "${{ inputs.files }}");
    for job in ["mutants-plan", "mutants", "mutants-2", "mutants-judge"] {
        assert_eq!(
            text(&w["jobs"][job]["env"]["FILES"]),
            "${{ needs.when.outputs.files }}",
            "{job}"
        );
    }
    for (job, step) in steps(&w) {
        if let Some(run) = step["run"].as_str() {
            assert!(
                !run.contains("inputs."),
                "{job} reads an input in its script"
            );
        }
    }
    // A change's diff never narrows: the plan and the judge read FILES only after the diff branch.
    for job in ["mutants-plan", "mutants-judge"] {
        let run = runs(&w, job);
        let diff = run.find("= diff ]").unwrap();
        assert!(run.find("$FILES").unwrap() > diff, "{job}");
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn every_shard_and_partition_runs_once() {
    let w = workflow();
    // Every job that runs cargo-mutants on a shard: as many as the plan has lists.
    let shard_jobs: Vec<String> = keys(&w["jobs"])
        .into_iter()
        .filter(|job| runs(&w, job).contains("--shard "))
        .collect();
    assert_eq!(shard_jobs, ["mutants", "mutants-2"]);
    assert_eq!(
        u64::try_from(shard_jobs.len()).unwrap(),
        geom_test::mutants::SHARD_JOBS,
        "one shard job for each of the plan's lists"
    );
    let plan = &w["jobs"]["mutants-plan"]["outputs"];
    let plan_run = runs(&w, "mutants-plan");
    for (job, list) in [("mutants", "shards"), ("mutants-2", "shards_2")] {
        let shards = &w["jobs"][job];
        assert_eq!(
            text(&shards["strategy"]["matrix"]["shard"]),
            format!("${{{{ fromJSON(needs.mutants-plan.outputs.{list}) }}}}")
        );
        // Shards are numbered in the whole run: every job divides by the plan's full count.
        assert_eq!(
            text(&shards["env"]["SHARDS"]),
            "${{ needs.mutants-plan.outputs.count }}"
        );
        assert_eq!(shards["strategy"]["fail-fast"].as_bool(), Some(false));
        let run = runs(&w, job);
        assert!(
            run.contains("--shard \"${{ matrix.shard }}/$SHARDS\""),
            "{run}"
        );
        assert!(run.contains("--no-shuffle"), "{run}");
        // The second job is the first, word for word, but for its matrix and condition.
        for key in ["runs-on", "timeout-minutes", "env", "defaults", "steps"] {
            assert_eq!(shards[key], w["jobs"]["mutants"][key], "{job} {key}");
        }
        assert_eq!(
            text(&plan[list]),
            format!("${{{{ steps.plan.outputs.{list} }}}}")
        );
        assert!(
            plan_run.contains(&format!(
                "echo \"{list}=$(jq -c .{list} <<<\"$plan\")\" >> \"$GITHUB_OUTPUT\""
            )),
            "{plan_run}"
        );
    }
    for count in ["count", "count_2"] {
        assert_eq!(
            text(&plan[count]),
            format!("${{{{ steps.plan.outputs.{count} }}}}")
        );
        assert!(
            plan_run.contains(&format!(
                "echo \"{count}=$(jq -r .{count} <<<\"$plan\")\" >> \"$GITHUB_OUTPUT\""
            )),
            "{plan_run}"
        );
    }
    // (Miri's partitions are its plan's: `miris_jobs_run_from_one_plan_each_within_fifteen_minutes`.)
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn miris_table_is_measured_and_every_unsafe_library_has_a_finishing_test() {
    // The table reads (no row at the budget, no native-only mark without its measured time, no
    // test in both states: `miri::parse_table`) and is in the form its one writer,
    // `kernel-quality miri-times`, gives it, so it was not edited by hand.
    let rows = geom_test::miri::parse_table(geom_test::miri::TIMES).unwrap();
    let again = geom_test::miri::write_table(geom_test::miri::TIMES, &[], (1, 1), 1, None, None);
    assert_eq!(
        again.as_deref(),
        Ok(geom_test::miri::TIMES),
        "miri-times.tsv is not as `kernel-quality miri-times` writes it"
    );
    // Each library with unsafe code the kernel uses is driven by a test Miri finishes; geom-pool's
    // tests have rows too, measured under Tree Borrows, as they run.
    let problems = miri::undriven(&rows, miri::UNSAFE_LIBRARIES);
    assert!(problems.is_empty(), "{problems:#?}");
    // Every library of the lock with unsafe code that the kernel uses is named, and nothing else.
    let named: BTreeSet<&str> = geom_test::miri::UNSAFE_LIBRARIES
        .iter()
        .map(|l| l.name)
        .collect();
    let found = unsafe_libraries();
    let missing: Vec<&String> = found
        .iter()
        .filter(|l| !named.contains(l.as_str()))
        .collect();
    let stale: Vec<&&str> = named.iter().filter(|l| !found.contains(**l)).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "libraries with unsafe code that the kernel uses, with no finishing test named in \
         geom_test::miri::UNSAFE_LIBRARIES: {missing:?}; named there, but not such a library: \
         {stale:?}"
    );
}

/// The registry packages the kernel uses whose own library sources hold the `unsafe` keyword, read
/// as Rust tokens so comments and strings do not count: for Linux, the kernel crates' normal
/// dependencies and theirs (what ships), and the registry crates the kernel crates' tests name
/// directly; never a build script's or a procedural macro's, which Miri does not run, nor what
/// reaches a test only through a development crate.
fn unsafe_libraries() -> BTreeSet<String> {
    let root = root();
    let out = std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args([
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--filter-platform",
            "x86_64-unknown-linux-gnu",
        ])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let packages: BTreeMap<&str, &serde_json::Value> = meta["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p))
        .collect();
    let kernel = geom_test::miri::packages(&meta).unwrap();
    // How far to walk from a package: 0 a kernel crate (its normal dependencies, and its registry
    // development dependencies themselves), 1 a normal dependency (its own), 2 no further.
    let mut work: Vec<(&str, u8)> = packages
        .iter()
        .filter(|(_, p)| {
            p["source"].is_null() && kernel.iter().any(|k| p["name"].as_str() == Some(k))
        })
        .map(|(id, _)| (*id, 0))
        .collect();
    let deps: BTreeMap<&str, &serde_json::Value> = meta["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["id"].as_str().unwrap(), &n["deps"]))
        .collect();
    let mut seen = BTreeSet::new();
    let mut found = BTreeSet::new();
    while let Some((id, walk)) = work.pop() {
        if !seen.insert((id, walk)) {
            continue;
        }
        let p = packages[id];
        if !p["source"].is_null() && has_unsafe(p) {
            found.insert(p["name"].as_str().unwrap().to_owned());
        }
        for d in deps[id].as_array().unwrap() {
            let target = packages[d["pkg"].as_str().unwrap()];
            let kinds: Vec<&serde_json::Value> = d["dep_kinds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|k| &k["kind"])
                .collect();
            let normal = kinds.iter().any(|k| k.is_null());
            let dev = walk == 0 && !target["source"].is_null() && kinds.iter().any(|k| *k == "dev");
            let macro_only = target["targets"].as_array().unwrap().iter().all(|t| {
                t["kind"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|k| k == "proc-macro")
            });
            if macro_only || walk == 2 {
                continue;
            }
            if normal {
                work.push((d["pkg"].as_str().unwrap(), 1));
            } else if dev {
                work.push((d["pkg"].as_str().unwrap(), 2));
            }
        }
    }
    found
}

/// Whether a registry package's library sources (its `src` folder, or the folder of its library
/// target) hold the `unsafe` keyword as a token.
fn has_unsafe(package: &serde_json::Value) -> bool {
    let Some(lib) = package["targets"].as_array().unwrap().iter().find(|t| {
        t["kind"]
            .as_array()
            .unwrap()
            .iter()
            .any(|k| matches!(k.as_str(), Some("lib" | "rlib")))
    }) else {
        return false;
    };
    let src = PathBuf::from(lib["src_path"].as_str().unwrap());
    let mut dirs = vec![src.parent().unwrap().to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                if let Ok(tokens) = text.parse::<proc_macro2::TokenStream>()
                    && tokens_hold_unsafe(tokens)
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Whether a token stream holds the identifier `unsafe`, at any depth.
fn tokens_hold_unsafe(tokens: proc_macro2::TokenStream) -> bool {
    let mut work = vec![tokens];
    while let Some(stream) = work.pop() {
        for t in stream {
            match t {
                proc_macro2::TokenTree::Ident(i) if i == "unsafe" => return true,
                proc_macro2::TokenTree::Group(g) => work.push(g.stream()),
                _ => {}
            }
        }
    }
    false
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
        "git show \"$BASE:projects/engines/geometry/quality-ledger.json\"",
    ] {
        assert!(plan.contains(needle), "the plan does not run {needle:?}");
    }
    // A narrowed full run: the shards' selection, every mutant, and the plan's check of the two.
    for needle in [
        "cargo mutants --re \"$FILES\" --list --json > \"$RUNNER_TEMP/plan/list.json\"",
        "cargo mutants --list --json > \"$RUNNER_TEMP/plan/all.json\"",
        "narrowed=(--files \"$FILES\" --all \"$RUNNER_TEMP/plan/all.json\")",
        "--list \"$RUNNER_TEMP/plan/list.json\" \"${narrowed[@]}\"",
    ] {
        assert!(plan.contains(needle), "the plan does not run {needle:?}");
    }
    let judge = runs(&w, "mutants-judge");
    for needle in [
        "args+=(--files \"$FILES\")",
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
    // Each shard selects as the plan listed: a pull request's diff, or a narrowed run's files.
    for needle in [
        "only=(--in-diff \"$RUNNER_TEMP/plan/pr.diff\")",
        "elif [ -n \"$FILES\" ]; then\n  only=(--re \"$FILES\")",
        "cargo mutants \"${only[@]}\" --shard",
        // A runaway mutant aborts its own test instead of shutting the runner down.
        "ulimit -d $((8 * 1024 * 1024))",
    ] {
        assert!(shard.contains(needle), "the shards do not run {needle:?}");
    }
    assert!(
        shard.find("ulimit -d").unwrap() < shard.find("cargo mutants ").unwrap(),
        "the limit is set before cargo-mutants starts"
    );
    assert_eq!(shard.matches("cargo mutants ").count(), 1);
    assert!(shard.contains("case \"$code\" in 0|2|3) ;; *) exit \"$code\" ;; esac"));
    assert!(shard.contains("test -f \"$RUNNER_TEMP/out/mutants.out/mutants.json\""));
    // Coverage: planned in parts by the judge's own list, each part measured on the nightly with
    // branches, each run's log kept, then every part judged together.
    let coverage = format!(
        "{}{}{}",
        runs(&w, "coverage-plan"),
        runs(&w, "coverage-parts"),
        runs(&w, "coverage")
    );
    for needle in [
        "kernel-quality -- coverage-parts $SCOPE",
        "for crate in $CRATES; do",
        "kernel-quality -- coverage $SCOPE",
        "--branch",
        "| tee \"$RUNNER_TEMP/cov/$crate.log\"",
    ] {
        assert!(
            coverage.contains(needle),
            "coverage does not run {needle:?}"
        );
    }
    let parts = &w["jobs"]["coverage-parts"];
    assert_eq!(
        text(&parts["strategy"]["matrix"]),
        "${{ fromJSON(needs.coverage-plan.outputs.matrix) }}"
    );
    assert_eq!(parts["strategy"]["fail-fast"].as_bool(), Some(false));
    assert_eq!(
        text(&w["jobs"]["coverage-plan"]["outputs"]["matrix"]),
        "${{ steps.parts.outputs.matrix }}"
    );
    let measure = w["jobs"]["coverage-parts"]["steps"]
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
    let miri = [runs(&w, "miri-plan"), runs(&w, "miri")].concat();
    for needle in [
        "kernel-quality -- miri-packages",
        "cargo miri nextest run --locked $PACKAGES",
        "profile=(--profile miri-smoke)",
    ] {
        assert!(miri.contains(needle), "Miri does not run {needle:?}");
    }
    // The doctests, in geom-pool's job.
    assert!(runs(&w, "miri-pool").contains("cargo miri test --locked $PACKAGES --doc"));
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
    // Every Miri run of the gate's jobs, and of each measuring part job, as (job, flags, run).
    let mut with_flags: Vec<(String, String, String)> = Vec::new();
    for (job, step) in steps(&w) {
        if let (Some(flags), Some(run)) = (step["env"]["MIRIFLAGS"].as_str(), step["run"].as_str())
        {
            with_flags.push((job, flags.to_owned(), run.to_owned()));
        }
    }
    // Only a Tree Borrows run turns Miri's leak check off (rayon's pools stop their threads
    // without joining them); every other run keeps it.
    for (job, flags, run) in &with_flags {
        if flags.contains("-Zmiri-ignore-leaks") {
            assert!(flags.contains("-Zmiri-tree-borrows"), "{job}: {run}");
        }
    }
    // In the gate's jobs one run is Tree Borrows, geom-pool's job's, of that crate alone; in each
    // measuring part job one, the same. Every other nextest run leaves the crate out.
    for (jobs, want) in [
        (vec!["miri-plan", "miri", "miri-pool"], "miri-pool"),
        (vec!["miri-measure"], "miri-measure"),
        (vec!["miri-measure-2"], "miri-measure-2"),
    ] {
        let tree: Vec<&(String, String, String)> = with_flags
            .iter()
            .filter(|(job, flags, _)| {
                jobs.contains(&job.as_str()) && flags.contains("-Zmiri-tree-borrows")
            })
            .collect();
        assert_eq!(tree.len(), 1, "one step of {jobs:?} runs Tree Borrows");
        assert_eq!(tree[0].0, want);
        let run = joined(&tree[0].2).join("\n");
        assert!(
            run.contains(&format!(
                "cargo miri nextest run --locked -p {TREE_BORROWS_CRATE} "
            )) && run.contains(&format!("-E \"package({TREE_BORROWS_CRATE}) & (")),
            "{run}"
        );
    }
    for (job, flags, run) in &with_flags {
        if flags.contains("-Zmiri-tree-borrows") {
            continue;
        }
        let run = joined(run).join("\n");
        if run.contains("cargo miri nextest run --locked $PACKAGES") {
            assert!(
                run.contains(&format!("-E \"not package({TREE_BORROWS_CRATE}) & (")),
                "{job}: {run}"
            );
        }
    }
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
        "crossbeam-epoch moved: look at kernel-quality.yml's Tree Borrows steps again"
    );
}

/// The line that puts the plan's cargo-nextest in place in a Miri job (the artifact keeps no file
/// modes), and the check of its version that must follow it.
const HANDOVER_INSTALL: &str =
    "install -m 755 \"$RUNNER_TEMP/miri-plan/cargo-nextest\" \"$HOME/.cargo/bin/cargo-nextest\"";
const HANDOVER_CHECK: &str =
    "cargo nextest --version | grep -qxF \"release: $CARGO_NEXTEST_VERSION\"";

/// The Miri jobs that run from the plan's hand-over.
const FROM_THE_PLAN: [&str; 2] = ["miri", "miri-pool"];

/// What is wrong with Miri's plan and the jobs that run from it, given the workflow's text: one plan
/// lists the tests and builds the tools, and hands them on for a day; the partitions are its own,
/// each within its job's time; geom-pool's job runs geom-pool's finishing tests.
fn miri_plan_problems(text: &str) -> Vec<String> {
    let w = YamlLoader::load_from_str(text).unwrap().remove(0);
    let jobs = &w["jobs"];
    let mut found = Vec::new();
    let plan = &jobs["miri-plan"];
    let needs = |job: &str| -> Vec<String> {
        jobs[job]["needs"]
            .as_vec()
            .into_iter()
            .flatten()
            .filter_map(|n| n.as_str().map(str::to_owned))
            .collect()
    };
    if needs("miri-plan") != ["when", "cheap"]
        || plan["outputs"]["parts"].as_str() != Some("${{ steps.plan.outputs.parts }}")
    {
        found.push("miri-plan does not hand on its partitions".to_owned());
    }
    let plan_runs = joined(&runs(&w, "miri-plan")).join("\n");
    for needle in [
        "cargo install --locked cargo-nextest --version \"$CARGO_NEXTEST_VERSION\"",
        "cargo miri nextest list --locked $PACKAGES --message-format json > \
         \"$RUNNER_TEMP/miri-plan/miri-list.json\"",
        "kernel-quality -- miri-plan --list \"$RUNNER_TEMP/miri-plan/miri-list.json\" --out \
         \"$RUNNER_TEMP/miri-plan\")",
        "echo \"parts=$(jq -c .parts <<<\"$plan\")\" >> \"$GITHUB_OUTPUT\"",
        "echo \"$PACKAGES\" > \"$RUNNER_TEMP/miri-plan/packages.txt\"",
        "cp \"$(command -v cargo-nextest)\" \"$RUNNER_TEMP/miri-plan/cargo-nextest\"",
    ] {
        if !plan_runs.contains(needle) {
            found.push(format!("miri-plan does not run {needle:?}"));
        }
    }
    // The hand-over: one artifact of this run, kept one day, under the name the jobs download.
    let uploads: Vec<&Yaml> = plan["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .filter(|s| {
            s["uses"]
                .as_str()
                .is_some_and(|u| u.starts_with("actions/upload-artifact@"))
        })
        .collect();
    let handed = uploads.len() == 1
        && uploads[0]["with"]["name"].as_str() == Some("miri-plan")
        && uploads[0]["with"]["path"].as_str() == Some("${{ runner.temp }}/miri-plan")
        && uploads[0]["with"]["retention-days"].as_i64() == Some(1);
    if !handed {
        found.push("miri-plan does not hand on one artifact kept one day".to_owned());
    }
    found.extend(from_the_plan_problems(&w));
    // The partitions are the plan's; a pull request runs the first.
    let miri = &jobs["miri"];
    if miri["strategy"]["matrix"]["partition"].as_str()
        != Some(
            "${{ fromJSON(github.event_name == 'pull_request' && '[1]' || \
             needs.miri-plan.outputs.parts) }}",
        )
        || miri["strategy"]["fail-fast"].as_bool() != Some(false)
    {
        found.push("miri does not run the plan's partitions".to_owned());
    }
    let partition = joined(&runs(&w, "miri")).join("\n");
    if !partition.contains(
        "cargo miri nextest run --locked $PACKAGES \"${profile[@]}\" -E \"not package(geom-pool) \
         & ($(cat \"$RUNNER_TEMP/miri-plan/part-${{ matrix.partition }}.txt\"))\"",
    ) {
        found.push("miri does not run its partition's tests".to_owned());
    }
    let pool = joined(&runs(&w, "miri-pool")).join("\n");
    if !pool.contains(
        "cargo miri nextest run --locked -p geom-pool \"${profile[@]}\" -E \"package(geom-pool) & \
         ($(cat \"$RUNNER_TEMP/miri-plan/pool.txt\"))\"",
    ) {
        found.push("geom-pool's job does not run the table's finishing tests".to_owned());
    }
    found
}

/// What is wrong with the Miri jobs that run from the plan's hand-over: each downloads it, puts
/// its cargo-nextest in place and checks its version, and builds, lists and saves nothing itself.
fn from_the_plan_problems(w: &Yaml) -> Vec<String> {
    let jobs = &w["jobs"];
    let mut found = Vec::new();
    let needs = |job: &str| -> Vec<String> {
        jobs[job]["needs"]
            .as_vec()
            .into_iter()
            .flatten()
            .filter_map(|n| n.as_str().map(str::to_owned))
            .collect()
    };
    for job in FROM_THE_PLAN {
        let steps = jobs[job]["steps"].as_vec().cloned().unwrap_or_default();
        let downloads = steps.iter().any(|s| {
            s["uses"]
                .as_str()
                .is_some_and(|u| u.starts_with("actions/download-artifact@"))
                && s["with"]["name"].as_str() == Some("miri-plan")
                && s["with"]["path"].as_str() == Some("${{ runner.temp }}/miri-plan")
        });
        if !needs(job).iter().any(|n| n == "miri-plan") || !downloads {
            found.push(format!("{job} does not run from the plan's hand-over"));
        }
        let commands = joined(&runs(w, job));
        let install = commands.iter().position(|c| c.trim() == HANDOVER_INSTALL);
        let check = commands.iter().position(|c| c.trim() == HANDOVER_CHECK);
        if !matches!((install, check), (Some(i), Some(c)) if i < c) {
            found.push(format!(
                "{job} does not put the plan's cargo-nextest in place and check its version"
            ));
        }
        let all = commands.join("\n");
        for (what, said) in [
            ("cargo install", "builds a tool"),
            ("nextest list", "lists every test"),
            ("-p geom-test", "builds kernel-quality"),
        ] {
            if all.contains(what) {
                found.push(format!("{job} {said}"));
            }
        }
        let saves = steps.iter().any(|s| {
            s["uses"]
                .as_str()
                .is_some_and(|u| u.starts_with("Swatinem/rust-cache@"))
                && s["with"]["save-if"].as_bool() != Some(false)
        });
        if saves {
            found.push(format!("{job} saves a cache"));
        }
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn miris_jobs_run_from_one_plan_each_within_fifteen_minutes() {
    let text = read(".github/workflows/kernel-quality.yml").replace("\r\n", "\n");
    let found = miri_plan_problems(&text);
    assert!(found.is_empty(), "{found:#?}");
    // The table's plan: every finishing test in one part, each partition's listing and tests
    // within PART_SECONDS in any order, geom-pool's tests in a part of their own within it, and
    // each filterset within one command-line argument (Linux takes at most 128 KiB in one).
    let rows = miri::parse_table(miri::TIMES).unwrap();
    let gate = miri::gate_plan(&rows).unwrap();
    for k in 1..=gate.parts.len() {
        assert!(gate.parts.ends_within(k, miri::Rule::Job), "partition {k}");
        let filter = gate.parts.filterset(k).unwrap();
        assert!(filter.len() < 100_000, "partition {k}: {}", filter.len());
    }
    assert!(gate.pool.ends_within(1, miri::Rule::Job));
    let placed = (1..=gate.parts.len())
        .map(|k| gate.parts.tests_of(k).len())
        .fold(gate.pool.tests_of(1).len(), |a, n| a + n);
    assert_eq!(
        placed,
        miri::finishing(&rows).len(),
        "a finishing test left out"
    );
    // The arithmetic: a part's set-up and its listing and tests fill 15 minutes, the tests
    // planned on a runner 1.6 times slower and never over the stop, and the longest test that
    // finishes fits a part alone with its binary's listing on that runner.
    assert_eq!(miri::FIXED_SECONDS + miri::PART_SECONDS, miri::JOB_SECONDS);
    assert_eq!(miri::JOB_SECONDS, 15 * 60);
    assert_eq!(miri::FIXED_SECONDS, 120);
    assert_eq!(miri::SLOWER, (8, 5));
    assert_eq!(miri::planned(miri::BUDGET - 1), miri::BUDGET);
    assert_eq!(miri::planned(300), 480);
    let listing = (miri::LIST_SECONDS * miri::SLOWER.0).div_ceil(miri::SLOWER.1);
    assert!(miri::BUDGET + listing <= miri::PART_SECONDS);
    assert_eq!(miri::TREE_BORROWS_CRATE, TREE_BORROWS_CRATE);
    // Seeded violations: each must be reported.
    for (from, to, said) in [
        // A partition that builds cargo-nextest again.
        (
            HANDOVER_INSTALL,
            "cargo install --locked cargo-nextest --version \"$CARGO_NEXTEST_VERSION\"",
            "miri builds a tool",
        ),
        // A partition that lists every test again.
        (
            "          cargo miri nextest run --locked $PACKAGES \"${profile[@]}\" \\\n",
            "          cargo miri nextest list --locked $PACKAGES > \"$RUNNER_TEMP/all.json\"\n          \
             cargo miri nextest run --locked $PACKAGES \"${profile[@]}\" \\\n",
            "miri lists every test",
        ),
        // A partition that builds kernel-quality to plan itself.
        (
            "          packages=$(cat \"$RUNNER_TEMP/miri-plan/packages.txt\")\n",
            "          packages=$(cargo run --locked -p geom-test --bin kernel-quality -- miri-packages)\n",
            "miri builds kernel-quality",
        ),
        // The hand-over's binary run unchecked.
        (
            HANDOVER_CHECK,
            "true",
            "miri does not put the plan's cargo-nextest in place and check its version",
        ),
        // A matrix of its own instead of the plan's.
        (
            "needs.miri-plan.outputs.parts) }}",
            "'[1, 2, 3]') }}",
            "miri does not run the plan's partitions",
        ),
        // The hand-over kept for a month, for later runs to find.
        (
            "          name: miri-plan\n          path: ${{ runner.temp }}/miri-plan\n          \
             retention-days: 1\n",
            "          name: miri-plan\n          path: ${{ runner.temp }}/miri-plan\n          \
             retention-days: 30\n",
            "miri-plan does not hand on one artifact kept one day",
        ),
        // geom-pool run whole, its stopped test to the stop again.
        (
            " \\\n            -E \"package(geom-pool) & ($(cat \"$RUNNER_TEMP/miri-plan/pool.txt\"))\"",
            "",
            "geom-pool's job does not run the table's finishing tests",
        ),
        // A partition that saves a cache of its own.
        (
            "          # Restores the cache the plan saves, and saves none.\n          save-if: false\n",
            "          # Restores the cache the plan saves, and saves none.\n          save-if: true\n",
            "miri saves a cache",
        ),
    ] {
        let planted = text.replacen(from, to, 1);
        assert_ne!(planted, text, "the planted edit {from:?} applies");
        let found = miri_plan_problems(&planted);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}

/// A Miri partition's report: the artifact `miri-<partition>`, the files nextest writes.
const MIRI_REPORT: (&str, &str) = (
    "miri-${{ matrix.partition }}",
    "target/nextest/*/junit*.xml",
);

/// The condition of the second try: the first was tried and did not succeed.
const MIRI_SECOND_TRY: &str =
    "${{ !cancelled() && steps.report.outcome != 'success' && steps.report.outcome != 'skipped' }}";

/// What is wrong with how a Miri partition uploads its report, given the workflow's text: its last
/// two steps are two tries of the same artifact, each bounded to a few minutes; the first runs
/// whatever the tests did and its failure only starts the second; the second runs only then, may
/// replace what the first left, and its failure fails the job. No other step continues on error.
fn miri_upload_problems(text: &str) -> Vec<String> {
    let w = YamlLoader::load_from_str(text).unwrap().remove(0);
    let steps: Vec<&Yaml> = w["jobs"]["miri"]["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .collect();
    let uploads: Vec<(usize, &Yaml)> = steps
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, s)| {
            s["uses"]
                .as_str()
                .is_some_and(|u| u.starts_with("actions/upload-artifact@"))
        })
        .collect();
    let mut found = Vec::new();
    let [(first_at, first), (second_at, second)] = uploads.as_slice() else {
        found.push(format!(
            "miri uploads its report in {} steps, not two tries",
            uploads.len()
        ));
        return found;
    };
    if *second_at != first_at + 1 || *second_at + 1 != steps.len() {
        found.push("miri's two tries are not its last two steps".to_owned());
    }
    for (which, s) in [("first", first), ("second", second)] {
        if s["with"]["name"].as_str() != Some(MIRI_REPORT.0)
            || s["with"]["path"].as_str() != Some(MIRI_REPORT.1)
        {
            found.push(format!(
                "miri's {which} try does not upload {} as {}",
                MIRI_REPORT.1, MIRI_REPORT.0
            ));
        }
        if !s["timeout-minutes"]
            .as_i64()
            .is_some_and(|m| (1..=5).contains(&m))
        {
            found.push(format!(
                "miri's {which} try is not bounded to a few minutes"
            ));
        }
    }
    if first["id"].as_str() != Some("report") || first["if"].as_str() != Some("${{ !cancelled() }}")
    {
        found.push("miri's first try does not run, as `report`, whatever the tests did".to_owned());
    }
    if first["continue-on-error"].as_bool() != Some(true) {
        found.push(
            "miri's first try fails the job by itself, so a second try that works cannot pass it"
                .to_owned(),
        );
    }
    if second["if"].as_str() != Some(MIRI_SECOND_TRY) {
        found.push("miri's second try does not run only when the first did not succeed".to_owned());
    }
    if second["with"]["overwrite"].as_bool() != Some(true) {
        found.push("miri's second try cannot replace what the first left".to_owned());
    }
    for (at, s) in steps.iter().enumerate() {
        if at != *first_at && !s["continue-on-error"].is_badvalue() {
            found.push(format!(
                "miri's step {} continues on error, which hides a lost report or a red test",
                at + 1
            ));
        }
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn a_partitions_report_is_uploaded_in_two_bounded_tries_and_a_lost_one_fails_the_job() {
    let text = read(".github/workflows/kernel-quality.yml").replace("\r\n", "\n");
    let found = miri_upload_problems(&text);
    assert!(found.is_empty(), "{found:#?}");
    let second_if = format!("        if: {MIRI_SECOND_TRY}\n");
    let second_if_timed = format!("{second_if}        timeout-minutes: 3\n");
    let second_if_hidden = format!("{second_if}        continue-on-error: true\n");
    let second = "      - name: The report again, the first try having failed\n        uses: \
                  actions/upload-artifact@";
    let second_gone = "      - name: The report again, the first try having failed\n        uses: \
                       actions/download-artifact@";
    let tests_step = "/part-${{ matrix.partition }}.txt\"))\"\n";
    let tests_step_hidden = format!("{tests_step}        continue-on-error: true\n");
    // Seeded violations: each must be reported.
    for (from, to, said) in [
        // The first try unbounded, or bounded to the job's limit.
        (
            "        timeout-minutes: 3\n        continue-on-error: true\n",
            "        continue-on-error: true\n",
            "first try is not bounded",
        ),
        (
            "        timeout-minutes: 3\n        continue-on-error: true\n",
            "        timeout-minutes: 30\n        continue-on-error: true\n",
            "first try is not bounded",
        ),
        // The first try's failure failing the job.
        (
            "        timeout-minutes: 3\n        continue-on-error: true\n",
            "        timeout-minutes: 3\n",
            "first try fails the job by itself",
        ),
        // The second try unbounded.
        (
            second_if_timed.as_str(),
            second_if.as_str(),
            "second try is not bounded",
        ),
        // The second try run every time, or only on a failure, not on a timeout.
        (
            second_if.as_str(),
            "        if: ${{ !cancelled() }}\n",
            "second try does not run only when",
        ),
        (
            second_if.as_str(),
            "        if: steps.report.outcome == 'failure'\n",
            "second try does not run only when",
        ),
        // The second try's failure hidden.
        (
            second_if.as_str(),
            second_if_hidden.as_str(),
            "continues on error",
        ),
        // The second try under another name than its reader's.
        (
            "          name: miri-${{ matrix.partition }}\n          path: target/nextest/*/junit*.xml\n          if-no-files-found: ignore\n          overwrite: true\n",
            "          name: miri-${{ matrix.partition }}-again\n          path: target/nextest/*/junit*.xml\n          if-no-files-found: ignore\n          overwrite: true\n",
            "second try does not upload",
        ),
        // The second try unable to replace a part the first left.
        (
            "          overwrite: true\n",
            "",
            "cannot replace what the first left",
        ),
        // No second try.
        (second, second_gone, "not two tries"),
        // The tests' step made to pass whatever they did.
        (tests_step, tests_step_hidden.as_str(), "continues on error"),
    ] {
        let planted = text.replacen(from, to, 1);
        assert_ne!(planted, text, "the planted edit {from:?} applies");
        let found = miri_upload_problems(&planted);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
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
    // The full run's budget: two periods of five minutes, so a partition's worst case fits the job.
    let full = &n["profile"]["default-miri"]["slow-timeout"];
    assert_eq!(full["period"].as_str(), Some("300s"));
    assert_eq!(full["terminate-after"].as_integer(), Some(2));
    assert!(n["profile"].as_table().unwrap().get("default").is_none());
}

/// The files of `plan` that are not among `want`, and of `want` that `plan` lacks.
fn plan_problems(plan: &increment::Plan, want: &[&str]) -> Vec<String> {
    let got: BTreeSet<&str> = plan.keys().map(String::as_str).collect();
    let want: BTreeSet<&str> = want.iter().copied().collect();
    let mut found: Vec<String> = want
        .difference(&got)
        .map(|f| format!("{f} is not judged again"))
        .collect();
    found.extend(
        got.difference(&want)
            .map(|f| format!("{f} is judged again")),
    );
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository and runs cargo metadata, which a WebAssembly test cannot"
)]
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "each planted case beside what it must judge reads best in one place"
)]
fn a_later_push_judges_again_what_its_change_can_reach() {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let metadata = geom_test::deps::cargo_metadata(&cargo, &root().join("Cargo.toml")).unwrap();
    let scope = Scope::read(&metadata).unwrap();
    let folder = |c: &str| scope.folders()[c].clone();
    // Two kernel crates whose tests one another's code does not reach, neither a reader.
    let reach = |c: &str| -> BTreeSet<String> {
        let file = format!("{}src/planted.rs", folder(c));
        scope
            .select(Some(&[file]))
            .running()
            .into_iter()
            .map(str::to_owned)
            .collect()
    };
    let kernel: Vec<&String> = scope
        .folders()
        .keys()
        .filter(|c| c.starts_with("geom-") && !scope.readers().contains(*c))
        .collect();
    let reached: BTreeMap<&String, BTreeSet<String>> =
        kernel.iter().map(|c| (*c, reach(c))).collect();
    let (x, y) = kernel
        .iter()
        .flat_map(|a| kernel.iter().map(move |b| (*a, *b)))
        .find(|(a, b)| a != b && !reached[a].contains(*b) && !reached[b].contains(*a))
        .map(|(a, b)| (a.clone(), b.clone()))
        .expect("two crates whose changes do not reach one another");
    let x = x.as_str();
    let xf = format!("{}src/planted.rs", folder(x));
    let yf = format!("{}src/planted.rs", folder(&y));
    let changed = vec![xf.clone(), yf.clone()];
    let ledger = |files: &[&str]| -> String {
        let survivors: Vec<String> = files
            .iter()
            .map(|f| format!("{{\"file\": \"{f}\", \"mutant\": \"m\"}}"))
            .collect();
        format!("{{\"survivors\": [{}]}}", survivors.join(", "))
    };
    let plan = |since: &Since| increment::mutants(&scope, &changed, since, &Changes::new());
    let s = |head: &[&str], added: &[&str], base: &[&str]| Since {
        head: head.iter().map(|f| (*f).to_owned()).collect(),
        added: added.iter().map(|f| (*f).to_owned()).collect(),
        base: base.iter().map(|f| (*f).to_owned()).collect(),
        ledger: (Some(ledger(&[])), Some(ledger(&[]))),
    };
    let x_test = format!("{}tests/attack.rs", folder(x));
    let x_new = format!("{}tests/new_target.rs", folder(x));
    let every = [xf.as_str(), yf.as_str()];
    let cases: Vec<(&str, Since, Vec<&str>)> = vec![
        ("nothing changed", s(&[], &[], &[]), vec![]),
        ("a changed line of x", s(&[&xf], &[], &[]), vec![&xf]),
        ("a changed test of x", s(&[&x_test], &[], &[]), vec![&xf]),
        (
            "a new test target of x",
            s(&[&x_new], &[&x_new], &[]),
            vec![],
        ),
        (
            "a change on the base's side in y",
            s(&[], &[], &[&yf]),
            vec![&yf],
        ),
        (
            "a file in no crate",
            s(&["Cargo.lock"], &[], &[]),
            every.to_vec(),
        ),
        (
            "the base's ledger",
            s(&[], &[], &[LEDGER_PATH]),
            every.to_vec(),
        ),
        (
            "a removed survivor of x",
            Since {
                ledger: (Some(ledger(&[&xf])), Some(ledger(&[]))),
                ..s(&[LEDGER_PATH], &[], &[])
            },
            vec![&xf],
        ),
        (
            "an added survivor of x",
            Since {
                ledger: (Some(ledger(&[])), Some(ledger(&[&xf]))),
                ..s(&[LEDGER_PATH], &[], &[])
            },
            vec![],
        ),
        (
            "a ledger that cannot be read",
            Since {
                ledger: (Some("not json".to_owned()), Some(ledger(&[]))),
                ..s(&[LEDGER_PATH], &[], &[])
            },
            every.to_vec(),
        ),
    ];
    for (what, since, want) in &cases {
        let found = plan_problems(&plan(since), want);
        assert!(found.is_empty(), "{what}: {found:#?}");
    }
    // Planted: a planner that reads only the lines changed since (the files the head changed),
    // which misses a changed test, a removed survivor and the base's side.
    let lines_only = |since: &Since| -> increment::Plan {
        changed
            .iter()
            .filter(|f| since.head.contains(f))
            .map(|f| (f.clone(), String::new()))
            .collect()
    };
    let missed: Vec<&str> = cases
        .iter()
        .filter(|(_, since, want)| !plan_problems(&lines_only(since), want).is_empty())
        .map(|(what, _, _)| *what)
        .collect();
    for what in [
        "a changed test of x",
        "a removed survivor of x",
        "a change on the base's side in y",
    ] {
        assert!(missed.contains(&what), "{what}: {missed:?}");
    }
}

/// What is wrong with how the workflow narrows a later push.
fn narrowing_problems(text: &str) -> Vec<String> {
    let w = YamlLoader::load_from_str(text).unwrap().remove(0);
    let mut found = Vec::new();
    let decide = runs(&w, "when");
    for needle in [
        "actions/workflows/kernel-quality.yml/runs?event=pull_request&status=completed",
        "select(any(.pull_requests[]; .number == $PR))",
        "grep -qxF \"success Mutants, judged against the ledger\" <<< \"$jobs\"",
        "grep -qxF \"success Coverage (lines, and branches on the pinned nightly)\" <<< \"$jobs\"",
    ] {
        if !decide.contains(needle) {
            found.push(format!("`when` does not read GitHub's record: {needle}"));
        }
    }
    let outputs = &w["jobs"]["when"]["outputs"];
    if outputs["since_mutants"].as_str() != Some("${{ steps.since.outputs.mutants }}")
        || outputs["since_coverage"].as_str() != Some("${{ steps.since.outputs.coverage }}")
    {
        found.push("`when` does not hand on the last clean heads".to_owned());
    }
    let plan = &w["jobs"]["mutants-plan"];
    if plan["env"]["SINCE"].as_str() != Some("${{ needs.when.outputs.since_mutants }}") {
        found.push("the plan does not read the last clean head".to_owned());
    }
    let plan_runs = runs(&w, "mutants-plan");
    for needle in [
        "kernel-quality -- mutants-increment",
        "git log --format= --name-only \"$SINCE..$BASE\"",
        "git diff --no-renames --name-only \"$SINCE\" HEAD^2",
        "xargs -a \"$t/again.txt\" -d '\\n' git diff --no-ext-diff --no-color --no-renames \"$BASE\" HEAD -- > \"$t/pr.diff\"",
    ] {
        if !plan_runs.contains(needle) {
            found.push(format!(
                "the plan does not narrow by the last clean head: {needle}"
            ));
        }
    }
    let narrowing = plan_runs.find("mutants-increment").unwrap_or(usize::MAX);
    let listing = plan_runs
        .find("cargo mutants --in-diff \"$RUNNER_TEMP/plan/pr.diff\" --list")
        .unwrap_or(0);
    if narrowing > listing {
        found.push("the plan lists the mutants before it narrows the diff".to_owned());
    }
    let coverage = runs(&w, "coverage-plan");
    if !coverage.contains("kernel-quality -- coverage-increment --since \"$SINCE\"") {
        found.push("coverage does not narrow by the last clean head".to_owned());
    }
    for job in ["mutants-plan", "coverage-plan"] {
        if w["jobs"][job]["steps"][0]["with"]["fetch-depth"].as_i64() != Some(0) {
            found.push(format!("{job} lacks the history the base's side needs"));
        }
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn a_later_push_is_narrowed_by_githubs_record_of_the_last_clean_run() {
    let text = read(".github/workflows/kernel-quality.yml").replace("\r\n", "\n");
    let found = narrowing_problems(&text);
    assert!(found.is_empty(), "{found:#?}");
    for (from, to, said) in [
        (
            "select(any(.pull_requests[]; .number == $PR))",
            "select(true)",
            "does not read GitHub's record",
        ),
        (
            "since_mutants: ${{ steps.since.outputs.mutants }}",
            "since_mutants: ''",
            "does not hand on",
        ),
        (
            "SINCE: ${{ needs.when.outputs.since_mutants }}",
            "SINCE: ''",
            "the plan does not read the last clean head",
        ),
        (
            "git log --format= --name-only \"$SINCE..$BASE\"",
            "true",
            "the plan does not narrow",
        ),
        (
            "coverage-increment --since",
            "coverage-increment-not --since",
            "coverage does not narrow",
        ),
        (
            "          # The whole history, for what changed since the last clean head on the base's side.\n          fetch-depth: 0",
            "          fetch-depth: 2",
            "lacks the history",
        ),
    ] {
        let planted = text.replacen(from, to, 1);
        assert_ne!(planted, text, "the planted edit {from:?} applies");
        let found = narrowing_problems(&planted);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}

/// The jobs of Miri's measuring run.
const MEASURE_JOBS: [&str; 3] = ["miri-measure-plan", "miri-measure", "miri-measure-2"];

/// The part jobs of Miri's measuring run, each with the plan's list and count it reads.
const MEASURE_PARTS: [(&str, &str, &str); 2] = [
    ("miri-measure", "parts", "count"),
    ("miri-measure-2", "parts_2", "count_2"),
];

/// Where a measuring part keeps its reports, one a run: nextest writes each under the workspace's
/// own target folder, and the part renames it for its borrow model.
const MEASURE_REPORTS: &str = "target/nextest/miri-measure-ci/junit-*.xml";

/// A script's commands with continued lines joined and runs of spaces made one, comments left out.
fn joined(script: &str) -> Vec<String> {
    script
        .replace("\\\n", " ")
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

/// The stop of a nextest profile, in seconds (its period times its periods), and whether it counts
/// a stopped test as passing; `None` without a slow timeout.
fn profile_stop(n: &toml::Table, profile: &str) -> Option<(i64, bool)> {
    let slow = n.get("profile")?.get(profile)?.get("slow-timeout")?;
    let period: i64 = slow
        .get("period")?
        .as_str()?
        .strip_suffix('s')?
        .parse()
        .ok()?;
    let after = slow.get("terminate-after")?.as_integer()?;
    Some((period * after, slow.get("on-timeout").is_some()))
}

/// What is wrong with how a dispatch asks for Miri's measuring run, and with `when`'s handling of
/// it: only a dispatch's input sets it, and it turns every gate off.
fn measure_input_problems(w: &Yaml) -> Vec<String> {
    let mut found = Vec::new();
    let inputs = &w["on"]["workflow_dispatch"]["inputs"];
    let options: Vec<&str> = inputs["miri_measure"]["options"]
        .as_vec()
        .map(|v| v.iter().filter_map(Yaml::as_str).collect())
        .unwrap_or_default();
    if inputs["miri_measure"]["type"].as_str() != Some("choice")
        || inputs["miri_measure"]["default"].as_str() != Some("none")
        || inputs["miri_measure"]["required"].as_bool() != Some(false)
        || options != ["none", "new", "all"]
    {
        found.push("the input miri_measure is not a choice of none, new and all".to_owned());
    }
    if inputs["miri_measure_tests"]["type"].as_str() != Some("string")
        || inputs["miri_measure_tests"]["default"].as_str() != Some("")
    {
        found.push("the input miri_measure_tests is not empty unless given".to_owned());
    }
    let when = &w["jobs"]["when"];
    let decide = when["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .find(|s| s["id"].as_str() == Some("when"))
        .cloned()
        .unwrap_or(Yaml::BadValue);
    if decide["env"]["INPUT_MIRI_MEASURE"].as_str() != Some("${{ inputs.miri_measure }}")
        || decide["env"]["INPUT_MIRI_MEASURE_TESTS"].as_str()
            != Some("${{ inputs.miri_measure_tests }}")
    {
        found.push("`when` does not read the inputs from its environment".to_owned());
    }
    let script = decide["run"].as_str().unwrap_or("");
    // The dispatch's branch of `when`'s case on the event, up to that case's end (YAML strips the
    // script's indent, so that case starts its lines and its branches are indented once).
    let dispatch = script.find("\n  workflow_dispatch)\n");
    let end = dispatch.and_then(|d| script[d..].find("\nesac\n").map(|e| d + e));
    let inside = |at: Option<usize>| matches!((dispatch, at, end), (Some(d), Some(a), Some(e)) if d < a && a < e);
    if !script.contains("measure=\"\" measure_tests=\"\"")
        || !inside(script.find("measure=\"$INPUT_MIRI_MEASURE\""))
    {
        found.push(
            "the measuring run runs without the input: `when` sets it outside a dispatch"
                .to_owned(),
        );
    }
    let branch = script.find("new | all)");
    let gates_off = branch.is_some_and(|b| {
        let rest = &script[b..];
        rest[..rest.find(";;").unwrap_or(0)].contains("mutants=false coverage=false miri=false")
    });
    if !inside(branch) || !gates_off {
        found.push("a measuring run runs the gates too".to_owned());
    }
    for output in ["miri_measure", "miri_measure_tests"] {
        let from = if output == "miri_measure" {
            "echo \"miri_measure=$measure\""
        } else {
            "echo \"miri_measure_tests=$measure_tests\""
        };
        if when["outputs"][output].as_str()
            != Some(format!("${{{{ steps.when.outputs.{output} }}}}").as_str())
            || !script.contains(from)
        {
            found.push(format!("`when` does not hand on {output}"));
        }
    }
    found
}

/// What is wrong with the measuring run's jobs: when they run, what they hand on, and that they
/// leave nothing another run reads.
fn measure_job_problems(w: &Yaml) -> Vec<String> {
    let jobs = &w["jobs"];
    let mut found = Vec::new();
    let plan = &jobs["miri-measure-plan"];
    if plan["if"].as_str() != Some("needs.when.outputs.miri_measure != ''") {
        found.push("the measuring run runs without the input: the plan's condition".to_owned());
    }
    if plan["env"]["MEASURE"].as_str() != Some("${{ needs.when.outputs.miri_measure }}")
        || plan["env"]["MEASURE_TESTS"].as_str()
            != Some("${{ needs.when.outputs.miri_measure_tests }}")
    {
        found.push("the plan does not read `when`'s choice".to_owned());
    }
    let plan_runs = joined(&runs(w, "miri-measure-plan")).join("\n");
    for needle in [
        "cargo miri nextest list --locked $PACKAGES --message-format json > \
         \"$RUNNER_TEMP/measure/miri-list.json\"",
        "cargo miri nextest list --locked $PACKAGES -E \"$MEASURE_TESTS\" --message-format json \
         > \"$RUNNER_TEMP/measure/only-list.json\"",
        "kernel-quality -- miri-measure --which \"$MEASURE\" --list \
         \"$RUNNER_TEMP/measure/miri-list.json\" \"${only[@]}\"",
    ] {
        if !plan_runs.contains(needle) {
            found.push(format!("the plan does not run {needle:?}"));
        }
    }
    for (job, list, count) in MEASURE_PARTS {
        let part = &jobs[job];
        let after_plan = part["needs"]
            .as_vec()
            .is_some_and(|v| v.iter().any(|j| j.as_str() == Some("miri-measure-plan")));
        if part["if"].as_str()
            != Some(format!("needs.miri-measure-plan.outputs.{count} != '0'").as_str())
            || !after_plan
        {
            found.push(format!(
                "the measuring run runs without the input: {job}'s condition"
            ));
        }
        if part["strategy"]["matrix"]["part"].as_str()
            != Some(format!("${{{{ fromJSON(needs.miri-measure-plan.outputs.{list}) }}}}").as_str())
            || part["strategy"]["fail-fast"].as_bool() != Some(false)
        {
            found.push(format!("{job} does not run every one of the plan's parts"));
        }
        for (out, how) in [(list, "-c"), (count, "-r")] {
            let echo =
                format!("echo \"{out}=$(jq {how} .{out} <<<\"$plan\")\" >> \"$GITHUB_OUTPUT\"");
            if plan["outputs"][out].as_str()
                != Some(format!("${{{{ steps.plan.outputs.{out} }}}}").as_str())
                || !plan_runs.contains(&echo)
            {
                found.push(format!("the plan does not hand on {out}"));
            }
        }
    }
    // The second part job is the first, word for word, but for its matrix and condition.
    for key in ["runs-on", "timeout-minutes", "env", "defaults", "steps"] {
        if jobs["miri-measure-2"][key] != jobs["miri-measure"][key] {
            found.push(format!("miri-measure-2's {key} is not miri-measure's"));
        }
    }
    found.extend(measure_leaves_problems(w));
    found
}

/// What the measuring run's jobs leave that another run could read: a cache saved, the table
/// written, anything pushed, an artifact kept more than a few days. The cache they restore is the
/// one the job `miri-plan` saves, named by its key alone.
fn measure_leaves_problems(w: &Yaml) -> Vec<String> {
    let jobs = &w["jobs"];
    let mut found = Vec::new();
    let is_cache = |s: &&Yaml| {
        s["uses"]
            .as_str()
            .is_some_and(|u| u.starts_with("Swatinem/rust-cache@"))
    };
    let plan_cache = jobs["miri-plan"]["steps"]
        .as_vec()
        .and_then(|v| v.iter().find(is_cache))
        .filter(|s| s["with"]["add-job-id-key"].as_bool() == Some(false));
    let miri_cache = plan_cache.and_then(|s| s["with"]["key"].as_str());
    for job in MEASURE_JOBS {
        for step in jobs[job]["steps"].as_vec().into_iter().flatten() {
            let uses = step["uses"].as_str().unwrap_or("");
            if (is_cache(&step) && step["with"]["save-if"].as_bool() != Some(false))
                || uses.starts_with("actions/cache")
            {
                found.push(format!("{job} saves a cache"));
            }
            if is_cache(&step)
                && (step["with"]["key"].as_str() != miri_cache
                    || miri_cache.is_none()
                    || step["with"]["add-job-id-key"].as_bool() != Some(false))
            {
                found.push(format!("{job} does not restore the job miri-plan's cache"));
            }
            if uses.starts_with("actions/upload-artifact@")
                && step["with"]["retention-days"]
                    .as_i64()
                    .is_none_or(|d| !(1..=7).contains(&d))
            {
                found.push(format!("{job} keeps an artifact more than a few days"));
            }
        }
        let writes = joined(&runs(w, job)).iter().any(|c| {
            (c.contains("miri-times") && !c.contains("--dry-run"))
                || c.contains("git push")
                || c.contains("git commit")
        });
        if writes {
            found.push(format!("{job} writes the table"));
        }
    }
    found
}

/// What is wrong with a measuring part: the measuring profile at the budget on its own tests,
/// geom-pool's under Tree Borrows as the gate runs them and the rest under Stacked Borrows, any
/// failure but a test's passed on, its reports uploaded whatever happened, then every test read,
/// so one that failed under Miri fails the part, named.
fn measure_part_problems(w: &Yaml, n: &toml::Table) -> Vec<String> {
    let mut found = Vec::new();
    let budget = i64::try_from(miri::BUDGET).unwrap();
    let inherits = n
        .get("profile")
        .and_then(|p| p.get("miri-measure-ci"))
        .and_then(|p| p.get("inherits"))
        .and_then(toml::Value::as_str);
    let stop = profile_stop(n, "miri-measure-ci");
    if inherits != Some("miri-measure") || stop != Some((budget, false)) {
        found.push(format!(
            "the miri-measure-ci profile does not stop a test at the budget as timed out: {stop:?}"
        ));
    }
    let measure = n.get("profile").and_then(|p| p.get("miri-measure"));
    let fail_fast = measure
        .and_then(|p| p.get("fail-fast"))
        .and_then(toml::Value::as_bool);
    let junit = measure
        .and_then(|p| p.get("junit"))
        .and_then(|j| j.get("path"))
        .and_then(toml::Value::as_str);
    if fail_fast != Some(false)
        || junit != Some("junit.xml")
        || profile_stop(n, "miri-measure").is_none_or(|(_, pass)| pass)
    {
        found.push("the miri-measure profile does not report every test as it ended".to_owned());
    }
    let steps = w["jobs"]["miri-measure"]["steps"]
        .as_vec()
        .cloned()
        .unwrap_or_default();
    let running = |s: &&Yaml, what: &str| s["run"].as_str().is_some_and(|r| r.contains(what));
    let run_at = steps
        .iter()
        .position(|s| running(&s, "cargo miri nextest run --locked $PACKAGES"));
    let pool_at = steps
        .iter()
        .position(|s| running(&s, "cargo miri nextest run --locked -p geom-pool"));
    let check_at = steps.iter().position(|s| running(&s, "miri-times"));
    let upload_at = steps.iter().position(|s| {
        s["uses"]
            .as_str()
            .is_some_and(|u| u.starts_with("actions/upload-artifact@"))
            && s["with"]["path"].as_str() == Some(MEASURE_REPORTS)
            && s["with"]["name"].as_str() == Some("miri-measure-${{ matrix.part }}")
            && s["if"].as_str() == Some("${{ !cancelled() }}")
    });
    match (run_at, pool_at, upload_at, check_at) {
        (Some(run), Some(pool), Some(upload), Some(check))
            if run < upload && pool < upload && upload < check => {}
        _ => found.push(format!(
            "a part uploads no report of {MEASURE_REPORTS} between its runs and its check: \
             {run_at:?} {pool_at:?} {upload_at:?} {check_at:?}"
        )),
    }
    // geom-pool's tests are measured as the gate's job `miri-pool` runs them.
    let pool_flags = w["jobs"]["miri-pool"]["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .find(|s| running(s, "cargo miri nextest run --locked -p geom-pool"))
        .and_then(|s| s["env"]["MIRIFLAGS"].as_str());
    let measured_flags = pool_at
        .and_then(|i| steps.get(i))
        .and_then(|s| s["env"]["MIRIFLAGS"].as_str());
    if pool_flags.is_none() || measured_flags != pool_flags {
        found.push(format!(
            "a part does not measure geom-pool under Tree Borrows as miri-pool runs it: \
             {measured_flags:?} against {pool_flags:?}"
        ));
    }
    let commands = joined(&runs(w, "miri-measure")).join("\n");
    let status = "case \"$status\" in 0|100) ;; *) exit \"$status\" ;; esac";
    if commands.matches(status).count() != 2 {
        found.push(format!("a part does not run {status:?} after each run"));
    }
    for needle in [
        "cargo miri nextest run --locked $PACKAGES --profile miri-measure-ci -E \"not \
         package(geom-pool) & ($(cat \"$RUNNER_TEMP/measure/part-${{ matrix.part }}.txt\"))\" || \
         status=$?"
            .to_owned(),
        "cargo miri nextest run --locked -p geom-pool --profile miri-measure-ci -E \
         \"package(geom-pool) & ($(cat \"$RUNNER_TEMP/measure/part-${{ matrix.part }}.txt\"))\" || \
         status=$?"
            .to_owned(),
        "if [ -f \"$dir/junit.xml\" ]; then mv \"$dir/junit.xml\" \"$dir/junit-stacked-borrows.xml\"; fi"
            .to_owned(),
        "if [ -f \"$dir/junit.xml\" ]; then mv \"$dir/junit.xml\" \"$dir/junit-tree-borrows.xml\"; fi"
            .to_owned(),
        format!("for f in {MEASURE_REPORTS}; do reports+=(--report \"$f\"); done"),
        format!(
            "kernel-quality -- miri-times \"${{reports[@]}}\" --scale 1 --stop {budget} --dry-run"
        ),
    ] {
        if !commands.contains(&needle) {
            found.push(format!("a part does not run {needle:?}"));
        }
    }
    found
}

/// What is wrong with Miri's measuring run, given the workflow's text and `.config/nextest.toml`'s.
fn measure_problems(text: &str, nextest: &str) -> Vec<String> {
    let w = YamlLoader::load_from_str(text).unwrap().remove(0);
    let n = nextest.parse::<toml::Table>().unwrap();
    let mut found = measure_input_problems(&w);
    found.extend(measure_job_problems(&w));
    found.extend(measure_part_problems(&w, &n));
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn a_measuring_run_runs_only_when_asked_and_leaves_nothing_behind() {
    let text = read(".github/workflows/kernel-quality.yml").replace("\r\n", "\n");
    let nextest = read(".config/nextest.toml").replace("\r\n", "\n");
    let found = measure_problems(&text, &nextest);
    assert!(found.is_empty(), "{found:#?}");
    for (from, to, said) in [
        // The measuring job running without the input, three ways.
        (
            "if: needs.when.outputs.miri_measure != ''",
            "if: ${{ always() }}",
            "runs without the input: the plan's condition",
        ),
        (
            "measure=\"\" measure_tests=\"\"",
            "measure=\"$INPUT_MIRI_MEASURE\" measure_tests=\"\"",
            "runs without the input: `when` sets it",
        ),
        (
            "if: needs.miri-measure-plan.outputs.count != '0'",
            "if: ${{ !cancelled() }}",
            "runs without the input: miri-measure's condition",
        ),
        // A measuring run that runs the gates too.
        (
            "                  mutants=false coverage=false miri=false\n                  why=\"Miri's",
            "                  why=\"Miri's",
            "runs the gates too",
        ),
        // A measuring job saving a cache.
        (
            "          # leaves nothing another run reads.\n          save-if: false\n",
            "          # leaves nothing another run reads.\n          save-if: ${{ github.ref == 'refs/heads/main' }}\n",
            "miri-measure-plan saves a cache",
        ),
        (
            "          # leaves nothing another run reads.\n          save-if: false\n          key: miri\n",
            "          # leaves nothing another run reads.\n          save-if: false\n          key: miri-miri\n",
            "miri-measure-plan does not restore the job miri-plan's cache",
        ),
        // The table written.
        (
            "--scale 1 --stop 600 --dry-run",
            "--scale 1 --stop 600",
            "miri-measure writes the table",
        ),
        // A part with no report upload, and one kept for months.
        (
            "          path: target/nextest/miri-measure-ci/junit-*.xml\n",
            "          path: target/nextest/miri-measure/junit.xml\n",
            "a part uploads no report",
        ),
        (
            "          path: target/nextest/miri-measure-ci/junit-*.xml\n          retention-days: 3\n",
            "          path: target/nextest/miri-measure-ci/junit-*.xml\n          retention-days: 90\n",
            "miri-measure keeps an artifact more than a few days",
        ),
        // geom-pool measured under Stacked Borrows, not as the gate runs it.
        (
            "          # The flags of the job `miri-pool`, so the times are that run's.\n          \
             MIRIFLAGS: -Zmiri-isolation-error=warn-nobacktrace -Zmiri-tree-borrows -Zmiri-ignore-leaks\n",
            "          # The flags of the job `miri-pool`, so the times are that run's.\n          \
             MIRIFLAGS: -Zmiri-isolation-error=warn-nobacktrace\n",
            "does not measure geom-pool under Tree Borrows",
        ),
        // geom-pool's tests left out of the measuring run.
        (
            "          cargo miri nextest run --locked -p geom-pool --profile miri-measure-ci",
            "          true --locked -p geom-pool --profile miri-measure-ci",
            "a part uploads no report",
        ),
        // A build that failed, read as a measured run.
        (
            "case \"$status\" in 0|100) ;; *) exit \"$status\" ;; esac",
            "true",
            "a part does not run",
        ),
    ] {
        let planted = text.replacen(from, to, 1);
        assert_ne!(planted, text, "the planted edit {from:?} applies");
        let found = measure_problems(&planted, &nextest);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
    // A stop other than the budget, or one that counts a stopped test as passing.
    let ci =
        "inherits = \"miri-measure\"\nslow-timeout = { period = \"300s\", terminate-after = 2 }";
    for to in [
        "inherits = \"miri-measure\"\nslow-timeout = { period = \"60s\", terminate-after = 2 }",
        "inherits = \"miri-measure\"\nslow-timeout = { period = \"300s\", terminate-after = 2, \
         on-timeout = \"pass\" }",
        "slow-timeout = { period = \"300s\", terminate-after = 2 }",
    ] {
        let planted = nextest.replacen(ci, to, 1);
        assert_ne!(planted, nextest, "the planted edit {to:?} applies");
        let found = measure_problems(&text, &planted);
        assert!(
            found
                .iter()
                .any(|p| p.contains("does not stop a test at the budget")),
            "{to}: {found:#?}"
        );
    }
}

/// What is wrong with how a dispatch asks for Miri's gate alone (`-f gate=miri`): only a dispatch
/// reads the input, through `when`'s environment, its default runs every gate, and `miri` turns
/// every other gate off and leaves Miri's on.
fn gate_input_problems(text: &str) -> Vec<String> {
    let w = YamlLoader::load_from_str(text).unwrap().remove(0);
    let mut found = Vec::new();
    let gate = &w["on"]["workflow_dispatch"]["inputs"]["gate"];
    let options: Vec<&str> = gate["options"]
        .as_vec()
        .map(|v| v.iter().filter_map(Yaml::as_str).collect())
        .unwrap_or_default();
    if gate["type"].as_str() != Some("choice")
        || gate["default"].as_str() != Some("all")
        || gate["required"].as_bool() != Some(false)
        || options != ["all", "miri"]
    {
        found.push("the input gate is not a choice of all and miri, all by default".to_owned());
    }
    // The input reaches the run in two places only: the concurrency group (a group of its own)
    // and `when`'s environment.
    for line in text.lines().filter(|l| l.contains("inputs.gate")) {
        let line = line.trim();
        if line != "INPUT_GATE: ${{ inputs.gate }}" && !line.starts_with("group: kernel-quality-") {
            found.push(format!(
                "the input gate is read outside `when`'s choice: {line}"
            ));
        }
    }
    let decide = w["jobs"]["when"]["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .find(|s| s["id"].as_str() == Some("when"))
        .cloned()
        .unwrap_or(Yaml::BadValue);
    if decide["env"]["INPUT_GATE"].as_str() != Some("${{ inputs.gate }}") {
        found.push("`when` does not read the input gate from its environment".to_owned());
    }
    let script = decide["run"].as_str().unwrap_or("");
    // The dispatch's branch of `when`'s case on the event (YAML strips the script's indent, so
    // that case starts its lines and its branches are indented once).
    let dispatch = script.find("\n  workflow_dispatch)\n");
    let end = dispatch.and_then(|d| script[d..].find("\nesac\n").map(|e| d + e));
    let reads: Vec<usize> = script
        .match_indices("$INPUT_GATE")
        .map(|(at, _)| at)
        .collect();
    let inside = |at: usize| matches!((dispatch, end), (Some(d), Some(e)) if d < at && at < e);
    if reads.is_empty() || !reads.iter().all(|&at| inside(at)) {
        found.push("`when` reads the input gate outside a dispatch".to_owned());
    }
    if !script.contains("case \"$INPUT_GATE\" in\n      all) ;;\n") {
        found.push("the default, all, does not run every gate as a dispatch runs them".to_owned());
    }
    let alone = script.find("\n      miri)\n").map(|at| {
        let rest = &script[at..];
        rest[..rest.find(";;").unwrap_or(0)].to_owned()
    });
    match alone {
        Some(branch) if reads.first().is_some_and(|&r| inside(r)) => {
            if !branch.contains("mutants=false coverage=false") {
                found.push("Miri's gate alone runs another gate".to_owned());
            }
            if branch.contains("miri=false") {
                found.push("Miri's gate alone does not run Miri".to_owned());
            }
        }
        _ => found.push("`when` has no branch for Miri's gate alone".to_owned()),
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn a_dispatch_alone_can_run_miris_gate_alone() {
    let text = read(".github/workflows/kernel-quality.yml").replace("\r\n", "\n");
    let found = gate_input_problems(&text);
    assert!(found.is_empty(), "{found:#?}");
    for (from, to, said) in [
        // A default other than every gate, and a choice that is not the two.
        (
            "        default: all\n        type: choice\n        options: [all, miri]\n",
            "        default: miri\n        type: choice\n        options: [all, miri]\n",
            "not a choice of all and miri, all by default",
        ),
        (
            "        options: [all, miri]\n",
            "        options: [all, miri, coverage]\n",
            "not a choice of all and miri, all by default",
        ),
        // The input read by another event: in `when`'s script before the case on the event, and
        // in another step's environment.
        (
            "          why=\"every gate runs whole\"\n",
            "          why=\"every gate runs whole\"\n          \
             if [ \"$INPUT_GATE\" = miri ]; then mutants=false coverage=false; fi\n",
            "reads the input gate outside a dispatch",
        ),
        (
            "          BASE: ${{ steps.when.outputs.base }}\n",
            "          BASE: ${{ steps.when.outputs.base }}\n          GATE: ${{ inputs.gate }}\n",
            "read outside `when`'s choice",
        ),
        (
            "          INPUT_GATE: ${{ inputs.gate }}\n",
            "          INPUT_GATE: ${{ inputs.gate || 'miri' }}\n",
            "does not read the input gate from its environment",
        ),
        // The default changing what a dispatch runs.
        (
            "                all) ;;\n",
            "                all) coverage=false ;;\n",
            "the default, all, does not run every gate",
        ),
        // Miri's gate alone running another gate, or not Miri.
        (
            "                  mutants=false coverage=false\n                  why=\"Miri's gate alone\"",
            "                  mutants=false\n                  why=\"Miri's gate alone\"",
            "Miri's gate alone runs another gate",
        ),
        (
            "                  mutants=false coverage=false\n                  why=\"Miri's gate alone\"",
            "                  mutants=false coverage=false miri=false\n                  why=\"Miri's gate alone\"",
            "Miri's gate alone does not run Miri",
        ),
    ] {
        let planted = text.replacen(from, to, 1);
        assert_ne!(planted, text, "the planted edit {from:?} applies");
        let found = gate_input_problems(&planted);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}

/// The lines the step of a Miri gate job that reads its report for stopped tests must hold: the
/// gate's stop and profile, a pull request's, the tool's run on that profile's report, and its
/// Markdown into the job's summary.
const STOPPED_LINES: [&str; 4] = [
    "stop=600 profile=default-miri",
    "stop=60 profile=miri-smoke",
    "kernel-quality miri-stopped --stop \"$stop\" --report \"target/nextest/$profile/junit.xml\"",
    "| tee -a \"$GITHUB_STEP_SUMMARY\"",
];

/// The plan's hand-over of the tool, and the line that puts it in place in a Miri job.
const TOOL_HANDOVER: &str =
    "cp target/debug/kernel-quality \"$RUNNER_TEMP/miri-plan/kernel-quality\"";
const TOOL_INSTALL: &str =
    "install -m 755 \"$RUNNER_TEMP/miri-plan/kernel-quality\" \"$HOME/.cargo/bin/kernel-quality\"";

/// Each Miri gate job and the command of its tests' step.
const STOPPED_JOBS: [(&str, &str); 2] = [
    ("miri", "cargo miri nextest run --locked $PACKAGES"),
    ("miri-pool", "cargo miri nextest run --locked -p geom-pool"),
];

/// What is wrong with how the Miri gate's jobs treat a test nextest stopped with no verdict (32.68),
/// given the workflow's and nextest's configuration: the plan hands on the tool; each gate job puts
/// it in place and, in the step right after its tests, whatever they did, reads the report of the
/// profile it ran, at that profile's stop, into its summary, and nothing hides that step's failure.
fn miri_stop_problems(text: &str, nextest: &str) -> Vec<String> {
    let w = YamlLoader::load_from_str(text).unwrap().remove(0);
    let n = nextest.parse::<toml::Table>().unwrap();
    let mut found = Vec::new();
    if !joined(&runs(&w, "miri-plan"))
        .iter()
        .any(|c| c == TOOL_HANDOVER)
    {
        found.push("miri-plan does not hand on kernel-quality".to_owned());
    }
    for (job, tests) in STOPPED_JOBS {
        if !joined(&runs(&w, job))
            .iter()
            .any(|c| c.trim() == TOOL_INSTALL)
        {
            found.push(format!(
                "{job} does not put the plan's kernel-quality in place"
            ));
        }
        let steps: Vec<Yaml> = w["jobs"][job]["steps"]
            .as_vec()
            .cloned()
            .unwrap_or_default();
        let run_of = |s: &Yaml| joined(s["run"].as_str().unwrap_or("")).join("\n");
        let at_tests = steps.iter().position(|s| run_of(s).contains(tests));
        let at_check = steps
            .iter()
            .position(|s| run_of(s).contains("kernel-quality miri-stopped"));
        let (Some(t), Some(c)) = (at_tests, at_check) else {
            found.push(format!(
                "{job} does not read its report for tests stopped with no verdict"
            ));
            continue;
        };
        if c != t + 1 {
            found.push(format!(
                "{job} does not read its report right after its tests"
            ));
        }
        let check = &steps[c];
        if !check["if"]
            .as_str()
            .is_some_and(|i| i.starts_with("${{ !cancelled()"))
        {
            found.push(format!(
                "{job} does not read its report whatever its tests did"
            ));
        }
        let script = run_of(check);
        for line in STOPPED_LINES {
            if !script.contains(line) {
                found.push(format!("{job}'s report is not read as {line:?}"));
            }
        }
        if !check["continue-on-error"].is_badvalue()
            || script.contains("||")
            || script.contains("set +e")
        {
            found.push(format!("{job}'s reading of its report cannot fail the job"));
        }
    }
    // The stops the step names are the profiles' own, and both count a stopped test as passing,
    // which is why the step reads the report at all.
    for (profile, stop) in [("default-miri", 600), ("miri-smoke", 60)] {
        if profile_stop(&n, profile) != Some((stop, true)) {
            found.push(format!(
                "nextest's {profile} profile does not stop a test at {stop} s as the step reads it"
            ));
        }
    }
    if i64::try_from(miri::BUDGET).ok() != Some(600) {
        found.push("the budget is not the default-miri profile's stop".to_owned());
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn a_miri_job_names_every_test_stopped_with_no_verdict_and_fails_on_one_that_may_not_stop() {
    let text = read(".github/workflows/kernel-quality.yml").replace("\r\n", "\n");
    let nextest = read(".config/nextest.toml");
    let found = miri_stop_problems(&text, &nextest);
    assert!(found.is_empty(), "{found:#?}");
    let check = "      - name: Tests stopped with no verdict (partition ${{ matrix.partition }})\n        \
                 if: ${{ !cancelled() }}\n";
    let hidden = format!("{check}        continue-on-error: true\n");
    // Seeded violations: each must be reported.
    for (from, to, said, in_nextest) in [
        // The plan does not hand the tool on.
        (
            TOOL_HANDOVER,
            "true",
            "does not hand on kernel-quality",
            false,
        ),
        // A partition without the tool.
        (
            "          install -m 755 \"$RUNNER_TEMP/miri-plan/kernel-quality\" \"$HOME/.cargo/bin/kernel-quality\"\n          packages",
            "          packages",
            "miri does not put the plan's kernel-quality in place",
            false,
        ),
        // A partition that never reads its report.
        (
            "          kernel-quality miri-stopped --stop \"$stop\" --report \"target/nextest/$profile/junit.xml\" \\\n            --yardstick target/nextest/miri-yardstick/junit.xml \\\n            --title \"Partition",
            "          echo \"Partition",
            "miri does not read its report for tests stopped",
            false,
        ),
        // Read only when the tests passed.
        (
            check,
            "      - name: Tests stopped with no verdict (partition ${{ matrix.partition }})\n        \
             if: ${{ success() }}\n",
            "miri does not read its report whatever its tests did",
            false,
        ),
        // Its failure hidden, two ways.
        (
            check,
            hidden.as_str(),
            "miri's reading of its report cannot fail the job",
            false,
        ),
        (
            "--title \"Partition ${{ matrix.partition }}\" | tee -a \"$GITHUB_STEP_SUMMARY\"",
            "--title \"Partition ${{ matrix.partition }}\" | tee -a \"$GITHUB_STEP_SUMMARY\" || true",
            "miri's reading of its report cannot fail the job",
            false,
        ),
        // The gate's report read at another stop than its profile's.
        (
            "          stop=600 profile=default-miri\n",
            "          stop=900 profile=default-miri\n",
            "miri's report is not read as \"stop=600 profile=default-miri\"",
            false,
        ),
        // geom-pool's job without the reading.
        (
            "          kernel-quality miri-stopped --stop \"$stop\" --report \"target/nextest/$profile/junit.xml\" \\\n            --yardstick target/nextest/miri-yardstick/junit.xml \\\n            --title \"geom-pool",
            "          echo \"geom-pool",
            "miri-pool does not read its report for tests stopped",
            false,
        ),
        // The gate's profile stopping a test as a failure, or later, than the step reads.
        (
            "slow-timeout = { period = \"300s\", terminate-after = 2, on-timeout = \"pass\" }",
            "slow-timeout = { period = \"300s\", terminate-after = 3, on-timeout = \"pass\" }",
            "default-miri profile does not stop a test at 600 s",
            true,
        ),
    ] {
        let (t, n) = if in_nextest {
            (text.clone(), nextest.replacen(from, to, 1))
        } else {
            (text.replacen(from, to, 1), nextest.clone())
        };
        assert!(
            t != text || n != nextest,
            "the planted edit {from:?} applies"
        );
        let found = miri_stop_problems(&t, &n);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}

/// What is wrong with the limit of each Miri gate job's tests' step (`miri`'s partitions and
/// `miri-pool`, whose tests the plan holds to a partition's seconds): it must be
/// [`miri::TESTS_STEP_MINUTES`], over the [`miri::PART_SECONDS`] the plan gives a partition's
/// listing and tests, and with the set-up ([`miri::FIXED_SECONDS`]) and the first try of the
/// report's upload within the job's limit, so an overrun ends in that step and what comes after
/// it still runs.
fn miri_tests_limit_problems(text: &str) -> Vec<String> {
    let w = YamlLoader::load_from_str(text).unwrap().remove(0);
    let run_of = |s: &Yaml| joined(s["run"].as_str().unwrap_or("")).join("\n");
    let mut found = Vec::new();
    for (name, command) in STOPPED_JOBS {
        let job = &w["jobs"][name];
        let steps = job["steps"].as_vec().cloned().unwrap_or_default();
        let Some(tests) = steps.iter().find(|s| run_of(s).contains(command)) else {
            found.push(format!("{name} has no tests' step"));
            continue;
        };
        let sigterm = format!(
            "timeout --signal=TERM --kill-after=30 {} {command}",
            miri::TESTS_RUN_SECONDS
        );
        if !run_of(tests).contains(&sigterm) {
            found.push(format!(
                "{name}'s tests are not stopped with SIGTERM after {} s",
                miri::TESTS_RUN_SECONDS
            ));
        }
        let step = tests["timeout-minutes"].as_i64();
        if step != i64::try_from(miri::TESTS_STEP_MINUTES).ok() {
            found.push(format!(
                "{name}'s tests' step is not limited to {} minutes ({step:?})",
                miri::TESTS_STEP_MINUTES
            ));
        }
        let upload = steps
            .iter()
            .find(|s| s["id"].as_str() == Some("report"))
            .and_then(|s| s["timeout-minutes"].as_i64())
            .unwrap_or(i64::MAX);
        let (Some(step), Some(whole)) = (step, job["timeout-minutes"].as_i64()) else {
            found.push(format!("{name}'s tests' step or job has no limit"));
            continue;
        };
        if step.saturating_mul(60) <= i64::try_from(miri::PART_SECONDS).unwrap_or(i64::MAX) {
            found.push(format!(
                "{name}'s tests' step may stop a partition the plan holds"
            ));
        }
        let fixed = i64::try_from(miri::FIXED_SECONDS).unwrap_or(i64::MAX);
        if step
            .saturating_add(upload)
            .saturating_mul(60)
            .saturating_add(fixed)
            > whole.saturating_mul(60)
        {
            found.push(format!(
                "{name}'s tests' step, set-up and first upload do not fit the job's limit"
            ));
        }
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn a_partition_that_overruns_fails_in_its_tests_step_with_its_log_kept() {
    let text = read(".github/workflows/kernel-quality.yml").replace("\r\n", "\n");
    let found = miri_tests_limit_problems(&text);
    assert!(found.is_empty(), "{found:#?}");
    let limited = "      - name: Every kernel test Miri finishes (partition ${{ matrix.partition }})\n        \
                   timeout-minutes: 24\n";
    let named =
        "      - name: Every kernel test Miri finishes (partition ${{ matrix.partition }})\n";
    let pool_if = "      - name: geom-pool's tests under Miri with Tree Borrows\n        if: env.REACHED \
                   == '' || contains(format('{0} ', env.REACHED), '-p geom-pool ')\n";
    let pool = format!("{pool_if}        timeout-minutes: 24\n");
    let bound = "        timeout-minutes: 3\n";
    let pool_upload = format!(
        "        if: ${{{{ !cancelled() }}}}\n{bound}        with:\n          name: miri-pool\n"
    );
    let term = "          timeout --signal=TERM --kill-after=30 1380 \\\n            cargo miri nextest run";
    let pool_term = format!("{term} --locked -p geom-pool");
    // Seeded violations: each must be reported.
    for (from, to, said) in [
        (
            term,
            "          cargo miri nextest run",
            "miri's tests are not stopped with SIGTERM after 1380 s",
        ),
        (
            &pool_term as &str,
            &pool_term.replace("1380", "1500") as &str,
            "miri-pool's tests are not stopped with SIGTERM after 1380 s",
        ),
        (
            limited,
            named,
            "miri's tests' step is not limited to 24 minutes",
        ),
        (
            limited,
            &limited.replace("24", "30") as &str,
            "miri's tests' step, set-up and first upload do not fit the job's limit",
        ),
        (
            limited,
            &limited.replace("24", "13") as &str,
            "miri's tests' step may stop a partition the plan holds",
        ),
        (
            &pool as &str,
            pool_if,
            "miri-pool's tests' step is not limited to 24 minutes",
        ),
        (
            &pool,
            &pool.replace("24", "13") as &str,
            "miri-pool's tests' step may stop a partition the plan holds",
        ),
        (
            &pool_upload,
            &pool_upload.replace(bound, "") as &str,
            "miri-pool's tests' step, set-up and first upload do not fit the job's limit",
        ),
    ] {
        let planted = text.replacen(from, to, 1);
        assert_ne!(planted, text, "the planted edit {to:?} applies");
        let found = miri_tests_limit_problems(&planted);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}

/// A nextest report of one test of `binary` and `test` that passed after `seconds`, as nextest
/// writes one for a test its `on-timeout = "pass"` stopped (a plain pass at the stop).
fn one_test_report(binary: &str, test: &str, seconds: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites name=\"nextest-run\" tests=\"1\" \
         failures=\"0\" errors=\"0\"><testsuite name=\"{binary}\" tests=\"1\" disabled=\"0\" \
         errors=\"0\" failures=\"0\"><testcase name=\"{test}\" classname=\"{binary}\" \
         timestamp=\"2026-11-18T00:00:00.000+00:00\" time=\"{seconds}\"></testcase></testsuite>\
         </testsuites>\n"
    )
}

/// `kernel-quality miri-stopped` on a report of one test: its exit code and what it printed.
fn stopped_tool(binary: &str, test: &str, seconds: &str, stop: u64) -> (i32, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("miri-stopped-{stop}-{test}"));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{test}.xml"));
    std::fs::write(&file, one_test_report(binary, test, seconds)).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_kernel-quality"))
        .args([
            "miri-stopped",
            "--stop",
            &stop.to_string(),
            "--title",
            "Planted",
            "--report",
        ])
        .arg(&file)
        .output()
        .unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "runs the kernel-quality binary, which a WebAssembly test run cannot"
)]
#[test]
fn a_test_off_the_list_stopped_at_the_budget_turns_its_job_red() {
    let rows = miri::parse_table(miri::TIMES).unwrap();
    // Planted faults: a test with no place on the list, and one whose row is well within the
    // budget, each stopped at the budget, fail the job and are named as stopped.
    let sure = miri::finishing(&rows)
        .into_iter()
        .find(|t| t.seconds <= 100)
        .unwrap();
    for (binary, test) in [
        ("geom-num::planted", "a_test_no_table_holds"),
        (sure.binary.as_str(), sure.test.as_str()),
    ] {
        let (code, said) = stopped_tool(binary, test, "600.019", miri::BUDGET);
        assert_eq!(code, 1, "{binary} {test}: {said}");
        assert!(
            said.contains(
                "1 test stopped at 600 s with no verdict that may not stop: this job fails"
            ) && said.contains(&format!("`{binary} {test}`")),
            "{said}"
        );
    }
    // A test on the list, stopped at the budget, does not fail its job, and is named as stopped,
    // never as a pass. With the list empty, the same holds at a pull request's short stop for a
    // test whose row a slow runner takes past it.
    let listed = miri::parse_may_stop(miri::MAY_STOP).unwrap();
    let long = miri::may_stop_at(&rows, 60).remove(0);
    let mut cases = vec![(long.binary.clone(), long.test.clone(), "60.004", 60)];
    if let Some((b, t)) = listed.first() {
        cases.push((b.clone(), t.clone(), "600.019", miri::BUDGET));
    }
    for (binary, test, seconds, stop) in cases {
        let (code, said) = stopped_tool(&binary, &test, seconds, stop);
        assert_eq!(code, 0, "{binary} {test}: {said}");
        assert!(
            said.contains(&format!("1 test stopped at {stop} s with no verdict**"))
                && said.contains(&format!("`{binary} {test}`"))
                && !said.contains("this job fails"),
            "{said}"
        );
    }
    // A test that finished under the stop is a plain verdict, even one on the list; and the
    // same report read at the budget fails a test off the list but not at the short stop.
    let (code, said) = stopped_tool(&sure.binary, &sure.test, "599.990", miri::BUDGET);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("No test stopped at 600 s"), "{said}");
    let (code, _) = stopped_tool(&long.binary, &long.test, "600.019", miri::BUDGET);
    assert_eq!(
        code,
        i32::from(!listed.contains(&(long.binary.clone(), long.test.clone())))
    );
    // A stop past the budget is refused.
    let (code, _) = stopped_tool(&sure.binary, &sure.test, "1.0", miri::BUDGET + 1);
    assert_eq!(code, 2);
    // A tests' step that ran out of its time limit before nextest wrote the report: the job
    // fails, and its summary says there is no report.
    let none = Path::new(env!("CARGO_TARGET_TMPDIR")).join("miri-stopped-no-report/junit.xml");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_kernel-quality"))
        .args([
            "miri-stopped",
            "--stop",
            "600",
            "--title",
            "Planted",
            "--report",
        ])
        .arg(&none)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{said}");
    assert!(
        said.contains("### Planted: tests stopped with no verdict")
            && said.contains("**No report: this job fails.**")
            && said.contains("is not there"),
        "{said}"
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_tests_that_may_stop_are_the_tables_rows_over_375_s_and_never_more() {
    let rows = miri::parse_table(miri::TIMES).unwrap();
    // The list is what the tool writes from the table, never edited by hand.
    assert_eq!(
        miri::MAY_STOP.replace("\r\n", "\n"),
        miri::render_may_stop(&rows),
        "{} is not what `kernel-quality miri-may-stop` writes from Miri's table",
        miri::MAY_STOP_PATH
    );
    let listed = miri::parse_may_stop(miri::MAY_STOP).unwrap();
    // 375 s is the budget over the plan's headroom: a row over it, on a runner 1.6 times
    // slower, reaches the stop, and a row at it does not.
    assert_eq!(
        miri::SURE_WITHIN * miri::SLOWER.0,
        miri::BUDGET * miri::SLOWER.1
    );
    assert_eq!(miri::planned(miri::SURE_WITHIN), miri::BUDGET);
    let over = |s: u64| {
        let row = format!("{s}\tgeom-num::planted\tplanted\n");
        !miri::may_stop_at(&miri::parse_table(&row).unwrap(), miri::BUDGET).is_empty()
    };
    assert!(!over(miri::SURE_WITHIN) && over(miri::SURE_WITHIN + 1));
    // The list may only shrink: more than MAY_STOP_MOST fails, and so does fewer, until the
    // change that shrank it lowers MAY_STOP_MOST to match.
    assert!(
        listed.len() <= miri::MAY_STOP_MOST,
        "the list of tests that may stop at the budget grew to {}, over {}: resize each new one \
         under cfg(miri) so Miri finishes it within {} s",
        listed.len(),
        miri::MAY_STOP_MOST,
        miri::SURE_WITHIN
    );
    assert_eq!(
        listed.len(),
        miri::MAY_STOP_MOST,
        "the list shrank to {}: lower geom_test::miri::MAY_STOP_MOST to it",
        listed.len()
    );
    // Planted faults: a row added by hand, and a list written from another table, are refused.
    let mut planted = miri::MAY_STOP.replace("\r\n", "\n");
    planted.push_str("400\tgeom-num::planted\tplanted\n");
    assert!(miri::allowed_stops(&rows, &planted, miri::BUDGET).is_err());
    let mut grown = rows.clone();
    grown.push(miri::Row {
        binary: "geom-num::planted".into(),
        test: "planted".into(),
        state: miri::State::Finishes(400),
    });
    let grown_list = miri::render_may_stop(&grown);
    assert_eq!(
        miri::parse_may_stop(&grown_list).unwrap().len(),
        listed.len() + 1
    );
    assert!(miri::allowed_stops(&rows, &grown_list, miri::BUDGET).is_err());
    assert!(miri::allowed_stops(&rows, miri::MAY_STOP, miri::BUDGET).is_ok());
    assert!(miri::parse_may_stop("375\tgeom-num::planted\tplanted\n").is_err());
}

/// A Reported of `binary test` with `outcome`, for the yardstick's tests (32.78).
fn reported(binary: &str, test: &str, outcome: miri::Outcome) -> miri::Reported {
    miri::Reported {
        binary: binary.to_owned(),
        test: test.to_owned(),
        outcome,
    }
}

#[test]
fn a_jobs_scale_is_its_yardstick_over_its_row_held_between_half_and_three() {
    let rows = miri::parse_table(miri::TIMES).unwrap();
    let (yb, yt) = miri::YARDSTICK;
    // The yardstick has its own finishing row, which every scale is counted against.
    let row = rows
        .iter()
        .find(|r| r.binary == yb && r.test == yt)
        .expect("the yardstick has a row in Miri's table");
    let miri::State::Finishes(seconds) = row.state else {
        panic!("the yardstick's row is native-only");
    };
    let ms = seconds * 1000;
    let scale_at =
        |took: u64| miri::job_scale(&rows, &[reported(yb, yt, miri::Outcome::Passed(took))]);
    let as_number = |s: (u64, u64)| miri::scale_text(s);
    // At its row's time the scale is 1; at 2.45 times it, 2.45; exactly 3 and exactly 0.5 stand.
    assert_eq!(as_number(scale_at(ms).unwrap()), "1.000");
    assert_eq!(as_number(scale_at(ms * 245 / 100).unwrap()), "2.450");
    assert_eq!(as_number(scale_at(ms * 3).unwrap()), "3.000");
    assert_eq!(as_number(scale_at(ms / 2).unwrap()), "0.500");
    // Faster than half: held at 0.5, never under it.
    assert_eq!(scale_at(ms / 2 - 1).unwrap(), miri::SCALE_LEAST);
    assert_eq!(scale_at(1).unwrap(), miri::SCALE_LEAST);
    // Past 3, by a millisecond: the runner is too slow to judge, an error and no scale.
    let slow = scale_at(ms * 3 + 1).unwrap_err();
    assert!(slow.contains("runner too slow to judge"), "{slow}");
    // A missing yardstick time is an error, never a default: not in the report, stopped, failed,
    // or no row in the table.
    assert!(miri::job_scale(&rows, &[]).is_err());
    assert!(miri::job_scale(&rows, &[reported(yb, "another", miri::Outcome::Passed(ms))]).is_err());
    assert!(miri::job_scale(&rows, &[reported(yb, yt, miri::Outcome::TimedOut(ms))]).is_err());
    assert!(
        miri::job_scale(
            &rows,
            &[reported(yb, yt, miri::Outcome::Failed("panic".into()))]
        )
        .is_err()
    );
    let without: Vec<miri::Row> = rows
        .iter()
        .filter(|r| !(r.binary == yb && r.test == yt))
        .cloned()
        .collect();
    assert!(miri::job_scale(&without, &[reported(yb, yt, miri::Outcome::Passed(ms))]).is_err());
    let native = miri::parse_table(&format!("native-only over 600\t{yb}\t{yt}\n")).unwrap();
    assert!(miri::job_scale(&native, &[reported(yb, yt, miri::Outcome::Passed(ms))]).is_err());
    let zero = miri::parse_table(&format!("0\t{yb}\t{yt}\n")).unwrap();
    assert!(miri::job_scale(&zero, &[reported(yb, yt, miri::Outcome::Passed(ms))]).is_err());
}

#[test]
fn a_tests_stop_is_its_row_times_1_6_times_the_scale_and_reaching_it_fails() {
    // The stop's arithmetic at the scales 0.5, 1, 2.45 and 3, in milliseconds, rounded up.
    assert_eq!(miri::stop_ms(100, (1, 2)), 80_000);
    assert_eq!(miri::stop_ms(100, (1, 1)), 160_000);
    assert_eq!(miri::stop_ms(100, (245, 100)), 392_000);
    assert_eq!(miri::stop_ms(100, (3, 1)), 480_000);
    assert_eq!(miri::stop_ms(1, (1, 3)), 534);
    let rows =
        miri::parse_table("100\tgeom-num\tplanted::slow\n500\tgeom-num\tplanted::long\n").unwrap();
    let one = |ms: u64, scale: (u64, u64)| {
        miri::over_stops(
            &[reported(
                "geom-num",
                "planted::slow",
                miri::Outcome::Passed(ms),
            )],
            &rows,
            scale,
            miri::BUDGET,
        )
        .over
    };
    // A planted slow test is red at scale 1 and at 2.45 once it reaches its stop, and only then.
    assert_eq!(one(159_999, (1, 1)), Vec::<String>::new());
    assert_eq!(one(160_000, (1, 1)).len(), 1);
    assert_eq!(one(391_999, (245, 100)), Vec::<String>::new());
    assert_eq!(one(392_000, (245, 100)).len(), 1);
    // A test nextest stopped is red whatever its time; one that failed is nextest's own red.
    let stopped = miri::over_stops(
        &[
            reported("geom-num", "planted::slow", miri::Outcome::TimedOut(1)),
            reported(
                "geom-num",
                "planted::long",
                miri::Outcome::Failed("panic".into()),
            ),
        ],
        &rows,
        (1, 1),
        miri::BUDGET,
    )
    .over;
    assert_eq!(stopped.len(), 1, "{stopped:?}");
    assert!(
        stopped[0].starts_with("geom-num planted::slow: 1 ms"),
        "{stopped:?}"
    );
    // A stop over the profile's own is cut to it: row 500 at scale 1 is 800 s, judged at 600.
    let long = |ms: u64| {
        miri::over_stops(
            &[reported(
                "geom-num",
                "planted::long",
                miri::Outcome::Passed(ms),
            )],
            &rows,
            (1, 1),
            miri::BUDGET,
        )
        .over
    };
    assert_eq!(long(599_999), Vec::<String>::new());
    assert_eq!(long(600_000).len(), 1);
    // A short run's stop (the pull request's 60 s) that ends a test before its own stop leaves it
    // with no verdict: named as cut, not a pass and not over; one that reached its own is over.
    let short = |test: &str, ms: u64| {
        miri::over_stops(
            &[reported("geom-num", test, miri::Outcome::Passed(ms))],
            &rows,
            (1, 1),
            60,
        )
    };
    let cut = short("planted::long", 60_000);
    assert!(cut.over.is_empty() && cut.cut.len() == 1, "{cut:?}");
    assert_eq!(short("planted::long", 59_999), miri::Judged::default());
    let quick = miri::parse_table("10\tgeom-num\tplanted::quick\n").unwrap();
    let over = miri::over_stops(
        &[reported(
            "geom-num",
            "planted::quick",
            miri::Outcome::Passed(16_000),
        )],
        &quick,
        (1, 1),
        60,
    );
    assert!(over.over.len() == 1 && over.cut.is_empty(), "{over:?}");
    // A test with no finishing row that ran is red: nothing runs under Miri without a row.
    let unknown = miri::over_stops(
        &[reported(
            "geom-num",
            "planted::none",
            miri::Outcome::Passed(1),
        )],
        &rows,
        (1, 1),
        miri::BUDGET,
    )
    .over;
    assert_eq!(unknown, ["geom-num planted::none: no finishing row"]);
}

/// What a Miri gate job's steps lack of the yardstick (32.78), one line a fault.
fn yardstick_faults(w: &Yaml, job: &str) -> Vec<String> {
    let runs: Vec<String> = w["jobs"][job]["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .map(|s| s["run"].as_str().unwrap_or("").to_owned())
        .collect();
    yardstick_faults_in(job, &runs)
}

/// [`yardstick_faults`] of a job's steps' `run:` scripts, in order (a step with none is empty).
fn yardstick_faults_in(job: &str, runs: &[String]) -> Vec<String> {
    let (yb, yt) = miri::YARDSTICK;
    let at = |needle: &str| runs.iter().position(|r| r.contains(needle));
    let mut out = Vec::new();
    let (Some(scale), Some(tests)) = (at("kernel-quality miri-scale"), at("timeout --signal=TERM"))
    else {
        return vec![format!("{job}: no yardstick step, or no tests' step")];
    };
    if scale >= tests {
        out.push(format!("{job}: the yardstick runs after the tests"));
    }
    let step = &runs[scale];
    for needle in [
        "/proc/cpuinfo".to_owned(),
        format!("cargo miri nextest run --locked -p {yb} --profile miri-yardstick"),
        format!("-E 'test(={yt})'"),
        "--report target/nextest/miri-yardstick/junit.xml".to_owned(),
    ] {
        if !step.contains(&needle) {
            out.push(format!("{job}: the yardstick step lacks {needle}"));
        }
    }
    let judged = runs.iter().any(|r| {
        r.contains("kernel-quality miri-stopped")
            && r.contains("--yardstick target/nextest/miri-yardstick/junit.xml")
    });
    if !judged {
        out.push(format!("{job}: its stops are not judged at its scale"));
    }
    out
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn every_miri_gate_job_prints_its_processor_and_judges_each_test_at_its_scale() {
    let w = workflow();
    for job in ["miri", "miri-pool"] {
        assert_eq!(yardstick_faults(&w, job), Vec::<String>::new());
    }
    // The yardstick's profile reports to its own file and counts a stopped yardstick as timed out.
    let profile = &toml_file(".config/nextest.toml")["profile"]["miri-yardstick"];
    assert_eq!(profile["junit"]["path"].as_str(), Some("junit.xml"));
    assert!(profile["slow-timeout"].get("on-timeout").is_none());
    // Planted: a job without the processor's name, one without its scale's judge, one whose
    // yardstick runs after its tests.
    let text = read(".github/workflows/kernel-quality.yml");
    let plant = |from: &str, to: &str| {
        assert!(text.contains(from), "{from}");
        let mut docs = YamlLoader::load_from_str(&text.replacen(from, to, 1)).unwrap();
        docs.remove(0)
    };
    let no_cpu = plant("grep -m1 '^model name' /proc/cpuinfo", "true");
    assert_ne!(yardstick_faults(&no_cpu, "miri"), Vec::<String>::new());
    let no_judge = plant("--yardstick target/nextest/miri-yardstick/junit.xml", "");
    assert_ne!(yardstick_faults(&no_judge, "miri"), Vec::<String>::new());
    let mut late: Vec<String> = w["jobs"]["miri-pool"]["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .map(|s| s["run"].as_str().unwrap_or("").to_owned())
        .collect();
    assert_eq!(
        yardstick_faults_in("miri-pool", &late),
        Vec::<String>::new()
    );
    let at = late.iter().position(|r| r.contains("miri-scale")).unwrap();
    let step = late.remove(at);
    late.push(step);
    assert_ne!(
        yardstick_faults_in("miri-pool", &late),
        Vec::<String>::new()
    );
}
