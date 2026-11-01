//! The format museum: sample files and operation logs of every released format version and
//! edition, each opened and replayed to its recorded hash forever (chapter 28.13; 24.6's archive
//! corpus; piece G99).
//!
//! What it does: `museum/catalogue.json` lists one [`Exhibit`] per file in `museum/`, compiled into
//! this crate so every target reads them, WebAssembly and big-endian ones included. [`open`] holds
//! one exhibit to its record: its bytes are the ones stored, a file reads whole and writes back
//! byte for byte, and an operation log decodes as its one encoding and replays to its hash.
//! [`visit`] does that for every exhibit, and its [`Visit::digest`] covers them all, so the golden
//! `geom-test/museum` holds the museum to the same matrix as every other golden: any change to
//! the compiler, the lock file or vendored numeric code that moves a hash fails there (28.13, the
//! toolchain guard). [`repo`] checks the folder against the catalogue and against `geom-format`'s
//! own archive of each format version.
//!
//! Guarantees:
//!
//! - **Exhibits never change.** A byte changed in a stored file is [`MuseumError::Changed`], and
//!   nothing else about that exhibit is read. An exhibit taken out, or a recorded hash edited,
//!   changes the digest, which is a golden and so needs its record and a kernel owner's approval.
//! - **A file reads whole, and writes back the same.** Every record is known to the reader, the
//!   count is the recorded one, and [`geom_format::file::write`] gives the same bytes.
//! - **A log replays to its hash.** A log that does not replay, whether the kernel refuses an
//!   operation or gives another hash, is [`MuseumError::Replay`]; the kernel must keep every
//!   version of every operation an exhibit names (26.2).
//! - **Nothing is left out.** Every `.skgf` file in the museum has an exhibit, every exhibit a
//!   file, and every file of `geom-format`'s archive is in the museum byte for byte ([`repo`]).
//! - **No file system at run time.** [`visit`] reads only what is compiled in, so it runs on every
//!   target; [`repo`] alone reads the tree, natively.
//!
//! Method: chapter 28.13's format museum and chapter 24.6's rule that files are read forever
//! (`projects/engines/geometry/architecture/28-testing.md`,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`), replaying through
//! `crate::fixture::run` as chapter 21 says.
//!
//! Cost: linear in the exhibits' bytes and in what their logs run.

use std::fmt;
use std::fs;
use std::path::Path;

use geom_format::file::{self, FileError};
use geom_format::registry;
use geom_format::source::SliceSource;
use geom_run::{Budget, WorkMeter};
use serde::Deserialize;

use crate::fixture::log::{LogError, OpLog};
use crate::fixture::run::{Engine, Replayed, replay};
use crate::golden::{Sha256, hex, sha256};

/// The catalogue's text.
const CATALOGUE: &str = include_str!("../museum/catalogue.json");

/// Every stored file, by its path under `museum/`.
const STORED: &[(&str, &[u8])] = &[
    (
        "v1/empty-log.skgf",
        include_bytes!("../museum/v1/empty-log.skgf"),
    ),
    (
        "v1/every-primitive-log.skgf",
        include_bytes!("../museum/v1/every-primitive-log.skgf"),
    ),
    (
        "v1/header-only.skgf",
        include_bytes!("../museum/v1/header-only.skgf"),
    ),
    (
        "v1/moved-box-log.skgf",
        include_bytes!("../museum/v1/moved-box-log.skgf"),
    ),
    (
        "v1/stamped-bodies.skgf",
        include_bytes!("../museum/v1/stamped-bodies.skgf"),
    ),
    (
        "v1/wall-body.skgf",
        include_bytes!("../museum/v1/wall-body.skgf"),
    ),
];

/// The folder of the museum, under the repository root.
const DIR: [&str; 6] = [
    "projects",
    "engines",
    "geometry",
    "crates",
    "geom-test",
    "museum",
];

/// The folder of `geom-format`'s archive, under the repository root.
const ARCHIVE: [&str; 7] = [
    "projects",
    "engines",
    "geometry",
    "crates",
    "geom-format",
    "tests",
    "archive",
];

/// What an exhibit is, and what it must come to.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum Kind {
    /// A file of records.
    File {
        /// How many records it holds besides its header and END record.
        records: u64,
    },
    /// An operation log.
    Log {
        /// The hash it replays to, in hex.
        replays_to: String,
    },
}

/// One stored file and its record.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Exhibit {
    /// Its id: `<format>/<name>`.
    pub id: String,
    /// What it is, in plain words.
    pub what: String,
    /// The format version it was written in.
    pub format: u64,
    /// The kernel edition it was written under.
    pub edition: String,
    /// Its path under `museum/`.
    pub file: String,
    /// The SHA-256 of its bytes, in hex.
    pub sha256: String,
    /// What it is and what it must come to.
    pub kind: Kind,
}

