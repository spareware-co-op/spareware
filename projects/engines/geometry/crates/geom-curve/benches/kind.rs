//! The benchmark of piece G60 (chapter 30): one timing per operation of the kind contract on the
//! NURBS curve kind, a cubic of 16 spans in 3D, nonrational and rational.
//!
//! Run with `cargo bench -p geom-curve`. It uses the standard library's clock only (wall-clock
//! time on this machine; the reference machine and instruction counts are geom-bench's, piece
//! G18). The inputs come from a fixed linear congruential sequence.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_curve::ParamCurve;
use geom_math::{Axis, Transform, TypedTransform};
use geom_nurbs::{Basis, Curve, Limit};
use geom_run::{Budget, WorkMeter};

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
    println!("{name:<52} {us:>10.2} µs");
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn main() {
    let mut rng = Lcg(60);
    let mut k = vec![0.0; 4];
    k.extend((1..16).map(f64::from));
    k.extend([16.0; 4]);
    let basis = Basis::new(3, k).unwrap();
    let pts: Vec<[f64; 3]> = (0..basis.count())
        .map(|_| [rng.unit() * 100.0, rng.unit() * 100.0, rng.unit() * 100.0])
        .collect();
    let w: Vec<f64> = (0..basis.count())
        .map(|i| 1.0 + (i % 3) as f64 * 0.25)
        .collect();
    let plain = Curve::new(basis.clone(), &pts).unwrap();
    let rational = Curve::new_rational(basis, &pts, &w).unwrap();
    let turn = TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap());
    for (label, c) in [("cubic", &plain), ("rational cubic", &rational)] {
        println!("{label}, 16 spans:");
        time("  enclose (point and two derivatives)", 1000, || {
            black_box(c.enclose(7.3, Limit::FromRight, 2, &mut meter()).unwrap());
        });
        time("  bound over [2.5, 9.5]", 1000, || {
            black_box(ParamCurve::bound(c, (2.5, 9.5), &mut meter()).unwrap());
        });
        time("  derivative_bound order 2 over [2.5, 9.5]", 1000, || {
            black_box(c.derivative_bound((2.5, 9.5), 2, &mut meter()).unwrap());
        });
        time("  split at 7.3", 1000, || {
            black_box(ParamCurve::split(c, 7.3, &mut meter()).unwrap());
        });
        time("  reversed", 10_000, || {
            black_box(c.reversed(&mut meter()).unwrap());
        });
        time("  transformed (certified rotation)", 10_000, || {
            black_box(c.transformed(&turn, &mut meter()).unwrap());
        });
    }
}
