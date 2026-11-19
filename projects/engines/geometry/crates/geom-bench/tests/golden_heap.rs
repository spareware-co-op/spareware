//! The instruction gate's teardown takes no heap: `golden::check`, which it runs after each measured
//! function inside the process DHAT measures, allocates nothing when the digests match.
//!
//! DHAT's "At t-gmax" reading of a benchmark is the benchmark's heap at the peak of the whole
//! process, so heap the teardown took would decide whether that peak falls in the teardown, where
//! everything the measured function kept is still live, or earlier, in the setup, where none of it
//! is. When `check` parsed the whole of golden.txt into a vector, its heap grew with the number of
//! benchmarks, and a benchmark that did not change could read its kept heap at t-gmax against a
//! merge base's 0: the vector's doubling past 1,024 lines moved the process's peak into the
//! teardown.
//!
//! The counter is the `cap` crate's (MIT OR Apache-2.0) with its `stats` feature, around the system
//! allocator, for this test binary only: it is the process's allocator, so this file holds one
//! test. It counts every thread's allocations, so the binary has its own harness, libtest-mimic's
//! (MIT OR Apache-2.0), told to use one thread: the test runs on the main thread, the process's
//! only one, as geom-check's `tests/memory.rs` explains.

#![allow(
    clippy::unwrap_used,
    reason = "test code: a failure should stop loudly"
)]
#![allow(
    clippy::arithmetic_side_effects,
    reason = "test code may use native arithmetic, and clippy sees no #[test] here"
)]

use std::alloc::System;
use std::process::ExitCode;

use cap::Cap;
use geom_bench::golden::{self, GOLDEN};
use libtest_mimic::{Arguments, Trial};

#[global_allocator]
static ALLOCATOR: Cap<System> = Cap::new(System, usize::MAX);

fn main() -> ExitCode {
    let mut args = Arguments::from_args();
    // One thread, whatever the command line asks: the test runs on this thread, and no other
    // thread exists to allocate while it counts.
    args.test_threads = Some(1);
    let trials = vec![Trial::test(
        "checking_a_recorded_benchmark_takes_no_heap",
        || {
            checking_a_recorded_benchmark_takes_no_heap();
            Ok(())
        },
    )];
    libtest_mimic::run(&args, trials).exit_code()
}

/// Every recorded benchmark, the first line of the file to its last, checked against its own
/// digests: each check passes and allocates not one byte.
fn checking_a_recorded_benchmark_takes_no_heap() {
    let entries = golden::parse(GOLDEN).unwrap();
    for entry in &entries {
        let before = ALLOCATOR.total_allocated();
        let verdict = golden::check(entry.name, &entry.digests);
        let taken = ALLOCATOR.total_allocated() - before;
        assert_eq!(verdict, Ok(()), "{}", entry.name);
        assert_eq!(
            taken, 0,
            "golden::check({}) took {taken} bytes of heap",
            entry.name
        );
    }
}
