//! `oracle`: the outside oracles' tool: list the catalogue, check it, write a run's jobs, compare
//! the answers, and record a triage.
//!
//! ```text
//! cargo run -p geom-test --bin oracle -- list
//! cargo run -p geom-test --bin oracle -- check [--root <dir>] [--repository <oracle checkout>]
//! cargo run -p geom-test --bin oracle -- jobs <oracle> --seed <hex> --count <n>
//! cargo run -p geom-test --bin oracle -- jobs <mesh oracle> --cases <set> [--seed <hex> --count <n>]
//! cargo run -p geom-test --bin oracle -- compare <oracle> --jobs <file> --answers <file>
//!     [--cases <set>]
//! cargo run -p geom-test --bin oracle -- triage <oracle> --jobs <file> --id <n> --kind <kind>
//!     --outcome <ours|theirs|within-bounds|undecided> --fixture <id> --why <words>
//!     [--reference <words>] [--root <dir>]
//! ```
//!
//! - `list` prints every oracle: id, how it runs, its state and its licence.
//! - `check` reads the catalogue and the triage register from the working tree, requires both to
//!   be the ones this binary was built with, fails if `Cargo.lock` holds a package an oracle says
//!   must never be linked, and with `--repository` audits the oracle repository's checkout:
//!   nothing can publish an image, and every base image is pinned by digest.
//! - `jobs` writes a run's jobs, one JSON line each, to standard output, seed printed to standard
//!   error, for the harness in the oracle repository. A mesh oracle's come from one case set
//!   (`--cases`, `seeded` by default; `geom_test::oracle::meshes::CASE_SETS`): the seeded set from
//!   the seed and count, a stored set from nothing else. A job of a model the set records as
//!   refused (`meshes::REFUSED_MODELS`) is not written: the oracle is not asked.
//! - `compare` judges each answer and prints the differences by signature. It fails on a
//!   difference the triage register does not hold, its signature on its job: a record covers only
//!   the job it was triaged on, so a second cause under a triaged signature fails, and each is
//!   judged once (28.10). A mesh oracle's comparison names its case set (`--cases`, `seeded` by
//!   default), every job must be of it, and
//!   the kernel must refuse each job the set records as refused with exactly that refusal, or the
//!   comparison fails. A job on a model that crosses itself, which the oracle's documentation says
//!   it does not take, is judged by the exact reference alone and printed as outside the oracle's
//!   stated input (decision 32.59).
//! - `triage` records one signature's outcome on one job in the register, the job pinned by its
//!   SHA-256, and writes its fixture from what the kernel gives for that job today, so "ours" comes
//!   after its fix.
//!
//! Guarantees: nothing here runs an oracle, and only `triage` writes a file: the register and the
//! one fixture. Exit codes: 0 when everything checked holds; 1 when a check fails or a difference
//! is not triaged; 2 for a usage error, or a file or a line that cannot be read (a broken harness is
//! never a pass).
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
use geom_format::operation::Operation;
use geom_test::fixture::kernel::Kernel;
use geom_test::fixture::repo as fixture_repo;
use geom_test::fixture::run::replay;
use geom_test::fixture::{OpLog, Triage};
use geom_test::golden::store::Store;
use geom_test::map::repo::workspace_root;
use geom_test::oracle::catalogue::{self, Catalogue, audit, linked};
use geom_test::oracle::geomdl::{self, Compared};
use geom_test::oracle::meshes;
use geom_test::oracle::triage::{
    self, Difference, Record, Register, Signature, fixture as fixture_of, group, job_digest,
};
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
    /// Write a run's jobs to standard output.
    Jobs {
        /// The oracle.
        oracle: String,
        /// A mesh oracle's case set.
        #[arg(long, default_value = meshes::SEEDED)]
        cases: String,
        /// The seed, in hexadecimal: the seeded set's.
        #[arg(long, requires = "count")]
        seed: Option<String>,
        /// How many: the seeded set's.
        #[arg(long, requires = "seed")]
        count: Option<u64>,
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
        /// A mesh oracle's case set, the one the jobs were written from.
        #[arg(long, default_value = meshes::SEEDED)]
        cases: String,
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
    if oracle == geomdl::ID || meshes::holds(oracle) {
        Ok(())
    } else {
        Err(format!("no jobs for {oracle}"))
    }
}

