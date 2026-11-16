//! The selection of fuzz targets and Kani proofs a change reaches (`geom_test::fuzz_scope`;
//! chapters 28.7 and 28.17), held to the crate graph, to `geom-fuzz`'s code and to
//! `.github/workflows/kernel-fuzz.yml`, which applies it to the Kani proofs. Each check is a function of what it reads,
//! and each is shown to fail on planted mistakes, so none passes by reading nothing:
//!
//! - `the_catalogue_is_every_job_and_target_of_the_workflow`: every job of the workflow is in the
//!   selection's catalogue (`fuzz_scope::JOBS`) or is one it does not decide; every Kani harness of
//!   the `kani` job is defined by a target whose tests hold a proof; and each job reads the
//!   selection: it needs `select`, its `if` reads its output, and `select` hands on every output.
//! - `a_change_selects_every_harness_that_builds_it`: for every workspace crate, a change to it
//!   selects every target whose code names it or a crate that depends on it through normal or build
//!   dependencies, read here from the targets' `use` items (syn) and the resolved graph, and every
//!   crate's Kani proofs whose crate builds it; a crate no harness can build selects nothing; and a
//!   change to a crate's tests selects nothing.
//! - `a_file_outside_every_crate_selects_everything`: a workflow, `Cargo.lock`, `.cargo/`, the
//!   toolchain file and an architecture chapter each select every job and target, and so does no
//!   list of changes at all.
//! - `geom_fuzz_files_select_the_targets_they_are_part_of`: a target's tests, corpus and crash
//!   files select it alone, its module selects it and every target that uses the module, and a
//!   file of the crate no one target owns selects every target.

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
#![allow(
    clippy::too_many_lines,
    reason = "each check and its planted mistakes read best written out in one place"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use geom_test::deps::cargo_metadata;
use geom_test::fuzz_scope::{JOBS, Job, Reach, Scope, Selection, UNSCOPED_JOBS};
use serde_json::Value;
use yaml_rust2::{Yaml, YamlLoader};

const WORKFLOW: &str = ".github/workflows/kernel-fuzz.yml";
const FUZZ: &str = "projects/engines/geometry/crates/geom-fuzz";

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

fn sources() -> BTreeMap<String, String> {
    geom_test::fuzz_scope::read_fuzz_sources(&root().join(FUZZ)).unwrap()
}

/// A mapping's string keys, in file order.
fn keys(y: &Yaml) -> Vec<String> {
    y.as_hash()
        .into_iter()
        .flatten()
        .filter_map(|(k, _)| k.as_str().map(str::to_owned))
        .collect()
}

/// A list's string items (numbers written as they read).
fn strings(y: &Yaml) -> Vec<String> {
    y.as_vec()
        .into_iter()
        .flatten()
        .map(|v| match v {
            Yaml::String(s) => s.clone(),
            Yaml::Integer(i) => i.to_string(),
            other => format!("{other:?}"),
        })
        .collect()
}

/// The jobs a job needs, whether written as one name or a list.
fn needs(job: &Yaml) -> Vec<String> {
    match &job["needs"] {
        Yaml::String(s) => vec![s.clone()],
        other => strings(other),
    }
}

/// The matrix variable a job's entries are listed under: the one key that is not `include` or
/// `exclude`.
fn matrix_variable(job: &Yaml) -> Option<String> {
    let names: Vec<String> = keys(&job["strategy"]["matrix"])
        .into_iter()
        .filter(|k| k != "include" && k != "exclude")
        .collect();
    match names.as_slice() {
        [one] => Some(one.clone()),
        _ => None,
    }
}

