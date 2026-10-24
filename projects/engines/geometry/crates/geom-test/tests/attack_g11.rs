//! Adversarial tests for piece G11, the development crates (`geom-test`, `geom-fuzz`, `geom-gen`,
//! `.github/workflows/kernel-dev.yml`).
//!
//! Each `attack_*` test fails if the piece breaks its board text ("no kernel crate has a normal
//! dependency on a development crate (checked in CI)", "every generator prints its seed", "CI
//! reruns every geom-gen generator"), the chapters it implements (27, 28) or a guarantee its own
//! module headers state. Every failure message gives the exact inputs, what was expected and what
//! happened. Tests without the prefix pass and record what was checked and held.
//!
//! Some attacks need things this crate cannot reach in-process, so they work the way a person
//! would: they build scratch Cargo workspaces under `CARGO_TARGET_TMPDIR` and run the real `cargo`
//! on them (`cargo metadata`, `cargo tree`, `cargo generate-lockfile`, all offline), run this test
//! binary as a child with a changed environment, or build a small probe program that links
//! `geom-gen` (in its own target folder under `CARGO_TARGET_TMPDIR`, so the first run compiles
//! `geom-gen`'s dependencies once). Nothing outside `CARGO_TARGET_TMPDIR` is written.

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

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use geom_num::grid::COORD_MAX;
use geom_test::Seed;
use geom_test::deps::{DEVELOPMENT_CRATES, cargo_metadata, check};
use geom_test::prop;
use proptest::prelude::{any, prop_assert};
use proptest::test_runner::{TestError, TestRunner};
use serde_json::Value;

// ---------------------------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------------------------

/// The cargo that built this test.
fn cargo() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsString::from(env!("CARGO")))
}

/// This crate's folder.
fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The folder holding every geometry crate.
fn crates_dir() -> PathBuf {
    crate_dir().parent().unwrap().to_path_buf()
}

/// The workspace root: five folders above this crate (projects/engines/geometry/crates/geom-test).
fn workspace_root() -> PathBuf {
    let root = crate_dir().ancestors().nth(5).unwrap().to_path_buf();
    assert!(root.join("Cargo.toml").is_file(), "{}", root.display());
    root
}

/// A scratch folder under `CARGO_TARGET_TMPDIR`, emptied first and removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Scratch {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("attack-g11-{label}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A path written so a TOML literal string can hold it on every platform.
fn toml_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Runs cargo with `args`; panics with its output if it fails.
fn run_cargo(args: &[&str], manifest: &Path) -> Output {
    let out = Command::new(cargo())
        .args(args)
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "fixture broken, not the piece: cargo {args:?} --manifest-path {} failed:\n{}{}",
        manifest.display(),
        text(&out.stdout),
        text(&out.stderr)
    );
    out
}

/// A minimal library package.
fn package(dir: &Path, name: &str, deps: &str) {
    write(
        &dir.join("Cargo.toml"),
        &format!(
            "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n{deps}"
        ),
    );
    write(&dir.join("src").join("lib.rs"), "//! fixture\n");
}

/// The kernel crate folder of a scratch workspace rooted at `root`.
fn kernel_dir(root: &Path, name: &str) -> PathBuf {
    root.join("projects")
        .join("engines")
        .join("geometry")
        .join("crates")
        .join(name)
}

// ---------------------------------------------------------------------------------------------
// 1. The dependency rule misses chains through crates that are not workspace members
// ---------------------------------------------------------------------------------------------

/// A scratch workspace laid out like ours: kernel crate `geom-k` and development crate `geom-test`
/// under projects/engines/geometry/crates. `geom-k` has a NORMAL path dependency on `shim`, and
/// `shim` has a NORMAL path dependency on `geom-test`. Where `shim` lives is the variant:
///
/// - "outside": next to the workspace folder, so it is not a member;
/// - "excluded": inside it at vendor/shim, listed in `[workspace] exclude`, so it is not a member.
///
/// Returns the workspace manifest.
fn non_member_chain_workspace(base: &Path, variant: &str) -> PathBuf {
    let root = base.join("root");
    let (shim_dir, shim_from_k, exclude) = match variant {
        "outside" => (base.join("shim"), "../../../../../../shim", ""),
        "excluded" => (
            root.join("vendor").join("shim"),
            "../../../../../vendor/shim",
            "exclude = [\"vendor/shim\"]\n",
        ),
        _ => unreachable!(),
    };
    write(
        &root.join("Cargo.toml"),
        &format!(
            "[workspace]\nresolver = \"3\"\nmembers = [\"projects/engines/geometry/crates/*\"]\n{exclude}"
        ),
    );
    package(
        &kernel_dir(&root, "geom-k"),
        "geom-k",
        &format!("[dependencies]\nshim = {{ path = '{shim_from_k}' }}\n"),
    );
    package(&kernel_dir(&root, "geom-test"), "geom-test", "");
    package(
        &shim_dir,
        "shim",
        &format!(
            "[dependencies]\ngeom-test = {{ path = '{}' }}\n",
            toml_path(&kernel_dir(&root, "geom-test"))
        ),
    );
    root.join("Cargo.toml")
}

