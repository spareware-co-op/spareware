//! Attacks on piece G20 (the toolchain and dependency guard). Each test states what the board's
//! done-when, chapter 28.13 of the kernel architecture ("The toolchain guard"), or a guarantee in
//! the `guard` module header requires, and tries to break it. A test that passes is a guard on a
//! guarantee the piece keeps.
//!
//! The done-when: "Any change to rust-toolchain.toml, Cargo.lock or vendored numeric code runs
//! every golden across the matrix and fails on any hash change; a deliberate change to vendored
//! libm proves it trips. The format museum joins it in phase 3."
//!
//! The proof itself (`guard prove`) builds `geom-num` against a changed `libm`, so it runs in
//! `kernel.yml`'s guard job, not here; these tests hold the job, the triggers, the change and the
//! verdict in place. The in-memory attacks run on every target, WebAssembly included; the ones
//! that read the real tree or run Cargo run natively only.

// Tests may use native arithmetic to index their fixtures.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "fixtures; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken fixture must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_test::guard::{GuardError, SIN_FILE, TRIPPED_GOLDEN, perturb_sin, verdict};

/// The kernel's CI workflow, as committed, read when the test is built so the test runs on every
/// target.
const KERNEL_YML: &str = include_str!("../../../../../../.github/workflows/kernel.yml");

/// Chapter 28 of the kernel architecture.
const TESTING_MD: &str = include_str!("../../../architecture/28-testing.md");

/// The kernel's guide for agents.
const AGENTS_MD: &str = include_str!("../../../AGENTS.md");

/// The paths a trigger of `kernel.yml` (`pull_request` or `push`) lists, in order.
fn trigger_paths(trigger: &str) -> Vec<String> {
    let lines: Vec<&str> = KERNEL_YML.lines().collect();
    let start = lines
        .iter()
        .position(|l| *l == format!("  {trigger}:"))
        .unwrap_or_else(|| panic!("kernel.yml has no `{trigger}` trigger"));
    let mut paths = Vec::new();
    let mut in_paths = false;
    for line in &lines[start + 1..] {
        if !line.starts_with("    ") {
            break;
        }
        if *line == "    paths:" {
            in_paths = true;
        } else if let Some(item) = line.strip_prefix("      - ").filter(|_| in_paths) {
            paths.push(item.trim_matches('"').to_owned());
        } else {
            in_paths = false;
        }
    }
    paths
}

/// Whether a path filter of GitHub's syntax matches a repository path: exactly, or as a folder
/// pattern `<folder>/**` matching everything below it. These are the only forms `kernel.yml`
/// uses, which [`only_simple_patterns`] holds it to.
fn matches(pattern: &str, path: &str) -> bool {
    match pattern.strip_suffix("/**") {
        Some(folder) => path.starts_with(&format!("{folder}/")),
        None => pattern == path,
    }
}

/// The lines of one job of `kernel.yml`, from its id line to the next job's.
fn job(id: &str) -> Vec<&'static str> {
    let lines: Vec<&'static str> = KERNEL_YML.lines().collect();
    let start = lines
        .iter()
        .position(|l| *l == format!("  {id}:"))
        .unwrap_or_else(|| panic!("kernel.yml has no job `{id}`"));
    let end = lines[start + 1..]
        .iter()
        .position(|l| {
            l.len() > 2 && l.starts_with("  ") && !l.starts_with("   ") && l.ends_with(':')
        })
        .map_or(lines.len(), |i| start + 1 + i);
    lines[start..end].to_vec()
}

/// The guard: a change to the toolchain pin, the lock file, the workspace manifest, Cargo's
/// configuration or anything under the kernel (where vendored numeric code lives) reruns
/// `kernel.yml`, on pull requests and on main alike.
#[test]
fn both_triggers_watch_the_toolchain_the_lock_and_the_kernel() {
    for trigger in ["pull_request", "push"] {
        let paths = trigger_paths(trigger);
        for watched in [
            "rust-toolchain.toml",
            "rust-toolchain",
            "Cargo.lock",
            "Cargo.toml",
            ".cargo/config.toml",
            "projects/engines/geometry/crates/geom-num/src/math/unproven.rs",
            "projects/engines/geometry/crates/geom-num/vendor/libm/src/math/sin.rs",
            ".github/workflows/kernel.yml",
        ] {
            assert!(
                paths.iter().any(|p| matches(p, watched)),
                "kernel.yml's {trigger} trigger does not watch {watched}: {paths:?}"
            );
        }
    }
}

