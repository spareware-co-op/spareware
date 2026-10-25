//! The small WebAssembly stack: every stated bound's test run there, and probes that show the
//! stack's limits are in force.
//!
//! What it does: [`run_bound_tests`] runs each crate's `tests/recursion.rs` for `wasm32-wasip1`,
//! in debug and in release, linked with a shadow stack of [`STACK_BYTES`] in linear memory
//! (`-C link-arg=-zstack-size=...`) and run by a WebAssembly engine whose own stack for
//! WebAssembly frames is limited to [`ENGINE_STACK_BYTES`] ([`Runner`]), and checks that every
//! test a statement names ran and passed. [`probes`] builds a small crate whose tests overflow
//! each of the two stacks and requires them to trap with the small limits and pass with
//! [`CONTROL_BYTES`], so a limit that was not applied fails the check instead of letting every
//! bound pass.
//!
//! The two stacks are WebAssembly's two: the shadow stack in linear memory, which Rust's
//! WebAssembly targets place first in memory (rustc's `--stack-first`), so that running past its
//! start wraps to an address beyond memory and traps; and the engine's stack of WebAssembly frames,
//! which wasmtime limits with `max-wasm-stack` and Node's V8 by the size of the thread it runs on.
//! Either overflowing ends the test program with a trap, which fails the run.
//!
//! The sizes, settled here as chapter 31.7 asked ("Set the stack the recursion check tests on,
//! and each call's share"): one call into the kernel may use at most a quarter of the 1 MiB shadow
//! stack Rust gives a WebAssembly program, 256 KiB, and the same of the engine's stack (half of
//! wasmtime's default 512 KiB), leaving the rest to the program and bindings that call it. The
//! bound tests run with exactly that, the test harness's own frames counted against it; the
//! predicates' 32 KiB a call (`geom-num`'s `tests/attack2_predicate.rs`) and `ExactSum`'s 41 KB
//! fit inside it.
//!
//! Guarantees:
//!
//! - **A named test that did not run is a failure**, as is one that failed or a run that trapped,
//!   so a test skipped, ignored or renamed cannot pass the check.
//! - **The limits are shown to be in force**: the probes trap with them and pass without them, in
//!   the same build and runner as the bound tests, or the check fails.
//! - **Nothing outside steers the build**: each run starts without the variables that change the
//!   compiler, its flags or the target (`SCRUBBED_ENV`), builds in its own folder and uses
//!   `--locked`.
//!
//! Method: wasm-ld's `-z stack-size` (<https://lld.llvm.org/WebAssembly.html>); wasmtime's
//! `-W max-wasm-stack` (<https://docs.wasmtime.dev/cli-options.html>); Node's `worker_threads`
//! `resourceLimits.stackSizeMb` and `node:wasi` (<https://nodejs.org/api/worker_threads.html>,
//! <https://nodejs.org/api/wasi.html>); cargo's target runner
//! (<https://doc.rust-lang.org/cargo/reference/config.html#targettriplerunner>); libtest's
//! output lines (`test <name> ... ok`).
//!
//! Cost: two builds of each crate's bound tests for `wasm32-wasip1` and their runs, and four
//! builds and runs of the probe crate.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::recursion::{Problem, Rule};

/// The shadow stack the bound tests are linked with: 256 KiB.
pub const STACK_BYTES: u32 = 256 * 1024;

/// The engine's stack for WebAssembly frames the bound tests run with: 256 KiB.
pub const ENGINE_STACK_BYTES: u32 = 256 * 1024;

/// The stacks the probes' control runs use, large enough for both probes: 4 MiB.
pub const CONTROL_BYTES: u32 = 4 * 1024 * 1024;

/// The target the bound tests run on.
pub const TARGET: &str = "wasm32-wasip1";

/// The environment variables every run starts without.
pub const SCRUBBED_ENV: [&str; 10] = [
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTFLAGS",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTC",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_TARGET",
    "CARGO_TARGET_WASM32_WASIP1_RUSTFLAGS",
];

/// How deep the shadow-stack probe goes: 128 frames of 4 KiB, 512 KiB.
pub const SHADOW_PROBE_DEPTH: u32 = 128;

/// How deep the engine-stack probe goes: 50 000 frames, each at least 16 bytes of engine stack
/// (a return address and a frame pointer) and at most 80, so 0.8 to 4 MB: past 256 KiB, within
/// the control's 4 MiB.
pub const ENGINE_PROBE_DEPTH: u32 = 50_000;

/// The WebAssembly engine that runs the tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Runner {
    /// wasmtime, with `-W max-wasm-stack`: the check's engine in CI.
    Wasmtime,
    /// Node's WASI in a worker thread whose stack is the limit: for machines without wasmtime.
    Node,
}

/// A build profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// `cargo test`.
    Debug,
    /// `cargo test --release`.
    Release,
}

impl Profile {
    fn name(self) -> &'static str {
        match self {
            Profile::Debug => "debug",
            Profile::Release => "release",
        }
    }
}

