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
//!   with the plan's count of the whole run; Miri's partitions are 1 to n for its `PARTITIONS: n`.
//! - **Every gate is judged.** The judge reads the plan's list; cargo-mutants' exit status is passed
//!   on unless it is one the judge decides (2, survivors; 3, timeouts); coverage keeps each run's
//!   log for the judge.
//! - **The tools' configuration.** cargo-mutants mutates only kernel crates' sources and judges
//!   them by integration tests, with `--locked`; nextest's Miri profiles stop a test at its budget
//!   as a pass and never fail fast, and the profile the workflow names exists.
//! - **One aliasing exception.** Miri runs geom-pool, the only crate that names rayon, under Tree
//!   Borrows and every other crate under Stacked Borrows, and only while `Cargo.lock` holds
//!   crossbeam-epoch 0.9.21, whose `IsElement::element_of` Stacked Borrows refuses.
//! - **A later push judges what it can reach.** `geom_test::increment` judges again every file of
//!   the change whose crate's tests what changed since the last clean head reaches, held here to
//!   planted changes that must be judged (a changed line, a changed test, a removed ledger entry,
//!   a change on the base's side, a file in no crate) and ones that need not (nothing, a new test
//!   target, an added ledger entry); and the workflow finds that head in GitHub's record of the
//!   pull request's runs and narrows the plan and coverage by it, each piece shown to be checked
//!   by a planted removal.

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
            "miri",
            "done",
        ]
    );
    // Each gate runs when `when` chose it; each shard job only when the plan gives it shards, and
    // the judge even when a shard failed, so a lost shard is reported, not skipped.
    for (job, gate) in [
        ("mutants-plan", "mutants"),
        ("coverage-plan", "coverage"),
        ("coverage-parts", "coverage"),
        ("gates-seeded", "coverage"),
        ("miri", "miri"),
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
fn only_a_dispatch_narrows_the_full_run() {
    let w = workflow();
    // An optional input of a dispatch, empty unless given ...
    let files = &w["on"]["workflow_dispatch"]["inputs"]["files"];
    assert_eq!(files["required"].as_bool(), Some(false));
    assert_eq!(text(&files["default"]), "");
    assert_eq!(text(&files["type"]), "string");
    assert_eq!(keys(&w["on"]["workflow_dispatch"]["inputs"]), ["files"]);
    // ... that no caller can set (kernel-release.yml's run is whole) ...
    assert!(w["on"]["workflow_call"].is_null() || w["on"]["workflow_call"]["inputs"].is_badvalue());
    // ... with a concurrency group of its own, so it never queues behind a whole run ...
    assert_eq!(
        text(&w["concurrency"]["group"]),
        "kernel-quality-${{ github.ref }}-${{ github.event_name }}-${{ inputs.files }}"
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
    // Miri: the full matrix lists partitions 1 to PARTITIONS, and each runs its own.
    let miri = &w["jobs"]["miri"];
    let n = miri["env"]["PARTITIONS"].as_i64().unwrap();
    let matrix = text(&miri["strategy"]["matrix"]["partition"]);
    let full = matrix
        .split("|| '")
        .nth(1)
        .and_then(|rest| rest.split('\'').next())
        .unwrap();
    let listed: Vec<i64> = serde_json::from_str(full).unwrap();
    assert_eq!(listed, (1..=n).collect::<Vec<_>>());
    assert!(
        matrix.contains("github.event_name == 'pull_request' && '[1]'"),
        "{matrix}"
    );
    assert_eq!(miri["strategy"]["fail-fast"].as_bool(), Some(false));
    // Each partition runs the tests the plan of the recorded times gives it.
    let run = runs(&w, "miri");
    for needle in [
        "kernel-quality -- miri-filter",
        "--partition \"${{ matrix.partition }}\" --of \"$PARTITIONS\" > \"$RUNNER_TEMP/miri-filter.txt\"",
        "-E \"not package(geom-pool) & ($(cat \"$RUNNER_TEMP/miri-filter.txt\"))\"",
    ] {
        assert!(run.contains(needle), "{needle}\n{run}");
    }
    assert!(!run.contains("--partition \"hash:"), "{run}");
    // The recorded times split into that many partitions keep each one's tests within
    // `PART_SECONDS` at four at once (principle 8), and each filterset fits in one command-line
    // argument (Linux takes at most 128 KiB in one).
    let times = geom_test::miri::parse_times(geom_test::miri::TIMES).unwrap();
    let parts = usize::try_from(n).unwrap();
    let plan = geom_test::miri::plan(&times, parts).unwrap();
    assert!(
        plan.longest() <= geom_test::miri::PART_SECONDS,
        "{} s in the slowest of {parts} partitions",
        plan.longest()
    );
    for k in 1..=parts {
        let filter = plan.filterset(k).unwrap();
        assert!(filter.len() < 100_000, "partition {k}: {}", filter.len());
    }
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
    // Only that step turns Miri's leak check off (rayon's pools stop their threads without
    // joining them); every other run keeps it.
    for (flags, run) in &with_flags {
        if flags.contains("-Zmiri-ignore-leaks") {
            assert!(flags.contains("-Zmiri-tree-borrows"), "{run}");
        }
    }
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
        main.contains(&format!("-E \"not package({TREE_BORROWS_CRATE}) & (")),
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