/// The matcher above is exact only for the two forms it knows, so the triggers may use no other.
#[test]
fn only_simple_patterns() {
    for trigger in ["pull_request", "push"] {
        for p in trigger_paths(trigger) {
            let body = p.strip_suffix("/**").unwrap_or(&p);
            assert!(
                !body.contains(['*', '?', '[', '!']),
                "kernel.yml's {trigger} trigger uses the pattern {p}, which this test cannot read"
            );
        }
    }
}

/// The proof runs in CI: a job of its own in `kernel.yml`, with the pinned toolchain, that nothing
/// skips or lets fail.
#[test]
fn the_guard_job_runs_the_proof() {
    let lines = job("guard");
    let text = lines.join("\n");
    assert!(
        text.contains("run: cargo run --locked --release -p geom-test --bin guard -- prove"),
        "{text}"
    );
    assert!(
        text.contains("toolchain: ${{ env.RUST_TOOLCHAIN }}"),
        "{text}"
    );
    // No condition on the job or its step, and no failure allowed.
    assert!(
        !lines.iter().any(|l| l.trim_start().starts_with("if:")),
        "{text}"
    );
    assert!(!text.contains("continue-on-error"), "{text}");
    assert!(text.contains("      - uses: actions/checkout@"), "{text}");
}

/// A fixture shaped as `libm`'s `sin.rs`: a header, an attribute, the function, and a caller.
const FIXTURE: &str = "/// The sine of `x` (f64).\n\
                       #[cfg_attr(assert_no_panic, no_panic::no_panic)]\n\
                       pub fn sin(x: f64) -> f64 {\n    x\n}\n\
                       pub fn twice(x: f64) -> f64 {\n    sin(x) + sin(x)\n}\n";

/// The change is one bit in one function: the original is renamed, still holds its body and
/// attribute, and the new `sin` returns its result with bit 0 flipped; nothing else moves.
#[test]
fn the_change_flips_one_bit_of_sin_and_nothing_else() {
    let changed = perturb_sin(FIXTURE).unwrap();
    assert!(changed.starts_with(&FIXTURE.replace(
        "pub fn sin(x: f64) -> f64 {",
        "fn sin_before_the_guard(x: f64) -> f64 {"
    )));
    assert_eq!(changed.matches("pub fn sin(x: f64) -> f64 {").count(), 1);
    assert_eq!(
        changed
            .matches("fn sin_before_the_guard(x: f64) -> f64 {")
            .count(),
        1
    );
    assert!(changed.contains("f64::from_bits(sin_before_the_guard(x).to_bits() ^ 1)"));
    assert!(changed.contains("#[cfg_attr(assert_no_panic, no_panic::no_panic)]\nfn sin_before"));
    // Callers inside the crate reach the changed sin, by its name.
    assert!(changed.contains("sin(x) + sin(x)"));
}

/// A `libm` that moved or doubled the function fails loudly instead of proving nothing.
#[test]
fn a_source_without_exactly_one_sin_is_refused() {
    assert_eq!(
        perturb_sin("pub fn cos(x: f64) -> f64 {}"),
        Err(GuardError::Signature(0))
    );
    let twice = format!("{FIXTURE}{FIXTURE}");
    assert_eq!(perturb_sin(&twice), Err(GuardError::Signature(2)));
    // The signature must match exactly: another spelling is not the function.
    assert_eq!(
        perturb_sin("pub fn sin(x:f64)->f64{x}"),
        Err(GuardError::Signature(0))
    );
}

/// The golden's line, as `golden::check` prints it, behind libtest's noise.
fn tripped_line() -> String {
    format!(
        "test the_corpus ... golden {TRIPPED_GOLDEN} (edition pre-edition): {} differs from the \
         store's {}",
        "8".repeat(64),
        "f".repeat(64)
    )
}

