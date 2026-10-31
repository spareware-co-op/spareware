//! `oracle`: the outside oracles' tool: list the catalogue, check it, write a night's jobs, compare
//! the answers, and record a triage.
//!
//! ```text
//! cargo run -p geom-test --bin oracle -- list
//! cargo run -p geom-test --bin oracle -- check [--root <dir>] [--repository <oracle checkout>]
//! cargo run -p geom-test --bin oracle -- jobs geomdl --seed <hex> --count <n>
//! cargo run -p geom-test --bin oracle -- compare geomdl --jobs <file> --answers <file>
//! ```
//!
//! - `list` prints every oracle: id, how it runs, its state and its licence.
//! - `check` reads the catalogue and the triage register from the working tree, requires both to
//!   be the ones this binary was built with, fails if `Cargo.lock` holds a package an oracle says
//!   must never be linked, and with `--repository` audits the oracle repository's checkout:
//!   nothing can publish an image, and every base image is pinned by digest.
//! - `jobs` writes a night's jobs, one JSON line each, to standard output, seed printed to standard
//!   error, for the harness in the oracle repository.
//! - `compare` judges each answer and prints the differences by signature. It fails only on a
//!   signature the triage register does not hold: each is judged once (28.10).
//!
//! Guarantees: nothing here runs an oracle or writes a file. Exit codes: 0 when everything checked
//! holds; 1 when a check fails or a new signature appears; 2 for a usage error, or a file or a line
//! that cannot be read (a broken harness is never a pass).
//!
//! Method: none; it parses its arguments and calls [`geom_test::oracle`], whose module headers
//! carry the methods.
//!
//! Cost: `compare` is linear in the jobs.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use geom_test::map::repo::workspace_root;
use geom_test::oracle::catalogue::{self, Catalogue, audit, linked};
use geom_test::oracle::geomdl::{self, Answer, Compared, Job, judge};
use geom_test::oracle::triage::{self, Register, group};
use geom_test::seed::Seed;

/// The outside oracles' tool.
#[derive(Parser)]
#[command(name = "oracle")]
struct Cli {
    #[command(subcommand)]
    command: Tool,
}

#[derive(Args)]
struct Root {
    /// The workspace root; the one this binary was built in by default.
    #[arg(long)]
    root: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Tool {
    /// Print every oracle.
    List,
    /// Check the catalogue, the register and nothing linked or published.
    Check {
        /// A checkout of the oracle repository, to audit.
        #[arg(long)]
        repository: Option<PathBuf>,
        #[command(flatten)]
        root: Root,
    },
    /// Write a night's jobs to standard output.
    Jobs {
        /// The oracle.
        oracle: String,
        /// The seed, in hexadecimal.
        #[arg(long)]
        seed: String,
        /// How many.
        #[arg(long)]
        count: u64,
    },
    /// Judge the answers.
    Compare {
        /// The oracle.
        oracle: String,
        /// The jobs, one JSON line each.
        #[arg(long)]
        jobs: PathBuf,
        /// The answers, one JSON line each.
        #[arg(long)]
        answers: PathBuf,
    },
}

fn main() -> ExitCode {
    match command(Cli::parse().command) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("oracle: {message}");
            ExitCode::from(2)
        }
    }
}

