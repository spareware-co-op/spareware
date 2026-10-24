//! `kernel-quality`: the CI tool behind the geometry kernel's mutation and coverage gates and its
//! weekly Miri run (section 28.8; `.github/workflows/kernel-quality.yml`).
//!
//! What it does: one subcommand per step of the workflow, each a thin front to a module of
//! `geom_test`; `kernel-quality --help` lists them and `kernel-quality <command> --help` gives each
//! one's options.
//!
//! - `mutants-plan` counts the mutants of a run from `cargo mutants --list --json` and prints how
//!   many shards the run needs ([`geom_test::mutants::shard_count`]), with the shard list for a
//!   GitHub Actions matrix.
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
//!   ([`geom_test::miri`]).
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
//! --changed` and `miri-packages`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use geom_test::coverage::{self, CrateReport};
use geom_test::ledger::{LEDGER_PATH, Ledger};
use geom_test::mutants::{self, Mode};

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

/// Which mutation gate is judging.
#[derive(Clone, Copy, ValueEnum)]
enum ModeArg {
    /// A pull request's diff.
    Diff,
    /// The weekly run over every mutant.
    Full,
}

#[derive(Subcommand)]
enum Command {
    /// Prints the number of shards a mutation run needs, and their list as JSON.
    MutantsPlan {
        /// The output of `cargo mutants --list --json` for the run (an empty file: no mutants).
        #[arg(long, value_name = "LIST.json")]
        list: PathBuf,
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
        /// The repository root.
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
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
    let mut failures = Vec::new();
    for krate in &crates {
        let log = dir.join(format!("{krate}.log"));
        match std::fs::read_to_string(&log) {
            Ok(text) => failures.extend(coverage::check_log(krate, &text)),
            Err(e) => failures.push(format!(
                "{krate}: no log of the measuring run ({}: {e})",
                log.display()
            )),
        }
        let file = dir.join(format!("{krate}.lcov"));
        match std::fs::read_to_string(&file) {
            Ok(text) => {
                reports.push(coverage::measure(&root, krate, &text).map_err(|e| e.to_string())?);
            }
            Err(e) => failures.push(format!(
                "{krate}: no coverage data ({}: {e}): every crate the change reaches must be \
                 measured",
                file.display()
            )),
        }
    }
    failures.extend(coverage::judge(&reports));
    failures.extend(coverage::check_marks(&root, &ledger).map_err(|e| e.to_string())?);
    print!("{}", coverage::table(&reports));
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

/// Runs the command: `Ok(true)` when the gate passes, `Ok(false)` when it fails.
fn run(cli: Cli) -> Result<bool, String> {
    match cli.command {
        Command::MutantsPlan { list } => {
            let mutants = mutants::read_mutant_list(&list).map_err(|e| e.to_string())?;
            let count = mutants::shard_count(mutants.len())?;
            println!("{}", mutants::shard_matrix(count));
            Ok(true)
        }
        Command::Mutants {
            mode,
            expected,
            base_ledger,
            repo,
            outs,
        } => {
            let root = canonical(&repo.repo)?;
            let ledger = read_ledger(&root, repo.ledger.as_deref())?;
            let base = base_ledger
                .map(|p| Ledger::read(&p).map_err(|e| format!("base ledger: {e}")))
                .transpose()?;
            let expected = mutants::read_mutant_list(&expected).map_err(|e| e.to_string())?;
            let mut runs = Vec::new();
            for dir in &outs {
                runs.push(mutants::read_run(dir).map_err(|e| e.to_string())?);
            }
            let mode = match mode {
                ModeArg::Diff => Mode::Diff,
                ModeArg::Full => Mode::Full,
            };
            let verdict = mutants::judge(
                &runs,
                &ledger,
                &root,
                mode,
                Some(expected.as_slice()),
                base.as_ref(),
            );
            print!("{}", mutants::report(&verdict));
            Ok(verdict.passes())
        }
        Command::Coverage {
            scope,
            lcov_dir,
            repo,
        } => judge_coverage(&scope, &lcov_dir, &repo),
        Command::Affected { scope, repo } => {
            for krate in measured(&scope, &canonical(&repo)?)? {
                println!("{krate}");
            }
            Ok(true)
        }
        Command::MiriPackages { repo } => {
            let packages = geom_test::miri::packages(&metadata(&canonical(&repo)?)?)?;
            println!("{}", geom_test::miri::cargo_args(&packages));
            Ok(true)
        }
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
