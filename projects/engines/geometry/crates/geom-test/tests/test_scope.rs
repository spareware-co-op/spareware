//! The selection of crates whose tests a pull request runs (`geom_test::test_scope`; chapter
//! 28.17), held to the crate graph, read here independently, and to the workflows that apply it.
//! Each check is a function of what it reads, and each is shown to fail on planted mistakes:
//!
//! - `a_change_runs_every_crate_whose_tests_build_it`: for every workspace crate X, a change to its
//!   code runs exactly the tests of every crate whose test binaries link X (itself, its normal and
//!   build dependencies' closure, its development dependencies' closure) and of every crate that
//!   reads files outside its folder; a change to X's tests runs X's and the readers'.
//! - `a_file_outside_every_crate_runs_everything`: a workflow, `Cargo.lock`, `.cargo/`, the toolchain
//!   file and an architecture chapter each run every crate's tests, and so does no change list.
//! - `a_file_no_crate_builds_runs_only_the_readers`: a file of geom-solidcore, the one folder the
//!   root Cargo.toml excludes, runs only the crates that read outside their folder, and reaches
//!   nothing `geom-bench` builds. Planted: no folder set apart (every crate runs), and an excluded
//!   folder that holds a workspace crate (it is not set apart).
//! - `a_crate_that_reads_outside_its_folder_is_found`: the words that mark a read outside a
//!   crate's folder are found in planted sources, and the repository's readers include
//!   `geom-test`.
//! - `the_suites_run_the_selection`: `kernel.yml`'s job `scope` runs `kernel-quality test-scope`
//!   on a pull request's changed files, and its suites test only the crates it names, keeping
//!   their job names, with nextest told that a part with no test passes (`NEXTEST_NO_TESTS`); `kernel-bench.yml`'s count
//!   gates measure only when the change reaches what `geom-bench` builds.
//! - `a_changed_map_row_or_ledger_entry_runs_the_crates_it_names`: a change to one crate's row of
//!   the repository's `MAP.json`, or to one survivor of its quality ledger, read against the
//!   unchanged file, runs exactly that crate's tests, the readers' and the tests of the crates that
//!   read the file whole; a change that cannot be placed runs everything; the count gates measure
//!   only when `geom-bench` is named or reads the file; and the Kani proofs run only where their
//!   code reads the file. Planted: the changed entries dropped, so the named crate does not run.
//! - `the_crates_that_read_a_shared_file_whole_are_found`: every crate whose Rust sources hold a
//!   string naming `MAP.json` or `quality-ledger.json`, found here by plain text, is among the
//!   readers `geom_test::shared` finds, and `geom-test` reads both.
//! - `the_release_suite_shares_run_every_crate_once`: the release suite's shares of every crate
//!   (`Selection::parts`, by `suite-times.tsv`) hold each crate but those set apart in exactly one
//!   share, for any number of shares, and the step output `parts` is those shares numbered from 1;
//!   `suite-times.tsv` names only workspace crates. Planted: a crate dropped, a crate in two shares,
//!   the crate set apart left in.
//! - `the_selections_read_the_shared_files_against_the_base`: `kernel.yml`'s scope, the Kani
//!   selection and the count gates' reach each pass the change's base (`--base`). Planted: each
//!   `--base` taken out.
//! - `the_wasm_dev_parts_run_every_unit_once`: `kernel-dev.yml`'s `wasm` job's matrix is exactly
//!   what `wasm_dev_matrix` writes for the development crates' test units (from `cargo metadata`)
//!   by `wasm-dev-times.tsv`, every unit is in exactly one part, the table lists exactly those
//!   units, and the job's step runs each part's arguments under wasmtime. Planted: a unit dropped
//!   from a part, a unit in two parts, a flag of no unit's form.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "counts; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use geom_test::coverage::parse_times;
use geom_test::coverage::plan_problems;
use geom_test::deps::cargo_metadata;
use geom_test::shared::{self, Changes, Reach};
use geom_test::test_scope::{
    SUITE_TIMES, Scope, Selection, WASM_DEV_CRATES, WASM_DEV_TIMES, apart, text_reads_outside,
    wasm_dev_matrix, wasm_dev_parts, wasm_dev_units,
};
use serde_json::Value;
use yaml_rust2::{Yaml, YamlLoader};

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

fn metadata() -> Value {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    cargo_metadata(&cargo, &root().join("Cargo.toml")).unwrap()
}

