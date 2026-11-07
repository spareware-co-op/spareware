//! `oracle`: the outside oracles' tool: list the catalogue, check it, write a night's jobs, compare
//! the answers, and record a triage.
//!
//! ```text
//! cargo run -p geom-test --bin oracle -- list
//! cargo run -p geom-test --bin oracle -- check [--root <dir>] [--repository <oracle checkout>]
//! cargo run -p geom-test --bin oracle -- jobs geomdl --seed <hex> --count <n>
//! cargo run -p geom-test --bin oracle -- compare geomdl --jobs <file> --answers <file>
//!     [--report <file>] [--baseline <report>] [--perturb]
//! cargo run -p geom-test --bin oracle -- triage geomdl --jobs <file> --id <n> --kind <kind>
//!     --outcome <ours|theirs|within-bounds|undecided> --fixture <id> --why <words>
//!     [--answers <file>] [--issue <address>] [--reference <words>] [--root <dir>]
//! ```
//!
//! - `list` prints every oracle: id, how it runs, its state and its licence.
//! - `check` reads the catalogue, the triage register and the waivers from the working tree,
//!   requires each to be the one this binary was built with, fails if a "theirs" was reproduced on
//!   a pin other than the catalogue's or a waiver names an oracle the catalogue lacks, fails if
//!   `Cargo.lock` holds a package an oracle says must never be linked, and with `--repository`
//!   audits the oracle repository's checkout: nothing can publish an image, and every base image
//!   is pinned by digest.
//! - `jobs` writes a night's jobs, one JSON line each, to standard output, seed printed to standard
//!   error, for the harness in the oracle repository.
//! - `compare` judges each answer and prints the differences by signature. It fails on a signature
//!   the triage register does not hold (each is judged once, 28.10), on a job the register's
//!   record cannot cover (a wrong answer of ours is never cleared, and a "theirs" covers only what
//!   the exact reference settles as theirs), and on a night with fewer cells than the earlier
//!   report `--baseline` names that no waiver covers. `--report` writes the night's report.
//!   `--perturb` also moves one value of every agreeing answer and fails unless the comparison
//!   catches each, so a comparison gone blind is seen.
//! - `triage` records one signature's outcome in the register and writes its fixture from what the
//!   kernel gives for that job today, so "ours" comes after its fix. Any outcome but "ours" is
//!   judged again from `--answers` and recorded with what the exact reference found; "theirs" also
//!   takes `--issue`, the upstream report, and records the job as its reproducer and the
//!   catalogue's pin as what it was reproduced on.
//!
//! Guarantees: nothing here runs an oracle, and only `triage` and `compare --report` write a file:
//! the register and the one fixture, and the report. Exit codes: 0 when everything checked holds; 1
//! when a check fails, a new signature appears or a record cannot cover a job; 2 for a usage error,
//! or a file or a line that cannot be read (a broken harness is never a pass).
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
use geom_test::oracle::geomdl::{self, Answer, Compared, Job, judge, sensitivity};
use geom_test::oracle::triage::{
    self, Evidence, Record, Register, Settled, Signature, fixture as fixture_of, group,
};
use geom_test::oracle::waiver::{self, Observed, Report, Waivers, apply, ratchet};
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
    Compare(Comparing),
    /// Record a signature's outcome and write its fixture.
    Triage(Triaging),
}