fn list() -> Result<ExitCode, String> {
    let catalogue = Catalogue::embedded().map_err(|e| e.to_string())?;
    for oracle in catalogue.oracles {
        println!(
            "{} {:?} {:?} {} ({})",
            oracle.id, oracle.run, oracle.state, oracle.licence, oracle.piece
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn joined(root: &Path, parts: &[&str]) -> PathBuf {
    parts
        .iter()
        .fold(root.to_path_buf(), |p, part| p.join(part))
}

fn check(repository: Option<&Path>, root: &Root) -> Result<ExitCode, String> {
    let root = match &root.root {
        Some(r) => r.clone(),
        None => workspace_root().map_err(|e| e.to_string())?,
    };
    let mut problems = Vec::new();
    let text = read(&joined(&root, &catalogue::CATALOGUE_PATH))?;
    let parsed = Catalogue::parse(&text).map_err(|e| e.to_string());
    if text != catalogue::EMBEDDED {
        problems.push("this binary was built with another catalogue".to_owned());
    }
    let register = read(&joined(&root, &triage::REGISTER_PATH))?;
    if let Err(e) = Register::parse(&register) {
        problems.push(e.to_string());
    }
    if register != triage::EMBEDDED {
        problems.push("this binary was built with another triage register".to_owned());
    }
    match parsed {
        Err(e) => problems.push(e),
        Ok(catalogue) => {
            let lock = read(&root.join("Cargo.lock"))?;
            for package in linked(&catalogue, &lock)? {
                problems.push(format!("Cargo.lock holds {package}, never to be linked"));
            }
            if let Some(repository) = repository {
                problems.extend(audit(&catalogue, repository)?);
            }
        }
    }
    for problem in &problems {
        eprintln!("oracle check: {problem}");
    }
    if problems.is_empty() {
        println!("oracle check: the catalogue and register hold, and nothing is linked");
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn known(oracle: &str) -> Result<(), String> {
    if oracle == geomdl::ID {
        Ok(())
    } else {
        Err(format!("no jobs for {oracle}"))
    }
}

fn jobs(oracle: &str, seed: &str, count: u64) -> Result<ExitCode, String> {
    known(oracle)?;
    let seed = u64::from_str_radix(seed, 16).map_err(|e| format!("the seed {seed:?}: {e}"))?;
    eprintln!("seed: {seed:016x}");
    for job in geomdl::jobs(Seed(seed), count) {
        println!(
            "{}",
            serde_json::to_string(&job).map_err(|e| e.to_string())?
        );
    }
    Ok(ExitCode::SUCCESS)
}

/// Each line of `text` as a `T`.
fn lines<T: serde::de::DeserializeOwned>(text: &str, what: &str) -> Result<Vec<T>, String> {
    text.lines()
        .map(|line| serde_json::from_str(line).map_err(|e| format!("{what}: {e}: {line}")))
        .collect()
}

fn compare(oracle: &str, jobs: &Path, answers: &Path) -> Result<ExitCode, String> {
    known(oracle)?;
    let jobs: Vec<Job> = lines(&read(jobs)?, "a job")?;
    let answers: Vec<Answer> = lines(&read(answers)?, "an answer")?;
    if jobs.len() != answers.len() {
        return Err(format!(
            "{} jobs and {} answers: the harness stopped early",
            jobs.len(),
            answers.len()
        ));
    }
    let mut differences = Vec::new();
    for (job, answer) in jobs.iter().zip(&answers) {
        if let Compared::Differ { signature, settled } = judge(job, answer)? {
            println!("job {}: {signature}: {settled}", job.id);
            differences.push(signature);
        }
    }
    let register = Register::embedded().map_err(|e| e.to_string())?;
    for (signature, count) in group(&differences) {
        println!("{signature}: {count}");
    }
    let new = register.new_signatures(&differences);
    println!(
        "oracle compare: {} jobs, {} differences, {} new signatures",
        jobs.len(),
        differences.len(),
        new.len()
    );
    for signature in &new {
        eprintln!("oracle compare: new signature {signature}: triage it (28.10)");
    }
    Ok(if new.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn command(tool: Tool) -> Result<ExitCode, String> {
    match tool {
        Tool::List => list(),
        Tool::Check { repository, root } => check(repository.as_deref(), &root),
        Tool::Jobs {
            oracle,
            seed,
            count,
        } => jobs(&oracle, &seed, count),
        Tool::Compare {
            oracle,
            jobs,
            answers,
        } => compare(&oracle, &jobs, &answers),
    }
}
