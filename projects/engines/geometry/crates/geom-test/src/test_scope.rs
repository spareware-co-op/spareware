//! Which crates' tests a pull request's change can reach, so that `kernel.yml`'s test suites run
//! only those (chapter 28.17: "nothing re-tests code that has not changed"), while a push to main,
//! a release tag and a run with no change list run every crate's tests.
//!
//! What it does: [`Scope::read`] reads the workspace's crate graph from `cargo metadata` and finds
//! the crates whose sources read files outside their own folder ([`reads_outside`]), and
//! [`Scope::select`] takes the files a change touches (`git diff --name-only`, one path from the
//! repository root each) and decides, for every workspace crate, whether its tests run, with the
//! changed file that selected it and why. [`Selection::outputs`] writes the decision as the step
//! outputs `kernel.yml`'s suites read, [`Selection::parts`] shares the selected crates out among
//! the release suite's parts by their measured seconds ([`SUITE_TIMES`]), and
//! [`Selection::report`] writes the decision for the job's log.
//! [`Scope::build_reached`] says whether a change reaches what one crate's tests and benchmarks
//! build, for `kernel-bench.yml`'s count gates. `kernel-quality test-scope` is the command line.
//!
//! A crate Y's tests run when a changed file is:
//!
//! - in no workspace crate's folder (a workflow, `Cargo.lock`, `.cargo/`, the toolchain file, an
//!   architecture chapter): every crate's tests run;
//! - `MAP.json` or `quality-ledger.json`, when the change's base is given: Y's tests run when one of
//!   the file's changed entries names Y, or Y's sources read the file whole ([`crate::shared`]); a
//!   change that cannot be placed so, or one given without its base, runs every crate's tests;
//! - in a crate X's folder, outside its `tests/`, `benches/` and `examples/`, where X is in what
//!   Y's test binaries link: Y itself, every crate Y reaches through normal and build
//!   dependencies, transitively, and every crate Y's development dependencies reach the same way
//!   (cargo builds a crate's development dependencies for its own tests, and never a dependency's
//!   development dependencies);
//! - in Y's own `tests/`, `benches/` or `examples/` folder;
//! - anywhere, when Y's sources or tests read files outside Y's folder (a path that climbs out
//!   of it, the workspace root, `cargo metadata`): such a crate's tests can change their verdict
//!   on any change, so they run on every change.
//!
//! Guarantees:
//!
//! - **Over, never under.** A crate's tests are skipped only when no changed file is in what its
//!   test binaries link or read, by the rule above; whatever cannot be placed selects every crate.
//!   `tests/test_scope.rs` holds the selection to the crate graph, read there independently, with
//!   planted mistakes.
//! - **Everything without a change.** [`Scope::select`] given no list runs every crate's tests.
//! - **Every decision is written down.** The report names each crate, whether its tests run, and
//!   the changed file that selected it or that none did.
//!
//! Method: `cargo metadata`'s resolved graph
//! (<https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>), read for normal, build and
//! development edges, and cargo's rule that development dependencies build only for the package's
//! own tests, examples and benchmarks
//! (<https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#development-dependencies>).
//!
//! Cost: linear in the size of the metadata and of the crates' sources, and in the changed files
//! times the workspace's crates.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde_json::Value;

use crate::fuzz_scope::{TEST_FOLDERS, packages};
use crate::shared::{Changes, Reach, Readers};

/// What in a crate's source says it reads files outside its own folder: a relative path that
/// climbs out of a folder, a walk up the folder tree, the workspace's root, or the workspace's
/// whole graph.
pub const READS_OUTSIDE: [&str; 7] = [
    "\"../",
    "join(\"..\")",
    "ancestors()",
    ".parent()",
    "workspace_root",
    "cargo_metadata",
    "CARGO_WORKSPACE_DIR",
];

