//! `geom-gen`: write, check or list every committed generator output.
//!
//! ```text
//! cargo run -p geom-gen -- write [--root <dir>]   write every output whose bytes changed
//! cargo run -p geom-gen -- check [--root <dir>]   exit 1, naming each file, if any output differs
//! cargo run -p geom-gen -- list                   list the generators and generated folders
//! ```
//!
//! Every command prints each generator's seed (`seed: <hex>`, or `seed: none` for a generator that
//! uses no randomness) before it runs any generator, so the seeds are printed even when a generator
//! fails. `--root` names the workspace root to work in; by default it is the workspace this binary
//! was built in. Exit codes: 0 success; 1 an output differs or is missing, or a generated folder
//! holds an orphan (`write` too reports orphans, and never deletes them); 2 a usage, registry,
//! generator or file error.
#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use geom_gen::files::{self, Outcome};
use geom_gen::registry::{REGISTRY, workspace_root};

const USAGE: &str = "usage: geom-gen <write|check|list> [--root <workspace root>]";

/// The command to run.
enum Command {
    Write,
    Check,
    List,
}

fn main() -> ExitCode {
    let (command, root) = match parse(std::env::args_os().skip(1).collect()) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("geom-gen: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    // Every generator's seed, before anything can fail.
    for g in REGISTRY.generators {
        println!("{}", g.seed_line());
    }
    let root = match root {
        Some(r) => r,
        None => match workspace_root() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("geom-gen: {e}");
                return ExitCode::from(2);
            }
        },
    };
    match command {
        Command::List => {
            for g in REGISTRY.generators {
                println!("{}: owns {}: {}", g.name, g.output, g.about);
            }
            for f in REGISTRY.folders {
                println!("generated folder: {} (also kept: {:?})", f.path, f.keep);
            }
            ExitCode::SUCCESS
        }
        Command::Write => report("write", files::write(&root, &REGISTRY), &root),
        Command::Check => report("check", files::check(&root, &REGISTRY), &root),
    }
}

/// Reads `<write|check|list> [--root <dir>]`.
fn parse(args: Vec<OsString>) -> Result<(Command, Option<PathBuf>), String> {
    let mut args = args.into_iter();
    let command = match args.next().as_ref().and_then(|a| a.to_str()) {
        Some("write") => Command::Write,
        Some("check") => Command::Check,
        Some("list") => Command::List,
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

/// Prints every outcome and a summary, and gives the exit code.
fn report(what: &str, result: Result<Vec<Outcome>, geom_gen::GenError>, root: &Path) -> ExitCode {
    let outcomes = match result {
        Ok(o) => o,
        Err(e) => {
            eprintln!("geom-gen {what}: {e}");
            return ExitCode::from(2);
        }
    };
    println!(
        "geom-gen {what}: {} generator(s), workspace root {}",
        REGISTRY.generators.len(),
        root.display()
    );
    for o in &outcomes {
        println!("{}", o.describe());
    }
    let stale: Vec<&Outcome> = outcomes.iter().filter(|o| !o.is_current()).collect();
    if stale.is_empty() {
        println!(
            "geom-gen {what}: every output is current, and no generated folder holds an orphan"
        );
        return ExitCode::SUCCESS;
    }
    for o in &stale {
        eprintln!("geom-gen {what}: {}", o.describe());
    }
    eprintln!(
        "geom-gen {what}: {} file(s) differ from their generators, are missing, or are orphans. \
         Run `cargo run -p geom-gen -- write` and commit the result, fix the generator, or delete \
         the orphans.",
        stale.len()
    );
    ExitCode::from(1)
}
