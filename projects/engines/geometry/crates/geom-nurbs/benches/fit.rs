//! The benchmark of piece G58 (chapter 30): one timing per fitting operation of The NURBS Book's
//! chapter 9 as `geom-nurbs` builds it, each with its certified error, on data of the sizes a
//! cached intersection curve, a loft section or a survey string brings.
//!
//! Run with `cargo bench -p geom-nurbs --bench fit`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18). The inputs come from correctly rounded maths and a fixed linear congruential
//! sequence, so every run measures the same work.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::similar_names,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_nurbs::{
    Constraint, Curve, FitVersion, KnotMode, Parametrization, Surface, least_squares_knots,
    parameters,
};
use geom_run::{Budget, WorkMeter};

const V: FitVersion = FitVersion::V1;

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

fn sinpi(x: f64) -> f64 {
    geom_num::math::sinpi(x).unwrap().get()
}

fn cospi(x: f64) -> f64 {
    geom_num::math::cospi(x).unwrap().get()
}

/// `count` points along a helix of 0.65 turns.
fn helix(count: usize) -> Vec<[f64; 3]> {
    (0..count)
        .map(|k| {
            let t = k as f64 / (count - 1) as f64 * 1.3;
            [cospi(t) * 2.0, sinpi(t) * 2.0, 0.9 * t]
        })
        .collect()
}

/// Times `f` over `runs` runs and prints the time per run.
fn time(name: &str, runs: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
    println!("{name:<60} {us:>12.1} µs");
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn main() {
    let small = helix(100);
    let large = helix(1000);
    println!("Per fit, with its certified error:");
    time("A9.1 interpolation, 100 points, cubic", 50, || {
        black_box(
            Curve::interpolate(&small, 3, Parametrization::ChordLength, V, &mut meter()).unwrap(),
        );
    });
    time("A9.1 interpolation, 1000 points, cubic", 10, || {
        black_box(
            Curve::interpolate(&large, 3, Parametrization::ChordLength, V, &mut meter()).unwrap(),
        );
    });
    time(
        "9.2.2 interpolation with end derivatives, 1000 points",
        10,
        || {
            let ends = [[0.0, 1.0, 0.3], [-1.0, 0.0, 0.3]];
            black_box(
                Curve::interpolate_with_ends(
                    &large,
                    ends,
                    3,
                    Parametrization::ChordLength,
                    V,
                    &mut meter(),
                )
                .unwrap(),
            );
        },
    );
    time("9.4.1 least squares, 1000 points to 50, cubic", 10, || {
        black_box(
            Curve::least_squares(&large, 50, 3, Parametrization::ChordLength, V, &mut meter())
                .unwrap(),
        );
    });
    let u = parameters(&large, Parametrization::ChordLength, &mut meter()).unwrap();
    let basis = least_squares_knots(&u, 50, 3, &mut meter()).unwrap();
    let weights: Vec<f64> = (0..1000).map(|k| 1.0 + f64::from(k % 3)).collect();
    let cons = [
        Constraint {
            parameter: 0.0,
            order: 0,
            value: large[0],
        },
        Constraint {
            parameter: 0.0,
            order: 1,
            value: [0.0, 2.0, 0.9],
        },
        Constraint {
            parameter: 1.0,
            order: 0,
            value: large[999],
        },
    ];
    time(
        "A9.6 weighted, 3 constraints, 1000 points to 50",
        10,
        || {
            black_box(
                Curve::least_squares_at(
                    &large,
                    Some(&weights),
                    &u,
                    basis.clone(),
                    &cons,
                    V,
                    &mut meter(),
                )
                .unwrap(),
            );
        },
    );
    for (count, tol) in [(200, 1e-3), (1000, 1e-4)] {
        let data = helix(count);
        for mode in [KnotMode::Single, KnotMode::Multiple] {
            let name = format!("9.4.4 to a stated error, {count} points, {tol:e}, {mode:?}");
            let fit = Curve::approximate(&data, 3, tol, mode, V, &mut meter()).unwrap();
            time(&name, 3, || {
                black_box(Curve::approximate(&data, 3, tol, mode, V, &mut meter()).unwrap());
            });
            println!(
                "    {} control points, certified error {:.3e}",
                fit.curve.homogeneous().len(),
                fit.error
            );
        }
    }
    let mut rng = Lcg(58);
    let (rows, cols) = (30, 30);
    let grid: Vec<[f64; 3]> = (0..rows * cols)
        .map(|k| {
            let (i, j) = ((k / cols) as f64, (k % cols) as f64);
            [i, j, sinpi(i * 0.05) * cospi(j * 0.04) + 0.001 * rng.unit()]
        })
        .collect();
    time("A9.4 surface interpolation, 30 × 30, bicubic", 10, || {
        black_box(
            Surface::interpolate(
                &grid,
                (rows, cols),
                (3, 3),
                Parametrization::ChordLength,
                V,
                &mut meter(),
            )
            .unwrap(),
        );
    });
    time(
        "A9.7 surface least squares, 30 × 30 to 10 × 10",
        10,
        || {
            black_box(
                Surface::least_squares(
                    &grid,
                    (rows, cols),
                    (10, 10),
                    (3, 3),
                    Parametrization::ChordLength,
                    V,
                    &mut meter(),
                )
                .unwrap(),
            );
        },
    );
}