/// The workspace's members, by name, with their folder from the root (ending in `/`).
fn members(metadata: &Value) -> BTreeMap<String, String> {
    let root = format!(
        "{}/",
        metadata["workspace_root"]
            .as_str()
            .unwrap()
            .replace('\\', "/")
            .trim_end_matches('/')
    );
    let ids: BTreeSet<&str> = metadata["workspace_members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| ids.contains(p["id"].as_str().unwrap()))
        .map(|p| {
            let manifest = p["manifest_path"].as_str().unwrap().replace('\\', "/");
            let folder = manifest
                .strip_prefix(&root)
                .unwrap()
                .strip_suffix("Cargo.toml")
                .unwrap()
                .to_owned();
            (p["name"].as_str().unwrap().to_owned(), folder)
        })
        .collect()
}

/// Each member's dependencies among the members, by kind: normal and build, and development.
fn edges(
    metadata: &Value,
) -> (
    BTreeMap<String, BTreeSet<String>>,
    BTreeMap<String, BTreeSet<String>>,
) {
    let names: BTreeMap<&str, &str> = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p["name"].as_str().unwrap()))
        .collect();
    let ids: BTreeSet<&str> = metadata["workspace_members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let mut built: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut dev: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for node in metadata["resolve"]["nodes"].as_array().unwrap() {
        let id = node["id"].as_str().unwrap();
        if !ids.contains(id) {
            continue;
        }
        for d in node["deps"].as_array().unwrap() {
            let pkg = d["pkg"].as_str().unwrap();
            if !ids.contains(pkg) {
                continue;
            }
            for k in d["dep_kinds"].as_array().unwrap() {
                let to = if k["kind"].as_str() == Some("dev") {
                    &mut dev
                } else {
                    &mut built
                };
                to.entry(names[id].to_owned())
                    .or_default()
                    .insert(names[pkg].to_owned());
            }
        }
    }
    (built, dev)
}

/// What `start`'s test binaries link: `start`, and the normal and build closure of `start` and of
/// each of its development dependencies.
fn links(
    built: &BTreeMap<String, BTreeSet<String>>,
    dev: &BTreeMap<String, BTreeSet<String>>,
    start: &str,
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack: Vec<String> = vec![start.to_owned()];
    stack.extend(dev.get(start).into_iter().flatten().cloned());
    while let Some(c) = stack.pop() {
        if out.insert(c.clone()) {
            stack.extend(built.get(&c).into_iter().flatten().cloned());
        }
    }
    out
}

/// The crates a selection runs.
fn running(selection: &Selection) -> BTreeSet<String> {
    selection.running().into_iter().map(str::to_owned).collect()
}

/// What a selection gets wrong against the graph: for each crate X, a change to its code that
/// does not run (or runs beyond) the tests that link X and the readers', and a change to its tests
/// that runs any but X's and the readers'.
fn graph_problems(
    select: &dyn Fn(&[String]) -> Selection,
    metadata: &Value,
    readers: &BTreeSet<String>,
) -> Vec<String> {
    let members = members(metadata);
    let (built, dev) = edges(metadata);
    let mut found = Vec::new();
    for (x, folder) in &members {
        let mut want: BTreeSet<String> = members
            .keys()
            .filter(|y| links(&built, &dev, y).contains(x))
            .cloned()
            .collect();
        want.extend(readers.iter().cloned());
        let got = running(&select(&[format!("{folder}src/lib.rs")]));
        for y in want.difference(&got) {
            found.push(format!(
                "a change to {x} does not run {y}'s tests, though they build or read it"
            ));
        }
        for y in got.difference(&want) {
            found.push(format!(
                "a change to {x} runs {y}'s tests, which do not build it"
            ));
        }
        let mut own: BTreeSet<String> = readers.clone();
        own.insert(x.clone());
        let got = running(&select(&[format!("{folder}tests/a_test.rs")]));
        if got != own {
            found.push(format!(
                "a change to {x}'s tests runs {got:?}, not {x}'s and the readers'"
            ));
        }
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository and runs cargo metadata, which a WebAssembly test cannot"
)]
#[test]
fn a_change_runs_every_crate_whose_tests_build_it() {
    let metadata = metadata();
    let scope = Scope::read(&metadata).unwrap();
    let readers = scope.readers().clone();
    let select = |files: &[String]| scope.select(Some(files));
    let found = graph_problems(&select, &metadata, &readers);
    assert!(found.is_empty(), "{found:#?}");
    // The check reaches something: some crate's tests are skipped on a change to geom-num's tests.
    let num = members(&metadata)["geom-num"].clone();
    let got = running(&select(&[format!("{num}tests/x.rs")]));
    assert!(got.len() < members(&metadata).len(), "{got:?}");

    // Planted: development dependencies not followed.
    let (_, dev) = edges(&metadata);
    let mut no_dev = metadata.clone();
    for node in no_dev["resolve"]["nodes"].as_array_mut().unwrap() {
        for d in node["deps"].as_array_mut().unwrap() {
            let kinds = d["dep_kinds"].as_array_mut().unwrap();
            kinds.retain(|k| k["kind"].as_str() != Some("dev"));
        }
    }
    assert!(dev.values().any(|d| !d.is_empty()));
    let blind = Scope::new(&no_dev, readers.clone()).unwrap();
    let found = graph_problems(&|f| blind.select(Some(f)), &metadata, &readers);
    assert!(
        found.iter().any(|p| p.contains("does not run")),
        "{found:#?}"
    );
    // Planted: the readers forgotten.
    let unread = Scope::new(&metadata, BTreeSet::new()).unwrap();
    let found = graph_problems(&|f| unread.select(Some(f)), &metadata, &readers);
    assert!(
        found
            .iter()
            .any(|p| p.contains("does not run geom-test's tests")),
        "{found:#?}"
    );
    // Planted: a selection that runs only the changed crate.
    let found = graph_problems(
        &|f| {
            let all = scope.select(Some(f));
            let only: Vec<String> = f.to_vec();
            let mut s = all;
            for e in &mut s.crates {
                let folder = &members(&metadata)[&e.name];
                if !only.iter().any(|o| o.starts_with(folder.as_str())) {
                    e.why = None;
                }
            }
            s
        },
        &metadata,
        &readers,
    );
    assert!(
        found.iter().any(|p| p.contains("does not run")),
        "{found:#?}"
    );
}

