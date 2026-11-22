//! `geom-bench-counts`: the checks the gates run on count files, from the command line. The checks
//! themselves are in `geom_bench::counts`.
//!
//! ```text
//! geom-bench-counts compare <base counts> <head counts> <gate %> <list %> <retired.txt> <title>
//! geom-bench-counts growth <head counts>
//! geom-bench-counts removed <base names> <head names> <retired.txt>
//! geom-bench-counts callgrind <summary folder> <head names> <gate %> <title>
//! geom-bench-counts parts <base|head> <parts folder> <parts>
//! geom-bench-counts times <summary folder>
//! geom-bench-counts plan <times file>...
//! ```
//!
//! `callgrind` judges the instruction-and-cache gate's Callgrind counts from the `summary.json`
//! files Gungraun saved under the folder (`geom_bench::callgrind`): instructions and estimated
//! cycles at the gate's percentage, with every miss count that changed listed and judged through
//! estimated cycles.
//!
//! `parts` checks the instruction gate's parts (`geom_bench::parts::check`): for each part k from
//! 1, `<parts folder>/k/<side>.names` lists the run's benchmarks and `<parts folder>/k/home.list`
//! the part's saved files; every listed benchmark must have been measured in exactly one part. A
//! part's files that are missing (a part that dropped out) are a file that cannot be read. `times`
//! prints each unit's seconds (`<unit> <seconds>`, a group or `group/op` for a split group) from the durations in the summaries, and
//! `plan` prints `parts.tsv` from one or more such files, each unit at the most seconds any of
//! them gives, after their comment lines (`geom_bench::parts::plan`, at `PART_SECONDS`).
//!
//! Each prints a Markdown report on standard output (`removed` prints one name per line) and exits
//! with 0 when the check passes, 1 when it fails, 2 when an argument or file is wrong, so a
//! workflow step fails exactly when the check does.
//!
//! Guarantees: the exit status is the check's verdict, and every check is exact for any count and
//! any whole-number percentage the arguments can hold (`geom_bench::counts`,
//! `geom_bench::callgrind`); a missing, unreadable or malformed file, a benchmark without exactly
//! one summary, or an argument that is not a whole number, exits with 2, never with a pass.
//!
//! Method: `geom_bench::counts`: Gungraun's soft limits
//! (<https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/README.md>)
//! and the doubling test (<https://algs4.cs.princeton.edu/14analysis/>).
//! Consulted: Gungraun, Apache-2.0 OR MIT,
//!   <https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/README.md>,
//!   for its library-benchmark API and its Callgrind and DHAT settings. No code copied.
//!
//! Cost: reading and parsing the files, then O(n log n) for n benchmarks.

use std::fmt::Write as _;
use std::io::Write as _;
use std::process::ExitCode;

#[cfg(all(feature = "judge", any(unix, windows)))]
use geom_bench::callgrind::{judge, read_summaries};
use geom_bench::counts::{Counts, compare, growth, growth_report, parse_names, unretired_removals};
#[cfg(all(feature = "judge", any(unix, windows)))]
use geom_bench::parts::{PART_SECONDS, Side, check, group_seconds, measured, plan, read_durations};

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
        #[cfg(all(feature = "judge", any(unix, windows)))]
        ["callgrind", folder, names, gate, title] => {
            let gate = percent(gate)?;
            let names = parse_names(&read(names)?);
            let judgement = judge(&read_summaries(std::path::Path::new(folder))?, &names, gate)?;
            out.push_str(&judgement.report(title));
            Ok(judgement.passed())
        }
        #[cfg(all(feature = "judge", any(unix, windows)))]
        ["parts", side, folder, parts] => {
            let (side, file) = match *side {
                "base" => (Side::Base, "base.names"),
                "head" => (Side::Head, "head.names"),
                _ => return Err(format!("parts: not base or head: {side}")),
            };
            let parts: usize = parts
                .parse()
                .ok()
                .filter(|&n| n > 0)
                .ok_or_else(|| format!("parts: not a number of parts: {parts}"))?;
            let (mut listed, mut done) = (Vec::new(), Vec::new());
            for k in 1..=parts {
                listed.push(parse_names(&read(&format!("{folder}/{k}/{file}"))?));
                let listing = read(&format!("{folder}/{k}/home.list"))?;
                done.push(measured(&listing, side).map_err(|e| format!("part {k}: {e}"))?);
            }
            let found = check(&listed, &done);
            let _ = writeln!(out, "#### The {} parts
", if side == Side::Base { "merge base's" } else { "change's" });
            if found.is_empty() {
                for (k, names) in done.iter().enumerate() {
                    let _ = writeln!(out, "- Part {}: {} benchmarks.", k + 1, names.len());
                }
                let _ = writeln!(out, "
Every benchmark was measured in exactly one part.");
            } else {
                for problem in &found {
                    let _ = writeln!(out, "- {problem}");
                }
            }
            Ok(found.is_empty())
        }
        #[cfg(all(feature = "judge", any(unix, windows)))]
        ["times", folder] => {
            let seconds = group_seconds(&read_durations(std::path::Path::new(folder))?);
            for (group, s) in seconds {
                let _ = writeln!(out, "{group} {s}");
            }
            Ok(true)
        }
        #[cfg(all(feature = "judge", any(unix, windows)))]
        ["plan", files @ ..] if !files.is_empty() => {
            let mut seconds = std::collections::BTreeMap::new();
            let mut header = vec![
                "The instruction gate's parts (kernel-bench.yml): each unit's part and seconds (a group, or an operation of mesh)."
                    .to_owned(),
                "Written by `geom-bench-counts plan` from the times of a run, never by hand."
                    .to_owned(),
            ];
            for file in files {
                for line in read(file)?.lines() {
                    if let Some(comment) = line.strip_prefix('#') {
                        header.push(comment.trim().to_owned());
                        continue;
                    }
                    let mut fields = line.split(' ');
                    let (Some(group), Some(s), None) = (fields.next(), fields.next(), fields.next())
                    else {
                        return Err(format!("{file}: not `<group> <seconds>`: {line:?}"));
                    };
                    let s: u64 = s.parse().map_err(|_| format!("{file}: not seconds: {line:?}"))?;
                    let most = seconds.entry(group.to_owned()).or_insert(0);
                    *most = s.max(*most);
                }
            }
            out.push_str(&plan(&seconds, PART_SECONDS).text(&header));
            Ok(true)
        }
        #[cfg(not(all(feature = "judge", any(unix, windows))))]
        ["callgrind" | "parts" | "times" | "plan", ..] => Err(
            "callgrind needs the `judge` feature, on Unix or Windows: Gungraun's summary model is \
             not built here"
                .to_owned(),
        ),
        _ => Err(
            "usage: geom-bench-counts compare <base> <head> <gate %> <list %> <retired> \
                  <title> | growth <counts> | removed <base names> <head names> <retired> | \
                  callgrind <summary folder> <head names> <gate %> <title> | parts <base|head>                   <parts folder> <parts> | times <summary folder> | plan <times file>..."
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
