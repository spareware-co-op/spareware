//! A refused allocation stops an operation with `OutOfMemory`, never an abort and never a defect
//! (23.10). The checker's reservation is the first allocation an operation makes that is larger
//! than a few kilobytes, so an allocator capped [`ROOM`] bytes above what is in use refuses it, and
//! nothing the operation allocates before it. A refusal in a column is `geom-topo`'s to report, as
//! `TopoError::stop` (`geom-topo`'s `tests/stop.rs`).
//!
//! The cap is the `cap` crate's (MIT OR Apache-2.0), around the system allocator, for this test
//! binary only: it is the process's allocator, so this file holds one test.

#![allow(
    clippy::unwrap_used,
    reason = "test code: a failure should stop loudly"
)]

use std::alloc::System;

use cap::Cap;
use geom_make::{Moved, SphereInput, move_by, sphere};
use geom_num::{GridStep, P3, Steps};
use geom_op::{ErrorKind, OpContext, OpId, OpPath};
use geom_run::Budget;

#[global_allocator]
static ALLOCATOR: Cap<System> = Cap::new(System, usize::MAX);

/// More than the small lists an operation builds first, less than the checker's reservation.
const ROOM: usize = 12 * 1024;

#[test]
fn a_refused_allocation_is_out_of_memory() {
    let ctx = OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED);
    let input = SphereInput {
        center: P3::new(0, 0, 0).unwrap(),
        radius: Steps(5),
        step: GridStep::Q0,
    };
    let ball = sphere(&ctx, &input).unwrap();
    let moved = Moved {
        body: &ball.result.body,
        exactness: ball.exactness,
        accept: None,
    };
    ALLOCATOR.set_limit(ALLOCATOR.allocated() + ROOM).unwrap();
    let built = sphere(&ctx, &input).map(|_| ());
    let copied = move_by(&ctx, moved, [1, 0, 0]).map(|_| ());
    ALLOCATOR.set_limit(usize::MAX).unwrap();
    assert_eq!(built.unwrap_err().kind(), ErrorKind::OutOfMemory);
    assert_eq!(copied.unwrap_err().kind(), ErrorKind::OutOfMemory);
}
