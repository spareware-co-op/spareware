//! The mutation gate: judges cargo-mutants' results against the quality ledger.
//!
//! What it does: [`read_run`] reads one cargo-mutants output directory (`mutants.out`, or one
//! shard's), checking that the run is whole; [`judge`] puts the runs of one gate together and
//! decides, mutant by mutant, under the rules of section 28.8:
//!
//! - **Caught** mutants pass. So do **unviable** ones (the mutated code does not build), but only
//!   when their log holds a compiler diagnostic at a source location; a build that failed for any
//!   other reason (a linker, the disk, a killed process) tested nothing and fails the gate.
//! - A **missed** mutant passes only if the ledger keeps it ([`crate::quality_ledger::Survivor`]) with a
//!   kind its place allows: in the foundation set only `equivalent`; elsewhere also `harmless`
//!   and `known`.
//! - A **timed-out** mutant passes only if the ledger keeps it as `hangs`.
//! - Anything cargo-mutants could not classify fails.
//!
//! Every mutant is judged by the tests cargo-mutants ran for it; `.cargo/mutants.toml` makes those
//! the mutated crate's integration tests (`tests/*.rs`), which see only the crate's public
//! interface (28.8: "mutants are judged by tests through each crate's public interface").
//!
//! Two modes:
//!
//! - [`Mode::Diff`], on a pull request (`cargo mutants --in-diff`): only the mutants in the diff
//!   ran. A ledger entry those mutants match must not match more of them than it counts, and must
//!   not match a mutant that is now caught (the entry is stale: remove it). A survivor may be kept
//!   as `known` only by an entry the base branch's ledger already holds (`base`): a `known` entry
//!   is an old gap the weekly run found, so a pull request that adds one for a survivor it made
//!   would merge a new unexplained survivor, which 28.8 forbids. A ledger-only pull request may
//!   still add `known` entries for the weekly run's survivors, because no mutant of its diff
//!   matches them, and the weekly run then holds each entry to its count.
//! - [`Mode::Full`], weekly, over shards (`cargo mutants --shard k/n`): `expected`, the list of
//!   every mutant (`cargo mutants --list --json` with the same filters), must equal the union of
//!   the shards' mutants exactly, with no mutant in two shards, so a lost or truncated shard
//!   fails. Every ledger entry must match exactly as many surviving mutants as it counts; one that
//!   matches none is stale and fails, so the ledger never keeps a reason for code that changed.
//!
//! In both modes every entry's file must exist and hold its `line_text` on some line, and every
//! entry's kind must be allowed where its file is.
//!
//! What makes a run whole ([`read_run`]): `mutants.json` lists the mutants the run was given;
//! `outcomes.json` must then exist (cargo-mutants writes none when it was given no mutants), hold
//! a passing baseline, have an end time (written only when the run finished), give every listed
//! mutant exactly one outcome and no other mutant any, and its counts must agree with its outcomes.
//!
//! Guarantees:
//!
//! - **A gate that cannot see everything fails.** A run that did not finish, a failed or missing
//!   baseline, a mutant with no outcome or two, totals that disagree with the outcomes, a shard
//!   missing from the full run or two shards that overlap are each a failure or a [`RunError`],
//!   never a smaller set of mutants judged.
//! - **Untested is never caught.** An unviable mutant counts only when its log shows a compiler
//!   error at a source location; any other failed build fails the gate.
//! - **28.8's rules, exactly.** In the foundation set a missed mutant passes only as `equivalent`;
//!   elsewhere also as `harmless`, or as `known` when the base branch's ledger already kept it; a
//!   timeout only as `hangs`. A ledger entry that matches more mutants than it counts, or a caught
//!   or unviable one, or (in the full run) fewer, fails as stale.
//! - **Reasons follow the code.** Entries are keyed by the mutated line's text, so editing that
//!   line puts the mutant before the gate again.
//!
//! Method: the files cargo-mutants 27.1 writes, as its book documents them (`book/src/mutants-out.md`)
//! and its source serialises them (`src/outcome.rs`, `src/mutant.rs`, `src/output.rs`), read at the
//! commit of its v27.1.0 tag (record below), with its manual's sharding and diff options
//! (<https://mutants.rs/shards.html>, <https://mutants.rs/in-diff.html>): an outcome's `summary` is
//! one of `Success`, `CaughtMutant`, `MissedMutant`, `Unviable`, `Failure` and `Timeout`; a
//! scenario is `"Baseline"` or `{"Mutant": {...}}`; a mutant's `name` is
//! `<file>:<line>:<column>: <description>`.
//!
//! Cost: linear in the size of the output files, plus one read of each mutated source file and of
//! each unviable mutant's log.
//!
//! Consulted: cargo-mutants, MIT,
//!   <https://github.com/sourcefrog/cargo-mutants/tree/8ab1dc786a1f61a4e370416cc6c68b81a704e917>,
//!   for the format of the output files this module reads (`book/src/mutants-out.md`,
//!   `src/outcome.rs`, `src/mutant.rs` and `src/output.rs` at its v27.1.0 tag). No code copied.

