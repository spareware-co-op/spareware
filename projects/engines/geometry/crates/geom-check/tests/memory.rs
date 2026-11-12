//! A refused allocation stops a check with the meter's `OutOfMemory`, never an abort (23.10). The
//! check makes two reservations before it reads the body, the faults and then the scratch every
//! check shares; an allocator capped just short of each refuses it, and the check stops there.
//!
//! The cap is the `cap` crate's (MIT OR Apache-2.0), around the system allocator, for this test
//! binary only: it is the process's allocator, so this file holds one test.
//!
//! The cap counts every thread's allocations, and the test sets it to the byte, so no other thread
//! may allocate while it is set. Rust's own test harness runs a test on a thread of its own while
//! its main thread goes on allocating (its list of running tests, and the channel it waits on the
//! first time it waits), and an allocation of that thread past the cap is an abort, not a refusal.
//! So this binary has its own harness (`harness = false`), libtest-mimic's (MIT OR Apache-2.0),
//! which lists and runs the test as the standard one does, for cargo test and nextest alike, and is
//! told to use one thread: the test then runs on the main thread, the process's only one.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a failure should stop loudly"
)]
#![allow(
    clippy::arithmetic_side_effects,
    reason = "test code may use native arithmetic, and clippy sees no #[test] here"
)]

mod common;

use std::alloc::System;
use std::process::ExitCode;

use cap::Cap;
use common::{cube, meter};
use geom_check::{Fault, Level, MAX_FAULTS, check};
use geom_num::Exactness;
use geom_run::Stop;
use geom_topo::BodyKind;
use libtest_mimic::{Arguments, Trial};

#[global_allocator]
static ALLOCATOR: Cap<System> = Cap::new(System, usize::MAX);

fn main() -> ExitCode {
    let mut args = Arguments::from_args();
    // One thread, whatever the command line asks: the test runs on this thread, and no other
    // thread exists to allocate while the cap is set.
    args.test_threads = Some(1);
    let trials = vec![Trial::test("a_refused_reservation_stops_the_check", || {
        a_refused_reservation_stops_the_check();
        Ok(())
    })];
    libtest_mimic::run(&args, trials).exit_code()
}

fn a_refused_reservation_stops_the_check() {
    let body = cube().body(BodyKind::Solid);
    let faults = MAX_FAULTS * size_of::<Fault>();
    // The cube's scratch: three words per vertex and one per edge, its largest need. Then group
    // 3's outlines: a list of six, and for each face a list of its one outline and of its four
    // sides.
    let scratch = (3 * 8 + 12) * size_of::<u32>();
    let outlines = 6 * size_of::<Vec<u8>>() + 6 * 128 + 6 * 4 * 512;
    let mut outcomes = Vec::new();
    for room in [
        0,
        faults - 1,
        faults,
        faults + scratch - 1,
        faults + scratch + 4 * outlines,
    ] {
        let mut m = meter();
        ALLOCATOR.set_limit(ALLOCATOR.allocated() + room).unwrap();
        let result = check(&body, Exactness::Exact, Level::Full, &mut m);
        ALLOCATOR.set_limit(usize::MAX).unwrap();
        outcomes.push(
            result
                .map(|r| r.passed())
                .map_err(geom_run::Stopped::reason),
        );
    }
    assert_eq!(
        outcomes,
        [
            Err(Stop::OutOfMemory),
            Err(Stop::OutOfMemory),
            Err(Stop::OutOfMemory),
            Err(Stop::OutOfMemory),
            Ok(true),
        ]
    );
}
