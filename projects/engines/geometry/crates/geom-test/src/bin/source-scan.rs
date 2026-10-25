//! `source-scan`: the banned-source scan of the geometry kernel's crates and the facade's.
//!
//! ```text
//! cargo run -p geom-test --bin source-scan -- check [--root <dir>]   exit 1 on any finding
//! ```
//!
//! Guarantees: it writes nothing. It prints every finding, one per line as
//! `<file>:<line>: <category>: "<marker>": <what the rule is and what to do>` (or
//! `<file>: Unreadable: ...`, `<file>: Stale exemption: ...`), and a summary. Exit codes: 0 when
//! nothing is found; 1 when anything is; 2 for a usage error or a folder or file that cannot be
//! read. `--root` names the workspace root to scan; by default it is the workspace this binary was
//! built in.
//!
//! Method: none; it parses its arguments and calls [`geom_test::scan::repo`], whose module headers
//! carry the method.
//!
//! Cost: one read of every scanned file ([`geom_test::scan::repo::check`]).

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use geom_test::map::repo::workspace_root;
use geom_test::scan::repo::{EXEMPT, check};

const USAGE: &str = "usage: source-scan check [--root <workspace root>]";

fn main() -> ExitCode {
    let root = match parse(std::env::args_os().skip(1).collect()) {
        Ok(root) => root,
        Err(message) => {
            eprintln!("source-scan: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let root = match root.map_or_else(workspace_root, Ok) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("source-scan: {e}");
            return ExitCode::from(2);
        }
    };
    let findings = match check(&root, EXEMPT) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("source-scan check: {e}");
            return ExitCode::from(2);
        }
    };
    for f in &findings {
        eprintln!("{f}");
    }
    if findings.is_empty() {
        println!(
            "source-scan check: no banned-source marker in the kernel's crates ({} exempt file(s))",
            EXEMPT.len()
        );
        return ExitCode::SUCCESS;
    }
    eprintln!(
        "source-scan check: {} finding(s). Each names the file, the line, what was found and what \
         to do (chapter 29.20, rules 2 and 7).",
        findings.len()
    );
    ExitCode::from(1)
}

/// Reads `check [--root <dir>]`.
fn parse(args: Vec<OsString>) -> Result<Option<PathBuf>, String> {
    let mut args = args.into_iter();
    match args.next().as_ref().and_then(|a| a.to_str()) {
        Some("check") => {}
        Some(other) => return Err(format!("unknown command {other:?}")),
        None => return Err("no command".to_owned()),
    }
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
    Ok(root)
}
