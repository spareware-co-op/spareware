//! `oracle`: the outside oracles' tool: list the catalogue, check it, write a night's jobs, compare
//! the answers, and record a triage.
//!
//! ```text
//! cargo run -p geom-test --bin oracle -- list
//! cargo run -p geom-test --bin oracle -- check [--root <dir>] [--repository <oracle checkout>]
//! cargo run -p geom-test --bin oracle -- jobs geomdl --seed <hex> --count <n>
//! cargo run -p geom-test --bin oracle -- compare geomdl --jobs <file> --answers <file>
//! cargo run -p geom-test --bin oracle -- triage geomdl --jobs <file> --id <n> --kind <kind>
//!     --outcome <ours|theirs|within-bounds|undecided> --fixture <id> --why <words>
//!     [--reference <words>] [--root <dir>]
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
//! - `triage` records one signature's outcome in the register and writes its fixture from what the
//!   kernel gives for that job today, so "ours" comes after its fix.
//!
//! Guarantees: nothing here runs an oracle, and only `triage` writes a file: the register and the
//! one fixture. Exit codes: 0 when everything checked
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
use geom_test::fixture::kernel::Kernel;
use geom_test::fixture::repo as fixture_repo;
use geom_test::fixture::run::replay;
use geom_test::fixture::{OpLog, Triage};
use geom_test::golden::store::Store;
use geom_test::map::repo::workspace_root;
use geom_test::oracle::catalogue::{self, Catalogue, audit, linked};
use geom_test::oracle::geomdl::{self, Answer, Compared, Job, judge_for};
use geom_test::oracle::triage::{self, Record, Register, Signature, fixture as fixture_of, group};
use geom_test::oracle::{sketches, smlib};
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
    /// Record a signature's outcome and write its fixture.
    Triage(Triaging),
}

#[derive(Args)]
struct Triaging {
    /// The oracle.
    oracle: String,
    /// The jobs, one JSON line each.
    #[arg(long)]
    jobs: PathBuf,
    /// The job that shows it.
    #[arg(long)]
    id: u64,
    /// The kind of difference.
    #[arg(long)]
    kind: String,
    /// `ours`, `theirs`, `within-bounds` or `undecided`.
    #[arg(long)]
    outcome: String,
    /// The fixture's id.
    #[arg(long)]
    fixture: String,
    /// Why, in a sentence.
    #[arg(long)]
    why: String,
    /// The upstream report, the fix, or what settles it.
    #[arg(long)]
    reference: Option<String>,
    #[command(flatten)]
    root: Root,
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
    if oracle == geomdl::ID || oracle == smlib::ID || sketch(oracle) {
        Ok(())
    } else {
        Err(format!("no jobs for {oracle}"))
    }
}

/// Whether `oracle` is a sketch solver, whose jobs are sketches (G222).
fn sketch(oracle: &str) -> bool {
    oracle == sketches::EZPZ || oracle == sketches::FIKSI
}

fn jobs(oracle: &str, seed: &str, count: u64) -> Result<ExitCode, String> {
    known(oracle)?;
    let seed = u64::from_str_radix(seed, 16).map_err(|e| format!("the seed {seed:?}: {e}"))?;
    eprintln!("seed: {seed:016x}");
    if let Some(drawn) = sketches::jobs(oracle, Seed(seed), count) {
        for job in drawn {
            println!(
                "{}",
                serde_json::to_string(&job).map_err(|e| e.to_string())?
            );
        }
        return Ok(ExitCode::SUCCESS);
    }
    let drawn = if oracle == smlib::ID {
        smlib::jobs(Seed(seed), count)
    } else {
        geomdl::jobs(Seed(seed), count)
    };
    for job in drawn {
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
    let (count, judged) = if sketch(oracle) {
        let jobs: Vec<sketches::Job> = lines(&read(jobs)?, "a job")?;
        let answers: Vec<sketches::Answer> = lines(&read(answers)?, "an answer")?;
        let judged = paired(&jobs, &answers)?
            .map(|(job, answer)| Ok((job.id, sketches::judge(oracle, job, answer)?)))
            .collect::<Result<Vec<_>, String>>()?;
        (jobs.len(), judged)
    } else {
        let jobs: Vec<Job> = lines(&read(jobs)?, "a job")?;
        let answers: Vec<Answer> = lines(&read(answers)?, "an answer")?;
        let judged = paired(&jobs, &answers)?
            .map(|(job, answer)| Ok((job.id, judge_for(oracle, job, answer)?)))
            .collect::<Result<Vec<_>, String>>()?;
        (jobs.len(), judged)
    };
    let mut differences = Vec::new();
    for (id, compared) in judged {
        if let Compared::Differ { signature, settled } = compared {
            println!("job {id}: {signature}: {settled}");
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
        count,
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

/// Each job with its answer, refusing answers that stop early.
fn paired<'a, J, A>(
    jobs: &'a [J],
    answers: &'a [A],
) -> Result<impl Iterator<Item = (&'a J, &'a A)>, String> {
    if jobs.len() == answers.len() {
        Ok(jobs.iter().zip(answers))
    } else {
        Err(format!(
            "{} jobs and {} answers: the harness stopped early",
            jobs.len(),
            answers.len()
        ))
    }
}

fn triage_command(t: &Triaging) -> Result<ExitCode, String> {
    known(&t.oracle)?;
    if sketch(&t.oracle) {
        return Err(format!(
            "{}: a sketch job is triaged by hand until G231 writes its fixtures",
            t.oracle
        ));
    }
    let root = match &t.root.root {
        Some(r) => r.clone(),
        None => workspace_root().map_err(|e| e.to_string())?,
    };
    let jobs: Vec<Job> = lines(&read(&t.jobs)?, "a job")?;
    let job = jobs
        .iter()
        .find(|j| j.id == t.id)
        .ok_or_else(|| format!("no job {}", t.id))?;
    let outcome: Triage = serde_json::from_value(serde_json::Value::String(t.outcome.clone()))
        .map_err(|e| format!("the outcome {:?}: {e}", t.outcome))?;
    let record = Record {
        signature: Signature {
            operation: job.op.name().to_owned(),
            oracle: t.oracle.clone(),
            kind: t.kind.clone(),
            family: job.family.clone(),
        },
        outcome,
        fixture: t.fixture.clone(),
        reference: t.reference.clone(),
        why: t.why.clone(),
    };
    let register_path = joined(&root, &triage::REGISTER_PATH);
    let mut register = Register::parse(&read(&register_path)?).map_err(|e| e.to_string())?;
    if register.knows(&record.signature) {
        eprintln!("oracle triage: {} is already triaged", record.signature);
        return Ok(ExitCode::from(1));
    }
    let log = OpLog::new(Vec::new(), vec![geomdl::operation(job)?]);
    let store = Store::embedded().map_err(|e| e.to_string())?;
    let edition = store.current().map_err(|e| e.to_string())?.edition.clone();
    let key = (edition, log.versions());
    let replayed = replay(&mut Kernel::new(), &log);
    let fixture = fixture_of(&record, log, key, &replayed)?;
    let text = fixture.to_json().map_err(|e| e.to_string())?;
    let path = joined(&root, &fixture_repo::DIR).join(format!("{}.json", fixture.id));
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    register.records.push(record);
    let text = register.to_json().map_err(|e| e.to_string())?;
    fs::write(&register_path, text).map_err(|e| format!("{}: {e}", register_path.display()))?;
    println!("oracle triage: {}", fixture.id);
    Ok(ExitCode::SUCCESS)
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
        Tool::Triage(triaging) => triage_command(&triaging),
    }
}
