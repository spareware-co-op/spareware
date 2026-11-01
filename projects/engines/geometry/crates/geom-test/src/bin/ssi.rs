//! `ssi`: the intersection acceptance harness's tool: place the corpus, write the catalogue, and
//! hold a scored run to chapter 12.17's gate.
//!
//! ```text
//! cargo run -p geom-test --bin ssi -- check
//! cargo run -p geom-test --bin ssi -- list --cases <dir>
//! cargo run -p geom-test --bin ssi -- place --cases <dir> [--tolerance <t>]
//! cargo run -p geom-test --bin ssi -- catalogue --out <dir>
//! cargo run -p geom-test --bin ssi -- gate --cases <dir> --raw <dir> --scores <file>
//!     --engine <name> --mode <certify|default> [--tolerance <t>] [--ceilings <file>]
//!     [--must-fail <rule>] [--report <file>]
//! cargo run -p geom-test --bin ssi -- measure --cases <dir> --raw <dir> --out <file> --why <text>
//! cargo run -p geom-test --bin ssi -- same <report> <report>
//! ```
//!
//! - `check` reads the register of ceilings, and places every case of the catalogue.
//! - `list` counts a folder's cases by class.
//! - `place` places every case of a folder, and holds the kernel's kinds to the case's closed forms.
//! - `catalogue` writes the failure catalogue in the corpus's schema.
//! - `gate` judges a run and prints what failed. With `--must-fail <rule>` the run must break that
//!   rule: this is how a perturbed answer proves the gate fails.
//! - `measure` writes the ceilings a set of answers needs, for a run that is to be judged by them.
//! - `same` compares two reports' answers bit for bit: 12.17's replay.
//!
//! Guarantees: `catalogue`, `measure` and `gate --report` write only what they are given; the rest
//! write nothing. Exit codes: 0 when everything checked holds; 1 when a check fails; 2 for a usage
//! error or a file that cannot be read or written.
//!
//! Method: none; it parses its arguments and calls [`geom_test::ssi`], whose header carries the
//! method.
//!
//! Cost: `gate` costs [`geom_test::ssi::cover`] for each case; the rest are linear.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use geom_test::corpus::{EMBEDDED, Manifest};
use geom_test::ssi::SsiError;
use geom_test::ssi::case::{Case, Class};
use geom_test::ssi::catalogue;
use geom_test::ssi::gate::{
    CEILINGS, Ceilings, Failure, Mode, Report, Run, accuracy_of, run, total_size,
};
use geom_test::ssi::output::Output;
use geom_test::ssi::place::{disagreement, place};

/// The tolerance the corpus's reference numbers were measured at.
const REFERENCE_TOLERANCE: f64 = 1e-6;

/// The harness's tool.
#[derive(Parser)]
#[command(name = "ssi")]
struct Cli {
    #[command(subcommand)]
    command: Tool,
}

#[derive(Args)]
struct Folder {
    /// The folder of cases.
    #[arg(long)]
    cases: PathBuf,
}

/// Which rules a run answers to.
#[derive(Clone, Copy, ValueEnum)]
enum Rules {
    /// Certify mode.
    Certify,
    /// The default mode.
    Default,
}

