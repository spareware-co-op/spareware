//! `guard`: proves the toolchain and dependency guard trips (chapter 28.13; piece G20).
//!
//! ```text
//! cargo run --locked --release -p geom-test --bin guard -- prove [--target-dir <dir>]
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
//! determinism test against the changed copy.

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use geom_test::deps::cargo_metadata;
use geom_test::guard::{SIN_FILE, perturb_sin, verdict};
use geom_test::map::repo::workspace_root;

const USAGE: &str = "usage: guard prove [--target-dir <dir>]";

fn main() -> ExitCode {
    let args: Vec<String> = match std::env::args_os()
        .skip(1)
        .map(OsString::into_string)
        .collect()
    {
        Ok(args) => args,
        Err(bad) => return usage(&format!("argument {bad:?} is not Unicode")),
    };
    match args.as_slice() {
        [command] if command == "prove" => prove(None),
        [command, flag, dir] if command == "prove" && flag == "--target-dir" => {
            prove(Some(PathBuf::from(dir)))
        }
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

fn prove(target_dir: Option<PathBuf>) -> ExitCode {
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
    let run = Command::new(&cargo)
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
        .arg(base.join("target"))
        .output();
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