use core::fmt;
use core::time::Duration;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::foundation::{Place, Set, place};
use crate::quality_ledger::{Kind, Ledger, Survivor};

/// One mutant, as cargo-mutants describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mutant {
    /// cargo-mutants' name: `<file>:<line>:<column>: <description>`.
    pub name: String,
    /// The package.
    pub package: String,
    /// The file, relative to the workspace root, with `/` separators.
    pub file: String,
    /// The line where the mutated span starts, from 1.
    pub line: usize,
    /// The description: the name after `<file>:<line>:<column>: `.
    pub description: String,
}

/// What happened to one mutant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outcome {
    /// The tests failed: the mutant was caught.
    Caught,
    /// The tests passed: the mutant survived.
    Missed,
    /// The tests did not finish in time.
    Timeout,
    /// The mutated code did not build.
    Unviable,
    /// The mutant built and its tests passed without a test phase, or failed unclassified:
    /// cargo-mutants' `Success` and `Failure`, which a mutant should never have.
    Unclassified,
}

impl Outcome {
    fn parse(summary: &str) -> Option<Outcome> {
        Some(match summary {
            "CaughtMutant" => Outcome::Caught,
            "MissedMutant" => Outcome::Missed,
            "Timeout" => Outcome::Timeout,
            "Unviable" => Outcome::Unviable,
            "Success" | "Failure" => Outcome::Unclassified,
            _ => return None,
        })
    }
}

/// One output directory, read and checked whole.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Run {
    /// The directory.
    pub dir: PathBuf,
    /// Every mutant the run was given, from `mutants.json`.
    pub mutants: Vec<Mutant>,
    /// What happened to each, in the same order.
    pub outcomes: Vec<Outcome>,
    /// How long each took, build and test, in the same order.
    pub durations: Vec<Duration>,
}

/// Why an output directory or a list could not be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunError {
    /// The file.
    pub file: PathBuf,
    /// What is wrong.
    pub why: String,
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.why)
    }
}

impl std::error::Error for RunError {}

fn run_err(file: &Path, why: impl Into<String>) -> RunError {
    RunError {
        file: file.to_path_buf(),
        why: why.into(),
    }
}

fn read_json(file: &Path) -> Result<Value, RunError> {
    let text = std::fs::read_to_string(file).map_err(|e| run_err(file, e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| run_err(file, format!("is not JSON: {e}")))
}

/// A mutant from its JSON object in `mutants.json`, `outcomes.json` or `--list --json`.
///
/// # Errors
///
/// A description of the missing or malformed field.
pub fn mutant_from_json(value: &Value) -> Result<Mutant, String> {
    let field = |key: &str| -> Result<&str, String> {
        value
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("a mutant without a string {key:?}"))
    };
    let name = field("name")?.to_owned();
    let file = field("file")?.to_owned();
    let package = field("package")?.to_owned();
    let line = value
        .get("span")
        .and_then(|s| s.get("start"))
        .and_then(|s| s.get("line"))
        .and_then(Value::as_u64)
        .and_then(|l| usize::try_from(l).ok())
        .filter(|&l| l >= 1)
        .ok_or_else(|| format!("mutant {name:?} has no start line"))?;
    // The name is "<file>:<line>:<column>: <description>".
    let description = name
        .strip_prefix(file.as_str())
        .and_then(|rest| rest.strip_prefix(':'))
        .and_then(|rest| rest.split_once(": "))
        .filter(|(location, _)| {
            location.split_once(':').is_some_and(|(l, c)| {
                l == line.to_string() && c.chars().all(|d| d.is_ascii_digit())
            })
        })
        .map(|(_, d)| d.to_owned())
        .filter(|d| !d.is_empty())
        .ok_or_else(|| {
            format!("mutant name {name:?} is not <file>:<line>:<column>: <description>")
        })?;
    Ok(Mutant {
        name,
        package,
        file,
        line,
        description,
    })
}

