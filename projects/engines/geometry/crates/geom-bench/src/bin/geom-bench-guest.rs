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
//! - `time <name> <milliseconds> <threads>`: the reference runner's form (piece G107; chapter
//!   23.13, "Wall clock and scaling"), at 1 to 64 threads. Each thread builds its own batch and
//!   checks it against `golden.txt` (one pass and its digests), so nothing is timed that computes
//!   the wrong thing; once every thread is ready they start together, and each times its own batch
//!   with the rounds above. The numbers layer has no pool and starts no thread of its own, so N
//!   threads are N copies of the benchmark running at once, one per thread, as N Web Workers are
//!   in a browser; a WebAssembly build always times one thread (the browser runner starts one
//!   guest per worker). It prints one line,
//!   `<name> <nanoseconds per input> <passes per round> <threads> <slowest thread's nanoseconds>`:
//!   the upper median of the threads' medians, the fewest passes any thread's last round held, and
//!   the slowest thread's median.
//!
//! Guarantees: for every other command, those of `geom_bench::guest`, unchanged: the exit status
//! and output are exactly the ones it returns. `time` exits 0 with its line, or 2 for an unknown
//! benchmark or a duration that is not a number from 1 to 60,000; with a thread count, also 2 for
//! a count that is not 1 to 64 (or not 1 on WebAssembly) or a thread that cannot start, and 1 when
//! a batch's digests differ from `golden.txt`, before anything is timed.
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
//! duration, after building one batch per thread.

use std::hint::black_box;
use std::io::Write as _;
use std::process::ExitCode;
use std::sync::{Condvar, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use geom_bench::harness::Bench;

/// Rounds of `time`, the first a warm-up.
const ROUNDS: usize = 6;

/// `time <name> <milliseconds>`: the line it prints, or why it cannot.
fn time(name: &str, milliseconds: &str) -> Result<String, String> {
    let budget = round_budget(milliseconds)?;
    let mut bench = geom_bench::catalogue::prepare_named(name)
        .ok_or_else(|| format!("no benchmark named {name}"))?;
    let (median, passes) = rounds(&mut *bench, budget);
    Ok(format!("{name} {median:.3} {passes}\n"))
}

/// The duration of one round, from its argument in milliseconds (1 to 60,000).
fn round_budget(milliseconds: &str) -> Result<Duration, String> {
    let ms: u64 = milliseconds
        .parse()
        .ok()
        .filter(|ms| (1..=60_000).contains(ms))
        .ok_or_else(|| {
            format!("the duration must be 1 to 60000 milliseconds, not {milliseconds}")
        })?;
    Ok(Duration::from_millis(ms))
}

/// [`ROUNDS`] rounds of as many passes over `bench`'s batch as fit `budget` each: the median
/// nanoseconds per input of all but the first, and the passes the last round held.
fn rounds(bench: &mut dyn Bench, budget: Duration) -> (f64, u64) {
    let inputs = u64::try_from(bench.len()).unwrap_or(1).max(1);
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
        black_box(&*bench);
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
    (median, passes)
}

/// Most threads `time` starts.
const MAX_THREADS: usize = 64;

/// Why `time` with a thread count printed no line: the exit status and the message.
struct Refusal {
    /// 1 for digests that differ from `golden.txt`, 2 for a usage error or a thread that could
    /// not start.
    status: u8,
    /// What to print on standard error.
    message: String,
}

impl Refusal {
    /// A usage error.
    fn usage(message: String) -> Refusal {
        Refusal { status: 2, message }
    }
}

/// Where the threads of `time` stand: how many have built and checked their batch, the first
/// check that failed, and the start signal (`Some(true)` to time, `Some(false)` to stop).
#[derive(Default)]
struct Start {
    ready: usize,
    failed: Option<String>,
    go: Option<bool>,
}

/// The start state and the condition its waiters sleep on.
type Signal = (Mutex<Start>, Condvar);

/// `time <name> <milliseconds> <threads>`: the line it prints, or why it cannot.
fn time_threads(name: &str, milliseconds: &str, threads: &str) -> Result<String, Refusal> {
    let budget = round_budget(milliseconds).map_err(Refusal::usage)?;
    let threads: usize = threads
        .parse()
        .ok()
        .filter(|n| (1..=MAX_THREADS).contains(n))
        .ok_or_else(|| {
            Refusal::usage(format!(
                "the thread count must be 1 to {MAX_THREADS}, not {threads}"
            ))
        })?;
    if cfg!(target_family = "wasm") && threads != 1 {
        return Err(Refusal::usage(format!(
            "a WebAssembly build times one thread, not {threads}: the browser runner starts one \
             guest per worker"
        )));
    }
    if geom_bench::catalogue::prepare_named(name).is_none() {
        return Err(Refusal::usage(format!("no benchmark named {name}")));
    }
    let signal: Signal = (Mutex::new(Start::default()), Condvar::new());
    let results = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        for _ in 0..threads {
            match thread::Builder::new().spawn_scoped(scope, || one_thread(name, budget, &signal)) {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    // The threads already started are waiting for the signal: tell them to stop.
                    let (lock, wake) = &signal;
                    lock.lock().unwrap_or_else(PoisonError::into_inner).go = Some(false);
                    wake.notify_all();
                    return Err(format!("a timing thread could not start: {error}"));
                }
            }
        }
        // Start every thread together, once all have built and checked their batches.
        let (lock, wake) = &signal;
        let mut state = lock.lock().unwrap_or_else(PoisonError::into_inner);
        while state.ready < threads {
            state = wake.wait(state).unwrap_or_else(PoisonError::into_inner);
        }
        state.go = Some(state.failed.is_none());
        wake.notify_all();
        drop(state);
        let mut results: Vec<(f64, u64)> = Vec::with_capacity(threads);
        for handle in handles {
            match handle.join() {
                Ok(Some(result)) => results.push(result),
                Ok(None) => {}
                Err(_) => return Err("a timing thread stopped early".to_string()),
            }
        }
        Ok(results)
    })
    .map_err(Refusal::usage)?;
    let state = signal
        .0
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some(message) = state.failed {
        return Err(Refusal { status: 1, message });
    }
    let passes = results.iter().map(|&(_, passes)| passes).min().unwrap_or(0);
    let mut medians: Vec<f64> = results.into_iter().map(|(median, _)| median).collect();
    medians.sort_by(f64::total_cmp);
    let median = medians.get(medians.len() / 2).copied().unwrap_or(0.0);
    let slowest = medians.last().copied().unwrap_or(0.0);
    Ok(format!(
        "{name} {median:.3} {passes} {threads} {slowest:.3}\n"
    ))
}

