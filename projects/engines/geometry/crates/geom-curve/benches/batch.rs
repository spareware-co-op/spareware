//! The benchmark of piece G104 (chapter 30): a batch of 10,000 items through `geom-op`'s batch
//! driver against the same 10,000 calls made one by one, and 10,000 points of a curve in one
//! flat-array call against 10,000 single evaluations.
//!
//! Run with `cargo bench -p geom-curve --bench batch`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18). The inputs come from a fixed linear congruential sequence.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::result_large_err,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::num::NonZeroUsize;
use std::time::Instant;

use geom_curve::{Circle, CurveError, CurveKind, Frame, ParamCurve};
use geom_nurbs::Limit;
use geom_op::{Batch, OpContext, OpError, OpId, OpPath, RunDetail};
use geom_run::{BatchBudget, Budget, ChunkPlan, Exec, Phase, WorkMeter};

struct Lcg(u64);

impl Lcg {
    fn unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn time(name: &str, runs: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
    println!("{name:<56} {us:>12.2} µs");
}

const N: usize = 10_000;

/// One item: a parameter of the curve and its key.
struct Item {
    key: [u8; 16],
    t: f64,
}

/// The item's operation: one point of the curve, one work unit.
fn item(
    curve: &CurveKind<3>,
) -> impl Fn(&OpContext<'_>, &Item, &mut WorkMeter<'_>) -> Result<[f64; 3], OpError> + Sync + '_ {
    move |ctx, it, meter| {
        meter
            .charge(1)
            .map_err(|_| OpError::new(ctx.path().clone(), RunDetail::Cancelled))?;
        curve
            .point(it.t, Limit::FromRight)
            .map_err(|_| OpError::new(ctx.path().clone(), RunDetail::Cancelled))
    }
}

fn main() {
    let circle = Circle::new([1.0, 2.0, 3.0], 4.0, Frame::standard().unwrap(), (0.0, 2.0)).unwrap();
    let curve = CurveKind::Circle(circle);
    let mut rng = Lcg(104);
    let items: Vec<Item> = (0..N)
        .map(|i| {
            let mut key = [0u8; 16];
            key[..8].copy_from_slice(&(i as u64).to_be_bytes());
            Item {
                key,
                t: 2.0 * rng.unit(),
            }
        })
        .collect();
    let ctx = OpContext::new(OpPath::new(OpId::from_bytes([1; 16])), Budget::UNLIMITED);
    let batch = Batch::new(
        &ctx,
        BatchBudget::new(Budget::of_work(10)),
        Phase::new("points"),
        ChunkPlan::new(NonZeroUsize::new(256).unwrap()),
    );
    let op = item(&curve);
    time("batch of 10,000 items (serial executor)", 20, || {
        black_box(
            batch
                .run(&Exec::serial(), black_box(&items), |it| it.key, &op)
                .unwrap(),
        );
    });
    time("10,000 single calls (Batch::single)", 20, || {
        for it in black_box(&items) {
            black_box(batch.single(it.key, it, &op).unwrap());
        }
    });
    let ts: Vec<f64> = items.iter().map(|it| it.t).collect();
    let mut out = vec![0.0; 3 * N];
    let mut results: Vec<Result<(), CurveError>> = vec![Ok(()); N];
    time("10,000 points in one flat-array call", 50, || {
        curve
            .points(black_box(&ts), Limit::FromRight, &mut out, &mut results)
            .unwrap();
        black_box(&out);
    });
    time("10,000 single point evaluations", 50, || {
        for &t in black_box(&ts) {
            black_box(curve.point(t, Limit::FromRight).unwrap());
        }
    });
}