/// A list of mutants: `mutants.json`, or the output of `cargo mutants --list --json`. A file that
/// holds nothing but white space is the empty list: `cargo mutants --in-diff` prints nothing, and
/// exits 0, when the diff is empty, changes no Rust file or touches no mutant.
///
/// # Errors
///
/// [`RunError`] if the file cannot be read, is not a JSON list of mutants, or two have one name.
pub fn read_mutant_list(file: &Path) -> Result<Vec<Mutant>, RunError> {
    let text = std::fs::read_to_string(file).map_err(|e| run_err(file, e.to_string()))?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let value: Value =
        serde_json::from_str(&text).map_err(|e| run_err(file, format!("is not JSON: {e}")))?;
    let list = value
        .as_array()
        .ok_or_else(|| run_err(file, "is not a list of mutants"))?;
    let mut names = BTreeSet::new();
    let mut out = Vec::new();
    for v in list {
        let m = mutant_from_json(v).map_err(|why| run_err(file, why))?;
        if !names.insert(m.name.clone()) {
            return Err(run_err(file, format!("lists {:?} twice", m.name)));
        }
        out.push(m);
    }
    Ok(out)
}

/// Whether a log holds a compiler diagnostic at a source location: a line starting `error` (with
/// or without an error code) followed, within the next three lines, by rustc's ` --> ` location
/// line.
fn has_compiler_error(log: &str) -> bool {
    let lines: Vec<&str> = log.lines().collect();
    lines.iter().enumerate().any(|(i, l)| {
        l.strip_prefix("error")
            .is_some_and(|rest| rest.starts_with("[E") || rest.starts_with(": "))
            && lines
                .iter()
                .skip(i.saturating_add(1))
                .take(3)
                .any(|n| n.trim_start().starts_with("--> "))
    })
}

/// One listed outcome: what happened, the log's path and how long the phases took.
struct Entry {
    outcome: Outcome,
    log: Option<String>,
    duration: Duration,
}

/// What `outcomes.json` lists, by mutant name.
struct Listed {
    baseline: Vec<String>,
    by_name: BTreeMap<String, Entry>,
}

/// The total of an outcome's `phase_results[].duration` (seconds, as cargo-mutants writes them).
fn phase_time(f: &Path, entry: &Value) -> Result<Duration, RunError> {
    let mut total = Duration::ZERO;
    for phase in entry
        .get("phase_results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let secs = phase
            .get("duration")
            .and_then(Value::as_f64)
            .and_then(|s| Duration::try_from_secs_f64(s).ok())
            .ok_or_else(|| run_err(f, "a phase without a duration in seconds"))?;
        total = total.saturating_add(secs);
    }
    Ok(total)
}

/// Reads the outcome list of `outcomes.json` (`f`, already parsed as `value`).
fn listed_outcomes(f: &Path, value: &Value) -> Result<Listed, RunError> {
    let list = value
        .get("outcomes")
        .and_then(Value::as_array)
        .ok_or_else(|| run_err(f, "has no outcomes list"))?;
    let mut listed = Listed {
        baseline: Vec::new(),
        by_name: BTreeMap::new(),
    };
    for entry in list {
        let summary = entry
            .get("summary")
            .and_then(Value::as_str)
            .ok_or_else(|| run_err(f, "an outcome without a summary"))?;
        let scenario = entry
            .get("scenario")
            .ok_or_else(|| run_err(f, "an outcome without a scenario"))?;
        if scenario.as_str() == Some("Baseline") {
            listed.baseline.push(summary.to_owned());
            continue;
        }
        let mutant = scenario
            .get("Mutant")
            .ok_or_else(|| run_err(f, "a scenario that is neither the baseline nor a mutant"))?;
        let m = mutant_from_json(mutant).map_err(|why| run_err(f, why))?;
        let outcome = Outcome::parse(summary)
            .ok_or_else(|| run_err(f, format!("unknown summary {summary:?} for {}", m.name)))?;
        let log = entry
            .get("log_path")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let duration = phase_time(f, entry)?;
        if listed
            .by_name
            .insert(
                m.name.clone(),
                Entry {
                    outcome,
                    log,
                    duration,
                },
            )
            .is_some()
        {
            return Err(run_err(f, format!("two outcomes for {}", m.name)));
        }
    }
    Ok(listed)
}

