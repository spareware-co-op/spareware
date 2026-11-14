//! Which of `kernel-fuzz.yml`'s fuzz targets and Kani proofs a change can affect, so that a pull
//! request fuzzes and proves only those (chapter 28.5: "each pull request fuzzes the crates it
//! changes briefly"; 28.7 and 28.17: on a pull request, the proofs the change can reach), while a
//! push to main, a release tag and the nightly campaign run every one.
//!
//! What it does: [`Scope::read`] reads the workspace's crate graph from `cargo metadata` and
//! `geom-fuzz`'s sources, and [`Scope::select`] takes the files a change touches (`git diff
//! --name-only`, one path from the repository root each) and decides, for each job of [`JOBS`],
//! whether it runs and, for the fuzz jobs, which targets, each with the file that selected it and
//! why. [`Selection::outputs`] writes the decision as the step outputs the workflow's jobs read: a
//! job's `if` reads `<output>`, and a fuzz job's matrix excludes `<output>_exclude` (`fromJSON`), so
//! the workflow's own matrix lists stay the catalogue of what exists. [`Selection::report`] writes
//! it for the job's log: every target and proof, run or skipped, and why. `kernel-quality
//! fuzz-scope` is the command line.
//!
//! A changed file reaches:
//!
//! - every job and target, when it is in no workspace crate's folder: a workflow, `Cargo.lock`,
//!   `.cargo/`, the toolchain file, an architecture chapter a test reads. Such a file can change any
//!   build or check, as `coverage::affected_crates` and the merge tools treat it.
//! - in `geom-fuzz`, the targets it is part of: `src/<module>.rs`, the targets whose code uses that
//!   module (a target's own module and every module it names through `crate::`, `super::` or
//!   `geom_fuzz::`, transitively); `tests/<target>.rs` and `tests/__fuzz__/<target>/` (its tests,
//!   corpus and crash files), that target; any other file of the crate (its manifest,
//!   `src/lib.rs`, `tests/common/`, a helper's tests), every target.
//! - in another crate's `tests/`, `benches/` or `examples/` folder, nothing here: cargo builds
//!   those only for that crate's own test targets, and no harness here is one. `MAP.json`,
//!   `PROVENANCE.md` and `NOTICE` count as `geom-test`'s tests, which check them.
//! - any other file of a crate C, every harness whose build holds C: a fuzz target whose code names
//!   C, or a crate that depends on C through normal or build dependencies, transitively; the Kani
//!   proofs beside a crate's code, when that crate is C or depends on it so.
//!
//! Development dependencies are not followed from one crate to the next: cargo builds a crate's
//! development dependencies only for that crate's own tests, never when the crate is a dependency.
//! A fuzz target is a test of `geom-fuzz`, whose code names the crates it uses (its development
//! dependencies among them), and `cargo kani -p <crate>` builds the crate's library alone.
//!
//! Guarantees:
//!
//! - **Over, never under.** A harness is skipped only when no changed file is in its build as
//!   above; whatever cannot be placed (a file in no crate, a `geom-fuzz` file no one target owns)
//!   selects every harness it could reach. `tests/fuzz_scope.rs` holds the selection to the crate
//!   graph and to the `use` lines of every target, with planted mistakes.
//! - **Everything without a change.** [`Scope::select`] given no list (a push, a tag, a night)
//!   runs every job and every target.
//! - **An error, never a guess.** Metadata without the graph, a `geom-fuzz` source that does not
//!   lex as Rust, or a target without its tests file is an error, and the workflow's later jobs do
//!   not start.
//! - **Every decision is written down.** The report names each target and proof, whether it runs,
//!   and the changed file that selected it or that none did.
//!
//! Method: `cargo metadata`'s resolved graph
//! (<https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>), read for normal and build
//! edges, and cargo's rule that development dependencies build only for the package's own tests
//! (<https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#development-dependencies>);
//! the sources are read with `proc-macro2`, Rust's lexer, so a word in a comment or a string is
//! never a name. GitHub Actions' matrix `exclude` and `fromJSON`
//! (<https://docs.github.com/en/actions/writing-workflows/workflow-syntax-for-github-actions#jobsjob_idstrategymatrixexclude>)
//! apply the decision.
//!
//! Cost: linear in the size of the metadata and of `geom-fuzz`'s sources, and in the changed
//! files times the workspace's crates.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use serde_json::Value;

/// The crate that holds the fuzz targets.
pub const FUZZ_CRATE: &str = "geom-fuzz";

