//! The banned-source scan of a workspace: every file of the kernel's crates and of the facade's,
//! scanned, with the scanner's own files as named exemptions.
//!
//! What it does: [`files`] lists every file under the kernel's crates folder and under each
//! facade crate (`engine-geometry` and its binding crates), skipping build output; [`check`] scans
//! each with [`crate::scan::scan_text`] and returns every [`Finding`]: a hit, a file that cannot be
//! read as a file, or an exemption that no longer applies.
//!
//! Guarantees:
//!
//! - **Every file, every kind.** Every regular file under the scanned folders is read, whatever its
//!   name or extension (source, manifest, data, fixture), in every reading [`readings`] gives: as
//!   UTF-8 with any invalid byte replaced, so a marker in ASCII is found even in a binary file; as
//!   UTF-16 when the file opens with a UTF-16 byte-order mark; and, when it holds a zero byte,
//!   again with its zero bytes dropped, so text stored as UTF-16 without a mark is read too. A
//!   file's hits are those of all its readings. Folders named `target` (build output) are the only
//!   ones skipped. A symbolic link or junction is reported, never followed or skipped, and so is a
//!   folder deeper than [`MAX_DEPTH`].
//! - **Exemptions are named, few and cannot rot.** A file in [`EXEMPT`] (or the list a test
//!   passes) is not reported, but each exemption must name a file that exists and still holds a
//!   hit; otherwise the exemption itself is a finding. Only the scanner's own marker tables and
//!   its seeded tests are exempt.
//! - **Deterministic order.** Files are scanned in path order and findings sorted by file, line
//!   and text, with paths relative to the workspace root and `/`, the same on every system.
//! - **I/O failures are errors, not passes**: [`MapError`], which the binary turns into exit code
//!   2.
//!
//! Method: the same walk as the map's ([`crate::map::repo`]): sorted listings, no recursion, links
//! reported; the scope is the code chapter 29.20's rule 7 covers, every crate of the kernel
//! (`projects/engines/geometry/architecture/29-sources-licences.md`).
//!
//! Cost: one listing per folder and one read of each file, plus the scan's own cost, linear in the
//! files' total size.

use core::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::map::repo::MapError;
use crate::map::{CRATES_DIR, FACADE_DIR, is_facade};
use crate::scan::{Hit, scan_text};

/// The deepest folder below a scanned crate folder that is read.
pub const MAX_DEPTH: usize = 16;

/// The folder name that holds build output, which is not scanned.
pub const BUILD_OUTPUT: &str = "target";

/// A file the scan does not report, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exempt {
    /// The file, relative to the workspace root.
    pub file: &'static str,
    /// Why its hits are not code taken from a banned source.
    pub reason: &'static str,
}

/// The scanner's own files, which hold every marker on purpose.
pub const EXEMPT: &[Exempt] = &[
    Exempt {
        file: "projects/engines/geometry/crates/geom-test/src/scan.rs",
        reason: "the scanner itself: its documentation and advice name the categories and sources \
                 it looks for and give examples of what it finds",
    },
    Exempt {
        file: "projects/engines/geometry/crates/geom-test/src/scan/markers.rs",
        reason: "the scanner's marker tables: each marker is written here once, as the thing to \
                 find, with the rule or evidence report that names it",
    },
    Exempt {
        file: "projects/engines/geometry/crates/geom-test/tests/scan_seeded.rs",
        reason: "the scanner's seeded positives: each marker is planted on purpose, to show that \
                 the scanner finds it",
    },
    Exempt {
        file: "projects/engines/geometry/crates/geom-test/tests/attack_g16.rs",
        reason: "piece G16's attack tests: they plant markers in every disguise, to try to slip one \
                 past the scanner",
    },
];

/// What a finding is.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A marker.
    Hit(Hit),
    /// A path the scan cannot read as a file: a link, or a folder too deep.
    Unreadable(String),
    /// An exemption that names a file that is gone or holds no hit any more.
    StaleExemption(String),
}

/// One finding, in one file.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    /// The file, relative to the workspace root, with `/`.
    pub file: String,
    /// What was found.
    pub kind: Kind,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            Kind::Hit(hit) => write!(f, "{}:{hit}", self.file),
            Kind::Unreadable(why) => write!(f, "{}: Unreadable: {why}", self.file),
            Kind::StaleExemption(why) => write!(f, "{}: Stale exemption: {why}", self.file),
        }
    }
}

fn error(path: &Path, e: &io::Error) -> MapError {
    MapError {
        path: path.display().to_string(),
        message: e.to_string(),
    }
}

/// One scanned file: its path relative to the root, and its readings ([`readings`]) or why it
/// cannot be read.
pub type Scanned = (String, Result<Vec<String>, String>);