/// Checks the run's own totals against its outcomes.
fn check_totals(f: &Path, value: &Value, listed: &Listed) -> Result<(), RunError> {
    if listed.baseline != ["Success"] {
        return Err(run_err(
            f,
            format!(
                "the baseline (the tests on unmutated code) must run once and pass; found {:?}",
                listed.baseline
            ),
        ));
    }
    let total = value.get("total_mutants").and_then(Value::as_u64);
    if total != u64::try_from(listed.by_name.len()).ok() {
        return Err(run_err(
            f,
            format!(
                "total_mutants is {total:?} but {} outcomes are listed",
                listed.by_name.len()
            ),
        ));
    }
    for (key, want) in [
        ("caught", Outcome::Caught),
        ("missed", Outcome::Missed),
        ("timeout", Outcome::Timeout),
        ("unviable", Outcome::Unviable),
    ] {
        let count = listed
            .by_name
            .values()
            .filter(|e| e.outcome == want)
            .count();
        let said = value.get(key).and_then(Value::as_u64);
        if said != u64::try_from(count).ok() {
            return Err(run_err(
                f,
                format!("{key} is {said:?} but {count} outcomes say so"),
            ));
        }
    }
    Ok(())
}

/// Checks that an unviable mutant's log shows a compiler error.
fn check_unviable(dir: &Path, name: &str, log: Option<&String>) -> Result<(), RunError> {
    let log = log.ok_or_else(|| run_err(dir, format!("unviable {name} has no log")))?;
    let path = dir.join(log);
    let text = std::fs::read_to_string(&path).map_err(|e| run_err(&path, e.to_string()))?;
    if has_compiler_error(&text) {
        Ok(())
    } else {
        Err(run_err(
            &path,
            format!(
                "{name} is counted unviable, but its log shows no compiler error at a source \
                 location: the build failed for another reason and the mutant was never tested"
            ),
        ))
    }
}

/// Reads one cargo-mutants output directory and checks it is whole (see the module
/// documentation).
///
/// # Errors
///
/// [`RunError`] for a missing or malformed file, a failed or missing baseline, an unfinished run,
/// counts that disagree, a mutant without an outcome or an outcome without a listed mutant, or an
/// unviable mutant whose log shows no compiler error.
pub fn read_run(dir: &Path) -> Result<Run, RunError> {
    let mutants = read_mutant_list(&dir.join("mutants.json"))?;
    let f = dir.join("outcomes.json");
    if mutants.is_empty() {
        // cargo-mutants returns before testing, and writes no outcomes, when it has no mutants.
        if f.exists() && read_json(&f)?.get("total_mutants").and_then(Value::as_u64) != Some(0) {
            return Err(run_err(&f, "has outcomes, but mutants.json lists none"));
        }
        return Ok(Run {
            dir: dir.to_path_buf(),
            ..Run::default()
        });
    }
    let value = read_json(&f)?;
    if value.get("end_time").is_none_or(Value::is_null) {
        return Err(run_err(&f, "has no end time: the run did not finish"));
    }
    let mut listed = listed_outcomes(&f, &value)?;
    check_totals(&f, &value, &listed)?;
    let mut outcomes = Vec::new();
    let mut durations = Vec::new();
    for m in &mutants {
        let e = listed.by_name.remove(&m.name).ok_or_else(|| {
            run_err(
                &f,
                format!("no outcome for {}: the run is incomplete", m.name),
            )
        })?;
        if e.outcome == Outcome::Unviable {
            check_unviable(dir, &m.name, e.log.as_ref())?;
        }
        outcomes.push(e.outcome);
        durations.push(e.duration);
    }
    if let Some(extra) = listed.by_name.keys().next() {
        return Err(run_err(
            &f,
            format!("an outcome for {extra}, which mutants.json does not list"),
        ));
    }
    Ok(Run {
        dir: dir.to_path_buf(),
        mutants,
        outcomes,
        durations,
    })
}

/// Which gate is judging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// A pull request's diff.
    Diff,
    /// The weekly run over everything, in shards.
    Full,
}

/// The key a ledger entry matches: (file, description, trimmed text of the mutant's first line).
type Key = (String, String, String);

/// A mutant with what happened to it, where it is and its key.
#[derive(Clone, Debug)]
struct Judged {
    mutant: Mutant,
    outcome: Outcome,
    place: Place,
    key: Key,
}

/// Totals for one crate and set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// Caught by a test.
    pub caught: u64,
    /// Did not build.
    pub unviable: u64,
    /// Survived, kept by the ledger.
    pub kept: u64,
    /// Timed out, kept by the ledger as `hangs`.
    pub hangs: u64,
    /// Survived or timed out with no entry that allows it: each fails the gate.
    pub unexplained: u64,
}

