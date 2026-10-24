//! The merge gate's builds, lints and workflow (piece G12; chapter 28.8).
//!
//! What it does:
//!
//! - `build` is the gate's build of one crate: `cargo build --target <target>` with warnings
//!   denied, in a build folder of its own, once for each feature selection of the crate
//!   (`feature_selections`). The gate builds every `no_std` kernel crate on its own for
//!   `wasm32v1-none`, a WebAssembly target with `core` and `alloc` and no `std`, which is what
//!   proves `no_std` (a crate that links `std`, through its own source or a dependency, does not
//!   build there), and for `wasm32-unknown-unknown`, the browsers' target (chapter 28.8: "A
//!   `no_std` build of every kernel crate, and a wasm32 build").
//! - `clippy` is the gate's clippy run: `cargo clippy --all-targets -- -D warnings`, so every lint
//!   the crates' settings warn on is an error too, once for each feature selection. The gate runs
//!   it over every crate that ships, and fails on clippy's "does not refer to a reachable" message
//!   (`lapsed_bans`), which clippy 1.99.0 prints as a warning that `-D warnings` does not turn
//!   into an error, so no ban whose path a toolchain renamed lapses in silence (chapters 21, 28.8).
//! - Code behind a feature is code too: cargo-deny reads every feature's dependencies, and the
//!   builds and clippy runs cover every feature the way cargo-hack's `--each-feature` does (default
//!   features, none, each feature on its own, and all), so a feature-gated module is linted and
//!   built like the rest (the facade's `threads` code, chapter 27, is such code).
//! - `compiled_sources` reads, from each run's artifact messages, the compiler's dep-info for every
//!   library it built of the crates asked for: every file rustc read to compile that library, a
//!   module taken from another folder by `#[path]`, a file pasted in by `include!` and code a build
//!   script wrote included. The gate holds all of them to the source rules (`globals`,
//!   `overrides`), so code the folder scan does not see is still read.
//! - Seeded probes show each fails when what it checks is wrong: a crate that links `std`, one that
//!   assumes a 64-bit `usize` (plainly, and behind a feature), and one with a warning, for the
//!   builds; and for clippy, a crate built with `geom-num`'s own lint settings (`Cargo.toml` and
//!   `clippy.toml`, read from the repository) that calls every method of chapter 21's lists once,
//!   names each of its types, and breaks each of the denials once (one of them behind a feature),
//!   line by line, beside lines that keep them all. Every seeded line must give exactly its own
//!   errors and every other line none, so a misspelt method path, which clippy 1.99.0 does not
//!   always report itself (a probe showed it passes `f64::no_such_method` in silence), fails here
//!   because its call passes; and the same settings on the clean lines alone must give no
//!   diagnostic at all. A configuration with a path that does not resolve must be caught.
//! - `workflow` reads `.github/workflows/kernel-gate.yml` and reports every way it does not run,
//!   pin or require what the gate needs, or could pass while a check did not run or its failure
//!   was swallowed (`gate_workflow_*` tests, with seeded faults); `filters_select_every_gate_test`
//!   shows the workflow's test filters select every ignored gate test and none selects nothing.
//!
//! What it guarantees: the builds and clippy runs are the same functions on the repository and on
//! the probes, so what the probes show is what the gate runs. A run that cannot start fails, and so
//! does a library whose dep-info cannot be found. The tests that build are ignored by a plain
//! `cargo test`, which may run where the WebAssembly targets or clippy are not installed; the merge
//! gate installs them and runs the tests (`--ignored gate_build`, `--ignored gate_clippy`). Each
//! builds in its own folder under the test's `CARGO_TARGET_TMPDIR`, never in the build folder of
//! the `cargo test` that runs it, so it never waits on that folder's lock. Every run starts without
//! the environment variables that would change what clippy reads or which compiler runs
//! (`SCRUBBED_ENV`), with the gate's own `RUSTFLAGS`.
//!
//! Method: cargo's `--target`, `--message-format json` and `RUSTFLAGS`
//! (<https://doc.rust-lang.org/cargo/reference/external-tools.html#json-messages>); rustc's
//! dep-info, the Makefile rules `--emit=dep-info` writes, each file it read also given a rule of
//! its own (<https://doc.rust-lang.org/rustc/command-line-arguments.html#--emit-specifies-the-types-of-output-files-to-generate>);
//! cargo-hack's `--each-feature` (taiki-e/cargo-hack README at commit
//! `d398b7c6e52652c0640e07711af959d65d94e4b4`, Apache-2.0 OR MIT; its method only, nothing
//! copied); rustc's platform support for `wasm32v1-none`
//! (<https://doc.rust-lang.org/rustc/platform-support/wasm32v1-none.html>); clippy's configuration
//! and `disallowed_methods` and `disallowed_types` lints
//! (<https://doc.rust-lang.org/clippy/lint_configuration.html>); libtest's `--list` and filters.
//!
//! Cost: one build per kernel crate, target and feature selection; one clippy run over the crates
//! that ship per feature selection; six small probe crates built three ways for the builds, and
//! three for clippy.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The kernel lints deny native arithmetic in kernel code (Cargo.toml). This is test code: it
// counts lines of a probe it writes, which tests may do with native arithmetic.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "test code; the rule is for kernel code"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

use crate::json::{self, Json};
use crate::lints::carries_the_posture;
use crate::posture::{
    ALGEBRAIC, CLOCKS_AND_IDENTITY, PLATFORM_MATHS, REDUCTIONS, REQUIRED_TYPES, required_methods,
};
use crate::repo;
use crate::table::{Kind, Table, full_name};

/// The target without `std` that proves a kernel crate `no_std`.
pub const NO_STD_TARGET: &str = "wasm32v1-none";

/// The browsers' WebAssembly target.
pub const WASM_TARGET: &str = "wasm32-unknown-unknown";

/// The targets the gate builds every kernel crate for.
pub const BUILD_TARGETS: [&str; 2] = [NO_STD_TARGET, WASM_TARGET];

/// The flags every gate build takes: a warning fails it.
pub const RUSTFLAGS: &str = "-D warnings";

/// The arguments every gate clippy run takes after its package and feature selection.
pub const CLIPPY_ARGS: [&str; 4] = ["--all-targets", "--", "-D", "warnings"];

/// The environment variables every gate run starts without: each would change, unseen by any
/// manifest or `clippy.toml`, which configuration clippy reads (`CLIPPY_CONF_DIR`), which compiler
/// or wrapper runs, which target a run without `--target` builds for, or the flags the gate sets.
pub const SCRUBBED_ENV: [&str; 9] = [
    "CARGO_ENCODED_RUSTFLAGS",
    "CLIPPY_CONF_DIR",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTC",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_TARGET",
];

/// A folder under the test's temporary folder, created.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs the cargo subcommand `args[0]` with the rest of `args` on `manifest`, building in
/// `target_dir`, with the gate's `RUSTFLAGS` in place of any the caller's environment holds and
/// `SCRUBBED_ENV` removed.
fn cargo(args: &[&str], manifest: &Path, target_dir: &Path) -> Output {
    let (subcommand, rest) = args.split_first().expect("a cargo subcommand");
    let mut command = Command::new(repo::cargo());
    command
        .arg(subcommand)
        .arg("--manifest-path")
        .arg(manifest)
        .args(rest)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("RUSTFLAGS", RUSTFLAGS);
    for name in SCRUBBED_ENV {
        command.env_remove(name);
    }
    command
        .output()
        .unwrap_or_else(|e| panic!("cannot start cargo: {e}"))
}

/// A crate's features and what each switches on.
pub type Features = BTreeMap<String, Vec<String>>;

/// Every crate's features in the workspace of `manifest`, by crate name, as `cargo metadata` lists
/// them, and the name of the manifest's own crate, if it has one.
fn workspace_features(manifest: &Path) -> (BTreeMap<String, Features>, Option<String>) {
    let output = Command::new(repo::cargo())
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .unwrap_or_else(|e| panic!("cannot start cargo metadata: {e}"));
    assert!(
        output.status.success(),
        "cargo metadata failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata = json::parse(&String::from_utf8_lossy(&output.stdout)).unwrap();
    let canonical = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let mut all = BTreeMap::new();
    let mut own = None;
    for package in metadata.get("packages").and_then(Json::as_array).unwrap() {
        let name = package
            .get("name")
            .and_then(Json::as_str)
            .unwrap()
            .to_owned();
        let is_own = package
            .get("manifest_path")
            .and_then(Json::as_str)
            .is_some_and(|m| canonical(Path::new(m)) == canonical(manifest));
        if is_own {
            own = Some(name.clone());
        }
        let features = package
            .get("features")
            .and_then(Json::as_object)
            .unwrap()
            .iter()
            .map(|(feature, on)| {
                let on = on
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|f| f.as_str().unwrap().to_owned())
                    .collect();
                (feature.clone(), on)
            })
            .collect();
        all.insert(name, features);
    }
    (all, own)
}