#[derive(Args)]
struct Gate {
    /// The folder of cases.
    #[arg(long)]
    cases: PathBuf,
    /// The folder of the runner's answers.
    #[arg(long)]
    raw: PathBuf,
    /// The scorer's file.
    #[arg(long)]
    scores: PathBuf,
    /// What answered.
    #[arg(long)]
    engine: String,
    /// The rules it answers to.
    #[arg(long, value_enum)]
    mode: Rules,
    /// The approximation tolerance of the run.
    #[arg(long, default_value_t = REFERENCE_TOLERANCE)]
    tolerance: f64,
    /// Ceilings to use in place of the register.
    #[arg(long)]
    ceilings: Option<PathBuf>,
    /// The run must break this rule.
    #[arg(long)]
    must_fail: Option<String>,
    /// Where to write the report.
    #[arg(long)]
    report: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Tool {
    /// Check the ceilings and the catalogue.
    Check,
    /// Count a folder's cases by class.
    List(Folder),
    /// Place every case of a folder.
    Place {
        #[command(flatten)]
        folder: Folder,
        /// The approximation tolerance of the run.
        #[arg(long, default_value_t = REFERENCE_TOLERANCE)]
        tolerance: f64,
    },
    /// Write the failure catalogue.
    Catalogue {
        /// Where.
        #[arg(long)]
        out: PathBuf,
    },
    /// Judge a run.
    Gate(Gate),
    /// Write the ceilings a set of answers needs.
    Measure {
        #[command(flatten)]
        folder: Folder,
        /// The folder of the runner's answers.
        #[arg(long)]
        raw: PathBuf,
        /// Where.
        #[arg(long)]
        out: PathBuf,
        /// Why they stand there.
        #[arg(long)]
        why: String,
    },
    /// Compare two reports' answers.
    Same {
        /// One report.
        first: PathBuf,
        /// The other.
        second: PathBuf,
    },
}

/// The file at `path`, as text.
fn text(path: &Path) -> Result<String, SsiError> {
    fs::read_to_string(path).map_err(|e| SsiError::File {
        path: path.display().to_string(),
        why: e.to_string(),
    })
}

/// The cases of `dir`, by name.
fn cases(dir: &Path) -> Result<Vec<Case>, SsiError> {
    let failed = |why: String| SsiError::File {
        path: dir.display().to_string(),
        why,
    };
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| failed(e.to_string()))? {
        let path = entry.map_err(|e| failed(e.to_string()))?.path();
        if path.extension().is_some_and(|e| e == "json") {
            paths.push(path);
        }
    }
    paths.sort();
    paths.iter().map(|p| Case::read(p)).collect()
}

/// Every case of `all`, placed and held to its closed forms; prints a line each, and gives how many
/// failed.
fn place_all(all: &[Case], tolerance: f64, quiet: bool) -> usize {
    let mut failed = 0_usize;
    for case in all {
        let result = place(case, Some(accuracy_of(tolerance))).and_then(|placed| {
            let off = disagreement(case, &placed)?;
            Ok((placed, off))
        });
        match result {
            Ok((placed, off)) if off <= case.cov_tol => {
                if !quiet {
                    let moved = placed.moved();
                    println!(
                        "{} 2^{} moved {moved} kinds off by {off:e}",
                        case.id, placed.log2
                    );
                }
            }
            Ok((_, off)) => {
                failed = failed.saturating_add(1);
                println!(
                    "{}: the kernel's kinds lie {off:e} off its closed forms",
                    case.id
                );
            }
            Err(e) => {
                failed = failed.saturating_add(1);
                println!("{e}");
            }
        }
    }
    failed
}

/// The register's credit for the corpus.
fn credit() -> Result<String, SsiError> {
    let malformed = |why: String| SsiError::Malformed {
        what: "the corpus register".into(),
        why,
    };
    let manifest = Manifest::parse(EMBEDDED).map_err(|e| malformed(e.to_string()))?;
    let entry = manifest
        .corpora
        .iter()
        .find(|c| c.id == "ssi-corpus")
        .ok_or_else(|| malformed("no ssi-corpus".into()))?;
    Ok(entry.credit.clone().unwrap_or_default())
}

/// `gate`: judges a run, and whether it went as asked.
fn gate(g: &Gate) -> Result<bool, SsiError> {
    let ceilings = match &g.ceilings {
        Some(path) => Ceilings::parse(&text(path)?)?,
        None => Ceilings::parse(CEILINGS)?,
    };
    let credit = credit()?;
    let report = run(&Run {
        cases: &g.cases,
        raw: &g.raw,
        scores: &g.scores,
        engine: &g.engine,
        mode: match g.mode {
            Rules::Certify => Mode::Certify,
            Rules::Default => Mode::Default,
        },
        tolerance: g.tolerance,
        ceilings: &ceilings,
        credit: &credit,
    })?;
    for row in report.rows.iter().filter(|r| !r.failures.is_empty()) {
        let rules: Vec<&str> = row.failures.iter().map(Failure::name).collect();
        println!("{}: {:?}: {}", row.id, row.verdict, rules.join(", "));
    }
    println!(
        "{}: {} cases, {} failed",
        report.engine,
        report.rows.len(),
        report.failed()
    );
    if let Some(path) = &g.report {
        let body = serde_json::to_string_pretty(&report).map_err(|e| SsiError::File {
            path: path.display().to_string(),
            why: e.to_string(),
        })?;
        fs::write(path, body).map_err(|e| SsiError::File {
            path: path.display().to_string(),
            why: e.to_string(),
        })?;
    }
    Ok(match &g.must_fail {
        Some(rule) => {
            let broke = report.broke(rule);
            if !broke {
                println!("{}: expected to break {rule}, and did not", report.engine);
            }
            broke
        }
        None => report.failed() == 0,
    })
}

