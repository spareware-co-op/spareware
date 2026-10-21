//! `kernel-quality`: the CI tool behind the geometry kernel's mutation and coverage gates
//! (section 28.8; `.github/workflows/kernel-quality.yml`).
//!
//! ```text
//! kernel-quality mutants  --mode diff|full [--expected LIST.json] [--repo DIR] [--ledger FILE] OUT...
//! kernel-quality coverage (--all | --changed FILES.txt | --crates A,B) --lcov-dir DIR [--repo DIR]
//!                         [--ledger FILE]
//! kernel-quality affected (--all | --changed FILES.txt | --crates A,B) [--repo DIR]
//! kernel-quality miri-plan [--repo DIR]
//! ```
//!
//! - `mutants` judges cargo-mutants output directories (`mutants.out`, one per shard) against the
//!   ledger ([`geom_test::mutants`]). `--expected` is the output of `cargo mutants --list --json`
//!   with the run's filters; it is required with `--mode full`, and in either mode proves that no
//!   shard is missing.
//! - `coverage` judges `DIR/<crate>.lcov`, one LCOV file per kernel crate measured by its own
//!   tests, with `DIR/<crate>.log`, the output of the cargo-llvm-cov run that wrote it
//!   ([`geom_test::coverage::check_log`]), for every crate `affected` names, and checks every
//!   `coverage(off)` mark in the kernel ([`geom_test::coverage`]).
//! - `affected` prints the kernel crates a pull request's coverage gate measures, one per line,
//!   leaving out (and naming on standard error) any without a function to measure
//!   ([`geom_test::coverage::has_code`]):
//!   all of them with `--all`, or those the changed files (one path per line, relative to the
//!   repository root) reach ([`geom_test::coverage::affected_crates`]), or the kernel crates named
//!   with `--crates` (for a local run; CI uses `--all` or `--changed`).
//! - `miri-plan` prints the weekly Miri run's jobs as a JSON list for a GitHub Actions matrix
//!   ([`geom_test::miri`]).
//!
//! `--repo` defaults to the current directory and `--ledger` to the repository's ledger
//! (`projects/engines/geometry/quality-ledger.json`).
//!
//! Exit status: 0 when the gate passes, 1 when it fails, 2 when the input cannot be used (a
//! missing or malformed file, a bad argument). Every failure is printed, one per line.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use geom_test::coverage::{self, CrateReport};
use geom_test::ledger::{LEDGER_PATH, Ledger};
use geom_test::mutants::{self, Mode};

const USAGE: &str = "usage:
  kernel-quality mutants  --mode diff|full [--expected LIST.json] [--repo DIR] [--ledger FILE] OUT...
  kernel-quality coverage (--all | --changed FILES.txt | --crates A,B) --lcov-dir DIR [--repo DIR]
                          [--ledger FILE]
  kernel-quality affected (--all | --changed FILES.txt | --crates A,B) [--repo DIR]
  kernel-quality miri-plan [--repo DIR]";

/// The parsed command line.
#[derive(Default)]
struct Args {
    command: String,
    mode: Option<String>,
    expected: Option<PathBuf>,
    repo: Option<PathBuf>,
    ledger: Option<PathBuf>,
    changed: Option<PathBuf>,
    crates: Option<String>,
    all: bool,
    lcov_dir: Option<PathBuf>,
    dirs: Vec<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args_os().skip(1);
    let mut args = Args {
        command: it
            .next()
            .and_then(|c| c.into_string().ok())
            .ok_or("no command")?,
        ..Args::default()
    };
    while let Some(arg) = it.next() {
        let text = arg.to_string_lossy().into_owned();
        let mut value = || {
            it.next()
                .map(PathBuf::from)
                .ok_or(format!("{text} needs a value"))
        };
        match text.as_str() {
            "--mode" => args.mode = Some(value()?.to_string_lossy().into_owned()),
            "--expected" => args.expected = Some(value()?),
            "--repo" => args.repo = Some(value()?),
            "--ledger" => args.ledger = Some(value()?),
            "--changed" => args.changed = Some(value()?),
            "--crates" => args.crates = Some(value()?.to_string_lossy().into_owned()),
            "--lcov-dir" => args.lcov_dir = Some(value()?),
            "--all" => args.all = true,
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ => args.dirs.push(PathBuf::from(arg)),
        }
    }
    Ok(args)
}

