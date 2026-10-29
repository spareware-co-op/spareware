//! The benchmark of piece G59 (chapter 30): one timing per operation of the Bernstein toolkit, on
//! the bicubic patch and cubic curve sizes the intersection engine meets most.
//!
//! Run with `cargo bench -p geom-nurbs --bench bernstein`. It uses the standard library's clock
//! only (wall-clock time on this machine; the reference machine and instruction counts are
//! geom-bench's, piece G18). The inputs come from a fixed linear congruential sequence.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::items_after_statements,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_num::grid::P3;
use geom_num::plane::Plane;
use geom_nurbs::exact::{normal_patch, plane_coefficients};
use geom_nurbs::{Basis, Curve, Direction, Surface};
use geom_run::{Budget, WorkMeter};

struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: u64) -> i64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % n) as i64
    }
}

/// Times `f`, which makes `calls` calls, and prints the time per call.
fn time(name: &str, runs: u32, calls: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(runs) / f64::from(calls);
    println!("{name:<56} {us:>10.2} µs");
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn clamped(p: usize, spans: usize) -> Vec<f64> {
    let mut k = vec![0.0; p + 1];
    k.extend((1..spans).map(|i| i as f64));
    k.extend(std::iter::repeat_n(spans as f64, p + 1));
    k
}

fn main() {
    let mut rng = Lcg(59);
    let mut c = || (rng.below(2001) - 1000) as f64;
    // A bicubic surface of 4 × 4 spans, nonrational and rational.
    let bu = Basis::new(3, clamped(3, 4)).unwrap();
    let bv = Basis::new(3, clamped(3, 4)).unwrap();
    let n = bu.count() * bv.count();
    let pts: Vec<[f64; 3]> = (0..n).map(|_| [c(), c(), c()]).collect();
    let w: Vec<f64> = (0..n).map(|k| 1.0 + (k % 3) as f64 * 0.5).collect();
    let s = Surface::new(bu.clone(), bv.clone(), &pts).unwrap();
    let r = Surface::new_rational(bu, bv, &pts, &w).unwrap();
    let curve = Curve::new(
        Basis::new(3, clamped(3, 16)).unwrap(),
        &(0..19).map(|_| [c(), c()]).collect::<Vec<_>>(),
    )
    .unwrap();
    let m = &mut meter();
    let patches = s.bernstein(m).unwrap();
    let rpatches = r.bernstein(m).unwrap();
    let pieces = curve.bernstein(m).unwrap();
    println!("Per call:");
    time(
        "Surface::bernstein, bicubic 4 × 4 spans (16 patches)",
        100,
        1,
        || {
            black_box(s.bernstein(&mut meter()).unwrap());
        },
    );
    time("BernsteinPatch::bound, bicubic", 1000, 16, || {
        for p in &patches {
            black_box(p.bound(&mut meter()).unwrap());
        }
    });
    time(
        "BernsteinPatch::derivative_bound u, bicubic",
        1000,
        16,
        || {
            for p in &patches {
                black_box(p.derivative_bound(Direction::U, &mut meter()).unwrap());
            }
        },
    );
    time(
        "BernsteinPatch::derivative_bound u, rational bicubic",
        100,
        16,
        || {
            for p in &rpatches {
                black_box(p.derivative_bound(Direction::U, &mut meter()).unwrap());
            }
        },
    );
    time("BernsteinPatch::fat_plane, bicubic", 1000, 16, || {
        for p in &patches {
            black_box(p.fat_plane(&mut meter()).unwrap());
        }
    });
    time(
        "BernsteinPatch::normal, bicubic (degree 5 × 5)",
        100,
        16,
        || {
            for p in &patches {
                black_box(p.normal(&mut meter()).unwrap());
            }
        },
    );
    time(
        "BernsteinPatch::normal, rational bicubic (8 × 8)",
        10,
        16,
        || {
            for p in &rpatches {
                black_box(p.normal(&mut meter()).unwrap());
            }
        },
    );
    time(
        "Surface::normal_patches, bicubic 4 × 4 spans",
        10,
        1,
        || {
            black_box(s.normal_patches(&mut meter()).unwrap());
        },
    );
    time("Curve::bernstein, cubic of 16 spans", 1000, 1, || {
        black_box(curve.bernstein(&mut meter()).unwrap());
    });
    let line = pieces[0].fat_line(m).unwrap();
    time("BernsteinCurve::fat_line, cubic", 10_000, 16, || {
        for p in &pieces {
            black_box(p.fat_line(&mut meter()).unwrap());
        }
    });
    time(
        "BernsteinCurve::clip, cubic against a fat line",
        10_000,
        16,
        || {
            for p in &pieces {
                black_box(p.clip(&line, &mut meter()).unwrap());
            }
        },
    );
    let a = patches[0].clone();
    let b = patches[1].clone();
    let b_on_a =
        geom_nurbs::BernsteinPatch::new(a.domain(), b.degrees(), b.points().to_vec(), None)
            .unwrap();
    time("BernsteinPatch::cross, bicubic × bicubic", 1000, 1, || {
        black_box(a.cross(&b_on_a, &mut meter()).unwrap());
    });
    // The exact paths, on a bicubic grid patch at the extremes of the grid.
    const M: i64 = (1 << 40) - 1;
    let mut g = Lcg(5);
    let grid: Vec<P3> = (0..16)
        .map(|_| {
            let mut e = || if g.below(2) == 0 { M } else { -M };
            P3::new(e(), e(), e()).unwrap()
        })
        .collect();
    time("exact::normal_patch, bicubic grid patch", 1000, 1, || {
        black_box(normal_patch(&grid, (3, 3), &mut meter()).unwrap());
    });
    let plane = Plane::through(grid[0], grid[5], grid[10]).unwrap();
    time(
        "exact::plane_coefficients, bicubic grid patch",
        10_000,
        1,
        || {
            black_box(plane_coefficients(&grid, &plane, &mut meter()).unwrap());
        },
    );
}
