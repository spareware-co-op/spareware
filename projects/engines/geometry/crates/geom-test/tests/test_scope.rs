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
//! - `a_crate_that_reads_outside_its_folder_is_found`: the words that mark a read outside a
//!   crate's folder are found in planted sources, and the repository's readers include
//!   `geom-test`.
//! - `the_suites_run_the_selection`: `kernel.yml`'s job `scope` runs `kernel-quality test-scope`
//!   on a pull request's changed files, and its suites test only the crates it names, keeping
//!   their job names, with nextest told that a part with no test passes (`NEXTEST_NO_TESTS`); `kernel-bench.yml`'s count
//!   gates measure only when the change reaches what `geom-bench` builds.

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

use geom_test::deps::cargo_metadata;
use geom_test::test_scope::{Scope, Selection, text_reads_outside};
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
        .any(|r| r.contains("git diff --no-ext-diff --no-renames --name-only HEAD^1 HEAD"))
    {
        found.push(
            "kernel.yml's job scope does not read the pull request's changed files".to_owned(),
        );
    }
    if text(&scope["outputs"]["crates"]) != "${{ steps.scope.outputs.crates }}" {
        found.push("kernel.yml's job scope does not hand on its crates".to_owned());
    }
    for job in ["suite", "wasm-suite"] {
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
        if !y["if"].is_badvalue() {
            found.push(format!(
                "{job} has a job condition, so a skipped run would not report under its names"
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
            "\"$RUNNER_TEMP/changed.txt\" --build-of geom-bench",
            "\"$RUNNER_TEMP/changed.txt\" --build-of geom-num",
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