/// What [`judge`] decided.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Verdict {
    /// Totals by (crate, set name).
    pub tallies: BTreeMap<(String, &'static str), Tally>,
    /// How many mutants the runs tested, and how long they took in all, build and test: what
    /// [`MINUTES_PER_MUTANT`] is measured from.
    pub time: (u64, Duration),
    /// Every reason the gate fails, one line each. Empty means it passes.
    pub failures: Vec<String>,
}

impl Verdict {
    /// Whether the gate passes.
    #[must_use]
    pub fn passes(&self) -> bool {
        self.failures.is_empty()
    }
}

/// The trimmed text of line `line` of `file` under `repo`, with the file's lines cached.
fn line_of(
    repo: &Path,
    cache: &mut BTreeMap<String, Option<Vec<String>>>,
    file: &str,
    line: usize,
) -> Option<String> {
    let lines = cache.entry(file.to_owned()).or_insert_with(|| {
        std::fs::read_to_string(repo.join(file))
            .ok()
            .map(|t| t.lines().map(|l| l.trim().to_owned()).collect())
    });
    lines.as_ref()?.get(line.checked_sub(1)?).cloned()
}

/// Whether the ledger's `kind` may keep a mutant with `outcome` in `set`.
fn allowed(kind: Kind, outcome: Outcome, set: Set) -> bool {
    match (kind, outcome) {
        (Kind::Hangs, Outcome::Timeout) | (Kind::Equivalent | Kind::Proved, Outcome::Missed) => {
            true
        }
        (Kind::Harmless | Kind::Known, Outcome::Missed) => set == Set::Elsewhere,
        _ => false,
    }
}

/// The ledger's own entries: where they are, and that their line is still there.
fn check_entries(ledger: &Ledger, repo: &Path, failures: &mut Vec<String>) {
    for s in &ledger.survivors {
        match place(&s.file) {
            Place::Kernel { set, .. } => {
                if set == Set::Foundation && matches!(s.kind, Kind::Harmless | Kind::Known) {
                    failures.push(format!(
                        "ledger: {} in {} is kept as {}, but the foundation set keeps only \
                         equivalent mutants (28.8)",
                        s.mutant,
                        s.file,
                        s.kind.name()
                    ));
                }
            }
            _ => failures.push(format!(
                "ledger: {} is not a source file of a kernel crate",
                s.file
            )),
        }
        let text = std::fs::read_to_string(repo.join(&s.file)).ok();
        let present = text
            .as_ref()
            .is_some_and(|t| t.lines().any(|l| l.trim() == s.line_text));
        if !present {
            failures.push(format!(
                "ledger: {} has no line {:?} (for {}): the code changed, so judge the mutant again \
                 and update or remove the entry",
                s.file, s.line_text, s.mutant
            ));
        }
        if let Some(proof) = &s.proof {
            let name = proof.rsplit_once("::").map_or(proof.as_str(), |(_, n)| n);
            if !text.as_ref().is_some_and(|t| is_harness(t, name)) {
                failures.push(format!(
                    "ledger: {} in {} is kept as proved by {proof}, but that file has no \
                     #[kani::proof] function {name}",
                    s.mutant, s.file
                ));
            }
        }
    }
}

/// Whether `text` has a function `name` marked `#[kani::proof]` (other attributes may stand
/// between the two).
fn is_harness(text: &str, name: &str) -> bool {
    let head = format!("fn {name}(");
    let mut marked = false;
    for line in text.lines().map(str::trim) {
        if line == "#[kani::proof]" {
            marked = true;
        } else if marked && line.starts_with(&head) {
            return true;
        } else if !line.starts_with("#[") {
            marked = false;
        }
    }
    false
}

/// Every mutant of every run once, with its key; shards that overlap and lines that cannot be
/// read are failures. Also returns every name seen.
fn collect(
    runs: &[Run],
    repo: &Path,
    failures: &mut Vec<String>,
) -> (Vec<Judged>, BTreeSet<String>) {
    let mut cache = BTreeMap::new();
    let mut judged = Vec::new();
    let mut seen: BTreeMap<String, PathBuf> = BTreeMap::new();
    for run in runs {
        for (m, &outcome) in run.mutants.iter().zip(&run.outcomes) {
            if let Some(first) = seen.insert(m.name.clone(), run.dir.clone()) {
                failures.push(format!(
                    "{} is in two runs ({} and {}): shards must not overlap",
                    m.name,
                    first.display(),
                    run.dir.display()
                ));
                continue;
            }
            let Some(text) = line_of(repo, &mut cache, &m.file, m.line) else {
                failures.push(format!(
                    "{}: line {} of {} cannot be read",
                    m.name, m.line, m.file
                ));
                continue;
            };
            judged.push(Judged {
                mutant: m.clone(),
                outcome,
                place: place(&m.file),
                key: (m.file.clone(), m.description.clone(), text),
            });
        }
    }
    (judged, seen.into_keys().collect())
}

