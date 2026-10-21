//! The dependency rule on the real workspace, proof that the check catches what it must, and proof
//! that CI reruns it whenever a manifest it reads changes.
//!
//! `workspace_keeps_the_dependency_rule` runs `cargo metadata` on this workspace and fails if any
//! kernel crate has a normal or build dependency on a development crate (chapter 27, 28.1). CI
//! runs it on every pull request that touches a file the rule reads
//! (`.github/workflows/kernel-dev.yml`); `ci_reruns_the_rule_when_a_manifest_it_reads_changes`
//! checks the workflow's path filters cover every such file.
//!
//! A check that never fails proves nothing, so the other tests seed violations into the real
//! metadata, in memory, one at a time: a normal, a build, a platform-specific, a renamed optional
//! and a transitive declaration, one from the facade, and chains that only cargo's resolved graph
//! shows, through a path crate outside the workspace, a git crate and an edge that is both a
//! dev-dependency and a normal one. Each must be caught, with its chain. A dev-dependency must not
//! be, and metadata the check cannot read must be an error, never a pass.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use geom_test::deps::{DepsError, Kind, Step, Violation, cargo_metadata, check};
use serde_json::{Value, json};

/// This workspace's `cargo metadata`, from the cargo that built this test.
fn metadata() -> Value {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from(env!("CARGO")));
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    cargo_metadata(&cargo, &manifest).unwrap()
}

fn workspace_root(m: &Value) -> PathBuf {
    PathBuf::from(m["workspace_root"].as_str().unwrap())
}

/// The package named `name` in the metadata (the first, if several versions are in the graph).
fn package<'a>(m: &'a mut Value, name: &str) -> &'a mut Value {
    m["packages"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("no package {name} in the graph"))
}

/// The id of the package named `name`.
fn id_of(m: &mut Value, name: &str) -> String {
    package(m, name)["id"].as_str().unwrap().to_owned()
}

/// Adds a dependency declaration, in `cargo metadata`'s format, to package `from`.
fn declare(m: &mut Value, from: &str, dep: Value) {
    package(m, from)["dependencies"]
        .as_array_mut()
        .unwrap()
        .push(dep);
}

/// Adds a package with no dependencies to the graph, and to the resolved graph; with `member` set,
/// also to the workspace members. Returns its id.
fn add_package(m: &mut Value, name: &str, source: &Value, manifest: &Path, member: bool) -> String {
    let id = format!("fixture+{name}#0.0.0");
    if member {
        m["workspace_members"]
            .as_array_mut()
            .unwrap()
            .push(json!(id));
    }
    m["packages"].as_array_mut().unwrap().push(json!({
        "name": name, "version": "0.0.0", "id": id, "source": source,
        "manifest_path": manifest.to_str().unwrap(), "dependencies": [],
    }));
    m["resolve"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": id, "deps": [], "dependencies": [], "features": [] }));
    id
}

/// A workspace member named `name` at projects/fixture/<name>, outside the kernel's folders: an
/// intermediate crate no real crate depends on, so a seeded chain through it is the only one.
/// Returns its folder.
fn add_member(m: &mut Value, name: &str) -> PathBuf {
    let folder = workspace_root(m)
        .join("projects")
        .join("fixture")
        .join(name);
    add_package(m, name, &Value::Null, &folder.join("Cargo.toml"), true);
    folder
}

/// Adds an edge of the resolved graph from package id `from` to package id `to`, with the given
/// kinds (`null`, `"build"` or `"dev"`).
fn resolve_edge(m: &mut Value, from: &str, to: &str, kinds: &[Value]) {
    let name = m["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == to)
        .map(|p| p["name"].as_str().unwrap().replace('-', "_"))
        .unwrap();
    let dep_kinds: Vec<Value> = kinds
        .iter()
        .map(|k| json!({ "kind": k, "target": null }))
        .collect();
    let node = m["resolve"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == from)
        .unwrap();
    node["deps"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "name": name, "pkg": to, "dep_kinds": dep_kinds }));
}

