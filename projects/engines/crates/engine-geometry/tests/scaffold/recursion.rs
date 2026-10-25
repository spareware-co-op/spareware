//! The merge gate's recursion check (piece G13, chapter 28.8): every recursive cycle of the crates
//! that ship has its depth bound stated, and each bound's test runs on the small WebAssembly stack.
//!
//! The check itself is `geom-test`'s `recursion` binary (`src/recursion` there); these ignored
//! tests are the gate's runs of it (`--ignored gate_recursion`, the gate's `recursion` job, which
//! installs the `wasm32-wasip1` target and a pinned wasmtime): `recursion gate` over the workspace,
//! and over seeded workspaces that must fail (a cycle without a statement, a bound its test
//! overflows the small stack at) or pass (a stated cycle whose test holds).
//!
//! The runs use wasmtime when it is installed (the gate's job installs it) and Node's WASI
//! otherwise, so a contributor without wasmtime can run them too.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::repo;

/// The WebAssembly engine the runs use: wasmtime when it answers, else Node.
fn runner() -> &'static str {
    let answers = |tool: &str| {
        Command::new(tool)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    };
    if answers("wasmtime") {
        "wasmtime"
    } else if answers("node") {
        "node"
    } else {
        panic!(
            "neither wasmtime nor node is installed: the recursion gate needs one to run wasm32-wasip1 tests"
        )
    }
}

/// Runs `recursion <args> --root <root>` (geom-test's binary, built in release by the workspace's
/// own cargo).
fn recursion(args: &[&str], root: &Path) -> Output {
    let workspace = repo::workspace_root();
    let output = Command::new(repo::cargo())
        .args([
            "run",
            "--locked",
            "--release",
            "--quiet",
            "--package",
            "geom-test",
            "--bin",
            "recursion",
            "--manifest-path",
        ])
        .arg(workspace.join("Cargo.toml"))
        .arg("--")
        .args(args)
        .arg("--root")
        .arg(root)
        .output()
        .unwrap_or_else(|e| panic!("cannot start cargo: {e}"));
    println!(
        "recursion {} --root {}: {}\n{}{}",
        args.join(" "),
        root.display(),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// The text of a run's output.
fn shown(output: &Output) -> String {
    format!(
        "{}\n{}{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The seeded crate's library: `walk`, recursive to `depth` levels, each frame holding `frame`
/// bytes; with the statement of its cycle when `stated`.
fn seeded_lib(stated: bool, depth: u32, frame: usize) -> String {
    let statement = if stated {
        format!(
            "//!\n//! Recursion:\n//!\n//! - Cycle: `walk`.\n//!   Bound: {depth} (`MAX_DEPTH`).\n\
             //!   Why: each call goes one level down, and `walk` stops at `MAX_DEPTH`.\n\
             //!   Test: `walk_at_its_bound`.\n"
        )
    } else {
        String::new()
    };
    format!(
        "//! A crate the recursion gate is seeded with.\n{statement}\
         /// The deepest `walk` goes.\npub const MAX_DEPTH: u32 = {depth};\n\
         /// Walks `n` levels down, holding {frame} bytes a level.\n\
         pub fn walk(n: u32) -> u32 {{\n    if n == 0 || n > MAX_DEPTH {{\n        return 0;\n    }}\n    \
         let buf = [1u8; {frame}];\n    core::hint::black_box(&buf);\n    \
         core::hint::black_box(walk(n - 1)) + u32::from(buf[0])\n}}\n"
    )
}

/// Writes a seeded workspace under `name`: one crate, `geom-seed`, whose library is `lib`, and its
/// `tests/recursion.rs` running `walk` at `depth`.
fn seeded_workspace(name: &str, lib: &str, depth: u32) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    let krate = root.join("projects/engines/geometry/crates/geom-seed");
    fs::create_dir_all(krate.join("src")).unwrap();
    fs::create_dir_all(krate.join("tests")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nresolver = \"3\"\nmembers = [\"projects/engines/geometry/crates/*\"]\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n\n[[package]]\nname = \"geom-seed\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fs::write(
        krate.join("Cargo.toml"),
        "[package]\nname = \"geom-seed\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n",
    )
    .unwrap();
    fs::write(krate.join("src/lib.rs"), lib).unwrap();
    fs::write(
        krate.join("tests/recursion.rs"),
        format!("//! The seeded bound's test.\n#[test]\nfn walk_at_its_bound() {{\n    assert_eq!(geom_seed::walk({depth}), {depth});\n}}\n"),
    )
    .unwrap();
    root
}

mod gate {
    use super::*;

    #[test]
    #[ignore = "runs wasm32-wasip1 tests under wasmtime or node, which the merge gate installs; run with --ignored"]
    fn gate_recursion_every_cycle_of_the_crates_that_ship_is_bounded_and_runs_on_the_small_stack() {
        let output = recursion(&["gate", "--runner", runner()], &repo::workspace_root());
        assert!(output.status.success(), "{}", shown(&output));
    }

    #[test]
    #[ignore = "runs wasm32-wasip1 tests under wasmtime or node, which the merge gate installs; run with --ignored"]
    fn gate_recursion_fails_on_seeded_violations() {
        // A recursive cycle without a statement fails the check.
        let root = seeded_workspace("recursion-unstated", &seeded_lib(false, 64, 16), 64);
        let output = recursion(&["check"], &root);
        assert!(!output.status.success(), "{}", shown(&output));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Unbounded"),
            "{}",
            shown(&output)
        );
        // A stated cycle whose test runs at its bound within the small stack passes the gate.
        let root = seeded_workspace("recursion-stated", &seeded_lib(true, 64, 16), 64);
        let output = recursion(&["gate", "--runner", runner()], &root);
        assert!(output.status.success(), "{}", shown(&output));
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("passed: walk_at_its_bound"),
            "{}",
            shown(&output)
        );
        // A stated bound whose test overflows the small stack (1000 levels of 1 KiB, past the
        // 256 KiB shadow stack) fails the gate.
        let root = seeded_workspace("recursion-overflow", &seeded_lib(true, 1000, 1024), 1000);
        let output = recursion(&["gate", "--runner", runner()], &root);
        assert!(!output.status.success(), "{}", shown(&output));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Stack"),
            "{}",
            shown(&output)
        );
    }
}
