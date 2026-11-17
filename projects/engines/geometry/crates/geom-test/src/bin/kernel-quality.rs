//! `kernel-quality`: the CI tool behind the geometry kernel's mutation and coverage gates and its
//! Miri run (section 28.8; `.github/workflows/kernel-quality.yml`).
//!
//! What it does: one subcommand per step of the workflow, each a thin front to a module of
//! `geom_test`; `kernel-quality --help` lists them and `kernel-quality <command> --help` gives each
//! one's options.
//!
//! - `mutants-plan` reads the mutants of a run from `cargo mutants --list --json` and prints how
//!   many shards the run needs ([`geom_test::mutants::shard_count`]), with the shard lists for the
//!   workflow's two shard jobs, one GitHub Actions matrix each. For a full run narrowed to the
//!   files a regular expression matches, `--files` and `--all` (the listing without `--re`) check
//!   the list holds exactly those files' mutants ([`geom_test::mutants::check_narrowed`]).
//! - `mutants` judges cargo-mutants output directories (`mutants.out`, one per shard) against the
//!   ledger ([`geom_test::mutants`]); `--expected`, the same list, proves no shard is missing, and
//!   in a pull request `--base-ledger` is the base branch's ledger, the only source of `known`
//!   entries a pull request may use.
//! - `coverage` judges `DIR/<crate>.lcov`, one LCOV file per kernel crate measured by its own
//!   tests, with `DIR/<crate>.log`, the output of the cargo-llvm-cov run that wrote it, for every
//!   crate `affected` names, and checks every `coverage(off)` mark in the kernel
//!   ([`geom_test::coverage`]).
//! - `affected` prints the kernel crates a coverage run measures, one per line, leaving out (and
//!   naming on standard error) any without a function to measure ([`geom_test::coverage::has_code`]).
//! - `miri-packages` prints the `-p` arguments of every kernel crate for the Miri run
//!   ([`geom_test::miri`]); with `--changed`, of those whose tests the change reaches.
//! - `miri-plan` plans the Miri gate once for every job of a run: it fails on any test of the
//!   run's nextest listing that Miri's table has no row for ([`geom_test::miri::unlisted`]),
//!   writes each partition's nextest filterset, its finishing tests balanced by their recorded
//!   times, and geom-pool's ([`geom_test::miri::gate_plan`]), and prints the partitions as JSON for
//!   the workflow's matrix.
//! - `miri-times` writes Miri's table from a measuring run's nextest reports (`junit.xml`)
//!   ([`geom_test::miri::write_table`]), the only way its rows change (32.58); with `--dry-run`
//!   it writes nothing and only says what the reports measured, failing on a test that failed.
//!   It writes the list of tests that may stop ([`geom_test::miri::render_may_stop`]) with it, and
//!   `miri-may-stop` writes that list alone from the table in the repository.
//! - `miri-stopped` reads a Miri gate job's nextest reports and names in Markdown, for its
//!   summary, each test nextest stopped with no verdict; it fails when one of them may not stop
//!   ([`geom_test::miri::stops`], [`geom_test::miri::allowed_stops`]).
//! - `miri-measure` plans a measuring run on GitHub's runners (the workflow's dispatch input
//!   `miri_measure`): the tests with no row, or every test, of a Miri listing, packed into parts
//!   that each end within the stop ([`geom_test::miri::measure_plan`]), each part's filterset
//!   written to a file and the parts printed as JSON for the workflow's two part matrices.
//! - `mutants-files` prints the regular expression of the files a push to main's full mutation
//!   run mutates ([`geom_test::mutants::push_files`]).
//! - `mutants-increment` and `coverage-increment` print what a later push to a pull request must
//!   mutate again, or measure the crates of, after the last head whose run finished clean
//!   ([`geom_test::increment`]).
//! - `fuzz-scope` prints which of `geom-fuzz`'s targets and `kernel-fuzz.yml`'s Kani proofs a
//!   change can affect, and why, and writes the step outputs that workflow's jobs read
//!   ([`geom_test::fuzz_scope`]); `.github/workflows/kernel-fuzz.yml`'s job `select` runs it.
//!
//! - `test-scope` prints which crates' tests a change can reach, and why, and writes the step
//!   outputs `kernel.yml`'s suites read ([`geom_test::test_scope`]), with `--parts <n>` also the
//!   release suite's n shares of the crates by their measured seconds; with `--build-of <crate>` it
//!   says instead whether the change reaches what that crate's tests and benchmarks build, for
//!   `kernel-bench.yml`'s count gates. With `--base <commit>`, `test-scope` and `fuzz-scope` read
//!   `MAP.json` and `quality-ledger.json` by their changed entries ([`geom_test::shared`]).
//!
//! Guarantees:
//!
//! - **Exit status says what happened.** 0 when the gate passes, 1 when it fails, 2 when the input
//!   cannot be used (a missing or malformed file, a bad argument, which clap reports with exit
//!   status 2). Every failure is printed, one per line.
//! - **Nothing judged twice by different rules.** The binary holds no rule of its own; the
//!   foundation set, the bars and the ledger's rules live in the library, where the tests reach
//!   them.
//!
//! Method: none; a command-line front end, parsed by clap's derive API
//! (<https://docs.rs/clap/4.6.7/clap/_derive/index.html>), over the library's gates.
//!
//! Cost: what each gate costs (see its module), plus one `cargo metadata` run for `affected
//! --changed`, `miri-packages` and `fuzz-scope`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use geom_test::coverage::{self, CrateReport};
use geom_test::fuzz_scope;
use geom_test::mutants::{self, Mode};
use geom_test::quality_ledger::{LEDGER_PATH, Ledger};
use geom_test::shared;
use geom_test::test_scope;
use regex_lite::Regex;

