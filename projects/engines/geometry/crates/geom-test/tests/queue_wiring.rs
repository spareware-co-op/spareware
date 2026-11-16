//! The merge queue's wiring (the owner's testing principles 2 and 3, decision 32.53; chapter
//! 28.17): a change cannot land on main without every test it reaches having run, once, on a
//! push to its pull request or in GitHub's merge queue, and a push to main skips only what the queue
//! passed on the identical tree. The workflows are read as YAML (yaml-rust2) and main's ruleset as
//! JSON (`serde_json`), so a check sees the structure GitHub sees, not a line that looks right:
//!
//! - `the_ruleset_requires_checks_that_always_report`: `.github/rulesets/protect-main.json`
//!   requires a merge queue that merges with a merge commit (so the sign-off check skips its
//!   commits once they are on main, `dco.yml`) and exactly the checks of [`REQUIRED`]; each is the
//!   name of a job of a workflow that runs on every pull request and in the merge queue, with no
//!   path filter, and each kernel one is its workflow's last job: it runs whatever happened
//!   (`always()`) and needs every other job.
//! - `each_event_runs_the_jobs_the_last_job_expects`: every job of `kernel.yml` but `changes` and
//!   `done` runs only when `changes` found kernel files and the queue had not passed the tree, and
//!   on the events its condition admits; for a pull request, the merge queue and any other event
//!   (a push to main the queue did not test, a release), that set of jobs is exactly the list
//!   `done` holds the run to, so a job the event must run cannot be skipped unseen.
//! - `the_queue_and_the_pull_request_cover_every_leg`: the goldens' matrix (`test`) and the
//!   pull request's Linux job (`linux`) together cover every native runner in release and debug,
//!   once each; the suites run optimised with overflow checks and debug assertions on, on Linux
//!   x86-64 for a pull request and on every other native runner in the queue, with one plain debug
//!   run on Linux x86-64.
//! - `each_change_is_measured_against_its_base_and_marked_once`: `changes` measures a pull request
//!   against its merge commit's first parent, the queue against the queue's base and a push against
//!   the commit main was at; only the merge queue's last job marks a tree as tested, under the name
//!   the push looks for; `kernel-quality.yml` marks a tree only when mutation and coverage both ran
//!   on a pull request or in the queue, under the name it looks for.
//!
//! Each check is also shown to fail on a seeded violation.

// Tests may use native arithmetic.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "indices; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;
use yaml_rust2::{Yaml, YamlLoader};

const WORKFLOWS: &str = ".github/workflows";
const RULESET: &str = ".github/rulesets/protect-main.json";

/// The checks main's ruleset requires: each workflow's last job, by its name.
const REQUIRED: [(&str, &str, &str); 3] = [
    ("kernel.yml", "done", "Kernel checks pass"),
    ("kernel-quality.yml", "done", "Kernel quality checks pass"),
    ("pieces.yml", "checked-out", "Piece is checked out"),
];

/// The events the last job of `kernel.yml` tells apart, as its `case` names them.
const EVENTS: [&str; 3] = ["pull_request", "merge_group", "*"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .to_path_buf()
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel))
        .unwrap()
        .replace("\r\n", "\n")
}

fn parse(text: &str) -> Yaml {
    let mut docs = YamlLoader::load_from_str(text).unwrap();
    assert_eq!(docs.len(), 1, "one document");
    docs.remove(0)
}

