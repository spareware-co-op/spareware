//! `geom-bench-guest`: the program the WebAssembly fuel gate runs inside Wasmtime, built for
//! `wasm32-wasip1`. It also runs natively. Everything it does is in `geom_bench::guest`, whose
//! documentation gives its commands, its exit status and how the fuel gate uses them; this program
//! only passes on its arguments, output and status, except for one command of its own, `time`.
//!
//! What it does: besides `geom_bench::guest`'s commands,
//!
//! - `time <name> <milliseconds>`: wall-clock time per input of the benchmark `name`, for chapter
//!   23.3's comparison of exact-path variants on every engine (piece G42), which the variant
//!   workflow runs natively, in Wasmtime, in Node and in browsers. It prints one line,
//!   `<name> <nanoseconds per input> <passes per round>`: six rounds, each as many passes over the
//!   batch as fit `milliseconds`, the first a warm-up, and the median of the other five. A round of
//!   many passes is one interval of the clock, so a browser's coarse timer (Firefox rounds to a
//!   millisecond) costs at most a millisecond per round. Timing lives in this program, not in the
//!   library, whose results never depend on a clock.
//!
//! Guarantees: for every other command, those of `geom_bench::guest`, unchanged: the exit status
//! and output are exactly the ones it returns. `time` exits 0 with its line, or 2 for an unknown
//! benchmark or a duration that is not a number from 1 to 60,000.
//!
//! Method: fuel metering (`Config::consume_fuel`,
//! <https://github.com/bytecodealliance/wasmtime/blob/5afd63588b1f5520391c6a37fc38dfd3ccd607c2/crates/wasmtime/src/config.rs#L627-L646>)
//! by differencing two runs (see `geom_bench::guest`); for `time`, the median of repeated
//! fixed-duration rounds after a warm-up round, as wall-clock benchmark harnesses take it
//! (divan's sample-and-median design, <https://docs.rs/divan/0.1.21/divan/>).
//! Consulted: Wasmtime, Apache-2.0 WITH LLVM-exception,
//!   <https://github.com/bytecodealliance/wasmtime/blob/5afd63588b1f5520391c6a37fc38dfd3ccd607c2/crates/wasmtime/src/config.rs#L627-L646>,
//!   for what fuel counts and how `Config::consume_fuel` meters it. No code copied.
//!
//! Cost: that of the command it runs (`geom_bench::guest`); `time` runs for about six times its
//! duration.

use std::hint::black_box;
use std::io::Write as _;
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// Rounds of `time`, the first a warm-up.
const ROUNDS: usize = 6;

/// `time <name> <milliseconds>`: the line it prints, or why it cannot.
fn time(name: &str, milliseconds: &str) -> Result<String, String> {
    let ms: u64 = milliseconds
        .parse()
        .ok()
        .filter(|ms| (1..=60_000).contains(ms))
        .ok_or_else(|| {
            format!("the duration must be 1 to 60000 milliseconds, not {milliseconds}")
        })?;
    let mut bench = geom_bench::catalogue::prepare_named(name)
        .ok_or_else(|| format!("no benchmark named {name}"))?;
    let inputs = u64::try_from(bench.len()).unwrap_or(1).max(1);
    let budget = Duration::from_millis(ms);
    let mut per_input: Vec<f64> = Vec::with_capacity(ROUNDS);
    let mut passes = 0u64;
    for _ in 0..ROUNDS {
        let start = Instant::now();
        passes = 0;
        while start.elapsed() < budget {
            bench.pass();
            passes = passes.saturating_add(1);
        }
        let elapsed = start.elapsed();
        black_box(&bench);
        // Nanoseconds per input: the round's time over its passes' inputs. f64 holds both counts
        // exactly at these sizes (below 2^53), and the quotient is a statistic, never a result.
        #[allow(
            clippy::cast_precision_loss,
            reason = "timings below 2^53 nanoseconds and counts"
        )]
        let ns = elapsed.as_nanos() as f64 / (passes.saturating_mul(inputs).max(1)) as f64;
        per_input.push(ns);
    }
    let mut measured: Vec<f64> = per_input.into_iter().skip(1).collect();
    measured.sort_by(f64::total_cmp);
    let median = measured.get(measured.len() / 2).copied().unwrap_or(0.0);
    Ok(format!("{name} {median:.3} {passes}\n"))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let outcome = match args.as_slice() {
        ["time", name, ms] => match time(name, ms) {
            Ok(line) => geom_bench::guest::Outcome {
                stdout: line,
                ..geom_bench::guest::Outcome::default()
            },
            Err(message) => geom_bench::guest::Outcome {
                status: 2,
                stderr: format!("{message}\nusage: geom-bench-guest time <name> <milliseconds>\n"),
                ..geom_bench::guest::Outcome::default()
            },
        },
        _ => geom_bench::guest::main(&args),
    };
    // Nothing useful can be done if standard output or error is closed, so failures are ignored.
    let _ = std::io::stdout().write_all(outcome.stdout.as_bytes());
    let _ = std::io::stderr().write_all(outcome.stderr.as_bytes());
    ExitCode::from(outcome.status)
}
