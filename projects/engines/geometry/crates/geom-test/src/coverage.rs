//! The coverage gate: line and branch coverage of each kernel crate by its own tests, against the
//! bars of section 28.8, and every `coverage(off)` mark against the proof it cites.
//!
//! What it does: [`parse_lcov`] reads the LCOV file cargo-llvm-cov writes for one crate
//! (`cargo llvm-cov -p <crate> --branch --lcov`). [`measure`] counts, for every file under the
//! crate's `src/`, the lines and branches outside test code (found by [`crate::rust_source`]) and
//! how many of them the crate's tests ran. [`judge`] applies the bars of
//! [`crate::foundation`]: in the foundation set every line run and every branch taken both ways;
//! elsewhere at least 90% of lines. [`check_marks`] checks every `coverage(off)` mark in the kernel's
//! crates against [`crate::ledger::Ledger::coverage_off`].
//!
//! Guarantees, each because a gate that passes when it should fail is worse than none:
//!
//! - **Test code is not counted.** Lines of items behind `#[cfg(test)]` are left out of both the
//!   lines found and the lines run, so tests cannot pad a crate's numbers.
//! - **Nothing is skipped silently.** A source file that defines a function but has no coverage
//!   data was never compiled into the measured tests, and fails; so does a crate with no LCOV file
//!   at all, and a foundation crate whose code has `if` or `while` but whose data holds no branch
//!   record (the run was made without `--branch`).
//! - **A missing foundation module fails.** A crate of [`crate::foundation::FOUNDATION_MODULES`]
//!   with code but no file of its foundation module fails, so moving the module cannot drop it from
//!   the foundation bar.
//! - **Every mark cites a checked proof.** A `coverage(off)` mark must have a comment right above it
//!   citing `proof: <path>::<name>`, the same proof as its ledger entry, and the cited file must
//!   define that function as a Kani harness (`#[kani::proof]`) or a Verus `proof fn`. A ledger
//!   entry with no mark is stale and fails.
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

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::deps::KERNEL_CRATES_DIR;
use crate::foundation::{
    ELSEWHERE_LINES, FOUNDATION_BRANCHES, FOUNDATION_LINES, Place, Set, kernel_crates,
    missing_foundation_module, place,
};
use crate::ledger::{Ledger, Tool};
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
    /// The branch outcomes llvm-cov counts (`BRF`). It leaves out folded outcomes (a condition
    /// that is a constant, such as `while false`), which `BRDA` still lists, always as 0, so
    /// `branches.len() − found` is the number of folded outcomes.
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
    /// Whether the file has `if` or `while` outside test code.
    pub has_branch_sites: bool,
    /// Why its branch count is not exact, when folded outcomes cannot be placed (see
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
    problem: Option<String>,
}

fn to_u64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// Counts a file's branch outcomes outside test code, leaving out folded ones.
///
/// LCOV lists every outcome in `BRDA`, folded ones (a constant condition) as 0, while `BRF`
/// counts only those that are not folded; so the file has `folded = BRDA lines − BRF` folded
/// outcomes, each with a count of 0, but which ones is not written. When every outcome with a
/// count of 0 is folded, or all of them are outside test code, the outcomes outside test code are
/// counted exactly. Otherwise the folded ones cannot be placed, so every outcome of 0 outside test
/// code is counted as missed and the file is reported, which can only make the gate stricter.
fn branch_counts(rel: &str, d: &FileData, scanned: &Scan) -> Result<Branches, CoverageError> {
    let all = to_u64(d.branches.len());
    let folded = match d.found {
        Some(found) => all.checked_sub(found).ok_or_else(|| {
            cov_err(format!(
                "{rel}: BRF says {found} branch outcomes but BRDA lists {all}"
            ))
        })?,
        None if all == 0 => 0,
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
    let all_zero = kernel_zero.saturating_add(test_zero);
    let (found, problem) = if folded == 0 {
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
        problem,
    })
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
        for (&line, &count) in d.lines.iter().filter(|(l, _)| !scanned.is_test_line(**l)) {
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
            has_branch_sites: !scanned.branch_sites.is_empty(),
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
            if let Some(note) = &x.branch_note {
                failures.push(note.clone());
            }
            if !meets(x.counts.lines_hit, x.counts.lines_found, FOUNDATION_LINES) {
                failures.push(format!(
                    "{}: lines {}/{} ({}), below {FOUNDATION_LINES}%; never run: {}",
                    x.file,
                    x.counts.lines_hit,
                    x.counts.lines_found,
                    percent(x.counts.lines_hit, x.counts.lines_found),
                    list(&x.missed_lines)
                ));
            }
            if !meets(
                x.counts.branches_hit,
                x.counts.branches_found,
                FOUNDATION_BRANCHES,
            ) {
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