/// What is wrong with the catalogue `jobs` against the workflow's text and the scope.
fn catalogue_problems(workflow: &str, jobs: &[Job], scope: &Scope) -> Vec<String> {
    let mut found = Vec::new();
    let w = parse(workflow);
    for name in keys(&w["jobs"]) {
        if !UNSCOPED_JOBS.contains(&name.as_str()) && !jobs.iter().any(|j| j.name == name) {
            found.push(format!(
                "kernel-fuzz.yml's job {name} is not in the selection's catalogue"
            ));
        }
    }
    let select = &w["jobs"]["select"];
    let outputs = &select["outputs"];
    let run = select["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .filter_map(|s| s["run"].as_str())
        .find(|r| r.contains("fuzz-scope"))
        .unwrap_or_default()
        .replace('\n', " ");
    if !run.contains("kernel-quality -- fuzz-scope $SCOPE ") {
        found.push(format!(
            "the job select does not run `kernel-quality fuzz-scope $SCOPE`: {run:?}"
        ));
    }
    let handed = |key: &str| {
        outputs[key].as_str() == Some(format!("${{{{ steps.scope.outputs.{key} }}}}").as_str())
    };
    for job in jobs {
        let y = &w["jobs"][job.name];
        if y.is_badvalue() {
            found.push(format!(
                "the selection's job {} is not in kernel-fuzz.yml",
                job.name
            ));
            continue;
        }
        if !needs(y).iter().any(|n| n == "select") {
            found.push(format!("{} does not need the job select", job.name));
        }
        let condition = format!("needs.select.outputs.{} == 'true'", job.output);
        if !y["if"].as_str().is_some_and(|c| c.contains(&condition)) {
            found.push(format!(
                "{} does not run by its selection: its if has no `{condition}`",
                job.name
            ));
        }
        if !handed(job.output) {
            found.push(format!("select does not hand on {}", job.output));
        }
        if !y["strategy"]["matrix"]["exclude"].is_badvalue() {
            found.push(format!("{} excludes entries it runs whole", job.name));
        }
        match job.reach {
            Reach::ProofTargets => {
                let Some(variable) = matrix_variable(y) else {
                    found.push(format!("{} has no matrix of harnesses", job.name));
                    continue;
                };
                for v in strings(&y["strategy"]["matrix"][variable.as_str()]) {
                    let defined = scope.proof_targets().iter().any(|t| {
                        read(&format!("{FUZZ}/tests/{t}.rs")).contains(&format!("fn {v}("))
                    });
                    if !defined {
                        found.push(format!(
                            "{}'s harness {v} is defined by no target whose tests hold a Kani \
                             proof",
                            job.name
                        ));
                    }
                }
            }
            Reach::Crate(krate) => {
                if !scope.folders().contains_key(krate) {
                    found.push(format!(
                        "{}'s crate {krate} is not in the workspace",
                        job.name
                    ));
                }
            }
        }
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository and runs cargo metadata, which a WebAssembly test cannot"
)]
#[test]
fn the_catalogue_is_every_job_and_target_of_the_workflow() {
    let workflow = read(WORKFLOW);
    let scope = Scope::new(&metadata(), &sources()).unwrap();
    let found = catalogue_problems(&workflow, &JOBS, &scope);
    assert!(found.is_empty(), "{found:#?}");

    let planted = |label: &str, text: &str, jobs: &[Job], expect: &str| {
        let found = catalogue_problems(text, jobs, &scope);
        assert!(
            found.iter().any(|p| p.contains(expect)),
            "{label}: {found:#?}"
        );
    };
    let edited = |from: &str, to: &str| {
        let text = workflow.replacen(from, to, 1);
        assert_ne!(text, workflow, "the planted edit {from:?} applies");
        text
    };
    // A job the catalogue lost, and a job the workflow gained.
    let lost: Vec<Job> = JOBS
        .iter()
        .copied()
        .filter(|j| j.name != "kani-op")
        .collect();
    planted(
        "lost",
        &workflow,
        &lost,
        "job kani-op is not in the selection's catalogue",
    );
    planted(
        "gained",
        &edited(
            "\n  kani-format:\n",
            "\n  kani-extra:\n    runs-on: ubuntu-latest\n    steps:\n      - run: \"true\"\n\n  kani-format:\n",
        ),
        &JOBS,
        "job kani-extra is not in the selection's catalogue",
    );
    // A Kani harness of a target without proofs.
    planted(
        "harness",
        &edited("- orient2d_proof_filter\n", "- wide_proof\n"),
        &JOBS,
        "harness wide_proof is defined by no target",
    );
    // A job that does not read the selection, in each of the ways it reads it.
    planted(
        "no-if",
        &edited("    if: needs.select.outputs.kani_num == 'true'\n", ""),
        &JOBS,
        "kani-num does not run by its selection",
    );
    planted(
        "no-needs",
        &edited(
            "    needs: select\n    if: needs.select.outputs.kani_op",
            "    if: needs.select.outputs.kani_op",
        ),
        &JOBS,
        "kani-op does not need the job select",
    );
    planted(
        "not-handed",
        &edited(
            "      kani_topo: ${{ steps.scope.outputs.kani_topo }}\n",
            "",
        ),
        &JOBS,
        "select does not hand on kani_topo",
    );
}

/// The workspace's members, by name: their folder from the root (ending in `/`) and the names
/// their library is used by in code.
fn members(metadata: &Value) -> BTreeMap<String, (String, Vec<String>)> {
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
    let mut out = BTreeMap::new();
    for p in metadata["packages"].as_array().unwrap() {
        if !ids.contains(p["id"].as_str().unwrap()) {
            continue;
        }
        let manifest = p["manifest_path"].as_str().unwrap().replace('\\', "/");
        let folder = manifest
            .strip_prefix(&root)
            .unwrap()
            .strip_suffix("Cargo.toml")
            .unwrap()
            .to_owned();
        let libs = p["targets"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| {
                t["kind"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|k| ["lib", "rlib", "proc-macro"].contains(&k.as_str().unwrap()))
            })
            .map(|t| t["name"].as_str().unwrap().replace('-', "_"))
            .collect();
        out.insert(p["name"].as_str().unwrap().to_owned(), (folder, libs));
    }
    out
}

/// Each member's normal and build dependencies among the members, from the resolved graph.
fn built_deps(metadata: &Value) -> BTreeMap<String, BTreeSet<String>> {
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
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for node in metadata["resolve"]["nodes"].as_array().unwrap() {
        let id = node["id"].as_str().unwrap();
        if !ids.contains(id) {
            continue;
        }
        let deps = out.entry(names[id].to_owned()).or_default();
        for d in node["deps"].as_array().unwrap() {
            let pkg = d["pkg"].as_str().unwrap();
            let built = d["dep_kinds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|k| k["kind"].as_str() != Some("dev"));
            if ids.contains(pkg) && built {
                deps.insert(names[pkg].to_owned());
            }
        }
    }
    out
}

/// `start` and every crate it builds through normal and build dependencies.
fn closure(deps: &BTreeMap<String, BTreeSet<String>>, start: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack = vec![start.to_owned()];
    while let Some(c) = stack.pop() {
        if out.insert(c.clone()) {
            stack.extend(deps.get(&c).into_iter().flatten().cloned());
        }
    }
    out
}

/// The first names of a file's `use` items, and the modules it uses through `crate::`, `super::`
/// or `geom_fuzz::`, read by syn.
fn uses(text: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    fn walk(
        tree: &syn::UseTree,
        under_root: bool,
        roots: &mut BTreeSet<String>,
        modules: &mut BTreeSet<String>,
    ) {
        let (ident, rest) = match tree {
            syn::UseTree::Path(p) => (&p.ident, Some(&p.tree)),
            syn::UseTree::Name(n) => (&n.ident, None),
            syn::UseTree::Rename(r) => (&r.ident, None),
            syn::UseTree::Group(g) => {
                for t in &g.items {
                    walk(t, under_root, roots, modules);
                }
                return;
            }
            syn::UseTree::Glob(_) => return,
        };
        let id = ident.to_string();
        if under_root {
            modules.insert(id);
        } else if let Some(rest) =
            rest.filter(|_| ["crate", "super", "geom_fuzz"].contains(&id.as_str()))
        {
            walk(rest, true, roots, modules);
        } else {
            roots.insert(id);
        }
    }
    struct Uses(BTreeSet<String>, BTreeSet<String>);
    impl<'ast> syn::visit::Visit<'ast> for Uses {
        fn visit_item_use(&mut self, u: &'ast syn::ItemUse) {
            walk(&u.tree, false, &mut self.0, &mut self.1);
        }
    }
    let file = syn::parse_file(text).unwrap();
    let mut v = Uses(BTreeSet::new(), BTreeSet::new());
    syn::visit::Visit::visit_file(&mut v, &file);
    (v.0, v.1)
}

/// The crates each target's `use` items name, through the modules they use, by target.
fn named_crates(
    sources: &BTreeMap<String, String>,
    libs: &BTreeMap<String, String>,
) -> BTreeMap<String, BTreeSet<String>> {
    let targets: Vec<String> = sources
        .iter()
        .filter(|(p, text)| p.starts_with("src/") && text.contains("pub fn harness("))
        .map(|(p, _)| p["src/".len()..p.len() - 3].to_owned())
        .collect();
    let mut out = BTreeMap::new();
    for t in targets {
        let mut files = vec![
            format!("src/{t}.rs"),
            format!("tests/{t}.rs"),
            "src/lib.rs".to_owned(),
            "tests/common/mod.rs".to_owned(),
        ];
        let mut crates = BTreeSet::new();
        let mut seen = BTreeSet::new();
        while let Some(f) = files.pop() {
            let Some(text) = sources.get(&f) else {
                continue;
            };
            if !seen.insert(f) {
                continue;
            }
            let (roots, modules) = uses(text);
            crates.extend(roots.iter().filter_map(|r| libs.get(r)).cloned());
            files.extend(modules.iter().map(|m| format!("src/{m}.rs")));
        }
        crates.remove("geom-fuzz");
        out.insert(t, crates);
    }
    out
}

/// What the selection gets wrong against the crate graph and the targets' `use` items: a crate's
/// change that does not select a harness that builds it, a crate no harness builds that selects
/// one, and a change to a crate's tests that selects one.
fn dependency_problems(
    select: &dyn Fn(&[String]) -> Selection,
    metadata: &Value,
    sources: &BTreeMap<String, String>,
) -> Vec<String> {
    let members = members(metadata);
    let deps = built_deps(metadata);
    let libs: BTreeMap<String, String> = members
        .iter()
        .flat_map(|(name, (_, libs))| libs.iter().map(move |l| (l.clone(), name.clone())))
        .collect();
    let needed: BTreeMap<String, BTreeSet<String>> = named_crates(sources, &libs)
        .into_iter()
        .map(|(t, crates)| {
            let all = crates.iter().flat_map(|c| closure(&deps, c)).collect();
            (t, all)
        })
        .collect();
    let proof_targets: BTreeSet<&String> = needed
        .keys()
        .filter(|t| sources[&format!("tests/{t}.rs")].contains("kani::proof"))
        .collect();
    // What any harness could build: geom-fuzz's whole graph, and each proof crate's.
    let mut reachable: BTreeSet<String> = deps
        .get("geom-fuzz")
        .into_iter()
        .flatten()
        .flat_map(|c| closure(&deps, c))
        .collect();
    let mut found = Vec::new();
    for job in JOBS {
        if let Reach::Crate(k) = job.reach {
            reachable.extend(closure(&deps, k));
        }
    }
    for (krate, (folder, _)) in &members {
        if krate == "geom-fuzz" || folder.is_empty() {
            continue;
        }
        let file = format!("{folder}Cargo.toml");
        let selection = select(std::slice::from_ref(&file));
        let runs = |job: &str| selection.job(job).is_some_and(|j| j.why.is_some());
        for (t, built) in &needed {
            if !built.contains(krate) {
                continue;
            }
            let entry = selection
                .targets
                .iter()
                .find(|e| e.name == format!("{t}_fuzz"));
            if entry.is_none_or(|e| e.why.is_none()) {
                found.push(format!(
                    "a change to {krate} does not select {t}_fuzz, though the target builds it"
                ));
            }
            if proof_targets.contains(t) && !runs("kani") {
                found.push(format!(
                    "a change to {krate} does not run kani, though {t}'s proofs build it"
                ));
            }
        }
        for job in JOBS {
            if let Reach::Crate(k) = job.reach {
                if closure(&deps, k).contains(krate) && !runs(job.name) {
                    found.push(format!(
                        "a change to {krate} does not run {}, though {k} builds it",
                        job.name
                    ));
                }
            }
        }
        if !reachable.contains(krate) {
            for e in selection.targets.iter().filter(|e| e.why.is_some()) {
                found.push(format!(
                    "a change to {krate}, which no harness builds, selects {}",
                    e.name
                ));
            }
            for j in &selection.jobs {
                if j.why.is_some() {
                    found.push(format!(
                        "a change to {krate}, which no harness builds, runs {}",
                        j.job.name
                    ));
                }
            }
        }
        let tests = select(&[format!("{folder}tests/a_test.rs")]);
        for e in tests.targets.iter().filter(|e| e.why.is_some()) {
            found.push(format!(
                "a change to {krate}'s tests, which no harness builds, selects {}",
                e.name
            ));
        }
        for j in &tests.jobs {
            if j.why.is_some() {
                found.push(format!(
                    "a change to {krate}'s tests, which no harness builds, runs {}",
                    j.job.name
                ));
            }
        }
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository and runs cargo metadata, which a WebAssembly test cannot"
)]
#[test]
fn a_change_selects_every_harness_that_builds_it() {
    let metadata = metadata();
    let sources = sources();
    let scope = Scope::new(&metadata, &sources).unwrap();
    let select = |files: &[String]| scope.select(Some(files));
    let found = dependency_problems(&select, &metadata, &sources);
    assert!(found.is_empty(), "{found:#?}");
    // The check reaches something on each side: geom-p21 is built by one target, and
    // geom-analysis by no harness (geom-bool now is, by the solids target).
    let p21 = select(&["projects/engines/geometry/crates/geom-p21/src/lib.rs".to_owned()]);
    let running: Vec<&str> = p21
        .targets
        .iter()
        .filter(|e| e.why.is_some())
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(running, ["p21_fuzz"]);

    // Planted: the graph read without geom-test's dependencies, as if the selection stopped at
    // the first crate a target names.
    let mut cut = metadata.clone();
    let geom_test = cut["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "geom-test")
        .unwrap()["id"]
        .clone();
    for node in cut["resolve"]["nodes"].as_array_mut().unwrap() {
        if node["id"] == geom_test {
            node["deps"] = Value::Array(Vec::new());
        }
    }
    let shallow = Scope::new(&cut, &sources).unwrap();
    let found = dependency_problems(&|f| shallow.select(Some(f)), &metadata, &sources);
    assert!(
        found
            .iter()
            .any(|p| p.starts_with("a change to geom-op does not select wide_fuzz")),
        "{found:#?}"
    );
    // Planted: a target's code read without the one crate only it names.
    let mut misread = sources.clone();
    for file in ["src/p21.rs", "tests/p21.rs"] {
        let text = misread.get_mut(file).unwrap();
        *text = text.replace("geom_p21", "geom_p21_x");
    }
    let blind = Scope::new(&metadata, &misread).unwrap();
    let found = dependency_problems(&|f| blind.select(Some(f)), &metadata, &sources);
    assert!(
        found
            .iter()
            .any(|p| p.starts_with("a change to geom-p21 does not select p21_fuzz")),
        "{found:#?}"
    );
    // Planted: a selection that runs everything whatever changed.
    let found = dependency_problems(&|_| scope.select(None), &metadata, &sources);
    assert!(
        found
            .iter()
            .any(|p| p.starts_with("a change to geom-analysis, which no harness builds, runs")),
        "{found:#?}"
    );
}

/// Files in no crate's folder, of each kind the rule names.
const OUTSIDE: [&str; 6] = [
    ".github/workflows/kernel-fuzz.yml",
    ".github/workflows/kernel.yml",
    "Cargo.lock",
    ".cargo/config.toml",
    "rust-toolchain.toml",
    "projects/engines/geometry/architecture/28-testing.md",
];

/// What a selection leaves out that it should run.
fn left_out(selection: &Selection) -> Vec<String> {
    let mut out = Vec::new();
    for j in &selection.jobs {
        if j.why.is_none() {
            out.push(j.job.name.to_owned());
        }
    }
    for e in selection.targets.iter().filter(|e| e.why.is_none()) {
        out.push(e.name.clone());
    }
    out
}

/// What a file outside every crate does not select.
fn outside_problems(select: &dyn Fn(&[String]) -> Selection) -> Vec<String> {
    let mut found = Vec::new();
    for file in OUTSIDE {
        let changed = [file.to_owned(), format!("{FUZZ}/tests/wide.rs")];
        let missing = left_out(&select(&changed));
        if !missing.is_empty() {
            found.push(format!(
                "{file} is in no crate, but the change does not select {missing:?}"
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
fn a_file_outside_every_crate_selects_everything() {
    let scope = Scope::new(&metadata(), &sources()).unwrap();
    for file in OUTSIDE {
        assert!(
            !scope
                .folders()
                .values()
                .any(|f| file.starts_with(f.as_str())),
            "{file} is in a crate's folder"
        );
    }
    let found = outside_problems(&|f| scope.select(Some(f)));
    assert!(found.is_empty(), "{found:#?}");
    assert_eq!(left_out(&scope.select(None)), Vec::<String>::new());
    for (key, value) in &scope.select(None).outputs() {
        assert_eq!(value, "true", "{key}");
    }

    // Planted: a selection that reads only the files under projects/.
    let found = outside_problems(&|f| {
        let kept: Vec<String> = f
            .iter()
            .filter(|p| p.starts_with("projects/"))
            .cloned()
            .collect();
        scope.select(Some(&kept))
    });
    assert!(
        found
            .iter()
            .any(|p| p.starts_with("Cargo.lock is in no crate, but the change does not select")),
        "{found:#?}"
    );
}

/// What `geom-fuzz`'s own files select wrongly: a target's tests, corpus or crashes anything but
/// it, its module not it and the targets that use the module, a shared file not everything.
fn fuzz_file_problems(
    select: &dyn Fn(&[String]) -> Selection,
    scope: &Scope,
    sources: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut found = Vec::new();
    let used: BTreeMap<&String, BTreeSet<String>> =
        sources.iter().map(|(f, text)| (f, uses(text).1)).collect();
    let running = |files: &[String]| -> BTreeSet<String> {
        select(files)
            .targets
            .iter()
            .filter(|e| e.why.is_some())
            .map(|e| e.name.trim_end_matches("_fuzz").to_owned())
            .collect()
    };
    for t in scope.targets().keys() {
        for own in [
            format!("{FUZZ}/tests/{t}.rs"),
            format!("{FUZZ}/tests/__fuzz__/{t}/corpus/0123abcd"),
            format!("{FUZZ}/tests/__fuzz__/{t}/crashes/crash-0123abcd"),
        ] {
            let got = running(std::slice::from_ref(&own));
            if got != BTreeSet::from([t.clone()]) {
                found.push(format!("{own} selects {got:?}, not {t} alone"));
            }
        }
        let module = format!("{FUZZ}/src/{t}.rs");
        let got = running(std::slice::from_ref(&module));
        let users: BTreeSet<String> = scope
            .targets()
            .keys()
            .filter(|u| {
                *u == t
                    || [format!("src/{u}.rs"), format!("tests/{u}.rs")]
                        .iter()
                        .any(|f| used.get(f).is_some_and(|m| m.contains(t)))
            })
            .cloned()
            .collect();
        if !users.is_subset(&got) {
            found.push(format!(
                "{module} selects {got:?}, not every one of {users:?}"
            ));
        }
    }
    let all: BTreeSet<String> = scope.targets().keys().cloned().collect();
    for shared in [
        "Cargo.toml",
        "src/lib.rs",
        "tests/common/mod.rs",
        "README.md",
    ] {
        let file = format!("{FUZZ}/{shared}");
        let got = running(std::slice::from_ref(&file));
        if got != all {
            found.push(format!(
                "{file} is no one target's, but selects only {got:?}"
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
fn geom_fuzz_files_select_the_targets_they_are_part_of() {
    let sources = sources();
    let scope = Scope::new(&metadata(), &sources).unwrap();
    let found = fuzz_file_problems(&|f| scope.select(Some(f)), &scope, &sources);
    assert!(found.is_empty(), "{found:#?}");
    // The module cbor is the target cbor's, and the target contract uses it too.
    let cbor = scope.select(Some(&[format!("{FUZZ}/src/cbor.rs")]));
    let running: Vec<&str> = cbor
        .targets
        .iter()
        .filter(|e| e.why.is_some())
        .map(|e| e.name.as_str())
        .collect();
    assert!(
        running.contains(&"contract_fuzz") && running.contains(&"cbor_fuzz"),
        "{running:?}"
    );
    // orient2d's tests hold the Kani harnesses: they run the kani job, and nothing else does.
    let kani = |file: &str| {
        scope
            .select(Some(&[file.to_owned()]))
            .job("kani")
            .unwrap()
            .why
            .is_some()
    };
    assert!(kani(&format!("{FUZZ}/tests/orient2d.rs")));
    assert!(!kani(&format!("{FUZZ}/tests/wide.rs")));

    // Planted: a selection blind to the corpus folders.
    let found = fuzz_file_problems(
        &|f| {
            let kept: Vec<String> = f
                .iter()
                .filter(|p| !p.contains("/__fuzz__/"))
                .cloned()
                .collect();
            scope.select(Some(&kept))
        },
        &scope,
        &sources,
    );
    assert!(
        found
            .iter()
            .any(|p| p.contains("/tests/__fuzz__/make/corpus/") && p.contains("not make alone")),
        "{found:#?}"
    );
}
