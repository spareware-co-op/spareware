//! Each gate of section 28.8 fails on a seeded fault and passes once the fault is fixed, run with
//! the real tools (piece G21).
//!
//! `quality_gates.rs` tests the judges on output written the way the tools write it; this file
//! runs the tools themselves on a seeded repository in a temporary folder: a foundation crate
//! named `geom-num` (so the gates hold it to the foundation bar), the repository's own
//! `.cargo/mutants.toml`, `rust-toolchain.toml` and an empty quality ledger. Then
//! `kernel-quality`, the binary CI runs, judges what the tools wrote:
//!
//! - **A seeded survivor** (`replace > with >= in is_positive`, which no test at zero catches)
//!   fails the mutation gate, in the full run and in the diff run; kept as `known` it still fails,
//!   because the foundation set keeps only equivalent mutants; a test at zero then catches it and
//!   the gate passes.
//! - **A seeded uncovered line and an untaken branch** fail the coverage gate; tests that run the
//!   line and take the branch both ways make it pass.
//! - **A `coverage(off)` mark** fails while it cites no proof, and passes once it cites a Kani
//!   harness its ledger entry names.
//!
//! The tests need cargo-mutants and cargo-llvm-cov installed, and cargo-llvm-cov a nightly
//! toolchain with `llvm-tools` (named by `SPAREKIT_COVERAGE_TOOLCHAIN`), so they are ignored in an
//! ordinary test run; CI runs them in `.github/workflows/kernel-quality.yml` (job `gates-seeded`)
//! with `--ignored`. Run that way, a missing tool fails the test rather than skipping it.

// The crate denies native arithmetic in kernel code (Cargo.toml); tests may use it.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "test fixtures; the rules are for kernel code"
)]
// A broken fixture must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The process id names each fixture's temporary folder, so two test runs at once never share
// one; it decides no result.
#![allow(
    clippy::disallowed_methods,
    reason = "the process id only names a temporary folder"
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The kernel crate the fixture holds: named as a foundation crate.
const CRATE: &str = "projects/engines/geometry/crates/geom-num";

/// The fixture's code: `is_positive` is the seeded survivor's home, `above` the seeded untaken
/// branch (line 11), `unused` the seeded uncovered lines (15 to 17). No mutant of it is
/// equivalent, so the strong tests catch every one.
const LIB: &str = "//! A seeded crate for the quality gates.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

/// Whether `x` is above zero.
pub fn is_positive(x: i32) -> bool {
    x > 0
}

/// 2 when `x` is above `top`, otherwise 1.
pub fn above(x: i32, top: i32) -> u8 {
    if x > top { 2 } else { 1 }
}

/// Twice `x`.
pub fn unused(x: i32) -> i32 {
    x.wrapping_mul(2)
}
";

/// The tests that leave the faults in: nothing tests `is_positive` at zero, `above` at or above
/// its top, or `unused` at all.
const WEAK_TESTS: &str = "use geom_num::{above, is_positive};

#[test]
fn signs() {
    assert!(is_positive(5));
    assert!(!is_positive(-5));
}

#[test]
fn below_the_top() {
    assert_eq!(above(1, 3), 1);
}
";

/// The tests that fix them.
const STRONG_TESTS: &str = "use geom_num::{above, is_positive, unused};

#[test]
fn signs() {
    assert!(is_positive(5));
    assert!(!is_positive(-5));
    assert!(!is_positive(0));
}

#[test]
fn on_both_sides_of_the_top_and_at_it() {
    assert_eq!(above(1, 3), 1);
    assert_eq!(above(3, 3), 1);
    assert_eq!(above(9, 3), 2);
}

#[test]
fn doubles() {
    assert_eq!(unused(4), 8);
}
";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .to_path_buf()
}

