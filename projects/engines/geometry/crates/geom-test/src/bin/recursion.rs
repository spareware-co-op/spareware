//! `recursion`: list every recursive cycle in the crates that ship, check each has a stated depth
//! bound that is true to the code, and run each bound's test on the small WebAssembly stack.
//!
//! ```text
//! cargo run -p geom-test --bin recursion -- list  [--root <dir>]
//! cargo run -p geom-test --bin recursion -- check [--root <dir>]
//! cargo run -p geom-test --bin recursion -- gate  [--root <dir>] [--runner wasmtime|node]
//! cargo run -p geom-test --bin recursion -- path <function> [--root <dir>]
//! ```
//!
//! Guarantees: `list` prints every cycle (its members, a path round it with the line of each call,
//! and its statement or "no stated bound") and every recursive type, and exits 0 unless the code
//! cannot be read; `check` prints every problem and exits 1 if there is any; `gate` runs `check`,
//! then every bound test on the small stack in debug and release and the probes, and exits 1 if
//! anything failed; `path` prints a shortest path round a cycle holding the named function, each
//! call with how it was found (by path, on the receiver's type, through a bound, through a value,
//! as a callback of outside code, ...), to show why it is recursive. Exit code 2 is a usage error or a workspace that cannot be read. None of them
//! writes anything in the repository; `gate` builds in `target/recursion-stack`.
//!
//! Method: none; it parses its arguments and calls [`geom_test::recursion`], whose module headers
//! carry the method.
//!
//! Cost: one analysis of the workspace ([`geom_test::recursion::analyse_repo`]); for `gate`, the
//! builds and runs of [`geom_test::recursion::stack`].

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use geom_test::map::repo;
use geom_test::recursion::stack::{self, Runner};
use geom_test::recursion::{Problem, Report, Rule, analyse_repo};

const USAGE: &str =
    "usage: recursion <list|check|gate|path <function>> [--root <workspace root>] [--runner wasmtime|node]";

/// What to do.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Command {
    List,
    Check,
    Gate,
    Path,
}

fn main() -> ExitCode {
    let (command, root, runner, function) = match parse(std::env::args().skip(1).collect()) {
        Ok(p) => p,
        Err(message) => {
            eprintln!("recursion: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let root = match root.map_or_else(|| repo::workspace_root().map_err(|e| e.to_string()), Ok) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("recursion: {e}");
            return ExitCode::from(2);
        }
    };
    let report = match analyse_repo(&root) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("recursion: {e}");
            return ExitCode::from(2);
        }
    };
    match command {
        Command::List => {
            list(&report);
            let unread: Vec<&Problem> = report.problems.iter().filter(|p| p.rule == Rule::Unread).collect();
            for p in &unread {
                eprintln!("{p}");
            }
            if unread.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        Command::Check => finish("check", &report, &report.problems),
        Command::Path => path(&report, function.as_deref().unwrap_or_default()),
        Command::Gate => {
            let mut problems = report.problems.clone();
            let target = root.join("target").join("recursion-stack");
            problems.extend(stack::run_bound_tests(&root, &report.bound_tests, runner, &target));
            problems.extend(stack::probes(runner, &target));
            finish("gate", &report, &problems)
        }
    }
}

/// Reads `<list|check|gate|path <function>> [--root <dir>] [--runner wasmtime|node]`.
fn parse(args: Vec<String>) -> Result<(Command, Option<PathBuf>, Runner, Option<String>), String> {
    let mut args = args.into_iter();
    let command = match args.next().as_deref() {
        Some("list") => Command::List,
        Some("check") => Command::Check,
        Some("gate") => Command::Gate,
        Some("path") => Command::Path,
        Some(other) => return Err(format!("unknown command {other:?}")),
        None => return Err("no command".to_owned()),
    };
    let function = if command == Command::Path {
        match args.next() {
            Some(f) => Some(f),
            None => return Err("path needs a function's name".to_owned()),
        }
    } else {
        None
    };
    let mut root = None;
    let mut runner = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" if root.is_none() => match args.next() {
                Some(dir) => root = Some(PathBuf::from(dir)),
                None => return Err("--root needs a folder".to_owned()),
            },
            "--runner" if runner.is_none() && command == Command::Gate => match args.next().as_deref() {
                Some("wasmtime") => runner = Some(Runner::Wasmtime),
                Some("node") => runner = Some(Runner::Node),
                _ => return Err("--runner is wasmtime or node".to_owned()),
            },
            _ => return Err(format!("unexpected argument {arg:?}")),
        }
    }
    Ok((command, root, runner.unwrap_or(Runner::Wasmtime), function))
}

/// Prints a shortest path round a cycle that holds the function named `name` (its full name as
/// listed, or the end of it).
fn path(report: &Report, name: &str) -> ExitCode {
    let found = report.cycles.iter().find(|c| {
        c.members
            .iter()
            .any(|m| m == name || m.ends_with(&format!("::{name}")))
    });
    let Some(cycle) = found else {
        println!("recursion path: `{name}` is in no recursive cycle");
        return ExitCode::SUCCESS;
    };
    println!(
        "recursion path: a cycle holding `{name}` ({}):",
        cycle.members.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", ")
    );
    for (n, site) in &cycle.path {
        println!("    {n}\n        calls on at {}:{}, {}", site.file, site.line, site.how);
    }
    ExitCode::SUCCESS
}

/// Prints every cycle and recursive type.
fn list(report: &Report) {
    println!(
        "recursion list: {} functions read; {} instances; {} edges",
        report.functions, report.instances, report.edges
    );
    for (i, c) in report.cycles.iter().enumerate() {
        let members: Vec<String> = c.members.iter().map(|m| format!("`{m}`")).collect();
        println!("cycle {}: {}", i.saturating_add(1), members.join(", "));
        for (name, site) in &c.path {
            println!("    {name}  -> calls on at {}:{}, {}", site.file, site.line, site.how);
        }
        match report.bounded.get(&i).and_then(|s| report.statements.get(*s)) {
            Some(s) => println!(
                "    bound {} ({}:{}); test `{}`",
                s.bound, s.file, s.line, s.test
            ),
            None => println!("    no stated bound"),
        }
    }
    for (i, t) in report.types.iter().enumerate() {
        let members: Vec<String> = t.members.iter().map(|m| format!("`{m}`")).collect();
        println!("recursive types {}: {}", i.saturating_add(1), members.join(", "));
        match report.typed.get(&i).and_then(|s| report.statements.get(*s)) {
            Some(s) => println!("    bound {} ({}:{}); test `{}`", s.bound, s.file, s.line, s.test),
            None => println!("    no stated bound"),
        }
    }
    println!(
        "recursion list: {} recursive cycle(s), {} group(s) of recursive types",
        report.cycles.len(),
        report.types.len()
    );
}

/// Prints the problems and a summary, and gives the exit code.
fn finish(what: &str, report: &Report, problems: &[Problem]) -> ExitCode {
    for p in problems {
        eprintln!("{p}");
    }
    if problems.is_empty() {
        println!(
            "recursion {what}: {} recursive cycle(s) and {} group(s) of recursive types, each with a stated \
             depth bound{}",
            report.cycles.len(),
            report.types.len(),
            if what == "gate" { " that holds on the small stack" } else { "" }
        );
        return ExitCode::SUCCESS;
    }
    eprintln!(
        "recursion {what}: {} problem(s). Each names the file, the line and the rule; the statement grammar is in \
         projects/engines/geometry/HEADERS.md (the Recursion field).",
        problems.len()
    );
    ExitCode::from(1)
}