/// The geometry kernel's mutation and coverage gates (section 28.8 of the kernel architecture).
#[derive(Parser)]
#[command(name = "kernel-quality", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Where the repository and its ledger are.
#[derive(Args)]
struct Repo {
    /// The repository root.
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    /// The quality ledger (default: the repository's).
    #[arg(long)]
    ledger: Option<PathBuf>,
}

/// Which kernel crates a coverage run covers.
#[derive(Args)]
#[group(required = true, multiple = false)]
struct Scope {
    /// Every kernel crate.
    #[arg(long)]
    all: bool,
    /// The crates a pull request's changed files reach: one path per line, relative to the
    /// repository root.
    #[arg(long, value_name = "FILES.txt")]
    changed: Option<PathBuf>,
    /// These kernel crates, for a local run.
    #[arg(long, value_delimiter = ',', value_name = "A,B")]
    crates: Vec<String>,
}

/// What changed since the last clean head of a pull request.
#[derive(Args)]
struct SinceArgs {
    /// The last clean head.
    #[arg(long, value_name = "COMMIT")]
    since: String,
    /// The files that differ between that head and this pull request's head.
    #[arg(long, value_name = "FILES.txt")]
    head_changes: PathBuf,
    /// Of those, the files the pull request's head added.
    #[arg(long, value_name = "FILES.txt")]
    added: PathBuf,
    /// The files the base's commits since that head touch.
    #[arg(long, value_name = "FILES.txt")]
    base_changes: PathBuf,
    /// The repository root, checked out at the change.
    #[arg(long, default_value = ".")]
    repo: PathBuf,
}

/// Which mutation gate is judging.
#[derive(Clone, Copy, ValueEnum)]
enum ModeArg {
    /// A pull request's diff.
    Diff,
    /// A full run over every mutant, or every mutant of some files.
    Full,
}

/// Which tests a Miri measuring run measures.
#[derive(Clone, Copy, ValueEnum)]
enum Which {
    /// The tests with no row in Miri's table.
    New,
    /// Every test, to measure the whole table again.
    All,
}