/// The features of `package` (or of the manifest's own crate) and what each switches on.
pub fn features_of(manifest: &Path, package: Option<&str>) -> Features {
    let (mut all, own) = workspace_features(manifest);
    let name = package
        .map(str::to_owned)
        .or(own)
        .unwrap_or_else(|| panic!("{} names no crate of its own", manifest.display()));
    all.remove(&name)
        .unwrap_or_else(|| panic!("no crate {name} in {}", manifest.display()))
}

/// The feature selections the gate builds and lints a crate with, each as cargo arguments, as
/// cargo-hack's `--each-feature` runs them: default features; when there is a `default` feature,
/// none; each other feature on its own; and every feature. A crate with no features has one.
pub fn feature_selections(features: &Features) -> Vec<Vec<String>> {
    let mut selections = vec![Vec::new()];
    if features.is_empty() {
        return selections;
    }
    if features.contains_key("default") {
        selections.push(vec!["--no-default-features".to_owned()]);
    }
    for feature in features.keys().filter(|f| *f != "default") {
        selections.push(vec![
            "--no-default-features".to_owned(),
            "--features".to_owned(),
            feature.clone(),
        ]);
    }
    selections.push(vec!["--all-features".to_owned()]);
    selections
}

/// One output for several runs: the first failure's status (or the last run's), and every run's
/// output in order, each run's error output after a line naming it.
fn combine(runs: Vec<(String, Output)>) -> Output {
    let status = runs
        .iter()
        .map(|(_, o)| o.status)
        .find(|s| !s.success())
        .or_else(|| runs.last().map(|(_, o)| o.status))
        .expect("at least one run");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    for (shown, output) in runs {
        stdout.extend(output.stdout);
        stderr.extend(format!("== cargo {shown} ({})\n", output.status).into_bytes());
        stderr.extend(output.stderr);
    }
    Output {
        status,
        stdout,
        stderr,
    }
}

/// The gate's build: `package` (or the manifest's own crate) for `target` (or the host), with
/// warnings denied, once for each of the crate's feature selections; `locked` for the workspace,
/// whose `Cargo.lock` is committed. Artifact messages are written to standard output as JSON.
pub fn build(
    manifest: &Path,
    package: Option<&str>,
    target: Option<&str>,
    locked: bool,
    target_dir: &Path,
) -> Output {
    let selections = feature_selections(&features_of(manifest, package));
    build_with(manifest, package, target, locked, target_dir, &selections)
}

/// `build`, with the feature selections given.
pub fn build_with(
    manifest: &Path,
    package: Option<&str>,
    target: Option<&str>,
    locked: bool,
    target_dir: &Path,
    selections: &[Vec<String>],
) -> Output {
    let mut runs = Vec::new();
    for selection in selections {
        let mut args = vec!["build", "--message-format", "json-render-diagnostics"];
        if locked {
            args.push("--locked");
        }
        if let Some(package) = package {
            args.extend(["--package", package]);
        }
        if let Some(target) = target {
            args.extend(["--target", target]);
        }
        args.extend(selection.iter().map(String::as_str));
        runs.push((args.join(" "), cargo(&args, manifest, target_dir)));
    }
    combine(runs)
}

/// The gate's clippy run over `packages` (none: the manifest's own crate), JSON messages on, once
/// for each feature selection: default features, every feature, none (when a crate has a `default`
/// feature) and each crate's each feature on its own.
pub fn clippy(manifest: &Path, packages: &[&str], locked: bool, target_dir: &Path) -> Output {
    let mut selections = BTreeSet::new();
    selections.insert(Vec::new());
    let (mut all, own) = workspace_features(manifest);
    let crates: Vec<(Option<&str>, Features)> = if packages.is_empty() {
        let own = own.unwrap_or_else(|| panic!("{} names no crate", manifest.display()));
        vec![(None, all.remove(&own).unwrap_or_default())]
    } else {
        packages
            .iter()
            .map(|p| {
                let features = all
                    .remove(*p)
                    .unwrap_or_else(|| panic!("no crate {p} in {}", manifest.display()));
                (Some(*p), features)
            })
            .collect()
    };
    for (package, features) in crates {
        for selection in feature_selections(&features) {
            // With several packages, a feature is named with its crate.
            let selection = match (package, selection.as_slice()) {
                (Some(p), [no_default, flag, feature]) => {
                    vec![no_default.clone(), flag.clone(), format!("{p}/{feature}")]
                }
                _ => selection,
            };
            selections.insert(selection);
        }
    }
    let mut runs = Vec::new();
    for selection in &selections {
        let mut args = vec!["clippy", "--message-format", "json"];
        if locked {
            args.push("--locked");
        }
        for package in packages {
            args.extend(["--package", package]);
        }
        args.extend(selection.iter().map(String::as_str));
        args.extend(CLIPPY_ARGS);
        runs.push((args.join(" "), cargo(&args, manifest, target_dir)));
    }
    combine(runs)
}

/// The text of a run's output, for a message.
fn shown(output: &Output) -> String {
    format!(
        "{}\n{}{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The JSON objects of a run's standard output.
fn json_lines(output: &Output) -> Vec<Json> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| l.starts_with('{'))
        .map(|line| json::parse(line).unwrap_or_else(|e| panic!("cargo wrote {line}: {e:?}")))
        .collect()
}

/// One diagnostic of a clippy run on a probe: its lint code (or `None`), level, file and line.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Message {
    /// `clippy::disallowed_methods` and so on; `None` for a message without a code.
    pub code: Option<String>,
    /// `error` or `warning`.
    pub level: String,
    /// The file of its primary span.
    pub file: String,
    /// The line of its primary span.
    pub line: usize,
}

/// Every diagnostic with a span that a run with `--message-format json` gave.
pub fn messages(output: &Output) -> Vec<Message> {
    let mut found = Vec::new();
    for value in json_lines(output) {
        if value.get("reason").and_then(Json::as_str) != Some("compiler-message") {
            continue;
        }
        let Some(message) = value.get("message") else {
            continue;
        };
        let spans = message
            .get("spans")
            .and_then(Json::as_array)
            .unwrap_or_default();
        let Some(span) = spans
            .iter()
            .find(|s| s.get("is_primary").and_then(Json::as_bool) == Some(true))
        else {
            continue;
        };
        found.push(Message {
            code: message
                .get("code")
                .and_then(|c| c.get("code"))
                .and_then(Json::as_str)
                .map(str::to_owned),
            level: message
                .get("level")
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_owned(),
            file: span
                .get("file_name")
                .and_then(Json::as_str)
                .unwrap_or_default()
                .replace('\\', "/"),
            line: span
                .get("line_start")
                .and_then(|n| match n {
                    Json::Number(text) => text.parse().ok(),
                    _ => None,
                })
                .unwrap_or_default(),
        });
    }
    found.sort();
    found
}

/// The words clippy uses when a disallowed path does not resolve: "`<path>` does not refer to a
/// reachable function" (or "type").
pub const LAPSED: &str = "does not refer to a reachable";

/// Every message of a run that says a disallowed path does not resolve, from its JSON messages,
/// with or without a span, and from its other output, sorted and once each.
pub fn lapsed_bans(output: &Output) -> Vec<String> {
    let mut found = BTreeSet::new();
    for value in json_lines(output) {
        let text = value
            .get("message")
            .and_then(|m| m.get("message"))
            .and_then(Json::as_str)
            .unwrap_or_default();
        if text.contains(LAPSED) {
            found.insert(text.to_owned());
        }
    }
    for line in String::from_utf8_lossy(&output.stderr).lines() {
        if line.contains(LAPSED) {
            found.insert(line.trim().to_owned());
        }
    }
    found.into_iter().collect()
}