/// The runs together must have tested exactly the expected mutants.
fn check_expected(
    mode: Mode,
    expected: Option<&[Mutant]>,
    seen: &BTreeSet<String>,
    failures: &mut Vec<String>,
) {
    let Some(list) = expected else {
        if mode == Mode::Full {
            failures.push(
                "the full run needs the list of every mutant to prove no shard is missing"
                    .to_owned(),
            );
        }
        return;
    };
    let want: BTreeSet<&str> = list.iter().map(|m| m.name.as_str()).collect();
    let got: BTreeSet<&str> = seen.iter().map(String::as_str).collect();
    for missing in want.difference(&got) {
        failures.push(format!("{missing}: listed, but no run tested it"));
    }
    for extra in got.difference(&want) {
        failures.push(format!("{extra}: tested, but not in the list of mutants"));
    }
}

/// What every mutant is judged against: the ledger's entries by key, and in [`Mode::Diff`] the
/// keys the base branch's ledger keeps as `known`.
struct Rules<'l> {
    mode: Mode,
    entries: BTreeMap<Key, &'l Survivor>,
    base_known: BTreeSet<Key>,
}

/// Judges one mutant: updates its tally, records a failure, and counts the ledger entry it uses.
fn judge_one(
    j: &Judged,
    rules: &Rules<'_>,
    matched: &mut BTreeMap<Key, u64>,
    verdict: &mut Verdict,
) {
    let Place::Kernel { krate, set } = &j.place else {
        verdict.failures.push(format!(
            "{}: not in a kernel crate's src/, so the cargo-mutants configuration is wrong",
            j.mutant.name
        ));
        return;
    };
    let set = *set;
    let tally = verdict
        .tallies
        .entry((krate.clone(), set.name()))
        .or_default();
    let entry = rules.entries.get(&j.key).copied();
    let failure = match (j.outcome, entry) {
        (Outcome::Caught, _) => {
            bump(&mut tally.caught);
            entry.map(|s| (s, "caught"))
        }
        (Outcome::Unviable, _) => {
            bump(&mut tally.unviable);
            entry.map(|s| (s, "unviable"))
        }
        (Outcome::Missed, Some(s))
            if s.kind == Kind::Known
                && rules.mode == Mode::Diff
                && !rules.base_known.contains(&j.key) =>
        {
            bump(&mut tally.unexplained);
            verdict.failures.push(format!(
                "{} survived ({} set; line text {:?}): the ledger keeps it as known, but the base \
                 branch's ledger does not, so it is a new unexplained survivor (28.8): catch it, \
                 or explain it as equivalent or harmless",
                j.mutant.name,
                set.name(),
                j.key.2
            ));
            None
        }
        (Outcome::Missed | Outcome::Timeout, Some(s)) if allowed(s.kind, j.outcome, set) => {
            if j.outcome == Outcome::Missed {
                bump(&mut tally.kept);
            } else {
                bump(&mut tally.hangs);
            }
            bump(matched.entry(j.key.clone()).or_insert(0));
            None
        }
        (Outcome::Missed | Outcome::Timeout, found) => {
            bump(&mut tally.unexplained);
            let what = if j.outcome == Outcome::Missed {
                "survived"
            } else {
                "timed out"
            };
            let why = found.map_or_else(
                || "no ledger entry explains it".to_owned(),
                |s| {
                    format!(
                        "the ledger keeps it as {}, which does not allow a mutant that {what} in \
                         the {} set",
                        s.kind.name(),
                        set.name()
                    )
                },
            );
            verdict.failures.push(format!(
                "{} {what} ({} set; line text {:?}): {why}",
                j.mutant.name,
                set.name(),
                j.key.2
            ));
            None
        }
        (Outcome::Unclassified, _) => {
            bump(&mut tally.unexplained);
            verdict.failures.push(format!(
                "{}: cargo-mutants could not classify its outcome",
                j.mutant.name
            ));
            None
        }
    };
    if let Some((s, now)) = failure {
        verdict.failures.push(format!(
            "{}: the ledger keeps it as {}, but it is now {now}: remove the stale entry",
            j.mutant.name,
            s.kind.name()
        ));
    }
}