/// Files in no crate's folder, of each kind the rule names.
const OUTSIDE: [&str; 6] = [
    ".github/workflows/kernel.yml",
    "Cargo.lock",
    ".cargo/config.toml",
    "rust-toolchain.toml",
    "projects/engines/geometry/architecture/28-testing.md",
    "projects/engines/geometry/MAP.json",
];

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository and runs cargo metadata, which a WebAssembly test cannot"
)]
#[test]
fn a_file_outside_every_crate_runs_everything() {
    let metadata = metadata();
    let scope = Scope::read(&metadata).unwrap();
    let all: BTreeSet<String> = members(&metadata).into_keys().collect();
    let outside = |select: &dyn Fn(&[String]) -> Selection| -> Vec<String> {
        OUTSIDE
            .iter()
            .filter(|f| running(&select(&[(**f).to_owned()])) != all)
            .map(|f| (*f).to_owned())
            .collect()
    };
    assert_eq!(outside(&|f| scope.select(Some(f))), Vec::<String>::new());
    assert_eq!(running(&scope.select(None)), all);
    let outputs: BTreeMap<String, String> = scope.select(None).outputs().into_iter().collect();
    assert_eq!(outputs["all"], "true");
    assert_eq!(
        outputs["crates"]
            .split(' ')
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        all
    );
    assert!(
        scope
            .build_reached(Some(&["Cargo.lock".to_owned()]), "geom-bench")
            .is_some()
    );
    // Planted: a selection that reads only the files under projects/.
    let found = outside(&|f| {
        let kept: Vec<String> = f
            .iter()
            .filter(|p| p.starts_with("projects/"))
            .cloned()
            .collect();
        scope.select(Some(&kept))
    });
    assert!(found.contains(&"Cargo.lock".to_owned()), "{found:?}");
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository and runs cargo metadata, which a WebAssembly test cannot"
)]
#[test]
fn a_file_no_crate_builds_runs_only_the_readers() {
    let metadata = metadata();
    let scope = Scope::read(&metadata).unwrap();
    let all: BTreeSet<String> = members(&metadata).into_keys().collect();
    let folder = "projects/engines/geometry/geom-solidcore/";
    assert_eq!(scope.apart(), [folder.to_owned()]);
    let file = [format!("{folder}tests/cases.rs")];
    let readers: BTreeSet<String> = scope.readers().iter().cloned().collect();
    assert!(
        readers.contains("geom-test") && readers != all,
        "{readers:?}"
    );
    assert_eq!(running(&scope.select(Some(&file))), readers);
    assert!(scope.build_reached(Some(&file), "geom-bench").is_none());
    // Planted: the same scope with no folder set apart runs every crate.
    let unset = scope.clone().with_apart(Vec::new());
    assert_eq!(running(&unset.select(Some(&file))), all);
    // Planted: an excluded folder that holds a workspace crate is not apart.
    let manifest = read("Cargo.toml");
    let planted = manifest.replacen(
        "exclude = [\"projects/engines/geometry/geom-solidcore\"]",
        "exclude = [\"projects/engines/geometry/geom-solidcore\", \"projects/engines/geometry/crates/geom-num\"]",
        1,
    );
    assert_ne!(planted, manifest, "the planted edit applies");
    assert_eq!(apart(&planted, &metadata).unwrap(), [folder.to_owned()]);
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository and runs cargo metadata, which a WebAssembly test cannot"
)]
#[test]
fn a_crate_that_reads_outside_its_folder_is_found() {
    for text in [
        "let p = Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"../geom-num/src\");",
        "let root = here.ancestors().nth(4);",
        "let up = dir.parent().unwrap();",
        "let m = cargo_metadata(&cargo, &manifest);",
        "const X: &str = include_str!(\"../../MAP.json\");",
    ] {
        assert!(text_reads_outside(text), "{text}");
    }
    assert!(!text_reads_outside(
        "let p = Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"tests/data\");"
    ));
    let scope = Scope::read(&metadata()).unwrap();
    assert!(
        scope.readers().contains("geom-test"),
        "{:?}",
        scope.readers()
    );
    let missing: Vec<&String> = scope
        .folders()
        .keys()
        .filter(|c| !scope.readers().contains(*c))
        .collect();
    assert!(!missing.is_empty(), "every crate reads outside its folder");
}