/// A trip counts only when the run failed, Cargo used the patch, and the golden said its digest
/// differs from the store's.
#[test]
fn a_trip_counts_only_for_the_right_reason() {
    let tripped = format!(
        "   Compiling libm v0.2.16\n{}\nerror: test failed\n",
        tripped_line()
    );
    assert_eq!(verdict(false, &tripped), Ok(()));
    // The run passed: the change moved nothing.
    assert_eq!(
        verdict(
            true,
            "golden geom-num/determinism (edition pre-edition): 00 agrees with the store"
        ),
        Err(GuardError::NotTripped)
    );
    // Cargo built the real libm.
    let unused = format!(
        "warning: Patch `libm v0.2.16 (/tmp/libm)` was not used in the crate graph.\n{tripped}"
    );
    assert_eq!(verdict(false, &unused), Err(GuardError::PatchUnused));
    assert_eq!(verdict(true, &unused), Err(GuardError::PatchUnused));
    // A build that failed proves nothing.
    assert_eq!(
        verdict(
            false,
            "error[E0425]: cannot find function `sin` in this scope"
        ),
        Err(GuardError::WrongFailure)
    );
    // Another golden's mismatch, or this golden agreeing, is not this golden tripping.
    let other = tripped_line().replace(TRIPPED_GOLDEN, "geom-math/determinism");
    assert_eq!(verdict(false, &other), Err(GuardError::WrongFailure));
    let agrees = tripped_line().replace("differs from the store's", "agrees with the store;");
    assert_eq!(verdict(false, &agrees), Err(GuardError::WrongFailure));
    // The two parts on different lines do not make a trip.
    let split = tripped_line().replace(" differs", "\n differs");
    assert_eq!(verdict(false, &split), Err(GuardError::WrongFailure));
}

/// Every refusal says what went wrong in words.
#[test]
fn every_refusal_explains_itself() {
    for (e, words) in [
        (GuardError::Signature(2), "2 times"),
        (GuardError::NotTripped, "no golden saw the change"),
        (GuardError::PatchUnused, "did not use the patched libm"),
        (GuardError::WrongFailure, TRIPPED_GOLDEN),
    ] {
        assert!(e.to_string().contains(words), "{e}");
    }
}

/// Chapter 28.13 and the agents' guide say how the guard is proved, and name the piece.
#[test]
fn the_guide_and_the_chapter_describe_the_proof() {
    assert!(TESTING_MD.contains("As built (G20)"));
    assert!(TESTING_MD.contains("`guard prove`"));
    assert!(TESTING_MD.contains("`attack_g20`"));
    assert!(AGENTS_MD.contains("--bin guard -- prove"));
}

#[cfg(not(target_family = "wasm"))]
mod native {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};

    use geom_test::deps::cargo_metadata;
    use geom_test::map::repo::workspace_root;

    use super::{SIN_FILE, matches, perturb_sin, trigger_paths};

    /// Every `Cargo.lock` under the repository, as repository paths with `/`, outside build output
    /// and git's own folder.
    fn lock_files(root: &Path, dir: &Path, found: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                if !matches!(name.as_str(), ".git" | "target" | "node_modules") {
                    lock_files(root, &path, found);
                }
            } else if name == "Cargo.lock" {
                let rel = path.strip_prefix(root).unwrap();
                let parts: Vec<String> = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                found.push(parts.join("/"));
            }
        }
    }

    /// Every lock file the kernel builds with, the workspace's and the second workspace of
    /// `geom-bench`'s WebAssembly fuel host among them, reruns `kernel.yml`.
    #[test]
    fn every_lock_file_of_the_kernel_is_watched() {
        let root = workspace_root().unwrap();
        let mut locks = Vec::new();
        lock_files(&root, &root, &mut locks);
        assert!(locks.contains(&"Cargo.lock".to_owned()), "{locks:?}");
        for lock in locks
            .iter()
            .filter(|l| *l == "Cargo.lock" || l.starts_with("projects/engines/"))
        {
            for trigger in ["pull_request", "push"] {
                assert!(
                    trigger_paths(trigger).iter().any(|p| matches(p, lock)),
                    "kernel.yml's {trigger} trigger does not watch {lock}"
                );
            }
        }
    }

    /// The `libm` the workspace resolves holds the function the proof changes, exactly once, so
    /// the proof in CI changes the code the goldens run.
    #[test]
    fn the_resolved_libm_holds_sin_once() {
        let root = workspace_root().unwrap();
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
        let metadata = cargo_metadata(&cargo, &root.join("Cargo.toml")).unwrap();
        let folders: Vec<PathBuf> = metadata["packages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| {
                p["name"] == "libm"
                    && p["source"]
                        .as_str()
                        .is_some_and(|s| s.starts_with("registry+"))
            })
            .map(|p| {
                Path::new(p["manifest_path"].as_str().unwrap())
                    .parent()
                    .unwrap()
                    .to_path_buf()
            })
            .collect();
        assert_eq!(folders.len(), 1, "{folders:?}");
        let source = fs::read_to_string(folders[0].join(SIN_FILE)).unwrap();
        perturb_sin(&source).unwrap();
    }
}
