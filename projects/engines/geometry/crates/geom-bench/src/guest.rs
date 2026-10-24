//! The program the WebAssembly fuel gate runs inside Wasmtime: it builds a benchmark and runs its
//! kernel a given number of times, or checks its golden digests on wasm32.
//!
//! Built for `wasm32-wasip1` as the binary `geom-bench-guest` (and natively too, for its tests).
//! Wasmtime's fuel counts the WebAssembly operators the guest executes, deterministically
//! (chapter 23.13, "WebAssembly fuel"). The host (`fuel/`, beside this crate) counts the fuel of a
//! whole run, so it runs each benchmark twice, `run <name> 1` and `run <name> 2`: the two runs
//! execute the same instructions except one more [`crate::harness::Bench::pass`] and one more step
//! of the loop around it, so the difference is the fuel of one pass over the batch plus one step
//! of that loop. Parsing the pass count takes the same path for both digits, and nothing but
//! [`run_passes`] uses the count, so nothing else differs: code that branched on it would run
//! differently in the two runs and move every count (an experiment that added a test of
//! `passes == 2` to [`main`] moved every count by about 20). [`run_passes`] is never inlined, so the
//! machine code of the loop, and the fuel of a step, cannot depend on the code around its call.
//!
//! Commands (arguments after the program name):
//!
//! - `list`: every benchmark name, one per line.
//! - `run <name> <passes>`: build the benchmark and run `passes` passes (0 to 255; the host asks
//!   for 1 and 2), printing nothing.
//! - `check <name>`: build the benchmark, run one pass, and check its digests against
//!   `golden.txt`: the results on wasm32 must be the bits recorded natively (principle 5).
//!
//! Exit status: 0 on success, 1 when a golden check fails, 2 for an unknown command or benchmark.
//!
//! Method: fuel metering as Wasmtime documents it (`Config::consume_fuel`,
//! <https://github.com/bytecodealliance/wasmtime/blob/5afd63588b1f5520391c6a37fc38dfd3ccd607c2/crates/wasmtime/src/config.rs#L627-L646>),
//! with the measured work isolated by differencing two runs that differ only in it.
//! Consulted: Wasmtime, Apache-2.0 WITH LLVM-exception,
//!   <https://github.com/bytecodealliance/wasmtime/blob/5afd63588b1f5520391c6a37fc38dfd3ccd607c2/crates/wasmtime/src/config.rs#L627-L646>,
//!   for what fuel counts and how `Config::consume_fuel` meters it. No code copied.
//!
//! Guarantees: the only difference between `run <name> 1` and `run <name> 2` is one pass and one
//! step of the loop in [`run_passes`], and `check` fails unless both digests match `golden.txt`.
//!
//! Cost: each command builds one batch, O([`crate::inputs::BATCH`]), then runs the passes asked
//! for, or one pass and its digests.

use core::hint::black_box;

use crate::catalogue::{names, prepare_named};
use crate::golden;
use crate::harness::Bench;

/// What the guest printed and how it ended.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The exit status: 0 success, 1 a failed golden check, 2 a usage error.
    pub status: u8,
    /// What to print on standard output.
    pub stdout: String,
    /// What to print on standard error.
    pub stderr: String,
}

impl Outcome {
    /// A usage error.
    fn usage(message: &str) -> Outcome {
        Outcome {
            status: 2,
            stdout: String::new(),
            stderr: format!(
                "{message}\nusage: geom-bench-guest list | run <name> <passes> | check <name>\n"
            ),
        }
    }
}

/// Runs the guest on its arguments (without the program name).
#[must_use]
pub fn main(args: &[&str]) -> Outcome {
    match args {
        ["list"] => Outcome {
            stdout: names().into_iter().map(|n| n + "\n").collect(),
            ..Outcome::default()
        },
        ["run", name, passes] => {
            let Ok(passes) = passes.parse::<u8>() else {
                return Outcome::usage("the pass count must be a number from 0 to 255");
            };
            let Some(mut bench) = prepare_named(name) else {
                return Outcome::usage(&format!("no benchmark named {name}"));
            };
            run_passes(&mut *bench, passes);
            black_box(&bench);
            Outcome::default()
        }
        ["check", name] => {
            let Some(mut bench) = prepare_named(name) else {
                return Outcome::usage(&format!("no benchmark named {name}"));
            };
            match golden::check(name, &bench.run()) {
                Ok(()) => Outcome::default(),
                Err(message) => Outcome {
                    status: 1,
                    stderr: message + "\n",
                    ..Outcome::default()
                },
            }
        }
        _ => Outcome::usage("unknown command"),
    }
}

/// Runs `passes` passes of `bench`, in order. The fuel gate's difference of two runs is one pass
/// and one step of this loop, so the loop lives in a function of its own that is never inlined:
/// its machine code, and so the fuel of a step, cannot depend on the code around the call.
#[inline(never)]
pub fn run_passes(bench: &mut dyn Bench, passes: u8) {
    for _ in 0..passes {
        bench.pass();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_runs_and_checks() {
        let list = main(&["list"]);
        assert_eq!(list.status, 0);
        assert_eq!(list.stdout.lines().count(), names().len());
        assert_eq!(
            main(&["run", "predicate/orient2d/typical", "2"]),
            Outcome::default()
        );
        assert_eq!(main(&["check", "growth/wide_mul/l8"]).status, 0);
        assert_eq!(main(&["run", "predicate/orient2d/typical", "x"]).status, 2);
        assert_eq!(main(&["run", "no/such/bench", "1"]).status, 2);
        assert_eq!(main(&["check", "no/such/bench"]).status, 2);
        assert_eq!(main(&[]).status, 2);
    }
}
