//! The fixtures folder: every stored fixture, on disk and compiled in, and the list the
//! fails-on-base check takes (chapter 28.16; piece G97).
//!
//! What it does: [`read_dir`] reads every `.json` file under the fixtures folder ([`DIR`]) from a
//! working tree; [`embedded`] gives the copy compiled into this crate, which every fixture test
//! reads, on every target (WebAssembly test runs have no file system, as for the golden store);
//! [`parse_all`] parses a set of files and checks what no single fixture can: each sits at the
//! path its id names, and no id repeats. [`must_fail_on_base`] lists the fixtures a pull request
//! adds whose source records a fix.
//!
//! Guarantees:
//!
//! - **The compiled copy is the tree's**: `build.rs` walks the folder and reruns when it changes,
//!   and the binary's `check` compares the two, so a stale build cannot pass.
//! - **Every problem is listed**, not only the first.
//! - **Files in byte order**, so a run reads the same on every machine.
//!
//! Method: chapter 28.16 (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: linear in the size of the folder.

use std::fs;
use std::path::Path;

use super::Fixture;

/// The folder's path, relative to the workspace root, one component per entry.
pub const DIR: [&str; 6] = [
    "projects",
    "engines",
    "geometry",
    "crates",
    "geom-test",
    "fixtures",
];

include!(concat!(env!("OUT_DIR"), "/fixtures_embedded.rs"));

/// One fixture file: its path under the folder, with `/`, and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    /// Its path under the fixtures folder.
    pub path: String,
    /// Its text.
    pub text: String,
}

/// The files compiled into this crate.
#[must_use]
pub fn embedded() -> Vec<File> {
    EMBEDDED
        .iter()
        .map(|(path, text)| File {
            path: (*path).to_owned(),
            text: (*text).to_owned(),
        })
        .collect()
}

/// Reads `dir`'s `.json` files, deepest paths in byte order, into `out`.
fn walk(dir: &Path, prefix: &str, out: &mut Vec<File>) -> Result<(), String> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            walk(&path, &format!("{prefix}{name}/"), out)?;
        } else if path.extension().is_some_and(|e| e == "json") {
            let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            out.push(File {
                path: format!("{prefix}{name}"),
                text,
            });
        }
    }
    Ok(())
}

/// Reads every fixture file under `root`'s fixtures folder; none is an empty list, since the
/// folder may hold only its README.
///
/// # Errors
///
/// A file or folder that cannot be read.
pub fn read_dir(root: &Path) -> Result<Vec<File>, String> {
    let dir = DIR.iter().fold(root.to_path_buf(), |p, part| p.join(part));
    let mut files = Vec::new();
    walk(&dir, "", &mut files)?;
    Ok(files)
}

/// Parses `files`, and checks each fixture's path and that no id repeats.
///
/// # Errors
///
/// Every problem found, each said in words.
pub fn parse_all(files: &[File]) -> Result<Vec<Fixture>, Vec<String>> {
    let mut fixtures: Vec<Fixture> = Vec::new();
    let mut problems = Vec::new();
    for file in files {
        match Fixture::parse(&file.text) {
            Ok(fixture) => {
                if format!("{}.json", fixture.id) != file.path {
                    problems.push(format!(
                        "{}: its id is {}, so it belongs at {}.json",
                        file.path, fixture.id, fixture.id
                    ));
                } else if fixtures.iter().any(|f| f.id == fixture.id) {
                    problems.push(format!("{}: the id repeats", file.path));
                } else {
                    fixtures.push(fixture);
                }
            }
            Err(e) => problems.push(format!("{}: {e}", file.path)),
        }
    }
    if problems.is_empty() {
        Ok(fixtures)
    } else {
        Err(problems)
    }
}

/// The ids of `head`'s fixtures that `base` does not hold and that must fail on the merge base.
#[must_use]
pub fn must_fail_on_base(base: &[Fixture], head: &[Fixture]) -> Vec<String> {
    head.iter()
        .filter(|f| f.must_fail_on_base() && !base.iter().any(|b| b.id == f.id))
        .map(|f| f.id.clone())
        .collect()
}
