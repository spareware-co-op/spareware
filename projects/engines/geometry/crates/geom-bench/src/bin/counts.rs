//! `geom-bench-counts`: the checks the gates run on count files, from the command line. The checks
//! themselves are in `geom_bench::counts`.
//!
//! ```text
//! geom-bench-counts compare <base counts> <head counts> <gate %> <list %> <retired.txt> <title>
//! geom-bench-counts growth <head counts>
//! geom-bench-counts removed <base names> <head names> <retired.txt>
//! ```
//!
//! Each prints a Markdown report on standard output (`removed` prints one name per line) and exits
//! with 0 when the check passes, 1 when it fails, 2 when an argument or file is wrong, so a
//! workflow step fails exactly when the check does.
//!
//! Guarantees: the exit status is the check's verdict, and every check is exact for any count and
//! any whole-number percentage the arguments can hold (`geom_bench::counts`); a missing,
//! unreadable or malformed file, or an argument that is not a whole number, exits with 2, never
//! with a pass.
//!
//! Method: `geom_bench::counts`: Gungraun's soft limits
//! (<https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/README.md>)
//! and the doubling test (<https://algs4.cs.princeton.edu/14analysis/>).
//!
//! Cost: reading the files, then O(n log n) for n benchmarks.

use std::io::Write as _;
use std::process::ExitCode;

use geom_bench::counts::{Counts, compare, growth, growth_report, parse_names, unretired_removals};

/// A file's text, or a message naming it.
fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))
}

/// A count file, or a message naming it.
fn counts(path: &str) -> Result<Counts, String> {
    Counts::parse(&read(path)?).map_err(|e| format!("{path}: {e}"))
}

/// A whole-number percentage, or a message.
fn percent(text: &str) -> Result<u64, String> {
    text.parse()
        .map_err(|_| format!("not a whole-number percentage: {text}"))
}

/// Runs a command; `Ok(true)` if its check passed.
fn run(args: &[&str], out: &mut String) -> Result<bool, String> {
    match args {
        ["compare", base, head, gate, list, retired, title] => {
            let (gate, list) = (percent(gate)?, percent(list)?);
            let retired = parse_names(&read(retired)?);
            let result = compare(&counts(base)?, &counts(head)?, gate, list, &retired);
            out.push_str(&result.report(title, gate, list));
            Ok(result.passed())
        }
        ["growth", head] => {
            let checks = growth(&counts(head)?);
            out.push_str(&growth_report(&checks));
            Ok(checks.iter().all(|c| c.passed()))
        }
        ["removed", base, head, retired] => {
            let removed = unretired_removals(
                &parse_names(&read(base)?),
                &parse_names(&read(head)?),
                &parse_names(&read(retired)?),
            );
            for name in &removed {
                out.push_str(name);
                out.push('\n');
            }
            Ok(removed.is_empty())
        }
        _ => Err(
            "usage: geom-bench-counts compare <base> <head> <gate %> <list %> <retired> \
                  <title> | growth <counts> | removed <base names> <head names> <retired>"
                .to_owned(),
        ),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut out = String::new();
    let status = match run(&args, &mut out) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(message) => {
            let _ = writeln!(std::io::stderr(), "geom-bench-counts: {message}");
            2
        }
    };
    let _ = std::io::stdout().write_all(out.as_bytes());
    ExitCode::from(status)
}