/// The folders of a crate whose sources [`reads_outside`] reads.
pub const SOURCE_FOLDERS: [&str; 5] = ["src", "tests", "benches", "examples", "build.rs"];

/// The crate graph, read once.
#[derive(Clone, Debug)]
pub struct Scope {
    /// Every workspace crate's folder, from the workspace root and ending in `/`, by name.
    folders: BTreeMap<String, String>,
    /// For each crate, the crates its test binaries link: itself, its normal and build
    /// dependencies' closure, and its development dependencies' closure.
    links: BTreeMap<String, BTreeSet<String>>,
    /// The crates whose sources read files outside their own folder.
    readers: BTreeSet<String>,
    /// The crates that read each shared file whole ([`crate::shared`]).
    whole: Readers,
}

/// Where a changed file is.
enum Place {
    /// In no crate's folder.
    Outside,
    /// In a crate's `tests/`, `benches/` or `examples/` folder.
    OwnTests(String),
    /// In a crate, where its library's build reads it.
    Crate(String),
}

/// The decision for one crate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The crate.
    pub name: String,
    /// Why its tests run, or `None` when they are skipped.
    pub why: Option<String>,
}

/// The decision for every crate of the workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    /// The changed files it was made from, or `None` for every crate.
    pub changed: Option<Vec<String>>,
    /// Every crate, in name order.
    pub crates: Vec<Entry>,
}

/// Why every crate's tests run when no change is given.
const EVERYTHING: &str = "no change was given (a push to main, a release tag or a run by hand): \
                          every crate's tests run";

impl Scope {
    /// Reads the crate graph from `metadata` and the crates' sources from disk.
    ///
    /// # Errors
    ///
    /// The metadata has no workspace root, members or resolved graph, or a source cannot be read.
    pub fn read(metadata: &Value) -> Result<Self, String> {
        let root = metadata
            .get("workspace_root")
            .and_then(Value::as_str)
            .ok_or("cargo metadata has no workspace root")?;
        let mut readers = BTreeSet::new();
        let mut libs = BTreeMap::new();
        for p in packages(metadata)? {
            let folder = Path::new(root).join(&p.folder);
            if reads_outside(&folder)? {
                readers.insert(p.name.clone());
            }
            if let Some(lib) = p.libs.first() {
                libs.insert(p.name, lib.clone());
            }
        }
        let scope = Self::new(metadata, readers)?;
        let whole = Readers::read(Path::new(root), &scope.folders, &libs)?;
        Ok(scope.with_whole_readers(whole))
    }

    /// The same scope, with `whole` the crates that read each shared file whole.
    #[must_use]
    pub fn with_whole_readers(self, whole: Readers) -> Self {
        Self { whole, ..self }
    }

    /// The crates that read each shared file whole.
    #[must_use]
    pub fn whole_readers(&self) -> &Readers {
        &self.whole
    }

