//! Compiles the fixtures folder into the crate: every fixture test reads this copy, on every target,
//! since WebAssembly test runs have no file system (src/fixture/repo.rs). Cargo reruns this whenever
//! a file under `fixtures/` changes.
//!
//! It also sets `cfg(unoptimised)` in a build at `opt-level` 0, as geom-fuzz's build script does:
//! the polyhedral corpus's reference tests take minutes unoptimised and are ignored there
//! (`tests/corpus_polyhedral.rs`). `debug_assertions` cannot say it, since every test leg of the
//! workflows turns debug assertions on in its optimised build too. Cargo gives a build script the
//! profile's optimisation level as `OPT_LEVEL` (the Cargo book, "Build Scripts", environment
//! variables), and `cargo::rustc-check-cfg` declares the name to the compiler's check of
//! configuration names, which CI denies a warning of.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

/// Every `.json` file under `dir`, each folder in byte order.
fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            walk(&path, &format!("{prefix}{name}/"), out)?;
        } else if path.extension().is_some_and(|e| e == "json") {
            out.push((format!("{prefix}{name}"), path));
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or(io::ErrorKind::NotFound)?);
    let dir = root.join("fixtures");
    println!("cargo:rerun-if-changed=fixtures");
    println!("cargo::rustc-check-cfg=cfg(unoptimised)");
    if env::var("OPT_LEVEL").is_ok_and(|level| level == "0") {
        println!("cargo::rustc-cfg=unoptimised");
    }
    let mut files = Vec::new();
    walk(&dir, "", &mut files)?;
    let mut text =
        String::from("/// Every fixture file: its path under the folder, and its text.\n");
    text.push_str("pub const EMBEDDED: &[(&str, &str)] = &[\n");
    for (path, file) in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        let file = file.display().to_string();
        writeln!(text, "    ({path:?}, include_str!({file:?})),").map_err(io::Error::other)?;
    }
    text.push_str("];\n");
    let out = PathBuf::from(env::var_os("OUT_DIR").ok_or(io::ErrorKind::NotFound)?);
    fs::write(out.join("fixtures_embedded.rs"), text)
}
