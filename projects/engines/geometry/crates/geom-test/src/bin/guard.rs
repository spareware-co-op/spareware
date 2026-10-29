//! `guard`: proves the toolchain and dependency guard trips (chapter 28.13; piece G20).
//!
//! ```text
//! cargo run --locked --release -p geom-test --bin guard -- prove [--target-dir <dir>]
//!     [--golden-run <golden run arguments>...]
//! ```
//!
//! What it does: `prove` finds the `libm` the workspace resolves (`cargo metadata --locked`),
//! copies its crate into `<dir>/guard/libm-<version>`, applies [`geom_test::guard::perturb_sin`]
//! (`sin`'s last bit flipped), and runs `cargo test --release -p geom-num --test determinism`
//! with that copy patched in for `libm` (`--config patch.crates-io.libm.path=...`), building in
//! `<dir>/guard/target` so the workspace's own build is untouched. It prints the run's output and
//! then [`geom_test::guard::verdict`]'s answer: the golden `geom-num/determinism` must report a
//! digest that differs from the store's. `<dir>` defaults to the workspace's `target` folder.
//!
//! With `--golden-run`, the changed `libm` goes through the golden store's own tool instead (gate
//! 1, piece G43), so a CI leg that runs `golden run` is shown to fail on a changed digest: the
//! WebAssembly legs give `--golden-run --target wasm32-wasip1 --release`. `prove` builds the
//! binary `golden` (in release, in the workspace's target folder), resolves the workspace once
//! with the patch (`cargo metadata`, which writes the patch into `Cargo.lock`, so the `--locked`
//! builds `golden run` makes accept it), and runs `golden run <arguments> --config <the patch>
//! --target-dir <dir>/guard/target`. The verdict is the same.
//!
//! Guarantees: the workspace's `Cargo.lock`, which Cargo rewrites while a patch is in force, is
//! restored byte for byte after the run, whether the run passed, failed or could not start; the
//! copy of `libm` lives under the target folder, never in the source tree. Exit codes: 0 when the
//! guard tripped as it must; 1 when it did not (the change moved no golden, the patch went unused,
//! or the run failed for another reason); 2 for a usage error or a step that cannot be done.
//!
//! Method: none; it runs Cargo and calls [`geom_test::guard`], whose module header carries the
//! method.
//!
//! Cost: one `cargo metadata`, a copy of `libm`'s crate, and one build and run of `geom-num`'s
//! determinism test against the changed copy; with `--golden-run`, a build of `golden`, a second
//! `cargo metadata`, and one build and run of every golden's test target instead.

#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output, Stdio};

use geom_test::deps::cargo_metadata;
use geom_test::guard::{SIN_FILE, perturb_sin, verdict};
use geom_test::map::repo::workspace_root;

const USAGE: &str =
    "usage: guard prove [--target-dir <dir>] [--golden-run <golden run arguments>...]";

/// What the changed `libm` is run through.
enum Through {
    /// `cargo test --release -p geom-num --test determinism`.
    Determinism,
    /// `golden run` with these arguments.
    GoldenRun(Vec<String>),
}

fn main() -> ExitCode {
    let args: Vec<String> = match std::env::args_os()
        .skip(1)
        .map(OsString::into_string)
        .collect()
    {
        Ok(args) => args,
        Err(bad) => return usage(&format!("argument {bad:?} is not Unicode")),
    };
    let Some((command, rest)) = args.split_first() else {
        return usage("no command");
    };
    if command != "prove" {
        return usage("unknown arguments");
    }
    let (options, through) = match rest.iter().position(|a| a == "--golden-run") {
        Some(at) => {
            let (options, golden) = rest.split_at(at);
            let golden = golden.get(1..).unwrap_or_default().to_vec();
            (options, Through::GoldenRun(golden))
        }
        None => (rest, Through::Determinism),
    };
    match options {
        [] => prove(None, &through),
        [flag, dir] if flag == "--target-dir" => prove(Some(PathBuf::from(dir)), &through),
        _ => usage("unknown arguments"),
    }
}

fn usage(message: &str) -> ExitCode {
    eprintln!("guard: {message}\n{USAGE}");
    ExitCode::from(2)
}

fn fail(message: &str) -> ExitCode {
    eprintln!("guard: {message}");
    ExitCode::from(2)
}

