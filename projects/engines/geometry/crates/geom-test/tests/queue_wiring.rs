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
//!   on the events its condition admits; for a pull request, the merge queue, a push (to main, or
//!   a release tag's) and a run by hand (`workflow_dispatch`), that set of jobs is exactly the list
//!   `done` holds the run to, by the first arm of its `case` the event matches, so a job the event
//!   must run cannot be skipped unseen. A run by hand runs exactly what a push runs, and it takes
//!   no inputs, so nothing can leave a job out of it.
//! - `a_run_by_hand_writes_nothing_later_runs_read`: in `kernel.yml` a step that leaves something
//!   for later runs (a build cache saved, an artifact uploaded, a release, a cache entry or a
//!   repository written through `gh` or `git push`) does so only on a push to main or in the merge
//!   queue, by `&&` terms of its condition with no `||` beside them, so a run by hand, on main or
//!   on any branch, writes nothing another run trusts.
//! - `the_queue_and_the_pull_request_cover_every_leg`: the goldens' matrix (`test`) and the
//!   pull request's Linux job (`linux`) together cover every native runner in release and debug,
//!   once each; the suites run optimised with overflow checks and debug assertions on, on Linux
//!   x86-64 for a pull request and on every other native runner in the queue, with one plain debug
//!   run on Linux x86-64; and each release leg of `test` shares its work among its parts with every
//!   golden and test step on exactly one part, each golden build's `--part j/m` shares whole, and
//!   geom-num's library, binaries, doctests and every test target once in each build.
//! - `the_goldens_run_only_the_runner_their_run_built_and_checked`: `golden-runner` builds the
//!   goldens' runner once for each of `test`'s targets, with no part's build flags, gives its
//!   sha256 as that target's output and uploads it, failing without the file; every part of
//!   `test` downloads this run's own runner, checks a full sha256 was given, the file is there and
//!   its digest is that one, before any golden step, and every golden step runs that checked file;
//!   no part builds the runner itself.
//! - `the_wasm32_legs_run_every_golden_and_test_once_in_parts`: each runtime's goldens on wasm32
//!   (`wasm`) are parts 1 to n, every golden and test step and the guard's proof on exactly one,
//!   each build's `--part j/m` shares whole; the suite on wasm32 is built once per build
//!   (`wasm-build`, its nextest archive uploaded and its doctests once under each runtime) and run
//!   from that archive in parts (`wasm-suite`, `--partition count:k/n` for k = 1 to n once under
//!   each runtime), the parts waiting for the build.
//! - `each_change_is_measured_against_its_base_and_marked_once`: `changes` measures a pull request
//!   against its merge commit's first parent, the queue against the queue's base and a push against
//!   the commit main was at; only the merge queue's last job marks a tree as tested, under the name
//!   the push looks for, and only a push to main looks for it; `kernel-quality.yml` marks a tree only when mutation and coverage both ran
//!   on a pull request or in the queue, under the name it looks for.
//! - `the_count_gates_measure_one_kind_a_push_and_the_base_once`: `kernel-bench.yml` runs in the
//!   merge queue too; its instruction legs are the job `scope`'s list, x86-64 on a pull request,
//!   aarch64 in the queue (which marks the tree it passed) and on a push to main unless that mark
//!   is there; the merge base's counts are restored only from a run of main or of the same pull
//!   request with the same workflow file, after their manifest is checked, and a missing count is
//!   measured (`kept` is false unless the restore succeeded, and the measuring step runs then);
//!   the merge base is measured and kept in parts that never run the change's own code; and the
//!   parts are geom-bench/parts.tsv's, the change's wait for the merge base's, and each leg's judge
//!   waits for both, judges when a part failed and checks every part of both sides.
//! - `the_ember_part_fetches_its_pair_alone_and_the_judge_counts_it`: the instruction gate's last
//!   part measures geom-bench's benchmark of EMBER's pair 827 (`mesh/ember`, Gate 4, G156); in
//!   both part jobs only that part fetches anything, exactly the pair's two models
//!   (`geom_test::ember::MODELS`), each held to the SHA-256 the corpus register holds for it before
//!   any benchmark runs; every part lists the ember benchmark with the catalogue's, so the judge,
//!   which reads both bench targets' summaries, refuses a run in which no part measured it.
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
use std::fmt::Write as _;
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

/// The events `kernel.yml` runs on, by the name its conditions read (`github.event_name`): a pull
/// request, the merge queue, a push (to main, or a release tag's, which kernel-release.yml calls
/// it for) and a run by hand.
const EVENTS: [&str; 4] = ["pull_request", "merge_group", "push", "workflow_dispatch"];

/// A run by hand, which must run what [`PUSH`] runs.
const BY_HAND: &str = "workflow_dispatch";

/// A push.
const PUSH: &str = "push";

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

/// The jobs `done` requires for each event of [`EVENTS`], from its `case`: the list of the first
/// arm whose patterns (`a|b`, or `*`) the event matches, as the shell picks it.
fn expected(script: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for line in script.lines() {
        let Some((patterns, rest)) = line.trim().split_once(") must=\"") else {
            continue;
        };
        let list: BTreeSet<String> = rest
            .split('"')
            .next()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        for event in EVENTS {
            if patterns.split('|').any(|p| p == event || p == "*") {
                out.entry(event.to_owned()).or_insert_with(|| list.clone());
            }
        }
    }
    out
}

/// What is wrong with `kernel.yml`'s job conditions against its last job's lists.
fn event_problems(kernel: &str) -> Vec<String> {
    let mut found = Vec::new();
    let w = parse(kernel);
    let by_hand = &w["on"][BY_HAND];
    if by_hand.is_badvalue() {
        found.push(format!(
            "kernel.yml has no {BY_HAND}, so a branch cannot be given a push's run"
        ));
    } else if !by_hand["inputs"].is_badvalue() {
        found.push(format!(
            "kernel.yml's {BY_HAND} takes inputs, which could leave a job out of the run"
        ));
    }
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
    let push = running.get(PUSH).cloned().unwrap_or_default();
    let by_hand = running.get(BY_HAND).cloned().unwrap_or_default();
    if by_hand != push {
        found.push(format!(
            "a run by hand runs {by_hand:?}, but a push runs {push:?}"
        ));
    }
    if lists.get(BY_HAND) != lists.get(PUSH) {
        found.push(format!(
            "done requires {:?} of a run by hand, but {:?} of a push",
            lists.get(BY_HAND),
            lists.get(PUSH)
        ));
    }
    found
}

/// Whether a condition, without its `${{ }}`, has `||` outside every bracket, so its `&&` terms
/// do not each hold.
fn or_outside_brackets(condition: &str) -> bool {
    let mut depth = 0_i32;
    let mut last = ' ';
    for c in condition.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            '|' if last == '|' && depth == 0 => return true,
            _ => {}
        }
        last = c;
    }
    false
}

/// The commands of a `run:` script that write something a later run can read.
const WRITES: [&str; 5] = [
    "git push",
    "gh release",
    "gh cache",
    "gh api -X",
    "gh api --method",
];