fn jobs(
    oracle: &str,
    cases: &str,
    seed: Option<&str>,
    count: Option<u64>,
) -> Result<ExitCode, String> {
    known(oracle)?;
    let seeded = match (seed, count) {
        (Some(seed), Some(count)) => {
            let seed =
                u64::from_str_radix(seed, 16).map_err(|e| format!("the seed {seed:?}: {e}"))?;
            eprintln!("seed: {seed:016x}");
            Some((Seed(seed), count))
        }
        _ => None,
    };
    if meshes::holds(oracle) {
        let all = meshes::case_jobs(cases, seeded)?;
        let jobs: Vec<&meshes::Job> = all.iter().filter(|j| meshes::asked(j)).collect();
        eprintln!(
            "cases: {cases}, {} jobs, {} more recorded as refused and not asked",
            jobs.len(),
            all.len().saturating_sub(jobs.len())
        );
        printed(&jobs)?;
    } else if cases != meshes::SEEDED {
        return Err(format!("{oracle} has only seeded jobs, not {cases}"));
    } else {
        let (seed, count) = seeded.ok_or("geomdl's jobs need --seed and --count")?;
        printed(&geomdl::jobs(seed, count))?;
    }
    Ok(ExitCode::SUCCESS)
}

/// `jobs`, one JSON line each.
fn printed<T: serde::Serialize>(jobs: &[T]) -> Result<(), String> {
    for job in jobs {
        println!("{}", serde_json::to_string(job).map_err(|e| e.to_string())?);
    }
    Ok(())
}

/// Each line of `text` as a `T`.
fn lines<T: serde::de::DeserializeOwned>(text: &str, what: &str) -> Result<Vec<T>, String> {
    text.lines()
        .map(|line| serde_json::from_str(line).map_err(|e| format!("{what}: {e}: {line}")))
        .collect()
}

/// Each answer in `answers` judged against its job in `jobs`, each difference printed, and each
/// job outside the oracle's stated input: how many jobs there were, and each difference with its
/// job's number.
fn judged<J, A>(
    jobs: &str,
    answers: &str,
    mut judge: impl FnMut(&J, &A) -> Result<(u64, Compared), String>,
) -> Result<(usize, Vec<(u64, Difference)>), String>
where
    J: serde::de::DeserializeOwned + serde::Serialize,
    A: serde::de::DeserializeOwned,
{
    let jobs: Vec<J> = lines(jobs, "a job")?;
    let answers: Vec<A> = lines(answers, "an answer")?;
    if jobs.len() != answers.len() {
        return Err(format!(
            "{} jobs and {} answers: the harness stopped early",
            jobs.len(),
            answers.len()
        ));
    }
    let mut differences = Vec::new();
    let mut outside = 0_usize;
    for (job, answer) in jobs.iter().zip(&answers) {
        match judge(job, answer)? {
            (id, Compared::Differ { signature, settled }) => {
                println!("job {id}: {signature}: {settled}");
                let job = pin(job)?;
                differences.push((id, Difference { signature, job }));
            }
            (id, Compared::Outside { why }) => {
                println!("job {id}: outside the oracle's stated input: {why}");
                outside = outside.saturating_add(1);
            }
            (_, Compared::Agree) => {}
        }
    }
    println!("oracle compare: {outside} jobs outside the oracle's stated input");
    Ok((jobs.len(), differences))
}

/// The kernel held to each refusal the case set `cases` records, each printed: whether every one
/// held.
fn refusals_held(cases: &str) -> Result<bool, String> {
    let mut held = true;
    for (job, expected) in meshes::recorded_refusals(cases)? {
        match meshes::held_refusal(&job, expected) {
            Ok(()) => println!(
                "job {}: {}: refused as recorded, {expected:?}; not asked",
                job.id, job.family
            ),
            Err(why) => {
                eprintln!("oracle compare: {why}");
                held = false;
            }
        }
    }
    Ok(held)
}