/// One thread of `time`: builds and checks its own batch, reports ready, waits for the start
/// signal and times its rounds; `None` when told to stop, or when the benchmark is unknown.
fn one_thread(name: &str, budget: Duration, signal: &Signal) -> Option<(f64, u64)> {
    let mut bench = geom_bench::catalogue::prepare_named(name);
    let checked = match bench.as_mut() {
        Some(bench) => geom_bench::golden::check(name, &bench.run()),
        None => Err(format!("no benchmark named {name}")),
    };
    let (lock, wake) = signal;
    let mut state = lock.lock().unwrap_or_else(PoisonError::into_inner);
    state.ready = state.ready.saturating_add(1);
    if let Err(message) = checked {
        state.failed.get_or_insert(message);
    }
    wake.notify_all();
    while state.go.is_none() {
        state = wake.wait(state).unwrap_or_else(PoisonError::into_inner);
    }
    let go = state.go == Some(true);
    drop(state);
    let mut bench = bench?;
    go.then(|| rounds(&mut *bench, budget))
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
        ["time", name, ms, threads] => match time_threads(name, ms, threads) {
            Ok(line) => geom_bench::guest::Outcome {
                stdout: line,
                ..geom_bench::guest::Outcome::default()
            },
            Err(refusal) => geom_bench::guest::Outcome {
                status: refusal.status,
                stderr: format!(
                    "{}\nusage: geom-bench-guest time <name> <milliseconds> <threads>\n",
                    refusal.message
                ),
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