/// What is wrong with the workflows against the selection.
fn workflow_problems(kernel: &str, bench: &str) -> Vec<String> {
    let mut found = Vec::new();
    let k = parse(kernel);
    let text = |y: &Yaml| y.as_str().unwrap_or_default().replace('\n', " ");
    let scope = &k["jobs"]["scope"];
    let runs: Vec<String> = scope["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .map(|s| text(&s["run"]))
        .collect();
    if !runs.iter().any(|r| {
        r.contains("kernel-quality -- test-scope $SCOPE")
            && r.contains("--github-output \"$GITHUB_OUTPUT\"")
    }) {
        found.push("kernel.yml's job scope does not run `kernel-quality test-scope`".to_owned());
    }
    if !runs
        .iter()
        .any(|r| r.contains("git diff --no-ext-diff --no-renames --name-only \"$BASE\" HEAD"))
        || text(&scope["steps"][3]["env"]["BASE"]) != "${{ needs.changes.outputs.base }}"
    {
        found.push("kernel.yml's job scope does not read the change's files".to_owned());
    }
    if text(&scope["outputs"]["crates"]) != "${{ steps.scope.outputs.crates }}" {
        found.push("kernel.yml's job scope does not hand on its crates".to_owned());
    }
    for job in ["linux", "suite", "wasm-build", "wasm-suite"] {
        let y = &k["jobs"][job];
        let needs: Vec<String> = y["needs"]
            .as_vec()
            .into_iter()
            .flatten()
            .filter_map(|n| n.as_str().map(str::to_owned))
            .collect();
        if !needs.iter().any(|n| n == "scope") {
            found.push(format!("{job} does not need the job scope"));
        }
        // A job is skipped by its event, never by the selection as a whole, so each part reports.
        if text(&y["if"]).contains("scope") {
            found.push(format!(
                "{job} is skipped on the selection as a whole, so its parts would not report"
            ));
        }
        let steps: Vec<&Yaml> = y["steps"].as_vec().into_iter().flatten().collect();
        let lists = steps.iter().any(|s| {
            text(&s["env"]["SELECTED"]) == "${{ needs.scope.outputs.crates }}"
                && text(&s["run"]).contains("case \" $SELECTED \" in")
        });
        if !lists {
            found.push(format!("{job} does not narrow its crates to the selection"));
        }
        for s in steps.iter().filter(|s| {
            let r = text(&s["run"]);
            r.contains("$KERNEL_CRATES") && !r.contains("KERNEL_CRATES=")
        }) {
            if !text(&s["if"]).contains("env.KERNEL_CRATES != ''") {
                found.push(format!(
                    "{job}'s step {:?} runs with no crate selected",
                    s["name"].as_str().unwrap_or_default()
                ));
            }
        }
        let nextest = steps
            .iter()
            .any(|s| text(&s["run"]).contains("cargo nextest run"));
        if nextest && text(&y["env"]["NEXTEST_NO_TESTS"]) != "pass" {
            found.push(format!(
                "{job} fails a part the selection left without a test"
            ));
        }
    }
    let b = parse(bench);
    let decide = b["jobs"]["scope"]["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .map(|s| text(&s["run"]))
        .collect::<Vec<_>>()
        .join("\n");
    if !decide.contains("kernel-quality -- test-scope --changed")
        || !decide.contains("--build-of geom-bench")
    {
        found.push(
            "kernel-bench.yml's scope does not ask whether the change reaches geom-bench's build"
                .to_owned(),
        );
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_suites_run_the_selection() {
    let kernel = read(".github/workflows/kernel.yml");
    let bench = read(".github/workflows/kernel-bench.yml");
    let found = workflow_problems(&kernel, &bench);
    assert!(found.is_empty(), "{found:#?}");
    for (from, to, said) in [
        (
            "kernel-quality -- test-scope $SCOPE",
            "kernel-quality -- fuzz-scope $SCOPE",
            "does not run `kernel-quality test-scope`",
        ),
        (
            "NEXTEST_NO_TESTS: pass",
            "NEXTEST_NO_TESTS: fail",
            "fails a part the selection left",
        ),
        (
            "case \" $SELECTED \" in",
            "case \" $OTHER \" in",
            "does not narrow its crates",
        ),
        (
            "&& env.KERNEL_CRATES != ''",
            "",
            "runs with no crate selected",
        ),
        (
            "--name-only \"$BASE\" HEAD > \"$RUNNER_TEMP/changed.txt\"\n            echo \"SCOPE=--changed",
            "--name-only HEAD^1 HEAD > \"$RUNNER_TEMP/changed.txt\"\n            echo \"SCOPE=--changed",
            "does not read the change's files",
        ),
        (
            "debug run on Linux x86-64.\n    if: github.event_name",
            "debug run on Linux x86-64.\n    if: needs.scope.outputs.crates != '' && github.event_name",
            "skipped on the selection as a whole",
        ),
        (
            "--base \"$merge_base\" --build-of geom-bench",
            "--base \"$merge_base\" --build-of geom-num",
            "reaches geom-bench's build",
        ),
    ] {
        let (k, b) = (kernel.replacen(from, to, 1), bench.replacen(from, to, 1));
        assert!(
            k != kernel || b != bench,
            "the planted edit {from:?} applies"
        );
        let found = workflow_problems(&k, &b);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}

const MAP: &str = "projects/engines/geometry/MAP.json";
const LEDGER: &str = "projects/engines/geometry/quality-ledger.json";

/// The repository's text of a shared file, and the same file with `edit` applied to its JSON.
fn edited(file: &str, edit: &dyn Fn(&mut Value)) -> (String, String) {
    let base = read(file);
    let mut v: Value = serde_json::from_str(&base).unwrap();
    edit(&mut v);
    let head = serde_json::to_string_pretty(&v).unwrap();
    assert_ne!(base, head, "the planted edit of {file} changes it");
    (base, head)
}

/// What a change to `file` from `base` to `head` reaches, as the selections take it.
fn reach_of(scope: &Scope, file: &str, base: &str, head: &str) -> Changes {
    Changes::from([(
        file.to_owned(),
        shared::compare(Some(base), Some(head), scope.folders()),
    )])
}

/// What a selection from a change to `file` alone runs that it should not, or does not run that
/// it should: `named`'s tests, every reader's and every crate that reads the file whole, nothing
/// else.
fn shared_problems(scope: &Scope, file: &str, named: &str, selection: &Selection) -> Vec<String> {
    let mut want: BTreeSet<String> = scope.readers().clone();
    want.extend(scope.whole_readers().crates(file));
    want.insert(named.to_owned());
    let got = running(selection);
    let mut found = Vec::new();
    for c in want.difference(&got) {
        found.push(format!(
            "a change to {named}'s entry of {file} does not run {c}'s tests"
        ));
    }
    for c in got.difference(&want) {
        found.push(format!(
            "a change to {named}'s entry of {file} runs {c}'s tests"
        ));
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository and runs cargo metadata, which a WebAssembly test cannot"
)]
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "each planted change beside what it must run reads best in one place"
)]
fn a_changed_map_row_or_ledger_entry_runs_the_crates_it_names() {
    let metadata = metadata();
    let scope = Scope::read(&metadata).unwrap();
    let all: BTreeSet<String> = members(&metadata).into_keys().collect();
    let one = |file: &str| vec![file.to_owned()];
    // A crate with a row in the map whose tests run only when something names it: no reader,
    // and no reader of either shared file.
    let map: Value = serde_json::from_str(&read(MAP)).unwrap();
    let (named, named_folder) = members(&metadata)
        .into_iter()
        .find(|(c, folder)| {
            !scope.readers().contains(c)
                && !scope.whole_readers().crates(MAP).contains(c)
                && !scope.whole_readers().crates(LEDGER).contains(c)
                && map["crates"].as_array().unwrap().iter().any(|r| {
                    r["folder"]
                        .as_str()
                        .is_some_and(|f| format!("{f}/") == *folder)
                })
        })
        .expect("a crate that reads nothing outside its folder");
    let named = named.as_str();

    // A crate's row of the map: its job reworded.
    let (base, head) = edited(MAP, &|v| {
        let row = v["crates"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| {
                r["folder"]
                    .as_str()
                    .unwrap()
                    .ends_with(&format!("/{named}"))
            })
            .unwrap();
        row["job"] = Value::String(format!("{} (planted)", row["job"].as_str().unwrap()));
    });
    let changes = reach_of(&scope, MAP, &base, &head);
    let selection = scope.select_with(Some(&one(MAP)), &changes);
    let found = shared_problems(&scope, MAP, named, &selection);
    assert!(found.is_empty(), "{found:#?}");
    assert!(
        running(&selection).len() < all.len(),
        "a row change runs everything"
    );
    // Planted: the changed entries dropped.
    let dropped = Changes::from([(MAP.to_owned(), Reach::Crates(BTreeMap::new()))]);
    let found = shared_problems(
        &scope,
        MAP,
        named,
        &scope.select_with(Some(&one(MAP)), &dropped),
    );
    assert!(
        found
            .iter()
            .any(|p| p.contains(&format!("does not run {named}'s tests"))),
        "{found:#?}"
    );

    // The ledger, with a planted survivor of that crate at its end: the survivor's reason
    // reworded, the survivor removed, another added; each names its crate.
    let planted = serde_json::json!({
        "file": format!("{named_folder}src/planted.rs"),
        "mutant": "replace + with - in planted",
        "line_text": "a + b",
        "occurrences": 1,
        "kind": "harmless",
        "reason": "planted by the test of the selection",
    });
    let with_planted = {
        let mut v: Value = serde_json::from_str(&read(LEDGER)).unwrap();
        v["survivors"].as_array_mut().unwrap().push(planted.clone());
        v
    };
    let at = with_planted["survivors"].as_array().unwrap().len() - 1;
    for (how, edit) in [
        (
            "reworded",
            Box::new(move |v: &mut Value| {
                let s = &mut v["survivors"][at];
                s["reason"] = Value::String(format!("{} (planted)", s["reason"].as_str().unwrap()));
            }) as Box<dyn Fn(&mut Value)>,
        ),
        (
            "removed",
            Box::new(move |v: &mut Value| {
                v["survivors"].as_array_mut().unwrap().remove(at);
            }),
        ),
        (
            "added",
            Box::new(move |v: &mut Value| {
                let mut s = v["survivors"][at].clone();
                s["mutant"] = Value::String("replace + with - in planted".to_owned());
                v["survivors"].as_array_mut().unwrap().push(s);
            }),
        ),
    ] {
        let base = serde_json::to_string_pretty(&with_planted).unwrap();
        let mut v = with_planted.clone();
        edit(&mut v);
        let head = serde_json::to_string_pretty(&v).unwrap();
        assert_ne!(base, head, "{how}: the planted edit changes the ledger");
        let changes = reach_of(&scope, LEDGER, &base, &head);
        let selection = scope.select_with(Some(&one(LEDGER)), &changes);
        let found = shared_problems(&scope, LEDGER, named, &selection);
        assert!(found.is_empty(), "{how}: {found:#?}");
        // The count gates measure only when geom-bench is named or reads the ledger whole.
        let bench = scope
            .build_reached_with(Some(&one(LEDGER)), &changes, "geom-bench")
            .is_some();
        assert_eq!(
            bench,
            scope.whole_readers().crates(LEDGER).contains("geom-bench"),
            "{how}"
        );
    }

    // What cannot be placed runs everything: a change to the ledger's own description, a file
    // that is not JSON, and a shared file given without its base.
    let (base, head) = edited(LEDGER, &|v| {
        v["about"] = Value::String("planted".to_owned());
    });
    for changes in [
        reach_of(&scope, LEDGER, &base, &head),
        reach_of(&scope, LEDGER, &base, "not json"),
        Changes::new(),
    ] {
        assert_eq!(
            running(&scope.select_with(Some(&one(LEDGER)), &changes)),
            all
        );
    }
    // Another file outside every crate beside it still runs everything.
    let changes = reach_of(&scope, MAP, &read(MAP), &read(MAP));
    let files = vec![MAP.to_owned(), "Cargo.lock".to_owned()];
    assert_eq!(running(&scope.select_with(Some(&files), &changes)), all);

    // The Kani proofs: a ledger entry reaches only the proofs whose code reads the ledger whole.
    let fuzz = geom_test::fuzz_scope::Scope::read(&metadata).unwrap();
    let at = read(LEDGER).parse::<Value>().unwrap()["survivors"]
        .as_array()
        .unwrap()
        .iter()
        .position(|s| s["file"].as_str().unwrap().contains("/geom-num/"))
        .unwrap();
    let (base, head) = edited(LEDGER, &move |v| {
        let s = &mut v["survivors"][at];
        s["reason"] = Value::String(format!("{} (planted)", s["reason"].as_str().unwrap()));
    });
    let changes = Changes::from([(
        LEDGER.to_owned(),
        shared::compare(Some(&base), Some(&head), fuzz.folders()),
    )]);
    let selection = fuzz.select_with(Some(&one(LEDGER)), &changes);
    let running_jobs: Vec<&str> = selection
        .jobs
        .iter()
        .filter(|j| j.why.is_some())
        .map(|j| j.job.name)
        .collect();
    assert!(running_jobs.is_empty(), "{running_jobs:?}");
    // Planted: the ledger placed in no crate, as without its base: every proof runs.
    let every = fuzz.select(Some(&one(LEDGER)));
    assert!(every.jobs.iter().all(|j| j.why.is_some()));
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository and runs cargo metadata, which a WebAssembly test cannot"
)]
#[test]
fn the_crates_that_read_a_shared_file_whole_are_found() {
    let metadata = metadata();
    let scope = Scope::read(&metadata).unwrap();
    let whole = scope.whole_readers();
    for (file, name) in [(MAP, "MAP.json\""), (LEDGER, "quality-ledger.json\"")] {
        let readers = whole.crates(file);
        assert!(readers.contains("geom-test"), "{file}: {readers:?}");
        for (krate, folder) in members(&metadata) {
            let mut stack: Vec<PathBuf> = ["src", "tests", "benches", "examples", "build.rs"]
                .iter()
                .map(|s| root().join(&folder).join(s))
                .collect();
            let mut names = false;
            while let Some(p) = stack.pop() {
                if p.is_dir() {
                    stack.extend(std::fs::read_dir(&p).unwrap().map(|e| e.unwrap().path()));
                } else if p.is_file() && p.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&p).unwrap();
                    names |= text
                        .lines()
                        .filter(|l| !l.trim_start().starts_with("//"))
                        .any(|l| l.contains(name));
                }
            }
            if names {
                assert!(
                    readers.contains(&krate),
                    "{krate} names {file} in a string, and is not found to read it"
                );
            }
        }
    }
    assert!(
        whole.crates(MAP).contains("geom-op"),
        "geom-op's attack_g82 reads the map"
    );
}

