//! A refused allocation stops a check with the meter's `OutOfMemory`, never an abort (23.10). The
//! check makes two reservations before it reads the body, the faults and then the scratch every
//! check shares; an allocator capped just short of each refuses it, and the check stops there.
//!
//! The cap is the `cap` crate's (MIT OR Apache-2.0), around the system allocator, for this test
//! binary only: it is the process's allocator, so this file holds one test.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use std::alloc::System;

use cap::Cap;
use common::{cube, meter};
use geom_check::{Fault, Level, MAX_FAULTS, check};
use geom_num::Exactness;
use geom_run::Stop;
use geom_topo::BodyKind;

#[global_allocator]
static ALLOCATOR: Cap<System> = Cap::new(System, usize::MAX);

#[test]
fn a_refused_reservation_stops_the_check() {
    let body = cube().body(BodyKind::Solid);
    let faults = MAX_FAULTS * size_of::<Fault>();
    // The cube's scratch: three words per vertex and one per edge, its largest need.
    let scratch = (3 * 8 + 12) * size_of::<u32>();
    let mut outcomes = Vec::new();
    for room in [
        0,
        faults - 1,
        faults,
        faults + scratch - 1,
        faults + scratch,
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
