//! The coverage gate: line and branch coverage of each kernel crate by its own tests, against the
//! bars of section 28.8, and every `coverage(off)` mark against the proof it cites.
//!
//! What it does: [`parse_lcov`] reads the LCOV file cargo-llvm-cov writes for one crate
//! (`cargo llvm-cov -p <crate> --branch --lcov`). [`measure`] counts, for every file under the
//! crate's `src/`, the lines and branches outside test code (found by [`crate::rust_source`]) and
//! how many of them the crate's tests ran. [`judge`] applies the bars of
//! [`crate::foundation`]: in the foundation set every line run and every branch taken both ways;
//! elsewhere at least 90% of lines. [`check_marks`] checks every `coverage(off)` mark in the kernel's
//! crates against [`crate::quality_ledger::Ledger::coverage_off`].
//!
//! Guarantees, each because a gate that passes when it should fail is worse than none:
//!
//! - **Test code is not counted.** Lines of items behind `#[cfg(test)]` are left out of both the
//!   lines found and the lines run, so tests cannot pad a crate's numbers.
//! - **A line with no code is not counted.** llvm-cov can give a line holding only a comment or an
//!   attribute (`#[must_use]` above a `const fn`) a count of 0 though the function runs
//!   ([`no_code_lines`]); such a line has nothing to run, so it is left out of both counts.
//! - **Nothing is skipped silently.** A source file that defines a function but has no coverage
//!   data was never compiled into the measured tests, and fails; so does a crate with no LCOV file
//!   at all, and a foundation crate whose code has `if` or `while` but whose data holds no branch
//!   record (the run was made without `--branch`).
//! - **A branch outcome the LCOV lines cannot place counts as missed.** `BRF` can count outcomes
//!   that no `BRDA` line lists, where llvm-cov writes branch regions that start at one place as one
//!   pair (rustc's copies of a match guard, one for each alternative of an or-pattern). They are in
//!   the file's own code, so they are counted as found and never as hit, and they fail the
//!   foundation bar, since no ledger entry can name their line.
//! - **A missing foundation module fails.** A crate of [`crate::foundation::FOUNDATION_MODULES`]
//!   with code but no file of its foundation module fails, so moving the module cannot drop it from
//!   the foundation bar.
//! - **Every mark cites a checked proof.** A `coverage(off)` mark must have a comment right above it
//!   citing `proof: <path>::<name>`, the same proof as its ledger entry, and the cited file must
//!   define that function as a Kani harness (`#[kani::proof]`) or a Verus `proof fn`. A ledger
//!   entry with no mark is stale and fails.
//! - **Waiting lines are named, and can only go.** A foundation line the tests do not run passes
//!   only while the ledger's `awaiting_proof` names it, with the piece that proves it unreachable
//!   or tests it ([`excuse_awaiting`]); an entry that matches more or fewer missed lines than it
//!   says fails. When every kernel branch outcome of a file with a count of 0 waits so, which of
//!   them llvm-cov folded no longer matters, and the file's folding note is not a failure.
//! - **Exact arithmetic.** Percentages are compared as integers: hit × 100 ≥ found × bar.
//!
//! Method: the LCOV tracefile records (`SF`, `DA`, `BRDA`, `BRF`, `end_of_record`) that
//! `llvm-cov export -format=lcov` writes (<https://llvm.org/docs/CommandGuide/llvm-cov.html#llvm-cov-export>),
//! each read by the `lcov` crate 0.8 (MIT OR Apache-2.0, <https://docs.rs/lcov/0.8.2/lcov/>);
//! cargo-llvm-cov 0.9.1 (<https://docs.rs/crate/cargo-llvm-cov/0.9.1>) runs it. Branch coverage is
//! rustc's `-Z coverage-options=branch`, nightly only
//! (<https://doc.rust-lang.org/rustc/instrument-coverage.html>), which cargo-llvm-cov's `--branch`
//! turns on.
//!
//! Cost: linear in the size of the LCOV files and the crates' sources.
//!
//! Consulted: LLVM's llvm-cov, Apache-2.0 WITH LLVM-exception,
//!   <https://github.com/rust-lang/llvm-project/blob/1b9c0d5ff9bbe7634aead059efe6b11a7eeba145/llvm/tools/llvm-cov/CoverageExporterLcov.cpp>,
//!   with `CoverageSummaryInfo.cpp` and `CodeCoverage.cpp` beside it at the same commit (the LLVM
//!   23.1.1 the pinned nightly ships), for how its LCOV export counts `BRF` and writes `BRDA`. No
//!   code copied.
//! Consulted: rustc, MIT OR Apache-2.0,
//!   <https://github.com/rust-lang/rust/blob/21b707e3f97e0b522ebd2f277a862339625ad83f/compiler/rustc_mir_build/src/builder/matches/mod.rs>,
//!   with `compiler/rustc_mir_transform/src/coverage/mappings.rs` and
//!   `compiler/rustc_codegen_llvm/src/coverageinfo/mapgen/covfun.rs` at the same commit (the pinned
//!   nightly), for which branch regions it writes and where. No code copied.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::deps::KERNEL_CRATES_DIR;
use crate::foundation::{
    ELSEWHERE_LINES, FOUNDATION_BRANCHES, FOUNDATION_LINES, Place, Set, kernel_crates,
    missing_foundation_module, place,
};
use crate::quality_ledger::{Ledger, Tool};
use crate::rust_source::{ProofTool, Scan, defines_proof, scan};

/// Why coverage data could not be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageError {
    /// What is wrong.
    pub why: String,
}

impl fmt::Display for CoverageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.why)
    }
}

impl std::error::Error for CoverageError {}

fn cov_err(why: impl Into<String>) -> CoverageError {
    CoverageError { why: why.into() }
}