/// Judges the runs of one gate against the ledger. `repo` is the repository root the runs mutated;
/// `expected` is required in [`Mode::Full`] and is the full list of mutants; `base` is the base
/// branch's ledger, which [`Mode::Diff`] reads for the `known` entries it may use (none when it is
/// `None`), and [`Mode::Full`] ignores.
#[must_use]
pub fn judge(
    runs: &[Run],
    ledger: &Ledger,
    repo: &Path,
    mode: Mode,
    expected: Option<&[Mutant]>,
    base: Option<&Ledger>,
) -> Verdict {
    let mut verdict = Verdict::default();
    check_entries(ledger, repo, &mut verdict.failures);
    let (judged, seen) = collect(runs, repo, &mut verdict.failures);
    for run in runs {
        for d in &run.durations {
            verdict.time.0 = verdict.time.0.saturating_add(1);
            verdict.time.1 = verdict.time.1.saturating_add(*d);
        }
    }
    check_expected(mode, expected, &seen, &mut verdict.failures);
    let rules = Rules {
        mode,
        entries: ledger.survivors.iter().map(|s| (s.key(), s)).collect(),
        base_known: base
            .map(|b| {
                b.survivors
                    .iter()
                    .filter(|s| s.kind == Kind::Known)
                    .map(Survivor::key)
                    .collect()
            })
            .unwrap_or_default(),
    };
    let mut matched: BTreeMap<Key, u64> = BTreeMap::new();
    for j in &judged {
        judge_one(j, &rules, &mut matched, &mut verdict);
    }
    for s in &ledger.survivors {
        let n = matched.get(&s.key()).copied().unwrap_or(0);
        let wrong = match mode {
            Mode::Full => n != s.occurrences,
            Mode::Diff => n > s.occurrences,
        };
        if wrong {
            verdict.failures.push(format!(
                "ledger: {} at {:?} in {} counts {} occurrence(s), but {n} matched{}",
                s.mutant,
                s.line_text,
                s.file,
                s.occurrences,
                if n == 0 {
                    ": the entry is stale, remove it"
                } else {
                    ""
                }
            ));
        }
    }
    verdict
}

fn bump(count: &mut u64) {
    *count = count.saturating_add(1);
}