/// The files, by crate and with absolute paths, that a run's compiler read to build the libraries
/// of the crates named in `crates` (every library unit, its test build included); `root` is the
/// folder cargo ran rustc in for them, its workspace root, against which rustc writes the paths of
/// a workspace crate's own files. Also every way the list could not be made, in words: a library
/// whose dep-info file is missing or unreadable.
pub fn compiled_sources(
    output: &Output,
    root: &Path,
    crates: &BTreeSet<&str>,
) -> (BTreeMap<String, BTreeSet<PathBuf>>, Vec<String>) {
    let mut found: BTreeMap<String, BTreeSet<PathBuf>> = BTreeMap::new();
    let mut problems = Vec::new();
    for value in json_lines(output) {
        if value.get("reason").and_then(Json::as_str) != Some("compiler-artifact") {
            continue;
        }
        let Some(package) = value
            .get("package_id")
            .and_then(Json::as_str)
            .map(package_name)
        else {
            continue;
        };
        if !crates.contains(package.as_str()) {
            continue;
        }
        let target = value.get("target");
        let is_lib = target
            .and_then(|t| t.get("kind"))
            .and_then(Json::as_array)
            .is_some_and(|kinds| kinds.iter().any(|k| k.as_str() == Some("lib")));
        if !is_lib {
            continue;
        }
        let name = target
            .and_then(|t| t.get("name"))
            .and_then(Json::as_str)
            .unwrap_or_default();
        let filenames = value
            .get("filenames")
            .and_then(Json::as_array)
            .unwrap_or_default();
        match dep_info_of(filenames, name) {
            Some(dep_info) => match fs::read_to_string(&dep_info) {
                Ok(text) => {
                    found
                        .entry(package.clone())
                        .or_default()
                        .extend(dep_info_files(&text, root));
                }
                Err(e) => problems.push(format!(
                    "{package}: cannot read the compiler's dep-info {}: {e}",
                    dep_info.display()
                )),
            },
            None => problems.push(format!(
                "{package}: no dep-info beside {filenames:?}, so the files the compiler read for \
                 its library cannot be listed"
            )),
        }
    }
    for krate in crates {
        if !found.contains_key(*krate) && problems.iter().all(|p| !p.starts_with(*krate)) {
            problems.push(format!(
                "{krate}: the run built no library of it, so the files the compiler read for it \
                 cannot be listed"
            ));
        }
    }
    (found, problems)
}

/// A crate's name from cargo's package id (`path+file:///.../geom-num#0.0.0`, or
/// `...#name@version` when the name differs from the folder's).
fn package_name(id: &str) -> String {
    let (url, fragment) = id.split_once('#').unwrap_or((id, ""));
    match fragment.split_once('@') {
        Some((name, _)) => name.to_owned(),
        None => url.rsplit('/').next().unwrap_or_default().to_owned(),
    }
}

/// rustc's dep-info file for a library unit: beside its artifact in `deps`, named `<name>-<hash>.d`
/// where the artifact is `lib<name>-<hash>.rlib` or `.rmeta`.
fn dep_info_of(filenames: &[Json], name: &str) -> Option<PathBuf> {
    filenames.iter().filter_map(Json::as_str).find_map(|file| {
        let path = Path::new(file);
        let folder = path.parent()?;
        if folder.file_name()? != "deps" {
            return None;
        }
        let stem = path.file_stem()?.to_str()?;
        let at = stem.find(&format!("{name}-"))?;
        let dep_info = folder.join(format!("{}.d", &stem[at..]));
        dep_info.is_file().then_some(dep_info)
    })
}

/// The files a rustc dep-info file lists: every rule with no prerequisites (rustc writes one for
/// each file it read), its escaped spaces undone, each path joined to `root` when relative and
/// with `.` and `..` resolved by name.
pub fn dep_info_files(text: &str, root: &Path) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    for line in text.lines().map(str::trim_end) {
        if line.starts_with('#') {
            continue;
        }
        let Some(file) = line.strip_suffix(':') else {
            continue;
        };
        if file.is_empty() || file.ends_with(':') {
            continue;
        }
        let file = PathBuf::from(file.replace("\\ ", " "));
        let joined = if file.is_absolute() {
            file
        } else {
            root.join(file)
        };
        let mut clean = PathBuf::new();
        for part in joined.components() {
            match part {
                Component::CurDir => {}
                Component::ParentDir => {
                    clean.pop();
                }
                other => clean.push(other),
            }
        }
        files.insert(clean);
    }
    files
}

/// A probe crate: `name` under the test's temporary folder, with its own `[workspace]` so the
/// repository's workspace does not claim it, `manifest_tail` after its package table, `lib` as
/// its library and `clippy_toml` beside it if given. Returns its manifest.
pub fn probe(name: &str, manifest_tail: &str, lib: &str, clippy_toml: Option<&str>) -> PathBuf {
    let dir = scratch("gate-probes").join(name);
    let src = dir.join("src");
    fs::create_dir_all(&src).unwrap();
    let manifest = dir.join("Cargo.toml");
    fs::write(
        &manifest,
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
             publish = false\n\n[workspace]\n\n{manifest_tail}"
        ),
    )
    .unwrap();
    fs::write(src.join("lib.rs"), lib).unwrap();
    let config = dir.join("clippy.toml");
    match clippy_toml {
        Some(text) => fs::write(&config, text).unwrap(),
        None if config.exists() => fs::remove_file(&config).unwrap(),
        None => {}
    }
    manifest
}

/// What the files a run's compiler read are, by crate: the crates asked for and every file of
/// each, as absolute paths.
pub type Compiled = BTreeMap<String, BTreeSet<PathBuf>>;

/// The gate's builds over the repository: every `no_std` kernel crate of the table in the
/// workspace, each on its own, for each of `BUILD_TARGETS`, with each feature selection. Returns
/// every failed build, shown, and the files the compiler read for each crate, with every way that
/// list could not be made.
pub fn every_kernel_crate_builds(table: &Table<'_>) -> (Vec<String>, Compiled, Vec<String>) {
    let root = repo::workspace_root();
    let observed = repo::observe();
    let target_dir = scratch("gate-build-target");
    let crates: Vec<&str> = table
        .rows
        .iter()
        .filter(|row| row.kind == Kind::Kernel && observed.packages.contains_key(row.name))
        .map(|row| row.name)
        .collect();
    assert!(
        !crates.is_empty(),
        "no kernel crate is in the workspace: the gate built nothing"
    );
    let mut failures = Vec::new();
    let mut compiled = Compiled::new();
    let mut problems = Vec::new();
    for target in BUILD_TARGETS {
        for krate in &crates {
            let selections = feature_selections(&observed.packages[*krate].features);
            let output = build_with(
                &root.join("Cargo.toml"),
                Some(krate),
                Some(target),
                true,
                &target_dir,
                &selections,
            );
            let ok = output.status.success();
            println!(
                "{} {krate} for {target}, {} feature selection(s)",
                if ok { "ok  " } else { "FAIL" },
                selections.len()
            );
            if ok {
                let (files, missing) = compiled_sources(&output, &root, &BTreeSet::from([*krate]));
                for (name, files) in files {
                    compiled.entry(name).or_default().extend(files);
                }
                problems.extend(missing);
            } else {
                failures.push(format!("{krate} for {target}: {}", shown(&output)));
            }
        }
    }
    (failures, compiled, problems)
}

/// The crates of the table that ship and are in the workspace.
fn crates_that_ship<'t>(table: &'t Table<'t>) -> Vec<&'t str> {
    let observed = repo::observe();
    table
        .rows
        .iter()
        .filter(|row| carries_the_posture(row.kind) && observed.packages.contains_key(row.name))
        .map(|row| row.name)
        .collect()
}

/// The gate's clippy run over the repository: every crate of the table that ships and is in the
/// workspace, all targets, warnings denied, each feature selection.
pub fn every_crate_that_ships_passes_clippy(table: &Table<'_>) -> Output {
    let root = repo::workspace_root();
    let crates = crates_that_ship(table);
    assert!(
        !crates.is_empty(),
        "no crate that ships is in the workspace: clippy checked nothing"
    );
    clippy(
        &root.join("Cargo.toml"),
        &crates,
        true,
        &scratch("gate-clippy-target"),
    )
}

/// The lint settings of the crate whose settings every crate that ships copies (the table's
/// `lints_from`), as written: its manifest from its first `[lints` table on, and its
/// `clippy.toml`.
pub fn reference_posture(table: &Table<'_>) -> (String, String) {
    let root = repo::workspace_root();
    let observed = repo::observe();
    let name = full_name(table.lints_from);
    let dir = observed
        .packages
        .get(&name)
        .unwrap_or_else(|| panic!("{name} is not in the workspace"))
        .dir
        .iter()
        .fold(root, |p, part| p.join(part));
    let manifest = fs::read_to_string(dir.join("Cargo.toml"))
        .unwrap()
        .replace("\r\n", "\n");
    let at = manifest
        .find("\n[lints")
        .unwrap_or_else(|| panic!("{name}'s Cargo.toml has no [lints] table"));
    let clippy_toml = fs::read_to_string(dir.join("clippy.toml")).unwrap();
    (manifest.split_at(at + 1).1.to_owned(), clippy_toml)
}

/// One line of the clippy probe: its source, the lints it must fire (none: it keeps every rule),
/// and the rule it shows works: a disallowed path, or a denied lint's name.
pub struct ProbeLine {
    /// The function, on one line.
    pub source: String,
    /// The lint codes clippy must give on that line, and no other.
    pub lints: &'static [&'static str],
    /// The disallowed path or denied lint the line breaks; `None` for a clean line.
    pub covers: Option<String>,
}

