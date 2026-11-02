//! `geom-bench-timings`: the reference runner's tool for its nightly records (piece G107; chapter
//! 23.13, "Wall clock and scaling"). Everything it computes is in `geom_bench::timings`; this
//! program reads files, passes them on and writes what comes back.
//!
//! What it does:
//!
//! - `assemble <header> <busy> <jobs> <names>`: prints the night's record, built from the header
//!   file (a record's lines up to the engines), the machine's busy samples and the runner's job
//!   lines, against the benchmarks the names file lists, one per line, each a benchmark of this
//!   build's catalogue: every benchmark for a night, a few for a test of the runner
//!   (`geom_bench::timings::assemble`).
//! - `check <record>...`: reads each record and prints how many timings and failures it holds.
//! - `report <history folder>`: reads every `*.txt` record in the folder, in the order of their
//!   names (each named for its night, `YYYY-MM-DD.txt`, so oldest first), and prints the report of
//!   the last night against the night before (`geom_bench::timings::report`).
//!
//! Guarantees: exit status 0 with the output, 1 when a file cannot be read or a record, sample or
//! job line is refused (the file and the line on standard error), 2 for a usage error, and 2 on a
//! platform other than Unix and Windows, where the records do not exist. Nothing is written but
//! standard output and standard error.
//!
//! Method: none; it reads its arguments and files and passes them to `geom_bench::timings`.
//!
//! Cost: that of the command (`geom_bench::timings`), plus reading its files.

use std::process::ExitCode;

#[cfg(any(unix, windows))]
use std::io::Write as _;
#[cfg(any(unix, windows))]
use std::path::Path;

#[cfg(any(unix, windows))]
use geom_bench::catalogue::names;
#[cfg(any(unix, windows))]
use geom_bench::timings::{Record, assemble, report};

#[cfg(any(unix, windows))]
/// What to print and how to end.
struct Outcome {
    status: u8,
    stdout: String,
    stderr: String,
}

#[cfg(any(unix, windows))]
impl Outcome {
    /// A success that prints `stdout`.
    fn print(stdout: String) -> Outcome {
        Outcome {
            status: 0,
            stdout,
            stderr: String::new(),
        }
    }

    /// A failure that prints `stderr`.
    fn fail(stderr: String) -> Outcome {
        Outcome {
            status: 1,
            stdout: String::new(),
            stderr: stderr + "\n",
        }
    }
}

#[cfg(any(unix, windows))]
/// The usage message.
const USAGE: &str = "usage: geom-bench-timings assemble <header> <busy> <jobs> <names> \
                     | check <record>... | report <history folder>\n";

#[cfg(any(unix, windows))]
/// Reads `path` as text, or says why it cannot.
fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))
}

#[cfg(any(unix, windows))]
/// `assemble <header> <busy> <jobs> <names>`.
fn assemble_files(header: &str, busy: &str, jobs: &str, listed: &str) -> Result<String, String> {
    let header_text = read(header)?;
    let busy_text = read(busy)?;
    let jobs_text = read(jobs)?;
    let catalogue = names();
    let mut timed: Vec<String> = Vec::new();
    for name in read(listed)?.lines().filter(|l| !l.is_empty()) {
        if !catalogue.iter().any(|known| known == name) {
            return Err(format!(
                "{listed}: {name} is not a benchmark of the catalogue"
            ));
        }
        if timed.iter().any(|t| t == name) {
            return Err(format!("{listed}: {name} is listed twice"));
        }
        timed.push(name.to_string());
    }
    assemble(&header_text, &busy_text, &jobs_text, &timed)
        .map(|record| record.to_string())
        .map_err(|refused| format!("{header}, {busy}, {jobs}: {refused}"))
}

#[cfg(any(unix, windows))]
/// `check <record>...`.
fn check_files(records: &[&str]) -> Result<String, String> {
    let mut out = String::new();
    for path in records {
        let record = Record::parse(&read(path)?).map_err(|refused| format!("{path}: {refused}"))?;
        out.push_str(&format!(
            "{path}: night {}, {} timings, {} failures\n",
            record.night,
            record.timings.len(),
            record.failures.len()
        ));
    }
    Ok(out)
}

#[cfg(any(unix, windows))]
/// `report <history folder>`.
fn report_folder(folder: &str) -> Result<String, String> {
    let entries = std::fs::read_dir(folder).map_err(|error| format!("{folder}: {error}"))?;
    let mut paths: Vec<String> = Vec::new();
    for entry in entries {
        let path = entry.map_err(|error| format!("{folder}: {error}"))?.path();
        if path.extension().is_some_and(|e| e == "txt") {
            paths.push(path.to_string_lossy().into_owned());
        }
    }
    paths.sort_by(|a, b| Path::new(a).file_name().cmp(&Path::new(b).file_name()));
    let mut history = Vec::with_capacity(paths.len());
    for path in &paths {
        history.push(Record::parse(&read(path)?).map_err(|refused| format!("{path}: {refused}"))?);
    }
    Ok(report(&history))
}

#[cfg(any(unix, windows))]
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["assemble", header, busy, jobs, listed] => {
            Some(assemble_files(header, busy, jobs, listed))
        }
        ["check", records @ ..] if !records.is_empty() => Some(check_files(records)),
        ["report", folder] => Some(report_folder(folder)),
        _ => None,
    };
    let outcome = match result {
        Some(Ok(text)) => Outcome::print(text),
        Some(Err(message)) => Outcome::fail(message),
        None => Outcome {
            status: 2,
            stdout: String::new(),
            stderr: USAGE.to_string(),
        },
    };
    // Nothing useful can be done if standard output or error is closed, so failures are ignored.
    let _ = std::io::stdout().write_all(outcome.stdout.as_bytes());
    let _ = std::io::stderr().write_all(outcome.stderr.as_bytes());
    ExitCode::from(outcome.status)
}

/// The records and the report exist on Unix and Windows only (geom-bench's Cargo.toml).
#[cfg(not(any(unix, windows)))]
fn main() -> ExitCode {
    ExitCode::from(2)
}
