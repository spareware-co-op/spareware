//! The ledger on disk: reading a workspace's map, licence texts and method notes, and writing or
//! checking the committed `PROVENANCE.md` and `NOTICE`.
//!
//! What it does: [`read`] builds the map of a workspace root ([`crate::map::repo::read`] and
//! [`crate::map::build`], with the known gaps) and reads every file directly under the licence
//! texts' folder and the method notes' folder; [`check`] builds the ledger and compares both files
//! with the committed ones; [`write()`] builds it and writes each file whose bytes differ.
//!
//! Guarantees:
//!
//! - **The map's own reading.** The records come from the map built exactly as the map's check
//!   builds it, so the ledger and `MAP.json` can never disagree about a header.
//! - **Every file, in a fixed order.** Each folder is listed in sorted order; a missing folder is
//!   an empty one; a file that is not UTF-8, and a folder or symbolic link inside either folder,
//!   are reported, never skipped.
//! - **`check` changes nothing**; `write` changes only `PROVENANCE.md` and `NOTICE`, and each only
//!   when its bytes differ.
//! - **I/O failures are errors, not passes**: [`MapError`], which the binary turns into exit
//!   code 2.
//!
//! Method: regenerate and compare, as [`crate::map::repo`] does for the map (chapter 27,
//! "Generated code and data have one home",
//! `projects/engines/geometry/architecture/27-crates-folders.md`).
//!
//! Cost: one build of the map, one read of each licence text and note, and one read and at most
//! one write of each generated file, plus the ledger's own cost ([`crate::ledger::build`]).

use std::fs;
use std::io;
use std::path::Path;

use crate::ledger::{
    Input, Ledger, NOTICE_FILE, NOTICES_DIR, PROVENANCE_FILE, Problem, build, compare,
};
use crate::map::provenance::METHODS_FOLDER;
use crate::map::repo::MapError;
use crate::map::{GAPS, Generated, SourceFile};

fn error(path: &Path, e: &io::Error) -> MapError {
    MapError {
        path: path.display().to_string(),
        message: e.to_string(),
    }
}

/// Every entry directly under `folder` (relative to `root`, ending in `/`), sorted by name: files
/// read as text, and a folder, symbolic link or file that is not UTF-8 recorded with why it cannot
/// be read. A missing folder has no entries.
///
/// # Errors
///
/// [`MapError`] if the folder exists but cannot be listed, or a file cannot be read.
pub fn files_in(root: &Path, folder: &str) -> Result<Vec<SourceFile>, MapError> {
    let dir = root.join(folder);
    let listing = match fs::read_dir(&dir) {
        Ok(listing) => listing,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(error(&dir, &e)),
    };
    let mut out = Vec::new();
    for entry in listing {
        let entry = entry.map_err(|e| error(&dir, &e))?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|e| error(&path, &e))?;
        let rel = format!("{folder}{}", entry.file_name().to_string_lossy());
        let text = if kind.is_symlink() {
            Err("a symbolic link or junction; keep the file itself here".to_owned())
        } else if kind.is_dir() {
            Err("a folder; the files here sit directly in the folder".to_owned())
        } else {
            let bytes = fs::read(&path).map_err(|e| error(&path, &e))?;
            String::from_utf8(bytes).map_err(|_| "the file is not UTF-8".to_owned())
        };
        out.push(SourceFile { path: rel, text });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// The ledger of the workspace at `root`.
///
/// # Errors
///
/// [`MapError`] if a folder or file cannot be read.
pub fn read(root: &Path) -> Result<Ledger, MapError> {
    let input = crate::map::repo::read(root)?;
    let Generated { map, problems, .. } = crate::map::build(&input, GAPS);
    let notices = files_in(root, NOTICES_DIR)?;
    let notes = files_in(root, METHODS_FOLDER)?;
    Ok(build(&Input {
        map: &map,
        map_problems: problems.len(),
        notices: &notices,
        notes: &notes,
    }))
}

/// A committed file's text, if it exists.
fn committed(root: &Path, file: &str) -> Result<Option<String>, MapError> {
    let path = root.join(file);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(error(&path, &e)),
    }
}

/// Builds the ledger of the workspace at `root` and compares it with the committed files: every
/// problem, and a stale-ledger problem for each file that is missing or differs, sorted.
///
/// # Errors
///
/// [`MapError`] if a folder or file cannot be read.
pub fn check(root: &Path) -> Result<Vec<Problem>, MapError> {
    let ledger = read(root)?;
    let mut problems = ledger.problems;
    for (file, text) in [
        (PROVENANCE_FILE, &ledger.provenance),
        (NOTICE_FILE, &ledger.notice),
    ] {
        problems.extend(compare(file, committed(root, file)?.as_deref(), text));
    }
    problems.sort();
    Ok(problems)
}

/// Builds the ledger of the workspace at `root` and writes each file whose bytes differ; returns
/// every problem and the files written.
///
/// # Errors
///
/// [`MapError`] if a folder or file cannot be read, or a file cannot be written.
pub fn write(root: &Path) -> Result<(Vec<Problem>, Vec<&'static str>), MapError> {
    let ledger = read(root)?;
    let mut written = Vec::new();
    for (file, text) in [
        (PROVENANCE_FILE, &ledger.provenance),
        (NOTICE_FILE, &ledger.notice),
    ] {
        let path = root.join(file);
        if fs::read(&path).ok().as_deref() == Some(text.as_bytes()) {
            continue;
        }
        fs::write(&path, text.as_bytes()).map_err(|e| error(&path, &e))?;
        written.push(file);
    }
    Ok((ledger.problems, written))
}