/// What is wrong with the selections' commands: each must pass the change's base.
fn base_problems(kernel: &str, fuzz: &str, bench: &str) -> Vec<String> {
    let mut found = Vec::new();
    if !kernel.contains("echo \"SCOPE=--changed $RUNNER_TEMP/changed.txt --base $BASE\"") {
        found.push("kernel.yml's scope reads the shared files without the base".to_owned());
    }
    if !fuzz.contains(
        "echo \"SCOPE=--changed $RUNNER_TEMP/changed.txt --base $(git rev-parse HEAD^1)\"",
    ) {
        found
            .push("kernel-fuzz.yml's selection reads the shared files without the base".to_owned());
    }
    if !bench.contains(
        "--changed \"$RUNNER_TEMP/changed.txt\" --base \"$merge_base\" --build-of geom-bench",
    ) {
        found.push("kernel-bench.yml's reach reads the shared files without the base".to_owned());
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_selections_read_the_shared_files_against_the_base() {
    let kernel = read(".github/workflows/kernel.yml");
    let fuzz = read(".github/workflows/kernel-fuzz.yml");
    let bench = read(".github/workflows/kernel-bench.yml");
    let found = base_problems(&kernel, &fuzz, &bench);
    assert!(found.is_empty(), "{found:#?}");
    for (which, said) in [
        (0, "kernel.yml"),
        (1, "kernel-fuzz.yml"),
        (2, "kernel-bench.yml"),
    ] {
        let mut texts = [kernel.clone(), fuzz.clone(), bench.clone()];
        let before = texts[which].clone();
        texts[which] = before
            .replacen(" --base $BASE\"", "\"", 1)
            .replacen(" --base $(git rev-parse HEAD^1)\"", "\"", 1)
            .replacen(" --base \"$merge_base\"", "", 1);
        assert_ne!(texts[which], before, "the planted edit applies to {said}");
        let found = base_problems(&texts[0], &texts[1], &texts[2]);
        assert!(found.iter().any(|p| p.contains(said)), "{said}: {found:#?}");
    }
}

/// What is wrong with the release suite's `parts` of `crates`: a crate in no part or in two, or a
/// part with a crate not in the run.
fn share_problems(crates: &[String], parts: &[Vec<String>]) -> Vec<String> {
    let mut found = Vec::new();
    for (k, part) in parts.iter().enumerate() {
        for c in part.iter().filter(|c| !crates.contains(c)) {
            found.push(format!("part {}: {c} is not in the run", k + 1));
        }
    }
    for c in crates {
        let times = parts.iter().flatten().filter(|p| *p == c).count();
        if times != 1 {
            found.push(format!("{c}'s tests run in {times} parts"));
        }
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_release_suite_shares_run_every_crate_once() {
    let scope = Scope::read(&metadata()).unwrap();
    let folders = scope.folders();
    let selection = scope.select(None);
    // The kernel's crates, read here from their folder.
    let all: Vec<String> = selection
        .running()
        .iter()
        .filter(|c| folders[**c].starts_with("projects/engines/geometry/crates/"))
        .map(|c| (*c).to_owned())
        .collect();
    assert!(
        all.len() > 40 && all.len() < selection.running().len(),
        "{all:?}"
    );
    let times = parse_times(SUITE_TIMES).unwrap();
    for krate in times.keys() {
        assert!(
            all.contains(krate),
            "suite-times.tsv names {krate}, no kernel crate"
        );
    }
    let apart = vec!["geom-num".to_owned(), "geom-test".to_owned()];
    let want: Vec<String> = all.iter().filter(|c| !apart.contains(c)).cloned().collect();
    for n in [1, 2, 7, all.len() + 3] {
        let parts = selection.parts(n, &apart, &times, folders);
        assert_eq!(parts.len(), n);
        assert_eq!(share_problems(&want, &parts), Vec::<String>::new(), "{n}");
    }
    // The step output is the same parts, numbered from 1, each its crates separated by spaces.
    let parts = selection.parts(7, &apart, &times, folders);
    let (key, json) = selection.parts_output(7, &apart, &times, folders);
    assert_eq!(key, "parts");
    assert!(!json.contains('\n'), "one line");
    let read: BTreeMap<String, String> = serde_json::from_str(&json).unwrap();
    let wanted: BTreeMap<String, String> = parts
        .iter()
        .enumerate()
        .map(|(k, p)| ((k + 1).to_string(), p.join(" ")))
        .collect();
    assert_eq!(read, wanted);
    // Planted: a crate no part runs, a crate run twice, and the crate set apart in a share.
    let mut dropped = parts.clone();
    let gone = dropped[0].pop().unwrap();
    assert!(
        share_problems(&want, &dropped).contains(&format!("{gone}'s tests run in 0 parts")),
        "dropped"
    );
    let mut twice = parts.clone();
    twice[1].push(parts[0][0].clone());
    assert!(
        share_problems(&want, &twice).contains(&format!("{}'s tests run in 2 parts", parts[0][0])),
        "twice"
    );
    let kept = selection.parts(7, &[], &times, folders);
    assert!(
        share_problems(&want, &kept)
            .iter()
            .any(|p| p.ends_with("geom-test is not in the run")),
        "kept"
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository and runs cargo metadata, which a WebAssembly test cannot"
)]
#[test]
fn the_wasm_dev_parts_run_every_unit_once() {
    let workflow = read(".github/workflows/kernel-dev.yml");
    let units = wasm_dev_units(&metadata(), &WASM_DEV_CRATES);
    assert!(
        units.iter().any(|u| u == "geom-test test laws_surfaces")
            && units.iter().any(|u| u == "geom-fuzz lib")
            && units.iter().any(|u| u == "geom-gen doc"),
        "{units:?}"
    );
    let times = parse_times(WASM_DEV_TIMES).unwrap();
    let listed: BTreeSet<&String> = times.keys().collect();
    let found: BTreeSet<&String> = units.iter().collect();
    assert_eq!(
        listed, found,
        "wasm-dev-times.tsv does not list exactly the development crates' test units"
    );

    // The matrix is the one the tool writes, and every unit is in exactly one part.
    let parts = wasm_dev_parts(&workflow).unwrap();
    assert!(parts.len() > 1, "the wasm job has no parts");
    let written = wasm_dev_matrix(&units, &times, parts.len());
    assert!(
        workflow.contains(&written),
        "kernel-dev.yml's wasm matrix is not what `kernel-quality wasm-dev-parts --parts {}`          writes:
{written}",
        parts.len()
    );
    let problems = plan_problems(&units, &parts);
    assert!(problems.is_empty(), "{problems:#?}");

    // The job runs each part's arguments under wasmtime, and nothing else of them.
    let yaml = parse(&workflow);
    let job = &yaml["jobs"]["wasm"];
    assert_eq!(
        job["strategy"]["matrix"]["part"].as_vec().map(Vec::len),
        Some(parts.len())
    );
    assert_eq!(
        job["env"]["CARGO_TARGET_WASM32_WASIP1_RUNNER"].as_str(),
        Some("wasmtime")
    );
    let steps: Vec<&Yaml> = job["steps"]
        .as_vec()
        .unwrap()
        .iter()
        .filter(|s| s["run"].as_str().is_some_and(|r| r.contains("cargo ")))
        .collect();
    assert_eq!(steps.len(), 1, "the wasm job runs cargo in one step");
    assert_eq!(
        steps[0]["env"]["RUN"].as_str(),
        Some("${{ matrix.part.run }}")
    );
    let run = steps[0]["run"].as_str().unwrap();
    for words in [
        "<<< \"$RUN\"",
        "cargo test --locked $group --target wasm32-wasip1",
        "|| status=1",
        "exit \"$status\"",
    ] {
        assert!(run.contains(words), "the wasm step lacks {words}");
    }

    // Planted: a unit dropped, a unit in two parts, a flag of no unit's form.
    let dropped = workflow.replacen(" --test laws_surfaces", "", 1);
    let problems = plan_problems(&units, &wasm_dev_parts(&dropped).unwrap());
    assert!(
        problems
            .iter()
            .any(|p| p == "geom-test test laws_surfaces is in no part"),
        "{problems:?}"
    );
    let twice = workflow.replacen(
        "--test laws_surfaces",
        "--test laws_surfaces --test laws_surfaces",
        1,
    );
    let problems = plan_problems(&units, &wasm_dev_parts(&twice).unwrap());
    assert!(
        problems
            .iter()
            .any(|p| p == "geom-test test laws_surfaces is in 2 parts"),
        "{problems:?}"
    );
    let unread = workflow.replacen("--test laws_surfaces", "--tests laws_surfaces", 1);
    assert!(wasm_dev_parts(&unread).is_err());
}