    /// The scope of the workspace `metadata` describes, with `readers` the crates whose sources
    /// read files outside their folder.
    ///
    /// # Errors
    ///
    /// The metadata has no workspace root, members or resolved graph.
    pub fn new(metadata: &Value, readers: BTreeSet<String>) -> Result<Self, String> {
        let packages = packages(metadata)?;
        let names: BTreeMap<&str, &str> = packages
            .iter()
            .map(|p| (p.id.as_str(), p.name.as_str()))
            .collect();
        let mut built: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut dev: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let nodes = metadata
            .get("resolve")
            .and_then(|r| r.get("nodes"))
            .and_then(Value::as_array)
            .ok_or("cargo metadata has no resolved graph")?;
        for node in nodes {
            let Some(user) = node
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| names.get(id))
            else {
                continue;
            };
            for d in node
                .get("deps")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(dependency) = d
                    .get("pkg")
                    .and_then(Value::as_str)
                    .and_then(|id| names.get(id))
                else {
                    continue;
                };
                for kind in d
                    .get("dep_kinds")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    // `kind` is null for a normal dependency, "build" or "dev" otherwise.
                    let is_dev = kind.get("kind").and_then(Value::as_str) == Some("dev");
                    let into = if is_dev { &mut dev } else { &mut built };
                    into.entry((*user).to_owned())
                        .or_default()
                        .insert((*dependency).to_owned());
                }
            }
        }
        let closure = |starts: &mut dyn Iterator<Item = &String>| -> BTreeSet<String> {
            let mut out = BTreeSet::new();
            let mut stack: Vec<String> = starts.cloned().collect();
            while let Some(c) = stack.pop() {
                if out.insert(c.clone()) {
                    stack.extend(built.get(&c).into_iter().flatten().cloned());
                }
            }
            out
        };
        let mut links = BTreeMap::new();
        for p in &packages {
            let own = [p.name.clone()];
            let devs = dev.get(&p.name).into_iter().flatten();
            let mut starts = own.iter().chain(devs);
            links.insert(p.name.clone(), closure(&mut starts));
        }
        let folders = packages.into_iter().map(|p| (p.name, p.folder)).collect();
        Ok(Self {
            folders,
            links,
            readers,
            whole: Readers::default(),
        })
    }

    /// The workspace crates and their folders, from the workspace root, each ending in `/`.
    #[must_use]
    pub fn folders(&self) -> &BTreeMap<String, String> {
        &self.folders
    }

    /// The crates whose sources read files outside their folder.
    #[must_use]
    pub fn readers(&self) -> &BTreeSet<String> {
        &self.readers
    }

    /// Where `file` is.
    fn place(&self, file: &str) -> Place {
        let owner = self
            .folders
            .iter()
            .filter(|(_, folder)| file.starts_with(folder.as_str()))
            .max_by_key(|(_, folder)| folder.len());
        let Some((krate, folder)) = owner else {
            return Place::Outside;
        };
        let inside = file.get(folder.len()..).unwrap_or_default();
        if TEST_FOLDERS.iter().any(|t| inside.starts_with(t)) {
            Place::OwnTests(krate.clone())
        } else {
            Place::Crate(krate.clone())
        }
    }

    /// Why the shared file `file`, whose change reaches `reach`, reaches `krate` (its tests and
    /// what they build), if it does: one of its changed entries names `krate`, or `krate`'s sources
    /// read it whole.
    fn shared_reaches(&self, file: &str, reach: &Reach, krate: &str) -> Option<String> {
        match reach {
            Reach::Everything(why) => Some(format!(
                "{file} changed and {why}, so it may change any build"
            )),
            Reach::Crates(named) => named
                .get(krate)
                .map(|why| format!("{file} changed: {why}"))
                .or_else(|| {
                    self.whole
                        .crates(file)
                        .contains(krate)
                        .then(|| format!("{file} changed, and {krate}'s sources read it whole"))
                }),
        }
    }

    /// Why `file` runs the tests of `krate`, if it does, with `shared` what the change's shared
    /// files reach.
    fn selects(&self, file: &str, krate: &str, shared: &Changes) -> Option<String> {
        if self.readers.contains(krate) {
            return Some(format!(
                "{file} changed, and {krate}'s sources read files outside its folder"
            ));
        }
        if let Some(reach) = shared.get(file) {
            return self.shared_reaches(file, reach, krate);
        }
        match self.place(file) {
            Place::Outside => Some(format!("{file} is in no crate, so it may change any build")),
            Place::OwnTests(c) => (c == krate).then(|| format!("{file} is among {krate}'s tests")),
            Place::Crate(c) => {
                let links = self.links.get(krate)?;
                links.contains(&c).then(|| {
                    if c == krate {
                        format!("{file} is in {krate}")
                    } else {
                        format!("{file} is in {c}, which {krate}'s tests build")
                    }
                })
            }
        }
    }

    /// Whether a change reaches what `krate`'s test binaries and benchmarks build (the crates they
    /// link), with the file that does: a file in no crate, or in one of those crates outside its
    /// test folders, or in `krate`'s own test folders. `None` when no change is given is never
    /// returned: no change list reaches everything.
    #[must_use]
    pub fn build_reached(&self, changed: Option<&[String]>, krate: &str) -> Option<String> {
        self.build_reached_with(changed, &Changes::new(), krate)
    }

    /// [`Scope::build_reached`], with `shared` what the change's shared files reach: such a file
    /// reaches `krate`'s build when one of its changed entries names `krate` or `krate`'s sources
    /// read it whole.
    #[must_use]
    pub fn build_reached_with(
        &self,
        changed: Option<&[String]>,
        shared: &Changes,
        krate: &str,
    ) -> Option<String> {
        let Some(files) = changed else {
            return Some(EVERYTHING.to_owned());
        };
        let links = self.links.get(krate)?;
        files.iter().map(|f| normal(f)).find_map(|f| {
            if let Some(reach) = shared.get(&f) {
                return self.shared_reaches(&f, reach, krate);
            }
            match self.place(&f) {
                Place::Outside => Some(format!("{f} is in no crate, so it may change any build")),
                Place::OwnTests(c) => (c == krate).then(|| format!("{f} is among {krate}'s own")),
                Place::Crate(c) => links
                    .contains(&c)
                    .then(|| format!("{f} is in {c}, which {krate} builds")),
            }
        })
    }

    /// The decision for every crate, from the files a change touches (paths from the repository
    /// root), or for every crate when `changed` is `None`.
    #[must_use]
    pub fn select(&self, changed: Option<&[String]>) -> Selection {
        self.select_with(changed, &Changes::new())
    }

    /// [`Scope::select`], with `shared` what the change's shared files reach
    /// ([`crate::shared::changes`]); a shared file it does not hold counts as a file in no crate.
    #[must_use]
    pub fn select_with(&self, changed: Option<&[String]>, shared: &Changes) -> Selection {
        let files: Option<Vec<String>> = changed.map(|c| {
            c.iter()
                .map(|f| normal(f))
                .filter(|f| !f.is_empty())
                .collect()
        });
        let crates = self
            .folders
            .keys()
            .map(|name| Entry {
                name: name.clone(),
                why: match &files {
                    None => Some(EVERYTHING.to_owned()),
                    Some(files) => files.iter().find_map(|f| self.selects(f, name, shared)),
                },
            })
            .collect();
        Selection {
            changed: files,
            crates,
        }
    }
}