/// The catalogue.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    /// The exhibits, by id.
    pub exhibits: Vec<Exhibit>,
}

/// Why the museum does not hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MuseumError {
    /// The catalogue does not parse, or breaks its rules.
    Catalogue(String),
    /// An exhibit has no stored file.
    Missing(String),
    /// A stored file's bytes are not the recorded ones.
    Changed(String),
    /// A file does not read, or does not hold what its record says.
    Unreadable {
        /// The exhibit.
        id: String,
        /// Why.
        why: String,
    },
    /// A file reads but does not write back the same.
    NotIdentical(String),
    /// A log does not replay to its hash.
    Replay {
        /// The exhibit.
        id: String,
        /// What it gave.
        got: String,
    },
}

impl fmt::Display for MuseumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MuseumError::Catalogue(why) => write!(f, "the catalogue: {why}"),
            MuseumError::Missing(id) => write!(f, "{id}: no stored file"),
            MuseumError::Changed(id) => write!(f, "{id}: its bytes are not the recorded ones"),
            MuseumError::Unreadable { id, why } => write!(f, "{id}: {why}"),
            MuseumError::NotIdentical(id) => write!(f, "{id}: it does not write back the same"),
            MuseumError::Replay { id, got } => write!(f, "{id}: it replayed to {got}"),
        }
    }
}

impl std::error::Error for MuseumError {}

/// A meter with no cap: exhibits are this crate's own, so nothing here needs a budget.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Reads `text` as a catalogue and checks its rules: ids `<format>/<name>` in ascending order, each
/// file under the folder of its format version, each digest 64 hex digits.
///
/// # Errors
///
/// [`MuseumError::Catalogue`] for the first that breaks.
pub fn parse(text: &str) -> Result<Catalogue, MuseumError> {
    let catalogue: Catalogue =
        serde_json::from_str(text).map_err(|e| MuseumError::Catalogue(e.to_string()))?;
    for pair in catalogue.exhibits.windows(2) {
        if let [a, b] = pair
            && a.id >= b.id
        {
            let why = format!("{} is not before {}", a.id, b.id);
            return Err(MuseumError::Catalogue(why));
        }
    }
    for e in &catalogue.exhibits {
        let folder = format!("v{}/", e.format);
        let digest = |d: &str| d.len() == 64 && d.bytes().all(|b| b.is_ascii_hexdigit());
        let replayed = match &e.kind {
            Kind::Log { replays_to } => digest(replays_to),
            Kind::File { .. } => true,
        };
        if !e.file.starts_with(&folder) || !e.id.starts_with(&format!("{}/", e.format)) {
            let why = format!("{} is not under format {}", e.id, e.format);
            return Err(MuseumError::Catalogue(why));
        }
        if !digest(&e.sha256) || !replayed {
            return Err(MuseumError::Catalogue(format!("{}: a bad digest", e.id)));
        }
    }
    Ok(catalogue)
}

/// The catalogue compiled in.
///
/// # Errors
///
/// [`MuseumError::Catalogue`], which the tests keep from shipping.
pub fn catalogue() -> Result<Catalogue, MuseumError> {
    parse(CATALOGUE)
}

/// The stored bytes of `file`.
#[must_use]
pub fn stored(file: &str) -> Option<&'static [u8]> {
    STORED.iter().find(|(p, _)| *p == file).map(|(_, b)| *b)
}

/// Holds a file to its record of `records` records.
fn open_file(id: &str, bytes: &[u8], records: u64) -> Result<(), MuseumError> {
    let unreadable = |why: String| MuseumError::Unreadable {
        id: id.to_owned(),
        why,
    };
    let say = |e: FileError| unreadable(e.to_string());
    let read =
        file::read(&mut SliceSource::new(bytes), registry::knows, &mut meter()).map_err(say)?;
    if !read.skipped.is_empty() {
        return Err(unreadable(format!(
            "{} records this reader does not know",
            read.skipped.len()
        )));
    }
    let count = u64::try_from(read.records.len()).unwrap_or(u64::MAX);
    if count != records {
        return Err(unreadable(format!("{count} records, not {records}")));
    }
    let again = file::write(&read.header, &read.records, &mut meter()).map_err(say)?;
    if again == bytes {
        Ok(())
    } else {
        Err(MuseumError::NotIdentical(id.to_owned()))
    }
}

