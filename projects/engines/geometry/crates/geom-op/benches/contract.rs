//! The benchmark of piece G82 (chapter 30): what the operation contract costs an operation, one
//! timing each.
//!
//! Run with `cargo bench -p geom-op --bench contract`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are
//! geom-bench's, piece G18).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_num::NumError;
use geom_op::{ErrorKind, NumDetail, OpContext, OpError, OpId, OpPath};
use geom_run::{Budget, CancelToken};

fn time(name: &str, runs: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
    println!("{name:<44} {us:>10.3} µs");
}

fn main() {
    let batch = OpPath::new(OpId::from_bytes([7; 16])).step("cut").unwrap();
    let token = CancelToken::new();
    time("OpPath::item (a batch item's path)", 100_000, || {
        black_box(black_box(&batch).item(black_box([3; 16])));
    });
    time("OpContext::new with a cancellation", 100_000, || {
        let ctx = OpContext::new(black_box(batch.clone()), Budget::UNLIMITED).with_cancel(&token);
        black_box(ctx);
    });
    time("NumDetail::from(NumError).kind()", 1_000_000, || {
        black_box(NumDetail::from(black_box(NumError::BudgetExceeded)).kind());
    });
    time("OpError::new from a NumError", 100_000, || {
        let error = OpError::new(
            black_box(batch.clone()),
            black_box(NumError::OutOfRange { value: 1 << 41 }),
        );
        assert_eq!(error.kind(), ErrorKind::FrameRangeExceeded);
        black_box(error);
    });
}