/// A path dependency declaration on `to`, in `folder`, of the given kind (`null`, `"build"` or
/// `"dev"`).
fn path_dep(to: &str, kind: &Value, folder: &Path) -> Value {
    json!({
        "name": to, "source": null, "req": "*", "kind": kind, "rename": null,
        "optional": false, "uses_default_features": true, "features": [], "target": null,
        "registry": null, "path": folder.to_str().unwrap(),
    })
}

/// A path dependency declaration on one of the real development crates.
fn dev_crate_dep(to: &str, kind: &Value) -> Value {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(to);
    path_dep(to, kind, &folder)
}

fn step(to: &str, kind: Kind) -> Step {
    Step {
        to: to.to_owned(),
        kind,
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "runs cargo as a child process, which WebAssembly cannot"
)]
#[test]
fn workspace_keeps_the_dependency_rule() {
    let report = check(&metadata()).unwrap();
    for v in &report.violations {
        println!("violation: {v}");
    }
    assert!(
        report.violations.is_empty(),
        "kernel crates depend on development crates: {:#?}",
        report.violations
    );
    // The check checked something: the numbers layer and the facade are kernel crates, and the
    // development crates this piece adds are recognised as such, never as kernel crates.
    println!("kernel crates checked: {:?}", report.kernel_crates);
    println!("manifests read: {:?}", report.manifests);
    assert!(report.kernel_crates.iter().any(|k| k == "geom-num"));
    assert!(report.kernel_crates.iter().any(|k| k == "engine-geometry"));
    for dev in ["geom-test", "geom-fuzz", "geom-gen"] {
        assert!(report.development_crates.iter().any(|d| d == dev), "{dev}");
        assert!(!report.kernel_crates.iter().any(|k| k == dev), "{dev}");
    }
    // Other projects' crates are not kernel crates, but the facade's kit-core is read.
    assert!(!report.kernel_crates.iter().any(|k| k == "kit-core"));
    for m in [
        "projects/engines/geometry/crates/geom-num/Cargo.toml",
        "projects/engines/crates/engine-geometry/Cargo.toml",
        "projects/kit/crates/kit-core/Cargo.toml",
    ] {
        assert!(report.manifests.iter().any(|r| r == m), "{m}");
    }
}

