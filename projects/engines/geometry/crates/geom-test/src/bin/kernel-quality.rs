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
//! - `miri-filter` prints the nextest filterset of one Miri partition, the tests the plan
//!   balanced by their recorded times gives it ([`geom_test::miri::plan`]).
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
//!   outputs `kernel.yml`'s suites read ([`geom_test::test_scope`]); with `--build-of <crate>` it
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
    /// Prints the nextest filterset of one Miri partition: its tests in the plan the recorded
    /// times balance (`geom_test::miri::plan`).
    MiriFilter {
        /// The partition, from 1.
        #[arg(long)]
        partition: usize,
        /// How many partitions the run has.
        #[arg(long)]
        of: usize,
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
        selection.outputs()
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
                return Err(format!("the coverage plan is wrong: {}", problems.join("; ")));
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
        Command::MiriFilter { partition, of } => {
            let times = geom_test::miri::parse_times(geom_test::miri::TIMES)?;
            println!(
                "{}",
                geom_test::miri::plan(&times, of)?.filterset(partition)?
            );
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
            github_output,
            repo,
        } => test_scope(
            changed.as_deref().filter(|_| !all),
            base.as_deref(),
            build_of.as_deref(),
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