/// The dependency rule is "no kernel crate has a normal dependency on a development crate", and
/// the check must not pass while it is broken. A kernel crate that reaches `geom-test` through a
/// normal dependency on a crate that is not a workspace member (a path crate outside the workspace
/// folder, or one listed in `exclude`; a git dependency behaves the same) breaks the rule: cargo
/// links `geom-test` into the kernel crate's normal build, as `cargo tree -e normal,build` shows.
/// `geom_test::deps::check` reads `cargo metadata --no-deps`, which lists members only, so it
/// never sees `shim`'s dependencies and reports nothing.
#[cfg_attr(
    target_family = "wasm",
    ignore = "runs cargo as a child process, which WebAssembly cannot"
)]
#[test]
fn attack_dependency_rule_follows_chains_through_non_member_crates() {
    let mut problems = Vec::new();
    for variant in ["outside", "excluded"] {
        let s = Scratch::new(&format!("nonmember-{variant}"));
        let manifest = non_member_chain_workspace(&s.0, variant);

        // Ground truth: what cargo actually builds for geom-k, normal and build edges only.
        let tree = run_cargo(
            &[
                "tree",
                "--offline",
                "-p",
                "geom-k",
                "-e",
                "normal,build",
                "--prefix",
                "none",
            ],
            &manifest,
        );
        let tree = text(&tree.stdout);
        assert!(
            tree.lines().any(|l| l.starts_with("geom-test v")),
            "fixture broken, not the piece: geom-k does not reach geom-test:\n{tree}"
        );

        let metadata = cargo_metadata(&cargo(), &manifest).unwrap();
        let report = check(&metadata).unwrap();
        assert!(
            report.kernel_crates.iter().any(|k| k == "geom-k"),
            "fixture broken, not the piece: geom-k is not a kernel crate: {report:?}"
        );
        if !report.violations.iter().any(|v| v.kernel_crate == "geom-k") {
            problems.push(format!(
                "variant {variant:?}: cargo tree -p geom-k -e normal,build gives\n{tree}\
                 so geom-k -(normal)-> shim -(normal)-> geom-test is in the kernel crate's normal \
                 build;\nexpected: check() reports a violation for geom-k\nactual: violations = \
                 {:?} (kernel crates checked: {:?})",
                report.violations, report.kernel_crates
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "the dependency rule passes while a kernel crate depends on geom-test:\n\n{}",
        problems.join("\n\n")
    );
}

// ---------------------------------------------------------------------------------------------
// 2. kernel-dev.yml does not run when a manifest the rule reads changes
// ---------------------------------------------------------------------------------------------

/// Evidence for the next attack: moving a development crate from `[dev-dependencies]` to
/// `[dependencies]` changes the manifest only. Cargo.lock lists a package's dependencies without
/// their kind, so it stays byte for byte the same, and a path filter on Cargo.lock does not fire.
/// The dependency rule itself catches the moved edge when it runs.
#[cfg_attr(
    target_family = "wasm",
    ignore = "runs cargo as a child process, which WebAssembly cannot"
)]
#[test]
fn moving_a_dev_dependency_to_dependencies_leaves_cargo_lock_unchanged() {
    let lock_with = |section: &str| -> (Vec<u8>, usize) {
        let s = Scratch::new(&format!("lock-{}", section.trim_matches(['[', ']'])));
        let root = s.0.join("root");
        write(
            &root.join("Cargo.toml"),
            "[workspace]\nresolver = \"3\"\nmembers = [\"projects/engines/geometry/crates/*\"]\n",
        );
        package(
            &kernel_dir(&root, "engine-k"),
            "engine-k",
            &format!("{section}\ngeom-test = {{ path = '../geom-test' }}\n"),
        );
        package(&kernel_dir(&root, "geom-test"), "geom-test", "");
        let manifest = root.join("Cargo.toml");
        run_cargo(&["generate-lockfile", "--offline"], &manifest);
        let lock = std::fs::read(root.join("Cargo.lock")).unwrap();
        let report = check(&cargo_metadata(&cargo(), &manifest).unwrap()).unwrap();
        (lock, report.violations.len())
    };
    let (dev_lock, dev_violations) = lock_with("[dev-dependencies]");
    let (normal_lock, normal_violations) = lock_with("[dependencies]");
    assert_eq!(dev_violations, 0);
    assert_eq!(normal_violations, 1, "the rule catches the moved edge");
    assert_eq!(
        text(&dev_lock),
        text(&normal_lock),
        "Cargo.lock differs between a dev-dependency and a normal dependency"
    );
}

/// The `paths:` lists of one trigger in a workflow file, in order of appearance (`pull_request`
/// first in kernel-dev.yml, then push). A minimal reader for the block-list form the file uses.
fn workflow_path_filters(yaml: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut in_on = false;
    let mut trigger: Option<String> = None;
    let mut paths_indent: Option<usize> = None;
    for line in yaml.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let t = line.trim();
        if indent == 0 {
            in_on = t == "on:";
            trigger = None;
            paths_indent = None;
            continue;
        }
        if !in_on {
            continue;
        }
        if indent == 2 && t.ends_with(':') {
            trigger = Some(t.trim_end_matches(':').to_owned());
            paths_indent = None;
            continue;
        }
        if t == "paths:" {
            paths_indent = Some(indent);
            out.entry(trigger.clone().unwrap()).or_default();
            continue;
        }
        if let (Some(pi), Some(tr)) = (paths_indent, &trigger) {
            if indent > pi && t.starts_with("- ") {
                let item = t[2..]
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_owned();
                out.get_mut(tr).unwrap().push(item);
                continue;
            }
            paths_indent = None;
        }
    }
    out
}