/// Each kernel crate's measured seconds in the release suite (`suite-times.tsv`, read by
/// [`crate::coverage::parse_times`]), which [`Selection::parts`] balances `kernel.yml`'s release
/// suite parts by.
pub const SUITE_TIMES: &str = include_str!("suite-times.tsv");

/// A changed file's path as `git diff` writes it, `/` between its parts, without `./`.
fn normal(file: &str) -> String {
    let f = file.trim().replace('\\', "/");
    f.strip_prefix("./")
        .map_or_else(|| f.clone(), str::to_owned)
}

impl Selection {
    /// The crates whose tests run, by name.
    #[must_use]
    pub fn running(&self) -> Vec<&str> {
        self.crates
            .iter()
            .filter(|e| e.why.is_some())
            .map(|e| e.name.as_str())
            .collect()
    }

    /// The step outputs, as `(name, value)`: `crates`, the names of the crates whose tests run,
    /// separated by spaces, and `all`, `true` when every crate's tests run.
    #[must_use]
    pub fn outputs(&self) -> Vec<(String, String)> {
        let running = self.running();
        vec![
            ("crates".to_owned(), running.join(" ")),
            (
                "all".to_owned(),
                (running.len() == self.crates.len()).to_string(),
            ),
        ]
    }

    /// The kernel's crates whose tests run (those whose folder, in `folders`, is in the kernel's
    /// crates folder, which `kernel.yml`'s suites test), but those `apart` names (which the
    /// workflow runs in parts of their own), shared out among `n` parts of a suite by their
    /// measured `times` ([`crate::coverage::balance`], as the coverage run shares its parts), each
    /// part in name order. Every such crate is in exactly one part; a part is empty when there are
    /// fewer crates than parts.
    #[must_use]
    pub fn parts(
        &self,
        n: usize,
        apart: &[String],
        times: &BTreeMap<String, u64>,
        folders: &BTreeMap<String, String>,
    ) -> Vec<Vec<String>> {
        let crates: Vec<String> = self
            .running()
            .into_iter()
            .filter(|c| {
                folders
                    .get(*c)
                    .is_some_and(|f| f.starts_with(crate::mutants::KERNEL_FOLDER))
            })
            .filter(|c| !apart.iter().any(|a| a == c))
            .map(str::to_owned)
            .collect();
        crate::coverage::balance(&crates, times, n)
            .into_iter()
            .map(|(_, mut part)| {
                part.sort();
                part
            })
            .collect()
    }