/// A mutation of the real metadata, and the one violation it must produce.
type Case = (&'static str, fn(&mut Value), Violation);

fn violation(kernel: &str, chain: Vec<Step>) -> Violation {
    Violation {
        kernel_crate: kernel.into(),
        chain,
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "runs cargo as a child process, which WebAssembly cannot"
)]
#[allow(
    clippy::too_many_lines,
    reason = "one table of seeded violations, each case beside the chain it must produce"
)]
#[test]
fn seeded_violations_are_caught() {
    let base = metadata();
    assert_eq!(check(&base).unwrap().violations, Vec::<Violation>::new());

    let cases: [Case; 9] = [
        (
            "normal",
            |m| {
                let d = dev_crate_dep("geom-test", &Value::Null);
                declare(m, "geom-num", d);
            },
            violation("geom-num", vec![step("geom-test", Kind::Normal)]),
        ),
        (
            "build",
            |m| {
                let d = dev_crate_dep("geom-gen", &json!("build"));
                declare(m, "geom-num", d);
            },
            violation("geom-num", vec![step("geom-gen", Kind::Build)]),
        ),
        (
            "platform-specific",
            |m| {
                let mut d = dev_crate_dep("geom-fuzz", &Value::Null);
                d["target"] = json!("cfg(target_os = \"none-such\")");
                declare(m, "geom-num", d);
            },
            violation("geom-num", vec![step("geom-fuzz", Kind::Normal)]),
        ),
        (
            "renamed, optional, from a registry",
            |m| {
                declare(
                    m,
                    "geom-num",
                    json!({
                        "name": "geom-bench", "source": "registry+https://github.com/rust-lang/crates.io-index",
                        "req": "^1", "kind": null, "rename": "benches", "optional": true,
                        "uses_default_features": true, "features": [], "target": null, "registry": null,
                    }),
                );
            },
            violation("geom-num", vec![step("geom-bench", Kind::Normal)]),
        ),
        (
            "transitive, declared through another workspace crate",
            |m| {
                let folder = add_member(m, "fixture-middle");
                declare(
                    m,
                    "geom-num",
                    path_dep("fixture-middle", &Value::Null, &folder),
                );
                let d = dev_crate_dep("geom-test", &json!("build"));
                declare(m, "fixture-middle", d);
            },
            violation(
                "geom-num",
                vec![
                    step("fixture-middle", Kind::Normal),
                    step("geom-test", Kind::Build),
                ],
            ),
        ),
        (
            "from the facade",
            |m| {
                let d = dev_crate_dep("geom-gen", &Value::Null);
                declare(m, "engine-geometry", d);
            },
            violation("engine-geometry", vec![step("geom-gen", Kind::Normal)]),
        ),
        (
            "through a path crate outside the workspace, in the resolved graph only",
            |m| {
                let outside = workspace_root(m)
                    .parent()
                    .unwrap()
                    .join("fixture-outside")
                    .join("Cargo.toml");
                let shim = add_package(m, "fixture-outside", &Value::Null, &outside, false);
                let num = id_of(m, "geom-num");
                let test = id_of(m, "geom-test");
                resolve_edge(m, &num, &shim, &[Value::Null]);
                resolve_edge(m, &shim, &test, &[Value::Null]);
            },
            violation(
                "geom-num",
                vec![
                    step("fixture-outside", Kind::Normal),
                    step("geom-test", Kind::Normal),
                ],
            ),
        ),
        (
            "through a git crate, as a build dependency, in the resolved graph only",
            |m| {
                let manifest = workspace_root(m)
                    .parent()
                    .unwrap()
                    .join("git-checkouts")
                    .join("fixture-git")
                    .join("Cargo.toml");
                let git = add_package(
                    m,
                    "fixture-git",
                    &json!("git+https://example.invalid/fixture-git#0123abcd"),
                    &manifest,
                    false,
                );
                let num = id_of(m, "geom-num");
                let fuzz = id_of(m, "geom-fuzz");
                resolve_edge(m, &num, &git, &[json!("build")]);
                resolve_edge(m, &git, &fuzz, &[Value::Null]);
            },
            violation(
                "geom-num",
                vec![
                    step("fixture-git", Kind::Build),
                    step("geom-fuzz", Kind::Normal),
                ],
            ),
        ),
        (
            "through an edge that is a dev-dependency and a normal one at once",
            |m| {
                add_member(m, "fixture-both");
                let both = id_of(m, "fixture-both");
                let num = id_of(m, "geom-num");
                let gen_id = id_of(m, "geom-gen");
                resolve_edge(m, &num, &both, &[json!("dev"), Value::Null]);
                resolve_edge(m, &both, &gen_id, &[Value::Null]);
            },
            violation(
                "geom-num",
                vec![
                    step("fixture-both", Kind::Normal),
                    step("geom-gen", Kind::Normal),
                ],
            ),
        ),
    ];

    for (what, mutate, want) in cases {
        let mut m = base.clone();
        mutate(&mut m);
        let report = check(&m).unwrap();
        assert_eq!(report.violations, vec![want.clone()], "case: {what}");
        println!("caught ({what}): {want}");
    }

    // A local crate outside the workspace that a kernel crate reaches is among the manifests read.
    let mut m = base.clone();
    let outside = workspace_root(&m)
        .parent()
        .unwrap()
        .join("fixture-outside")
        .join("Cargo.toml");
    let shim = add_package(&mut m, "fixture-outside", &Value::Null, &outside, false);
    let num = id_of(&mut m, "geom-num");
    resolve_edge(&mut m, &num, &shim, &[Value::Null]);
    let report = check(&m).unwrap();
    assert_eq!(report.violations, Vec::<Violation>::new());
    assert!(
        report
            .manifests
            .iter()
            .any(|r| Path::new(r) == outside.as_path()),
        "{:?}",
        report.manifests
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "runs cargo as a child process, which WebAssembly cannot"
)]
#[test]
fn development_edges_are_allowed() {
    let base = metadata();

    // A kernel crate may take a development crate as a dev-dependency (28.1).
    let mut m = base.clone();
    let d = dev_crate_dep("geom-test", &json!("dev"));
    declare(&mut m, "geom-num", d);
    assert_eq!(check(&m).unwrap().violations, Vec::<Violation>::new());

    // A dev edge is not followed, declared or resolved: what a dev-dependency depends on never
    // enters a kernel build.
    let mut m = base.clone();
    let folder = add_member(&mut m, "fixture-middle");
    declare(
        &mut m,
        "geom-num",
        path_dep("fixture-middle", &json!("dev"), &folder),
    );
    let d = dev_crate_dep("geom-test", &Value::Null);
    declare(&mut m, "fixture-middle", d);
    let (num, middle, test) = (
        id_of(&mut m, "geom-num"),
        id_of(&mut m, "fixture-middle"),
        id_of(&mut m, "geom-test"),
    );
    resolve_edge(&mut m, &num, &middle, &[json!("dev")]);
    resolve_edge(&mut m, &middle, &test, &[Value::Null]);
    assert_eq!(check(&m).unwrap().violations, Vec::<Violation>::new());

    // The fixture's chain is live: the same edge as a normal one is caught.
    resolve_edge(&mut m, &num, &middle, &[Value::Null]);
    assert_eq!(check(&m).unwrap().violations.len(), 1);

    // Development crates may depend on each other and on kernel crates.
    let mut m = base.clone();
    let d = dev_crate_dep("geom-fuzz", &Value::Null);
    declare(&mut m, "geom-gen", d);
    assert_eq!(check(&m).unwrap().violations, Vec::<Violation>::new());
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "runs cargo as a child process, which WebAssembly cannot"
)]
#[test]
fn unreadable_metadata_is_an_error_not_a_pass() {
    let base = metadata();

    let mut m = base.clone();
    m.as_object_mut().unwrap().remove("packages");
    assert_eq!(
        check(&m),
        Err(DepsError::Field("packages".into())),
        "missing packages"
    );

    let mut m = base.clone();
    let d = dev_crate_dep("geom-test", &json!("peer"));
    declare(&mut m, "geom-num", d);
    assert_eq!(
        check(&m),
        Err(DepsError::UnknownKind {
            package: "geom-num".into(),
            kind: "peer".into()
        })
    );

    // An unknown kind in the resolved graph is refused too.
    let mut m = base.clone();
    let (num, test) = (id_of(&mut m, "geom-num"), id_of(&mut m, "geom-test"));
    resolve_edge(&mut m, &num, &test, &[json!("peer")]);
    assert!(matches!(check(&m), Err(DepsError::UnknownKind { .. })));

    // Without the resolved graph (what `cargo metadata --no-deps` gives) the check would miss
    // every chain through a crate that is not a workspace member, so it refuses.
    let mut m = base.clone();
    m["resolve"] = Value::Null;
    assert!(matches!(check(&m), Err(DepsError::Field(f)) if f.starts_with("resolve")));

    // Without its kernel crates the check would check nothing, so it refuses.
    let mut m = base.clone();
    m["workspace_root"] = json!("/not/this/workspace");
    assert_eq!(check(&m), Err(DepsError::NoKernelCrates));

    assert!(matches!(check(&json!({})), Err(DepsError::Field(_))));
}