/// What is wrong with the steps of `kernel.yml` that leave something for later runs: each must
/// write only on a push to main (its condition's terms `github.event_name == 'push'` and
/// `github.ref == 'refs/heads/main'`) or in the merge queue (`github.event_name ==
/// 'merge_group'`), never on a run by hand. The condition is a build cache's `save-if` (the cache
/// saves when it has none) or the step's `if`.
fn writer_problems(kernel: &str) -> Vec<String> {
    let mut found = Vec::new();
    let w = parse(kernel);
    for job in keys(&w["jobs"]) {
        for (i, step) in w["jobs"][job.as_str()]["steps"]
            .as_vec()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let uses = text(&step["uses"]);
            let run = text(&step["run"]);
            // A hand-over inside one run (`wasm-build`'s archive for `wasm-suite`'s parts, and
            // `golden-runner`'s runner for `test`'s parts) is not for later runs: it is named by
            // its build or system, kept one day, and downloaded under the same name by a job of
            // the same run. Anything else that uploads is held to the rule.
            let handed = |maker: &str, name: &str, taker: &str, taken: &str| {
                uses.starts_with("actions/upload-artifact@")
                    && job == maker
                    && text(&step["with"]["name"]) == name
                    && step["with"]["retention-days"].as_i64() == Some(1)
                    && w["jobs"][taker]["steps"]
                        .as_vec()
                        .into_iter()
                        .flatten()
                        .any(|d| {
                            text(&d["uses"]).starts_with("actions/download-artifact@")
                                && text(&d["with"]["name"]) == taken
                        })
            };
            if handed(
                "wasm-build",
                "wasm-${{ matrix.build }}",
                "wasm-suite",
                "wasm-${{ matrix.part.build }}",
            ) || handed("golden-runner", RUNNER_ARTIFACT, "test", RUNNER_ARTIFACT)
            {
                continue;
            }
            let condition = if uses.starts_with("Swatinem/rust-cache@") {
                text(&step["with"]["save-if"])
            } else if uses.starts_with("actions/upload-artifact@")
                || uses.starts_with("actions/cache@")
                || uses.starts_with("actions/cache/save@")
                || WRITES.iter().any(|c| run.contains(c))
            {
                text(&step["if"])
            } else {
                continue;
            };
            let said = format!("{job}'s step {} ({})", i + 1, text(&step["name"]));
            let t = terms(&condition);
            let has = |term: &str| t.iter().any(|x| x == term);
            let push = has("github.event_name == 'push'") && has("github.ref == 'refs/heads/main'");
            let queue = has("github.event_name == 'merge_group'");
            let bare = condition
                .trim()
                .strip_prefix("${{")
                .and_then(|c| c.strip_suffix("}}"))
                .unwrap_or(&condition);
            if !(push || queue) || or_outside_brackets(bare) {
                found.push(format!(
                    "{said} writes for later runs on a run by hand, not on a push to main or in \
                     the queue alone: {condition:?}"
                ));
            }
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
    found.extend(linux_part_problems(&w));
    found.extend(suite_part_problems(&w));
    found.extend(test_part_problems(&w, &geom_num_tests()));
    found.extend(wasm_part_problems(&w));
    found.extend(wasm_suite_part_problems(&w));
    found
}

/// The parts of `linux` a step's condition runs it on, of the matrix's `parts`: every part when it
/// names no part, else those `matrix.k == n` or `matrix.k <= n` admits. `None` for a condition on
/// the part this reading does not know, so a new form of condition is looked at, not passed.
fn parts_of(condition: &str, parts: &[i64]) -> Option<Vec<i64>> {
    let Some(at) = condition.find("matrix.k ") else {
        return Some(parts.to_vec());
    };
    let rest = &condition[at + "matrix.k ".len()..];
    let (op, rest) = rest.split_once(' ')?;
    let n: i64 = rest
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    if condition.matches("matrix.k").count() != 1 {
        return None;
    }
    match op {
        "==" => Some(parts.iter().copied().filter(|&k| k == n).collect()),
        "<=" => Some(parts.iter().copied().filter(|&k| k <= n).collect()),
        _ => None,
    }
}

/// The release suite's shares the job `scope` writes: the `--parts n` of its `test-scope` step and
/// the crates `--apart` leaves out of them, or `None` when the step gives no `--parts`.
fn scope_shares(w: &Yaml) -> Option<(i64, Vec<String>)> {
    let run = w["jobs"]["scope"]["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .map(|s| text(&s["run"]).replace('\n', " "))
        .find(|r| r.contains("kernel-quality -- test-scope"))?;
    let words: Vec<&str> = run.split_whitespace().collect();
    let n = words
        .iter()
        .position(|w| *w == "--parts")
        .and_then(|at| words.get(at + 1))
        .and_then(|n| n.parse().ok())?;
    let apart = words
        .windows(2)
        .filter(|p| p[0] == "--apart")
        .map(|p| p[1].to_owned())
        .collect();
    Some((n, apart))
}

/// The expression with which a part of `job` takes its share of the crates: the crate its matrix
/// entry or number names, else share k of the job `scope`'s `parts`.
fn share_expression(job: &str, apart: &str, own_part: i64) -> String {
    let parts = "fromJSON(needs.scope.outputs.parts)";
    if job == "suite" {
        format!(
            "${{{{ matrix.part.mode == 'release' && (matrix.part.crate || {parts}[format('{{0}}', \
             matrix.part.k)]) || '' }}}}"
        )
    } else {
        format!(
            "${{{{ matrix.k == {own_part} && '{apart}' || {parts}[format('{{0}}', matrix.k)] }}}}"
        )
    }
}

/// What is wrong with how a job's listing step narrows the crates to its part's share: the share
/// not taken by `expected`, or the script not narrowing by it.
fn share_step_problems(job: &str, steps: &Yaml, expected: &str) -> Vec<String> {
    let listing = steps
        .as_vec()
        .into_iter()
        .flatten()
        .find(|s| text(&s["run"]).contains("KERNEL_CRATES="));
    let Some(listing) = listing else {
        return vec![format!("{job} does not list its crates")];
    };
    let mut found = Vec::new();
    if text(&listing["env"]["SHARE"]) != expected {
        found.push(format!(
            "{job}'s parts do not take their own share: SHARE is {:?}",
            text(&listing["env"]["SHARE"])
        ));
    }
    if !text(&listing["run"]).contains("case \" $SHARE \" in *\" $c \"*) ;; *) continue ;; esac") {
        found.push(format!(
            "{job}'s listing does not narrow the crates to the share"
        ));
    }
    found
}

/// What is wrong with how `linux` shares its work among its parts: parts not 1 to n or a name
/// that does not count them; the tests and doctests not on exactly the parts of the job `scope`'s
/// shares and of the crates it sets apart (one part each), or run on a partition or a selection of
/// their share; a part's share not its own; a golden step on no part or on more than one (run
/// twice), or a golden build's `--part j/m` shares not exactly 1 to m; another step on no part or
/// more than one.
fn linux_part_problems(w: &Yaml) -> Vec<String> {
    let mut found = Vec::new();
    let linux = &w["jobs"]["linux"];
    let parts: Vec<i64> = linux["strategy"]["matrix"]["k"]
        .as_vec()
        .into_iter()
        .flatten()
        .filter_map(Yaml::as_i64)
        .collect();
    let n = i64::try_from(parts.len()).unwrap();
    if parts != (1..=n).collect::<Vec<_>>() {
        found.push(format!("linux's parts are {parts:?}, not 1 to {n}"));
    }
    if !text(&linux["name"]).ends_with(&format!("part ${{{{ matrix.k }}}} of {n})")) {
        found.push(format!(
            "linux's name does not count its {n} parts: {}",
            text(&linux["name"])
        ));
    }
    let Some((shares, apart)) = scope_shares(w) else {
        found.push("the job scope gives the suites no shares (--parts)".to_owned());
        return found;
    };
    if apart.is_empty() {
        found.push("the scope sets no crate apart for linux's part of its own".to_owned());
    }
    // Shares 1 to n, then one part for the crates set apart.
    let tested: Vec<i64> = (1..=shares + 1).collect();
    found.extend(share_step_problems(
        "linux",
        &linux["steps"],
        &share_expression("linux", &apart.join(" "), shares + 1),
    ));
    let mut goldens: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for step in linux["steps"].as_vec().into_iter().flatten() {
        let run = text(&step["run"]);
        let name = text(&step["name"]);
        let golden = run.starts_with("cargo run --locked -p geom-test --bin golden");
        let heavy = golden || run.starts_with("cargo test") || run.starts_with("cargo nextest");
        if !heavy {
            continue;
        }
        let Some(on) = parts_of(&text(&step["if"]), &parts) else {
            found.push(format!(
                "{name}: its condition on the part is not read: {}",
                text(&step["if"])
            ));
            continue;
        };
        if run.contains("$KERNEL_CRATES") {
            let whole = [
                "cargo nextest run $KERNEL_CRATES --release --locked --no-fail-fast",
                "cargo test $KERNEL_CRATES --doc --release --locked --no-fail-fast",
            ];
            if !whole.contains(&run.as_str()) {
                found.push(format!(
                    "{name}: runs a partition or a selection of its share: {run}"
                ));
            }
            if on != tested {
                found.push(format!(
                    "{name}: runs on parts {on:?}, not the shares' and the crate apart's {tested:?}"
                ));
            }
            continue;
        }
        if on.len() != 1 {
            found.push(format!("{name}: runs on parts {on:?}, not exactly one"));
        }
        if golden {
            let words: Vec<&str> = run.split_whitespace().collect();
            let share = words
                .iter()
                .position(|w| *w == "--part")
                .and_then(|at| words.get(at + 1))
                .map_or_else(|| "1/1".to_owned(), |s| (*s).to_owned());
            let build = format!(
                "{} {} {} {}",
                text(&step["env"]["RUSTFLAGS"]),
                text(&step["env"]["CARGO_TARGET_DIR"]),
                text(&step["env"]["CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS"]),
                run.replacen(&format!("--part {share} "), "", 1)
            );
            goldens.entry(build).or_default().push(share);
        }
    }
    for (build, got) in &goldens {
        let m = got.len();
        let wanted: BTreeSet<String> = (1..=m).map(|j| format!("{j}/{m}")).collect();
        if got.iter().cloned().collect::<BTreeSet<_>>() != wanted {
            found.push(format!(
                "linux's goldens of `{build}` run shares {got:?}, not each of {wanted:?} once"
            ));
        }
    }
    found
}

/// What is wrong with the suite's release parts of a crate set apart (`entries` whose `crate` is
/// `krate`): its library, binaries and doctests not once, or a file of its `tests` folder in no
/// share `[x-y]*` or in two.
fn own_part_problems(entries: &[&Yaml], krate: &str) -> Vec<String> {
    let mut selected: Vec<String> = Vec::new();
    for p in entries.iter().filter(|p| text(&p["crate"]) == krate) {
        if p["doc"].as_bool() == Some(true) {
            selected.push("doc".to_owned());
        }
        selected.extend(
            text(&p["targets"])
                .split(" --")
                .map(|s| s.trim().trim_start_matches("--").to_owned())
                .filter(|s| !s.is_empty()),
        );
    }
    vector_problems(
        &format!("suite's {krate} parts"),
        &selected,
        &crate_tests(krate),
    )
    .into_iter()
    .map(|p| p.replace("geom-num's", &format!("{krate}'s")))
    .collect()
}

/// What is wrong with how `suite`'s release parts share the tests: parts not 1 to n of one n; the
/// parts that take a share of the job `scope` not exactly 1 to its `--parts`, each with its
/// doctests; a crate set apart without parts of its own, or one with parts the scope does not
/// set apart; a crate's own parts not running its library, binaries and doctests once and every
/// file of its `tests` folder in exactly one share `[x-y]*`; the run or the doctests not those of
/// the part's own crates and targets; `done` not checking the same shares.
fn suite_part_problems(w: &Yaml) -> Vec<String> {
    let mut found = Vec::new();
    let suite = &w["jobs"]["suite"];
    let entries: Vec<&Yaml> = suite["strategy"]["matrix"]["part"]
        .as_vec()
        .into_iter()
        .flatten()
        .filter(|p| text(&p["mode"]) == "release")
        .collect();
    let ks: Vec<i64> = entries.iter().filter_map(|p| p["k"].as_i64()).collect();
    let n = i64::try_from(entries.len()).unwrap();
    if ks != (1..=n).collect::<Vec<_>>() || entries.iter().any(|p| p["n"].as_i64() != Some(n)) {
        found.push(format!(
            "suite's release parts are {ks:?}, not 1 to {n} of {n}"
        ));
    }
    let Some((shares, apart)) = scope_shares(w) else {
        found.push("the job scope gives the suites no shares (--parts)".to_owned());
        return found;
    };
    let taking: Vec<i64> = entries
        .iter()
        .filter(|p| p["crate"].is_badvalue())
        .filter_map(|p| p["k"].as_i64())
        .collect();
    if taking != (1..=shares).collect::<Vec<_>>() {
        found.push(format!(
            "suite's parts {taking:?} take a share, not the scope's shares 1 to {shares}"
        ));
    }
    for p in entries.iter().filter(|p| p["crate"].is_badvalue()) {
        if p["doc"].as_bool() != Some(true) || !p["targets"].is_badvalue() {
            found.push(format!(
                "suite's share part {} does not run its crates' every target and doctest",
                p["k"].as_i64().unwrap_or(0)
            ));
        }
    }
    let own: BTreeSet<String> = entries
        .iter()
        .filter(|p| !p["crate"].is_badvalue())
        .map(|p| text(&p["crate"]))
        .collect();
    if own != apart.iter().cloned().collect::<BTreeSet<_>>() {
        found.push(format!(
            "suite runs {own:?} in parts of their own, the scope sets apart {apart:?}"
        ));
    }
    for krate in &own {
        found.extend(own_part_problems(&entries, krate));
    }
    found.extend(share_step_problems(
        "suite",
        &suite["steps"],
        &share_expression("suite", "", 0),
    ));
    let steps: Vec<&Yaml> = suite["steps"].as_vec().into_iter().flatten().collect();
    let release = |s: &&Yaml| text(&s["if"]).contains("matrix.part.mode == 'release'");
    let commands: Vec<String> = steps
        .iter()
        .filter(|s| release(s))
        .filter(|s| text(&s["run"]).starts_with("cargo"))
        .map(|s| text(&s["run"]))
        .collect();
    let wanted = [
        "cargo nextest run $KERNEL_CRATES --release --locked --no-fail-fast ${{ matrix.part.targets }}",
        "cargo test $KERNEL_CRATES --doc --release --locked --no-fail-fast",
    ];
    if commands != wanted {
        found.push(format!(
            "suite's release parts do not run their own crates and targets: {commands:#?}"
        ));
    }
    if !steps.iter().filter(|s| release(s)).any(|s| {
        text(&s["run"]).contains("--doc") && text(&s["if"]).contains("&& matrix.part.doc &&")
    }) {
        found.push("suite's release doctests do not run on the parts that say `doc`".to_owned());
    }
    let done = runs(w, "done");
    let check = format!(
        "jq -e --argjson n {shares} --arg apart \"{}\"",
        apart.join(" ")
    );
    if !done.contains(&check) {
        found.push(format!(
            "done does not check the scope's {shares} shares: {check}"
        ));
    }
    found
}

/// A test-target share as cargo's `--test` glob takes it, `'<prefix>[x-y]*'` (the targets whose
/// name starts with the prefix, then a letter from x to y; the prefix may be empty) or
/// `'<prefix>*'` (a prefix of at least one character), read as the prefix and the letters; `None`
/// for any other form, so a new form is looked at, not passed.
fn letters(share: &str) -> Option<(String, Option<core::ops::RangeInclusive<char>>)> {
    let inner = share.strip_prefix('\'')?.strip_suffix("*'")?;
    let plain = |p: &str| {
        p.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    let Some((prefix, range)) = inner.strip_suffix(']').and_then(|r| r.split_once('[')) else {
        return (!inner.is_empty() && plain(inner)).then(|| (inner.to_owned(), None));
    };
    let mut chars = range.chars();
    let (from, dash, to) = (chars.next()?, chars.next()?, chars.next()?);
    (plain(prefix)
        && chars.next().is_none()
        && dash == '-'
        && from.is_ascii_lowercase()
        && to.is_ascii_lowercase())
    .then(|| (prefix.to_owned(), Some(from..=to)))
}

/// Whether the share [`letters`] read takes the test target `name`.
fn takes(share: &(String, Option<core::ops::RangeInclusive<char>>), name: &str) -> bool {
    let (prefix, range) = share;
    name.strip_prefix(prefix.as_str()).is_some_and(|rest| {
        range
            .as_ref()
            .is_none_or(|r| rest.chars().next().is_some_and(|c| r.contains(&c)))
    })
}

/// What is wrong with how `test` shares a release leg among its parts: parts not 1 to n or a name
/// that does not count them, a debug leg on more than one part, a heavy step on no part or on more
/// than one, a golden step's `--part j/m` shares not exactly 1 to m in each build (one dropped from
/// every part), and geom-num's every test vector, in each build, not its library, binaries,
/// doctests and every test target of `crates/geom-num/tests` exactly once (`tests` names them).
fn test_part_problems(w: &Yaml, tests: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    let test = &w["jobs"]["test"];
    let parts: Vec<i64> = test["strategy"]["matrix"]["k"]
        .as_vec()
        .into_iter()
        .flatten()
        .filter_map(Yaml::as_i64)
        .collect();
    let n = i64::try_from(parts.len()).unwrap();
    if parts != (1..=n).collect::<Vec<_>>() {
        found.push(format!("test's parts are {parts:?}, not 1 to {n}"));
    }
    let excluded: BTreeSet<i64> = test["strategy"]["matrix"]["exclude"]
        .as_vec()
        .into_iter()
        .flatten()
        .filter(|x| text(&x["mode"]) == "debug" && x["target"].is_badvalue())
        .filter_map(|x| x["k"].as_i64())
        .collect();
    // A debug leg is parts 1 to d, the rest excluded.
    let d = n - i64::try_from(excluded.len()).unwrap();
    if d < 1 || excluded != (d + 1..=n).collect::<BTreeSet<_>>() {
        found.push(format!(
            "the debug legs' parts are not 1 to {d}: parts {excluded:?} excluded"
        ));
    }
    let counted =
        format!("format('part {{0}} of {{1}}', matrix.k, matrix.mode == 'release' && {n} || {d})");
    if !text(&test["name"]).contains(&counted) {
        found.push(format!(
            "test's name does not count its {n} release and {d} debug parts"
        ));
    }
    // A golden step's build (its flags and folder) and command without `--part`, to its shares.
    let mut goldens: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // geom-num's builds, to what each step of it runs.
    let mut vectors: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for step in test["steps"].as_vec().into_iter().flatten() {
        let run = text(&step["run"]);
        let name = text(&step["name"]);
        // The runner `golden-runner` built for this system, checked by its digest.
        let golden = run.starts_with("cargo run --locked -p geom-test --bin golden")
            || run.starts_with(RUNS_GOLDEN);
        if !(golden || run.starts_with("cargo test") || run.starts_with("cargo nextest")) {
            continue;
        }
        match parts_of(&text(&step["if"]), &parts) {
            Some(on) if on.len() == 1 => {}
            Some(on) => found.push(format!("{name}: runs on parts {on:?}, not exactly one")),
            None => found.push(format!("{name}: its condition on the part is not read")),
        }
        let build = format!(
            "{} {} {}",
            text(&step["env"]["RUSTFLAGS"]),
            text(&step["env"]["CARGO_TARGET_DIR"]),
            text(&step["if"]).contains("'debug'")
        );
        let words: Vec<&str> = run.split_whitespace().collect();
        if golden {
            let share = words
                .iter()
                .position(|w| *w == "--part")
                .and_then(|at| words.get(at + 1))
                .map_or_else(|| "1/1".to_owned(), |s| (*s).to_owned());
            let whole = run.replacen(&format!("--part {share}"), "", 1);
            goldens
                .entry(format!("{build} {whole}"))
                .or_default()
                .push(share);
        } else if let Some(rest) =
            run.strip_prefix("cargo test -p geom-num --release --locked --no-fail-fast ")
        {
            vectors.entry(build).or_default().extend(
                rest.split(" --")
                    .map(|s| s.trim_start_matches("--").to_owned()),
            );
        }
    }
    for (build, shares) in &goldens {
        let m = shares.len();
        let wanted: BTreeSet<String> = (1..=m).map(|j| format!("{j}/{m}")).collect();
        let got: BTreeSet<String> = shares.iter().cloned().collect();
        if got != wanted || shares.len() != got.len() {
            found.push(format!(
                "the goldens of `{build}` run shares {shares:?}, not each of {wanted:?} once"
            ));
        }
    }
    if vectors.len() != 2 {
        found.push(format!(
            "geom-num's every test vector runs in {} builds, not as shipped and for the runner's \
             own CPU",
            vectors.len()
        ));
    }
    for (build, runs) in &vectors {
        found.extend(vector_problems(build, runs, tests));
    }
    found
}

/// What is wrong with what one build of geom-num's every test vector runs (`runs`, each step's
/// target selections): its library, binaries and doctests not once, a share not of the form
/// `[x-y]*`, or a file of its `tests` folder (`tests`) in no share or in two.
fn vector_problems(build: &str, runs: &[String], tests: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for whole in ["lib", "bins", "doc"] {
        let times = runs.iter().filter(|r| *r == whole).count();
        if times != 1 {
            found.push(format!(
                "geom-num's --{whole} runs {times} times in `{build}`"
            ));
        }
    }
    let mut shares = Vec::new();
    for r in runs
        .iter()
        .filter(|r| !["lib", "bins", "doc"].contains(&r.as_str()))
    {
        match r.strip_prefix("test ").and_then(letters) {
            Some(l) => shares.push(l),
            None => found.push(format!("geom-num's share --{r} in `{build}` is not read")),
        }
    }
    for t in tests {
        let times = shares.iter().filter(|l| takes(l, t)).count();
        if times != 1 {
            found.push(format!(
                "geom-num's tests/{t}.rs runs {times} times in `{build}`"
            ));
        }
    }
    found
}

/// The parts of a matrix's `k`, checked to be 1 to n, and the problem when they are not.
fn numbered(job: &str, k: &Yaml, found: &mut Vec<String>) -> Vec<i64> {
    let parts: Vec<i64> = k
        .as_vec()
        .into_iter()
        .flatten()
        .filter_map(Yaml::as_i64)
        .collect();
    let n = i64::try_from(parts.len()).unwrap();
    if n == 0 || parts != (1..=n).collect::<Vec<_>>() {
        found.push(format!("{job}'s parts are {parts:?}, not 1 to {n}"));
    }
    parts
}

/// What is wrong with how `wasm` shares a runtime's leg among its parts: parts not 1 to n or a
/// name that does not count them, a golden, test or guard step on no part or on more than one,
/// and a golden build's `--part j/m` shares not exactly 1 to m (one dropped from every part, or
/// run twice). A golden build is its flags, whether it is in release, and its command without
/// `--part`; the guard's proof runs one whole golden run, which the shares of its build make up.
fn wasm_part_problems(w: &Yaml) -> Vec<String> {
    let mut found = Vec::new();
    let wasm = &w["jobs"]["wasm"];
    let parts = numbered("wasm", &wasm["strategy"]["matrix"]["k"], &mut found);
    let n = parts.len();
    if !text(&wasm["name"]).ends_with(&format!("part ${{{{ matrix.k }}}} of {n})")) {
        found.push(format!("wasm's name does not count its {n} parts"));
    }
    let mut goldens: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut guarded = Vec::new();
    for step in wasm["steps"].as_vec().into_iter().flatten() {
        let run = text(&step["run"]);
        let name = text(&step["name"]);
        let golden = run.starts_with("cargo run --locked -p geom-test --bin golden");
        let guard = run.contains("--bin guard -- prove");
        if !(golden || guard || run.starts_with("cargo test")) {
            continue;
        }
        match parts_of(&text(&step["if"]), &parts) {
            Some(on) if on.len() == 1 => {}
            Some(on) => found.push(format!("{name}: runs on parts {on:?}, not exactly one")),
            None => found.push(format!("{name}: its condition on the part is not read")),
        }
        if guard {
            guarded.push(
                run.split("--golden-run")
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .to_owned(),
            );
        }
        if golden {
            let words: Vec<&str> = run.split_whitespace().collect();
            let share = words
                .iter()
                .position(|w| *w == "--part")
                .and_then(|at| words.get(at + 1))
                .map_or_else(|| "1/1".to_owned(), |s| (*s).to_owned());
            let whole = run.replacen(&format!("--part {share} "), "", 1);
            goldens
                .entry(format!("{} | {whole}", text(&step["env"]["RUSTFLAGS"])))
                .or_default()
                .push(share);
        }
    }
    for (build, shares) in &goldens {
        let m = shares.len();
        let wanted: BTreeSet<String> = (1..=m).map(|j| format!("{j}/{m}")).collect();
        let got: BTreeSet<String> = shares.iter().cloned().collect();
        if got != wanted || shares.len() != got.len() {
            found.push(format!(
                "wasm's goldens of `{build}` run shares {shares:?}, not each of {wanted:?} once"
            ));
        }
    }
    if goldens.len() != 3 {
        found.push(format!(
            "wasm runs the goldens in {} builds, not with overflow checks on, as shipped and in \
             debug",
            goldens.len()
        ));
    }
    if guarded.len() != 1 {
        found.push(format!(
            "wasm proves the guard {} times, not once",
            guarded.len()
        ));
    }
    found
}

/// What is wrong with how the wasm32 suite is built once and run in parts: `wasm-build`'s builds
/// are not the two of `wasm-suite`'s parts, a build does not archive what its parts run or does
/// not upload it under the name its parts download, its doctests (which nextest does not run) not
/// once under each runtime, a build's parts not `--partition count:k/n` for exactly k = 1 to n
/// under each runtime (a part dropped, or run twice), or the parts not waiting for the build.
fn wasm_suite_part_problems(w: &Yaml) -> Vec<String> {
    let mut found = Vec::new();
    let build = &w["jobs"]["wasm-build"];
    let suite = &w["jobs"]["wasm-suite"];
    let needs: Vec<String> = suite["needs"]
        .as_vec()
        .into_iter()
        .flatten()
        .map(text)
        .collect();
    if !needs.contains(&"wasm-build".to_owned()) {
        found.push("wasm-suite's parts do not wait for wasm-build".to_owned());
    }
    let builds = strings(&build["strategy"]["matrix"]["build"]);
    let runtimes = strings(&suite["strategy"]["matrix"]["runtime"]);
    if runtimes != ["wasmtime", "node"] {
        found.push(format!(
            "wasm-suite runs under {runtimes:?}, not wasmtime and node"
        ));
    }
    let steps: Vec<&Yaml> = build["steps"].as_vec().into_iter().flatten().collect();
    let mut parts: BTreeMap<String, Vec<(i64, i64)>> = BTreeMap::new();
    for p in suite["strategy"]["matrix"]["part"]
        .as_vec()
        .into_iter()
        .flatten()
    {
        let (Some(k), Some(n)) = (p["k"].as_i64(), p["n"].as_i64()) else {
            found.push(format!("a wasm-suite part is not read: {p:?}"));
            continue;
        };
        parts.entry(text(&p["build"])).or_default().push((k, n));
    }
    let wanted: BTreeSet<String> = builds.iter().cloned().collect();
    if parts.keys().cloned().collect::<BTreeSet<_>>() != wanted {
        found.push(format!(
            "wasm-suite's parts run the builds {:?}, but wasm-build makes {wanted:?}",
            parts.keys()
        ));
    }
    for (b, ks) in &parts {
        let n = ks.first().map_or(0, |(_, n)| *n);
        let mut got: Vec<i64> = ks.iter().map(|(k, _)| *k).collect();
        got.sort_unstable();
        if ks.iter().any(|(_, m)| *m != n) || got != (1..=n).collect::<Vec<_>>() {
            found.push(format!(
                "wasm-suite runs the {b} build's parts {ks:?}, not each of 1 to {n} once"
            ));
        }
    }
    found.extend(wasm_build_problems(&steps, &builds));
    let suite_steps: Vec<&Yaml> = suite["steps"].as_vec().into_iter().flatten().collect();
    let downloads = suite_steps.iter().any(|s| {
        text(&s["uses"]).starts_with("actions/download-artifact@")
            && text(&s["with"]["name"]) == "wasm-${{ matrix.part.build }}"
    });
    let runs = suite_steps.iter().any(|s| {
        text(&s["run"])
            == "cargo nextest run --archive-file \"$RUNNER_TEMP/wasm-${{ matrix.part.build }}.tar.zst\" \
                --workspace-remap . --no-fail-fast --partition count:${{ matrix.part.k }}/${{ matrix.part.n }}"
    });
    if !(downloads && runs) {
        found.push(
            "a wasm-suite part does not run its share of the archive its build uploaded".to_owned(),
        );
    }
    found
}

/// What is wrong with `wasm-build`'s steps (`steps`) for its builds (`builds`): a build that does
/// not archive its tests, its doctests not once under each runtime, or its archive not uploaded
/// under the name its parts download.
fn wasm_build_problems(steps: &[&Yaml], builds: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for b in builds {
        let of_build: Vec<&&Yaml> = steps
            .iter()
            .filter(|s| text(&s["if"]).contains(&format!("matrix.build == '{b}'")))
            .collect();
        let archived = of_build.iter().any(|s| {
            text(&s["run"]).starts_with(&format!(
                "cargo nextest archive $KERNEL_CRATES --target wasm32-wasip1 --release --locked \
                 --archive-file \"$RUNNER_TEMP/wasm-{b}.tar.zst\""
            ))
        });
        if !archived {
            found.push(format!("wasm-build does not archive the {b} build's tests"));
        }
        for runtime in ["wasmtime", "node"] {
            let doctests = of_build
                .iter()
                .filter(|s| {
                    text(&s["run"])
                        .starts_with("cargo test $KERNEL_CRATES --doc --target wasm32-wasip1")
                        && text(&s["env"]["CARGO_TARGET_WASM32_WASIP1_RUNNER"]).starts_with(runtime)
                })
                .count();
            if doctests != 1 {
                found.push(format!(
                    "the {b} build's doctests run {doctests} times under {runtime}"
                ));
            }
        }
    }
    let uploads = steps.iter().any(|s| {
        text(&s["uses"]).starts_with("actions/upload-artifact@")
            && text(&s["with"]["name"]) == "wasm-${{ matrix.build }}"
            && text(&s["with"]["path"]) == "${{ runner.temp }}/wasm-${{ matrix.build }}.tar.zst"
    });
    if !uploads {
        found.push("wasm-build does not upload its archive as wasm-<build>".to_owned());
    }
    found
}

/// geom-num's test targets: the files of its `tests` folder.
fn geom_num_tests() -> Vec<String> {
    crate_tests("geom-num")
}

/// A kernel crate's test targets: the files of its `tests` folder.
fn crate_tests(krate: &str) -> Vec<String> {
    let dir = root().join(format!("projects/engines/geometry/crates/{krate}/tests"));
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            name.strip_suffix(".rs").map(str::to_owned)
        })
        .collect();
    out.sort();
    out
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
        (
            "if [ \"$EVENT\" = push ] && [ \"$GITHUB_REF\" = refs/heads/main ]; then\n  if marks=",
            "the queue's mark on a push to main alone, so a run by hand skips nothing",
        ),
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
            " wasm, wasm-build, wasm-suite, big-endian, large-models]",
            " wasm, wasm-build, wasm-suite, large-models]"
        )
        .iter()
        .any(|p| p.contains("does not judge [\"big-endian\"]"))
    );
    // The wasm32 suite's one build left out of the last job: a failed build would go unjudged.
    assert!(
        seeded(
            "kernel.yml",
            " wasm, wasm-build, wasm-suite, big-endian, large-models]",
            " wasm, wasm-suite, big-endian, large-models]"
        )
        .iter()
        .any(|p| p.contains("does not judge [\"wasm-build\"]"))
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
            "merge_group) must=\"lint scope golden-runner test suite wasm wasm-build wasm-suite big-endian large-models\"",
            "merge_group) must=\"lint scope golden-runner test suite wasm-build wasm-suite big-endian large-models\"",
            "on merge_group the jobs",
        ),
        // The goldens' runner's build dropped from what the queue must run.
        (
            "merge_group) must=\"lint scope golden-runner test suite wasm wasm-build wasm-suite big-endian large-models\"",
            "merge_group) must=\"lint scope test suite wasm wasm-build wasm-suite big-endian large-models\"",
            "on merge_group the jobs",
        ),
        // The wasm32 suite's build dropped from what the queue must run.
        (
            "merge_group) must=\"lint scope golden-runner test suite wasm wasm-build wasm-suite big-endian large-models\"",
            "merge_group) must=\"lint scope golden-runner test suite wasm wasm-suite big-endian large-models\"",
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
        // A job a push runs left out of a run by hand.
        (
            "    if: github.event_name != 'pull_request' && needs.changes.outputs.kernel == 'true' && needs.changes.outputs.tested != 'true'\n    runs-on: ubuntu-24.04\n",
            "    if: github.event_name != 'pull_request' && github.event_name != 'workflow_dispatch' && needs.changes.outputs.kernel == 'true' && needs.changes.outputs.tested != 'true'\n    runs-on: ubuntu-24.04\n",
            "a run by hand runs",
        ),
        // The last job asking less of a run by hand than of a push.
        (
            "              *) must=",
            "              workflow_dispatch) must=\"lint scope linux guard\" ;;\n              *) must=",
            "of a run by hand, but",
        ),
        // No run by hand at all.
        (
            "  # By hand, on any branch: what a push to main runs (the paragraph at the top). No inputs.\n  workflow_dispatch:\n",
            "",
            "kernel.yml has no workflow_dispatch",
        ),
        // A run by hand given an input that could choose what runs.
        (
            "  workflow_dispatch:\n",
            "  workflow_dispatch:\n    inputs:\n      jobs:\n        type: string\n",
            "takes inputs",
        ),
    ] {
        let k = kernel.replacen(from, to, 1);
        assert_ne!(k, kernel, "the planted edit {from:?} applies");
        let found = event_problems(&k);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }

    // An arm naming a push and a run by hand together is read as both, and asks the same.
    let full =
        "lint scope linux guard test suite wasm wasm-build wasm-suite big-endian large-models";
    let both = kernel.replacen(
        "              *) must=",
        &format!(
            "              push|workflow_dispatch) must=\"{full}\" ;;\n              *) must="
        ),
        1,
    );
    assert_ne!(both, kernel);
    assert_eq!(event_problems(&both), Vec::<String>::new());
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn a_run_by_hand_writes_nothing_later_runs_read() {
    let kernel = read(&format!("{WORKFLOWS}/kernel.yml"));
    assert_eq!(writer_problems(&kernel), Vec::<String>::new());

    // Seeded violations: each must be reported.
    for (from, to, said) in [
        // A cache saved by any run of main, a run by hand on main among them.
        (
            "save-if: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' && matrix.k == 1 }}",
            "save-if: ${{ github.ref == 'refs/heads/main' && matrix.k == 1 }}",
            "linux's step 3",
        ),
        // A cache with no `save-if`, which saves on every run.
        (
            "          save-if: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}\n          key: guard\n",
            "          key: guard\n",
            "guard's step 3",
        ),
        // A run by hand let in beside the terms.
        (
            "save-if: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}\n          key: large-models",
            "save-if: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' || github.event_name == 'workflow_dispatch' }}\n          key: large-models",
            "large-models's step 3",
        ),
        // The queue's mark uploaded on every event.
        (
            "        if: github.event_name == 'merge_group' && needs.changes.outputs.kernel == 'true'\n        with:",
            "        if: needs.changes.outputs.kernel == 'true'\n        with:",
            "done's step 3",
        ),
        // The in-run archive hand-over kept for a month, for later runs to find.
        (
            "          retention-days: 1
          # The archive is compressed already.",
            "          retention-days: 30
          # The archive is compressed already.",
            "wasm-build's step 8",
        ),
        // The same upload under another name, which no part of this run downloads.
        (
            "          name: wasm-${{ matrix.build }}
          path:",
            "          name: kernel-archive-${{ matrix.build }}
          path:",
            "wasm-build's step 8",
        ),
        // The runner's hand-over kept for a month, for later runs to find.
        (
            "          retention-days: 1\n          if-no-files-found: error\n",
            "          retention-days: 30\n          if-no-files-found: error\n",
            "golden-runner's step 4",
        ),
        // The runner uploaded under a name no part of this run downloads.
        (
            "          name: golden-runner-${{ matrix.target.os }}\n          path: ${{ runner.temp }}/golden-runner\n          retention-days: 1",
            "          name: runner-${{ matrix.target.os }}\n          path: ${{ runner.temp }}/golden-runner\n          retention-days: 1",
            "golden-runner's step 4",
        ),
        // A timing record written through `gh` on every run.
        (
            "      - name: The merge queue's mark for this tree\n",
            "      - name: A timing record for later runs\n        run: gh api -X PUT \"repos/$GITHUB_REPOSITORY/actions/variables/TIMES\" -f value=1\n      - name: The merge queue's mark for this tree\n",
            "(A timing record for later runs)",
        ),
    ] {
        let k = kernel.replacen(from, to, 1);
        assert_ne!(k, kernel, "the planted edit {from:?} applies");
        let found = writer_problems(&k);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
#[allow(clippy::too_many_lines, reason = "the cases are written out in full")]
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
            "    runs-on: ubuntu-latest\n    timeout-minutes: 120\n    strategy:\n      fail-fast: false\n      matrix:\n        k: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]",
            "    runs-on: ubuntu-24.04-arm\n    timeout-minutes: 120\n    strategy:\n      fail-fast: false\n      matrix:\n        k: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]",
            "the goldens run 0 times on ubuntu-latest in release",
        ),
        // A part of the pull request's Linux job dropped: its goldens run nowhere.
        (
            "        k: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]",
            "        k: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]",
            "Every golden in release for this machine's own CPU (part 2 of 2): runs on parts [], not exactly one",
        ),
        // A golden step on two parts: run twice.
        (
            "      - name: Every golden in release as shipped, without overflow checks (part 1 of 2)\n        if: ${{ !cancelled() && matrix.k == 12 }}",
            "      - name: Every golden in release as shipped, without overflow checks (part 1 of 2)\n        if: ${{ !cancelled() && matrix.k <= 12 }}",
            "Every golden in release as shipped, without overflow checks (part 1 of 2): runs on parts [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12], not exactly one",
        ),
        // The tests on fewer parts than there are shares: the last share's crates run nowhere.
        (
            "        if: ${{ !cancelled() && matrix.k <= 9 && env.KERNEL_CRATES != '' }}",
            "        if: ${{ !cancelled() && matrix.k <= 8 && env.KERNEL_CRATES != '' }}",
            "runs on parts [1, 2, 3, 4, 5, 6, 7, 8], not the shares' and the crate apart's",
        ),
        // A Linux part runs only a partition of its share: the rest of its tests run nowhere.
        (
            "          CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS: \"true\"\n        run: cargo nextest run $KERNEL_CRATES --release --locked --no-fail-fast\n",
            "          CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS: \"true\"\n        run: cargo nextest run $KERNEL_CRATES --release --locked --no-fail-fast --partition count:${{ matrix.k }}/2\n",
            "runs a partition or a selection of its share",
        ),
        // A Linux part takes another part's share: its own share runs nowhere.
        (
            "|| fromJSON(needs.scope.outputs.parts)[format('{0}', matrix.k)] }}",
            "|| fromJSON(needs.scope.outputs.parts)[format('{0}', 1)] }}",
            "linux's parts do not take their own share",
        ),
        // A release part of the suite dropped: its share's crates run nowhere on any platform.
        (
            "          - { mode: release, k: 8, n: 25, doc: true }\n",
            "",
            "take a share, not the scope's shares 1 to 8",
        ),
        // The scope gives one share more than there are parts to run it.
        (
            "--parts 8 --apart geom-num",
            "--parts 9 --apart geom-num",
            "take a share, not the scope's shares 1 to 9",
        ),
        // `done` checks a different number of shares than the parts run.
        (
            "jq -e --argjson n 8",
            "jq -e --argjson n 7",
            "done does not check the scope's 8 shares",
        ),
        // A suite part takes another part's share.
        (
            "matrix.part.crate || fromJSON(needs.scope.outputs.parts)[format('{0}', matrix.part.k)]",
            "matrix.part.crate || fromJSON(needs.scope.outputs.parts)[format('{0}', 1)]",
            "suite's parts do not take their own share",
        ),
        // A test target of geom-test that no part runs: its first letter in no share.
        (
            "targets: \"--test '[s-z]*'\"",
            "targets: \"--test '[t-z]*'\"",
            "geom-test's tests/scan_repo.rs runs 0 times",
        ),
        // A test target of another crate set apart (geom-fuzz) that no part runs.
        (
            "crate: geom-fuzz, targets: \"--test '[m-z]*'\"",
            "crate: geom-fuzz, targets: \"--test '[n-z]*'\"",
            "geom-fuzz's tests/make.rs runs 0 times in `suite's geom-fuzz parts`",
        ),
        // A test target of geom-num that no suite part runs: an attack test whose name is in no
        // share of the suite's geom-num parts.
        (
            "targets: \"--test 'attack_[h-r]*'\"",
            "targets: \"--test 'attack_[q-r]*'\"",
            "geom-num's tests/attack_plane.rs runs 0 times in `suite's geom-num parts`",
        ),
        // A test target of geom-num that two suite parts run: two shares that overlap.
        (
            "crate: geom-num, targets: \"--test '[b-z]*'\"",
            "crate: geom-num, targets: \"--test '[a-z]*'\"",
            "geom-num's tests/attack_plane.rs runs 2 times in `suite's geom-num parts`",
        ),
        // A share in a form the reading does not know is looked at, not passed.
        (
            "targets: \"--test 'attack2_[p-z]*' --test",
            "targets: \"--test 'attack?*' --test",
            "geom-num's share --test 'attack?*' in `suite's geom-num parts` is not read",
        ),
        // geom-test's doctests in two of its parts.
        (
            "{ mode: release, k: 16, n: 25, crate: geom-test,",
            "{ mode: release, k: 16, n: 25, doc: true, crate: geom-test,",
            "geom-test's --doc runs 2 times",
        ),
        // geom-test kept in the shares as well as in parts of its own: run twice.
        (
            "--parts 8 --apart geom-num --apart geom-test",
            "--parts 8 --apart geom-num",
            "the scope sets apart [\"geom-num\", \"geom-bench\", \"geom-format\", \"geom-fuzz\"]",
        ),
        // A release part runs a selection of its crates' targets.
        (
            "--no-fail-fast ${{ matrix.part.targets }}",
            "--no-fail-fast --lib ${{ matrix.part.targets }}",
            "suite's release parts do not run their own crates and targets",
        ),
        // A part added without its name counting it.
        (
            "optimised with checks on, part ${{ matrix.k }} of 16)",
            "optimised with checks on, part ${{ matrix.k }} of 15)",
            "linux's name does not count its 16 parts",
        ),
        // The pull request's tests without debug assertions.
        (
            "          CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS: \"true\"\n        run: cargo nextest run $KERNEL_CRATES --release --locked --no-fail-fast\n",
            "        run: cargo nextest run $KERNEL_CRATES --release --locked --no-fail-fast\n",
            "the pull request's tests are not optimised",
        ),
        // The queue's suite without debug assertions.
        (
            "      # Debug assertions on in the release legs too (the release steps add overflow checks).\n      CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS: \"true\"\n",
            "",
            "without debug assertions",
        ),
        // A release part dropped from `test`: its step runs nowhere.
        (
            "        k: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46]",
            "        k: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45]",
            "geom-num's every test vector in release for this machine's own CPU (part 6 of 6): runs on parts [], not exactly one",
        ),
        // A golden step on two parts.
        (
            "        if: ${{ !cancelled() && matrix.mode == 'release' && matrix.k == 1 }}",
            "        if: ${{ !cancelled() && matrix.mode == 'release' && matrix.k <= 2 }}",
            "(part 1 of 11): runs on parts [1, 2], not exactly one",
        ),
        // A golden share dropped from every part: the second share run as the first again.
        (
            "run --part 2/11 --release",
            "run --part 1/11 --release",
            "run shares [\"1/11\", \"1/11\"",
        ),
        // A debug leg on a part past its last.
        (
            "          - { mode: debug, k: 46 }\n",
            "",
            "the debug legs' parts are not 1 to 5",
        ),
        // A debug golden share dropped from every part: the second share run as the first again.
        (
            "'\"$GOLDEN\" run --part 2/4'",
            "'\"$GOLDEN\" run --part 1/4'",
            "run shares [\"1/4\", \"1/4\"",
        ),
        // A part added without the name counting it.
        (
            "matrix.mode == 'release' && 46 || 4)",
            "matrix.mode == 'release' && 45 || 4)",
            "test's name does not count its 46 release and 4 debug parts",
        ),
        // geom-num's test targets of one first letter in no share.
        (
            "--test '[b-z]*'",
            "--test '[e-z]*'",
            "geom-num's tests/dd.rs runs 0 times",
        ),
        // geom-num's doctests dropped.
        (
            "--no-fail-fast --doc",
            "--no-fail-fast --lib",
            "geom-num's --doc runs 0 times",
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

/// How a step of `test` runs the goldens: the runner `golden-runner` built, by the path its
/// digest check put in `GOLDEN`.
const RUNS_GOLDEN: &str = "\"$GOLDEN\" run";
/// The artifact a system's runner goes to its parts as, named by the system's label.
const RUNNER_ARTIFACT: &str = "golden-runner-${{ matrix.target.os }}";
/// The digest check's tests: a full sha256 given, the file there, and its digest that one.
const RUNNER_CHECKS: [&str; 3] = [
    "[[ $want =~ ^[0-9a-f]{64}$ ]] ||",
    "[ -f \"$golden\" ] ||",
    "[ \"$got\" = \"$want\" ] ||",
];

/// A target's output name in `golden-runner` (`macOS x86-64` is `macos-x86-64`) and its digest's
/// variable in the check (`SHA256_MACOS_X86_64`).
fn runner_key(target: &str) -> (String, String) {
    let key = target.to_lowercase().replace(' ', "-");
    let var = format!("SHA256_{}", key.to_uppercase().replace('-', "_"));
    (key, var)
}

/// The targets of a job's matrix, as (name, label).
fn targets(job: &Yaml) -> Vec<(String, String)> {
    job["strategy"]["matrix"]["target"]
        .as_vec()
        .into_iter()
        .flatten()
        .map(|t| (text(&t["name"]), text(&t["os"])))
        .collect()
}

/// What is wrong with how `test` gets its runner: `golden-runner` not building it once for each
/// of `test`'s targets, alone, with no build flags of a part, and giving its sha256 as that
/// target's output and uploading it whole (a missing file failing the upload); a part that does
/// not download this run's runner on every part, unguarded, before it checks it; a check that
/// runs on some parts only, lacks a target's digest or one of [`RUNNER_CHECKS`], or does not give
/// the checked path; a golden step that runs anything but that runner, or before the check; and
/// a step that builds the runner in a part.
#[allow(
    clippy::too_many_lines,
    reason = "one list of the hand-over's rules, read in order"
)]
fn runner_problems(w: &Yaml) -> Vec<String> {
    let mut found = Vec::new();
    let builder = &w["jobs"]["golden-runner"];
    let test = &w["jobs"]["test"];
    if builder.is_badvalue() {
        return vec!["kernel.yml has no job golden-runner".to_owned()];
    }
    let wanted = targets(test);
    if targets(builder) != wanted {
        found.push(format!(
            "golden-runner builds for {:?}, not test's targets {wanted:?}",
            targets(builder)
        ));
    }
    if text(&builder["if"]) != text(&test["if"]) {
        found.push("golden-runner does not run exactly when test does".to_owned());
    }
    if !strings(&test["needs"]).contains(&"golden-runner".to_owned()) {
        found.push("test does not wait for golden-runner".to_owned());
    }
    let steps: Vec<&Yaml> = builder["steps"].as_vec().into_iter().flatten().collect();
    let build = steps
        .iter()
        .find(|s| text(&s["run"]).starts_with("cargo build --locked -p geom-test --bin golden\n"));
    match build {
        None => found.push("golden-runner does not build the runner alone".to_owned()),
        Some(b) => {
            if !b["env"]["RUSTFLAGS"].is_badvalue() || !b["env"]["CARGO_TARGET_DIR"].is_badvalue()
            {
                found.push("golden-runner builds the runner with a part's flags".to_owned());
            }
            if !text(&b["run"]).contains("echo \"$key=$sha256\" >> \"$GITHUB_OUTPUT\"") {
                found.push("golden-runner does not give the runner's sha256".to_owned());
            }
            if !b["if"].is_badvalue() {
                found.push("golden-runner's build has a condition".to_owned());
            }
        }
    }
    for (name, _) in &wanted {
        let (key, _) = runner_key(name);
        let output = text(&builder["outputs"][key.as_str()]);
        if output != format!("${{{{ steps.build.outputs['{key}'] }}}}") {
            found.push(format!(
                "golden-runner's output {key} is {output:?}, not its build's"
            ));
        }
    }
    let upload = steps
        .iter()
        .find(|s| text(&s["uses"]).starts_with("actions/upload-artifact@"));
    if !upload.is_some_and(|u| {
        text(&u["with"]["name"]) == RUNNER_ARTIFACT
            && text(&u["with"]["if-no-files-found"]) == "error"
            && u["if"].is_badvalue()
    }) {
        found.push(format!(
            "golden-runner does not upload {RUNNER_ARTIFACT} on every leg, failing without it"
        ));
    }
    let steps: Vec<&Yaml> = test["steps"].as_vec().into_iter().flatten().collect();
    let download = steps.iter().position(|s| {
        text(&s["uses"]).starts_with("actions/download-artifact@")
            && text(&s["with"]["name"]) == RUNNER_ARTIFACT
    });
    let check = steps.iter().position(|s| {
        let run = text(&s["run"]);
        RUNNER_CHECKS.iter().any(|c| run.contains(c))
    });
    match download {
        None => found.push(format!("test does not download {RUNNER_ARTIFACT}")),
        Some(d) => {
            let s = steps[d];
            if !s["if"].is_badvalue() || !s["continue-on-error"].is_badvalue() {
                found.push("test's download of its runner is not on every part, unguarded".into());
            }
            if !text(&s["with"]["repository"]).is_empty() || !text(&s["with"]["run-id"]).is_empty()
            {
                found.push("test's runner is downloaded from another run".to_owned());
            }
            if check.is_some_and(|c| c < d) {
                found.push("test checks its runner before it downloads it".to_owned());
            }
        }
    }
    let Some(c) = check else {
        found.push("test does not check its runner's digest".to_owned());
        return found;
    };
    let s = steps[c];
    let run = text(&s["run"]);
    if !s["if"].is_badvalue() || !s["continue-on-error"].is_badvalue() {
        found.push("test's runner check is not on every part, unguarded".to_owned());
    }
    for needed in RUNNER_CHECKS {
        if !run.contains(needed) {
            found.push(format!("test's runner check lacks `{needed}`"));
        }
    }
    if !run.contains("echo \"GOLDEN=$golden\" >> \"$GITHUB_ENV\"") {
        found.push("test's runner check does not give the path it checked".to_owned());
    }
    for (name, _) in &wanted {
        let (key, var) = runner_key(name);
        let got = text(&s["env"][var.as_str()]);
        if got != format!("${{{{ needs.golden-runner.outputs['{key}'] }}}}") {
            found.push(format!(
                "test's runner check reads {var} as {got:?}, not golden-runner's output {key}"
            ));
        }
    }
    for (i, s) in steps.iter().enumerate() {
        let run = text(&s["run"]);
        let name = text(&s["name"]);
        if run.contains("--bin golden") {
            found.push(format!("{name}: builds the runner in a part"));
        }
        if name.starts_with("Every golden") && !run.starts_with(RUNS_GOLDEN) {
            found.push(format!("{name}: does not run the checked runner"));
        }
        if run.starts_with(RUNS_GOLDEN) && i < c {
            found.push(format!("{name}: runs the runner before it is checked"));
        }
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_goldens_run_only_the_runner_their_run_built_and_checked() {
    let kernel = read(&format!("{WORKFLOWS}/kernel.yml"));
    assert_eq!(runner_problems(&parse(&kernel)), Vec::<String>::new());

    // Seeded violations: each must be reported.
    for (from, to, said) in [
        // The digest no longer compared: any file named golden would run.
        (
            "          [ \"$got\" = \"$want\" ] ||\n",
            "          [ -n \"$got\" ] ||\n",
            "lacks `[ \"$got\" = \"$want\" ] ||`",
        ),
        // A missing runner passed over, so a part could pass having run nothing.
        (
            "          [ -f \"$golden\" ] || { echo",
            "          [ -f \"$golden\" ] && { echo",
            "lacks `[ -f \"$golden\" ] ||`",
        ),
        // The download let fail quietly.
        (
            "      - uses: actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333 # v8.0.2\n        with:\n          name: golden-runner-",
            "      - uses: actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333 # v8.0.2\n        continue-on-error: true\n        with:\n          name: golden-runner-",
            "not on every part, unguarded",
        ),
        // The check skipped on the debug parts.
        (
            "      - name: Check the goldens' runner by the digest its build gave\n",
            "      - name: Check the goldens' runner by the digest its build gave\n        if: ${{ matrix.mode == 'release' }}\n",
            "test's runner check is not on every part",
        ),
        // One system's digest not handed to the check.
        (
            "          SHA256_MACOS_X86_64: ${{ needs.golden-runner.outputs['macos-x86-64'] }}\n",
            "",
            "reads SHA256_MACOS_X86_64",
        ),
        // A part building the runner again (and the minutes with it).
        (
            "        run: '\"$GOLDEN\" run --part 3/11 --release'",
            "        run: cargo run --locked -p geom-test --bin golden -- run --part 3/11 --release",
            "builds the runner in a part",
        ),
        // A system the runner is not built for.
        (
            "          - name: macOS x86-64\n            os: macos-15-intel\n    defaults:",
            "    defaults:",
            "golden-runner builds for",
        ),
        // The upload let pass with no file.
        (
            "          retention-days: 1\n          if-no-files-found: error\n",
            "          retention-days: 1\n          if-no-files-found: warn\n",
            "failing without it",
        ),
        // The runner built with a part's flags.
        (
            "      - name: Build the goldens' runner and give its sha256\n        id: build\n        env:\n",
            "      - name: Build the goldens' runner and give its sha256\n        id: build\n        env:\n          RUSTFLAGS: -C target-cpu=native\n",
            "with a part's flags",
        ),
    ] {
        let k = kernel.replacen(from, to, 1);
        assert_ne!(k, kernel, "the planted edit {from:?} applies");
        let found = runner_problems(&parse(&k));
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_wasm32_legs_run_every_golden_and_test_once_in_parts() {
    let kernel = read(&format!("{WORKFLOWS}/kernel.yml"));
    assert_eq!(leg_problems(&kernel), Vec::<String>::new());

    // Seeded violations: each must be reported.
    for (from, to, said) in [
        // A part of the wasm32 goldens dropped from the matrix: its share runs nowhere.
        (
            "        k: [1, 2, 3, 4, 5, 6, 7, 8]",
            "        k: [1, 2, 3, 4, 5, 6, 7]",
            "Every golden on wasm32 in debug (part 3 of 3): runs on parts [], not exactly one",
        ),
        // A wasm32 golden step on two parts: run twice.
        (
            "        if: ${{ !cancelled() && matrix.k == 3 }}\n        env:\n          RUSTFLAGS: -C overflow-checks=on\n        run: cargo run --locked -p geom-test --bin golden -- run --part 2/2",
            "        if: ${{ !cancelled() && matrix.k <= 3 }}\n        env:\n          RUSTFLAGS: -C overflow-checks=on\n        run: cargo run --locked -p geom-test --bin golden -- run --part 2/2",
            "(part 2 of 2): runs on parts [1, 2, 3], not exactly one",
        ),
        // A wasm32 golden share in no part: the second debug share run as the first again.
        (
            "run --part 2/3 --target wasm32-wasip1\n",
            "run --part 1/3 --target wasm32-wasip1\n",
            "run shares [\"1/3\", \"1/3\", \"3/3\"]",
        ),
        // The wasm32 guard's proof on every part.
        (
            "        if: ${{ matrix.k == 1 }}\n        run: cargo run --locked --release -p geom-test --bin guard",
            "        run: cargo run --locked --release -p geom-test --bin guard",
            "A one-bit change to libm's sin fails a golden on wasm32: runs on parts [1, 2, 3, 4, 5, 6, 7, 8]",
        ),
        // A part added to the wasm32 goldens without its name counting it.
        (
            "    name: Test (wasm32-wasip1 under ${{ matrix.runtime }}, part ${{ matrix.k }} of 8)",
            "    name: Test (wasm32-wasip1 under ${{ matrix.runtime }}, part ${{ matrix.k }} of 7)",
            "wasm's name does not count its 8 parts",
        ),
        // A part of the wasm32 suite dropped: its share of the tests runs nowhere.
        (
            "          - { build: checked, k: 2, n: ",
            "          - { build: checked, k: 1, n: ",
            "wasm-suite runs the checked build's parts",
        ),
        // The shipped build's parts dropped: geom-num's vectors as shipped run nowhere.
        (
            "          - { build: shipped, k: 1, n: ",
            "          - { build: checked, k: 9, n: ",
            "wasm-suite's parts run the builds",
        ),
        // The parts no longer waiting for the build.
        (
            "    needs: [changes, lint, scope, wasm-build]\n",
            "    needs: [changes, lint, scope]\n",
            "do not wait for wasm-build",
        ),
        // A build's doctests dropped under one runtime.
        (
            "      - name: geom-num's doctests on wasm32 in release as shipped, under Node\n        if: ${{ !cancelled() && matrix.build == 'shipped'",
            "      - name: geom-num's doctests on wasm32 in release as shipped, under Node\n        if: ${{ !cancelled() && matrix.build == 'checked'",
            "the shipped build's doctests run 0 times under node",
        ),
        // The archive uploaded under a name no part downloads.
        (
            "          name: wasm-${{ matrix.build }}\n",
            "          name: wasm-suite-${{ matrix.build }}\n",
            "does not upload its archive",
        ),
        // A part that runs the whole archive, not its share.
        (
            "--workspace-remap . --no-fail-fast --partition count:${{ matrix.part.k }}/${{ matrix.part.n }}\n",
            "--workspace-remap . --no-fail-fast\n",
            "does not run its share",
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
            kernel.replacen(
                "if [ \"$EVENT\" = push ] && [ \"$GITHUB_REF\" = refs/heads/main ]; then",
                "if [ \"$EVENT\" != pull_request ]; then",
                1,
            ),
            quality.clone(),
            "the queue's mark on a push to main alone",
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

/// What is wrong with the count gates' legs and their kept base counts.
fn bench_problems(bench: &str) -> Vec<String> {
    let w = parse(bench);
    let mut found = Vec::new();
    if w["on"]["merge_group"].is_badvalue() {
        found.push("kernel-bench.yml does not run in the merge queue".to_owned());
    }
    let legs = w["jobs"]["scope"]["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .find(|s| s["id"].as_str() == Some("legs"))
        .map(|s| text(&s["run"]))
        .unwrap_or_default();
    for (needle, said) in [
        (
            "pull_request) legs=\"[$x86]\" ;;",
            "a pull request measures x86-64 alone",
        ),
        (
            "merge_group) legs=\"[$arm]\" ;;",
            "the merge queue measures aarch64",
        ),
        (
            "name=bench-aarch64-tested-$tree",
            "a push to main looks for the queue's mark",
        ),
        (
            "legs=\"[$x86]\"\n    fi",
            "a marked push measures x86-64 alone",
        ),
    ] {
        if !legs.contains(needle) {
            found.push(format!("the legs are wrong: {said}"));
        }
    }
    let judge = &w["jobs"]["instructions"];
    if text(&judge["strategy"]["matrix"]["include"]) != "${{ fromJSON(needs.scope.outputs.legs) }}"
    {
        found.push("the instruction legs are not the scope's list".to_owned());
    }
    found.extend(bench_part_problems(&w));
    let job = &w["jobs"]["instructions-base"];
    let steps: Vec<&Yaml> = job["steps"].as_vec().into_iter().flatten().collect();
    let at = |name: &str| steps.iter().position(|s| s["name"].as_str() == Some(name));
    let kept =
        at("The merge base's counts, if a trusted run kept them").map(|i| text(&steps[i]["run"]));
    let kept = kept.unwrap_or_default();
    let reset = kept.find("kept=false").unwrap_or(usize::MAX);
    let check = kept
        .find("sha256sum --check --strict manifest.sha256")
        .unwrap_or(0);
    let set = kept.find("kept=true").unwrap_or(0);
    if !(reset < check && check < set) {
        found.push("a missing or unchecked base count is taken as kept".to_owned());
    }
    for needle in [
        "if [ \"$branch\" = main ]; then",
        "[ \"$branch\" = \"$HEAD_REF\" ]",
        "[ \"$(git -C head rev-parse \"$sha:$workflow\" 2>/dev/null)\" = \"$mine\" ]",
        "name=\"bench-base-$TARGET-$tree-$print\"",
    ] {
        if !kept.contains(needle) {
            found.push(format!("the kept counts are trusted wrongly: {needle}"));
        }
    }
    let measure = at("Benchmark the merge base").map(|i| text(&steps[i]["if"]));
    if measure.as_deref()
        != Some("needs.scope.outputs.base_gate == 'true' && steps.kept.outputs.kept != 'true'")
    {
        found.push("a missing base count is not measured".to_owned());
    }
    // Nothing in the merge base's parts runs the change's code: the change is only read there.
    if at("Keep the merge base's counts").is_none() {
        found.push("the merge base's counts are not kept".to_owned());
    }
    for s in &steps {
        let run = text(&s["run"]);
        let heads_code = text(&s["working-directory"]) == "head" || run.contains("$HEAD_TOOLCHAIN");
        if heads_code && run.contains("cargo") {
            found.push(format!(
                "{} runs the change's code before the merge base is kept",
                text(&s["name"])
            ));
        }
    }
    let steps: Vec<&Yaml> = judge["steps"].as_vec().into_iter().flatten().collect();
    let mark = steps.iter().any(|s| {
        text(&s["with"]["name"]) == "bench-aarch64-tested-${{ env.TREE }}"
            && text(&s["if"])
                .contains("github.event_name == 'merge_group' && matrix.target == 'aarch64'")
    });
    if !mark {
        found.push("the merge queue does not mark the tree aarch64 passed".to_owned());
    }
    found
}

/// What is wrong with the instruction gate's parts: each leg's parts are the scope's list, read
/// from geom-bench/parts.tsv; the change's parts wait for the merge base's; the judge waits for
/// both, judges when a part failed, and checks every part of both sides by the artifact names the
/// parts upload; the planted slowdown runs in the part the scope names.
fn bench_part_problems(w: &Yaml) -> Vec<String> {
    let mut found = Vec::new();
    let jobs = &w["jobs"];
    let runs = |job: &str| -> String {
        jobs[job]["steps"]
            .as_vec()
            .into_iter()
            .flatten()
            .map(|s| format!("{}\n{}", text(&s["run"]), text(&s["with"]["name"])))
            .collect::<Vec<_>>()
            .join("\n")
    };
    for job in ["instructions-base", "instructions-parts"] {
        if text(&jobs[job]["strategy"]["matrix"]["include"])
            != "${{ fromJSON(needs.scope.outputs.parts) }}"
        {
            found.push(format!("the parts of {job} are not the scope's list"));
        }
    }
    let scope = runs("scope");
    for needle in [
        "plan=\"$BENCH/parts.tsv\"",
        "range(1; $of + 1) | $leg + {part: ., of: $of, trip: (. == $trip), ember: (. == $of)}",
        "if ($1 == \"sign\") s = $2",
    ] {
        if !scope.contains(needle) {
            found.push(format!("the scope's parts are not parts.tsv's: {needle}"));
        }
    }
    let needs = |job: &str| {
        jobs[job]["needs"]
            .as_vec()
            .into_iter()
            .flatten()
            .map(text)
            .collect::<Vec<_>>()
    };
    if !needs("instructions-parts").contains(&"instructions-base".to_owned()) {
        found.push("the change's parts do not wait for the merge base's".to_owned());
    }
    let judge_needs = needs("instructions");
    if !["instructions-base", "instructions-parts"]
        .iter()
        .all(|j| judge_needs.contains(&(*j).to_owned()))
    {
        found.push("the judge does not wait for every part".to_owned());
    }
    if !text(&jobs["instructions"]["if"]).contains("!cancelled()") {
        found.push("the judge does not judge when a part failed".to_owned());
    }
    if text(&jobs["instructions"]["env"]["OF"]) != "${{ needs.scope.outputs.of }}" {
        found.push("the judge does not read the scope's number of parts".to_owned());
    }
    let judge = runs("instructions");
    for needle in [
        "--name \"count-$side-$TARGET-$k\"",
        "for k in $(seq 1 \"$OF\"); do\n    gh run download",
        "sides=\"base head\"",
        "\"$counts\" parts \"$side\" \"$t/parts/$side\" \"$OF\" >> \"$t/parts.md\" || status=$?",
    ] {
        if !judge.contains(needle) {
            found.push(format!("the judge does not check every part: {needle}"));
        }
    }
    for (job, name) in [
        (
            "instructions-base",
            "count-base-${{ matrix.target }}-${{ matrix.part }}",
        ),
        (
            "instructions-parts",
            "count-head-${{ matrix.target }}-${{ matrix.part }}",
        ),
    ] {
        if !runs(job).contains(name) {
            found.push(format!("{job} does not upload its counts as {name}"));
        }
    }
    let trip = jobs["instructions-parts"]["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .find(|s| text(&s["run"]).contains("--bin trip -- gungraun"));
    if trip.is_none_or(|s| text(&s["if"]) != "matrix.trip") {
        found.push("the planted slowdown does not run in the part that measures sign".to_owned());
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_count_gate_parts_measure_everything_once_and_a_lost_part_fails() {
    let bench = read(&format!("{WORKFLOWS}/kernel-bench.yml"));
    let found = bench_part_problems(&parse(&bench));
    assert!(found.is_empty(), "{found:#?}");
    for (from, to, said) in [
        (
            "include: ${{ fromJSON(needs.scope.outputs.parts) }}",
            "include: ${{ fromJSON('[]') }}",
            "the parts of instructions-base",
        ),
        (
            "plan=\"$BENCH/parts.tsv\"",
            "plan=\"$BENCH/retired.txt\"",
            "not parts.tsv's",
        ),
        (
            "    needs: [scope, instructions-base]\n",
            "    needs: [scope]\n",
            "do not wait for the merge base's",
        ),
        (
            "    needs: [scope, instructions-base, instructions-parts]\n",
            "    needs: [scope, instructions-parts]\n",
            "does not wait for every part",
        ),
        (
            "is named.\n    if: ${{ !cancelled() && ",
            "is named.\n    if: ${{ ",
            "when a part failed",
        ),
        (
            "            sides=\"base head\"\n",
            "            sides=\"head\"\n",
            "does not check every part",
        ),
        (
            "for k in $(seq 1 \"$OF\"); do\n              gh run download",
            "for k in 1; do\n              gh run download",
            "does not check every part",
        ),
        (
            "name: count-head-${{ matrix.target }}-${{ matrix.part }}",
            "name: count-head-${{ matrix.target }}",
            "does not upload its counts",
        ),
        (
            "        if: matrix.trip\n",
            "        if: matrix.part == 1\n",
            "the part that measures sign",
        ),
    ] {
        let planted = bench.replacen(from, to, 1);
        assert_ne!(planted, bench, "the planted edit {from:?} applies");
        let found = bench_part_problems(&parse(&planted));
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}

/// Each file of `ember-cleared-pairs` and its SHA-256 in the corpus register, in hexadecimal.
fn ember_register() -> BTreeMap<String, String> {
    let manifest = geom_test::corpus::Manifest::embedded().unwrap();
    let corpus = manifest.get(geom_test::ember::CORPUS).unwrap();
    let Some(geom_test::corpus::Fetch::Files(files)) = &corpus.fetch else {
        panic!("{} is not fetched file by file", geom_test::ember::CORPUS)
    };
    files
        .iter()
        .map(|f| {
            let hex = f.sha256.0.iter().fold(String::new(), |mut hex, b| {
                let _ = write!(hex, "{b:02x}");
                hex
            });
            (f.name.clone(), hex)
        })
        .collect()
}

/// What is wrong with the instruction gate's ember part (the module documentation): the scope
/// makes the last part of each leg the ember part; in each part job a step of that part alone
/// fetches the pair's two models by the register's URLs, before the benchmarks, and checks each
/// against the register's SHA-256; the measuring steps list the ember target with the catalogue
/// and measure it in the ember part; the judge reads both targets' summaries.
fn ember_problems(w: &Yaml, register: &BTreeMap<String, String>) -> Vec<String> {
    let mut found = Vec::new();
    let jobs = &w["jobs"];
    let scope = runs(w, "scope");
    if !scope.contains("of=$((planned + 1))") || !scope.contains("ember: (. == $of)}") {
        found.push("the scope makes no ember part, the last of each leg".to_owned());
    }
    // Nothing else fetches: no other step downloads a file or names the corpus.
    for job in keys(jobs) {
        for s in jobs[job.as_str()]["steps"].as_vec().into_iter().flatten() {
            let run = text(&s["run"]);
            let fetches = run.contains("curl ") || run.contains("ember-cleared-pairs");
            if fetches && !text(&s["if"]).starts_with("matrix.ember") {
                found.push(format!(
                    "{job}'s step {:?} fetches outside the ember part",
                    text(&s["name"])
                ));
            }
        }
    }
    let models = geom_test::ember::MODELS;
    for job in ["instructions-base", "instructions-parts"] {
        if text(&jobs[job]["env"]["EMBER"]) != "${{ matrix.ember }}" {
            found.push(format!("{job} does not know its ember part"));
        }
        let steps: Vec<&Yaml> = jobs[job]["steps"].as_vec().into_iter().flatten().collect();
        let fetch = steps.iter().position(|s| {
            text(&s["if"]).starts_with("matrix.ember")
                && text(&s["run"]).contains("sha256sum --check --strict")
        });
        // The measuring step: `base` in the merge base's parts, `gate` in the change's.
        let measure = steps
            .iter()
            .position(|s| matches!(s["id"].as_str(), Some("base" | "gate")));
        let (Some(fetch), Some(measure)) = (fetch, measure) else {
            found.push(format!(
                "{job}'s ember part fetches no models, or measures nothing"
            ));
            continue;
        };
        if fetch > measure {
            found.push(format!("{job} fetches the models after it measures"));
        }
        let run = text(&steps[fetch]["run"]);
        if !run.contains(&format!("for model in {}; do", models.join(" ")))
            || !run.contains("select(.id == \"ember-cleared-pairs\")")
        {
            found.push(format!(
                "{job} does not fetch exactly {models:?} by the register's URLs"
            ));
        }
        // Every SHA-256 the step checks, by the model it names.
        let mut checked = BTreeMap::new();
        for line in run.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            for pair in words.windows(2) {
                let hex = pair[0].len() == 64 && pair[0].chars().all(|c| c.is_ascii_hexdigit());
                let model = Path::new(pair[1]).extension().is_some_and(|e| e == "stl");
                if hex && model {
                    checked.insert(pair[1].to_owned(), pair[0].to_owned());
                }
            }
        }
        let wanted: BTreeMap<String, String> = models
            .iter()
            .map(|m| {
                (
                    (*m).to_owned(),
                    register.get(*m).cloned().unwrap_or_default(),
                )
            })
            .collect();
        if checked != wanted {
            found.push(format!(
                "{job} checks the models against digests that are not the register's: {checked:?}"
            ));
        }
        let measuring = text(&steps[measure]["run"]);
        for needle in [
            "names ember > \"$RUNNER_TEMP/ember.names\"",
            "sort -u \"$RUNNER_TEMP/instructions.names\" \"$RUNNER_TEMP/ember.names\"",
            "if [ \"$EMBER\" = true ]; then\n  bench=ember\n  export SPAREKIT_EMBER=$MODELS",
        ] {
            if !measuring.contains(needle) {
                found.push(format!(
                    "{job} does not list or measure the ember benchmark: {needle}"
                ));
            }
        }
    }
    if !runs(w, "instructions")
        .contains("\"$GUNGRAUN_HOME/geom-bench\" \"$RUNNER_TEMP/judged.names\"")
    {
        found.push("the judge does not read the ember benchmark's summary".to_owned());
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_ember_part_fetches_its_pair_alone_and_the_judge_counts_it() {
    let bench = read(&format!("{WORKFLOWS}/kernel-bench.yml"));
    let register = ember_register();
    let found = ember_problems(&parse(&bench), &register);
    assert!(found.is_empty(), "{found:#?}");
    let first = register[geom_test::ember::MODELS[0]].as_str();
    let wrong = format!(
        "{}{}",
        if first.starts_with('0') { "1" } else { "0" },
        &first[1..]
    );
    for (from, to, said) in [
        // The part dropped.
        (
            "ember: (. == $of)}",
            "ember: false}",
            "the scope makes no ember part",
        ),
        (
            "        if: matrix.ember && steps.kept.outputs.kept == 'false'\n",
            "        if: false\n",
            "instructions-base's ember part fetches no models",
        ),
        // The models fetched in every part.
        (
            "        if: matrix.ember\n        run: |\n          set -euo pipefail\n          register=",
            "        run: |\n          set -euo pipefail\n          register=",
            "fetches outside the ember part",
        ),
        // A digest wrong.
        (first, wrong.as_str(), "not the register's"),
        // The benchmark missing from the judge: not listed, or its summary not read.
        (
            "  names ember > \"$RUNNER_TEMP/ember.names\"\n",
            "  : > \"$RUNNER_TEMP/ember.names\"\n",
            "does not list or measure the ember benchmark",
        ),
        (
            "\"$GUNGRAUN_HOME/geom-bench\" \"$RUNNER_TEMP/judged.names\"",
            "\"$GUNGRAUN_HOME/geom-bench/instructions\" \"$RUNNER_TEMP/judged.names\"",
            "the judge does not read the ember benchmark's summary",
        ),
    ] {
        let planted = bench.replacen(from, to, 1);
        assert_ne!(planted, bench, "the planted edit {from:?} applies");
        let found = ember_problems(&parse(&planted), &register);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_count_gates_measure_one_kind_a_push_and_the_base_once() {
    let bench = read(&format!("{WORKFLOWS}/kernel-bench.yml"));
    let found = bench_problems(&bench);
    assert!(found.is_empty(), "{found:#?}");
    for (from, to, said) in [
        ("  merge_group:\n", "", "does not run in the merge queue"),
        (
            "pull_request) legs=\"[$x86]\" ;;",
            "pull_request) legs=\"[$x86,$arm]\" ;;",
            "x86-64 alone",
        ),
        (
            "merge_group) legs=\"[$arm]\" ;;",
            "merge_group) legs=\"[$x86]\" ;;",
            "measures aarch64",
        ),
        (
            "include: ${{ fromJSON(needs.scope.outputs.legs) }}",
            "include: ${{ fromJSON('[]') }}",
            "not the scope's list",
        ),
        (
            "          kept=false\n",
            "          kept=true\n",
            "taken as kept",
        ),
        (
            " && steps.kept.outputs.kept != 'true'",
            " && steps.kept.outputs.kept == 'true'",
            "a missing base count is not measured",
        ),
        (
            "if [ \"$branch\" = main ]; then",
            "if true; then",
            "trusted wrongly",
        ),
        (
            "matrix.target == 'aarch64'\n        with:\n          name: bench-aarch64-tested",
            "matrix.target == 'x86-64'\n        with:\n          name: bench-aarch64-tested",
            "does not mark",
        ),
    ] {
        let planted = bench.replacen(from, to, 1);
        assert_ne!(planted, bench, "the planted edit {from:?} applies");
        let found = bench_problems(&planted);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
    // Planted: the change's own checks moved into the merge base's parts.
    let check = "      - name: Check the benchmarks' inputs, results and wiring\n        working-directory: head\n        run: cargo \"+$HEAD_TOOLCHAIN\" test --locked --release -p geom-bench\n\n";
    let planted = bench.replacen(check, "", 1).replacen(
        "      - name: Install the Gungraun runner the merge base needs",
        &format!("{check}      - name: Install the Gungraun runner the merge base needs"),
        1,
    );
    assert_ne!(planted, bench);
    let found = bench_problems(&planted);
    assert!(
        found
            .iter()
            .any(|p| p.contains("before the merge base is kept")),
        "{found:#?}"
    );
}