/// Holds a log to its hash, replayed through `engine`.
fn open_log(
    id: &str,
    bytes: &[u8],
    replays_to: &str,
    engine: &mut dyn Engine,
) -> Result<(), MuseumError> {
    let log = OpLog::decode(bytes).map_err(|e: LogError| MuseumError::Unreadable {
        id: id.to_owned(),
        why: e.to_string(),
    })?;
    let got = match replay(engine, &log) {
        Replayed::Done { hash, .. } | Replayed::Failed { hash, .. } => hex(hash.0),
        Replayed::Broken(why) => why,
    };
    if got == replays_to {
        Ok(())
    } else {
        Err(MuseumError::Replay {
            id: id.to_owned(),
            got,
        })
    }
}

/// Holds `exhibit` to its record, given its `bytes`.
///
/// # Errors
///
/// The first [`MuseumError`] it meets.
pub fn open(exhibit: &Exhibit, bytes: &[u8], engine: &mut dyn Engine) -> Result<(), MuseumError> {
    if hex(sha256(bytes)) != exhibit.sha256 {
        return Err(MuseumError::Changed(exhibit.id.clone()));
    }
    match &exhibit.kind {
        Kind::File { records } => open_file(&exhibit.id, bytes, *records),
        Kind::Log { replays_to } => open_log(&exhibit.id, bytes, replays_to, engine),
    }
}

/// What a visit to every exhibit found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visit {
    /// How many exhibits held.
    pub exhibits: usize,
    /// The SHA-256 of every exhibit's id, digest and what it must come to, in catalogue order.
    pub digest: [u8; 32],
}

/// Holds every exhibit of `catalogue` to its record, reading `bytes` for its files.
///
/// # Errors
///
/// The first [`MuseumError`] it meets.
pub fn visit(
    catalogue: &Catalogue,
    bytes: impl Fn(&str) -> Option<&'static [u8]>,
    engine: &mut dyn Engine,
) -> Result<Visit, MuseumError> {
    let mut hash = Sha256::new();
    for e in &catalogue.exhibits {
        let stored = bytes(&e.file).ok_or_else(|| MuseumError::Missing(e.id.clone()))?;
        open(e, stored, engine)?;
        hash.write(e.id.as_bytes());
        hash.write(e.sha256.as_bytes());
        if let Kind::Log { replays_to } = &e.kind {
            hash.write(replays_to.as_bytes());
        }
    }
    Ok(Visit {
        exhibits: catalogue.exhibits.len(),
        digest: hash.finish(),
    })
}

/// Every `.skgf` file under `dir`, as `<folder>/<file>` with its bytes, in byte order.
fn listed(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let say = |e: std::io::Error| format!("{}: {e}", dir.display());
    let mut out = Vec::new();
    for folder in fs::read_dir(dir).map_err(say)? {
        let folder = folder.map_err(say)?.path();
        if !folder.is_dir() {
            continue;
        }
        let name = folder
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        for file in fs::read_dir(&folder).map_err(say)? {
            let path = file.map_err(say)?.path();
            if path.extension().is_some_and(|e| e == "skgf") {
                let leaf = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                out.push((format!("{name}/{leaf}"), bytes));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Checks the folder under the repository at `root`: every file of the museum is compiled in with
/// the same bytes and has an exhibit, every exhibit a file, and every file of `geom-format`'s
/// archive is in the museum, byte for byte.
///
/// # Errors
///
/// Every problem found, each in words.
pub fn repo(root: &Path, catalogue: &Catalogue) -> Result<(), Vec<String>> {
    let at = |parts: &[&str]| {
        parts
            .iter()
            .fold(root.to_path_buf(), |p, part| p.join(part))
    };
    let mut problems = Vec::new();
    let museum = listed(&at(&DIR)).unwrap_or_else(|e| {
        problems.push(e);
        Vec::new()
    });
    let archive = listed(&at(&ARCHIVE)).unwrap_or_else(|e| {
        problems.push(e);
        Vec::new()
    });
    for (path, bytes) in &museum {
        if stored(path) != Some(bytes.as_slice()) {
            problems.push(format!("{path} is not compiled in with these bytes"));
        }
        if !catalogue.exhibits.iter().any(|e| e.file == *path) {
            problems.push(format!("{path} has no exhibit"));
        }
    }
    for (path, _) in STORED {
        if !museum.iter().any(|(p, _)| p == path) {
            problems.push(format!("{path} is compiled in but not in the folder"));
        }
    }
    for (path, bytes) in &archive {
        if stored(path) != Some(bytes.as_slice()) {
            problems.push(format!("the archive's {path} is not in the museum"));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}