#[derive(Args)]
struct Comparing {
    /// The oracle.
    oracle: String,
    /// The jobs, one JSON line each.
    #[arg(long)]
    jobs: PathBuf,
    /// The answers, one JSON line each.
    #[arg(long)]
    answers: PathBuf,
    /// Write the night's report here.
    #[arg(long)]
    report: Option<PathBuf>,
    /// The earlier night's report to hold this one to.
    #[arg(long)]
    baseline: Option<PathBuf>,
    /// Also check that the comparison catches a moved answer.
    #[arg(long)]
    perturb: bool,
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
    /// The answers, one JSON line each; required unless the outcome is `ours`.
    #[arg(long)]
    answers: Option<PathBuf>,
    /// The upstream report's address; required by `theirs`.
    #[arg(long)]
    issue: Option<String>,
    /// The fix, or what settles it.
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
    let waivers = read(&joined(&root, &waiver::WAIVERS_PATH))?;
    if let Err(e) = Waivers::parse(&waivers) {
        problems.push(e.to_string());
    }
    if waivers != waiver::EMBEDDED {
        problems.push("this binary was built with other waivers".to_owned());
    }
    match parsed {
        Err(e) => problems.push(e),
        Ok(catalogue) => {
            if let Ok(register) = Register::parse(&register) {
                problems.extend(register.check_catalogue(&catalogue));
            }
            if let Ok(waivers) = Waivers::parse(&waivers) {
                problems.extend(waivers.check_catalogue(&catalogue));
            }
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
        println!("oracle check: the catalogue, register and waivers hold, and nothing is linked");
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

fn compare(c: &Comparing) -> Result<ExitCode, String> {
    known(&c.oracle)?;
    let jobs: Vec<Job> = lines(&read(&c.jobs)?, "a job")?;
    let answers: Vec<Answer> = lines(&read(&c.answers)?, "an answer")?;
    if jobs.len() != answers.len() {
        return Err(format!(
            "{} jobs and {} answers: the harness stopped early",
            jobs.len(),
            answers.len()
        ));
    }
    let register = Register::embedded().map_err(|e| e.to_string())?;
    let mut differences = Vec::new();
    let mut observed = Vec::new();
    let mut contradictions = 0_usize;
    for (job, answer) in jobs.iter().zip(&answers) {
        let found = match judge(job, answer)? {
            Compared::Agree => None,
            Compared::Differ {
                signature,
                settled,
                verdict,
            } => {
                println!("job {}: {signature}: {settled}", job.id);
                if let Some(why) = register.contradicts(&signature, verdict) {
                    eprintln!("oracle compare: job {}: {why}", job.id);
                    contradictions = contradictions.saturating_add(1);
                }
                let kind = signature.kind.clone();
                differences.push(signature);
                Some((kind, verdict))
            }
        };
        observed.push(Observed {
            operation: job.op.name().to_owned(),
            family: job.family.clone(),
            found,
        });
    }
    for (signature, count) in group(&differences) {
        println!("{signature}: {count}");
    }
    let new = register.new_signatures(&differences);
    println!(
        "oracle compare: {} jobs, {} differences, {} new signatures, {} contradicted",
        jobs.len(),
        differences.len(),
        new.len(),
        contradictions
    );
    for signature in &new {
        eprintln!("oracle compare: new signature {signature}: triage it (28.10)");
    }
    let mut failed = !new.is_empty() || contradictions > 0;
    let report = Report::from_night(&c.oracle, observed);
    if let Some(path) = &c.report {
        let text = report.to_json().map_err(|e| e.to_string())?;
        fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    if let Some(path) = &c.baseline {
        failed |= hold_to(&report, path)?;
    }
    if c.perturb {
        failed |= perturbed_check(&jobs, &answers)?;
    }
    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

/// Holds `report` to the earlier report at `path`; whether a violation no waiver covers remains.
fn hold_to(report: &Report, path: &Path) -> Result<bool, String> {
    let base = Report::parse(&read(path)?).map_err(|e| format!("{}: {e}", path.display()))?;
    let violations = ratchet(&base, report).map_err(|e| e.to_string())?;
    let waivers = Waivers::embedded().map_err(|e| e.to_string())?;
    let applied = apply(&waivers, violations);
    for (violation, reason) in &applied.waived {
        println!("oracle compare: waived: {violation}: {reason}");
    }
    for waiver in &applied.unused {
        eprintln!(
            "oracle compare: a waiver for {} covers nothing: remove it ({})",
            waiver.oracle, waiver.reason
        );
    }
    for violation in &applied.unwaived {
        eprintln!("oracle compare: {violation}");
    }
    Ok(!applied.unwaived.is_empty())
}

/// Moves one value of every agreeing answer; whether the comparison missed any.
fn perturbed_check(jobs: &[Job], answers: &[Answer]) -> Result<bool, String> {
    let found = sensitivity(jobs, answers, judge)?;
    for id in &found.missed {
        eprintln!("oracle compare: job {id}: a moved answer still agrees");
    }
    let blind = !found.missed.is_empty();
    println!(
        "oracle compare: perturbed {}, caught {}, nothing to move {}: {}",
        found.moved,
        found.caught,
        found.unmoved,
        if blind { "BLIND" } else { "SENSITIVE" }
    );
    Ok(blind)
}

/// What the exact reference found, judged again here from the answers, never taken from the
/// command line.
fn settle_verdict(
    outcome: Triage,
    judged: Option<Compared>,
    job: &Job,
    kind: &str,
) -> Result<Settled, String> {
    match (outcome, judged) {
        (Triage::Ours, Some(Compared::Differ { verdict, .. })) => Err(format!(
            "job {} is still a difference ({verdict:?}): ours is recorded after its fix",
            job.id
        )),
        (Triage::Ours, _) => Ok(Settled::Ours),
        (_, None) => {
            Err("--answers is required: the verdict is judged again, not typed".to_owned())
        }
        (_, Some(Compared::Agree)) => Err(format!("job {} agrees: there is no difference", job.id)),
        (
            _,
            Some(Compared::Differ {
                signature, verdict, ..
            }),
        ) => {
            if signature.kind == kind {
                Ok(verdict)
            } else {
                Err(format!(
                    "job {} differs as {:?}, not {kind:?}",
                    job.id, signature.kind
                ))
            }
        }
    }
}

/// What a "theirs" carries: the report, the catalogue's pin, and the job as the reproducer.
fn evidence_for(outcome: Triage, t: &Triaging, job: &Job) -> Result<Option<Evidence>, String> {
    if outcome != Triage::Theirs {
        return Ok(None);
    }
    let issue = t
        .issue
        .clone()
        .ok_or_else(|| "theirs takes --issue, the upstream report's address".to_owned())?;
    let pin = Catalogue::embedded()
        .map_err(|e| e.to_string())?
        .get(&t.oracle)
        .and_then(|o| o.pin.clone())
        .ok_or_else(|| format!("{} has no pin in the catalogue", t.oracle))?;
    Ok(Some(Evidence {
        issue,
        reproduced_on: pin,
        reproducer: serde_json::to_string(job).map_err(|e| e.to_string())?,
    }))
}

fn triage_command(t: &Triaging) -> Result<ExitCode, String> {
    known(&t.oracle)?;
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
    let judged = match &t.answers {
        None => None,
        Some(path) => {
            let answers: Vec<Answer> = lines(&read(path)?, "an answer")?;
            let answer = answers
                .iter()
                .find(|a| a.id == job.id)
                .ok_or_else(|| format!("no answer for job {}", job.id))?;
            Some(judge(job, answer)?)
        }
    };
    let settled = settle_verdict(outcome, judged, job, &t.kind)?;
    let evidence = evidence_for(outcome, t, job)?;
    let record = Record {
        signature: Signature {
            operation: job.op.name().to_owned(),
            oracle: t.oracle.clone(),
            kind: t.kind.clone(),
            family: job.family.clone(),
        },
        outcome,
        settled,
        fixture: t.fixture.clone(),
        reference: t.reference.clone(),
        evidence,
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
    register.records.push(record);
    let register_text = register.to_json().map_err(|e| e.to_string())?;
    // The record's rules hold before anything is written.
    Register::parse(&register_text).map_err(|e| e.to_string())?;
    let path = joined(&root, &fixture_repo::DIR).join(format!("{}.json", fixture.id));
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    fs::write(&register_path, register_text)
        .map_err(|e| format!("{}: {e}", register_path.display()))?;
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
        Tool::Compare(comparing) => compare(&comparing),
        Tool::Triage(triaging) => triage_command(&triaging),
    }
}