fn repo_root(args: &Args) -> Result<PathBuf, String> {
    let repo = args.repo.clone().unwrap_or_else(|| PathBuf::from("."));
    std::fs::canonicalize(&repo).map_err(|e| format!("{}: {e}", repo.display()))
}

fn ledger(args: &Args, repo: &Path) -> Result<Ledger, String> {
    let path = args
        .ledger
        .clone()
        .unwrap_or_else(|| repo.join(LEDGER_PATH));
    Ledger::read(&path).map_err(|e| e.to_string())
}

/// The crates `affected` names for these arguments.
fn affected(args: &Args, repo: &Path) -> Result<Vec<String>, String> {
    let all = geom_test::foundation::kernel_crates(repo).map_err(|e| e.to_string())?;
    match (args.all, &args.changed, &args.crates) {
        (true, None, None) => Ok(all),
        (false, None, Some(list)) => {
            let named: Vec<String> = list.split(',').map(str::trim).map(str::to_owned).collect();
            match named.iter().find(|c| !all.contains(c)) {
                Some(unknown) => Err(format!("{unknown} is not a kernel crate")),
                None => Ok(named),
            }
        }
        (false, Some(file), None) => {
            let text =
                std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
            let changed: Vec<String> = text.lines().map(str::to_owned).collect();
            let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
            let metadata = geom_test::deps::cargo_metadata(&cargo, &repo.join("Cargo.toml"))
                .map_err(|e| e.to_string())?;
            coverage::affected_crates(repo, &changed, &metadata).map_err(|e| e.to_string())
        }
        _ => Err("give exactly one of --all, --changed FILES.txt and --crates A,B".to_owned()),
    }
}

/// The crates `affected` names that have code to measure ([`coverage::has_code`]); the others are
/// named on standard error, so the log shows what was skipped and why.
fn measured(args: &Args, repo: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for krate in affected(args, repo)? {
        if coverage::has_code(repo, &krate).map_err(|e| e.to_string())? {
            out.push(krate);
        } else {
            eprintln!("{krate}: no function outside test code, so no line to measure");
        }
    }
    Ok(out)
}

/// Runs the command: `Ok(true)` when the gate passes, `Ok(false)` when it fails.
fn run(args: &Args) -> Result<bool, String> {
    let repo = repo_root(args)?;
    match args.command.as_str() {
        "mutants" => {
            let mode = match args.mode.as_deref() {
                Some("diff") => Mode::Diff,
                Some("full") => Mode::Full,
                _ => return Err("--mode must be diff or full".to_owned()),
            };
            if args.dirs.is_empty() {
                return Err("name at least one cargo-mutants output directory".to_owned());
            }
            let ledger = ledger(args, &repo)?;
            let expected = match &args.expected {
                Some(f) => Some(mutants::read_mutant_list(f).map_err(|e| e.to_string())?),
                None => None,
            };
            let mut runs = Vec::new();
            for dir in &args.dirs {
                runs.push(mutants::read_run(dir).map_err(|e| e.to_string())?);
            }
            let verdict = mutants::judge(&runs, &ledger, &repo, mode, expected.as_deref());
            print!("{}", mutants::report(&verdict));
            Ok(verdict.passes())
        }
        "coverage" => {
            let ledger = ledger(args, &repo)?;
            let dir = args.lcov_dir.as_ref().ok_or("--lcov-dir is required")?;
            let crates = measured(args, &repo)?;
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
                        reports.push(coverage::measure(&repo, krate, &text).map_err(|e| e.to_string())?);
                    }
                    Err(e) => failures.push(format!(
                        "{krate}: no coverage data ({}: {e}): every crate the change reaches must be \
                         measured",
                        file.display()
                    )),
                }
            }
            failures.extend(coverage::judge(&reports));
            failures.extend(coverage::check_marks(&repo, &ledger).map_err(|e| e.to_string())?);
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
        "miri-plan" => {
            let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
            let metadata = geom_test::deps::cargo_metadata(&cargo, &repo.join("Cargo.toml"))
                .map_err(|e| e.to_string())?;
            println!(
                "{}",
                geom_test::miri::matrix(&geom_test::miri::plan(&metadata)?)
            );
            Ok(true)
        }
        "affected" => {
            for krate in measured(args, &repo)? {
                println!("{krate}");
            }
            Ok(true)
        }
        other => Err(format!("unknown command {other:?}")),
    }
}

fn main() -> ExitCode {
    match parse_args().and_then(|args| run(&args)) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(why) => {
            eprintln!("kernel-quality: {why}\n{USAGE}");
            ExitCode::from(2)
        }
    }
}