/// One branch record: line, block and branch, as LCOV's `BRDA` names them.
pub type BranchId = (usize, String, String);

/// Coverage data for one source file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileData {
    /// Execution count per line (`DA`).
    pub lines: BTreeMap<usize, u64>,
    /// Times taken per branch outcome (`BRDA`); `None` when its condition never ran (`-`).
    pub branches: BTreeMap<BranchId, Option<u64>>,
    /// The branch outcomes llvm-cov counts (`BRF`): each outcome of each branch region, except
    /// folded ones (a condition that is a constant, such as `while false`). `BRDA` still lists a
    /// folded outcome, always as 0, but writes branch regions that start at one place as one pair,
    /// so either can be the larger; the gate reads only their difference.
    pub found: Option<u64>,
}

/// Reads an LCOV tracefile: the data per source file, keyed by the path its `SF` line gives. The
/// `lcov` crate reads each record; this function checks how they nest.
///
/// # Errors
///
/// [`CoverageError`] for a line that is not an LCOV record (the `lcov` crate's
/// [`lcov::reader::Error`]), a record outside `SF` ... `end_of_record`, two records for one file
/// (llvm-cov writes one), a line or branch listed twice in a record, two `BRF` lines in a record,
/// or a file that ends inside a record.
pub fn parse_lcov(text: &str) -> Result<BTreeMap<String, FileData>, CoverageError> {
    let mut out: BTreeMap<String, FileData> = BTreeMap::new();
    let mut current: Option<String> = None;
    for record in lcov::Reader::new(text.as_bytes()) {
        let record = record.map_err(|e| cov_err(format!("LCOV: {e}")))?;
        let file = match (&record, current.as_ref()) {
            (lcov::Record::SourceFile { path }, None) => {
                let path = path.to_string_lossy().into_owned();
                if out.insert(path.clone(), FileData::default()).is_some() {
                    return Err(cov_err(format!("LCOV: two records for {path}")));
                }
                current = Some(path);
                continue;
            }
            (lcov::Record::SourceFile { path }, Some(_)) => {
                return Err(cov_err(format!(
                    "LCOV: a record for {} inside another record",
                    path.display()
                )));
            }
            (lcov::Record::TestName { .. }, None) => continue,
            (_, None) => {
                return Err(cov_err(format!("LCOV: {record:?} outside a record")));
            }
            (lcov::Record::EndOfRecord, Some(_)) => {
                current = None;
                continue;
            }
            (_, Some(file)) => file.clone(),
        };
        let data = out.entry(file.clone()).or_default();
        match record {
            lcov::Record::LineData { line, count, .. } => {
                let n = to_usize(line);
                if data.lines.insert(n, count).is_some() {
                    return Err(cov_err(format!("LCOV: line {n} listed twice in {file}")));
                }
            }
            lcov::Record::BranchData {
                line,
                block,
                branch,
                taken,
            } => {
                let id = (to_usize(line), block.to_string(), branch.to_string());
                if data.branches.insert(id, taken).is_some() {
                    return Err(cov_err(format!(
                        "LCOV: branch {line},{block},{branch} listed twice in {file}"
                    )));
                }
            }
            lcov::Record::BranchesFound { found } => {
                let earlier = data.found.replace(u64::from(found));
                if earlier.is_some() {
                    return Err(cov_err(format!("LCOV: two BRF lines for {file}")));
                }
            }
            // FN, FNDA, FNF, FNH, BRH, LF and LH are summaries this gate recomputes itself; a test
            // name inside a record names the run.
            _ => {}
        }
    }
    if current.is_some() {
        return Err(cov_err("LCOV: the file ends inside a record"));
    }
    Ok(out)
}

fn to_usize(n: u32) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

/// Lines and branches found and run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Lines with code.
    pub lines_found: u64,
    /// Lines with code the tests ran.
    pub lines_hit: u64,
    /// Branch outcomes.
    pub branches_found: u64,
    /// Branch outcomes the tests took.
    pub branches_hit: u64,
}

impl Counts {
    fn add(&mut self, other: Counts) {
        self.lines_found = self.lines_found.saturating_add(other.lines_found);
        self.lines_hit = self.lines_hit.saturating_add(other.lines_hit);
        self.branches_found = self.branches_found.saturating_add(other.branches_found);
        self.branches_hit = self.branches_hit.saturating_add(other.branches_hit);
    }
}

/// `hit` out of `found` as a percentage with two decimals, for reports; `-` when nothing is
/// found.
#[must_use]
pub fn percent(hit: u64, found: u64) -> String {
    let basis = u128::from(hit)
        .checked_mul(10_000)
        .and_then(|h| h.checked_div(u128::from(found)));
    match basis {
        Some(b) => format!(
            "{}.{:02}%",
            b.checked_div(100).unwrap_or(0),
            b.checked_rem(100).unwrap_or(0)
        ),
        None => "-".to_owned(),
    }
}

/// Whether `hit` out of `found` reaches `bar` percent, exactly; nothing found always does.
#[must_use]
pub fn meets(hit: u64, found: u64, bar: u64) -> bool {
    u128::from(hit).checked_mul(100) >= u128::from(found).checked_mul(u128::from(bar))
}