// ---------------------------------------------------------------------------------------------
// CI reruns the rule whenever a file it reads changes
// ---------------------------------------------------------------------------------------------

/// The `paths:` lists of each trigger under `on:` in a workflow written in the block-list form
/// kernel-dev.yml uses. Anything else under a trigger is skipped; a flow-form list (`[a, b]`) for
/// `paths` is refused, so the reader never misses a filter.
fn path_filters(yaml: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut in_on = false;
    let mut trigger: Option<String> = None;
    let mut paths_indent: Option<usize> = None;
    for line in yaml.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent == 0 {
            in_on = t == "on:";
            trigger = None;
            paths_indent = None;
            continue;
        }
        if !in_on {
            continue;
        }
        if indent == 2 {
            assert!(t.ends_with(':'), "unexpected trigger line {line:?}");
            trigger = Some(t.trim_end_matches(':').to_owned());
            paths_indent = None;
            continue;
        }
        if t.starts_with("paths:") {
            assert_eq!(t, "paths:", "paths must be a block list: {line:?}");
            paths_indent = Some(indent);
            out.entry(trigger.clone().unwrap()).or_default();
            continue;
        }
        if let (Some(pi), Some(tr)) = (paths_indent, &trigger) {
            if indent > pi {
                let item = t.strip_prefix("- ").unwrap_or_else(|| panic!("{line:?}"));
                let item = item.trim().trim_matches('"').trim_matches('\'').to_owned();
                out.get_mut(tr).unwrap().push(item);
                continue;
            }
            paths_indent = None;
        }
    }
    out
}

