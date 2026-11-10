//! The oracle catalogue: the maintained list of outside oracles (chapter 28.10; piece G113), and the
//! checks that none is linked or published.
//!
//! What it does: [`Catalogue::parse`] reads `oracles/catalogue.json` ([`CATALOGUE_PATH`]) into typed
//! records and checks its rules. [`linked`] reads a `Cargo.lock` and names any package a catalogue
//! entry says must never enter a build. [`audit`] reads a checkout of the oracle repository and names
//! anything that could publish an image or build from an unpinned one.
//!
//! The rules, from 28.10, 32.6 and 29.20's rule 8:
//!
//! - **Out of process, or permissive.** An oracle is a development dependency of `geom-test` only if
//!   its licence is permissive; everything else runs out of process from the oracle repository.
//! - **A live oracle is pinned**: its source at a full commit, or a release, and the folder that
//!   holds it in the oracle repository.
//! - **Nothing an oracle is made of is linked.** [`linked`] fails on any package named in an
//!   out-of-process entry's `never_linked`.
//! - **Never pushed.** [`audit`] fails on a workflow that logs in to a registry or pushes an image,
//!   and on a Dockerfile whose every base image is not pinned by digest.
//!
//! Guarantees:
//!
//! - **Nothing ambiguous parses**: unknown fields, a missing field, a malformed id or two entries
//!   with one id are each a [`CatalogueError`].
//! - **Read as text, never run**: the audit executes nothing from the repository it reads.
//!
//! Method: chapter 28.10, decision 32.6, 29.20's rule 8
//! (`projects/engines/geometry/architecture/`). `Cargo.lock` is read as TOML.
//!
//! Cost: linear in the catalogue, the lock file and the repository's text files.

use core::fmt;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::licence::Expr;

/// The catalogue's path, relative to the workspace root, one component per entry.
pub const CATALOGUE_PATH: [&str; 7] = [
    "projects",
    "engines",
    "geometry",
    "crates",
    "geom-test",
    "oracles",
    "catalogue.json",
];

/// The catalogue's text as this crate was built with it.
pub const EMBEDDED: &str = include_str!("../../oracles/catalogue.json");

/// The catalogue format this reader reads.
pub const FORMAT: u32 = 1;

/// How an oracle runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Run {
    /// A separate program, in its own container, from the oracle repository.
    OutOfProcess,
    /// A permissive Rust crate, a development dependency of `geom-test` only.
    DevDependency,
    /// A permissive program with its script in this repository, run by `kernel-nightly.yml` in a
    /// container pinned by digest (G63, G86).
    KernelNightly,
}

/// Whether an oracle runs yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// It runs.
    Live,
    /// It is planned; its piece builds it.
    Planned,
}

/// One oracle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Oracle {
    /// Its id: lowercase letters, digits and hyphens, and its folder in the oracle repository.
    pub id: String,
    /// What it is.
    pub what: String,
    /// Its licence, an SPDX expression.
    pub licence: String,
    /// How it runs.
    pub run: Run,
    /// Whether it runs yet.
    pub state: State,
    /// What it checks.
    pub checks: String,
    /// Where its source lives.
    pub source: String,
    /// The pinned commit or release; `None` while planned.
    pub pin: Option<String>,
    /// The packages a link to it would bring into a build.
    pub never_linked: Vec<String>,
    /// The piece that builds it.
    pub piece: String,
}

/// The catalogue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    /// The format, [`FORMAT`].
    pub format: u32,
    /// What the file is, for a person reading it.
    pub about: String,
    /// The oracle repository.
    pub repository: String,
    /// Every oracle.
    pub oracles: Vec<Oracle>,
}

/// Why the catalogue cannot be read, or which rule it breaks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogueError {
    /// The text is not the catalogue's JSON shape (serde's message).
    Json(String),
    /// A catalogue of another format.
    Format(u32),
    /// A rule is broken.
    Rule {
        /// The oracle.
        id: String,
        /// The rule.
        what: String,
    },
}

impl fmt::Display for CatalogueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogueError::Json(e) => write!(f, "not the oracle catalogue: {e}"),
            CatalogueError::Format(n) => {
                write!(f, "catalogue format {n}, this reader reads {FORMAT}")
            }
            CatalogueError::Rule { id, what } => write!(f, "oracle {id}: {what}"),
        }
    }
}

impl core::error::Error for CatalogueError {}