fn keys(y: &Yaml) -> Vec<String> {
    y.as_hash()
        .map(|h| {
            h.keys()
                .filter_map(|k| k.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn strings(y: &Yaml) -> Vec<String> {
    y.as_vec()
        .map(|v| {
            v.iter()
                .filter_map(|s| s.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn text(y: &Yaml) -> String {
    y.as_str().unwrap_or_default().to_owned()
}

/// The workflows [`REQUIRED`] names, by file name, as text.
fn workflows() -> BTreeMap<String, String> {
    REQUIRED
        .iter()
        .map(|(file, _, _)| ((*file).to_owned(), read(&format!("{WORKFLOWS}/{file}"))))
        .collect()
}

/// The `run:` scripts of a job, joined.
fn runs(w: &Yaml, job: &str) -> String {
    w["jobs"][job]["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .filter_map(|s| s["run"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// What is wrong with the ruleset and the jobs it requires.
fn ruleset_problems(ruleset: &str, workflows: &BTreeMap<String, String>) -> Vec<String> {
    let mut found = Vec::new();
    let rules: Value = serde_json::from_str(ruleset).unwrap();
    let rule = |kind: &str| -> Value {
        rules["rules"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["type"] == kind)
            .cloned()
            .unwrap_or(Value::Null)
    };
    let queue = rule("merge_queue");
    if queue.is_null() {
        found.push("main's ruleset has no merge queue".to_owned());
    } else if queue["parameters"]["merge_method"] != "MERGE" {
        found.push(format!(
            "the merge queue merges by {}, not with a merge commit: it would rewrite a train's \
             signed commits",
            queue["parameters"]["merge_method"]
        ));
    }
    let required: BTreeSet<String> =
        rule("required_status_checks")["parameters"]["required_status_checks"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["context"].as_str().map(str::to_owned))
            .collect();
    let wanted: BTreeSet<String> = REQUIRED.iter().map(|(_, _, n)| (*n).to_owned()).collect();
    if required != wanted {
        found.push(format!(
            "main's ruleset requires {required:?}, not {wanted:?}"
        ));
    }
    for (file, job, name) in REQUIRED {
        let w = parse(&workflows[file]);
        let on = keys(&w["on"]);
        for event in ["pull_request", "merge_group"] {
            if !on.iter().any(|k| k == event) {
                found.push(format!(
                    "{file} does not run on {event}, so \"{name}\" never reports there"
                ));
            }
        }
        if !w["on"]["pull_request"]["paths"].is_badvalue() {
            found.push(format!(
                "{file} has a path filter, so \"{name}\" never reports on other pull requests"
            ));
        }
        let j = &w["jobs"][job];
        if text(&j["name"]) != name {
            found.push(format!("{file}'s job {job} is not named \"{name}\""));
        }
        if file.starts_with("kernel") {
            if text(&j["if"]) != "${{ always() }}" {
                found.push(format!("{file}'s {job} does not run whatever happened"));
            }
            let needs: BTreeSet<String> = strings(&j["needs"]).into_iter().collect();
            let others: BTreeSet<String> =
                keys(&w["jobs"]).into_iter().filter(|k| k != job).collect();
            if needs != others {
                found.push(format!(
                    "{file}'s {job} does not judge {:?}",
                    others.difference(&needs).collect::<Vec<_>>()
                ));
            }
        }
    }
    found
}

/// The terms of a job condition joined by `&&`, without the `${{ }}` around it.
fn terms(condition: &str) -> Vec<String> {
    let c = condition.trim();
    let c = c
        .strip_prefix("${{")
        .and_then(|c| c.strip_suffix("}}"))
        .unwrap_or(c);
    c.split("&&").map(|t| t.trim().to_owned()).collect()
}

/// The events of [`EVENTS`] a job condition admits, with `changes` having found kernel files and
/// the queue not having passed the tree; `Err` for a term this test cannot read, or a condition
/// that lacks either of those two terms.
fn admitted(condition: &str) -> Result<BTreeSet<&'static str>, String> {
    let mut events: BTreeSet<&str> = EVENTS.into_iter().collect();
    let (mut kernel, mut tested) = (false, false);
    for term in terms(condition) {
        if term == "needs.changes.outputs.kernel == 'true'" {
            kernel = true;
        } else if term == "needs.changes.outputs.tested != 'true'" {
            tested = true;
        } else if let Some(e) = term
            .strip_prefix("github.event_name != '")
            .and_then(|e| e.strip_suffix('\''))
        {
            events.retain(|x| *x != e);
        } else {
            return Err(format!("a term this test cannot read: {term}"));
        }
    }
    if !kernel {
        return Err("it runs on a change that touches no kernel file".to_owned());
    }
    if !tested {
        return Err("it runs again on a tree the merge queue passed".to_owned());
    }
    Ok(events)
}

/// The jobs `done` requires for each event, from its `case`.
fn expected(script: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for line in script.lines() {
        let line = line.trim();
        for event in EVENTS {
            if let Some(rest) = line.strip_prefix(&format!("{event}) must=\"")) {
                let list = rest.split('"').next().unwrap_or_default();
                out.insert(
                    event.to_owned(),
                    list.split_whitespace().map(str::to_owned).collect(),
                );
            }
        }
    }
    out
}

/// What is wrong with `kernel.yml`'s job conditions against its last job's lists.
fn event_problems(kernel: &str) -> Vec<String> {
    let mut found = Vec::new();
    let w = parse(kernel);
    let lists = expected(&runs(&w, "done"));
    if lists.len() != EVENTS.len() {
        found.push(format!("done lists the jobs of {:?} only", lists.keys()));
    }
    let mut running: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for job in keys(&w["jobs"]) {
        if job == "changes" || job == "done" {
            continue;
        }
        match admitted(&text(&w["jobs"][job.as_str()]["if"])) {
            Ok(events) => {
                for e in events {
                    running.entry(e).or_default().insert(job.clone());
                }
            }
            Err(e) => found.push(format!("{job}: {e}")),
        }
    }
    for event in EVENTS {
        let runs = running.get(event).cloned().unwrap_or_default();
        let listed = lists.get(event).cloned().unwrap_or_default();
        if runs != listed {
            found.push(format!(
                "on {event} the jobs {runs:?} run, but done requires {listed:?}"
            ));
        }
    }
    found
}

/// A matrix target's runner.
fn os(target: &Yaml) -> String {
    text(&target["os"])
}

/// What is wrong with which legs run where.
fn leg_problems(kernel: &str) -> Vec<String> {
    let mut found = Vec::new();
    let w = parse(kernel);
    let test = &w["jobs"]["test"]["strategy"]["matrix"];
    let runners: Vec<String> = test["target"].as_vec().unwrap().iter().map(os).collect();
    let modes = strings(&test["mode"]);
    let mut legs: BTreeMap<(String, String), usize> = BTreeMap::new();
    for r in &runners {
        for m in &modes {
            let excluded = test["exclude"]
                .as_vec()
                .into_iter()
                .flatten()
                .any(|x| text(&x["mode"]) == *m && os(&x["target"]) == *r);
            if !excluded {
                *legs.entry((r.clone(), m.clone())).or_default() += 1;
            }
        }
    }
    // The pull request's job holds Linux x86-64 in release.
    if text(&w["jobs"]["linux"]["runs-on"]) == "ubuntu-latest" {
        *legs
            .entry(("ubuntu-latest".to_owned(), "release".to_owned()))
            .or_default() += 1;
    }
    for r in &runners {
        for m in ["release", "debug"] {
            match legs.get(&(r.clone(), m.to_owned())) {
                Some(1) => {}
                n => found.push(format!(
                    "the goldens run {} times on {r} in {m}",
                    n.copied().unwrap_or(0)
                )),
            }
        }
    }
    // The suites: optimised with checks on, debug assertions included.
    let linux = &w["jobs"]["linux"];
    let checked = linux["steps"].as_vec().unwrap().iter().any(|s| {
        text(&s["run"]).starts_with("cargo nextest run $KERNEL_CRATES --release")
            && text(&s["env"]["RUSTFLAGS"]).contains("-C overflow-checks=on")
            && text(&s["env"]["CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS"]) == "true"
    });
    if !checked {
        found.push(
            "the pull request's tests are not optimised with overflow checks and debug \
             assertions on"
                .to_owned(),
        );
    }
    let suite = &w["jobs"]["suite"];
    if text(&suite["env"]["CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS"]) != "true" {
        found.push("the queue's optimised tests run without debug assertions".to_owned());
    }
    let matrix = &suite["strategy"]["matrix"];
    let release: BTreeSet<String> = matrix["target"].as_vec().unwrap().iter().map(os).collect();
    let wanted: BTreeSet<String> = runners
        .iter()
        .filter(|r| *r != "ubuntu-latest")
        .cloned()
        .collect();
    if release != wanted {
        found.push(format!(
            "the queue's optimised suite runs on {release:?}, not every other runner {wanted:?}"
        ));
    }
    let debug: Vec<(String, String)> = matrix["include"]
        .as_vec()
        .into_iter()
        .flatten()
        .map(|i| (text(&i["part"]["mode"]), os(&i["target"])))
        .collect();
    if debug.is_empty()
        || debug
            .iter()
            .any(|(m, r)| m != "debug" || r != "ubuntu-latest")
    {
        found.push(format!(
            "the one plain debug run is not on Linux x86-64 alone: {debug:?}"
        ));
    }
    found
}

/// What is wrong with how a change finds its base and how a tree is marked.
fn mark_problems(kernel: &str, quality: &str) -> Vec<String> {
    let mut found = Vec::new();
    let k = parse(kernel);
    let decide = runs(&k, "changes");
    let step = &k["jobs"]["changes"]["steps"][1];
    for (needle, why) in [
        (
            "base=$(git rev-parse HEAD^1)",
            "a pull request against its merge commit's parent",
        ),
        (
            "merge_group) base=\"$QUEUE_BASE\"",
            "the queue against its base",
        ),
        ("base=\"$BEFORE\"", "a push against the commit main was at"),
        ("name=kernel-tested-$tree", "the queue's mark by its tree"),
    ] {
        if !decide.contains(needle) {
            found.push(format!("changes does not measure {why}"));
        }
    }
    for (var, value) in [
        ("QUEUE_BASE", "${{ github.event.merge_group.base_sha }}"),
        ("BEFORE", "${{ github.event.before }}"),
    ] {
        if text(&step["env"][var]) != value {
            found.push(format!("changes reads {var} from somewhere else"));
        }
    }
    let marks = |w: &Yaml, prefix: &str| -> Vec<(String, String)> {
        w["jobs"]["done"]["steps"]
            .as_vec()
            .into_iter()
            .flatten()
            .filter(|s| text(&s["uses"]).starts_with("actions/upload-artifact@"))
            .map(|s| (text(&s["with"]["name"]), text(&s["if"])))
            .filter(|(n, _)| n.starts_with(prefix))
            .collect()
    };
    let kernel_marks = marks(&k, "kernel-tested-");
    if kernel_marks
        != [(
            "kernel-tested-${{ needs.changes.outputs.tree }}".to_owned(),
            "github.event_name == 'merge_group' && needs.changes.outputs.kernel == 'true'"
                .to_owned(),
        )]
    {
        found.push(format!(
            "only the merge queue may mark a tree as tested by kernel.yml: {kernel_marks:?}"
        ));
    }
    let q = parse(quality);
    if !runs(&q, "when").contains("name=quality-tested-$tree") {
        found.push("kernel-quality.yml does not look for its own mark".to_owned());
    }
    let quality_marks = marks(&q, "quality-tested-");
    let condition = "(github.event_name == 'pull_request' || github.event_name == 'merge_group') \
                     && needs.when.outputs.mutants == 'true' && needs.when.outputs.coverage == 'true'";
    if quality_marks
        != [(
            "quality-tested-${{ needs.when.outputs.tree }}".to_owned(),
            condition.to_owned(),
        )]
    {
        found.push(format!(
            "kernel-quality.yml marks a tree its diff gates did not both judge: {quality_marks:?}"
        ));
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_ruleset_requires_checks_that_always_report() {
    let ruleset = read(RULESET);
    let workflows = workflows();
    assert_eq!(ruleset_problems(&ruleset, &workflows), Vec::<String>::new());

    // Seeded violations: each must be reported.
    let squash = ruleset.replacen(
        "\"merge_method\": \"MERGE\"",
        "\"merge_method\": \"SQUASH\"",
        1,
    );
    assert_ne!(squash, ruleset);
    assert!(
        ruleset_problems(&squash, &workflows)
            .iter()
            .any(|p| p.contains("rewrite a train's signed commits"))
    );
    let unrequired = ruleset.replacen("{ \"context\": \"Kernel quality checks pass\" }, ", "", 1);
    assert_ne!(unrequired, ruleset);
    assert!(
        ruleset_problems(&unrequired, &workflows)
            .iter()
            .any(|p| p.contains("main's ruleset requires"))
    );
    let seeded = |file: &str, from: &str, to: &str| {
        let mut w = workflows.clone();
        let t = w.get_mut(file).unwrap();
        assert!(t.contains(from), "{file} lacks {from:?}");
        *t = t.replacen(from, to, 1);
        ruleset_problems(&ruleset, &w)
    };
    assert!(
        seeded("kernel.yml", "  merge_group:\n", "")
            .iter()
            .any(|p| p.contains("kernel.yml does not run on merge_group"))
    );
    assert!(
        seeded("pieces.yml", "  merge_group:\n", "")
            .iter()
            .any(|p| p.contains("pieces.yml does not run on merge_group"))
    );
    assert!(
        seeded(
            "kernel-quality.yml",
            "  pull_request:\n",
            "  pull_request:\n    paths: [\"projects/**\"]\n"
        )
        .iter()
        .any(|p| p.contains("has a path filter"))
    );
    assert!(
        seeded(
            "kernel.yml",
            " wasm, wasm-suite, big-endian, large-models]",
            " wasm, wasm-suite, large-models]"
        )
        .iter()
        .any(|p| p.contains("does not judge [\"big-endian\"]"))
    );
    assert!(
        seeded(
            "kernel-quality.yml",
            "    name: Kernel quality checks pass\n",
            "    name: Quality\n"
        )
        .iter()
        .any(|p| p.contains("is not named"))
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn each_event_runs_the_jobs_the_last_job_expects() {
    let kernel = read(&format!("{WORKFLOWS}/kernel.yml"));
    assert_eq!(event_problems(&kernel), Vec::<String>::new());

    // Seeded violations: each must be reported.
    let queue_job = "    # The merge queue's: the tests the change can reach on the other platforms, optimised with\n    # overflow checks and debug assertions on, and one plain debug run on Linux x86-64.\n    if: github.event_name != 'pull_request'";
    for (from, to, said) in [
        // The queue's suite moved to the pull request: the queue no longer runs it.
        (
            queue_job,
            "    # The merge queue's: the tests the change can reach on the other platforms, optimised with\n    # overflow checks and debug assertions on, and one plain debug run on Linux x86-64.\n    if: github.event_name != 'merge_group'",
            "on merge_group the jobs",
        ),
        // A job dropped from what the queue must run.
        (
            "merge_group) must=\"lint scope test suite wasm wasm-suite big-endian large-models\"",
            "merge_group) must=\"lint scope test suite wasm-suite big-endian large-models\"",
            "on merge_group the jobs",
        ),
        // A job that would run again on a tree the queue passed.
        (
            "    if: github.event_name != 'merge_group' && needs.changes.outputs.kernel == 'true' && needs.changes.outputs.tested != 'true'\n    runs-on: ubuntu-latest\n    timeout-minutes: 60",
            "    if: github.event_name != 'merge_group' && needs.changes.outputs.kernel == 'true'\n    runs-on: ubuntu-latest\n    timeout-minutes: 60",
            "guard: it runs again on a tree the merge queue passed",
        ),
        // A condition the model cannot read is refused, not guessed.
        (
            queue_job,
            "    # The merge queue's: the tests the change can reach on the other platforms, optimised with\n    # overflow checks and debug assertions on, and one plain debug run on Linux x86-64.\n    if: github.actor != 'someone' && github.event_name != 'pull_request'",
            "a term this test cannot read",
        ),
    ] {
        let k = kernel.replacen(from, to, 1);
        assert_ne!(k, kernel, "the planted edit {from:?} applies");
        let found = event_problems(&k);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_queue_and_the_pull_request_cover_every_leg() {
    let kernel = read(&format!("{WORKFLOWS}/kernel.yml"));
    assert_eq!(leg_problems(&kernel), Vec::<String>::new());

    // Seeded violations: each must be reported.
    for (from, to, said) in [
        // A second leg excluded from the goldens.
        (
            "        exclude:\n",
            "        exclude:\n          - mode: debug\n            target:\n              name: Windows x86-64\n              os: windows-latest\n",
            "the goldens run 0 times on windows-latest in debug",
        ),
        // The excluded leg's job moved off Linux x86-64.
        (
            "    runs-on: ubuntu-latest\n    timeout-minutes: 120\n    strategy:\n      fail-fast: false\n      matrix:\n        k: [1, 2]",
            "    runs-on: ubuntu-24.04-arm\n    timeout-minutes: 120\n    strategy:\n      fail-fast: false\n      matrix:\n        k: [1, 2]",
            "the goldens run 0 times on ubuntu-latest in release",
        ),
        // The pull request's tests without debug assertions.
        (
            "          CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS: \"true\"\n        run: cargo nextest run $KERNEL_CRATES --release --locked --no-fail-fast --partition count:${{ matrix.k }}/2",
            "        run: cargo nextest run $KERNEL_CRATES --release --locked --no-fail-fast --partition count:${{ matrix.k }}/2",
            "the pull request's tests are not optimised",
        ),
        // The queue's suite without debug assertions.
        (
            "      # Debug assertions on in the release legs too (the release steps add overflow checks).\n      CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS: \"true\"\n",
            "",
            "without debug assertions",
        ),
        // A platform left out of the queue.
        (
            "          - name: Windows arm64\n            os: windows-11-arm\n          - name: macOS arm64\n            os: macos-latest\n          - name: macOS x86-64\n            os: macos-15-intel\n        # The one plain debug run",
            "          - name: macOS arm64\n            os: macos-latest\n          - name: macOS x86-64\n            os: macos-15-intel\n        # The one plain debug run",
            "not every other runner",
        ),
    ] {
        let k = kernel.replacen(from, to, 1);
        assert_ne!(k, kernel, "the planted edit {from:?} applies");
        let found = leg_problems(&k);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn each_change_is_measured_against_its_base_and_marked_once() {
    let kernel = read(&format!("{WORKFLOWS}/kernel.yml"));
    let quality = read(&format!("{WORKFLOWS}/kernel-quality.yml"));
    assert_eq!(mark_problems(&kernel, &quality), Vec::<String>::new());

    // Seeded violations: each must be reported.
    let mark = "        if: github.event_name == 'merge_group' && needs.changes.outputs.kernel == 'true'\n        with:";
    for (k, q, said) in [
        (
            kernel.replacen(
                "QUEUE_BASE: ${{ github.event.merge_group.base_sha }}",
                "QUEUE_BASE: ${{ github.event.merge_group.head_sha }}",
                1,
            ),
            quality.clone(),
            "changes reads QUEUE_BASE",
        ),
        (
            kernel.replacen(
                mark,
                "        if: needs.changes.outputs.kernel == 'true'\n        with:",
                1,
            ),
            quality.clone(),
            "only the merge queue may mark",
        ),
        (
            kernel.clone(),
            quality.replacen(
                " && needs.when.outputs.coverage == 'true'\n        with:",
                "\n        with:",
                1,
            ),
            "kernel-quality.yml marks a tree",
        ),
        (
            kernel.clone(),
            quality.replacen("name=quality-tested-$tree", "name=quality-$tree", 1),
            "does not look for its own mark",
        ),
    ] {
        assert!(
            k != kernel || q != quality,
            "{said}: the planted edit applies"
        );
        let found = mark_problems(&k, &q);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}
