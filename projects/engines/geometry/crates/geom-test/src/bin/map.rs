//! `map`: write or check `projects/engines/geometry/MAP.json`, the generated map of the geometry
//! kernel's crates, modules, invariants, sources and error kinds.
//!
//! ```text
//! cargo run -p geom-test --bin map -- write [--root <dir>]   write the map if it changed
//! cargo run -p geom-test --bin map -- check [--root <dir>]   exit 1 on any problem or difference
//! ```
//!
//! Guarantees: `check` writes nothing; `write` writes only `MAP.json`, and only when its bytes
//! change. Both print every problem, one per line as `<file>: <field>: <what to write>`, and a
//! summary. Exit codes: 0 when every header is complete and the committed map is current (for
//! `write`, when every header is complete); 1 when any header problem, stale gap or difference is
//! found; 2 for a usage error or a file that cannot be read or written. `--root` names the
//! workspace root to work in; by default it is the workspace this binary was built in.
//!
//! Method: none; it parses its arguments and calls [`geom_test::map::repo`], whose module headers
//! carry the method.
//!
//! Cost: one build of the map ([`geom_test::map::build`]) and one read or write of `MAP.json`.

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use geom_test::map::{GAPS, GRAMMAR_FILE, Problem, WRITE_COMMAND, repo};

const USAGE: &str = "usage: map <write|check> [--root <workspace root>]";

/// The command to run.
enum Command {
    Write,
    Check,
}

fn main() -> ExitCode {
    let (command, root) = match parse(std::env::args_os().skip(1).collect()) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("map: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let root = match root.map_or_else(repo::workspace_root, Ok) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("map: {e}");
            return ExitCode::from(2);
        }
    };
    let (what, result) = match command {
        Command::Check => ("check", repo::check(&root, GAPS).map(|p| (p, false))),
        Command::Write => ("write", repo::write(&root, GAPS)),
    };
    match result {
        Ok((problems, wrote)) => report(what, &problems, wrote),
        Err(e) => {
            eprintln!("map {what}: {e}");
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
fn report(what: &str, problems: &[Problem], wrote: bool) -> ExitCode {
    if what == "write" {
        println!(
            "map write: {}",
            if wrote {
                "wrote projects/engines/geometry/MAP.json"
            } else {
                "projects/engines/geometry/MAP.json was already current"
            }
        );
    }
    for p in problems {
        eprintln!("{p}");
    }
    if problems.is_empty() {
        println!("map {what}: every crate and module header is complete, and the map is current");
        return ExitCode::SUCCESS;
    }
    eprintln!(
        "map {what}: {} problem(s). Each names the file and the field of the grammar in {GRAMMAR_FILE}; \
         fix the headers, then run `{WRITE_COMMAND}` and commit MAP.json.",
        problems.len()
    );
    ExitCode::from(1)
}