#[derive(Subcommand)]
enum Command {
    /// Prints the number of shards a mutation run needs, and each shard job's list, as JSON.
    MutantsPlan {
        /// The output of `cargo mutants --list --json` for the run (an empty file: no mutants).
        #[arg(long, value_name = "LIST.json")]
        list: PathBuf,
        /// A full run narrowed to the files this regular expression matches: the list is
        /// `cargo mutants --list --json --re <REGEX>`.
        #[arg(long, value_name = "REGEX", requires = "all")]
        files: Option<String>,
        /// With `--files`: `cargo mutants --list --json` without `--re`, every mutant.
        #[arg(long, value_name = "ALL.json", requires = "files")]
        all: Option<PathBuf>,
    },
    /// Judges cargo-mutants output directories against the ledger.
    Mutants {
        /// The gate.
        #[arg(long, value_enum)]
        mode: ModeArg,
        /// The output of `cargo mutants --list --json` with the run's filters: every mutant the
        /// shards together must have tested (an empty file: no mutants).
        #[arg(long, value_name = "LIST.json")]
        expected: PathBuf,
        /// The base branch's ledger, in a pull request: the `known` entries it may use.
        #[arg(long, value_name = "FILE")]
        base_ledger: Option<PathBuf>,
        /// A full run narrowed to the files this regular expression matches, as its plan was: only
        /// those files' ledger entries are counted.
        #[arg(long, value_name = "REGEX")]
        files: Option<String>,
        #[command(flatten)]
        repo: Repo,
        /// The output directories, one per shard (`mutants.out`).
        #[arg(value_name = "OUT")]
        outs: Vec<PathBuf>,
    },
    /// Judges each measured crate's coverage against 28.8's bars, and every coverage(off) mark.
    Coverage {
        #[command(flatten)]
        scope: Scope,
        /// The folder holding `<crate>.lcov` and `<crate>.log` for each crate.
        #[arg(long, value_name = "DIR")]
        lcov_dir: PathBuf,
        #[command(flatten)]
        repo: Repo,
    },
    /// Prints the parts of a coverage run as a GitHub Actions matrix: `{"include": [{"part": k,
    /// "of": n, "crates": "a b"}, ...]}`, the crates `affected` names balanced by their measured
    /// times.
    CoverageParts {
        #[command(flatten)]
        scope: Scope,
        /// The repository root.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Prints the kernel crates a coverage run measures, one per line.
    Affected {
        #[command(flatten)]
        scope: Scope,
        /// The repository root.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Prints the `-p` arguments of every kernel crate, for the Miri run.
    MiriPackages {
        /// Only the kernel crates whose tests these changed files reach (`test-scope`'s rule):
        /// one path per line from the repository root.
        #[arg(long, value_name = "FILES.txt")]
        changed: Option<PathBuf>,
        /// The repository root.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Plans the Miri gate (`geom_test::miri::gate_plan`): writes partition k's nextest filterset
    /// to `OUT/part-<k>.txt` and geom-pool's to `OUT/pool.txt`, and prints `{"count": n, "parts":
    /// [1, ..., n], "tests": t, "pool": p}`.
    MiriPlan {
        /// The run's tests, `cargo miri nextest list --message-format json`: every one must have
        /// a row in the table.
        #[arg(long, value_name = "LIST.json")]
        list: PathBuf,
        /// Where to write the filtersets.
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
    },
    /// Writes Miri's table (geom-test's `src/miri-times.tsv`) from a measuring run's nextest
    /// reports (`junit.xml`): each test they name gets the state they measured; a failure
    /// writes nothing.
    MiriTimes {
        /// The reports (`target/nextest/<profile>/junit.xml`).
        #[arg(long, value_name = "JUNIT.xml", required = true)]
        report: Vec<PathBuf>,
        /// CI seconds for one of the run's seconds, as a decimal: 1 for a CI run's own reports.
        #[arg(long)]
        scale: String,
        /// The run's stop for one test, in its own seconds (its profile's slow timeout).
        #[arg(long)]
        stop: u64,
        /// A comment line to add to the table's header, saying where the run was.
        #[arg(long)]
        note: Option<String>,
        /// The kernel's whole nextest listing (`--message-format json`): a row naming a test it
        /// lacks, renamed or removed, is dropped.
        #[arg(long, value_name = "LIST.json")]
        list: Option<PathBuf>,
        /// Reads the reports and prints what the table would hold, writing nothing: a test that
        /// failed under Miri still fails it, named.
        #[arg(long)]
        dry_run: bool,
        /// The repository root.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Writes the list of the tests a gate job may see stopped at the budget (geom-test's
    /// `src/miri-may-stop.tsv`, 32.68) from Miri's table as it is in the repository.
    MiriMayStop {
        /// Prints the list, writing nothing.
        #[arg(long)]
        dry_run: bool,
        /// The repository root.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Reads a Miri gate job's nextest reports and names, as Markdown for the job's summary, every
    /// test nextest stopped at `--stop` with no verdict: fails (exit 1) when one of them may not
    /// be stopped (32.68). At the budget only the list `src/miri-may-stop.tsv` may be; at a
    /// shorter stop (the `miri-smoke` profile's) every test whose row a slow runner can take past
    /// it.
    MiriStopped {
        /// The reports (`target/nextest/<profile>/junit.xml`).
        #[arg(long, value_name = "JUNIT.xml", required = true)]
        report: Vec<PathBuf>,
        /// The job's stop for one test, in seconds (its profile's slow timeout).
        #[arg(long)]
        stop: u64,
        /// What the summary calls the job.
        #[arg(long)]
        title: String,
    },
    /// Plans a measuring run of Miri's table on GitHub's runners: picks the tests, packs them into
    /// parts that each end within the stop, writes each part's nextest filterset to
    /// `OUT/part-<k>.txt` and prints the parts as JSON for the workflow's matrices.
    MiriMeasure {
        /// Which tests: those with no row in the table, or all of them.
        #[arg(long, value_enum)]
        which: Which,
        /// The run's whole listing, `cargo miri nextest list --message-format json`.
        #[arg(long, value_name = "LIST.json")]
        list: PathBuf,
        /// A narrower listing of the same run (a filterset's), to measure only its tests.
        #[arg(long, value_name = "LIST.json")]
        only: Option<PathBuf>,
        /// Where to write the parts' filtersets.
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
    },
    /// Prints the regular expression of the files a push to main's full mutation run mutates
    /// (empty: none), for `cargo mutants --re` and the judge's `--files`.
    MutantsFiles {
        /// The files the push touches, one path per line from the repository root.
        #[arg(long, value_name = "FILES.txt")]
        changed: PathBuf,
        /// The push's unified diff of its Rust files.
        #[arg(long, value_name = "DIFF")]
        diff: PathBuf,
        #[command(flatten)]
        repo: Repo,
    },
    /// Prints which crates' tests a change can reach, and why.
    TestScope {
        /// Every crate: a push to main, a release tag.
        #[arg(long, conflicts_with = "changed", required_unless_present = "changed")]
        all: bool,
        /// The files the change touches, one path per line from the repository root.
        #[arg(long, value_name = "FILES.txt")]
        changed: Option<PathBuf>,
        /// The change's base commit: `MAP.json` and `quality-ledger.json` are then read by their
        /// changed entries (`geom_test::shared`); without it each counts as a file in no crate.
        #[arg(long, value_name = "COMMIT", requires = "changed")]
        base: Option<String>,
        /// Whether the change reaches what this crate's tests and benchmarks build, as the output
        /// `reached`, instead of the crates whose tests run.
        #[arg(long, value_name = "CRATE")]
        build_of: Option<String>,
        /// Also the output `parts`: the selected crates but `--apart`'s shared out among this many
        /// parts of the release suite by their measured seconds
        /// (`geom_test::test_scope::Selection::parts`).
        #[arg(long, value_name = "N", conflicts_with = "build_of", value_parser = clap::value_parser!(u16).range(1..))]
        parts: Option<u16>,
        /// A crate the workflow runs in parts of its own, left out of `--parts`'s shares.
        #[arg(long, value_name = "CRATE", requires = "parts")]
        apart: Vec<String>,
        /// Appends the step outputs to this file (GitHub's `$GITHUB_OUTPUT`) instead of printing
        /// them.
        #[arg(long, value_name = "FILE")]
        github_output: Option<PathBuf>,
        /// The repository root.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Prints the change's Rust files a later push mutates again, one per line, after the last
    /// head whose mutants were judged clean (`geom_test::increment::mutants`); why goes to
    /// standard error.
    MutantsIncrement {
        #[command(flatten)]
        since: SinceArgs,
        /// The change's files, base against head, one path per line from the repository root.
        #[arg(long, value_name = "FILES.txt")]
        changed: PathBuf,
    },
    /// Prints the files whose crates a later push's coverage run measures, one per line, after the
    /// last head whose coverage passed (`geom_test::increment::coverage`).
    CoverageIncrement {
        #[command(flatten)]
        since: SinceArgs,
    },
    /// Prints the `part:` lines of `kernel-dev.yml`'s `wasm` job's matrix: the development crates'
    /// test units shared out among this many parts by their measured seconds
    /// (`geom_test::test_scope::wasm_dev_matrix`).
    WasmDevParts {
        /// How many parts.
        #[arg(long, value_name = "N", value_parser = clap::value_parser!(u16).range(1..))]
        parts: u16,
        /// The repository root.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Prints which fuzz targets and Kani proofs a change can affect, and why.
    FuzzScope {
        /// Every target and proof: a push to main, a release tag.
        #[arg(long, conflicts_with = "changed", required_unless_present = "changed")]
        all: bool,
        /// The files the change touches, one path per line from the repository root.
        #[arg(long, value_name = "FILES.txt")]
        changed: Option<PathBuf>,
        /// The change's base commit: `MAP.json` and `quality-ledger.json` are then read by their
        /// changed entries (`geom_test::shared`); without it each is placed as before.
        #[arg(long, value_name = "COMMIT", requires = "changed")]
        base: Option<String>,
        /// Appends the step outputs to this file (GitHub's `$GITHUB_OUTPUT`) instead of printing
        /// them.
        #[arg(long, value_name = "FILE")]
        github_output: Option<PathBuf>,
        /// The repository root.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

/// A file's lines: one path each.
fn read_lines(file: &Path) -> Result<Vec<String>, String> {
    std::fs::read_to_string(file)
        .map(|text| text.lines().map(str::to_owned).collect())
        .map_err(|e| format!("{}: {e}", file.display()))
}

/// The tests a `cargo nextest list --message-format json` file lists to run.
fn read_listing(file: &Path) -> Result<Vec<(String, String)>, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let listing: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    geom_test::miri::listed_tests(&listing).map_err(|e| format!("{}: {e}", file.display()))
}

fn canonical(repo: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(repo).map_err(|e| format!("{}: {e}", repo.display()))
}

fn read_ledger(repo: &Path, path: Option<&Path>) -> Result<Ledger, String> {
    let path = path.map_or_else(|| repo.join(LEDGER_PATH), Path::to_path_buf);
    Ledger::read(&path).map_err(|e| e.to_string())
}

fn metadata(repo: &Path) -> Result<serde_json::Value, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    geom_test::deps::cargo_metadata(&cargo, &repo.join("Cargo.toml")).map_err(|e| e.to_string())
}

/// The crates a scope names.
fn affected(scope: &Scope, repo: &Path) -> Result<Vec<String>, String> {
    let all = geom_test::foundation::kernel_crates(repo).map_err(|e| e.to_string())?;
    if scope.all {
        return Ok(all);
    }
    if let Some(file) = &scope.changed {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let changed: Vec<String> = text.lines().map(str::to_owned).collect();
        return coverage::affected_crates(repo, &changed, &metadata(repo)?)
            .map_err(|e| e.to_string());
    }
    match scope.crates.iter().find(|c| !all.contains(c)) {
        Some(unknown) => Err(format!("{unknown} is not a kernel crate")),
        None => Ok(scope.crates.clone()),
    }
}

/// The crates of a scope that have code to measure ([`coverage::has_code`]); the others are named
/// on standard error, so the log shows what was skipped and why.
fn measured(scope: &Scope, repo: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for krate in affected(scope, repo)? {
        if coverage::has_code(repo, &krate).map_err(|e| e.to_string())? {
            out.push(krate);
        } else {
            eprintln!("{krate}: no function outside test code, so no line to measure");
        }
    }
    Ok(out)
}

fn judge_coverage(scope: &Scope, dir: &Path, repo: &Repo) -> Result<bool, String> {
    let root = canonical(&repo.repo)?;
    let ledger = read_ledger(&root, repo.ledger.as_deref())?;
    let crates = measured(scope, &root)?;
    let mut reports: Vec<CrateReport> = Vec::new();
    // Every crate this judge lists itself must have its files here, whichever part measured it.
    let mut failures = coverage::missing_outputs(&crates, dir);
    for krate in &crates {
        let log = dir.join(format!("{krate}.log"));
        match std::fs::read_to_string(&log) {
            Ok(text) => failures.extend(coverage::check_log(krate, &text)),
            Err(e) if log.is_file() => failures.push(format!("{}: {e}", log.display())),
            Err(_) => {}
        }
        let file = dir.join(format!("{krate}.lcov"));
        match std::fs::read_to_string(&file) {
            Ok(text) => {
                reports.push(coverage::measure(&root, krate, &text).map_err(|e| e.to_string())?);
            }
            Err(e) if file.is_file() => failures.push(format!("{}: {e}", file.display())),
            Err(_) => {}
        }
    }
    failures.extend(
        coverage::excuse_awaiting(&root, &mut reports, &ledger).map_err(|e| e.to_string())?,
    );
    failures.extend(coverage::judge(&reports));
    failures.extend(coverage::check_marks(&root, &ledger).map_err(|e| e.to_string())?);
    print!("{}", coverage::table(&reports));
    for note in coverage::awaiting_notes(&reports) {
        println!("awaiting proof: {note}");
    }
    if failures.is_empty() {
        println!(
            "PASS: {} crate(s) meet section 28.8's coverage bars",
            crates.len()
        );
    } else {
        println!("FAIL: {} problem(s)", failures.len());
        for f in &failures {
            println!("  {f}");
        }
    }
    Ok(failures.is_empty())
}

/// What the change's shared files reach (`geom_test::shared`): each compared with its text at
/// `base` (`git show <base>:<file>` in `repo`; a file missing there, or a base git cannot read,
/// reaches everything). None without a base or a change list, so each shared file is then placed
/// as before.
fn shared_changes(
    repo: &Path,
    files: Option<&[String]>,
    base: Option<&str>,
    folders: &BTreeMap<String, String>,
) -> shared::Changes {
    let (Some(files), Some(base)) = (files, base) else {
        return shared::Changes::new();
    };
    let changes = shared::changes(files, &|file: &str| show(repo, base, file), repo, folders);
    for (file, reach) in &changes {
        match reach {
            shared::Reach::Everything(why) => {
                println!("{file} changed and {why}: it reaches everything.");
            }
            shared::Reach::Crates(reached) => {
                let list: Vec<&str> = reached.keys().map(String::as_str).collect();
                let list = if list.is_empty() {
                    "no crate".to_owned()
                } else {
                    list.join(", ")
                };
                println!("{file} changed: its changed entries name {list}.");
            }
        }
    }
    changes
}

/// The text of `file` at `commit` (`git show`), or `None` when git cannot give it.
fn show(repo: &Path, commit: &str, file: &str) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("show")
        .arg(format!("{commit}:{file}"))
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// What changed since the last clean head, as `geom_test::increment` reads it.
fn since(args: &SinceArgs, root: &Path) -> Result<geom_test::increment::Since, String> {
    Ok(geom_test::increment::Since {
        head: read_lines(&args.head_changes)?,
        added: read_lines(&args.added)?.into_iter().collect(),
        base: read_lines(&args.base_changes)?,
        ledger: (
            show(root, &args.since, LEDGER_PATH),
            std::fs::read_to_string(root.join(LEDGER_PATH)).ok(),
        ),
    })
}

/// `fuzz-scope`: the decision for every job, printed, and its step outputs written.
fn fuzz_scope(
    all: bool,
    changed: Option<&Path>,
    base: Option<&str>,
    github_output: Option<&Path>,
    repo: &Path,
) -> Result<bool, String> {
    let root = canonical(repo)?;
    let scope = fuzz_scope::Scope::read(&metadata(&root)?)?;
    let files: Option<Vec<String>> = match changed {
        Some(file) if !all => {
            let text =
                std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
            Some(text.lines().map(str::to_owned).collect())
        }
        _ => None,
    };
    let shared = shared_changes(&root, files.as_deref(), base, scope.folders());
    let selection = scope.select_with(files.as_deref(), &shared);
    print!("{}", selection.report());
    let mut lines = String::new();
    for (key, value) in selection.outputs() {
        lines.push_str(&key);
        lines.push('=');
        lines.push_str(&value);
        lines.push('\n');
    }
    if let Some(path) = github_output {
        use std::io::Write as _;
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .and_then(|mut f| f.write_all(lines.as_bytes()))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    } else {
        println!();
        print!("{lines}");
    }
    Ok(true)
}

/// `test-scope`: the decision for every crate (or for one crate's build), printed, and its step
/// outputs written.
fn test_scope(
    changed: Option<&Path>,
    base: Option<&str>,
    build_of: Option<&str>,
    parts: Option<(usize, &[String])>,
    github_output: Option<&Path>,
    repo: &Path,
) -> Result<bool, String> {
    let root = canonical(repo)?;
    let scope = test_scope::Scope::read(&metadata(&root)?)?;
    let files: Option<Vec<String>> = changed
        .map(|file| {
            std::fs::read_to_string(file)
                .map(|text| text.lines().map(str::to_owned).collect())
                .map_err(|e| format!("{}: {e}", file.display()))
        })
        .transpose()?;
    let shared = shared_changes(&root, files.as_deref(), base, scope.folders());
    let outputs = if let Some(krate) = build_of {
        if !scope.folders().contains_key(krate) {
            return Err(format!("{krate} is not a workspace crate"));
        }
        let reached = scope.build_reached_with(files.as_deref(), &shared, krate);
        if let Some(why) = &reached {
            println!("The change reaches what {krate} builds: {why}.");
        } else {
            println!("The change reaches nothing {krate} builds.");
        }
        vec![("reached".to_owned(), reached.is_some().to_string())]
    } else {
        let selection = scope.select_with(files.as_deref(), &shared);
        print!("{}", selection.report());
        let mut outputs = selection.outputs();
        if let Some((n, apart)) = parts {
            if let Some(c) = apart.iter().find(|c| !scope.folders().contains_key(*c)) {
                return Err(format!("--apart {c}: not a workspace crate"));
            }
            let times = geom_test::coverage::parse_times(test_scope::SUITE_TIMES)
                .map_err(|e| format!("suite-times.tsv: {e}"))?;
            let output = selection.parts_output(n, apart, &times, scope.folders());
            println!("The release suite's parts: {}", output.1);
            outputs.push(output);
        }
        outputs
    };
    let mut lines = String::new();
    for (key, value) in outputs {
        lines.push_str(&key);
        lines.push('=');
        lines.push_str(&value);
        lines.push('\n');
    }
    if let Some(path) = github_output {
        use std::io::Write as _;
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .and_then(|mut f| f.write_all(lines.as_bytes()))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    } else {
        println!();
        print!("{lines}");
    }
    Ok(true)
}

/// A `--files` regular expression, in the syntax cargo-mutants' `--re` takes.
fn regex(files: &str) -> Result<Regex, String> {
    Regex::new(files).map_err(|e| format!("--files {files:?}: {e}"))
}

/// `mutants`: the runs judged against the ledger, and the verdict printed.
fn judge_mutants(
    mode: ModeArg,
    expected: &Path,
    base_ledger: Option<&Path>,
    files: Option<&str>,
    repo: &Repo,
    outs: &[PathBuf],
) -> Result<bool, String> {
    let root = canonical(&repo.repo)?;
    let ledger = read_ledger(&root, repo.ledger.as_deref())?;
    let base = base_ledger
        .map(|p| Ledger::read(p).map_err(|e| format!("base ledger: {e}")))
        .transpose()?;
    let expected = mutants::read_mutant_list(expected).map_err(|e| e.to_string())?;
    let mut runs = Vec::new();
    for dir in outs {
        runs.push(mutants::read_run(dir).map_err(|e| e.to_string())?);
    }
    let mode = match mode {
        ModeArg::Diff => Mode::Diff,
        ModeArg::Full => Mode::Full,
    };
    let files = files.map(regex).transpose()?;
    if let Some(re) = &files {
        if mode == Mode::Diff {
            return Err("--files narrows a full run; a diff is narrowed already".to_owned());
        }
        let outside = ledger
            .survivors
            .iter()
            .filter(|s| !re.is_match(&s.file))
            .count();
        println!(
            "narrowed to the files matching {:?}: {} mutant(s); {outside} ledger entr{} of \
             other files not counted",
            re.as_str(),
            expected.len(),
            if outside == 1 { "y" } else { "ies" }
        );
    }
    let verdict = mutants::judge(
        &runs,
        &ledger,
        &root,
        mode,
        Some(expected.as_slice()),
        base.as_ref(),
        files.as_ref(),
    );
    print!("{}", mutants::report(&verdict));
    Ok(verdict.passes())
}

/// `mutants-increment`: the change's Rust files a later push mutates again, printed one per
/// line, with why on standard error.
fn mutants_increment(args: &SinceArgs, changed: &Path) -> Result<bool, String> {
    let root = canonical(&args.repo)?;
    let scope = test_scope::Scope::read(&metadata(&root)?)?;
    let since = since(args, &root)?;
    let shared = shared_changes(&root, Some(&since.head), Some(&args.since), scope.folders());
    let plan = geom_test::increment::mutants(&scope, &read_lines(changed)?, &since, &shared);
    for (file, why) in &plan {
        eprintln!("mutates {file} again: {why}");
        println!("{file}");
    }
    if plan.is_empty() {
        eprintln!(
            "nothing changed since {} that any mutant's tests build or read",
            args.since
        );
    }
    Ok(true)
}

/// Runs the command: `Ok(true)` when the gate passes, `Ok(false)` when it fails.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per subcommand, each a call into the library"
)]
fn run(cli: Cli) -> Result<bool, String> {
    match cli.command {
        Command::MutantsPlan { list, files, all } => {
            let mutants = mutants::read_mutant_list(&list).map_err(|e| e.to_string())?;
            if let (Some(files), Some(all)) = (files, all) {
                let all = mutants::read_mutant_list(&all).map_err(|e| e.to_string())?;
                let strays = mutants::check_narrowed(&mutants, &all, &regex(&files)?)?;
                if strays > 0 {
                    eprintln!(
                        "{strays} mutant(s) of other files come along: cargo-mutants lists every \
                         {} mutant whatever --re says",
                        mutants::UNFILTERED_GENRE
                    );
                }
            }
            let count = mutants::shard_count(&mutants)?;
            println!("{}", mutants::shard_matrix(count));
            Ok(true)
        }
        Command::Mutants {
            mode,
            expected,
            base_ledger,
            files,
            repo,
            outs,
        } => judge_mutants(
            mode,
            &expected,
            base_ledger.as_deref(),
            files.as_deref(),
            &repo,
            &outs,
        ),
        Command::Coverage {
            scope,
            lcov_dir,
            repo,
        } => judge_coverage(&scope, &lcov_dir, &repo),
        Command::CoverageParts { scope, repo } => {
            let crates = measured(&scope, &canonical(&repo)?)?;
            let times = coverage::parse_times(coverage::TIMES)?;
            let parts = coverage::plan_parts(&crates, &times);
            let problems = coverage::plan_problems(&crates, &parts);
            if !problems.is_empty() {
                return Err(format!(
                    "the coverage plan is wrong: {}",
                    problems.join("; ")
                ));
            }
            let n = parts.len();
            let include: Vec<serde_json::Value> = parts
                .iter()
                .enumerate()
                .map(|(k, p)| {
                    serde_json::json!({"part": k.saturating_add(1), "of": n, "crates": p.join(" ")})
                })
                .collect();
            println!("{}", serde_json::json!({ "include": include }));
            Ok(true)
        }
        Command::Affected { scope, repo } => {
            for krate in measured(&scope, &canonical(&repo)?)? {
                println!("{krate}");
            }
            Ok(true)
        }
        Command::MiriPackages { changed, repo } => {
            let metadata = metadata(&canonical(&repo)?)?;
            let mut packages = geom_test::miri::packages(&metadata)?;
            if let Some(file) = changed {
                let files = read_lines(&file)?;
                let selection = test_scope::Scope::read(&metadata)?.select(Some(&files));
                let running = selection.running();
                packages.retain(|p| running.contains(&p.as_str()));
            }
            println!("{}", geom_test::miri::cargo_args(&packages));
            Ok(true)
        }
        Command::MiriPlan { list, out } => {
            let rows = geom_test::miri::parse_table(geom_test::miri::TIMES)?;
            let missing = geom_test::miri::unlisted(&rows, &read_listing(&list)?);
            if !missing.is_empty() {
                eprintln!(
                    "{} tests have no row in Miri's table; measure them under Miri and write \
                     the table with `kernel-quality miri-times`:",
                    missing.len()
                );
                for m in &missing {
                    eprintln!("{m}");
                }
                return Ok(false);
            }
            let gate = geom_test::miri::gate_plan(&rows)?;
            std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            let count = gate.parts.len();
            let mut tests = 0_usize;
            for k in 1..=count {
                let file = out.join(format!("part-{k}.txt"));
                std::fs::write(&file, gate.parts.filterset(k)?)
                    .map_err(|e| format!("{}: {e}", file.display()))?;
                tests = tests.saturating_add(gate.parts.tests_of(k).len());
                eprintln!(
                    "partition {k}: {} tests, at most {} s",
                    gate.parts.tests_of(k).len(),
                    gate.parts.part_seconds(k)
                );
            }
            let file = out.join("pool.txt");
            std::fs::write(&file, gate.pool.filterset(1)?)
                .map_err(|e| format!("{}: {e}", file.display()))?;
            let pool = gate.pool.tests_of(1).len();
            eprintln!(
                "{}: {pool} tests, at most {} s",
                geom_test::miri::TREE_BORROWS_CRATE,
                gate.pool.part_seconds(1)
            );
            let parts: Vec<usize> = (1..=count).collect();
            println!(
                "{}",
                serde_json::json!({"count": count, "parts": parts, "tests": tests, "pool": pool})
            );
            Ok(true)
        }
        Command::MiriTimes {
            report,
            scale,
            stop,
            note,
            list,
            dry_run,
            repo,
        } => {
            let path = canonical(&repo)?.join(geom_test::miri::TABLE_PATH);
            let old =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let listed = match &list {
                Some(file) => Some(read_listing(file)?),
                None => None,
            };
            let mut reported = Vec::new();
            for file in &report {
                let xml = std::fs::read_to_string(file)
                    .map_err(|e| format!("{}: {e}", file.display()))?;
                reported.extend(
                    geom_test::miri::read_junit(&xml)
                        .map_err(|e| format!("{}: {e}", file.display()))?,
                );
            }
            let scale = geom_test::miri::parse_scale(&scale)?;
            let text = geom_test::miri::write_table(
                &old,
                &reported,
                scale,
                stop,
                listed.as_deref(),
                note.as_deref(),
            )?;
            let of_report = geom_test::miri::measured(&[], &reported, scale, stop)?;
            let report_finish = geom_test::miri::finishing(&of_report).len();
            println!(
                "the reports measured {} tests: {report_finish} finish under Miri, {} native-only",
                of_report.len(),
                of_report.len().saturating_sub(report_finish)
            );
            let rows = geom_test::miri::parse_table(&text)?;
            let may_path = canonical(&repo)?.join(geom_test::miri::MAY_STOP_PATH);
            let may = geom_test::miri::render_may_stop(&rows);
            if dry_run {
                println!(
                    "a dry run: {} and {} are not written",
                    path.display(),
                    may_path.display()
                );
            } else {
                std::fs::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
                std::fs::write(&may_path, &may)
                    .map_err(|e| format!("{}: {e}", may_path.display()))?;
            }
            println!(
                "{} tests may be stopped at the {} s budget with no verdict (over {} s), at most {} \
                 allowed",
                geom_test::miri::parse_may_stop(&may)?.len(),
                geom_test::miri::BUDGET,
                geom_test::miri::SURE_WITHIN,
                geom_test::miri::MAY_STOP_MOST
            );
            let finish = geom_test::miri::finishing(&rows).len();
            println!(
                "{} rows from {} reported tests: {finish} finish under Miri, {} native-only",
                rows.len(),
                reported.len(),
                rows.len().saturating_sub(finish)
            );
            match geom_test::miri::gate_plan(&rows) {
                Ok(gate) => println!(
                    "the gate's plan, its tests at {}/{} of their seconds and never over the stop: \
                     {} partitions within {} s, the slowest {} s, and {}'s {} tests in {} s",
                    geom_test::miri::SLOWER.0,
                    geom_test::miri::SLOWER.1,
                    gate.parts.len(),
                    geom_test::miri::PART_SECONDS,
                    gate.parts.longest(),
                    geom_test::miri::TREE_BORROWS_CRATE,
                    gate.pool.tests_of(1).len(),
                    gate.pool.longest()
                ),
                Err(why) => println!("the gate cannot be planned: {why}"),
            }
            Ok(true)
        }
        Command::MiriMayStop { dry_run, repo } => {
            let root = canonical(&repo)?;
            let table = root.join(geom_test::miri::TABLE_PATH);
            let text =
                std::fs::read_to_string(&table).map_err(|e| format!("{}: {e}", table.display()))?;
            let may = geom_test::miri::render_may_stop(&geom_test::miri::parse_table(&text)?);
            if dry_run {
                print!("{may}");
            } else {
                let path = root.join(geom_test::miri::MAY_STOP_PATH);
                std::fs::write(&path, &may).map_err(|e| format!("{}: {e}", path.display()))?;
            }
            Ok(true)
        }
        Command::MiriStopped {
            report,
            stop,
            title,
        } => {
            let rows = geom_test::miri::parse_table(geom_test::miri::TIMES)?;
            let budget = geom_test::miri::BUDGET;
            let may = geom_test::miri::allowed_stops(&rows, geom_test::miri::MAY_STOP, stop)?;
            let mut reported = Vec::new();
            for file in &report {
                let xml = std::fs::read_to_string(file)
                    .map_err(|e| format!("{}: {e}", file.display()))?;
                reported.extend(
                    geom_test::miri::read_junit(&xml)
                        .map_err(|e| format!("{}: {e}", file.display()))?,
                );
            }
            let stops = geom_test::miri::stops(&reported, stop, &may);
            let seconds = |name: &str| {
                rows.iter()
                    .find(|r| format!("{} {}", r.binary, r.test) == name)
                    .map_or_else(
                        || "no row".to_owned(),
                        |r| match r.state {
                            geom_test::miri::State::Finishes(s) => format!("its row {s} s"),
                            geom_test::miri::State::NativeOnly(_) => "native-only".to_owned(),
                        },
                    )
            };
            let tests = |n: usize| {
                if n == 1 {
                    "1 test".to_owned()
                } else {
                    format!("{n} tests")
                }
            };
            let mut out = vec![
                format!("### {title}: tests stopped with no verdict"),
                String::new(),
            ];
            if stops.allowed.is_empty() && stops.refused.is_empty() {
                out.push(format!(
                    "No test stopped at {stop} s: each of the {} tests reported ran to its verdict.",
                    stops.reported
                ));
            }
            let which = if stop == budget {
                "on the list of tests that may stop (geom-test's src/miri-may-stop.tsv)".to_owned()
            } else {
                format!("each a test this short run's stop may end (its row over {stop} s / 1.6)")
            };
            if !stops.allowed.is_empty() {
                out.push(format!(
                    "**{} stopped at {stop} s with no verdict**, {which}: not passes; Miri did \
                     not see them finish.",
                    tests(stops.allowed.len())
                ));
                out.push(String::new());
                for name in &stops.allowed {
                    out.push(format!("- `{name}` ({})", seconds(name)));
                }
                out.push(String::new());
            }
            if !stops.refused.is_empty() {
                out.push(format!(
                    "**{} stopped at {stop} s with no verdict that may not stop: this job \
                     fails.** Each is to be resized under `cfg(miri)` so Miri finishes it, and \
                     measured again.",
                    tests(stops.refused.len())
                ));
                out.push(String::new());
                for name in &stops.refused {
                    out.push(format!("- `{name}` ({})", seconds(name)));
                    eprintln!(
                        "::error::{name} was stopped at {stop} s with no verdict, and is not a test \
                         that may stop"
                    );
                }
            }
            println!("{}", out.join("\n"));
            Ok(stops.refused.is_empty())
        }
        Command::MiriMeasure {
            which,
            list,
            only,
            out,
        } => {
            let whole = read_listing(&list)?;
            let picked = match &only {
                Some(file) => {
                    let narrow = read_listing(file)?;
                    let all: std::collections::BTreeSet<&(String, String)> = whole.iter().collect();
                    if let Some((b, t)) = narrow.iter().find(|n| !all.contains(n)) {
                        return Err(format!(
                            "{}: {b} {t} is not in the run's whole listing",
                            file.display()
                        ));
                    }
                    narrow
                }
                None => whole,
            };
            let rows = geom_test::miri::parse_table(geom_test::miri::TIMES)?;
            let which = match which {
                Which::New => geom_test::miri::Measuring::New,
                Which::All => geom_test::miri::Measuring::All,
            };
            let tests = geom_test::miri::to_measure(&rows, &picked, which);
            let plan = geom_test::miri::measure_plan(&tests)?;
            let count = if tests.is_empty() { 0 } else { plan.len() };
            std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            for k in 1..=count {
                let file = out.join(format!("part-{k}.txt"));
                std::fs::write(&file, plan.filterset(k)?)
                    .map_err(|e| format!("{}: {e}", file.display()))?;
            }
            eprintln!(
                "{} of the run's {} tests to measure, in {count} parts of at most {} s at {} at once",
                tests.len(),
                picked.len(),
                geom_test::miri::BUDGET,
                geom_test::miri::AT_ONCE
            );
            println!("{}", geom_test::miri::measure_matrix(tests.len(), count));
            Ok(true)
        }
        Command::MutantsFiles {
            changed,
            diff,
            repo,
        } => {
            let root = canonical(&repo.repo)?;
            let ledger = read_ledger(&root, repo.ledger.as_deref())?;
            let kernel = geom_test::foundation::kernel_crates(&root).map_err(|e| e.to_string())?;
            let diff =
                std::fs::read_to_string(&diff).map_err(|e| format!("{}: {e}", diff.display()))?;
            let files = mutants::push_files(&read_lines(&changed)?, &diff, &ledger, &kernel);
            for f in &files {
                eprintln!("mutates {f}");
            }
            println!("{}", mutants::files_pattern(&files));
            Ok(true)
        }
        Command::TestScope {
            all,
            changed,
            base,
            build_of,
            parts,
            apart,
            github_output,
            repo,
        } => test_scope(
            changed.as_deref().filter(|_| !all),
            base.as_deref(),
            build_of.as_deref(),
            parts.map(|n| (usize::from(n), apart.as_slice())),
            github_output.as_deref(),
            &repo,
        ),
        Command::MutantsIncrement { since, changed } => mutants_increment(&since, &changed),
        Command::CoverageIncrement { since: args } => {
            let root = canonical(&args.repo)?;
            for file in geom_test::increment::coverage(&since(&args, &root)?) {
                println!("{file}");
            }
            Ok(true)
        }
        Command::WasmDevParts { parts, repo } => {
            let units = test_scope::wasm_dev_units(
                &metadata(&canonical(&repo)?)?,
                &test_scope::WASM_DEV_CRATES,
            );
            let times = coverage::parse_times(test_scope::WASM_DEV_TIMES)
                .map_err(|e| format!("wasm-dev-times.tsv: {e}"))?;
            print!(
                "{}",
                test_scope::wasm_dev_matrix(&units, &times, usize::from(parts))
            );
            Ok(true)
        }
        Command::FuzzScope {
            all,
            changed,
            base,
            github_output,
            repo,
        } => fuzz_scope(
            all,
            changed.as_deref(),
            base.as_deref(),
            github_output.as_deref(),
            &repo,
        ),
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(why) => {
            eprintln!("kernel-quality: {why}");
            ExitCode::from(2)
        }
    }
}