/// Every reading of a file's bytes as text: UTF-8 with invalid bytes replaced; UTF-16 (little- or
/// big-endian) when the bytes open with its byte-order mark; and UTF-8 with every zero byte
/// dropped when there is one, which reads ASCII text stored as UTF-16 without a mark.
#[must_use]
pub fn readings(bytes: &[u8]) -> Vec<String> {
    let mut out = vec![String::from_utf8_lossy(bytes).into_owned()];
    let utf16 = |rest: &[u8], unit: fn([u8; 2]) -> u16| -> String {
        let units = rest
            .chunks_exact(2)
            .map(|pair| unit([pair.first().copied().unwrap_or(0), pair.get(1).copied().unwrap_or(0)]));
        char::decode_utf16(units)
            .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect()
    };
    if let Some(rest) = bytes.strip_prefix(&[0xff, 0xfe]) {
        out.push(utf16(rest, u16::from_le_bytes));
    } else if let Some(rest) = bytes.strip_prefix(&[0xfe, 0xff]) {
        out.push(utf16(rest, u16::from_be_bytes));
    } else if bytes.contains(&0) {
        let kept: Vec<u8> = bytes.iter().copied().filter(|b| *b != 0).collect();
        out.push(String::from_utf8_lossy(&kept).into_owned());
    }
    out
}

/// Every file under `folder` (relative to `root`), in path order, skipping build output.
fn walk(root: &Path, folder: &str, out: &mut Vec<Scanned>) -> Result<(), MapError> {
    let mut stack: Vec<(PathBuf, String, usize)> = vec![(root.join(folder), folder.to_owned(), 0)];
    while let Some((dir, rel, depth)) = stack.pop() {
        let listing = fs::read_dir(&dir).map_err(|e| error(&dir, &e))?;
        let mut entries = Vec::new();
        for entry in listing {
            let entry = entry.map_err(|e| error(&dir, &e))?;
            let kind = entry.file_type().map_err(|e| error(&entry.path(), &e))?;
            entries.push((entry.file_name().to_string_lossy().into_owned(), entry.path(), kind));
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, path, kind) in entries {
            let child = format!("{rel}/{name}");
            if kind.is_symlink() {
                out.push((
                    child,
                    Err("a symbolic link or junction; the scan reads files and folders only, so \
                         keep what it points to here"
                        .to_owned()),
                ));
            } else if kind.is_dir() {
                if name == BUILD_OUTPUT {
                    continue;
                }
                if depth >= MAX_DEPTH {
                    out.push((child, Err(format!("a folder deeper than {MAX_DEPTH} levels"))));
                } else {
                    stack.push((path, child, depth.saturating_add(1)));
                }
            } else {
                let bytes = fs::read(&path).map_err(|e| error(&path, &e))?;
                out.push((child, Ok(readings(&bytes))));
            }
        }
    }
    Ok(())
}

/// Every file the scan reads in the workspace at `root`: the kernel's crates folder, and each
/// facade crate's folder when the workspace has the shared engines' folder.
///
/// # Errors
///
/// [`MapError`] if a folder cannot be listed or a file cannot be read.
pub fn files(root: &Path) -> Result<Vec<Scanned>, MapError> {
    let mut out = Vec::new();
    walk(root, CRATES_DIR, &mut out)?;
    let facades = root.join(FACADE_DIR);
    if facades.is_dir() {
        let listing = fs::read_dir(&facades).map_err(|e| error(&facades, &e))?;
        let mut names = Vec::new();
        for entry in listing {
            let entry = entry.map_err(|e| error(&facades, &e))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_facade(&name) {
                names.push((name, entry.file_type().map_err(|e| error(&entry.path(), &e))?));
            }
        }
        names.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, kind) in names {
            let folder = format!("{FACADE_DIR}/{name}");
            if kind.is_symlink() {
                out.push((folder, Err("a crate folder that is a symbolic link or junction".to_owned())));
            } else if kind.is_dir() {
                walk(root, &folder, &mut out)?;
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Scans `files` with `exempt` as the exemptions: every hit outside them, every unreadable path,
/// and every exemption that no longer applies, sorted.
#[must_use]
pub fn scan_files(files: &[Scanned], exempt: &[Exempt]) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut applied = vec![false; exempt.len()];
    for (path, text) in files {
        let texts = match text {
            Ok(texts) => texts,
            Err(why) => {
                findings.push(Finding {
                    file: path.clone(),
                    kind: Kind::Unreadable(why.clone()),
                });
                continue;
            }
        };
        let mut hits: Vec<Hit> = texts.iter().flat_map(|t| scan_text(t)).collect();
        hits.sort();
        hits.dedup();
        match exempt.iter().position(|e| e.file == path) {
            Some(k) => {
                if let Some(flag) = applied.get_mut(k) {
                    *flag = !hits.is_empty();
                }
            }
            None => findings.extend(hits.into_iter().map(|hit| Finding {
                file: path.clone(),
                kind: Kind::Hit(hit),
            })),
        }
    }
    for (e, was_applied) in exempt.iter().zip(&applied) {
        if !was_applied {
            findings.push(Finding {
                file: e.file.to_owned(),
                kind: Kind::StaleExemption(
                    "the exempt file is gone or holds no marker any more: delete its entry from \
                     scan::repo::EXEMPT"
                        .to_owned(),
                ),
            });
        }
    }
    findings.sort();
    findings
}

/// Scans the workspace at `root` with `exempt` as the exemptions ([`EXEMPT`] for this workspace).
///
/// # Errors
///
/// [`MapError`] if a folder cannot be listed or a file cannot be read.
pub fn check(root: &Path, exempt: &[Exempt]) -> Result<Vec<Finding>, MapError> {
    if !root.join(CRATES_DIR).is_dir() {
        return Err(MapError {
            path: root.join(CRATES_DIR).display().to_string(),
            message: "is not a folder: not the workspace root".to_owned(),
        });
    }
    Ok(scan_files(&files(root)?, exempt))
}