/// Files outside every crate's folder that only `geom-test`'s tests read (its tools write them):
/// they count as that crate's tests.
pub const TEST_INPUTS: [&str; 3] = [
    "projects/engines/geometry/MAP.json",
    "projects/engines/geometry/PROVENANCE.md",
    "projects/engines/geometry/NOTICE",
];

/// The folders of a crate that cargo builds only for that crate's own test targets.
pub const TEST_FOLDERS: [&str; 3] = ["tests/", "benches/", "examples/"];

/// What a job of the workflow runs, and so which files can affect it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    /// Each of `geom-fuzz`'s fuzz targets on its own, as the matrix entry `<target>_fuzz`.
    FuzzTargets,
    /// The Kani proofs in `geom-fuzz`'s tests, together: the targets whose tests hold one.
    ProofTargets,
    /// The Kani proofs beside a crate's own code, together.
    Crate(&'static str),
    /// Whatever another job runs (its seeded run).
    Job(&'static str),
}

/// How a job's matrix is narrowed to what runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Narrow {
    /// The whole job runs or is skipped.
    Whole,
    /// The matrix excludes each skipped target by this variable (`{"target": "p21_fuzz"}`).
    Entries(&'static str),
    /// The matrix excludes, by this variable, the shards beyond the number of targets that run,
    /// so that no shard is left without one.
    Shards(&'static str),
}

/// One job of `kernel-fuzz.yml` that the selection decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Job {
    /// The job's id in the workflow.
    pub name: &'static str,
    /// The step output its `if` reads; `<output>_exclude` and `<output>_targets` go with it.
    pub output: &'static str,
    /// What it runs.
    pub reach: Reach,
    /// How its matrix is narrowed.
    pub narrow: Narrow,
}

/// The catalogue: every job of `kernel-fuzz.yml` that fuzzes or proves, and what it runs.
/// `tests/fuzz_scope.rs` holds it to the workflow: every other job is one of [`UNSCOPED_JOBS`].
pub const JOBS: [Job; 9] = [
    Job {
        name: "libfuzzer",
        output: "libfuzzer",
        reach: Reach::FuzzTargets,
        narrow: Narrow::Entries("target"),
    },
    Job {
        name: "kani",
        output: "kani",
        reach: Reach::ProofTargets,
        narrow: Narrow::Whole,
    },
    Job {
        name: "kani-num",
        output: "kani_num",
        reach: Reach::Crate("geom-num"),
        narrow: Narrow::Whole,
    },
    Job {
        name: "kani-topo",
        output: "kani_topo",
        reach: Reach::Crate("geom-topo"),
        narrow: Narrow::Whole,
    },
    Job {
        name: "kani-limbs",
        output: "kani_limbs",
        reach: Reach::Crate("geom-num"),
        narrow: Narrow::Whole,
    },
    Job {
        name: "kani-op",
        output: "kani_op",
        reach: Reach::Crate("geom-op"),
        narrow: Narrow::Whole,
    },
    Job {
        name: "kani-format",
        output: "kani_format",
        reach: Reach::Crate("geom-format"),
        narrow: Narrow::Whole,
    },
    Job {
        name: "clusterfuzzlite",
        output: "clusterfuzzlite",
        reach: Reach::FuzzTargets,
        narrow: Narrow::Shards("shard"),
    },
    Job {
        name: "clusterfuzzlite-seeded",
        output: "clusterfuzzlite_seeded",
        reach: Reach::Job("clusterfuzzlite"),
        narrow: Narrow::Whole,
    },
];

/// The workflow's jobs the selection does not decide: the wait for the cheap checks, and the
/// selection itself.
pub const UNSCOPED_JOBS: [&str; 2] = ["cheap", "select"];

/// One fuzz target of `geom-fuzz`: what its code is made of.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Target {
    /// The modules of `geom-fuzz`'s `src/` its code uses, its own among them.
    pub modules: BTreeSet<String>,
    /// The workspace crates its code names, `geom-fuzz` left out.
    pub crates: BTreeSet<String>,
}

/// The crate graph and `geom-fuzz`'s targets, read once.
#[derive(Clone, Debug)]
pub struct Scope {
    /// Every workspace crate's folder, from the workspace root and ending in `/`, by package name.
    folders: BTreeMap<String, String>,
    /// For each crate, the crates whose build holds it: itself, and every crate that depends on it
    /// through normal or build dependencies, transitively.
    built_by: BTreeMap<String, BTreeSet<String>>,
    /// `geom-fuzz`'s modules (the stems of `src/*.rs`, `lib` left out).
    modules: BTreeSet<String>,
    /// `geom-fuzz`'s fuzz targets, by module name.
    targets: BTreeMap<String, Target>,
    /// The targets whose tests hold a Kani proof.
    proof_targets: BTreeSet<String>,
}

