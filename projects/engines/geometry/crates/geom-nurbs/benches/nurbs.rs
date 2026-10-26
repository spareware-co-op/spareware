//! The benchmark of piece G56 (chapter 30): one timing per algorithm of The NURBS Book's chapters
//! 2 to 5 as `geom-nurbs` builds them, on a cubic curve of 100 control points (nonrational and
//! rational) and a bicubic surface of 20 × 20.
//!
//! Run with `cargo bench -p geom-nurbs`. It uses the standard library's clock only (wall-clock
//! time on this machine; the reference machine and instruction counts are geom-bench's, piece
//! G18). The inputs come from a fixed linear congruential sequence, so every run measures the same
//! work.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_nurbs::{Basis, Curve, Direction, Limit, Surface};
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

/// A clamped knot vector of degree `p` with `n` functions and interior knots 1, 2, ….
fn clamped(p: usize, n: usize) -> Vec<f64> {
    let inner = n - p - 1;
    let mut k = vec![0.0; p + 1];
    k.extend((1..=inner).map(|i| i as f64));
    k.extend(std::iter::repeat_n((inner + 1) as f64, p + 1));
    k
}

/// Times `f`, which makes `calls` calls of the algorithm, and prints the time per call.
fn time(name: &str, runs: u32, calls: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(runs) / f64::from(calls);
    println!("{name:<52} {us:>10.3} µs");
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

#[allow(
    clippy::too_many_lines,
    reason = "one timing per algorithm, listed in book order"
)]
fn main() {
    let mut rng = Lcg(56);
    let p = 3;
    let basis = Basis::new(p, clamped(p, 100)).unwrap();
    let (lo, hi) = basis.domain();
    let params: Vec<f64> = (0..256).map(|_| lo + (hi - lo) * rng.unit()).collect();
    let points: Vec<[f64; 3]> = (0..100)
        .map(|_| [rng.unit(), rng.unit(), rng.unit()])
        .collect();
    let weights: Vec<f64> = (0..100).map(|_| 0.5 + rng.unit()).collect();
    let curve = Curve::new(basis.clone(), &points).unwrap();
    let rational = Curve::new_rational(basis.clone(), &points, &weights).unwrap();

    println!("Per call, averaged over 256 parameters, on the cubic curve of 100 control points:");
    time("A2.1 span", 1000, 256, || {
        for &u in &params {
            black_box(basis.span(black_box(u), Limit::FromRight).unwrap());
        }
    });
    let mut n = [0.0; 4];
    time("A2.2 basis functions", 1000, 256, || {
        for &u in &params {
            black_box(basis.eval(black_box(u), Limit::FromRight, &mut n).unwrap());
        }
    });
    let mut nd = [0.0; 3 * 4];
    time("A2.3 basis derivatives, order 2", 1000, 256, || {
        for &u in &params {
            black_box(
                basis
                    .eval_derivs(black_box(u), Limit::FromRight, 2, &mut nd)
                    .unwrap(),
            );
        }
    });
    time("A2.4 one basis function", 1000, 256, || {
        for &u in &params {
            black_box(basis.eval_one(50, black_box(u), Limit::FromRight).unwrap());
        }
    });
    let mut od = [0.0; 3];
    time(
        "A2.5 one basis function's derivatives, order 2",
        1000,
        256,
        || {
            for &u in &params {
                basis
                    .eval_one_derivs(50, black_box(u), Limit::FromRight, &mut od)
                    .unwrap();
                black_box(&od);
            }
        },
    );
    time("A3.1 curve point", 1000, 256, || {
        for &u in &params {
            black_box(curve.point(black_box(u), Limit::FromRight).unwrap());
        }
    });
    time("A4.1 rational curve point", 1000, 256, || {
        for &u in &params {
            black_box(rational.point(black_box(u), Limit::FromRight).unwrap());
        }
    });
    time("A5.2 curve point by corner cutting", 1000, 256, || {
        for &u in &params {
            black_box(
                curve
                    .point_by_corner_cutting(black_box(u), Limit::FromRight)
                    .unwrap(),
            );
        }
    });
    let mut cd = [[0.0; 3]; 3];
    time("A3.2 curve derivatives, order 2", 1000, 256, || {
        for &u in &params {
            curve
                .derivs(black_box(u), Limit::FromRight, &mut cd)
                .unwrap();
            black_box(&cd);
        }
    });
    time(
        "A4.2 rational curve derivatives, order 2",
        1000,
        256,
        || {
            for &u in &params {
                rational
                    .derivs(black_box(u), Limit::FromRight, &mut cd)
                    .unwrap();
                black_box(&cd);
            }
        },
    );

    println!("Per call, on the curve of 100 control points:");
    time("A3.3 derivative curve, order 2", 200, 1, || {
        black_box(curve.derivative_curve(2, &mut meter()).unwrap());
    });
    time("A5.1 knot insertion, 3 times", 200, 1, || {
        black_box(curve.insert_knot(black_box(41.5), 3, &mut meter()).unwrap());
    });
    let x: Vec<f64> = (0..96).map(|i| f64::from(i) + 0.5).collect();
    time("A5.4 refinement, 96 knots", 50, 1, || {
        black_box(curve.refine(black_box(&x), &mut meter()).unwrap());
    });
    time("A5.6 Bezier extraction", 50, 1, || {
        black_box(curve.to_bezier(&mut meter()).unwrap());
    });
    time("A5.8 knot removal, certified", 200, 1, || {
        black_box(
            curve
                .remove_knot(black_box(41.0), 1, 1e-3, &mut meter())
                .unwrap(),
        );
    });
    time("A5.9 degree elevation by 1", 50, 1, || {
        black_box(curve.elevate(1, &mut meter()).unwrap());
    });
    let up = curve.elevate(1, &mut meter()).unwrap().value;
    time("A5.11 degree reduction, certified", 50, 1, || {
        black_box(up.reduce_degree(1e-6, &mut meter()).unwrap());
    });
    time("A5.9 rational degree elevation by 1", 50, 1, || {
        black_box(rational.elevate(1, &mut meter()).unwrap());
    });

    let bu = Basis::new(3, clamped(3, 20)).unwrap();
    let bv = Basis::new(3, clamped(3, 20)).unwrap();
    let net: Vec<[f64; 3]> = (0..400)
        .map(|_| [rng.unit(), rng.unit(), rng.unit()])
        .collect();
    let surface = Surface::new(bu.clone(), bv.clone(), &net).unwrap();
    let (slo, shi) = bu.domain();
    let uv: Vec<(f64, f64)> = (0..256)
        .map(|_| {
            (
                slo + (shi - slo) * rng.unit(),
                slo + (shi - slo) * rng.unit(),
            )
        })
        .collect();
    let lim = (Limit::FromRight, Limit::FromRight);
    println!("Per call, averaged over 256 parameters, on the bicubic surface of 20 × 20:");
    time("A3.5 surface point", 1000, 256, || {
        for &(u, v) in &uv {
            black_box(surface.point(black_box(u), black_box(v), lim).unwrap());
        }
    });
    let mut sd = [[0.0; 3]; 9];
    time("A3.6 surface derivatives, orders (2, 2)", 200, 256, || {
        for &(u, v) in &uv {
            surface
                .derivs(black_box(u), black_box(v), lim, (2, 2), &mut sd)
                .unwrap();
            black_box(&sd);
        }
    });
    println!("Per call, on the surface:");
    time("A3.7 derivative surface, orders (1, 1)", 100, 1, || {
        black_box(surface.derivative_surface((1, 1), &mut meter()).unwrap());
    });
    time("A5.3 surface knot insertion in u", 100, 1, || {
        black_box(
            surface
                .insert_knot(Direction::U, black_box(7.5), 1, &mut meter())
                .unwrap(),
        );
    });
    let xs: Vec<f64> = (0..16).map(|i| f64::from(i) + 0.5).collect();
    time("A5.5 surface refinement in v, 16 knots", 50, 1, || {
        black_box(
            surface
                .refine(Direction::V, black_box(&xs), &mut meter())
                .unwrap(),
        );
    });
    time("A5.7 surface Bezier extraction", 20, 1, || {
        black_box(surface.to_bezier(&mut meter()).unwrap());
    });
    time("A5.10 surface degree elevation in u by 1", 20, 1, || {
        black_box(surface.elevate(Direction::U, 1, &mut meter()).unwrap());
    });
}