/// A report of the verdict, for CI's log: totals per crate and set, then every failure.
#[must_use]
pub fn report(verdict: &Verdict) -> String {
    let mut lines =
        vec!["crate        set         caught unviable kept hangs unexplained".to_owned()];
    for ((krate, set), t) in &verdict.tallies {
        lines.push(format!(
            "{krate:<12} {set:<11} {:>6} {:>8} {:>4} {:>5} {:>11}",
            t.caught, t.unviable, t.kept, t.hangs, t.unexplained
        ));
    }
    if verdict.tallies.is_empty() {
        lines.push("(no mutants)".to_owned());
    }
    let (n, total) = verdict.time;
    if let Some(mean) = total.checked_div(u32::try_from(n).unwrap_or(u32::MAX)) {
        lines.push(format!(
            "time: {n} mutant(s), {} min in all, {} s each on average (MINUTES_PER_MUTANT is {})",
            total.as_secs().checked_div(60).unwrap_or(0),
            mean.as_secs(),
            MINUTES_PER_MUTANT
        ));
    }
    if verdict.passes() {
        lines.push("PASS: no unexplained survivor".to_owned());
    } else {
        lines.push(format!("FAIL: {} problem(s)", verdict.failures.len()));
        lines.extend(verdict.failures.iter().map(|f| format!("  {f}")));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// The minutes one mutant takes in CI, building and testing, on average over the weekly run: the
/// report of every run prints the measured mean beside it, so it can be kept true. A GitHub-hosted
/// `ubuntu-latest` runner rebuilds geom-num incrementally, links its integration-test binaries
/// and runs them until one fails (all of them for a survivor).
pub const MINUTES_PER_MUTANT: u64 = 4;

/// The minutes of mutation one shard may plan for: the shard job's timeout (345 minutes, under
/// GitHub's six-hour limit for a job) less its setup and a margin for runs slower than the mean.
pub const SHARD_MINUTES: u64 = 240;

/// The most jobs one GitHub Actions matrix may hold.
pub const MAX_SHARDS: u64 = 256;

/// How many shards a run of `mutants` mutants needs so each shard plans for at most
/// [`SHARD_MINUTES`] at [`MINUTES_PER_MUTANT`], and no fewer: none for no mutant (a diff that
/// touches none), whose judge then has nothing to wait for.
///
/// # Errors
///
/// A description, when the run would need more than [`MAX_SHARDS`] shards: raise the shard budget
/// or split the run before the gate silently loses mutants.
pub fn shard_count(mutants: usize) -> Result<u64, String> {
    let work = u64::try_from(mutants)
        .unwrap_or(u64::MAX)
        .saturating_mul(MINUTES_PER_MUTANT);
    let shards = work.div_ceil(SHARD_MINUTES);
    if shards > MAX_SHARDS {
        return Err(format!(
            "{mutants} mutants at {MINUTES_PER_MUTANT} min each need {shards} shards of \
             {SHARD_MINUTES} min, more than the {MAX_SHARDS} jobs a matrix may hold"
        ));
    }
    Ok(shards)
}

/// The shard plan as JSON for the workflow: `{"count": n, "shards": [0, ..., n - 1]}`. A GitHub
/// Actions matrix may not be empty, so a plan of no shard lists shard 0 all the same; the workflow
/// runs the shard job only when the count is not 0.
#[must_use]
pub fn shard_matrix(count: u64) -> String {
    let shards: Vec<u64> = (0..count.max(1)).collect();
    serde_json::json!({"count": count, "shards": shards}).to_string()
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;

    #[test]
    fn names_split_into_file_line_and_description() {
        let v: Value = serde_json::from_str(
            "{\"name\": \"a/src/b.rs:21:9: replace < with == in f\", \"package\": \"p\", \
             \"file\": \"a/src/b.rs\", \"span\": {\"start\": {\"line\": 21, \"column\": 9}}}",
        )
        .unwrap();
        let m = mutant_from_json(&v).unwrap();
        assert_eq!(m.description, "replace < with == in f");
        assert_eq!(m.line, 21);
        let bad: Value = serde_json::from_str(
            "{\"name\": \"a/src/b.rs:22:9: x\", \"package\": \"p\", \"file\": \"a/src/b.rs\", \
             \"span\": {\"start\": {\"line\": 21, \"column\": 9}}}",
        )
        .unwrap();
        assert!(mutant_from_json(&bad).is_err());
    }

    #[test]
    fn compiler_errors_are_told_from_other_failures() {
        let rustc = "error[E0308]: mismatched types\n  --> src/a.rs:3:5\n   |\n";
        let lint = "error: this arithmetic operation will overflow\n   --> src/a.rs:9:1\n";
        let linker = "error: linking with `cc` failed: exit status: 1\n  |\n  = note: ...\n";
        let killed = "error: could not compile `x` (lib)\n\nCaused by:\n  process didn't exit\n";
        assert!(has_compiler_error(rustc));
        assert!(has_compiler_error(lint));
        assert!(!has_compiler_error(linker));
        assert!(!has_compiler_error(killed));
    }

    #[test]
    fn kinds_follow_28_8() {
        for o in [Outcome::Missed, Outcome::Timeout] {
            assert!(!allowed(Kind::Known, o, Set::Foundation));
            assert!(!allowed(Kind::Harmless, o, Set::Foundation));
        }
        assert!(allowed(Kind::Equivalent, Outcome::Missed, Set::Foundation));
        assert!(allowed(Kind::Hangs, Outcome::Timeout, Set::Foundation));
        assert!(!allowed(Kind::Hangs, Outcome::Missed, Set::Elsewhere));
        assert!(!allowed(Kind::Equivalent, Outcome::Timeout, Set::Elsewhere));
        assert!(allowed(Kind::Known, Outcome::Missed, Set::Elsewhere));
        assert!(allowed(Kind::Proved, Outcome::Missed, Set::Foundation));
        assert!(!allowed(Kind::Proved, Outcome::Timeout, Set::Foundation));
    }

    #[test]
    fn a_proof_must_be_a_harness_of_its_file() {
        let text = "fn f() {}\n#[kani::proof]\n#[kani::unwind(3)]\nfn f_is_exact() {}\n";
        assert!(is_harness(text, "f_is_exact"));
        assert!(!is_harness(text, "f"));
        assert!(!is_harness(
            "#[kani::proof]\nconst X: u8 = 0;\nfn g() {}\n",
            "g"
        ));
    }
}