/// Where a changed file is.
enum Place {
    /// In no crate's folder.
    Outside,
    /// In `geom-fuzz`, at this path inside its folder.
    Fuzz(String),
    /// In a crate's own test folders (or one of [`TEST_INPUTS`]).
    OwnTests,
    /// In a crate, where its build reads it.
    Crate(String),
}

/// The decision for one target of a fuzz job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The matrix value: `<target>_fuzz`.
    pub name: String,
    /// Why it runs, or `None` when it is skipped.
    pub why: Option<String>,
}

/// The decision for one job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobSelection {
    /// The job.
    pub job: Job,
    /// Why it runs, or `None` when it is skipped.
    pub why: Option<String>,
    /// For a fuzz job, every target, in name order.
    pub entries: Vec<Entry>,
}

/// The decision for every job of [`JOBS`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    /// The changed files it was made from, or `None` for every target and proof.
    pub changed: Option<Vec<String>>,
    /// One per job of [`JOBS`], in that order.
    pub jobs: Vec<JobSelection>,
}

/// Why every harness runs when no change is given.
const EVERYTHING: &str = "no change was given (a push to main, a release tag or a night): \
                          everything runs";

/// Why a skipped harness is skipped.
const UNREACHED: &str = "no changed file is in its build";

impl Scope {
    /// Reads the crate graph from `metadata` (`cargo metadata --format-version 1`) and
    /// `geom-fuzz`'s sources from its folder on disk.
    ///
    /// # Errors
    ///
    /// What is missing or unreadable: see [`Scope::new`], and a source file that cannot be read.
    pub fn read(metadata: &Value) -> Result<Self, String> {
        let manifest = packages(metadata)?
            .into_iter()
            .find(|p| p.name == FUZZ_CRATE)
            .map(|p| p.manifest)
            .ok_or_else(|| format!("the workspace has no {FUZZ_CRATE}"))?;
        let folder = Path::new(&manifest)
            .parent()
            .ok_or_else(|| format!("{manifest} has no folder"))?;
        Self::new(metadata, &read_fuzz_sources(folder)?)
    }

    /// The scope of the workspace `metadata` describes, with `geom-fuzz`'s sources given as its
    /// `src/*.rs`, `tests/*.rs` and `tests/common/*.rs` files' text, by path inside its folder
    /// (as [`read_fuzz_sources`] reads them).
    ///
    /// # Errors
    ///
    /// The metadata has no workspace root, members or resolved graph, a member's manifest is
    /// outside the root, a source does not lex as Rust, or a target has no `tests/<target>.rs`.
    pub fn new(metadata: &Value, sources: &BTreeMap<String, String>) -> Result<Self, String> {
        let packages = packages(metadata)?;
        let mut folders = BTreeMap::new();
        let mut libs = BTreeMap::new();
        for p in &packages {
            folders.insert(p.name.clone(), p.folder.clone());
            for lib in &p.libs {
                libs.insert(lib.clone(), p.name.clone());
            }
        }
        let built_by = built_by(metadata, &packages)?;
        let mut streams = BTreeMap::new();
        for (path, text) in sources {
            let stream: TokenStream = text
                .parse()
                .map_err(|e| format!("{FUZZ_CRATE}/{path} is not Rust: {e}"))?;
            streams.insert(path.clone(), stream);
        }
        let modules: BTreeSet<String> = streams
            .keys()
            .filter_map(|p| p.strip_prefix("src/")?.strip_suffix(".rs"))
            .filter(|m| !m.contains('/') && *m != "lib")
            .map(str::to_owned)
            .collect();
        let mut targets = BTreeMap::new();
        let mut proof_targets = BTreeSet::new();
        for m in &modules {
            let defines = streams
                .get(&format!("src/{m}.rs"))
                .is_some_and(defines_harness);
            if !defines {
                continue;
            }
            let tests = format!("tests/{m}.rs");
            let Some(test) = streams.get(&tests) else {
                return Err(format!("{FUZZ_CRATE}'s target {m} has no {tests}"));
            };
            if names_kani_proof(test) {
                proof_targets.insert(m.clone());
            }
            targets.insert(m.clone(), target(m, &streams, &modules, &libs));
        }
        Ok(Self {
            folders,
            built_by,
            modules,
            targets,
            proof_targets,
        })
    }