/// GitHub's filter-pattern match for the forms the workflows use: `**` matches any characters,
/// `*` any characters except `/`, everything else itself.
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

/// Every manifest the dependency rule reads to decide its result, relative to the workspace root
/// with `/` separators: every kernel crate (as `geom_test::deps` defines them) and every workspace
/// member they reach through normal or build path dependencies.
fn manifests_the_rule_reads(metadata: &Value) -> BTreeSet<String> {
    let root = PathBuf::from(metadata["workspace_root"].as_str().unwrap());
    let members: BTreeSet<&str> = metadata["workspace_members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap())
        .collect();
    let mut by_name: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    for p in metadata["packages"].as_array().unwrap() {
        if !members.contains(p["id"].as_str().unwrap()) {
            continue;
        }
        let rel = Path::new(p["manifest_path"].as_str().unwrap())
            .strip_prefix(&root)
            .unwrap()
            .components()
            .map(|c| c.as_os_str().to_str().unwrap().to_owned())
            .collect::<Vec<_>>()
            .join("/");
        let deps = p["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["kind"].is_null() || d["kind"] == "build")
            .filter(|d| d["path"].is_string())
            .map(|d| d["name"].as_str().unwrap().to_owned())
            .collect();
        by_name.insert(p["name"].as_str().unwrap().to_owned(), (rel, deps));
    }
    let is_kernel = |name: &str, rel: &str| {
        let parts: Vec<&str> = rel.split('/').collect();
        !DEVELOPMENT_CRATES.contains(&name)
            && ((parts.len() == 6
                && parts[..4] == ["projects", "engines", "geometry", "crates"]
                && parts[5] == "Cargo.toml")
                || (parts.len() == 5
                    && parts[..3] == ["projects", "engines", "crates"]
                    && parts[4] == "Cargo.toml"
                    && (name == "engine-geometry" || name.starts_with("engine-geometry-"))))
    };
    let mut seen = BTreeSet::new();
    let mut queue: VecDeque<String> = by_name
        .iter()
        .filter(|(n, (rel, _))| is_kernel(n, rel))
        .map(|(n, _)| n.clone())
        .collect();
    let mut out = BTreeSet::new();
    while let Some(n) = queue.pop_front() {
        if !seen.insert(n.clone()) {
            continue;
        }
        if let Some((rel, deps)) = by_name.get(&n) {
            out.insert(rel.clone());
            queue.extend(deps.iter().cloned());
        }
    }
    out
}

/// "No kernel crate has a normal dependency on a development crate (checked in CI)." The check
/// is a test that kernel-dev.yml runs, and the workflow runs only when a changed file matches its
/// path filters. Those filters cover projects/engines/geometry/**, the root Cargo.toml, Cargo.lock
/// and the workflow, but the rule also reads the facade's manifest
/// (projects/engines/crates/engine-geometry/Cargo.toml, a kernel crate by `geom_test::deps`'s own
/// definition) and kit-core's (the facade's normal path dependency). A pull request that only
/// moves `geom-test` from `[dev-dependencies]` to `[dependencies]` in either file breaks the rule,
/// leaves Cargo.lock unchanged (previous test), and never starts the workflow.
#[cfg_attr(
    target_family = "wasm",
    ignore = "runs cargo as a child process, which WebAssembly cannot"
)]
#[test]
fn attack_kernel_dev_workflow_runs_when_any_manifest_the_rule_reads_changes() {
    let root = workspace_root();
    let yaml_path = root
        .join(".github")
        .join("workflows")
        .join("kernel-dev.yml");
    let yaml = std::fs::read_to_string(&yaml_path).unwrap();
    let filters = workflow_path_filters(&yaml);
    assert_eq!(
        filters.keys().map(String::as_str).collect::<Vec<_>>(),
        ["pull_request", "push"],
        "could not read the triggers' path filters: {filters:?}"
    );
    let metadata = cargo_metadata(&cargo(), &crate_dir().join("Cargo.toml")).unwrap();
    let manifests = manifests_the_rule_reads(&metadata);
    assert!(
        manifests.contains("projects/engines/geometry/crates/geom-num/Cargo.toml"),
        "{manifests:?}"
    );
    let mut uncovered = Vec::new();
    for (trigger, patterns) in &filters {
        for m in &manifests {
            if !patterns.iter().any(|p| glob(p.as_bytes(), m.as_bytes())) {
                uncovered.push(format!("{trigger}: {m}"));
            }
        }
    }
    assert!(
        uncovered.is_empty(),
        "kernel-dev.yml does not run when these manifests change, though the dependency rule \
         reads them:\n  {}\nfilters: {filters:?}\nexpected: every manifest the rule reads matches a \
         filter of both triggers (for example \"projects/engines/crates/engine-geometry*/**\" and \
         \"projects/kit/crates/kit-core/**\", or drop the path filters)\nactual: {} unmatched",
        uncovered.join("\n  "),
        uncovered.len()
    );
}

// ---------------------------------------------------------------------------------------------
// 3. proptest's environment variables still change a seeded run
// ---------------------------------------------------------------------------------------------