    /// [`Selection::parts`] as the step output `parts`: one line of JSON, an object from each
    /// part's number (`"1"` to `"n"`) to its crates separated by spaces, which a workflow's part k
    /// reads as `fromJSON(needs.scope.outputs.parts)['k']`.
    #[must_use]
    pub fn parts_output(
        &self,
        n: usize,
        apart: &[String],
        times: &BTreeMap<String, u64>,
        folders: &BTreeMap<String, String>,
    ) -> (String, String) {
        let object: serde_json::Map<String, Value> = self
            .parts(n, apart, times, folders)
            .into_iter()
            .enumerate()
            .map(|(k, part)| (k.saturating_add(1).to_string(), Value::from(part.join(" "))))
            .collect();
        ("parts".to_owned(), Value::Object(object).to_string())
    }

    /// The decision, for the job's log and summary: the changed files, then every crate, each
    /// with whether its tests run and why.
    #[must_use]
    pub fn report(&self) -> String {
        let mut out = String::new();
        match &self.changed {
            None => {
                let _ = writeln!(out, "Every crate's tests run: {EVERYTHING}.");
            }
            Some(files) => {
                let _ = writeln!(out, "The change touches {} file(s):", files.len());
                for f in files {
                    let _ = writeln!(out, "- {f}");
                }
            }
        }
        let _ = writeln!(
            out,
            "\nThe tests of {} of {} crates run:",
            self.running().len(),
            self.crates.len()
        );
        for e in &self.crates {
            match &e.why {
                Some(why) => {
                    let _ = writeln!(out, "- runs {}: {why}", e.name);
                }
                None => {
                    let _ = writeln!(
                        out,
                        "- skips {}: no changed file is in what its tests build or read",
                        e.name
                    );
                }
            }
        }
        out
    }
}

/// Whether the Rust sources of the crate in `folder` (its `src/`, `tests/`, `benches/`,
/// `examples/` and `build.rs`) say they read files outside it ([`READS_OUTSIDE`]).
///
/// # Errors
///
/// A folder or file that cannot be read.
pub fn reads_outside(folder: &Path) -> Result<bool, String> {
    let mut stack: Vec<std::path::PathBuf> =
        SOURCE_FOLDERS.iter().map(|s| folder.join(s)).collect();
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            let listing =
                std::fs::read_dir(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            for entry in listing {
                stack.push(
                    entry
                        .map_err(|e| format!("{}: {e}", path.display()))?
                        .path(),
                );
            }
        } else if path.extension().is_some_and(|e| e == "rs") && path.is_file() {
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            if text_reads_outside(&text) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Whether a source's `text` names one of [`READS_OUTSIDE`].
#[must_use]
pub fn text_reads_outside(text: &str) -> bool {
    READS_OUTSIDE.iter().any(|w| text.contains(w))
}