    /// `geom-fuzz`'s fuzz targets, by module name.
    #[must_use]
    pub fn targets(&self) -> &BTreeMap<String, Target> {
        &self.targets
    }

    /// The targets whose tests hold a Kani proof.
    #[must_use]
    pub fn proof_targets(&self) -> &BTreeSet<String> {
        &self.proof_targets
    }

    /// The workspace crates and their folders, from the workspace root, each ending in `/`.
    #[must_use]
    pub fn folders(&self) -> &BTreeMap<String, String> {
        &self.folders
    }

    /// Where `file` is.
    fn place(&self, file: &str) -> Place {
        if TEST_INPUTS.contains(&file) {
            return Place::OwnTests;
        }
        let owner = self
            .folders
            .iter()
            .filter(|(_, folder)| file.starts_with(folder.as_str()))
            .max_by_key(|(_, folder)| folder.len());
        let Some((krate, folder)) = owner else {
            return Place::Outside;
        };
        let inside = file.get(folder.len()..).unwrap_or_default();
        if krate == FUZZ_CRATE {
            Place::Fuzz(inside.to_owned())
        } else if TEST_FOLDERS.iter().any(|t| inside.starts_with(t)) {
            Place::OwnTests
        } else {
            Place::Crate(krate.clone())
        }
    }

    /// The targets a `geom-fuzz` file is part of, with why, or `None` when it is no one target's
    /// (every target).
    fn fuzz_file_targets(&self, inside: &str) -> Option<BTreeMap<&str, String>> {
        if let Some(m) = inside
            .strip_prefix("src/")
            .and_then(|r| r.strip_suffix(".rs"))
            .filter(|m| self.modules.contains(*m))
        {
            return Some(
                self.targets
                    .iter()
                    .filter(|(_, t)| t.modules.contains(m))
                    .map(|(name, _)| {
                        let why = if name == m {
                            format!("is the target {name}'s own module")
                        } else {
                            format!("is the module {m}, which the target {name} uses")
                        };
                        (name.as_str(), why)
                    })
                    .collect(),
            );
        }
        let own = inside
            .strip_prefix("tests/__fuzz__/")
            .and_then(|r| r.split_once('/'))
            .map(|(t, _)| t)
            .or_else(|| {
                inside
                    .strip_prefix("tests/")
                    .and_then(|r| r.strip_suffix(".rs"))
            });
        let (name, _) = self.targets.get_key_value(own?)?;
        Some(BTreeMap::from([(
            name.as_str(),
            format!("is among the target {name}'s own tests, corpus and crashes"),
        )]))
    }

    /// Why `file` selects the fuzz target `name`, if it does.
    fn selects_target(&self, file: &str, name: &str, target: &Target) -> Option<String> {
        match self.place(file) {
            Place::Outside => Some(format!("{file} is in no crate, so it may change any build")),
            Place::Fuzz(inside) => match self.fuzz_file_targets(&inside) {
                None => Some(format!(
                    "{file} is {FUZZ_CRATE}'s, and no one target's, so every target's"
                )),
                Some(owned) => owned.get(name).map(|why| format!("{file} {why}")),
            },
            Place::OwnTests => None,
            Place::Crate(krate) => {
                let users = self.built_by.get(&krate)?;
                let through = target.crates.iter().find(|c| users.contains(*c))?;
                Some(if *through == krate {
                    format!("{file} is in {krate}, which the target {name} names")
                } else {
                    format!(
                        "{file} is in {krate}, which the target {name} builds through {through}"
                    )
                })
            }
        }
    }

    /// Why `file` selects the Kani proofs beside `krate`'s code, if it does.
    fn selects_crate(&self, file: &str, krate: &str) -> Option<String> {
        let changed = match self.place(file) {
            Place::Outside => {
                return Some(format!("{file} is in no crate, so it may change any build"));
            }
            Place::OwnTests => return None,
            Place::Fuzz(inside) => {
                if TEST_FOLDERS.iter().any(|t| inside.starts_with(t)) {
                    return None;
                }
                FUZZ_CRATE.to_owned()
            }
            Place::Crate(c) => c,
        };
        if !self.built_by.get(&changed)?.contains(krate) {
            return None;
        }
        Some(if changed == krate {
            format!("{file} is in {krate}")
        } else {
            format!("{file} is in {changed}, which {krate} builds")
        })
    }