/// One source file's numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileReport {
    /// The file, relative to the repository root, with `/` separators.
    pub file: String,
    /// The bar it is held to.
    pub set: Set,
    /// Its numbers.
    pub counts: Counts,
    /// Lines with code the tests never ran.
    pub missed_lines: Vec<usize>,
    /// Lines with a branch outcome the tests never took.
    pub missed_branches: Vec<usize>,
    /// Missed lines the ledger's `awaiting_proof` excuses, moved here from `missed_lines` by
    /// [`excuse_awaiting`].
    pub awaiting_lines: Vec<usize>,
    /// Lines with untaken branch outcomes the ledger excuses, moved here from `missed_branches`.
    pub awaiting_branches: Vec<usize>,
    /// Whether the file has `if` or `while` outside test code.
    pub has_branch_sites: bool,
    /// Branch outcomes `BRF` counts beyond the `BRDA` lines, which no line can be given: in
    /// `counts` as found and never as hit, and no ledger entry can excuse them.
    pub unplaced_branches: u64,
    /// Why its branch count is not exact, when folded or unplaced outcomes cannot be placed (see
    /// [`measure`]); the foundation bar then fails the file.
    pub branch_note: Option<String>,
}

/// One crate's numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrateReport {
    /// The crate.
    pub krate: String,
    /// Every source file with data, in path order.
    pub files: Vec<FileReport>,
    /// Problems found while measuring: a file never compiled into the tests, a missing
    /// foundation module.
    pub problems: Vec<String>,
}

impl CrateReport {
    /// The totals of the files held to `set`.
    #[must_use]
    pub fn totals(&self, set: Set) -> Counts {
        let mut c = Counts::default();
        for f in self.files.iter().filter(|f| f.set == set) {
            c.add(f.counts);
        }
        c
    }
}