fn compare(oracle: &str, jobs: &Path, answers: &Path, cases: &str) -> Result<ExitCode, String> {
    known(oracle)?;
    let (jobs, answers) = (read(jobs)?, read(answers)?);
    let mut held = true;
    let (count, differences) = if meshes::holds(oracle) {
        let mut judge = meshes::Judge::new();
        let found = judged(&jobs, &answers, |job: &meshes::Job, answer| {
            if !meshes::of_case_set(cases, job) {
                return Err(format!("job {} is not of the case set {cases}", job.id));
            }
            Ok((job.id, judge.judge(oracle, job, answer)?))
        })?;
        held = refusals_held(cases)?;
        found
    } else if cases != meshes::SEEDED {
        return Err(format!("{oracle} has only seeded jobs, not {cases}"));
    } else {
        judged(&jobs, &answers, |job: &geomdl::Job, answer| {
            Ok((job.id, geomdl::judge(job, answer)?))
        })?
    };
    let register = Register::embedded().map_err(|e| e.to_string())?;
    for (signature, count) in group(differences.iter().map(|(_, d)| &d.signature)) {
        println!("{signature}: {count}");
    }
    let new: Vec<&(u64, Difference)> = differences
        .iter()
        .filter(|(_, d)| !register.knows(d))
        .collect();
    println!(
        "oracle compare: {} jobs, {} differences, {} not triaged",
        count,
        differences.len(),
        new.len()
    );
    for (id, d) in &new {
        eprintln!(
            "oracle compare: job {id}: {} is not triaged on this job (job {}): triage it (28.10)",
            d.signature, d.job
        );
    }
    if !held {
        eprintln!("oracle compare: a job recorded as refused was not refused so");
    }
    Ok(if new.is_empty() && held {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// The pin of `job` ([`job_digest`]): of its line as `jobs` prints it.
fn pin<T: serde::Serialize>(job: &T) -> Result<String, String> {
    let line = serde_json::to_string(job).map_err(|e| e.to_string())?;
    Ok(job_digest(&line))
}

/// Job `id` of `text`: its operation's name, its family, its operation for a log, and its pin.
fn chosen(
    oracle: &str,
    text: &str,
    id: u64,
) -> Result<(String, String, Operation, String), String> {
    let missing = || format!("no job {id}");
    if meshes::holds(oracle) {
        let jobs: Vec<meshes::Job> = lines(text, "a job")?;
        let job = jobs.iter().find(|j| j.id == id).ok_or_else(missing)?;
        let name = job.op.name().to_owned();
        Ok((name, job.family.clone(), meshes::operation(job)?, pin(job)?))
    } else {
        let jobs: Vec<geomdl::Job> = lines(text, "a job")?;
        let job = jobs.iter().find(|j| j.id == id).ok_or_else(missing)?;
        let name = job.op.name().to_owned();
        Ok((name, job.family.clone(), geomdl::operation(job)?, pin(job)?))
    }
}

fn triage_command(t: &Triaging) -> Result<ExitCode, String> {
    known(&t.oracle)?;
    let root = match &t.root.root {
        Some(r) => r.clone(),
        None => workspace_root().map_err(|e| e.to_string())?,
    };
    let (operation, family, logged, job) = chosen(&t.oracle, &read(&t.jobs)?, t.id)?;
    let outcome: Triage = serde_json::from_value(serde_json::Value::String(t.outcome.clone()))
        .map_err(|e| format!("the outcome {:?}: {e}", t.outcome))?;
    let record = Record {
        signature: Signature {
            operation,
            oracle: t.oracle.clone(),
            kind: t.kind.clone(),
            family,
        },
        job,
        outcome,
        fixture: t.fixture.clone(),
        reference: t.reference.clone(),
        why: t.why.clone(),
    };
    let register_path = joined(&root, &triage::REGISTER_PATH);
    let mut register = Register::parse(&read(&register_path)?).map_err(|e| e.to_string())?;
    let difference = Difference {
        signature: record.signature.clone(),
        job: record.job.clone(),
    };
    if register.knows(&difference) {
        eprintln!(
            "oracle triage: {} on job {} is already triaged",
            record.signature, t.id
        );
        return Ok(ExitCode::from(1));
    }
    let log = OpLog::new(Vec::new(), vec![logged]);
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
            cases,
            seed,
            count,
        } => jobs(&oracle, &cases, seed.as_deref(), count),
        Tool::Compare {
            oracle,
            jobs,
            answers,
            cases,
        } => compare(&oracle, &jobs, &answers, &cases),
        Tool::Triage(triaging) => triage_command(&triaging),
    }
}