    /// The decision for every job of [`JOBS`], from the files a change touches (paths from the
    /// repository root), or for everything when `changed` is `None`.
    #[must_use]
    pub fn select(&self, changed: Option<&[String]>) -> Selection {
        let files: Option<Vec<String>> = changed.map(|c| {
            c.iter()
                .map(|f| f.trim().replace('\\', "/"))
                .map(|f| {
                    f.strip_prefix("./")
                        .map_or_else(|| f.clone(), str::to_owned)
                })
                .filter(|f| !f.is_empty())
                .collect()
        });
        let first = |why: &dyn Fn(&str) -> Option<String>| -> Option<String> {
            match &files {
                None => Some(EVERYTHING.to_owned()),
                Some(files) => files.iter().find_map(|f| why(f)),
            }
        };
        let entries: Vec<Entry> = self
            .targets
            .iter()
            .map(|(name, t)| Entry {
                name: format!("{name}_fuzz"),
                why: first(&|f| self.selects_target(f, name, t)),
            })
            .collect();
        let mut jobs: Vec<JobSelection> = Vec::new();
        for job in JOBS {
            let (why, listed) = match job.reach {
                Reach::FuzzTargets => {
                    let running = entries.iter().filter(|e| e.why.is_some()).count();
                    let why = (running > 0)
                        .then(|| format!("{running} of {} targets run", entries.len()));
                    (why, entries.clone())
                }
                Reach::ProofTargets => {
                    let why = first(&|f| {
                        self.proof_targets.iter().find_map(|name| {
                            let t = self.targets.get(name)?;
                            self.selects_target(f, name, t)
                        })
                    });
                    (why, Vec::new())
                }
                Reach::Crate(krate) => (first(&|f| self.selects_crate(f, krate)), Vec::new()),
                Reach::Job(other) => {
                    let why = jobs
                        .iter()
                        .find(|j| j.job.name == other)
                        .and_then(|j| j.why.as_ref())
                        .map(|_| format!("{other} runs, and this is its seeded run"));
                    (why, Vec::new())
                }
            };
            jobs.push(JobSelection {
                job,
                why,
                entries: listed,
            });
        }
        Selection {
            changed: files,
            jobs,
        }
    }
}

impl Selection {
    /// The decision for the job `name`, if it is one of [`JOBS`].
    #[must_use]
    pub fn job(&self, name: &str) -> Option<&JobSelection> {
        self.jobs.iter().find(|j| j.job.name == name)
    }

    /// The step outputs, as `(name, value)`: for each job `<output>` (`true` or `false`); for a
    /// fuzz job `<output>_targets`, the JSON list of the targets that run, and `<output>_exclude`,
    /// the JSON list of matrix entries to leave out (empty when the job is skipped: it runs no
    /// matrix). A sharded job's exclusions are the shards beyond the number of targets that run,
    /// of `shards` in its matrix; without `shards` (the campaign, which has no such job) they are
    /// left out.
    ///
    /// # Errors
    ///
    /// `shards` of 0.
    pub fn outputs(&self, shards: Option<usize>) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        for j in &self.jobs {
            let runs = j.why.is_some();
            out.push((j.job.output.to_owned(), runs.to_string()));
            if j.job.reach == Reach::FuzzTargets {
                let running: Vec<Value> = j
                    .entries
                    .iter()
                    .filter(|e| e.why.is_some())
                    .map(|e| Value::String(e.name.clone()))
                    .collect();
                let count = running.len();
                out.push((
                    format!("{}_targets", j.job.output),
                    Value::Array(running).to_string(),
                ));
                let exclude: Vec<Value> = match j.job.narrow {
                    Narrow::Shards(_) if shards.is_none() => continue,
                    _ if !runs => Vec::new(),
                    Narrow::Whole => Vec::new(),
                    Narrow::Entries(variable) => j
                        .entries
                        .iter()
                        .filter(|e| e.why.is_none())
                        .map(|e| one(variable, Value::String(e.name.clone())))
                        .collect(),
                    Narrow::Shards(variable) => {
                        let total = shards
                            .filter(|n| *n > 0)
                            .ok_or("a sharded job has at least one shard")?;
                        (count.min(total)..total)
                            .filter_map(|i| i.checked_add(1))
                            .map(|n| one(variable, Value::from(n)))
                            .collect()
                    }
                };
                out.push((
                    format!("{}_exclude", j.job.output),
                    Value::Array(exclude).to_string(),
                ));
            }
        }
        Ok(out)
    }

    /// The decision, for the job's log and summary: the changed files, then every job, and for a
    /// fuzz job every target, each with whether it runs and why.
    #[must_use]
    pub fn report(&self) -> String {
        let mut out = String::new();
        match &self.changed {
            None => {
                let _ = writeln!(out, "Every target and proof runs: {EVERYTHING}.");
            }
            Some(files) => {
                let _ = writeln!(out, "The change touches {} file(s):", files.len());
                for f in files {
                    let _ = writeln!(out, "- {f}");
                }
            }
        }
        for j in &self.jobs {
            let _ = writeln!(out);
            match &j.why {
                Some(why) => {
                    let _ = writeln!(out, "{}: runs: {why}.", j.job.name);
                }
                None => {
                    let _ = writeln!(out, "{}: skipped: {UNREACHED}.", j.job.name);
                }
            }
            for e in &j.entries {
                match &e.why {
                    Some(why) => {
                        let _ = writeln!(out, "- runs {}: {why}", e.name);
                    }
                    None => {
                        let _ = writeln!(out, "- skips {}: {UNREACHED}", e.name);
                    }
                }
            }
        }
        out
    }
}