/// A seeded repository in a temporary folder, removed when dropped.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Fixture {
        let dir =
            std::env::temp_dir().join(format!("geom-test-seeded-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = Fixture {
            root: std::fs::canonicalize(&dir).unwrap(),
        };
        let real = repo_root();
        for file in [".cargo/mutants.toml", "rust-toolchain.toml"] {
            f.write(file, &std::fs::read_to_string(real.join(file)).unwrap());
        }
        f.write(
            "Cargo.toml",
            "[workspace]\nresolver = \"3\"\nmembers = [\"projects/engines/geometry/crates/*\"]\n",
        );
        f.write(
            &format!("{CRATE}/Cargo.toml"),
            "[package]\nname = \"geom-num\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
             [lints.rust]\nunexpected_cfgs = { level = \"warn\", check-cfg = [\"cfg(coverage_nightly)\", \"cfg(kani)\"] }\n",
        );
        f.write(&format!("{CRATE}/src/lib.rs"), LIB);
        f.write(&format!("{CRATE}/tests/api.rs"), WEAK_TESTS);
        f.write(
            "projects/engines/geometry/quality-ledger.json",
            &ledger("[]", "[]"),
        );
        f.run(cargo().arg("generate-lockfile"), "cargo generate-lockfile");
        f
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.path(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// Runs a command in the fixture and returns its output, failing the test if it cannot start.
    fn output(&self, cmd: &mut Command, what: &str) -> Output {
        cmd.current_dir(&self.root)
            .output()
            .unwrap_or_else(|e| panic!("{what} could not run (is it installed?): {e}"))
    }

    /// Runs a command that must succeed.
    fn run(&self, cmd: &mut Command, what: &str) -> Output {
        let out = self.output(cmd, what);
        assert!(
            out.status.success(),
            "{what} failed: {}\n{}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }

    /// `kernel-quality` on this fixture: its exit code and standard output.
    fn judge(&self, args: &[&str]) -> (i32, String) {
        let out = self.output(
            Command::new(env!("CARGO_BIN_EXE_kernel-quality"))
                .args(args)
                .arg("--repo")
                .arg(&self.root),
            "kernel-quality",
        );
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        (out.status.code().unwrap(), text)
    }

    /// Runs cargo-mutants over the fixture's crate, optionally on a diff, and returns its output
    /// directory and the list of mutants it was given.
    fn mutate(&self, label: &str, diff: Option<&Path>) -> (PathBuf, PathBuf) {
        let out = self.path(&format!("out-{label}"));
        let list = self.path(&format!("list-{label}.json"));
        let mut list_cmd = cargo();
        list_cmd.args(["mutants", "--list", "--json"]);
        if let Some(d) = diff {
            list_cmd.arg("--in-diff").arg(d);
        }
        let listed = self.run(&mut list_cmd, "cargo mutants --list");
        std::fs::write(&list, &listed.stdout).unwrap();
        let mut run = cargo();
        run.args(["mutants", "--no-shuffle", "--in-place", "--output"])
            .arg(&out);
        if let Some(d) = diff {
            run.arg("--in-diff").arg(d);
        }
        let result = self.output(&mut run, "cargo mutants");
        let code = result.status.code().unwrap();
        assert!(
            [0, 2, 3].contains(&code),
            "cargo mutants failed with {code}: {}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        (out.join("mutants.out"), list)
    }

    /// Measures the fixture's crate with cargo-llvm-cov on the coverage toolchain, as the coverage
    /// job does, into `DIR/geom-num.lcov` and `DIR/geom-num.log`; returns `DIR`.
    fn measure(&self, label: &str) -> PathBuf {
        let toolchain = std::env::var("SPAREKIT_COVERAGE_TOOLCHAIN").expect(
            "SPAREKIT_COVERAGE_TOOLCHAIN must name the nightly toolchain with llvm-tools that \
             cargo-llvm-cov's branch coverage needs",
        );
        let dir = self.path(&format!("cov-{label}"));
        std::fs::create_dir_all(&dir).unwrap();
        let out = self.run(
            cargo()
                .env("RUSTUP_TOOLCHAIN", &toolchain)
                .env("RUSTFLAGS", "-Z cross-crate-inline-threshold=never")
                .args([
                    "llvm-cov",
                    "--locked",
                    "-p",
                    "geom-num",
                    "--release",
                    "--branch",
                ])
                .args(["--lcov", "--output-path"])
                .arg(dir.join("geom-num.lcov")),
            "cargo llvm-cov",
        );
        let log = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        std::fs::write(dir.join("geom-num.log"), log).unwrap();
        dir
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// `cargo` through rustup's proxy, so the fixture's `rust-toolchain.toml` (or `RUSTUP_TOOLCHAIN`,
/// where a step sets it) picks the toolchain, as it does in CI; the variables the test runner's own
/// cargo set are removed so they cannot pick it instead.
fn cargo() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.env_remove("CARGO").env_remove("RUSTUP_TOOLCHAIN");
    cmd
}

fn ledger(survivors: &str, coverage_off: &str) -> String {
    format!(
        "{{\"schema\": \"spare-kit-quality-ledger/1\", \"survivors\": {survivors}, \
         \"coverage_off\": {coverage_off}}}"
    )
}

/// The seeded survivor, kept as `kind` in the ledger.
fn kept_as(kind: &str) -> String {
    ledger(
        &format!(
            "[{{\"file\": \"{CRATE}/src/lib.rs\", \"mutant\": \"replace > with >= in is_positive\", \
             \"line_text\": \"x > 0\", \"occurrences\": 1, \"kind\": \"{kind}\", \"reason\": \
             \"Kept to show what the gate makes of this kind in the foundation set.\"}}]"
        ),
        "[]",
    )
}

fn assert_fails_with(result: &(i32, String), needles: &[&str]) {
    assert_eq!(result.0, 1, "the gate should fail:\n{}", result.1);
    for n in needles {
        assert!(result.1.contains(n), "missing {n:?} in:\n{}", result.1);
    }
}

fn assert_passes(result: &(i32, String)) {
    assert_eq!(result.0, 0, "the gate should pass:\n{}", result.1);
}

#[test]
#[ignore = "runs cargo-mutants; CI's gates-seeded job runs it with --ignored"]
fn a_seeded_survivor_fails_the_mutation_gate_until_a_test_catches_it() {
    let f = Fixture::new("mutants");
    let (out, list) = f.mutate("weak", None);
    let (out, list) = (out.to_str().unwrap(), list.to_str().unwrap());
    let full = ["mutants", "--mode", "full", "--expected", list, out];
    assert_fails_with(
        &f.judge(&full),
        &[
            "replace > with >= in is_positive",
            "survived (foundation set",
            "no ledger entry explains it",
        ],
    );
    // Kept as known, or as harmless, the foundation set still refuses it (28.8).
    f.write(
        "projects/engines/geometry/quality-ledger.json",
        &kept_as("known"),
    );
    assert_fails_with(
        &f.judge(&full),
        &["the foundation set keeps only equivalent mutants"],
    );
    f.write(
        "projects/engines/geometry/quality-ledger.json",
        &ledger("[]", "[]"),
    );
    // A test at zero catches it, and the whole run passes.
    f.write(&format!("{CRATE}/tests/api.rs"), STRONG_TESTS);
    let (out, list) = f.mutate("strong", None);
    let (out, list) = (out.to_str().unwrap(), list.to_str().unwrap());
    assert_passes(&f.judge(&["mutants", "--mode", "full", "--expected", list, out]));
}

#[test]
#[ignore = "runs cargo-mutants; CI's gates-seeded job runs it with --ignored"]
fn a_pull_request_that_adds_a_survivor_fails_the_diff_gate() {
    let f = Fixture::new("diff");
    // The pull request adds is_positive's body; the diff holds exactly that change.
    let diff = format!(
        "--- a/{CRATE}/src/lib.rs\n+++ b/{CRATE}/src/lib.rs\n@@ -5,3 +5,3 @@\n pub fn is_positive(x: i32) -> bool {{\n-    false\n+    x > 0\n }}\n"
    );
    f.write("pr.diff", &diff);
    let (out, list) = f.mutate("diff", Some(&f.path("pr.diff")));
    let (out, list) = (out.to_str().unwrap(), list.to_str().unwrap());
    let args = ["mutants", "--mode", "diff", "--expected", list, out];
    assert_fails_with(&f.judge(&args), &["replace > with >= in is_positive"]);
    // A pull request cannot excuse its own survivor as known, even outside the foundation set's
    // rules: the base branch's ledger must already hold the entry.
    f.write("base-ledger.json", &ledger("[]", "[]"));
    let base = f.path("base-ledger.json");
    let with_base = [
        "mutants",
        "--mode",
        "diff",
        "--expected",
        list,
        "--base-ledger",
        base.to_str().unwrap(),
        out,
    ];
    assert_fails_with(&f.judge(&with_base), &["replace > with >= in is_positive"]);
}

#[test]
#[ignore = "runs cargo-llvm-cov on a nightly toolchain; CI's gates-seeded job runs it with --ignored"]
fn seeded_uncovered_code_fails_the_coverage_gate_until_tests_reach_it() {
    let f = Fixture::new("coverage");
    let dir = f.measure("weak");
    let args = [
        "coverage",
        "--crates",
        "geom-num",
        "--lcov-dir",
        dir.to_str().unwrap(),
    ];
    let weak = f.judge(&args);
    assert_fails_with(&weak, &["src/lib.rs: lines", "src/lib.rs: branches"]);
    // The uncovered lines are `unused`'s (15 to 17) and the untaken branch is `above`'s `if`
    // (line 11).
    assert!(weak.1.contains("never run: 15, 16, 17"), "{}", weak.1);
    assert!(weak.1.contains("both ways on lines: 11"), "{}", weak.1);
    f.write(&format!("{CRATE}/tests/api.rs"), STRONG_TESTS);
    let dir = f.measure("strong");
    let args = [
        "coverage",
        "--crates",
        "geom-num",
        "--lcov-dir",
        dir.to_str().unwrap(),
    ];
    assert_passes(&f.judge(&args));
}

#[test]
#[ignore = "runs cargo-llvm-cov on a nightly toolchain; CI's gates-seeded job runs it with --ignored"]
fn a_coverage_off_mark_fails_until_it_cites_a_proof_the_ledger_names() {
    let f = Fixture::new("marks");
    f.write(&format!("{CRATE}/tests/api.rs"), STRONG_TESTS);
    let marked = LIB.replace(
        "/// Twice `x`.\n",
        "/// Twice `x`.\n#[cfg_attr(coverage_nightly, coverage(off))]\n",
    );
    f.write(&format!("{CRATE}/src/lib.rs"), &marked);
    let dir = f.measure("unproven");
    let args = [
        "coverage",
        "--crates",
        "geom-num",
        "--lcov-dir",
        dir.to_str().unwrap(),
    ];
    assert_fails_with(&f.judge(&args), &["has no ledger entry citing its proof"]);
    // The mark cites a Kani harness, and the ledger names the same one.
    let cited = LIB.replace(
        "/// Twice `x`.\n",
        "/// Twice `x`.\n// proof: projects/engines/geometry/crates/geom-num/src/proofs.rs::doubles_never_overflow\n\
         #[cfg_attr(coverage_nightly, coverage(off))]\n",
    ) + "\n#[cfg(kani)]\nmod proofs;\n";
    f.write(&format!("{CRATE}/src/lib.rs"), &cited);
    f.write(
        &format!("{CRATE}/src/proofs.rs"),
        "//! Kani harnesses.\n\n#[kani::proof]\nfn doubles_never_overflow() {}\n",
    );
    f.write(
        "projects/engines/geometry/quality-ledger.json",
        &ledger(
            "[]",
            &format!(
                "[{{\"file\": \"{CRATE}/src/lib.rs\", \"item\": \"pub fn unused(x: i32) -> i32 {{\", \
                 \"proof\": \"{CRATE}/src/proofs.rs::doubles_never_overflow\", \"tool\": \"kani\", \
                 \"reason\": \"A seeded mark: the cited harness stands for the proof a real mark needs.\"}}]"
            ),
        ),
    );
    let dir = f.measure("proven");
    let args = [
        "coverage",
        "--crates",
        "geom-num",
        "--lcov-dir",
        dir.to_str().unwrap(),
    ];
    assert_passes(&f.judge(&args));
}