/// The code of `disallowed_methods`.
const METHOD: &[&str] = &["clippy::disallowed_methods"];

/// The code of `disallowed_types`.
const TYPE: &[&str] = &["clippy::disallowed_types"];

/// The probe's lines that call the reductions and std's clocks and identity, each with the path
/// it calls: (source, path, lints).
const CALLS: [(&str, &str, &[&str]); 12] = [
    (
        "fn r1(v: &[u64]) -> u64 { v.iter().sum() }",
        REDUCTIONS[0],
        METHOD,
    ),
    (
        "fn r2(v: &[u64]) -> u64 { v.iter().product() }",
        REDUCTIONS[1],
        METHOD,
    ),
    (
        "fn r3(v: &[f64]) -> f64 { <f64 as core::iter::Sum<&f64>>::sum(v.iter()) }",
        REDUCTIONS[2],
        METHOD,
    ),
    (
        "fn r4(v: &[u64]) -> u64 { <u64 as core::iter::Product<&u64>>::product(v.iter()) }",
        REDUCTIONS[3],
        METHOD,
    ),
    (
        "fn k1() -> std::time::SystemTime { std::time::SystemTime::now() }",
        CLOCKS_AND_IDENTITY[0],
        METHOD,
    ),
    (
        "fn k2(t: std::time::SystemTime) -> bool { t.elapsed().is_ok() }",
        CLOCKS_AND_IDENTITY[1],
        METHOD,
    ),
    (
        "fn k3() -> std::time::Instant { std::time::Instant::now() }",
        CLOCKS_AND_IDENTITY[2],
        METHOD,
    ),
    (
        "fn k4(t: std::time::Instant) -> std::time::Duration { t.elapsed() }",
        CLOCKS_AND_IDENTITY[3],
        METHOD,
    ),
    (
        "fn k5() -> u32 { std::process::id() }",
        CLOCKS_AND_IDENTITY[4],
        METHOD,
    ),
    (
        "fn k6() -> bool { std::thread::current().name().is_some() }",
        CLOCKS_AND_IDENTITY[5],
        METHOD,
    ),
    (
        "fn k7(t: &std::thread::Thread) -> String { format!(\"{:?}\", t.id()) }",
        CLOCKS_AND_IDENTITY[6],
        METHOD,
    ),
    (
        "fn k8() -> usize { core::mem::size_of_val(&std::hash::RandomState::new()) }",
        CLOCKS_AND_IDENTITY[7],
        &["clippy::disallowed_methods", "clippy::disallowed_types"],
    ),
];

/// The probe's lines that name each disallowed type: (source, path, lints). Constructing a
/// `HashMap` or a `HashSet` with `new` breaks two bans at once, since geom-num's configuration
/// bans both the types and their constructors that seed from entropy (G32), so those two lines
/// expect both lints.
const TYPES: [(&str, &str, &[&str]); 4] = [
    (
        "fn t1() -> usize { std::collections::HashMap::<u8, u8>::new().len() }",
        REQUIRED_TYPES[0],
        &["clippy::disallowed_methods", "clippy::disallowed_types"],
    ),
    (
        "fn t2() -> usize { std::collections::HashSet::<u8>::new().len() }",
        REQUIRED_TYPES[1],
        &["clippy::disallowed_methods", "clippy::disallowed_types"],
    ),
    (
        "fn t3() -> usize { core::mem::size_of::<std::hash::RandomState>() }",
        REQUIRED_TYPES[2],
        TYPE,
    ),
    (
        "fn t4() -> usize { core::mem::size_of::<std::thread::ThreadId>() }",
        REQUIRED_TYPES[3],
        TYPE,
    ),
];

/// The probe's lines that break each of the denials: (source, lint).
const DENIALS: [(&str, &str); 7] = [
    ("fn d1(x: Option<u8>) -> u8 { x.unwrap() }", "unwrap_used"),
    (
        "fn d2(x: Option<u8>) -> u8 { x.expect(\"x\") }",
        "expect_used",
    ),
    ("fn d3() { panic!(\"x\") }", "panic"),
    ("fn d4(v: &[u8]) -> u8 { v[0] }", "indexing_slicing"),
    ("fn d5() -> u8 { todo!() }", "todo"),
    ("fn d6() -> u8 { unimplemented!() }", "unimplemented"),
    ("fn d7() -> u8 { unreachable!() }", "unreachable"),
];

/// The clippy probe's lines: one call of each method chapter 21 disallows, one use of each type,
/// one break of each denial, one behind a feature (`FEATURE_TAIL` declares it), and lines that
/// keep every rule.
pub fn probe_lines() -> Vec<ProbeLine> {
    let mut lines = Vec::new();
    let mut n = 0;
    for width in ["f64", "f32"] {
        for name in ["max", "min"]
            .iter()
            .chain(ALGEBRAIC.iter())
            .chain(PLATFORM_MATHS.iter())
        {
            n += 1;
            let binary = matches!(*name, "max" | "min" | "atan2" | "log" | "powf" | "hypot")
                || name.starts_with("algebraic_");
            let source = match *name {
                _ if binary => {
                    format!("fn m{n}(x: {width}, y: {width}) -> {width} {{ x.{name}(y) }}")
                }
                "powi" => format!("fn m{n}(x: {width}) -> {width} {{ x.powi(2) }}"),
                "sin_cos" => format!("fn m{n}(x: {width}) -> {width} {{ x.sin_cos().0 }}"),
                _ => format!("fn m{n}(x: {width}) -> {width} {{ x.{name}() }}"),
            };
            lines.push(ProbeLine {
                source,
                lints: METHOD,
                covers: Some(format!("{width}::{name}")),
            });
        }
    }
    for (source, path, lints) in CALLS {
        lines.push(ProbeLine {
            source: source.to_owned(),
            lints,
            covers: Some(path.to_owned()),
        });
    }
    for (source, path, lints) in TYPES {
        lines.push(ProbeLine {
            source: source.to_owned(),
            lints,
            covers: Some(path.to_owned()),
        });
    }
    for (source, lint) in DENIALS {
        lines.push(ProbeLine {
            source: source.to_owned(),
            lints: denial(lint),
            covers: Some(format!("clippy::{lint}")),
        });
    }
    // Behind a feature no default switches on: linted only because the gate lints every feature.
    lines.push(ProbeLine {
        source: "#[cfg(feature = \"extra\")] fn e1(x: Option<u8>) -> u8 { x.unwrap() }".to_owned(),
        lints: denial("unwrap_used"),
        covers: None,
    });
    // Lines that keep every rule: IEEE 754's exact operations, which chapter 21 says Rust keeps on
    // every target, an ordered map, and checked access.
    for clean in [
        "fn c1(x: f64) -> f64 { x.sqrt() }",
        "fn c2(x: f64) -> f64 { x.mul_add(x, x) }",
        "fn c3(x: f64, y: f64) -> f64 { x.abs().copysign(y).floor() }",
        "fn c4(x: f64, y: f64) -> f64 { x.rem_euclid(y) }",
        "fn c5() -> usize { std::collections::BTreeMap::<u8, u8>::new().len() }",
        "fn c6(v: &[u8]) -> Option<&u8> { v.first() }",
        "fn c7(x: Option<u8>) -> u8 { x.unwrap_or(0) }",
    ] {
        lines.push(ProbeLine {
            source: clean.to_owned(),
            lints: &[],
            covers: None,
        });
    }
    lines
}

/// The lint code of a denial, as a one-item list.
fn denial(lint: &str) -> &'static [&'static str] {
    match lint {
        "unwrap_used" => &["clippy::unwrap_used"],
        "expect_used" => &["clippy::expect_used"],
        "panic" => &["clippy::panic"],
        "indexing_slicing" => &["clippy::indexing_slicing"],
        "todo" => &["clippy::todo"],
        "unimplemented" => &["clippy::unimplemented"],
        "unreachable" => &["clippy::unreachable"],
        other => panic!("no probe line for clippy::{other}"),
    }
}

/// The manifest lines a probe needs for its feature-gated line: a feature off by default.
pub const FEATURE_TAIL: &str = "[features]\nextra = []\n\n";

/// The probe's library from `lines`: a header, then one line each, from line `FIRST_LINE` on.
pub fn probe_source(lines: &[ProbeLine]) -> String {
    let mut text = String::from(
        "//! Probe of the merge gate: each line below breaks one kernel lint, or keeps them all.\n\
         #![allow(dead_code)]\n",
    );
    for line in lines {
        text.push_str(&line.source);
        text.push('\n');
    }
    text
}

/// The line of the probe's library that holds the first probe line.
pub const FIRST_LINE: usize = 3;

