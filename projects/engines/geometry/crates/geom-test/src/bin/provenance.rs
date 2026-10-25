//! `provenance`: write or check `projects/engines/geometry/PROVENANCE.md` and
//! `projects/engines/geometry/NOTICE`, the kernel's source ledger and notices, generated from the
//! map's provenance records, the licence texts under `notices/` and the method notes under
//! `methods/`.
//!
//! ```text
//! cargo run -p geom-test --bin provenance -- write [--root <dir>]   write each file that changed
//! cargo run -p geom-test --bin provenance -- check [--root <dir>]   exit 1 on any problem or difference
//! ```
//!
//! Guarantees: `check` writes nothing; `write` writes only `PROVENANCE.md` and `NOTICE`, and each
//! only when its bytes change. Both print every problem, one per line as
//! `<file>: <rule>: <what to do>`, and a summary. Exit codes: 0 when there is no problem and (for
//! `check`) both committed files are current; 1 when any problem or difference is found; 2 for a
//! usage error or a file that cannot be read or written. `--root` names the workspace root to work
//! in; by default it is the workspace this binary was built in.
//!
//! Method: none; it parses its arguments and calls [`geom_test::ledger::repo`], whose module
//! headers carry the method.
//!
//! Cost: one build of the map and the ledger ([`geom_test::ledger::build`]) and one read or write of
//! each generated file.

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use geom_test::ledger::{NOTICE_FILE, PROVENANCE_FILE, Problem, WRITE_COMMAND, repo};
use geom_test::map::repo::workspace_root;

const USAGE: &str = "usage: provenance <write|check> [--root <workspace root>]";

/// The command to run.
enum Command {
    Write,
    Check,
}

fn main() -> ExitCode {
    let (command, root) = match parse(std::env::args_os().skip(1).collect()) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("provenance: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let root = match root.map_or_else(workspace_root, Ok) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("provenance: {e}");
            return ExitCode::from(2);
        }
    };
    let (what, result) = match command {
        Command::Check => ("check", repo::check(&root).map(|p| (p, Vec::new()))),
        Command::Write => ("write", repo::write(&root)),
    };
    match result {
        Ok((problems, written)) => report(what, &problems, &written),
        Err(e) => {
            eprintln!("provenance {what}: {e}");
            ExitCode::from(2)
        }
    }
}

/// Reads `<write|check> [--root <dir>]`.
fn parse(args: Vec<OsString>) -> Result<(Command, Option<PathBuf>), String> {
    let mut args = args.into_iter();
    let command = match args.next().as_ref().and_then(|a| a.to_str()) {
        Some("write") => Command::Write,
        Some("check") => Command::Check,
        Some(other) => return Err(format!("unknown command {other:?}")),
        None => return Err("no command".to_owned()),
    };
    let mut root = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--root") if root.is_none() => match args.next() {
                Some(dir) => root = Some(PathBuf::from(dir)),
                None => return Err("--root needs a folder".to_owned()),
            },
            _ => return Err(format!("unexpected argument {arg:?}")),
        }
    }
    Ok((command, root))
}

/// Prints every problem and a summary, and gives the exit code.
fn report(what: &str, problems: &[Problem], written: &[&str]) -> ExitCode {
    if what == "write" {
        if written.is_empty() {
            println!("provenance write: {PROVENANCE_FILE} and {NOTICE_FILE} were already current");
        }
        for file in written {
            println!("provenance write: wrote {file}");
        }
    }
    for p in problems {
        eprintln!("{p}");
    }
    if problems.is_empty() {
        println!("provenance {what}: every record is in the ledger, and both files are current");
        return ExitCode::SUCCESS;
    }
    eprintln!(
        "provenance {what}: {} problem(s). Fix each, then run `{WRITE_COMMAND}` and commit \
         {PROVENANCE_FILE} and {NOTICE_FILE}.",
        problems.len()
    );
    ExitCode::from(1)
}