fn prove(target_dir: Option<PathBuf>, through: &Through) -> ExitCode {
    let root = match workspace_root() {
        Ok(root) => root,
        Err(e) => return fail(&e.to_string()),
    };
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let metadata = match cargo_metadata(&cargo, &root.join("Cargo.toml")) {
        Ok(m) => m,
        Err(e) => return fail(&e.to_string()),
    };
    let (version, source) = match libm_crate(&metadata) {
        Ok(found) => found,
        Err(m) => return fail(&m),
    };
    let base = target_dir
        .unwrap_or_else(|| root.join("target"))
        .join("guard");
    let copy = base.join(format!("libm-{version}"));
    if let Err(e) = fresh_copy(&source, &copy) {
        return fail(&format!(
            "cannot copy {} to {}: {e}",
            source.display(),
            copy.display()
        ));
    }
    let sin = copy.join(SIN_FILE);
    let changed = match fs::read_to_string(&sin).map(|text| perturb_sin(&text)) {
        Ok(Ok(changed)) => changed,
        Ok(Err(e)) => return fail(&e.to_string()),
        Err(e) => return fail(&format!("cannot read {}: {e}", sin.display())),
    };
    if let Err(e) = fs::write(&sin, changed) {
        return fail(&format!("cannot write {}: {e}", sin.display()));
    }
    let lock = root.join("Cargo.lock");
    let saved = match fs::read(&lock) {
        Ok(bytes) => bytes,
        Err(e) => return fail(&format!("cannot read {}: {e}", lock.display())),
    };
    // A TOML literal string: the path is taken as written, backslashes included.
    let patch = format!("patch.crates-io.libm.path='{}'", copy.display());
    let target = base.join("target");
    let run = match through {
        Through::Determinism => Command::new(&cargo)
            .current_dir(&root)
            .args([
                "test",
                "--release",
                "-p",
                "geom-num",
                "--test",
                "determinism",
            ])
            .arg("--config")
            .arg(&patch)
            .arg("--target-dir")
            .arg(&target)
            .output(),
        Through::GoldenRun(arguments) => golden_run(&cargo, &root, &patch, &target, arguments),
    };
    let restored = fs::write(&lock, &saved);
    let out = match run {
        Ok(out) => out,
        Err(e) => return fail(&format!("cannot run cargo: {e}")),
    };
    if let Err(e) = restored {
        return fail(&format!("cannot restore {}: {e}", lock.display()));
    }
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    println!("{output}");
    match verdict(out.status.success(), &output) {
        Ok(()) => {
            println!(
                "guard: libm {version}'s sin changed by one bit, and golden geom-num/determinism \
                 failed: the guard trips"
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("guard: {e}");
            ExitCode::from(1)
        }
    }
}

/// Runs `golden run <arguments>` against the changed `libm`: builds `golden` with the lock as it
/// is, writes the patch into the lock by resolving once with it, and gives each of `golden run`'s
/// `cargo test` runs the patch and the guard's target folder.
fn golden_run(
    cargo: &OsStr,
    root: &Path,
    patch: &str,
    target: &Path,
    arguments: &[String],
) -> io::Result<Output> {
    let golden = golden_binary(cargo, root)?;
    let resolved = Command::new(cargo)
        .current_dir(root)
        .args(["metadata", "--format-version", "1", "--config", patch])
        .stdout(Stdio::null())
        .output()?;
    if !resolved.status.success() {
        return Err(io::Error::other(format!(
            "cargo metadata with the patch failed: {}",
            String::from_utf8_lossy(&resolved.stderr)
        )));
    }
    Command::new(golden)
        .current_dir(root)
        .arg("run")
        .args(arguments)
        .arg("--config")
        .arg(patch)
        .arg("--target-dir")
        .arg(target)
        .output()
}

/// Builds the binary `golden` in release and returns its path, read from Cargo's JSON messages.
fn golden_binary(cargo: &OsStr, root: &Path) -> io::Result<PathBuf> {
    let built = Command::new(cargo)
        .current_dir(root)
        .args([
            "build",
            "--locked",
            "--release",
            "-p",
            "geom-test",
            "--bin",
            "golden",
            "--message-format",
            "json",
        ])
        .output()?;
    if !built.status.success() {
        return Err(io::Error::other(format!(
            "cargo build of golden failed: {}",
            String::from_utf8_lossy(&built.stderr)
        )));
    }
    String::from_utf8_lossy(&built.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|m| {
            m.get("reason").and_then(serde_json::Value::as_str) == Some("compiler-artifact")
                && m.pointer("/target/name")
                    .and_then(serde_json::Value::as_str)
                    == Some("golden")
        })
        .and_then(|m| m.get("executable")?.as_str().map(PathBuf::from))
        .ok_or_else(|| io::Error::other("cargo built no executable named golden"))
}

/// The version and crate folder of the one `libm` from crates.io in the resolved graph.
fn libm_crate(metadata: &serde_json::Value) -> Result<(String, PathBuf), String> {
    let packages = metadata["packages"]
        .as_array()
        .ok_or("cargo metadata has no packages list")?;
    let found: Vec<&serde_json::Value> = packages
        .iter()
        .filter(|p| {
            p["name"] == "libm"
                && p["source"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("registry+"))
        })
        .collect();
    let [libm] = found.as_slice() else {
        return Err(format!(
            "the graph holds {} libm crates from a registry, not one",
            found.len()
        ));
    };
    let version = libm["version"]
        .as_str()
        .ok_or("libm has no version")?
        .to_owned();
    let folder = libm["manifest_path"]
        .as_str()
        .map(Path::new)
        .and_then(Path::parent)
        .ok_or("libm has no manifest path")?
        .to_path_buf();
    Ok((version, folder))
}

/// Copies the folder `from` to `to`, replacing whatever was at `to`.
fn fresh_copy(from: &Path, to: &Path) -> io::Result<()> {
    if to.exists() {
        fs::remove_dir_all(to)?;
    }
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fresh_copy(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