/// `{variable: value}`, one matrix entry to exclude.
fn one(variable: &str, value: Value) -> Value {
    let mut map = serde_json::Map::new();
    map.insert(variable.to_owned(), value);
    Value::Object(map)
}

/// A workspace member, as the selection needs it.
struct Package {
    id: String,
    name: String,
    /// Its manifest's path, as cargo gave it.
    manifest: String,
    /// Its folder from the workspace root, ending in `/`.
    folder: String,
    /// The names its library is used by in code (`geom_num`).
    libs: Vec<String>,
}

/// The kinds of target whose name code uses a crate by.
const LIBRARY_KINDS: [&str; 6] = ["lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"];

/// A path with `/` between its parts.
fn forward(path: &str) -> String {
    path.replace('\\', "/")
}

/// The workspace members of `metadata`.
fn packages(metadata: &Value) -> Result<Vec<Package>, String> {
    let root = metadata
        .get("workspace_root")
        .and_then(Value::as_str)
        .ok_or("cargo metadata has no workspace root")?;
    let root = format!("{}/", forward(root).trim_end_matches('/'));
    let members: BTreeSet<&str> = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .ok_or("cargo metadata has no workspace members")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let mut out = Vec::new();
    for p in metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("cargo metadata has no packages")?
    {
        let field = |key: &str| {
            p.get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("a package without {key}"))
        };
        let id = field("id")?;
        if !members.contains(id) {
            continue;
        }
        let name = field("name")?;
        let manifest = field("manifest_path")?;
        let relative = forward(manifest)
            .strip_prefix(&root)
            .map(str::to_owned)
            .ok_or_else(|| format!("{name}'s manifest {manifest} is outside the workspace"))?;
        let folder = relative
            .strip_suffix("Cargo.toml")
            .ok_or_else(|| format!("{name}'s manifest {manifest} is not a Cargo.toml"))?;
        let libs = p
            .get("targets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|t| {
                t.get("kind")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .any(|k| LIBRARY_KINDS.contains(&k))
            })
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .map(|n| n.replace('-', "_"))
            .collect();
        out.push(Package {
            id: id.to_owned(),
            name: name.to_owned(),
            manifest: manifest.to_owned(),
            folder: folder.to_owned(),
            libs,
        });
    }
    if out.is_empty() {
        return Err("cargo metadata has no workspace member".to_owned());
    }
    // A member at the workspace root would hold every file; none is (the root manifest is a
    // virtual one), and one that were would be no crate's folder for this rule.
    out.retain(|p| !p.folder.is_empty());
    Ok(out)
}

