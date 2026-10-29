//! `trip`: proves a gate that runs an outside tool fails on a planted violation (gate 1, piece
//! G43).
//!
//! ```text
//! cargo run --locked -p geom-test --bin trip -- <violation> -- <the gate's command ...>
//! ```
//!
//! What it does: finds the violation `<violation>` in [`geom_test::trip::VIOLATIONS`], plants it
//! in the working tree ([`geom_test::trip::apply`]: one line of one kernel file changed), runs the
//! gate's command from the repository root, prints its output, restores the file, and prints
//! [`geom_test::trip::verdict`]'s answer: the command must fail, printing the tool's own report
//! of the violation.
//!
//! Guarantees: the file is restored byte for byte after the run, whether the run passed, failed
//! or could not start; `RUSTUP_TOOLCHAIN`, which a `cargo run` passes down, is removed from the
//! command's environment, so the command picks its toolchain as the job would (Verus names its
//! own). Exit codes: 0 when the gate tripped as it must; 1 when it did not (the run passed, or
//! failed for another reason); 2 for a usage error or a step that cannot be done.
//!
//! Method: none; it runs the command and calls [`geom_test::trip`], whose module header carries
//! the method.
//!
//! Cost: one read and two writes of the file, and one run of the command.

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fs;
use std::process::{Command, ExitCode};

use geom_test::map::repo::workspace_root;
use geom_test::trip::{VIOLATIONS, apply, verdict, violation};

fn usage(message: &str) -> ExitCode {
    let names: Vec<&str> = VIOLATIONS.iter().map(|v| v.name).collect();
    eprintln!(
        "trip: {message}\nusage: trip <violation> -- <command ...>\nviolations: {}",
        names.join(", ")
    );
    ExitCode::from(2)
}

fn fail(message: &str) -> ExitCode {
    eprintln!("trip: {message}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let [name, separator, program, rest @ ..] = args.as_slice() else {
        return usage("too few arguments");
    };
    if separator != "--" {
        return usage("the command follows `--`");
    }
    let Some(v) = name.to_str().and_then(violation) else {
        return usage(&format!("no violation {name:?}"));
    };
    let root = match workspace_root() {
        Ok(root) => root,
        Err(e) => return fail(&e.to_string()),
    };
    let file = root.join(v.file);
    let original = match fs::read(&file) {
        Ok(bytes) => bytes,
        Err(e) => return fail(&format!("cannot read {}: {e}", file.display())),
    };
    let Ok(text) = String::from_utf8(original.clone()) else {
        return fail(&format!("{} is not UTF-8", file.display()));
    };
    let planted = match apply(v, &text) {
        Ok(planted) => planted,
        Err(e) => return fail(&format!("{}: {e}", v.file)),
    };
    if let Err(e) = fs::write(&file, planted) {
        return fail(&format!("cannot write {}: {e}", file.display()));
    }
    println!(
        "trip: planted `{}` in {} for {}; running {program:?} {rest:?}",
        v.planted, v.file, v.gate
    );
    let run = Command::new(program)
        .args(rest)
        .current_dir(&root)
        .env_remove("RUSTUP_TOOLCHAIN")
        .output();
    let restored = fs::write(&file, &original);
    let out = match run {
        Ok(out) => out,
        Err(e) => return fail(&format!("cannot run {program:?}: {e}")),
    };
    if let Err(e) = restored {
        return fail(&format!("cannot restore {}: {e}", file.display()));
    }
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    println!("{output}");
    match verdict(v, out.status.success(), &output) {
        Ok(()) => {
            println!(
                "trip: {} caught the planted violation in {}: the gate trips",
                v.gate, v.file
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("trip: {}: {e}", v.gate);
            ExitCode::from(1)
        }
    }
}
