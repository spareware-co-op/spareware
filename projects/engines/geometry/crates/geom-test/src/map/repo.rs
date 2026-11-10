//! The map on disk: reading a workspace's crates, and writing or checking the committed map.
//!
//! What it does: [`read`] lists the crate folders under `projects/engines/geometry/crates` of a
//! workspace root, and the facade `engine-geometry` and its binding crates `engine-geometry-*`
//! under `projects/engines/crates`, and reads each one's `Cargo.toml` and every `.rs` file under
//! its `src/` into an [`Input`], with every repository path the headers link to that exists.
//! [`check`] builds the map and compares it with the committed `MAP.json`; [`write()`] builds it
//! and writes `MAP.json` when its bytes differ. Both return every problem.
//!
//! Guarantees:
//!
//! - **Every file, in a fixed order.** Folders are listed in sorted order and walked without
//!   recursion, at most [`MAX_DEPTH`] folders deep, so the input, and so the map, is the same on
//!   every system. A file that is not UTF-8, and a symbolic link (or Windows junction) under a
//!   crate's `src/` that is a folder or a `.rs` file, or that is a crate folder itself, are
//!   reported, not skipped.
//! - **A link into the repository exists as written.** A path a header links to is recorded as
//!   existing only when every component is found with exactly that name, so the answer is the same
//!   on a file system that ignores case as on one that does not.
//! - **Paths are the repository's.** Every path in the input is relative to the workspace root and
//!   uses `/`, so the map holds no absolute path and reads the same on Windows.
//! - **`check` changes nothing**; `write` changes only `MAP.json`, and only when its bytes differ.
//! - **I/O failures are errors, not passes.** A folder or file that cannot be listed or read stops
//!   the run with [`MapError`], which the binary turns into exit code 2.
//!
//! Method: the default target layout of Cargo, which chapter 27's "one folder per crate, folder
//! name = crate name" and the crate manifests on this branch follow: the library root
//! `src/lib.rs`, the binary roots `src/main.rs` and `src/bin/`, and modules under `src/`
//! (<https://doc.rust-lang.org/cargo/reference/cargo-targets.html#target-auto-discovery>); a
//! manifest that moves a target's root is reported by the map's manifest rule.
//!
//! Cost: one listing of each folder under the crates' `src/` folders, one read of each manifest
//! and source file, and one listing per component of each repository path a header links to,
//! plus the map's own cost ([`crate::map::build`]).

use core::fmt;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::map::header::Header;
use crate::map::links::{self, LinkKind};
use crate::map::{
    CRATES_DIR, CrateInput, FACADE_DIR, Gap, Generated, Input, MAP_FILE, PENDING, Pending, Problem,
    Registry, Rule, SourceFile, build, compare, is_facade,
};

/// The deepest folder under `src/` that is read.
pub const MAX_DEPTH: usize = 8;

/// The facade's generated tool descriptions, which hold every operation in the registry.
pub const TOOLS: &str = "projects/engines/crates/engine-geometry/generated/mcp-tools.json";