/// For each member, the members whose build holds it: itself and its dependents through normal
/// and build dependencies, transitively.
fn built_by(
    metadata: &Value,
    packages: &[Package],
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let names: BTreeMap<&str, &str> = packages
        .iter()
        .map(|p| (p.id.as_str(), p.name.as_str()))
        .collect();
    let mut dependents: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
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
        for dep in node
            .get("deps")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(dependency) = dep
                .get("pkg")
                .and_then(Value::as_str)
                .and_then(|id| names.get(id))
            else {
                continue;
            };
            // `kind` is null for a normal dependency, "build" or "dev" otherwise.
            let built = dep
                .get("dep_kinds")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|k| {
                    k.get("kind")
                        .is_none_or(|kind| kind.as_str() != Some("dev"))
                });
            if built {
                dependents.entry(dependency).or_default().insert(user);
            }
        }
    }
    let mut out = BTreeMap::new();
    for p in packages {
        let mut reached: BTreeSet<String> = BTreeSet::new();
        let mut stack = vec![p.name.as_str()];
        while let Some(c) = stack.pop() {
            if reached.insert(c.to_owned()) {
                stack.extend(dependents.get(c).into_iter().flatten());
            }
        }
        out.insert(p.name.clone(), reached);
    }
    Ok(out)
}

/// Reads `geom-fuzz`'s `src/*.rs`, `tests/*.rs` and `tests/common/*.rs` from its folder, by path
/// inside it.
///
/// # Errors
///
/// A folder or file that cannot be read.
pub fn read_fuzz_sources(folder: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for sub in ["src", "tests", "tests/common"] {
        let dir = folder.join(sub);
        let listing = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in listing {
            let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
            let is_rust = path.extension().is_some_and(|e| e == "rs");
            let Some(stem) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !is_rust || !path.is_file() {
                continue;
            }
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            out.insert(format!("{sub}/{stem}"), text);
        }
    }
    Ok(out)
}

/// The words a path to one of `geom-fuzz`'s own modules starts with.
const MODULE_ROOTS: [&str; 3] = ["crate", "super", "geom_fuzz"];

/// The token trees of every level of `stream`, the outermost first.
fn levels(stream: &TokenStream) -> Vec<Vec<TokenTree>> {
    let mut out = Vec::new();
    let mut stack = vec![stream.clone()];
    while let Some(s) = stack.pop() {
        let trees: Vec<TokenTree> = s.into_iter().collect();
        for t in &trees {
            if let TokenTree::Group(g) = t {
                stack.push(g.stream());
            }
        }
        out.push(trees);
    }
    out
}

/// Whether `a` and `b` are the two colons of `::`.
fn is_path_sep(a: &TokenTree, b: &TokenTree) -> bool {
    matches!((a, b), (TokenTree::Punct(a), TokenTree::Punct(b)) if a.as_char() == ':' && b.as_char() == ':')
}