/// The four runs of the gate's workflow, by job: the job, its exact `run:` line, and the filter of
/// ignored tests it selects (none for the rules job, which runs every test not ignored).
pub const RUNS: [(&str, &str, Option<&str>); 4] = [
    (
        "rules",
        "run: cargo test --locked --package engine-geometry --test scaffold",
        None,
    ),
    (
        "deny",
        "run: cargo test --locked --package engine-geometry --test scaffold -- --ignored gate_deny --nocapture --test-threads 1",
        Some("gate_deny"),
    ),
    (
        "clippy",
        "run: cargo test --locked --package engine-geometry --test scaffold -- --ignored gate_clippy --nocapture --test-threads 1",
        Some("gate_clippy"),
    ),
    (
        "builds",
        "run: cargo test --locked --package engine-geometry --test scaffold -- --ignored gate_build --nocapture --test-threads 1",
        Some("gate_build"),
    ),
];

/// What `.github/workflows/kernel-gate.yml` gets wrong for the gate, given its text and
/// `kernel.yml`'s (both with `\n` line ends), in words; empty when nothing.
pub fn workflow(yml: &str, kernel_yml: &str) -> Vec<String> {
    let lines: Vec<&str> = yml.lines().collect();
    let mut wrong = Vec::new();
    workflow_triggers_and_pins(yml, &lines, kernel_yml, &mut wrong);
    workflow_cargo_deny(yml, &lines, &mut wrong);
    workflow_runs(&lines, &mut wrong);
    workflow_nothing_swallowed(&lines, &mut wrong);
    workflow_gate_job(&lines, &mut wrong);
    wrong
}

/// The lines of `lines` that are not blank and not YAML comments, trimmed.
fn code(lines: &[&str]) -> Vec<String> {
    lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// The top-level block of `lines` that starts at `key:`, its lines trimmed, blank and comment
/// lines left out.
fn block(lines: &[&str], key: &str) -> Vec<String> {
    let Some(start) = lines.iter().position(|l| *l == format!("{key}:")) else {
        return Vec::new();
    };
    let body: Vec<&str> = lines[start + 1..]
        .iter()
        .take_while(|l| l.is_empty() || l.starts_with(' ') || l.starts_with('#'))
        .copied()
        .collect();
    code(&body)
}

/// The lines of the job `id` under `jobs:`, raw, up to the next job or the end.
fn job_lines<'a>(lines: &[&'a str], id: &str) -> Vec<&'a str> {
    let header = format!("  {id}:");
    lines
        .iter()
        .skip_while(|l| **l != "jobs:")
        .skip_while(|l| **l != header)
        .skip(1)
        .take_while(|l| !is_job_header(l))
        .copied()
        .collect()
}

/// Whether a line opens a job under `jobs:`.
fn is_job_header(line: &str) -> bool {
    line.starts_with("  ") && !line.starts_with("   ") && line.ends_with(':')
}