/// An I/O failure that stops the run.
#[derive(Debug)]
pub struct MapError {
    /// The path that could not be read or written.
    pub path: String,
    /// Why.
    pub message: String,
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for MapError {}

fn error(path: &Path, e: &io::Error) -> MapError {
    MapError {
        path: path.display().to_string(),
        message: e.to_string(),
    }
}

/// The entries of a folder, sorted by name, each with whether it is a folder; symbolic links are
/// returned as files whose read fails.
fn entries(folder: &Path) -> Result<Vec<(String, PathBuf, Kind)>, MapError> {
    let mut out = Vec::new();
    for entry in fs::read_dir(folder).map_err(|e| error(folder, &e))? {
        let entry = entry.map_err(|e| error(folder, &e))?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|e| error(&path, &e))?;
        let kind = if kind.is_symlink() {
            Kind::Link
        } else if kind.is_dir() {
            Kind::Folder
        } else {
            Kind::File
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        out.push((name, path, kind));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// What a folder entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Folder,
    File,
    Link,
}

/// Whether a file name ends in `.rs`, in any case.
fn is_rust(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("rs"))
}

/// Every `.rs` file under `src`, in path order, with `prefix` (the folder's path relative to the
/// root) put before each name.
fn sources(src: &Path, prefix: &str) -> Result<Vec<SourceFile>, MapError> {
    let mut out = Vec::new();
    let mut folders: Vec<(PathBuf, String, usize)> =
        vec![(src.to_path_buf(), prefix.to_owned(), 0)];
    while let Some((folder, relative, depth)) = folders.pop() {
        for (name, path, kind) in entries(&folder)? {
            let rel = format!("{relative}/{name}");
            match kind {
                Kind::Folder if depth < MAX_DEPTH => {
                    folders.push((path, rel, depth.saturating_add(1)));
                }
                Kind::Folder => out.push(SourceFile {
                    path: rel,
                    text: Err(format!(
                        "a folder deeper than {MAX_DEPTH} levels under src/"
                    )),
                }),
                Kind::Link
                    if is_rust(&name) || fs::metadata(&path).ok().is_none_or(|m| m.is_dir()) =>
                {
                    out.push(SourceFile {
                        path: rel,
                        text: Err(
                            "a symbolic link or junction; the map reads files and folders \
                             only, so move what it points to under src/"
                                .to_owned(),
                        ),
                    });
                }
                Kind::File if is_rust(&name) => {
                    let text = match fs::read(&path) {
                        Ok(bytes) => {
                            String::from_utf8(bytes).map_err(|_| "the file is not UTF-8".to_owned())
                        }
                        Err(e) => return Err(error(&path, &e)),
                    };
                    out.push(SourceFile { path: rel, text });
                }
                _ => {}
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Whether `relative` (a path under `root`, `/`-separated, a folder when it ends in `/`) exists
/// with exactly these names, whatever the file system's case rules.
fn exists_exactly(root: &Path, relative: &str) -> bool {
    let folder = relative.ends_with('/');
    let parts: Vec<&str> = relative.split('/').filter(|p| !p.is_empty()).collect();
    let mut at = root.to_path_buf();
    for (k, part) in parts.iter().enumerate() {
        let Ok(listing) = entries(&at) else {
            return false;
        };
        let last = k.saturating_add(1) == parts.len();
        let Some((_, path, kind)) = listing.into_iter().find(|(name, _, _)| name == part) else {
            return false;
        };
        let wanted = if last && !folder {
            kind == Kind::File
        } else {
            kind == Kind::Folder
        };
        if !wanted {
            return false;
        }
        at = path;
    }
    true
}

/// The crate folders under `dir` (relative to `root`) whose names `keep` accepts, read; a crate
/// folder that is a symbolic link is added to `unreadable`.
fn crates_in(
    root: &Path,
    dir: &str,
    keep: impl Fn(&str) -> bool,
    unreadable: &mut Vec<SourceFile>,
) -> Result<Vec<CrateInput>, MapError> {
    let mut crates = Vec::new();
    for (name, path, kind) in entries(&root.join(dir))? {
        if !keep(&name) {
            continue;
        }
        if kind == Kind::Link {
            unreadable.push(SourceFile {
                path: format!("{dir}/{name}"),
                text: Err(
                    "a crate folder that is a symbolic link or junction; the map reads \
                     real folders only"
                        .to_owned(),
                ),
            });
            continue;
        }
        if kind != Kind::Folder {
            continue;
        }
        let folder = format!("{dir}/{name}");
        let manifest_path = path.join("Cargo.toml");
        let manifest = if manifest_path.is_file() {
            Some(fs::read_to_string(&manifest_path).map_err(|e| error(&manifest_path, &e))?)
        } else {
            None
        };
        let src = path.join("src");
        let files = if src.is_dir() {
            sources(&src, &format!("{folder}/src"))?
        } else {
            Vec::new()
        };
        crates.push(CrateInput {
            name,
            folder,
            manifest,
            files,
        });
    }
    Ok(crates)
}

/// Reads every crate folder of the workspace at `root`: the kernel's, and the facade's when the
/// workspace has the shared engines' folder. The pending folders are [`PENDING`].
///
/// # Errors
///
/// [`MapError`] if a folder cannot be listed or a file cannot be read.
pub fn read(root: &Path) -> Result<Input, MapError> {
    read_with(root, PENDING)
}

/// [`read`] with `pending` as the folders still to arrive, so a test can exercise the rule with a
/// list of its own while [`PENDING`] is empty.
///
/// # Errors
///
/// [`MapError`] if a folder cannot be listed or a file cannot be read.
pub fn read_with(root: &Path, pending: &[Pending]) -> Result<Input, MapError> {
    let mut unreadable = Vec::new();
    let mut crates = crates_in(root, CRATES_DIR, |_| true, &mut unreadable)?;
    if root.join(FACADE_DIR).is_dir() {
        crates.extend(crates_in(root, FACADE_DIR, is_facade, &mut unreadable)?);
    }
    let mut linked: BTreeSet<String> = BTreeSet::new();
    for file in crates.iter().flat_map(|c| &c.files) {
        if let Ok(text) = &file.text {
            let header = Header::parse(text).lines.join(
                "
",
            );
            linked.extend(
                links::find(&header)
                    .into_iter()
                    .filter(|l| l.kind == LinkKind::Repo)
                    .map(|l| l.text),
            );
        }
    }
    let repo_paths = linked
        .into_iter()
        .filter(|path| exists_exactly(root, path))
        .collect();
    let pending = pending
        .iter()
        .filter(|p| !exists_exactly(root, p.folder))
        .copied()
        .collect();
    let registry = registry(root, &crates)?;
    Ok(Input {
        crates,
        unreadable,
        repo_paths: Some(repo_paths),
        pending,
        registry,
    })
}

/// What the map's operations and kinds are read from, if the tree holds the facade's generated
/// tool descriptions: them, and every crate's tests, fuzz targets and benchmarks.
fn registry(root: &Path, crates: &[CrateInput]) -> Result<Option<Registry>, MapError> {
    let path = root.join(TOOLS);
    if !path.is_file() {
        return Ok(None);
    }
    let tools = fs::read_to_string(&path).map_err(|e| error(&path, &e))?;
    let mut evidence = Vec::new();
    for krate in crates {
        let tests = root.join(&krate.folder).join("tests");
        if tests.is_dir() {
            evidence.extend(sources(&tests, &format!("{}/tests", krate.folder))?);
        }
        if krate.name == "geom-fuzz" || krate.name == "geom-bench" {
            evidence.extend(krate.files.iter().cloned());
        }
    }
    Ok(Some(Registry { tools, evidence }))
}

/// A problem for every entry of `pending` whose folder the workspace at `root` now holds.
fn stale_pending(root: &Path, pending: &[Pending]) -> Vec<Problem> {
    pending
        .iter()
        .filter(|p| exists_exactly(root, p.folder))
        .map(|p| Problem {
            file: p.folder.to_owned(),
            rule: Rule::GapStale,
            message: "the pending folder is in the tree now, so links into it are checked: delete \
                      its entry from map::PENDING"
                .to_owned(),
        })
        .collect()
}

/// The committed map's text, if it exists.
fn committed(root: &Path) -> Result<Option<String>, MapError> {
    let path = root.join(MAP_FILE);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(error(&path, &e)),
    }
}

/// Builds the map of the workspace at `root`, with `gaps` accepted ([`crate::map::GAPS`] for this
/// workspace), and compares it with the committed one: every header problem, and a
/// [`crate::map::Rule::Stale`] problem if the map is missing or differs, sorted by file, rule and
/// message.
///
/// # Errors
///
/// [`MapError`] if a folder or file cannot be read.
pub fn check(root: &Path, gaps: &[Gap]) -> Result<Vec<Problem>, MapError> {
    check_with(root, gaps, PENDING)
}

/// [`check`] with `pending` as the folders still to arrive (see [`read_with`]).
///
/// # Errors
///
/// [`MapError`] if a folder or file cannot be read.
pub fn check_with(
    root: &Path,
    gaps: &[Gap],
    pending: &[Pending],
) -> Result<Vec<Problem>, MapError> {
    let Generated {
        text, mut problems, ..
    } = build(&read_with(root, pending)?, gaps);
    problems.extend(compare(committed(root)?.as_deref(), &text));
    problems.extend(stale_pending(root, pending));
    problems.sort();
    Ok(problems)
}

/// Builds the map of the workspace at `root`, with `gaps` accepted, and writes it if its bytes
/// differ from the committed file; returns every header problem, and whether it wrote.
///
/// # Errors
///
/// [`MapError`] if a folder or file cannot be read, or the map cannot be written.
pub fn write(root: &Path, gaps: &[Gap]) -> Result<(Vec<Problem>, bool), MapError> {
    write_with(root, gaps, PENDING)
}

/// [`write`] with `pending` as the folders still to arrive (see [`read_with`]).
///
/// # Errors
///
/// [`MapError`] if a folder or file cannot be read, or the map cannot be written.
pub fn write_with(
    root: &Path,
    gaps: &[Gap],
    pending: &[Pending],
) -> Result<(Vec<Problem>, bool), MapError> {
    let Generated {
        text, mut problems, ..
    } = build(&read_with(root, pending)?, gaps);
    problems.extend(stale_pending(root, pending));
    problems.sort();
    let path = root.join(MAP_FILE);
    let current = fs::read(&path).ok();
    if current.as_deref() == Some(text.as_bytes()) {
        return Ok((problems, false));
    }
    fs::write(&path, text.as_bytes()).map_err(|e| error(&path, &e))?;
    Ok((problems, true))
}

/// The workspace this crate was built in: five folders above its manifest
/// (`projects/engines/geometry/crates/geom-test`).
///
/// # Errors
///
/// [`MapError`] if there is no `Cargo.toml` there.
pub fn workspace_root() -> Result<PathBuf, MapError> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.ancestors().nth(5).ok_or_else(|| MapError {
        path: manifest_dir.display().to_string(),
        message: "has fewer than five parent folders".to_owned(),
    })?;
    if !root.join("Cargo.toml").is_file() {
        return Err(MapError {
            path: root.display().to_string(),
            message: "has no Cargo.toml: not the workspace root".to_owned(),
        });
    }
    Ok(root.to_path_buf())
}