/// The modules a file names through `crate::`, `super::` or `geom_fuzz::`: the name after it, or
/// each first name of a `{ ... }` group after it.
fn module_refs(stream: &TokenStream) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for trees in levels(stream) {
        let mut rest = trees.as_slice();
        while let Some((first, tail)) = rest.split_first() {
            rest = tail;
            let TokenTree::Ident(root) = first else {
                continue;
            };
            if !MODULE_ROOTS.contains(&root.to_string().as_str()) {
                continue;
            }
            let [a, b, next, ..] = tail else {
                continue;
            };
            if !is_path_sep(a, b) {
                continue;
            }
            match next {
                TokenTree::Ident(m) => {
                    out.insert(m.to_string());
                }
                TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                    let mut first_of_item = true;
                    for t in g.stream() {
                        match t {
                            TokenTree::Punct(p) if p.as_char() == ',' => first_of_item = true,
                            TokenTree::Ident(m) if first_of_item => {
                                out.insert(m.to_string());
                                first_of_item = false;
                            }
                            _ => first_of_item = false,
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// Every identifier of a file, at every level.
fn idents(stream: &TokenStream) -> BTreeSet<String> {
    levels(stream)
        .into_iter()
        .flatten()
        .filter_map(|t| match t {
            TokenTree::Ident(i) => Some(i.to_string()),
            _ => None,
        })
        .collect()
}

/// Whether a module defines its target's harness: `pub fn harness` among its top-level items.
fn defines_harness(stream: &TokenStream) -> bool {
    let trees: Vec<TokenTree> = stream.clone().into_iter().collect();
    trees.windows(3).any(|w| {
        matches!(w, [TokenTree::Ident(a), TokenTree::Ident(b), TokenTree::Ident(c)]
            if *a == "pub" && *b == "fn" && *c == "harness")
    })
}

/// Whether a tests file names `kani::proof`, at any level (`#[cfg_attr(kani, kani::proof)]`).
fn names_kani_proof(stream: &TokenStream) -> bool {
    levels(stream).iter().any(|trees| {
        trees.windows(4).any(|w| {
            matches!(w, [TokenTree::Ident(k), a, b, TokenTree::Ident(p)]
                if *k == "kani" && *p == "proof" && is_path_sep(a, b))
        })
    })
}

/// The modules and crates of the target `name`: its module, its tests file, the tests' shared
/// `tests/common/`, the crate root, and every module those name, transitively.
fn target(
    name: &str,
    streams: &BTreeMap<String, TokenStream>,
    modules: &BTreeSet<String>,
    libs: &BTreeMap<String, String>,
) -> Target {
    let mut files: Vec<String> = vec!["src/lib.rs".to_owned(), format!("tests/{name}.rs")];
    files.extend(
        streams
            .keys()
            .filter(|p| p.starts_with("tests/common/"))
            .cloned(),
    );
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut queue: Vec<String> = vec![name.to_owned()];
    for f in &files {
        queue.extend(streams.get(f).map(module_refs).into_iter().flatten());
    }
    while let Some(m) = queue.pop() {
        if !modules.contains(&m) || !used.insert(m.clone()) {
            continue;
        }
        let file = format!("src/{m}.rs");
        queue.extend(streams.get(&file).map(module_refs).into_iter().flatten());
        files.push(file);
    }
    let crates = files
        .iter()
        .filter_map(|f| streams.get(f))
        .flat_map(idents)
        .filter_map(|i| libs.get(&i).cloned())
        .filter(|c| c != FUZZ_CRATE)
        .collect();
    Target {
        modules: used,
        crates,
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests may unwrap and index"
)]
mod tests {
    use super::*;

    fn stream(text: &str) -> TokenStream {
        text.parse().unwrap()
    }

    #[test]
    fn module_paths_are_read_from_tokens_not_text() {
        let s = stream(
            "use crate::{adversarial::{a, b}, cbor::hex, Finding};\n\
             use geom_fuzz::wide::{check, harness};\n\
             // crate::make in a comment\n\
             const X: &str = \"crate::open\";\n\
             fn f() { super::names::g(); crate :: prune ::h(); }\n",
        );
        let refs = module_refs(&s);
        let want: BTreeSet<String> = ["adversarial", "cbor", "Finding", "wide", "names", "prune"]
            .map(str::to_owned)
            .into();
        assert_eq!(refs, want);
        assert!(!idents(&s).contains("open"));
        assert!(idents(&s).contains("hex"));
    }

    #[test]
    fn a_harness_and_a_kani_proof_are_found_where_they_are() {
        assert!(defines_harness(&stream("pub fn harness(d: &[u8]) {}")));
        assert!(!defines_harness(&stream("fn harness(d: &[u8]) {}")));
        assert!(!defines_harness(&stream("mod m { pub fn harness() {} }")));
        assert!(names_kani_proof(&stream(
            "#[test]\n#[cfg_attr(kani, kani::proof)]\nfn p() {}"
        )));
        assert!(!names_kani_proof(&stream("// kani::proof\nfn p() {}")));
    }

    #[test]
    fn shards_beyond_the_targets_that_run_are_excluded() {
        let entries = |run: usize| -> Vec<Entry> {
            (0..5)
                .map(|i| Entry {
                    name: format!("t{i}_fuzz"),
                    why: (i < run).then(|| "x".to_owned()),
                })
                .collect()
        };
        let selection = |run: usize| Selection {
            changed: Some(Vec::new()),
            jobs: vec![JobSelection {
                job: JOBS[7],
                why: (run > 0).then(|| "x".to_owned()),
                entries: entries(run),
            }],
        };
        let get = |run: usize, key: &str| {
            selection(run)
                .outputs(Some(4))
                .unwrap()
                .into_iter()
                .find(|(k, _)| k == key)
                .unwrap()
                .1
        };
        assert_eq!(
            get(2, "clusterfuzzlite_exclude"),
            r#"[{"shard":3},{"shard":4}]"#
        );
        assert_eq!(get(5, "clusterfuzzlite_exclude"), "[]");
        assert_eq!(get(0, "clusterfuzzlite_exclude"), "[]");
        assert_eq!(get(0, "clusterfuzzlite"), "false");
        assert_eq!(
            get(2, "clusterfuzzlite_targets"),
            r#"["t0_fuzz","t1_fuzz"]"#
        );
        let unsharded = selection(2).outputs(None).unwrap();
        assert!(
            unsharded
                .iter()
                .all(|(k, _)| k != "clusterfuzzlite_exclude")
        );
        assert!(selection(2).outputs(Some(0)).is_err());
    }
}