/// Every environment variable proptest 1.12 reads (`test_runner`/config.rs, `contextualize_config`).
const PROPTEST_VARS: [&str; 13] = [
    "PROPTEST_CASES",
    "PROPTEST_MAX_LOCAL_REJECTS",
    "PROPTEST_MAX_GLOBAL_REJECTS",
    "PROPTEST_MAX_FLAT_MAP_REGENS",
    "PROPTEST_MAX_SHRINK_TIME",
    "PROPTEST_MAX_SHRINK_ITERS",
    "PROPTEST_MAX_DEFAULT_SIZE_RANGE",
    "PROPTEST_VERBOSE",
    "PROPTEST_RNG_ALGORITHM",
    "PROPTEST_RNG_SEED",
    "PROPTEST_DISABLE_FAILURE_PERSISTENCE",
    "PROPTEST_FORK",
    "PROPTEST_TIMEOUT",
];

/// The child: the cases a few seeded proptest draws produce. Run directly, it simply passes.
#[test]
fn child_proptest_probe() {
    let points: Vec<_> = (0..8)
        .map(|i| prop::first_value(&prop::p3(), Seed(i)).unwrap())
        .collect();
    println!("PROBE| p3 first values: {points:?}");
    let lengths: Vec<usize> = (0..16)
        .map(|i| prop::first_value(&any::<Vec<u8>>(), Seed(i)).unwrap().len())
        .collect();
    println!("PROBE| Vec<u8> first-value lengths: {lengths:?}");
    let longest = Cell::new(0usize);
    let total = Cell::new(0usize);
    let r = prop::run("attack probe", Seed(7), 64, &any::<Vec<u8>>(), |v| {
        longest.set(longest.get().max(v.len()));
        total.set(total.get() + v.len());
        Ok(())
    });
    println!(
        "PROBE| prop::run over 64 Vec<u8> cases from seed 7: ok {}, longest {}, total length {}",
        r.is_ok(),
        longest.get(),
        total.get()
    );
}

/// Runs `child_proptest_probe` in a child process with every proptest variable and
/// `SPAREKIT_SEED` removed, then `set` applied; returns whether it succeeded, its `probe` lines
/// and its whole output.
fn probe_raw(set: &[(&str, &str)]) -> (bool, Vec<String>, String) {
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "child_proptest_probe",
        "--exact",
        "--nocapture",
        "--test-threads=1",
    ]);
    for v in PROPTEST_VARS {
        cmd.env_remove(v);
    }
    cmd.env_remove("SPAREKIT_SEED");
    for (k, v) in set {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    let stdout = text(&out.stdout);
    // libtest prints "test child_proptest_probe ... " on the same line as the first output.
    let lines: Vec<String> = stdout
        .lines()
        .filter_map(|l| l.find("PROBE| ").map(|i| l[i..].to_owned()))
        .collect();
    (
        out.status.success(),
        lines,
        format!("{stdout}{}", text(&out.stderr)),
    )
}

/// As [`probe_raw`], for a child that must succeed; returns its `probe` lines.
fn probe(set: &[(&str, &str)]) -> Vec<String> {
    let (ok, lines, all) = probe_raw(set);
    assert!(ok, "{set:?}: {all}");
    assert_eq!(lines.len(), 3, "{all}");
    lines
}

/// What `geom_test::prop` does keep: proptest's seed, case-count and algorithm variables cannot
/// change which cases a seeded run draws.
#[cfg_attr(
    target_family = "wasm",
    ignore = "runs this test binary as a child process, which WebAssembly cannot"
)]
#[test]
fn proptest_seed_case_and_algorithm_variables_cannot_change_a_run() {
    let base = probe(&[]);
    let changed = probe(&[
        ("PROPTEST_RNG_SEED", "12345"),
        ("PROPTEST_CASES", "1"),
        ("PROPTEST_RNG_ALGORITHM", "xs"),
        ("PROPTEST_MAX_SHRINK_ITERS", "0"),
    ]);
    assert_eq!(base, changed);
}