/// `measure`: the ceilings the answers in `raw` need.
fn measure(dir: &Path, raw: &Path, why: &str) -> Result<Ceilings, SsiError> {
    let mut ceilings = Ceilings::parse(CEILINGS)?;
    for case in cases(dir)? {
        let class = case.class()?;
        let path = raw.join(format!("{}.json", case.id));
        let output: Output =
            serde_json::from_str(&text(&path)?).map_err(|e| SsiError::Malformed {
                what: path.display().to_string(),
                why: e.to_string(),
            })?;
        if !output.flagged.is_empty() {
            let count = output.flagged.len() as u64;
            ceilings.raise(class, count, total_size(&output.flagged), why);
        }
    }
    Ok(ceilings)
}

/// `same`: the cases two reports answer differently.
fn same(first: &Path, second: &Path) -> Result<Vec<String>, SsiError> {
    let read = |path: &Path| -> Result<Report, SsiError> {
        serde_json::from_str(&text(path)?).map_err(|e| SsiError::Malformed {
            what: path.display().to_string(),
            why: e.to_string(),
        })
    };
    let (a, b) = (read(first)?, read(second)?);
    let mut differ = a.differs_from(&b);
    differ.extend(b.differs_from(&a));
    differ.sort();
    differ.dedup();
    Ok(differ)
}

/// The tool, past its arguments: whether everything checked holds.
fn tool(command: &Tool) -> Result<bool, SsiError> {
    match command {
        Tool::Check => {
            Ceilings::parse(CEILINGS)?;
            let all = catalogue::all();
            let failed = place_all(&all, REFERENCE_TOLERANCE, true);
            println!("{} catalogue cases, {failed} failed", all.len());
            Ok(failed == 0)
        }
        Tool::List(folder) => {
            let mut counts: BTreeMap<Class, usize> = BTreeMap::new();
            for case in cases(&folder.cases)? {
                let count = counts.entry(case.class()?).or_default();
                *count = count.saturating_add(1);
            }
            for (class, count) in counts {
                println!("{class:?} {count}");
            }
            Ok(true)
        }
        Tool::Place { folder, tolerance } => {
            let all = cases(&folder.cases)?;
            let failed = place_all(&all, *tolerance, false);
            println!("{} cases, {failed} failed", all.len());
            Ok(failed == 0)
        }
        Tool::Catalogue { out } => {
            catalogue::write(out)?;
            Ok(true)
        }
        Tool::Gate(g) => gate(g),
        Tool::Measure {
            folder,
            raw,
            out,
            why,
        } => {
            let ceilings = measure(&folder.cases, raw, why)?;
            let body = serde_json::to_string_pretty(&ceilings).map_err(|e| SsiError::File {
                path: out.display().to_string(),
                why: e.to_string(),
            })?;
            fs::write(out, body).map_err(|e| SsiError::File {
                path: out.display().to_string(),
                why: e.to_string(),
            })?;
            Ok(true)
        }
        Tool::Same { first, second } => {
            let differ = same(first, second)?;
            for id in &differ {
                println!("{id} differs");
            }
            Ok(differ.is_empty())
        }
    }
}

fn main() -> ExitCode {
    match tool(&Cli::parse().command) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("ssi: {e}");
            ExitCode::from(2)
        }
    }
}