/// GitHub's filter-pattern match for the forms the workflows use: `**` matches any characters,
/// `*` any characters except `/`, everything else itself. A pattern with any other special
/// character (`?`, `[`, `]`, `+`, a leading `!`) is refused, so the matcher never misreads one.
fn glob(p: &[u8], s: &[u8]) -> bool {
    match p {
        [] => s.is_empty(),
        [b'*', b'*', rest @ ..] => (0..=s.len()).any(|i| glob(rest, &s[i..])),
        [b'*', rest @ ..] => {
            let mut i = 0;
            loop {
                if glob(rest, &s[i..]) {
                    return true;
                }
                if i == s.len() || s[i] == b'/' {
                    return false;
                }
                i += 1;
            }
        }
        [c, rest @ ..] => s.first() == Some(c) && glob(rest, &s[1..]),
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "runs cargo as a child process, which WebAssembly cannot"
)]
#[test]
fn ci_reruns_the_rule_when_a_manifest_it_reads_changes() {
    let report = check(&metadata()).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .to_path_buf();
    let yaml = std::fs::read_to_string(root.join(".github/workflows/kernel-dev.yml")).unwrap();
    let filters = path_filters(&yaml);
    assert_eq!(
        filters.keys().map(String::as_str).collect::<Vec<_>>(),
        ["pull_request", "push"],
        "{filters:?}"
    );
    for patterns in filters.values() {
        for p in patterns {
            assert!(
                !p.starts_with('!') && !p.contains(['?', '[', ']', '+']),
                "unsupported filter pattern {p:?}"
            );
        }
    }
    // The manifests the rule read, and what else decides its answer: the workspace manifest
    // (members, [patch], [workspace.dependencies]), the lock file, any cargo configuration, and
    // the workflow itself.
    let mut files = report.manifests.clone();
    for extra in [
        "Cargo.toml",
        "Cargo.lock",
        ".cargo/config.toml",
        ".github/workflows/kernel-dev.yml",
    ] {
        files.push(extra.to_owned());
    }
    let mut uncovered = Vec::new();
    for (trigger, patterns) in &filters {
        for f in &files {
            if !patterns.iter().any(|p| glob(p.as_bytes(), f.as_bytes())) {
                uncovered.push(format!("{trigger}: {f}"));
            }
        }
    }
    assert!(
        uncovered.is_empty(),
        "kernel-dev.yml does not run when these files change, though the dependency rule reads \
         them:\n  {}\nfilters: {filters:?}",
        uncovered.join("\n  ")
    );
}

#[test]
fn the_filter_matcher_reads_patterns_as_github_does() {
    for (p, s, want) in [
        (
            "projects/engines/geometry/**",
            "projects/engines/geometry/crates/a/Cargo.toml",
            true,
        ),
        (
            "projects/engines/geometry/**",
            "projects/engines/crates/a/Cargo.toml",
            false,
        ),
        (
            "projects/kit/crates/*/Cargo.toml",
            "projects/kit/crates/kit-core/Cargo.toml",
            true,
        ),
        (
            "projects/kit/crates/*/Cargo.toml",
            "projects/kit/crates/a/b/Cargo.toml",
            false,
        ),
        ("Cargo.toml", "Cargo.toml", true),
        ("Cargo.toml", "x/Cargo.toml", false),
        (".cargo/**", ".cargo/config.toml", true),
    ] {
        assert_eq!(glob(p.as_bytes(), s.as_bytes()), want, "{p} on {s}");
    }
    let yaml = "on:\n  pull_request:\n    paths:\n      - \"a/**\"\n      - b\n  push:\n    \
                branches: [main]\n    paths:\n      - 'c'\npermissions:\n  contents: read\n";
    let f = path_filters(yaml);
    assert_eq!(f["pull_request"], ["a/**", "b"]);
    assert_eq!(f["push"], ["c"]);
}