impl Oracle {
    fn rules(&self) -> Result<(), String> {
        let label = !self.id.is_empty()
            && self
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !label {
            return Err("the id must be lowercase letters, digits and hyphens".to_owned());
        }
        for (name, text) in [
            ("what", &self.what),
            ("licence", &self.licence),
            ("checks", &self.checks),
            ("source", &self.source),
            ("piece", &self.piece),
        ] {
            if text.trim().is_empty() {
                return Err(format!("{name} is empty"));
            }
        }
        let permissive =
            Expr::parse(&self.licence).is_ok_and(|e| e.permissive()) || self.licence == "BSL-1.0";
        if self.run != Run::OutOfProcess && !permissive {
            return Err(format!(
                "{} is not permissive, so it runs out of process (28.10)",
                self.licence
            ));
        }
        if self.state == State::Live && self.pin.is_none() {
            return Err("a live oracle is pinned".to_owned());
        }
        if self.run == Run::DevDependency && !self.never_linked.is_empty() {
            return Err("a development dependency is linked, by being one".to_owned());
        }
        if self.run == Run::OutOfProcess
            && self.licence.contains("GPL")
            && self.never_linked.is_empty()
        {
            return Err("a copyleft oracle names what a link to it would bring in".to_owned());
        }
        Ok(())
    }
}

impl Catalogue {
    /// Reads the catalogue and checks its rules.
    ///
    /// # Errors
    ///
    /// [`CatalogueError`], at the first thing that does not hold.
    pub fn parse(text: &str) -> Result<Catalogue, CatalogueError> {
        let catalogue: Catalogue =
            serde_json::from_str(text).map_err(|e| CatalogueError::Json(e.to_string()))?;
        if catalogue.format != FORMAT {
            return Err(CatalogueError::Format(catalogue.format));
        }
        let mut seen = BTreeSet::new();
        for oracle in &catalogue.oracles {
            oracle.rules().map_err(|what| CatalogueError::Rule {
                id: oracle.id.clone(),
                what,
            })?;
            if !seen.insert(oracle.id.as_str()) {
                return Err(CatalogueError::Rule {
                    id: oracle.id.clone(),
                    what: "two entries with one id".to_owned(),
                });
            }
        }
        Ok(catalogue)
    }

    /// The catalogue compiled into this crate.
    ///
    /// # Errors
    ///
    /// As [`Catalogue::parse`].
    pub fn embedded() -> Result<Catalogue, CatalogueError> {
        Catalogue::parse(EMBEDDED)
    }

    /// The oracle `id`.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Oracle> {
        self.oracles.iter().find(|o| o.id == id)
    }
}

/// Every package of the `Cargo.lock` text `lock` that an out-of-process oracle says must never enter
/// a build, as `<package>: <oracle>`.
///
/// # Errors
///
/// A lock file that is not TOML.
pub fn linked(catalogue: &Catalogue, lock: &str) -> Result<Vec<String>, String> {
    let lock: toml::Table = lock.parse().map_err(|e| format!("Cargo.lock: {e}"))?;
    let packages: BTreeSet<&str> = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| p.get("name").and_then(toml::Value::as_str))
        .collect();
    let mut found = Vec::new();
    for oracle in &catalogue.oracles {
        for package in &oracle.never_linked {
            if packages.contains(package.as_str()) {
                found.push(format!("{package}: {}", oracle.id));
            }
        }
    }
    Ok(found)
}

/// What may not appear in a workflow of the oracle repository.
const PUBLISHING: [&str; 5] = [
    "docker push",
    "docker login",
    "docker/login-action",
    "push: true",
    "ghcr.io",
];

/// Every breach of the rules in the oracle repository's checkout `dir`: a workflow that could
/// publish an image, a `FROM` not pinned by digest, or a live oracle with no folder.
///
/// # Errors
///
/// A file or folder that cannot be read.
pub fn audit(catalogue: &Catalogue, dir: &Path) -> Result<Vec<String>, String> {
    let read =
        |path: &Path| fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()));
    let mut problems = Vec::new();
    let workflows = dir.join(".github").join("workflows");
    let entries = fs::read_dir(&workflows).map_err(|e| format!("{}: {e}", workflows.display()))?;
    let mut names: Vec<_> = entries
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{}: {e}", workflows.display()))?;
    names.sort_by_key(fs::DirEntry::file_name);
    for entry in names {
        let text = read(&entry.path())?;
        for word in PUBLISHING {
            if text
                .lines()
                .any(|l| l.contains(word) && !l.trim_start().starts_with('#'))
            {
                problems.push(format!(
                    "{}: `{word}` would publish an image (29.20, rule 8)",
                    entry.path().display()
                ));
            }
        }
    }
    for oracle in &catalogue.oracles {
        if oracle.run != Run::OutOfProcess || oracle.state != State::Live {
            continue;
        }
        let dockerfile = dir.join(&oracle.id).join("Dockerfile");
        if !dockerfile.is_file() {
            problems.push(format!("{}: a live oracle needs a Dockerfile", oracle.id));
            continue;
        }
        for line in read(&dockerfile)?.lines() {
            if line.trim_start().to_ascii_uppercase().starts_with("FROM ")
                && !line.contains("@sha256:")
            {
                problems.push(format!("{}: `{line}` is not pinned by digest", oracle.id));
            }
        }
    }
    Ok(problems)
}
