//! The benchmark of piece G66 (chapter 30): the offset surface kind's main operations, one timing
//! each, for offsets of a bicubic NURBS patch, a rational one, a cylinder and an offset of an
//! offset.
//!
//! Run with `cargo bench -p geom-surface --bench offsets`. It uses the standard library's clock
//! only (wall-clock time on this machine; the reference machine and instruction counts are
//! geom-bench's, piece G18).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design, and names the domain's ends as the formulas do"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_curve::Frame;
use geom_math::{Transform, TypedTransform};
use geom_nurbs::{Basis, Direction, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, Offset, ParamSurface, SurfaceKind};

fn time(name: &str, runs: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
    println!("{name:<44} {us:>12.3} µs");
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A bicubic patch over [0, 1]² with a knot at u = 1/2 and fixed waves.
fn patch(rational: bool) -> Surface<3> {
    let bu = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let bv = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut pts = Vec::new();
    let mut ws = Vec::new();
    for i in 0..5_u8 {
        for j in 0..4_u8 {
            let (x, y) = (f64::from(i), f64::from(j));
            pts.push([x * 0.75, y, 0.2 * ((x + 2.0 * y) % 3.0 - 1.0)]);
            ws.push(1.0 + 0.1 * ((x + y) % 2.0));
        }
    }
    if rational {
        Surface::new_rational(bu, bv, &pts, &ws).unwrap()
    } else {
        Surface::new(bu, bv, &pts).unwrap()
    }
}

fn main() {
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let once = Offset::new(SurfaceKind::Nurbs(patch(false)), 0.3).unwrap();
    let kinds: Vec<(&str, Offset)> = vec![
        ("offset of a bicubic patch", once.clone()),
        (
            "offset of a rational bicubic patch",
            Offset::new(SurfaceKind::Nurbs(patch(true)), 0.3).unwrap(),
        ),
        (
            "offset of a cylinder",
            Offset::new(
                SurfaceKind::Cylinder(
                    Cylinder::new([0.0; 3], 3.0, f, ((0.0, 0.5), (0.0, 2.0))).unwrap(),
                ),
                0.5,
            )
            .unwrap(),
        ),
        (
            "offset of an offset",
            Offset::new(SurfaceKind::Offset(once), -0.2).unwrap(),
        ),
    ];
    let squash = TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap());
    for (name, k) in &kinds {
        bench(name, k, &squash);
    }
}

fn bench(name: &str, k: &Offset, t: &TypedTransform) {
    let ((a, b), (c, d)) = k.domain();
    let (u, v) = (a + (b - a) * 0.37, c + (d - c) * 0.61);
    let quarter = ((a, f64::midpoint(a, b)), (c, f64::midpoint(c, d)));
    let small = ((u, u + (b - a) / 32.0), (v, v + (d - c) / 32.0));
    let sides = (Limit::FromRight, Limit::FromRight);
    println!("{name}:");
    time("  enclose (point and both partials)", 1_000, || {
        black_box(k.enclose(u, v, sides, &mut meter()).unwrap());
    });
    time("  partials to order 3 at a point", 1_000, || {
        black_box(
            k.partials_over(((u, u), (v, v)), sides, 3, &mut meter())
                .unwrap(),
        );
    });
    time("  partials to order 3 over a small box", 100, || {
        black_box(k.partials_over(small, sides, 3, &mut meter()).unwrap());
    });
    time("  bound over a quarter of the domain", 100, || {
        black_box(k.bound(quarter, &mut meter()).unwrap());
    });
    time("  normal bound over a quarter", 100, || {
        black_box(k.normal_bound(quarter, &mut meter()).unwrap());
    });
    time("  to_nurbs (within the fit bound)", 3, || {
        black_box(k.to_nurbs(&mut meter()).unwrap());
    });
    time("  to_nurbs_within 1e-3", 1, || {
        black_box(k.to_nurbs_within(1e-3, &mut meter()).unwrap());
    });
    time("  split in u", 1_000, || {
        black_box(
            k.split(Direction::U, f64::midpoint(a, b), &mut meter())
                .unwrap(),
        );
    });
    let cached = k.cached(&mut meter()).unwrap();
    time("  non-uniform scale (of the kept cache)", 100, || {
        black_box(cached.transformed(t, &mut meter()).unwrap());
    });
}