/// prop.rs, as first written: "proptest's own environment variables (`PROPTEST_RNG_SEED`,
/// `PROPTEST_CASES`, `PROPTEST_MAX_DEFAULT_SIZE_RANGE` and the rest) cannot change a run:
/// `SPAREKIT_SEED` is the one override." It did not hold for `PROPTEST_MAX_DEFAULT_SIZE_RANGE`:
/// proptest never reads `Config::max_default_size_range` from the runner's configuration, and
/// `SizeRange::default()` (collection.rs) reads `Config::default()`, the process-wide default built
/// from the environment, when a default-sized collection strategy (`any::<Vec<T>>()`,
/// `any::<String>()`) is built. With the variable set to 1, the probe's `Vec<u8>` first values went
/// from lengths [50, 31, 32, 62, ...] to all 0, so a printed seed did not replay in a shell
/// without the variable.
///
/// Changed by the fixer (G11). The original test required the same cases with the variable set.
/// No code in `geom_test::prop` can meet that: the size is fixed inside the caller's strategy before
/// `prop::run` or `prop::first_value` sees it, and safe Rust cannot change this process's
/// environment or proptest's process-wide default. The promise the principles need is that a
/// printed seed never silently draws other cases (principle 5; 28.5, "any failure replays
/// exactly"). So `prop` now refuses to draw while the variable changes the default size, naming the
/// variable, and this test checks exactly that: the child fails with that refusal and prints no
/// cases. Unset, or set to proptest's default of 100, the child draws the cases recorded above.
#[cfg_attr(
    target_family = "wasm",
    ignore = "runs this test binary as a child process, which WebAssembly cannot"
)]
#[test]
fn attack_proptest_max_default_size_range_cannot_silently_change_a_seeded_run() {
    let base = probe(&[]);
    assert_eq!(probe(&[("PROPTEST_MAX_DEFAULT_SIZE_RANGE", "100")]), base);
    for size in ["0", "1", "37"] {
        let (ok, lines, all) = probe_raw(&[("PROPTEST_MAX_DEFAULT_SIZE_RANGE", size)]);
        assert!(
            !ok,
            "\nwith PROPTEST_MAX_DEFAULT_SIZE_RANGE={size} the seeded child ran:\n{all}"
        );
        assert!(
            all.contains("nothing was drawn: PROPTEST_MAX_DEFAULT_SIZE_RANGE"),
            "\nthe refusal must name the variable:\n{all}"
        );
        assert!(
            lines.is_empty(),
            "\nwith PROPTEST_MAX_DEFAULT_SIZE_RANGE={size} the child drew cases: {lines:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 4. prop::coord() does not shrink towards zero
// ---------------------------------------------------------------------------------------------

/// The minimal failing coordinate proptest reports for `holds`, from `seed`, as `prop::run` would
/// run it (the same configuration, without reading `SPAREKIT_SEED`).
fn minimal_failing_coord(seed: Seed, holds: fn(i64) -> bool) -> Option<i64> {
    let mut runner = TestRunner::new(prop::config(seed, 256));
    match runner.run(&prop::coord(), |c| {
        prop_assert!(holds(c.get()));
        Ok(())
    }) {
        Err(TestError::Fail(_, c)) => Some(c.get()),
        _ => None,
    }
}

/// prop.rs: "**Grid strategies stay on the grid** and shrink towards zero", and `coord()`
/// "Shrinks towards zero". The extremes alternative is `proptest::sample::select(EXTREMES)`,
/// which shrinks towards the FIRST element, `COORD_MAX`, so a failure found there shrinks away
/// from zero and stops at ±(2^40 − 1), even when far smaller failing values exist.
#[test]
fn attack_coord_strategy_shrinks_towards_zero() {
    let mut problems = Vec::new();
    // Fails for every |v| ≥ 2^30: the minimal failing value has magnitude 2^30.
    let big = |v: i64| v.abs() < 1 << 30;
    // Fails only at the frame's edge: ±(2^40 − 2) and ±(2^40 − 1); the minimal is 2^40 − 2.
    let edge = |v: i64| v.abs() < COORD_MAX - 1;
    for (law, holds, want) in [
        ("|v| < 2^30", big as fn(i64) -> bool, 1i64 << 30),
        ("|v| < 2^40 - 2", edge, COORD_MAX - 1),
    ] {
        for seed in 0..4 {
            let got = minimal_failing_coord(Seed(seed), holds);
            if got.map(i64::abs) != Some(want) {
                problems.push(format!(
                    "property {law}, seed {seed:#018x}, 256 cases: expected the minimal failing \
                     coordinate to have magnitude {want}; actual minimal failing input {got:?}"
                ));
            }
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

// ---------------------------------------------------------------------------------------------
// 5. The same seed gives different proptest cases on wasm32 and on 64-bit targets
// ---------------------------------------------------------------------------------------------

/// The first `prop::p3()` value of seeds 0..8, as `(x, y, z)`, as the adversary measured them on
/// Windows x86-64 (`cargo test`) and under wasm32-wasip1 (`cargo test --target wasm32-wasip1`,
/// wasmtime 50) before the fix. Seeds 2, 4 and 5 agreed; 0, 1, 3, 6 and 7 did not. Kept as the
/// evidence of the attack; the test below no longer reads them.
#[allow(dead_code)]
const P3_FIRST_X86_64_BEFORE_THE_FIX: [(i64, i64, i64); 8] = [
    (1_059_968_056_541, -1_099_511_627_775, 85_446_732_093),
    (-763_051_705_532, -151_791_219_982, 1),
    (-386_594_962_953, 6, -765_518_524_944),
    (-1_033_217_621_579, 406_094_445_385, 0),
    (166_617_143_637, 58_512_892_229, 1),
    (1_096_092_523_772, 11, 1_073_358_076_768),
    (462_718_231_816, -1_099_511_627_774, 152_362_041_343),
    (76_103_167_404, -1_099_511_627_774, 935_465_445_497),
];
#[allow(dead_code)]
const P3_FIRST_WASM32_BEFORE_THE_FIX: [(i64, i64, i64); 8] = [
    (1_059_968_056_541, -1_099_511_627_775, -1_099_511_627_775),
    (-763_051_705_532, -151_791_219_982, 0),
    (-386_594_962_953, 6, -765_518_524_944),
    (-1_033_217_621_579, 406_094_445_385, -1),
    (166_617_143_637, 58_512_892_229, 1),
    (1_096_092_523_772, 11, 1_073_358_076_768),
    (462_718_231_816, 1_099_511_627_774, 1_095_831_078_180),
    (76_103_167_404, 0, 1),
];

/// The first `prop::p3()` value of seeds 0..8 after the fix, measured with this file on Windows
/// x86-64 (`cargo test`) and under wasm32-wasip1 (`cargo test --target wasm32-wasip1`, wasmtime
/// 50): the same on both. Seeds 0, 3 and 4 draw extremes (±(2^40 − 1), ±(2^40 − 2)) and seeds 2,
/// 3, 4, 5 and 6 small values, so the kinds that used `proptest::sample::select` are covered.
const P3_FIRST: [(i64, i64, i64); 8] = [
    (791_725_522_716, -983_533_838_266, 1_099_511_627_775),
    (535_221_390_345, -155_071_321_939, -507_385_296_581),
    (8, 13, 67_830_043_593),
    (-1_099_511_627_774, -620_756_512_220, -4),
    (287_457_949_166, -13, 1_099_511_627_774),
    (-141_404_862_958, 16, 897_768_519_911),
    (548_336_618_847, -8, 358_649_223_817),
    (805_912_488_293, -322_013_981_243, -285_975_500_275),
];

/// geom-test's header: "**The same data on every target.** Test data comes from integer
/// arithmetic only, so a seed gives the same inputs on every target and only the kernel's results
/// can differ." The proptest strategies of `geom_test::prop` broke it: `coord()`'s extremes
/// alternative was `proptest::sample::select`, which draws a `Range<usize>`, and proptest samples
/// a usize range as a u64 on 64-bit targets and as a u32 on 32-bit ones (proptest 1.12,
/// src/num.rs, `sample_uniform!` under `cfg(target_pointer_width)`). So after any extremes pick
/// the two targets read the `ChaCha` stream at different offsets, and every later coordinate of the
/// case differed. A seed printed by a failing property under WebAssembly did not replay natively,
/// nor the other way round.
///
/// Changed by the fixer (G11). The original test compared the current target with a record for
/// its pointer width and then required the x86-64 and wasm32 records, two constants measured from
/// the old strategies, to be equal, so it could only pass if someone edited the evidence. The
/// strategies now draw one whole `u64` per coordinate and decide everything in integer arithmetic
/// (`prop::coord`), so this test checks the property itself: whatever the target, its first cases
/// equal the one record above, which was measured, identical, on x86-64 and on wasm32-wasip1. CI
/// runs it on both (`kernel-dev.yml`, the wasm32 job), and `prop`'s own golden hash covers 64
/// seeds the same way.
#[test]
fn attack_prop_strategies_draw_the_same_cases_on_every_target() {
    let here: Vec<(i64, i64, i64)> = (0..8)
        .map(|i| {
            let p = prop::first_value(&prop::p3(), Seed(i)).unwrap();
            (p.x.get(), p.y.get(), p.z.get())
        })
        .collect();
    let target = if cfg!(target_pointer_width = "64") {
        "this 64-bit target"
    } else {
        "this 32-bit target"
    };
    let differ: Vec<String> = (0..8)
        .filter(|&i| here[i] != P3_FIRST[i])
        .map(|i| {
            format!(
                "seed {i:#018x}: {target} {:?}, the record measured on x86-64 and wasm32-wasip1 {:?}",
                here[i], P3_FIRST[i]
            )
        })
        .collect();
    assert!(
        differ.is_empty(),
        "expected: prop::p3() draws the same first case from a seed on every target\n\
         actual: {} of 8 seeds differ:\n  {}",
        differ.len(),
        differ.join("\n  ")
    );
}

// ---------------------------------------------------------------------------------------------
// 6. geom-gen's registry rules let two generators own one file
// ---------------------------------------------------------------------------------------------

/// The probe program: builds registries `geom_gen::files::validate` must refuse and prints what
/// `validate`, `write` and `check` do with them, under the folder given as its argument.
///
/// Changed by the fixer (G11), calls only: `validate`, `write` and `check` now take a
/// `geom_gen::Registry`, the generators together with the generated folders whose orphans `check`
/// reports (a review finding). The probe's registries are its two generators with one generated
/// folder, `generated`, holding both outputs, so each is refused for the reason the attack names,
/// not for lying outside a folder. The outputs and the expectations are unchanged.
const GEN_PROBE_MAIN: &str = r#"
use geom_gen::files::{check, validate, write};
use geom_gen::{Folder, GenError, Generate, Generator, Registry};
use std::path::Path;

fn a() -> Result<Vec<u8>, GenError> { Ok(b"from generator a\n".to_vec()) }
fn b() -> Result<Vec<u8>, GenError> { Ok(b"from generator b\n".to_vec()) }

fn pair(out_a: &'static str, out_b: &'static str) -> [Generator; 2] {
    [
        Generator { name: "a", output: out_a, about: "probe", generate: Generate::Plain(a) },
        Generator { name: "b", output: out_b, about: "probe", generate: Generate::Plain(b) },
    ]
}

const FOLDERS: [Folder; 1] = [Folder { path: "generated", keep: &[] }];

fn main() {
    let base = std::env::args().nth(1).unwrap();
    let base = Path::new(&base);
    for (label, out_a, out_b) in [
        ("case", "generated/Table.md", "generated/table.md"),
        ("dot", "generated/t.md", "generated/t.md."),
        ("nested", "generated/x", "generated/x/y.md"),
    ] {
        let generators = pair(out_a, out_b);
        let reg = Registry { generators: &generators, folders: &FOLDERS };
        let root = base.join(label);
        println!("{label} outputs: {out_a:?} {out_b:?}");
        println!("{label} validate: {:?}", validate(&reg));
        let w = write(&root, &reg);
        println!("{label} write: {:?}", w.map(|o| o.iter().map(|x| x.describe()).collect::<Vec<_>>()));
        println!("{label} first output is a file after write: {}", root.join(out_a).is_file());
        let c = check(&root, &reg);
        println!("{label} check: {:?}", c.map(|o| o.iter().map(|x| x.describe()).collect::<Vec<_>>()));
    }
}
"#;

/// Builds and runs the probe once per test process; its stdout.
fn gen_probe() -> &'static str {
    static OUT: OnceLock<String> = OnceLock::new();
    OUT.get_or_init(|| {
        let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
        // The probe's sources and target folder persist, so later runs reuse the build.
        let dir = tmp.join("attack-g11-gen-probe");
        let gen_dir = crates_dir().join("geom-gen");
        write(
            &dir.join("Cargo.toml"),
            &format!(
                "[package]\nname = \"attack-g11-gen-probe\"\nversion = \"0.0.0\"\nedition = \
                 \"2024\"\npublish = false\n\n[dependencies]\ngeom-gen = {{ path = '{}' }}\n\n\
                 [workspace]\n",
                toml_path(&gen_dir)
            ),
        );
        write(&dir.join("src").join("main.rs"), GEN_PROBE_MAIN);
        if !dir.join("Cargo.lock").exists() {
            std::fs::copy(workspace_root().join("Cargo.lock"), dir.join("Cargo.lock")).unwrap();
        }
        let s = Scratch::new("gen-probe-run");
        let out = Command::new(cargo())
            .args(["run", "--offline", "--quiet", "--manifest-path"])
            .arg(dir.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(tmp.join("attack-g11-gen-probe-target"))
            .arg("--")
            .arg(&s.0)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "fixture broken, not the piece: the geom-gen probe did not build or run:\n{}{}",
            text(&out.stdout),
            text(&out.stderr)
        );
        text(&out.stdout)
    })
}

fn probe_line<'a>(out: &'a str, prefix: &str) -> &'a str {
    out.lines()
        .find(|l| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("no line {prefix:?} in\n{out}"))
}

/// files.rs: "two generators may not share a name or an output". `validate` compares output paths
/// as strings, but CI runs `geom-gen -- check` on Windows and macOS, whose file systems are
/// case-insensitive and (Windows) drop a trailing dot. So "generated/Table.md" and
/// "generated/table.md", or "generated/t.md" and "generated/t.md.", pass `validate` and are one
/// file there: `write` lets the second generator overwrite the first, and `check` then fails for
/// ever on those runners while passing on Linux.
#[cfg_attr(
    target_family = "wasm",
    ignore = "builds and runs a probe program with cargo, which WebAssembly cannot"
)]
#[test]
fn attack_geom_gen_validate_refuses_outputs_that_are_one_file_on_a_ci_runner() {
    let out = gen_probe();
    let mut problems = Vec::new();
    for label in ["case", "dot"] {
        let v = probe_line(out, &format!("{label} validate: "));
        if v.ends_with("Ok(())") {
            problems.push(format!(
                "{}\n  expected: Err(Registry(..)), the two outputs name one file on Windows or \
                 macOS\n  actual: {v}\n  then on this machine: {}\n  {}",
                probe_line(out, &format!("{label} outputs: ")),
                probe_line(out, &format!("{label} write: ")),
                probe_line(out, &format!("{label} check: ")),
            ));
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

/// files.rs: "**All or nothing before writing.** `write` validates the registry and runs every
/// generator before it writes any file". An output nested under another output
/// ("generated/x" and "generated/x/y.md") passes `validate`, although both cannot be files; `write`
/// then writes the first, fails on the second with an I/O error, and leaves the first written.
#[cfg_attr(
    target_family = "wasm",
    ignore = "builds and runs a probe program with cargo, which WebAssembly cannot"
)]
#[test]
fn attack_geom_gen_validate_refuses_an_output_inside_another_output() {
    let out = gen_probe();
    let v = probe_line(out, "nested validate: ");
    let w = probe_line(out, "nested write: ");
    let left = probe_line(out, "nested first output is a file after write: ");
    assert!(
        !v.ends_with("Ok(())"),
        "\n{}\nexpected: validate refuses it (Err(Registry(..))), so write writes nothing\n\
         actual: {v}\n{w}\n{left}",
        probe_line(out, "nested outputs: ")
    );
}

// ---------------------------------------------------------------------------------------------
// 7. Kernel crate rules the piece's code breaks
// ---------------------------------------------------------------------------------------------

/// The `//!` header at the top of a source file.
fn header(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .take_while(|l| l.starts_with("//!"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every non-root module of the three crates.
fn modules() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for krate in ["geom-test", "geom-fuzz", "geom-gen"] {
        let src = crates_dir().join(krate).join("src");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&src)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
            .filter(|p| {
                let n = p.file_name().unwrap();
                n != "lib.rs" && n != "main.rs"
            })
            .collect();
        files.sort();
        out.extend(files);
    }
    out
}

/// Task rule (and chapter 27): "each module starts with what it does, what it guarantees, the
/// published method it follows (with a link) and its cost". geom-num's modules all link their
/// method. A module passes when its header has a "Method" paragraph containing a link
/// (`http` or a `.md` path) and a "Cost" paragraph.
#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the crates' files from the host, which WASI cannot without a preopened folder"
)]
#[test]
fn attack_every_module_header_links_its_method_and_states_its_cost() {
    let mut problems = Vec::new();
    for path in modules() {
        let h = header(&path);
        let method = h
            .split("\n//!\n")
            .find(|para| para.trim_start_matches("//! ").starts_with("Method"));
        let cost = h.contains("//! Cost");
        let linked = method.is_some_and(|m| m.contains("http") || m.contains(".md"));
        if !(linked && cost) {
            problems.push(format!(
                "{}: method paragraph {}, link in it {linked}, cost {cost}{}",
                path.strip_prefix(crates_dir()).unwrap().display(),
                if method.is_some() {
                    "present"
                } else {
                    "MISSING"
                },
                method
                    .map(|m| format!("\n      {}", m.replace('\n', "\n      ")))
                    .unwrap_or_default()
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "module headers without a linked method or a cost:\n  {}",
        problems.join("\n  ")
    );
}

/// Task rule: "no panics outside tests (no unwrap, expect, panic!, unreachable!, todo!, no []
/// indexing)". geom-num, the precedent, has no `[]` indexing outside its tests. Library code here
/// (every line before `#[cfg(test)]`, comments skipped) must not index with `x[i]` either; a
/// constant index into an array cannot panic, but the rule names the syntax, and `to_le_bytes`,
/// `u128::try_from` or a shift and `u64::try_from` say the same thing without it.
#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the crates' files from the host, which WASI cannot without a preopened folder"
)]
#[test]
fn attack_library_code_has_no_bracket_indexing() {
    let mut hits = Vec::new();
    let mut files = modules();
    for krate in ["geom-test", "geom-fuzz", "geom-gen"] {
        files.push(crates_dir().join(krate).join("src").join("lib.rs"));
    }
    files.push(crates_dir().join("geom-gen").join("src").join("main.rs"));
    for path in files {
        let source = std::fs::read_to_string(&path).unwrap();
        for (n, line) in source.lines().enumerate() {
            if line.contains("#[cfg(test)]") {
                break;
            }
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            let b = code.as_bytes();
            for i in 1..b.len() {
                let prev = b[i - 1];
                let indexing = b[i] == b'['
                    && (prev.is_ascii_alphanumeric()
                        || prev == b'_'
                        || prev == b')'
                        || prev == b']')
                    && b.get(i + 1).is_some_and(|&c| c != b']');
                if indexing {
                    hits.push(format!(
                        "{}:{}: {}",
                        path.strip_prefix(crates_dir()).unwrap().display(),
                        n + 1,
                        code
                    ));
                    break;
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "expected no [] indexing in library code; found {}:\n  {}",
        hits.len(),
        hits.join("\n  ")
    );
}

// ---------------------------------------------------------------------------------------------
// 8. Checks that hold (passing)
// ---------------------------------------------------------------------------------------------

/// The three crates copy geom-num's `[lints]` tables and clippy.toml exactly, and forbid unsafe
/// code at the crate root.
#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the crates' files from the host, which WASI cannot without a preopened folder"
)]
#[test]
fn lints_and_clippy_config_match_geom_num() {
    let lints = |krate: &str| {
        // Line ends are normalised: a Windows checkout may give geom-num's files CRLF.
        let toml = std::fs::read_to_string(crates_dir().join(krate).join("Cargo.toml"))
            .unwrap()
            .replace("\r\n", "\n");
        toml[toml.find("[lints.rust]").unwrap()..].to_owned()
    };
    let clippy = |krate: &str| {
        std::fs::read_to_string(crates_dir().join(krate).join("clippy.toml"))
            .unwrap()
            .replace("\r\n", "\n")
    };
    for krate in ["geom-test", "geom-fuzz", "geom-gen"] {
        assert_eq!(lints(krate), lints("geom-num"), "{krate}");
        assert_eq!(clippy(krate), clippy("geom-num"), "{krate}");
        for root in ["lib.rs", "main.rs"] {
            let p = crates_dir().join(krate).join("src").join(root);
            if p.exists() {
                let s = std::fs::read_to_string(&p).unwrap();
                assert!(s.contains("#![forbid(unsafe_code)]"), "{}", p.display());
            }
        }
    }
}

/// Every step `geom_num::grid::GridStep` has (walked with `finer()` from Q0) has a row in the
/// committed grid-steps table, and the row's exact sizes agree with an independent computation.
/// The generator lists the steps by hand, so a new step would be missed silently; today it is not.
#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the crates' files from the host, which WASI cannot without a preopened folder"
)]
#[test]
fn the_grid_steps_table_has_every_step_and_exact_sizes() {
    use geom_num::grid::GridStep;
    let table = std::fs::read_to_string(
        crates_dir()
            .join("geom-gen")
            .join("generated")
            .join("grid-steps.md"),
    )
    .unwrap();
    let mut step = Some(GridStep::Q0);
    let mut seen = 0;
    while let Some(s) = step {
        let q = u128::from(s.quarter_nm());
        let name = format!("{s:?}");
        let row = table
            .lines()
            .find(|l| l.starts_with(&format!("| {name} | ")))
            .unwrap_or_else(|| panic!("no row for {name}"));
        // 1 mm = 4,000,000 quarter-nm, so steps per mm = 4,000,000 / q (q divides it today).
        assert_eq!(4_000_000 % q, 0, "{name}");
        assert!(row.contains(&format!("| {} |", 4_000_000 / q)), "{row}");
        assert!(
            row.starts_with(&format!("| {name} | {q} | 1/{} |", 4_000_000 / q)),
            "{row}"
        );
        seen += 1;
        step = s.finer();
    }
    assert_eq!(seen, 3);
}