/// Whether `text` is a lowercase hexadecimal string of `length` digits.
fn lower_hex(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// Triggers: every pull request, every push to main and every merge-queue check, with no filter,
/// so the gate always reports and a required check never waits on a run that did not start.
/// Permissions: read only. Actions: only kernel.yml's, each pinned to kernel.yml's full commit, and
/// no checkout keeps its token.
fn workflow_triggers_and_pins(
    yml: &str,
    lines: &[&str],
    kernel_yml: &str,
    wrong: &mut Vec<String>,
) {
    let on = block(lines, "on");
    if on != ["pull_request:", "push:", "branches: [main]", "merge_group:"] {
        wrong.push(format!(
            "its triggers are {on:?}; the gate runs on every pull request, every push to main and \
             every merge-queue check, with no path or branch filter"
        ));
    }
    if block(lines, "permissions") != ["contents: read"] || yml.contains(": write") {
        wrong.push("it may do more than read the repository".to_owned());
    }
    let pins = action_pins(yml);
    let kernel_pins = action_pins(kernel_yml);
    if pins.is_empty() {
        wrong.push("it uses no action".to_owned());
    }
    for (action, refs) in &pins {
        let full = refs.iter().all(|r| lower_hex(r, 40));
        if !full || refs.len() != 1 || kernel_pins.get(action) != Some(refs) {
            wrong.push(format!(
                "{action} is pinned to {refs:?}; kernel.yml pins it to {:?}: one full commit, \
                 kernel.yml's, is required",
                kernel_pins.get(action)
            ));
        }
    }
    let checkouts = yml.matches("uses: actions/checkout@").count();
    if checkouts == 0 || yml.matches("persist-credentials: false").count() != checkouts {
        wrong.push("a checkout keeps its credentials".to_owned());
    }
}

/// cargo-deny: one exact release from its project's own releases, checked against its SHA-256
/// before it runs.
fn workflow_cargo_deny(yml: &str, lines: &[&str], wrong: &mut Vec<String>) {
    let env_value = |key: &str| {
        lines
            .iter()
            .find_map(|l| l.trim().strip_prefix(&format!("{key}:")))
            .map(|v| v.trim().trim_matches('"').to_owned())
    };
    let version_ok = env_value("CARGO_DENY_VERSION").is_some_and(|v| {
        let parts: Vec<&str> = v.split('.').collect();
        parts.len() == 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    });
    let sha_ok = env_value("CARGO_DENY_SHA256").is_some_and(|v| lower_hex(&v, 64));
    if !version_ok || !sha_ok || !yml.contains("sha256sum --check --strict") {
        wrong.push(
            "cargo-deny is not installed as one exact release checked against its SHA-256"
                .to_owned(),
        );
    }
    if !yml.contains("https://github.com/EmbarkStudios/cargo-deny/releases/download/") {
        wrong.push("cargo-deny does not come from its project's own releases".to_owned());
    }
}

/// What it runs: each check job runs its one test command, written out exactly, and no other
/// line runs `cargo test`; the jobs are those four and the gate; the tools they need are
/// installed.
fn workflow_runs(lines: &[&str], wrong: &mut Vec<String>) {
    let expected: BTreeSet<&str> = RUNS.iter().map(|(_, run, _)| *run).collect();
    for (job, run, _) in RUNS {
        let found = code(&job_lines(lines, job))
            .iter()
            .filter(|l| l.as_str() == run)
            .count();
        if found != 1 {
            wrong.push(format!(
                "the {job} job does not run exactly `{run}` once (it does {found} times)"
            ));
        }
    }
    for line in code(lines) {
        if line.contains("cargo test") && !expected.contains(line.as_str()) {
            wrong.push(format!(
                "`{line}` runs tests in a form the gate does not know: each job's run is written \
                 out exactly, with nothing after it"
            ));
        }
    }
    for (needle, job) in [
        ("targets: wasm32v1-none, wasm32-unknown-unknown", "builds"),
        ("components: clippy", "clippy"),
    ] {
        if !code(&job_lines(lines, job)).iter().any(|l| l == needle) {
            wrong.push(format!("the {job} job does not install {needle:?}"));
        }
    }
    let jobs: BTreeSet<String> = lines
        .iter()
        .skip_while(|l| **l != "jobs:")
        .skip(1)
        .filter(|l| is_job_header(l))
        .map(|l| l.trim().trim_end_matches(':').to_owned())
        .collect();
    let want: BTreeSet<String> = RUNS
        .iter()
        .map(|(job, _, _)| (*job).to_owned())
        .chain(["gate".to_owned()])
        .collect();
    if jobs != want {
        wrong.push(format!("its jobs are {jobs:?}, not exactly {want:?}"));
    }
}

/// The workflow's lines of YAML structure, trimmed: not blank, not comments, and not inside a block
/// scalar (the lines of a `run: |` script, indented below the line that opens it).
fn structure_lines(lines: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    // The indentation of the line that opened the block scalar being read, if any.
    let mut scalar: Option<usize> = None;
    for line in lines {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();
        if let Some(open) = scalar {
            if trimmed.is_empty() || indent > open {
                continue;
            }
            scalar = None;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let value = trimmed.split_once(':').map_or("", |(_, v)| v.trim());
        if ["|", "|-", "|+", ">", ">-", ">+"].contains(&value) {
            scalar = Some(indent);
        }
        found.push(trimmed.to_owned());
    }
    found
}

/// The key a workflow line sets, without a sequence item's `- `; `None` for a line that sets no
/// key (a script line).
fn key_of(line: &str) -> Option<&str> {
    let line = line.trim().trim_start_matches("- ");
    let (key, _) = line.split_once(':')?;
    let plain = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    plain.then_some(key)
}

/// Nothing can make a check pass that did not run or failed: no `continue-on-error`, no `if:` but
/// the gate job's, every shell plain bash, no command whose failure is swallowed (`||`, `set +e`,
/// `exit 0`), no YAML form this line reader would misread (a quoted key, a flow mapping, an anchor,
/// an alias or a merge key), and no environment beyond the workflow's own pins and the gate job's
/// results.
fn workflow_nothing_swallowed(lines: &[&str], wrong: &mut Vec<String>) {
    let all = code(lines);
    let structure = structure_lines(lines);
    let keys: Vec<&str> = structure.iter().filter_map(|l| key_of(l)).collect();
    if keys.contains(&"continue-on-error") {
        wrong.push("a step or job continues on error, so its failure passes".to_owned());
    }
    let ifs = keys.iter().filter(|k| **k == "if").count();
    let gate_if = code(&job_lines(lines, "gate"))
        .iter()
        .filter(|l| key_of(l) == Some("if"))
        .count();
    if ifs != 1 || gate_if != 1 {
        wrong.push(format!(
            "it has {ifs} condition(s): only the gate job may have one, so no check is skipped"
        ));
    }
    for line in structure.iter().filter(|l| key_of(l) == Some("shell")) {
        if line.trim_start_matches("- ") != "shell: bash" {
            wrong.push(format!(
                "`{line}`: every step runs in plain bash, which stops at a failing command"
            ));
        }
    }
    for line in &all {
        if line.contains("||") || line.contains("set +") || line.contains("exit 0") {
            wrong.push(format!("`{line}`: a failure there would be swallowed"));
        }
    }
    for line in &structure {
        let body = line.trim_start_matches("- ");
        let misread = body.starts_with(['"', '\'', '{', '?'])
            || body.starts_with("<<")
            || line.contains(": &")
            || line.contains(": *")
            || line.contains(": !");
        if misread {
            wrong.push(format!(
                "`{line}`: a YAML form this check does not read: write plain block-style keys"
            ));
        }
    }
    // Every `env:` block and its keys, by indentation.
    let mut blocks: Vec<BTreeSet<String>> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.trim().trim_start_matches("- ") != "env:" {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let keys = lines[i + 1..]
            .iter()
            .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
            .take_while(|l| l.len() - l.trim_start().len() > indent)
            .filter(|l| l.len() - l.trim_start().len() == indent + 2)
            .filter_map(|l| l.trim().split_once(':').map(|(k, _)| k.to_owned()))
            .collect();
        blocks.push(keys);
    }
    let own: BTreeSet<String> = [
        "RUST_TOOLCHAIN",
        "CARGO_TERM_COLOR",
        "CARGO_DENY_VERSION",
        "CARGO_DENY_SHA256",
    ]
    .map(str::to_owned)
    .into();
    let results: BTreeSet<String> = ["RESULTS".to_owned()].into();
    if blocks != [own, results] {
        wrong.push(format!(
            "its environment is {blocks:?}: only the workflow's own pins and the gate job's \
             results, so nothing redirects what the tools read"
        ));
    }
}

/// The required check: the `gate` job, which needs every other job, runs even when one failed,
/// and passes only when each succeeded, exactly as written here.
fn workflow_gate_job(lines: &[&str], wrong: &mut Vec<String>) {
    let others: Vec<&str> = RUNS.iter().map(|(job, _, _)| *job).collect();
    let want = [
        "name: Merge gate".to_owned(),
        format!("needs: [{}]", others.join(", ")),
        "if: ${{ always() }}".to_owned(),
        "runs-on: ubuntu-latest".to_owned(),
        "timeout-minutes: 5".to_owned(),
        "steps:".to_owned(),
        "- name: Every job of the gate passed".to_owned(),
        "shell: bash".to_owned(),
        "env:".to_owned(),
        "RESULTS: ${{ join(needs.*.result, ' ') }}".to_owned(),
        "run: |".to_owned(),
        "set -euo pipefail".to_owned(),
        "echo \"results: $RESULTS\"".to_owned(),
        "test -n \"$RESULTS\"".to_owned(),
        "for result in $RESULTS; do".to_owned(),
        "if [ \"$result\" != \"success\" ]; then".to_owned(),
        "echo \"::error::a job of the kernel's merge gate did not pass ($result)\"".to_owned(),
        "exit 1".to_owned(),
        "fi".to_owned(),
        "done".to_owned(),
    ];
    let got = code(&job_lines(lines, "gate"));
    if got != want {
        wrong.push(format!(
            "the gate job is {got:#?}, not exactly the job that needs {others:?}, runs whatever \
             they gave and fails unless each succeeded"
        ));
    }
}

/// Every action a workflow uses, with the refs it pins it to.
fn action_pins(yml: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut pins: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in yml.lines() {
        let line = line.trim_start();
        let line = line.strip_prefix("- ").unwrap_or(line);
        let Some(spec) = line.strip_prefix("uses:") else {
            continue;
        };
        let spec = spec.split_whitespace().next().unwrap_or_default();
        let (action, pin) = spec.split_once('@').unwrap_or((spec, ""));
        pins.entry(action.to_owned())
            .or_default()
            .insert(pin.to_owned());
    }
    pins
}

/// What the test filters of `RUNS` select among `ignored`, the full names of the ignored tests of
/// the scaffold test: in words, every ignored test of the gate (its name's last part starts with
/// `gate_`) that no filter selects, and every filter that selects no test. libtest selects a test
/// when its full name contains the filter.
pub fn filters_select_every_gate_test(ignored: &[String]) -> Vec<String> {
    let mut wrong = Vec::new();
    let filters: Vec<&str> = RUNS.iter().filter_map(|(_, _, f)| *f).collect();
    for filter in &filters {
        if !ignored.iter().any(|t| t.contains(filter)) {
            wrong.push(format!("the filter {filter:?} selects no test"));
        }
    }
    for test in ignored {
        let last = test.rsplit("::").next().unwrap_or(test);
        if last.starts_with("gate_") && !filters.iter().any(|f| test.contains(f)) {
            wrong.push(format!("no job of the gate runs the ignored test {test}"));
        }
    }
    wrong
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{Rule, Violation};
    use crate::globals::{self, Sources};
    use crate::overrides;
    use crate::table::TABLE;

    /// A repository file's text, line ends normalised.
    fn read(relative: &str) -> String {
        fs::read_to_string(repo::workspace_root().join(relative))
            .unwrap_or_else(|e| panic!("cannot read {relative}: {e}"))
            .replace("\r\n", "\n")
    }

    // ------------------------------------------------------------------ the workflow

    #[test]
    fn gate_workflow_runs_pins_and_requires_the_gate() {
        let found: Vec<Violation> = workflow(
            &read(".github/workflows/kernel-gate.yml"),
            &read(".github/workflows/kernel.yml"),
        )
        .into_iter()
        .map(|detail| Violation {
            krate: "kernel-gate.yml".to_owned(),
            rule: Rule::GateWorkflow,
            detail,
        })
        .collect();
        assert!(found.is_empty(), "{}", crate::report(&found));
    }

    #[test]
    fn gate_workflow_filters_select_every_gate_test() {
        // This test binary's ignored tests, as libtest lists them ("name: test").
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--list", "--ignored"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", shown(&output));
        let ignored: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|l| l.strip_suffix(": test"))
            .map(str::to_owned)
            .collect();
        assert!(
            ignored.iter().filter(|t| t.contains("gate_")).count() >= 3,
            "{ignored:?}"
        );
        let wrong = filters_select_every_gate_test(&ignored);
        assert!(wrong.is_empty(), "{wrong:#?}");
        // Seeded: a gate test no filter selects, and a filter that selects nothing.
        let mut seeded = ignored.clone();
        seeded.push("deny::gate::gate_licences_of_a_new_kind".to_owned());
        assert_eq!(filters_select_every_gate_test(&seeded).len(), 1);
        let only_deny: Vec<String> = ignored
            .iter()
            .filter(|t| t.contains("gate_deny"))
            .cloned()
            .collect();
        assert_eq!(filters_select_every_gate_test(&only_deny).len(), 2);
    }

    /// Changes to kernel-gate.yml, each of which the check must refuse: (what, the changed text).
    fn workflow_faults(yml: &str) -> Vec<(&'static str, String)> {
        let first_sha = action_pins(yml)
            .values()
            .next()
            .and_then(|r| r.iter().next().cloned())
            .unwrap();
        let swap = |from: &str, to: &str| {
            assert!(yml.contains(from), "kernel-gate.yml lacks {from:?}");
            yml.replacen(from, to, 1)
        };
        let deny_step = "      - name: Licences, advisories, sources and bans of every crate, and \
                         each rule's seeded violations\n";
        vec![
            (
                "a path filter",
                swap(
                    "  pull_request:\n",
                    "  pull_request:\n    paths:\n      - \"projects/**\"\n",
                ),
            ),
            ("no merge queue", swap("  merge_group:\n", "")),
            (
                "a write permission",
                swap("contents: read", "contents: write"),
            ),
            ("an action by tag", swap(&format!("@{first_sha}"), "@v7")),
            (
                "another commit",
                swap(
                    &format!("@{first_sha}"),
                    "@0000000000000000000000000000000000000000",
                ),
            ),
            (
                "credentials kept",
                swap("persist-credentials: false", "persist-credentials: true"),
            ),
            (
                "cargo-deny unchecked",
                swap("sha256sum --check --strict", "true"),
            ),
            (
                "no deny runs",
                swap("--ignored gate_deny", "--ignored gate_nothing"),
            ),
            (
                "no builds",
                swap("--ignored gate_build", "--ignored gate_nothing"),
            ),
            (
                "no no_std target",
                swap(
                    "targets: wasm32v1-none, wasm32-unknown-unknown",
                    "targets: wasm32-unknown-unknown",
                ),
            ),
            (
                "a job left out of the gate",
                swap(
                    "needs: [rules, deny, clippy, builds]",
                    "needs: [rules, deny, clippy]",
                ),
            ),
            (
                "the gate skipped on failure",
                swap("if: ${{ always() }}", "if: ${{ success() }}"),
            ),
            (
                "a failure ignored",
                swap(
                    deny_step,
                    &format!("{deny_step}        continue-on-error: true\n"),
                ),
            ),
            (
                "a step skipped",
                swap(
                    deny_step,
                    &format!("{deny_step}        if: ${{{{ false }}}}\n"),
                ),
            ),
            (
                "a failure swallowed",
                swap(
                    "--test-threads 1\n\n  builds:",
                    "--test-threads 1 || true\n\n  builds:",
                ),
            ),
            (
                "a step that skips tests",
                swap(
                    "--ignored gate_clippy --nocapture",
                    "--ignored gate_clippy --skip gate_clippy --nocapture",
                ),
            ),
            (
                "the gate's results ignored",
                swap("          exit 1\n", "          exit 0\n"),
            ),
            (
                "a shell that ignores failures",
                swap("        shell: bash\n", "        shell: bash +e {0}\n"),
            ),
            (
                "the environment redirected",
                swap(
                    "  CARGO_TERM_COLOR: always\n",
                    "  CARGO_TERM_COLOR: always\n  CLIPPY_CONF_DIR: /tmp\n",
                ),
            ),
        ]
    }

    #[test]
    fn gate_workflow_fails_on_seeded_faults() {
        let yml = read(".github/workflows/kernel-gate.yml");
        let kernel = read(".github/workflows/kernel.yml");
        for (name, text) in &workflow_faults(&yml) {
            assert_ne!(text, &yml, "{name}: the seed changed nothing");
            assert!(
                !workflow(text, &kernel).is_empty(),
                "{name}: the fault passed"
            );
        }
    }

    // ------------------------------------------------------------------ dep-info

    #[test]
    fn dep_info_lists_every_file_the_compiler_read() {
        let root = Path::new("/w");
        // rustc's own form: the outputs' rules, then one rule per file read; a relative path for a
        // workspace file, an absolute one for a build script's output, an escaped space.
        let text = "/w/target/deps/x-1.d: src/lib.rs src/../tests/state.rs /o/out/gen.rs\n\n\
                    src/lib.rs:\nsrc/../tests/state.rs:\nsrc/my\\ file.in:\n/o/out/gen.rs:\n\n\
                    # env-dep:OUT_DIR=/o/out\n";
        let files: Vec<PathBuf> = dep_info_files(text, root).into_iter().collect();
        assert_eq!(
            files,
            [
                PathBuf::from("/o/out/gen.rs"),
                PathBuf::from("/w/src/lib.rs"),
                PathBuf::from("/w/src/my file.in"),
                PathBuf::from("/w/tests/state.rs"),
            ]
        );
        assert_eq!(package_name("path+file:///c/p/geom-num#0.0.0"), "geom-num");
        assert_eq!(
            package_name("path+file:///c/p/dir#attack-probe@0.1.0"),
            "attack-probe"
        );
    }

    #[test]
    fn feature_selections_are_cargo_hacks_each_feature() {
        assert_eq!(feature_selections(&BTreeMap::new()), [Vec::<String>::new()]);
        let features: BTreeMap<String, Vec<String>> = [
            ("default".to_owned(), vec!["a".to_owned()]),
            ("a".to_owned(), Vec::new()),
            ("threads".to_owned(), vec!["dep:geom-pool".to_owned()]),
        ]
        .into();
        let shown: Vec<String> = feature_selections(&features)
            .iter()
            .map(|s| s.join(" "))
            .collect();
        assert_eq!(
            shown,
            [
                "",
                "--no-default-features",
                "--no-default-features --features a",
                "--no-default-features --features threads",
                "--all-features",
            ]
        );
    }

    // ------------------------------------------------------------------ the builds

    /// The probe libraries, and on which targets each must build: (name, manifest tail, source,
    /// host, `no_std` target, browsers' target).
    const BUILD_PROBES: [(&str, &str, &str, bool, bool, bool); 5] = [
        (
            "gate-build-clean",
            "",
            "//! Probe.\n#![no_std]\n#![forbid(unsafe_code)]\n/// One.\npub fn one() -> u8 {\n    1\n}\n",
            true,
            true,
            true,
        ),
        // `std` exists for the browsers' target, so only wasm32v1-none proves no_std.
        (
            "gate-build-links-std",
            "",
            "//! Probe.\n#![no_std]\nextern crate std;\n/// Now.\npub fn now() -> std::time::Instant {\n    std::time::Instant::now()\n}\n",
            true,
            false,
            true,
        ),
        // A 64-bit assumption builds natively and fails on both 32-bit WebAssembly targets.
        (
            "gate-build-64-bit",
            "",
            "//! Probe.\n#![no_std]\nconst _: () = assert!(core::mem::size_of::<usize>() == 8);\n/// One.\npub fn one() -> u8 {\n    1\n}\n",
            true,
            false,
            false,
        ),
        // The same behind a feature no default switches on: built because every feature is.
        (
            "gate-build-feature-64-bit",
            FEATURE_TAIL,
            "//! Probe.\n#![no_std]\n#[cfg(feature = \"extra\")]\nconst _: () = assert!(core::mem::size_of::<usize>() == 8);\n/// One.\npub fn one() -> u8 {\n    1\n}\n",
            true,
            false,
            false,
        ),
        // Warnings are denied.
        (
            "gate-build-warning",
            "",
            "//! Probe.\n#![no_std]\n/// Unused.\npub fn f() {\n    let x = 1;\n}\n",
            false,
            false,
            false,
        ),
    ];

    #[test]
    #[ignore = "builds for wasm32v1-none and wasm32-unknown-unknown, which the merge gate installs; run with --ignored"]
    fn gate_build_every_kernel_crate_without_std_and_for_wasm32() {
        let (failures, compiled, problems) = every_kernel_crate_builds(&TABLE);
        assert!(failures.is_empty(), "{}", failures.join("\n\n"));
        assert!(problems.is_empty(), "{problems:#?}");
        let violations = source_rules_on(&compiled);
        assert!(violations.is_empty(), "{}", crate::report(&violations));
        let files = compiled
            .values()
            .fold(0usize, |count, files| count.saturating_add(files.len()));
        println!(
            "{} kernel crates: every file the compiler read ({files}) holds no global state and \
             lowers no denied lint",
            compiled.len()
        );
    }

    /// The source rules (`globals`, `overrides`) on the folder scan of the crates in `compiled`
    /// and every file the compiler read for them.
    fn source_rules_on(compiled: &Compiled) -> Vec<Violation> {
        let observed = repo::observe();
        let crates: BTreeSet<&str> = compiled.keys().map(String::as_str).collect();
        let mut sources: Sources = repo::crate_sources(&observed, &crates);
        globals::merge(&mut sources, repo::compiled_sources(compiled));
        let mut violations = globals::check(&TABLE, &sources);
        violations.extend(overrides::check(&TABLE, &sources));
        violations
    }

    #[test]
    #[ignore = "builds for wasm32v1-none and wasm32-unknown-unknown, which the merge gate installs; run with --ignored"]
    fn gate_build_fails_on_seeded_violations() {
        let target_dir = scratch("gate-probe-target");
        let mut wrong = Vec::new();
        for (name, tail, lib, host, no_std, wasm) in BUILD_PROBES {
            let manifest = probe(name, tail, lib, None);
            for (target, want) in [
                (None, host),
                (Some(NO_STD_TARGET), no_std),
                (Some(WASM_TARGET), wasm),
            ] {
                let output = build(&manifest, None, target, false, &target_dir);
                if output.status.success() != want {
                    wrong.push(format!(
                        "{name} for {}: expected {}, got {}",
                        target.unwrap_or("the host"),
                        if want { "a build" } else { "a failure" },
                        shown(&output)
                    ));
                }
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
    }

    #[test]
    #[ignore = "builds a probe crate; the merge gate runs it"]
    fn gate_build_reads_every_file_the_compiler_read() {
        // A library with three statics the folder scan cannot see (a module from tests/ by
        // #[path], a file pasted in by include!, and code its build script writes, the keyword
        // assembled from two strings) and an allow of a denied lint in the pasted file.
        let manifest = probe(
            "gate-build-compiled",
            "",
            "//! Probe.\n#![no_std]\n#[path = \"../tests/state.rs\"]\nmod state;\n\
             include!(\"counter.in\");\ninclude!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n\
             /// Reads the globals.\npub fn read() -> u32 {\n    \
             state::COUNT.load(core::sync::atomic::Ordering::Relaxed) + OTHER + GENERATED\n}\n",
            None,
        );
        let dir = manifest.parent().unwrap().to_path_buf();
        fs::create_dir_all(dir.join("tests")).unwrap();
        fs::write(
            dir.join("tests").join("state.rs"),
            "//! A counter.\npub static COUNT: core::sync::atomic::AtomicU32 = \
             core::sync::atomic::AtomicU32::new(0);\n",
        )
        .unwrap();
        fs::write(
            dir.join("src").join("counter.in"),
            "#[allow(clippy::unwrap_used)]\nstatic OTHER: u32 = 1;\n",
        )
        .unwrap();
        fs::write(
            dir.join("build.rs"),
            "//! Writes a global.\nfn main() {\n    let keyword = [\"sta\", \"tic\"].concat();\n    \
             let out = std::env::var(\"OUT_DIR\").unwrap();\n    \
             let text = format!(\"pub {keyword} GENERATED: u32 = 2;\\n\");\n    \
             std::fs::write(std::path::Path::new(&out).join(\"generated.rs\"), text).unwrap();\n}\n",
        )
        .unwrap();
        let output = build(&manifest, None, None, false, &scratch("gate-probe-target"));
        assert!(output.status.success(), "{}", shown(&output));
        let crates = BTreeSet::from(["gate-build-compiled"]);
        let (compiled, problems) = compiled_sources(&output, &dir, &crates);
        assert!(problems.is_empty(), "{problems:#?}");
        // Read as if it were geom-op, a kernel crate the rules cover.
        let files = compiled["gate-build-compiled"].clone();
        let sources: Sources =
            repo::compiled_sources(&BTreeMap::from([("geom-op".to_owned(), files)]));
        let statics = globals::check(&TABLE, &sources);
        let lowered = overrides::check(&TABLE, &sources);
        assert_eq!(statics.len(), 3, "{}", crate::report(&statics));
        assert_eq!(lowered.len(), 1, "{}", crate::report(&lowered));
        // A run that names a crate it did not build cannot list its files, and says so.
        let (_, problems) = compiled_sources(&output, &dir, &BTreeSet::from(["geom-elsewhere"]));
        assert_eq!(problems.len(), 1, "{problems:#?}");
    }

    // ------------------------------------------------------------------ clippy

    #[test]
    #[ignore = "runs clippy over every crate that ships, which takes minutes; the merge gate runs it"]
    fn gate_clippy_every_crate_that_ships() {
        let output = every_crate_that_ships_passes_clippy(&TABLE);
        let errors: Vec<Message> = messages(&output)
            .into_iter()
            .filter(|m| m.level == "error" || m.level == "warning")
            .collect();
        let lapsed = lapsed_bans(&output);
        assert!(
            output.status.success() && errors.is_empty() && lapsed.is_empty(),
            "clippy refused the crates that ship: {errors:#?}\nbans that lapsed: {lapsed:#?}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        // Every file the compiler read for them, held to the source rules.
        let root = repo::workspace_root();
        let crates: BTreeSet<&str> = crates_that_ship(&TABLE).into_iter().collect();
        let (compiled, problems) = compiled_sources(&output, &root, &crates);
        assert!(problems.is_empty(), "{problems:#?}");
        let violations = source_rules_on(&compiled);
        assert!(violations.is_empty(), "{}", crate::report(&violations));
    }

    #[test]
    #[ignore = "runs clippy on a probe crate; the merge gate runs it"]
    fn gate_clippy_fails_on_each_seeded_violation() {
        let (manifest_lints, clippy_toml) = reference_posture(&TABLE);
        let lines = probe_lines();
        // The clean lines alone: no diagnostic at all, so the configuration itself is sound.
        let clean: Vec<ProbeLine> = probe_lines()
            .into_iter()
            .filter(|l| l.lints.is_empty())
            .collect();
        let target_dir = scratch("gate-probe-target");
        let manifest = probe(
            "gate-clippy-clean",
            &manifest_lints,
            &probe_source(&clean),
            Some(&clippy_toml),
        );
        let output = clippy(&manifest, &[], false, &target_dir);
        let found = messages(&output);
        assert!(
            output.status.success() && found.is_empty() && lapsed_bans(&output).is_empty(),
            "clippy with geom-num's settings complains about lines that keep every rule: \
             {found:#?}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        // Every line: each seeded line gives exactly its own errors, every other line none.
        let manifest = probe(
            "gate-clippy-seeded",
            &format!("{FEATURE_TAIL}{manifest_lints}"),
            &probe_source(&lines),
            Some(&clippy_toml),
        );
        let output = clippy(&manifest, &[], false, &target_dir);
        let found = messages(&output);
        let mut got: BTreeMap<usize, BTreeSet<String>> = BTreeMap::new();
        let mut elsewhere = Vec::new();
        for m in &found {
            if m.file.ends_with("src/lib.rs") && m.level == "error" {
                got.entry(m.line)
                    .or_default()
                    .insert(m.code.clone().unwrap_or_else(|| "(no code)".to_owned()));
            } else {
                elsewhere.push(m.clone());
            }
        }
        let expected: BTreeMap<usize, BTreeSet<String>> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.lints.is_empty())
            .map(|(i, l)| {
                let codes = l.lints.iter().map(|c| (*c).to_owned()).collect();
                (FIRST_LINE + i, codes)
            })
            .collect();
        let mut wrong = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let n = FIRST_LINE + i;
            if got.get(&n) != expected.get(&n) {
                wrong.push(format!(
                    "line {n} `{}`: wanted {:?}, got {:?}",
                    line.source,
                    expected.get(&n),
                    got.get(&n)
                ));
            }
        }
        assert!(
            !output.status.success() && wrong.is_empty() && elsewhere.is_empty(),
            "clippy with geom-num's settings did not refuse exactly the seeded lines:\n{}\n\
             other diagnostics: {elsewhere:#?}",
            wrong.join("\n")
        );
        println!(
            "{} seeded lines refused, each with its own lints; {} clean lines passed",
            expected.len(),
            lines.len() - expected.len()
        );
    }

    #[test]
    fn the_probe_covers_every_rule() {
        // Every disallowed path, and every denied lint, has a line that breaks it.
        let covered: BTreeSet<String> =
            probe_lines().into_iter().filter_map(|l| l.covers).collect();
        let mut wanted: BTreeSet<String> = required_methods();
        wanted.extend(REQUIRED_TYPES.iter().map(|t| (*t).to_owned()));
        wanted.extend(
            crate::posture::DENIED_LINTS
                .iter()
                .filter(|l| !l.starts_with("disallowed_"))
                .map(|l| format!("clippy::{l}")),
        );
        assert_eq!(covered, wanted);
    }

    #[test]
    #[ignore = "runs clippy on a probe crate; the merge gate runs it"]
    fn gate_clippy_fails_on_a_ban_that_lapsed() {
        // geom-num's settings with one std path misspelt, as a toolchain that renamed it would
        // leave it: clippy only warns, and the gate must see it.
        let (manifest_lints, clippy_toml) = reference_posture(&TABLE);
        let from = "\"std::time::Instant::now\"";
        assert!(clippy_toml.contains(from));
        let lapsed_toml = clippy_toml.replacen(from, "\"std::time::Instant::now_renamed\"", 1);
        let clean: Vec<ProbeLine> = probe_lines()
            .into_iter()
            .filter(|l| l.lints.is_empty())
            .collect();
        let manifest = probe(
            "gate-clippy-lapsed",
            &manifest_lints,
            &probe_source(&clean),
            Some(&lapsed_toml),
        );
        let output = clippy(&manifest, &[], false, &scratch("gate-probe-target"));
        let lapsed = lapsed_bans(&output);
        assert!(
            lapsed.iter().any(|m| m.contains("now_renamed")),
            "the lapsed ban passed unseen: {}",
            shown(&output)
        );
    }
}