/// The repository-relative, `/`-separated form of `path` (absolute, as LCOV writes it), or `None`
/// when it is not inside `repo`.
fn relative(repo: &Path, path: &str) -> Option<String> {
    let canonical = std::fs::canonicalize(path).ok()?;
    let rel = canonical.strip_prefix(repo).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

/// Every `.rs` file under `dir`, recursively, in path order; none when `dir` does not exist.
fn rust_files(dir: &Path) -> Result<Vec<PathBuf>, CoverageError> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries =
            std::fs::read_dir(&d).map_err(|e| cov_err(format!("{}: {e}", d.display())))?;
        for entry in entries {
            let path = entry
                .map_err(|e| cov_err(format!("{}: {e}", d.display())))?
                .path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The folder of kernel crate `krate` under `repo`.
fn crate_dir(repo: &Path, krate: &str) -> PathBuf {
    KERNEL_CRATES_DIR
        .iter()
        .fold(repo.to_path_buf(), |p, c| p.join(c))
        .join(krate)
}

/// The repository-relative path of a file found under `repo`.
fn rel_of(repo: &Path, path: &Path) -> Result<String, CoverageError> {
    let rel = path
        .strip_prefix(repo)
        .map_err(|_| cov_err(format!("{} is outside the repository", path.display())))?;
    Ok(rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/"))
}

/// Every source file of `krate` under `repo` that is not test code, with its repository-relative
/// path and what [`scan`] found, in path order. The files of out-of-line test modules
/// (`#[cfg(test)] mod name;` in a file whose module folder is `dir`: `dir/name.rs` and everything
/// under `dir/name/`) are test code as a whole and are left out.
///
/// # Errors
///
/// [`CoverageError`] if a source file cannot be listed, read or scanned.
fn crate_sources(repo: &Path, krate: &str) -> Result<Vec<(String, Scan)>, CoverageError> {
    let mut all = Vec::new();
    for path in rust_files(&crate_dir(repo, krate).join("src"))? {
        let rel = rel_of(repo, &path)?;
        let text = std::fs::read_to_string(&path).map_err(|e| cov_err(format!("{rel}: {e}")))?;
        let scanned = scan(&text).map_err(|e| cov_err(format!("{rel}: {e}")))?;
        all.push((path, rel, scanned));
    }
    let mut test_paths: Vec<PathBuf> = Vec::new();
    for (path, _, scanned) in &all {
        let Some(parent) = path.parent() else {
            continue;
        };
        let owns_folder = path
            .file_name()
            .is_some_and(|n| n == "lib.rs" || n == "main.rs" || n == "mod.rs");
        let dir = match path.file_stem() {
            Some(stem) if !owns_folder => parent.join(stem),
            _ => parent.to_path_buf(),
        };
        for name in &scanned.test_modules {
            test_paths.push(dir.join(format!("{name}.rs")));
            test_paths.push(dir.join(name));
        }
    }
    Ok(all
        .into_iter()
        .filter(|(path, _, _)| !test_paths.iter().any(|t| path.starts_with(t)))
        .map(|(_, rel, scanned)| (rel, scanned))
        .collect())
}

/// The branch outcomes of one file outside test code.
struct Branches {
    found: u64,
    hit: u64,
    missed_lines: Vec<usize>,
    /// Outcomes counted in `found` and missed that no line can be given.
    unplaced: u64,
    problem: Option<String>,
}

fn to_u64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// Counts a file's branch outcomes outside test code, leaving out folded ones.
///
/// `BRF` and the `BRDA` lines are two different counts in llvm-cov's LCOV export (LLVM 23.1.1, as
/// the pinned nightly ships it), and they can differ either way:
///
/// - **Folded outcomes make `BRDA` the longer.** An outcome folded away (a constant condition)
///   is listed in `BRDA`, as 0, but `sumBranches` leaves it out of `BRF`
///   (<https://github.com/rust-lang/llvm-project/blob/1b9c0d5ff9bbe7634aead059efe6b11a7eeba145/llvm/tools/llvm-cov/CoverageSummaryInfo.cpp#L19-L37>).
/// - **Merged regions make `BRF` the larger.** `BRF` counts each branch region of each function,
///   but `BRDA` writes all the regions that start at one line and column as one pair with their
///   counts added (`combineInstanceCounts`,
///   <https://github.com/rust-lang/llvm-project/blob/1b9c0d5ff9bbe7634aead059efe6b11a7eeba145/llvm/tools/llvm-cov/CoverageExporterLcov.cpp#L161-L179>,
///   which export's default `-unify-instantiations` turns on to join a generic function's
///   copies). rustc lowers a match guard on an or-pattern once for each alternative, each copy a
///   region with the guard's own span
///   (<https://github.com/rust-lang/rust/blob/21b707e3f97e0b522ebd2f277a862339625ad83f/compiler/rustc_mir_build/src/builder/matches/mod.rs#L491-L551>),
///   so `Err(A | B) if g` has four outcomes in `BRF` and two in `BRDA`.
///
/// LCOV gives only the difference. When `BRDA` lists more, the file has `folded = BRDA lines −
/// BRF` folded outcomes, each with a count of 0, but which ones is not written. When every outcome
/// with a count of 0 is folded, or all of them are outside test code, the outcomes outside test
/// code are counted exactly. Otherwise the folded ones cannot be placed, so every outcome of 0
/// outside test code is counted as missed and the file is reported, which can only make the gate
/// stricter.
///
/// When `BRF` counts more, the outcomes beyond the `BRDA` lines are copies merged into a pair on
/// the same line, so in the file's own code, never in a macro of `core` or `std`: rustc writes no
/// expansion regions and drops every branch span a macro introduces
/// (<https://github.com/rust-lang/rust/blob/21b707e3f97e0b522ebd2f277a862339625ad83f/compiler/rustc_mir_transform/src/coverage/mappings.rs#L81-L86>),
/// and a guard written inside `matches!` is the file's own text. Which line holds them is not
/// written, nor whether each copy ran (a sum can hide a copy that never did), so unless every
/// `BRDA` line is test code they are counted as found and never as hit, and the file is reported.
fn branch_counts(rel: &str, d: &FileData, scanned: &Scan) -> Result<Branches, CoverageError> {
    let all = to_u64(d.branches.len());
    let (folded, beyond) = match d.found {
        Some(found) => (all.saturating_sub(found), found.saturating_sub(all)),
        None if all == 0 => (0, 0),
        None => return Err(cov_err(format!("{rel}: BRDA lines without a BRF line"))),
    };
    let zero = |t: &Option<u64>| !t.is_some_and(|t| t > 0);
    let (kernel, test): (Vec<_>, Vec<_>) = d
        .branches
        .iter()
        .partition(|(k, _)| !scanned.is_test_line(k.0));
    let kernel_zero = to_u64(kernel.iter().filter(|(_, t)| zero(t)).count());
    let test_zero = to_u64(test.iter().filter(|(_, t)| zero(t)).count());
    let hit = to_u64(kernel.len()).saturating_sub(kernel_zero);
    let mut missed_lines: Vec<usize> = Vec::new();
    for ((line, _, _), t) in &kernel {
        if zero(t) && missed_lines.last() != Some(line) {
            missed_lines.push(*line);
        }
    }
    // A merged copy shares its line with a BRDA record, so it is test code only when every BRDA
    // line is; with no BRDA line at all it cannot be placed.
    let unplaced = if kernel.is_empty() && !test.is_empty() {
        0
    } else {
        beyond
    };
    let all_zero = kernel_zero.saturating_add(test_zero);
    let (found, problem) = if unplaced > 0 {
        (
            to_u64(kernel.len()).saturating_add(unplaced),
            Some(format!(
                "{rel}: BRF counts {unplaced} branch outcome(s) that no BRDA line lists: llvm-cov \
                 writes the branch regions that start at one place as one BRDA pair, such as \
                 rustc's copy of a match guard for each alternative of an or-pattern; they cannot \
                 be placed, so they are counted as missed (give each alternative an arm and guard \
                 of its own, or test the condition inside the arm)"
            )),
        )
    } else if folded == 0 {
        (to_u64(kernel.len()), None)
    } else if all_zero == folded {
        missed_lines.clear();
        (to_u64(kernel.len()).saturating_sub(kernel_zero), None)
    } else if test_zero == 0 {
        (to_u64(kernel.len()).saturating_sub(folded), None)
    } else {
        (
            to_u64(kernel.len()),
            Some(format!(
                "{rel}: {folded} folded branch outcome(s) among {all_zero} with a count of 0, in \
                 test and kernel code both, so they cannot be told apart; every one outside test \
                 code is counted as missed (llvm-cov show --show-branches=count marks the folded)"
            )),
        )
    };
    Ok(Branches {
        found,
        hit,
        missed_lines,
        unplaced,
        problem,
    })
}

/// The lines of `text` that hold no code: blank, a line comment, or one attribute alone
/// (`#[...]` on a line of its own). Numbered from 1.
#[must_use]
pub fn no_code_lines(text: &str) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        let attribute = t.starts_with("#[") && t.ends_with(']');
        if t.is_empty() || t.starts_with("//") || attribute {
            out.insert(i.saturating_add(1));
        }
    }
    out
}

/// Measures crate `krate` from the LCOV text of a run of its own tests. `repo` must be the
/// canonical repository root.
///
/// # Errors
///
/// [`CoverageError`] if the LCOV text is malformed or a source file cannot be read or scanned.
pub fn measure(repo: &Path, krate: &str, lcov: &str) -> Result<CrateReport, CoverageError> {
    let mut data: BTreeMap<String, FileData> = BTreeMap::new();
    for (path, d) in parse_lcov(lcov)? {
        if let Some(rel) = relative(repo, &path) {
            data.insert(rel, d);
        }
    }
    let mut report = CrateReport {
        krate: krate.to_owned(),
        files: Vec::new(),
        problems: Vec::new(),
    };
    let mut rels = Vec::new();
    let mut any_code = false;
    for (rel, scanned) in crate_sources(repo, krate)? {
        any_code |= scanned.has_functions;
        rels.push(rel.clone());
        let Place::Kernel { set, .. } = place(&rel) else {
            continue;
        };
        let Some(d) = data.get(&rel) else {
            if scanned.has_functions {
                report.problems.push(format!(
                    "{rel} defines functions but has no coverage data: it was never compiled into \
                     {krate}'s tests"
                ));
            }
            continue;
        };
        let mut counts = Counts::default();
        let mut missed_lines = Vec::new();
        let mut missed_branches = Vec::new();
        let text = std::fs::read_to_string(repo.join(&rel))
            .map_err(|e| cov_err(format!("{rel} cannot be read: {e}")))?;
        let silent = no_code_lines(&text);
        for (&line, &count) in d
            .lines
            .iter()
            .filter(|(l, _)| !scanned.is_test_line(**l) && !silent.contains(*l))
        {
            counts.lines_found = counts.lines_found.saturating_add(1);
            if count > 0 {
                counts.lines_hit = counts.lines_hit.saturating_add(1);
            } else {
                missed_lines.push(line);
            }
        }
        let branches = branch_counts(&rel, d, &scanned)?;
        counts.branches_found = branches.found;
        counts.branches_hit = branches.hit;
        missed_branches.extend(branches.missed_lines);
        report.files.push(FileReport {
            file: rel,
            set,
            counts,
            missed_lines,
            missed_branches,
            awaiting_lines: Vec::new(),
            awaiting_branches: Vec::new(),
            has_branch_sites: !scanned.branch_sites.is_empty(),
            unplaced_branches: branches.unplaced,
            branch_note: branches.problem,
        });
    }
    if let Some(module) = missing_foundation_module(krate, any_code, &rels) {
        report.problems.push(format!(
            "{krate} has code but no file src/{module}.rs or src/{module}/: its foundation module \
             (28.8) is missing, so update foundation::FOUNDATION_MODULES to where it now lives"
        ));
    }
    Ok(report)
}

/// Checks the output of the cargo-llvm-cov run that measured `krate` (its standard error, or both
/// streams): `None` when it is clean, or why the data cannot be trusted.
///
/// llvm-cov warns "N functions have mismatched data" when a function's coverage record does not
/// match its profile. llvm-cov then skips that record, and keeps only the first record of each
/// function name it reads, so a function can lose its counts (reported as never run) or vanish from
/// the report (its lines no longer counted at all, which would make the bar easier). Both happen
/// in optimised builds when rustc lets small functions be inlined into other crates, which
/// `-Z cross-crate-inline-threshold=never` stops; the workflow passes it, and this check fails a
/// run where it did not work. (The workflow also passes `-C link-dead-code`, so a function the
/// crate never calls itself has no all-zero record to shadow the copy a test runs; that case
/// gives no warning, and the seeded gates test it.)
#[must_use]
pub fn check_log(krate: &str, log: &str) -> Option<String> {
    let line = log.lines().find(|l| l.contains("mismatched data"))?;
    Some(format!(
        "{krate}: llvm-cov warned {:?}, so its coverage data cannot be trusted (see \
         geom_test::coverage::check_log)",
        line.trim()
    ))
}

/// The gate's decisions for measured crates: every reason to fail, one line each.
#[must_use]
pub fn judge(reports: &[CrateReport]) -> Vec<String> {
    let mut failures = Vec::new();
    for r in reports {
        failures.extend(r.problems.iter().cloned());
        let f = r.totals(Set::Foundation);
        let e = r.totals(Set::Elsewhere);
        let foundation_files = r.files.iter().filter(|x| x.set == Set::Foundation);
        let branch_sites = foundation_files.clone().any(|x| x.has_branch_sites);
        if branch_sites && f.branches_found == 0 {
            failures.push(format!(
                "{}: the foundation code has branches but the data has no branch record: measure \
                 with --branch on the pinned nightly",
                r.krate
            ));
        }
        for x in foundation_files {
            // Unplaced outcomes have no line for an entry to name, so they never wait.
            let branches_wait = x.unplaced_branches == 0
                && x.missed_branches.is_empty()
                && !x.awaiting_branches.is_empty();
            if let Some(note) = &x.branch_note
                && !branches_wait
            {
                failures.push(note.clone());
            }
            // Every miss the counts show is either listed, or waits for its proof.
            let lines_wait = x.missed_lines.is_empty() && !x.awaiting_lines.is_empty();
            if !lines_wait && !meets(x.counts.lines_hit, x.counts.lines_found, FOUNDATION_LINES) {
                failures.push(format!(
                    "{}: lines {}/{} ({}), below {FOUNDATION_LINES}%; never run: {}",
                    x.file,
                    x.counts.lines_hit,
                    x.counts.lines_found,
                    percent(x.counts.lines_hit, x.counts.lines_found),
                    list(&x.missed_lines)
                ));
            }
            if !branches_wait
                && !meets(
                    x.counts.branches_hit,
                    x.counts.branches_found,
                    FOUNDATION_BRANCHES,
                )
            {
                failures.push(format!(
                    "{}: branches {}/{} ({}), below {FOUNDATION_BRANCHES}%; not taken both ways \
                     on lines: {}",
                    x.file,
                    x.counts.branches_hit,
                    x.counts.branches_found,
                    percent(x.counts.branches_hit, x.counts.branches_found),
                    list(&x.missed_branches)
                ));
            }
        }
        if !meets(e.lines_hit, e.lines_found, ELSEWHERE_LINES) {
            failures.push(format!(
                "{}: lines {}/{} ({}) outside the foundation set, below {ELSEWHERE_LINES}%",
                r.krate,
                e.lines_hit,
                e.lines_found,
                percent(e.lines_hit, e.lines_found)
            ));
        }
    }
    failures
}

/// Moves every missed line and branch line of the foundation set that the ledger's
/// `awaiting_proof` names out of `missed_lines` and `missed_branches`, into `awaiting_lines` and
/// `awaiting_branches`, so [`judge`] accepts them, and returns every entry that does not fit, one
/// line each.
///
/// An entry fits when exactly `occurrences` lines of its file, missed as lines or as branches,
/// have its `line_text` as their trimmed text. An entry that matches another number excuses none
/// of them and fails: fewer means the line runs now, changed or is gone, so the entry must go; more
/// means a new line no test runs, which needs its own reason. An entry outside the foundation set,
/// or for a file the run has no data for, fails too. Entries of crates the run did not measure are
/// left for the run that does.
///
/// # Errors
///
/// [`CoverageError`] if a file an entry names cannot be read.
pub fn excuse_awaiting(
    repo: &Path,
    reports: &mut [CrateReport],
    ledger: &Ledger,
) -> Result<Vec<String>, CoverageError> {
    let mut failures = Vec::new();
    for entry in &ledger.awaiting_proof {
        let what = format!(
            "{}: awaiting_proof entry {:?} ({})",
            entry.file, entry.line_text, entry.piece
        );
        let Place::Kernel { krate, set } = place(&entry.file) else {
            failures.push(format!("{what} is not a kernel source file"));
            continue;
        };
        let Some(report) = reports.iter_mut().find(|r| r.krate == krate) else {
            continue;
        };
        if set != Set::Foundation {
            failures.push(format!(
                "{what} is outside the foundation set, whose bar needs no entry"
            ));
            continue;
        }
        let Some(file) = report.files.iter_mut().find(|f| f.file == entry.file) else {
            failures.push(format!(
                "{what} names a file the run has no data for: remove it if the file is gone"
            ));
            continue;
        };
        let text = std::fs::read_to_string(repo.join(&entry.file))
            .map_err(|e| cov_err(format!("{} cannot be read: {e}", entry.file)))?;
        let lines: Vec<&str> = text.lines().collect();
        let matching = |line: usize| {
            line.checked_sub(1)
                .and_then(|i| lines.get(i))
                .is_some_and(|t| t.trim() == entry.line_text)
        };
        let found: BTreeSet<usize> = file
            .missed_lines
            .iter()
            .chain(&file.missed_branches)
            .copied()
            .filter(|&l| matching(l))
            .collect();
        if to_u64(found.len()) != entry.occurrences {
            failures.push(format!(
                "{what} expects {} missed line(s) with that text, and the run has {} ({}): remove \
                 or correct it, or test the line",
                entry.occurrences,
                found.len(),
                list(&found.iter().copied().collect::<Vec<_>>())
            ));
            continue;
        }
        let (wait, miss): (Vec<usize>, Vec<usize>) =
            file.missed_lines.iter().partition(|l| found.contains(l));
        file.awaiting_lines.extend(wait);
        file.awaiting_lines.sort_unstable();
        file.missed_lines = miss;
        let (wait, miss): (Vec<usize>, Vec<usize>) =
            file.missed_branches.iter().partition(|l| found.contains(l));
        file.awaiting_branches.extend(wait);
        file.awaiting_branches.sort_unstable();
        file.missed_branches = miss;
    }
    Ok(failures)
}

/// The foundation lines that wait for their proofs ([`excuse_awaiting`]), one line per file, for
/// the gate to print beside its verdict.
#[must_use]
pub fn awaiting_notes(reports: &[CrateReport]) -> Vec<String> {
    let mut out = Vec::new();
    for r in reports {
        for x in &r.files {
            let mut parts = Vec::new();
            if !x.awaiting_lines.is_empty() {
                parts.push(format!("lines {}", list(&x.awaiting_lines)));
            }
            if !x.awaiting_branches.is_empty() {
                parts.push(format!("branches on lines {}", list(&x.awaiting_branches)));
            }
            if !parts.is_empty() {
                out.push(format!(
                    "{}: {} wait for their proofs",
                    x.file,
                    parts.join(" and ")
                ));
            }
        }
    }
    out
}

fn list(lines: &[usize]) -> String {
    lines
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// A table of the reports, file by file with crate totals, for CI's log.
#[must_use]
pub fn table(reports: &[CrateReport]) -> String {
    let mut lines = Vec::new();
    for r in reports {
        lines.push(r.krate.clone());
        for x in &r.files {
            lines.push(format!(
                "  {:<10} {:<60} lines {:>5}/{:<5} {:>8}  branches {:>4}/{:<4} {:>8}",
                x.set.name(),
                x.file,
                x.counts.lines_hit,
                x.counts.lines_found,
                percent(x.counts.lines_hit, x.counts.lines_found),
                x.counts.branches_hit,
                x.counts.branches_found,
                percent(x.counts.branches_hit, x.counts.branches_found)
            ));
        }
        for set in [Set::Foundation, Set::Elsewhere] {
            let c = r.totals(set);
            if c.lines_found > 0 || c.branches_found > 0 {
                lines.push(format!(
                    "  total {:<10} lines {}/{} {}  branches {}/{} {}",
                    set.name(),
                    c.lines_hit,
                    c.lines_found,
                    percent(c.lines_hit, c.lines_found),
                    c.branches_hit,
                    c.branches_found,
                    percent(c.branches_hit, c.branches_found)
                ));
            }
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Whether the file at `repo`/`path` defines `name` as a proof of `tool`
/// ([`crate::rust_source::defines_proof`]): a Kani harness or a Verus `proof fn`. A file that
/// cannot be read or parsed defines none.
fn proof_exists(repo: &Path, path: &str, name: &str, tool: Tool) -> bool {
    let Ok(text) = std::fs::read_to_string(repo.join(path)) else {
        return false;
    };
    let tool = match tool {
        Tool::Kani => ProofTool::Kani,
        Tool::Verus => ProofTool::Verus,
    };
    defines_proof(&text, name, tool).unwrap_or(false)
}

/// Checks every `coverage(off)` mark in the kernel crates under `repo` against the ledger (see the
/// module documentation): every reason to fail, one line each.
///
/// # Errors
///
/// [`CoverageError`] if a crate's sources cannot be listed, read or scanned.
pub fn check_marks(repo: &Path, ledger: &Ledger) -> Result<Vec<String>, CoverageError> {
    let mut failures = Vec::new();
    let crates = kernel_crates(repo).map_err(|e| cov_err(e.to_string()))?;
    if crates.is_empty() {
        return Err(cov_err(
            "no kernel crate found: a check that checked nothing must not pass",
        ));
    }
    let mut seen = BTreeSet::new();
    for krate in &crates {
        for path in rust_files(&crate_dir(repo, krate).join("src"))? {
            let rel = rel_of(repo, &path)?;
            let text =
                std::fs::read_to_string(&path).map_err(|e| cov_err(format!("{rel}: {e}")))?;
            let scanned = scan(&text).map_err(|e| cov_err(format!("{rel}: {e}")))?;
            for mark in &scanned.coverage_off {
                let at = format!("{rel}:{}", mark.line);
                let entry = ledger
                    .coverage_off
                    .iter()
                    .find(|c| c.file == rel && c.item == mark.item);
                let Some(entry) = entry else {
                    failures.push(format!(
                        "{at}: coverage(off) on {:?} has no ledger entry citing its proof",
                        mark.item
                    ));
                    continue;
                };
                seen.insert((entry.file.clone(), entry.item.clone()));
                if mark.proof.as_deref() != Some(entry.proof.as_str()) {
                    failures.push(format!(
                        "{at}: the comment above the mark must cite `proof: {}` (it cites {:?})",
                        entry.proof, mark.proof
                    ));
                }
                let found = entry
                    .proof
                    .rsplit_once("::")
                    .is_some_and(|(p, n)| proof_exists(repo, p, n, entry.tool));
                if !found {
                    failures.push(format!(
                        "{at}: the cited proof {} is not a {} proof in that file",
                        entry.proof,
                        entry.tool.name()
                    ));
                }
            }
        }
    }
    for c in &ledger.coverage_off {
        if !seen.contains(&(c.file.clone(), c.item.clone())) {
            failures.push(format!(
                "ledger: coverage_off for {:?} in {} matches no mark: the entry is stale",
                c.item, c.file
            ));
        }
    }
    Ok(failures)
}

/// Whether kernel crate `krate` under `repo` has code to measure: a function defined outside
/// test code in some file under its `src/`. A crate without one (a scaffold of declarations) has no
/// line to count, and cargo-llvm-cov cannot report on it ("no coverage data found"), so the gate
/// skips it, and says so.
///
/// # Errors
///
/// [`CoverageError`] if a source file cannot be read or scanned.
pub fn has_code(repo: &Path, krate: &str) -> Result<bool, CoverageError> {
    Ok(crate_sources(repo, krate)?
        .iter()
        .any(|(_, scanned)| scanned.has_functions))
}

/// The kernel crates a pull request's coverage gate measures: every crate whose folder a changed
/// file is in, and every kernel crate that depends on one of those, directly or not, by any kind
/// of dependency (a dev-dependency's change moves the tests too). A changed file outside the
/// kernel crates' folders (the workspace manifest, the lock file, cargo's configuration, the
/// toolchain, the ledger, a workflow) or in a development crate (the gates themselves) can affect
/// every crate, so then every kernel crate is measured.
///
/// `changed` holds paths relative to the repository root; `metadata` is `cargo metadata`'s
/// output for the workspace.
///
/// # Errors
///
/// [`CoverageError`] if the kernel crates cannot be listed or the metadata has no package list.
pub fn affected_crates(
    repo: &Path,
    changed: &[String],
    metadata: &Value,
) -> Result<Vec<String>, CoverageError> {
    let all = kernel_crates(repo).map_err(|e| cov_err(e.to_string()))?;
    let mut set: BTreeSet<String> = BTreeSet::new();
    for file in changed.iter().filter(|f| !f.trim().is_empty()) {
        match place(file.trim()) {
            Place::Kernel { krate, .. } | Place::KernelOther { krate } => {
                set.insert(krate);
            }
            Place::Development { .. } | Place::Outside => return Ok(all),
        }
    }
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| cov_err("cargo metadata has no packages"))?;
    // Reverse edges among kernel crates: dependency -> dependents.
    let mut dependents: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for p in packages {
        let Some(name) = p.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !all.iter().any(|k| k == name) {
            continue;
        }
        for d in p
            .get("dependencies")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(dep) = d.get("name").and_then(Value::as_str) {
                dependents
                    .entry(dep.to_owned())
                    .or_default()
                    .insert(name.to_owned());
            }
        }
    }
    let mut queue: Vec<String> = set.iter().cloned().collect();
    while let Some(k) = queue.pop() {
        for d in dependents.get(&k).into_iter().flatten() {
            if set.insert(d.clone()) {
                queue.push(d.clone());
            }
        }
    }
    Ok(all.into_iter().filter(|k| set.contains(k)).collect())
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
    fn lcov_records_read_and_malformed_ones_fail() {
        let text = "TN:\nSF:/a/b.rs\nDA:3,1\nDA:4,0\nBRDA:3,0,0,2\nBRDA:3,0,1,-\nBRF:2\nBRH:1\n\
                    end_of_record\nSF:/a/c.rs\nDA:4,5\nend_of_record\n";
        let data = parse_lcov(text).unwrap();
        let f = &data["/a/b.rs"];
        assert_eq!(f.lines[&4], 0);
        assert_eq!(f.branches[&(3, "0".to_owned(), "1".to_owned())], None);
        assert_eq!(f.found, Some(2));
        assert_eq!(data["/a/c.rs"].lines[&4], 5);
        for bad in [
            "DA:1,1\n",
            "SF:/a\nDA:x,1\nend_of_record\n",
            "SF:/a\nBRDA:1,0,0\nend_of_record\n",
            "SF:/a\nDA:1,1\n",
            "SF:/a\nSF:/b\nend_of_record\n",
            "SF:/a\nend_of_record\nSF:/a\nend_of_record\n",
            "SF:/a\nDA:1,1\nDA:1,2\nend_of_record\n",
            "SF:/a\nBRF:1\nBRF:1\nend_of_record\n",
        ] {
            assert!(parse_lcov(bad).is_err(), "{bad:?}");
        }
    }

    /// Code on lines `first` to `last` is test code.
    fn tests_on(first: usize, last: usize) -> Scan {
        Scan {
            test_lines: vec![(first, last)],
            ..Scan::default()
        }
    }

    #[test]
    fn outcomes_brf_counts_beyond_the_brda_lines_are_missed() {
        // What the pinned nightly's llvm-cov writes for `Err(E::A | E::B) if g => true` on line 10,
        // its guard run once by way of `E::A`, and `Err(E::A) if g => true` on line 18, never
        // run: rustc lowers the first guard once for each alternative, and LCOV writes the two
        // copies as one pair, so BRF counts six outcomes and BRDA lists four.
        let text = "SF:/a/lib.rs\nBRDA:10,0,0,1\nBRDA:10,0,1,0\nBRDA:18,0,0,-\nBRDA:18,0,1,-\n\
                    BRF:6\nBRH:1\nend_of_record\n";
        let data = parse_lcov(text).unwrap();
        let d = &data["/a/lib.rs"];
        let b = branch_counts("lib.rs", d, &Scan::default()).unwrap();
        assert_eq!((b.found, b.hit, b.unplaced), (6, 1, 2));
        assert_eq!(b.missed_lines, vec![10, 18]);
        assert!(b.problem.is_some_and(|p| p.contains("no BRDA line lists")));
        // A merged pair taken both ways can still hide a copy that never ran: never a hit.
        let taken = "SF:/a/lib.rs\nBRDA:10,0,0,3\nBRDA:10,0,1,2\nBRF:4\nend_of_record\n";
        let data = parse_lcov(taken).unwrap();
        let b = branch_counts("lib.rs", &data["/a/lib.rs"], &Scan::default()).unwrap();
        assert_eq!((b.found, b.hit, b.unplaced), (4, 2, 2));
        assert!(b.missed_lines.is_empty() && b.problem.is_some());
        // While some BRDA line is kernel code the copies may be its, so they count against it.
        let b = branch_counts("lib.rs", d, &tests_on(15, 20)).unwrap();
        assert_eq!((b.found, b.hit, b.unplaced), (4, 1, 2));
        assert_eq!(b.missed_lines, vec![10]);
        // A copy shares its line with a BRDA record, so when every one is test code, so is it.
        let b = branch_counts("lib.rs", d, &tests_on(1, 20)).unwrap();
        assert_eq!((b.found, b.hit, b.unplaced), (0, 0, 0));
        assert!(b.missed_lines.is_empty() && b.problem.is_none());
        // With no BRDA line at all, BRF's outcomes have nowhere to be placed.
        let data = parse_lcov("SF:/a/lib.rs\nBRF:2\nend_of_record\n").unwrap();
        let b = branch_counts("lib.rs", &data["/a/lib.rs"], &tests_on(1, 20)).unwrap();
        assert_eq!((b.found, b.hit, b.unplaced), (2, 0, 2));
        assert!(b.problem.is_some());
        // BRDA lines without BRF are still not data the gate can use.
        let data = parse_lcov("SF:/a/lib.rs\nBRDA:10,0,0,1\nend_of_record\n").unwrap();
        assert!(branch_counts("lib.rs", &data["/a/lib.rs"], &Scan::default()).is_err());
    }

    #[test]
    fn unplaced_outcomes_fail_the_foundation_bar_whatever_waits() {
        let file = |unplaced: u64, set: Set| FileReport {
            file: "lib.rs".to_owned(),
            set,
            counts: Counts {
                lines_found: 3,
                lines_hit: 3,
                branches_found: 4_u64.saturating_add(unplaced),
                branches_hit: 3,
            },
            missed_lines: Vec::new(),
            missed_branches: Vec::new(),
            awaiting_lines: Vec::new(),
            awaiting_branches: vec![2],
            has_branch_sites: true,
            unplaced_branches: unplaced,
            branch_note: (unplaced > 0).then(|| "lib.rs: unplaced".to_owned()),
        };
        let report = |f: FileReport| CrateReport {
            krate: "geom-num".to_owned(),
            files: vec![f],
            problems: Vec::new(),
        };
        // Every listed miss waits for its proof: the file passes.
        assert_eq!(
            judge(&[report(file(0, Set::Foundation))]),
            Vec::<String>::new()
        );
        // The same file with two outcomes no line can be given: no entry names them, so it fails,
        // with the note and the bar.
        let f = judge(&[report(file(2, Set::Foundation))]);
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f.iter().any(|x| x == "lib.rs: unplaced"), "{f:?}");
        // Outside the foundation set branches have no bar, so they are only counted.
        assert_eq!(
            judge(&[report(file(2, Set::Elsewhere))]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn bars_compare_exactly() {
        assert!(meets(9, 10, 90));
        assert!(!meets(8, 10, 90));
        assert!(meets(0, 0, 100));
        assert!(!meets(99, 100, 100));
        assert!(meets(u64::MAX, u64::MAX, 100));
        assert_eq!(percent(1, 3), "33.33%");
        assert_eq!(percent(0, 0), "-");
    }
}