/// The Node script that runs a WASI program in a worker thread with a given stack.
const NODE_SCRIPT: &str = r"// Runs a wasm32-wasip1 program under Node's WASI in a worker thread whose stack is the first
// argument, in bytes: cargo's runner for the recursion check (geom-test, src/recursion/stack.rs).
import { readFile } from 'node:fs/promises';
import { WASI } from 'node:wasi';
import { Worker, isMainThread, workerData, parentPort } from 'node:worker_threads';
import { argv, env, exit } from 'node:process';

if (isMainThread) {
  const [bytes, file, ...args] = argv.slice(2);
  const worker = new Worker(new URL(import.meta.url), {
    workerData: { file, args },
    resourceLimits: { stackSizeMb: Number(bytes) / 1048576 },
  });
  worker.on('message', (code) => exit(code));
  worker.on('error', (e) => { console.error(String(e)); exit(134); });
  worker.on('exit', (code) => { if (code !== 0) exit(code); });
} else {
  const { file, args } = workerData;
  const wasi = new WASI({ version: 'preview1', args: [file, ...args], env, returnOnExit: true });
  const module = await WebAssembly.compile(await readFile(file));
  const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
  parentPort.postMessage(wasi.start(instance));
}
";

/// The cargo runner command for `runner` with an engine stack of `bytes`.
fn runner_command(runner: Runner, bytes: u32, scratch: &Path) -> Result<String, String> {
    match runner {
        Runner::Wasmtime => Ok(format!("wasmtime run -W max-wasm-stack={bytes}")),
        Runner::Node => {
            fs::create_dir_all(scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
            let script = scratch.join("wasi-run.mjs");
            fs::write(&script, NODE_SCRIPT).map_err(|e| format!("{}: {e}", script.display()))?;
            let shown = script.display().to_string().replace('\\', "/");
            if shown.contains(' ') {
                return Err(format!(
                    "the runner script's path {shown} has a space, which cargo's runner splits"
                ));
            }
            Ok(format!("node --no-warnings {shown} {bytes}"))
        }
    }
}

/// One run of `cargo test` for the bound tests or a probe.
#[derive(Clone, Debug)]
pub struct Run {
    /// Whether cargo and the tests succeeded.
    pub success: bool,
    /// The tests that passed.
    pub passed: Vec<String>,
    /// Everything printed, for the report.
    pub output: String,
}

/// Runs `cargo test` for `TARGET` on `manifest`, package `package`, test target `test`, with the
/// given stacks, in `target_dir`; `filter` selects tests.
#[allow(clippy::too_many_arguments, reason = "each is one setting of the run")]
fn cargo_test(
    manifest: &Path,
    package: Option<&str>,
    profile: Profile,
    runner: Runner,
    shadow: u32,
    engine: u32,
    target_dir: &Path,
    filter: Option<&str>,
) -> Result<Run, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .arg("test")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(manifest)
        .args(["--test", "recursion", "--target", TARGET]);
    if let Some(p) = package {
        command.args(["--package", p]);
    }
    if profile == Profile::Release {
        command.arg("--release");
    }
    command.args(["--", "--test-threads", "1"]);
    if let Some(f) = filter {
        command.args(["--exact", f]);
    }
    for name in SCRUBBED_ENV {
        command.env_remove(name);
    }
    command
        .env("CARGO_TARGET_DIR", target_dir)
        .env("RUSTFLAGS", format!("-C link-arg=-zstack-size={shadow}"))
        .env(
            "CARGO_TARGET_WASM32_WASIP1_RUNNER",
            runner_command(runner, engine, &target_dir.join("runner"))?,
        );
    let out = command
        .output()
        .map_err(|e| format!("cannot start cargo: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let passed = stdout
        .lines()
        .filter_map(|l| {
            l.strip_prefix("test ")
                .and_then(|r| r.strip_suffix(" ... ok"))
        })
        .map(str::to_owned)
        .collect();
    Ok(Run {
        success: out.status.success(),
        passed,
        output: format!("{stdout}{stderr}"),
    })
}

/// The last lines of an output, for a message.
fn tail(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .skip(lines.len().saturating_sub(25))
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Runs every crate's bound tests on the small stack, in debug and release: `crates` gives each
/// crate's package, its `tests/recursion.rs`, and the tests its statements name.
#[must_use]
pub fn run_bound_tests(
    root: &Path,
    crates: &[(String, String, Vec<String>)],
    runner: Runner,
    target_dir: &Path,
) -> Vec<Problem> {
    let mut problems = Vec::new();
    let manifest = root.join("Cargo.toml");
    for (package, file, tests) in crates {
        for profile in [Profile::Debug, Profile::Release] {
            println!(
                "recursion stack: {package} tests/recursion.rs, {}, {TARGET}, shadow stack {STACK_BYTES} bytes, engine stack {ENGINE_STACK_BYTES} bytes",
                profile.name()
            );
            let run = cargo_test(
                &manifest,
                Some(package),
                profile,
                runner,
                STACK_BYTES,
                ENGINE_STACK_BYTES,
                target_dir,
                None,
            );
            match run {
                Err(e) => problems.push(Problem::new(file, 1, Rule::Stack, e)),
                Ok(run) => {
                    let missing: Vec<&String> =
                        tests.iter().filter(|t| !run.passed.contains(t)).collect();
                    if !run.success || !missing.is_empty() {
                        problems.push(Problem::new(
                            file,
                            1,
                            Rule::Stack,
                            format!(
                                "the bound tests failed on the small stack in {} (not passed: {}):\n{}",
                                profile.name(),
                                if missing.is_empty() {
                                    "a test the statements do not name".to_owned()
                                } else {
                                    missing.iter().map(|t| format!("`{t}`")).collect::<Vec<_>>().join(", ")
                                },
                                tail(&run.output)
                            ),
                        ));
                    } else {
                        println!("  passed: {}", run.passed.join(", "));
                    }
                }
            }
        }
    }
    problems
}

/// The probe crate's files: a library and its `tests/recursion.rs`.
fn write_probe(dir: &Path) -> Result<PathBuf, String> {
    let write = |rel: &str, text: &str| -> Result<(), String> {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
    };
    write(
        "Cargo.toml",
        "[package]\nname = \"geom-recursion-probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n",
    )?;
    write(
        "Cargo.lock",
        "version = 4\n\n[[package]]\nname = \"geom-recursion-probe\"\nversion = \"0.0.0\"\n",
    )?;
    write(
        "src/lib.rs",
        "//! The recursion check's probe: recursion that overflows each WebAssembly stack.\n\
         use std::hint::black_box;\n\n\
         /// Recursion holding 4 KiB of linear memory a frame, for the shadow stack.\n\
         #[inline(never)]\n\
         pub fn shadow(depth: u32) -> u64 {\n    \
             let mut buf = [0u8; 4096];\n    \
             black_box(&mut buf);\n    \
             if depth == 0 {\n        return u64::from(buf[0]);\n    }\n    \
             let below = shadow(black_box(depth - 1));\n    \
             black_box(&buf);\n    \
             below + u64::from(buf[1])\n}\n\n\
         /// Recursion with nothing in linear memory, for the engine's stack.\n\
         #[inline(never)]\n\
         pub fn engine(depth: u32) -> u64 {\n    \
             if depth == 0 {\n        return 0;\n    }\n    \
             black_box(engine(black_box(depth - 1))) + 1\n}\n",
    )?;
    write(
        "tests/recursion.rs",
        &format!(
            "//! Probes: each test overflows one stack at its small size.\n\
             #[test]\nfn shadow_probe() {{\n    assert_eq!(geom_recursion_probe::shadow({SHADOW_PROBE_DEPTH}), 0);\n}}\n\n\
             #[test]\nfn engine_probe() {{\n    assert_eq!(geom_recursion_probe::engine({ENGINE_PROBE_DEPTH}), u64::from({ENGINE_PROBE_DEPTH}u32));\n}}\n"
        ),
    )?;
    Ok(dir.join("Cargo.toml"))
}

/// Shows the small stacks are in force: each probe traps with them and passes with the control
/// stacks, in release, with `runner`.
#[must_use]
pub fn probes(runner: Runner, target_dir: &Path) -> Vec<Problem> {
    let mut problems = Vec::new();
    let file = "projects/engines/geometry/crates/geom-test/src/recursion/stack.rs";
    let dir = target_dir.join("probe");
    let manifest = match write_probe(&dir) {
        Ok(m) => m,
        Err(e) => return vec![Problem::new(file, 1, Rule::Stack, e)],
    };
    let probe_target = target_dir.join("probe-target");
    for (test, shadow, engine, what) in [
        ("shadow_probe", STACK_BYTES, CONTROL_BYTES, "shadow stack"),
        (
            "engine_probe",
            CONTROL_BYTES,
            ENGINE_STACK_BYTES,
            "engine stack",
        ),
    ] {
        println!("recursion stack: probe {test} must trap with the small {what}");
        let small = cargo_test(
            &manifest,
            None,
            Profile::Release,
            runner,
            shadow,
            engine,
            &probe_target,
            Some(test),
        );
        let control = cargo_test(
            &manifest,
            None,
            Profile::Release,
            runner,
            CONTROL_BYTES,
            CONTROL_BYTES,
            &probe_target,
            Some(test),
        );
        match (small, control) {
            (Err(e), _) | (_, Err(e)) => problems.push(Problem::new(file, 1, Rule::Stack, e)),
            (Ok(small), Ok(control)) => {
                if small.success || small.passed.iter().any(|t| t == test) {
                    problems.push(Problem::new(
                        file,
                        1,
                        Rule::Stack,
                        format!("the {what} probe passed with the small {what}: the limit is not in force"),
                    ));
                }
                if !control.success || !control.passed.iter().any(|t| t == test) {
                    problems.push(Problem::new(
                        file,
                        1,
                        Rule::Stack,
                        format!(
                            "the {what} probe failed with the control stacks, so its trap with the small \
                             one shows nothing:\n{}",
                            tail(&control.output)
                        ),
                    ));
                }
            }
        }
    }
    problems
}
