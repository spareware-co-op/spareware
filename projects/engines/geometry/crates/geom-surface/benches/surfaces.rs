//! The benchmark of piece G65 (chapter 30): the analytic and swept surface kinds' main
//! operations, one timing each.
//!
//! Run with `cargo bench -p geom-surface --bench surfaces`. It uses the standard library's clock
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

use geom_curve::{Basis, Circle, Frame, Line, Trimmed};
use geom_math::{Transform, TypedTransform};
use geom_nurbs::{Direction, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, Extrusion, ParamSurface, Plane, Revolution, Ruled, Sphere, SurfaceKind, Torus,
};

fn time(name: &str, runs: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
    println!("{name:<44} {us:>10.3} µs");
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn main() {
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let line = Trimmed::whole(Basis::Line(
        Line::new([2.0, 0.0, 1.0], [1.0, 0.5, 2.0], (0.0, 1.0)).unwrap(),
    ))
    .unwrap();
    let arc = Trimmed::whole(Basis::Circle(
        Circle::new([0.0; 3], 2.0, Frame::standard().unwrap(), (0.0, 1.0)).unwrap(),
    ))
    .unwrap();
    let kinds: Vec<(&str, SurfaceKind)> = vec![
        (
            "plane",
            SurfaceKind::Plane(
                Plane::new(
                    [1.0, 2.0, 3.0],
                    [1.0, 0.0, 0.5],
                    [0.0, 1.0, 0.0],
                    ((0.0, 2.0), (0.0, 2.0)),
                )
                .unwrap(),
            ),
        ),
        (
            "cylinder, three quarter turns",
            SurfaceKind::Cylinder(
                Cylinder::new([0.0; 3], 3.0, f, ((0.0, 1.5), (0.0, 4.0))).unwrap(),
            ),
        ),
        (
            "cone",
            SurfaceKind::Cone(Cone::new([0.0; 3], 3.0, 0.4, f, ((0.0, 1.5), (0.0, 4.0))).unwrap()),
        ),
        (
            "sphere zone",
            SurfaceKind::Sphere(Sphere::new([0.0; 3], 3.0, f, ((0.0, 1.5), (-0.4, 0.3))).unwrap()),
        ),
        (
            "torus",
            SurfaceKind::Torus(
                Torus::new([0.0; 3], 5.0, 2.0, f, ((0.0, 1.5), (-1.0, 1.0))).unwrap(),
            ),
        ),
        (
            "revolution of a line",
            SurfaceKind::Revolution(
                Revolution::new(line.clone(), [0.0; 3], f, (0.0, 1.5)).unwrap(),
            ),
        ),
        (
            "extrusion of an arc",
            SurfaceKind::Extrusion(
                Extrusion::new(arc.clone(), [0.0, 1.0, 4.0], (0.0, 2.0)).unwrap(),
            ),
        ),
        (
            "ruled, line to arc",
            SurfaceKind::Ruled(Ruled::new(line, arc, (0.0, 1.0)).unwrap()),
        ),
    ];
    let squash = TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap());
    for (name, k) in &kinds {
        bench(name, k, &squash);
    }
}

fn bench(name: &str, k: &SurfaceKind, t: &TypedTransform) {
    let ((a, b), (c, d)) = k.domain();
    let (u, v) = (a + (b - a) * 0.37, c + (d - c) * 0.61);
    let half = ((a, f64::midpoint(a, b)), (c, f64::midpoint(c, d)));
    let sides = (Limit::FromRight, Limit::FromRight);
    println!("{name}:");
    time("  enclose (point and both partials)", 10_000, || {
        black_box(k.enclose(u, v, sides, &mut meter()).unwrap());
    });
    time("  bound over a quarter of the domain", 1_000, || {
        black_box(k.bound(half, &mut meter()).unwrap());
    });
    time("  normal bound over a quarter", 1_000, || {
        black_box(k.normal_bound(half, &mut meter()).unwrap());
    });
    time("  to_nurbs", 100, || {
        black_box(k.to_nurbs(&mut meter()).unwrap());
    });
    time("  split in u", 1_000, || {
        black_box(
            k.split(Direction::U, f64::midpoint(a, b), &mut meter())
                .unwrap(),
        );
    });
    time("  non-uniform scale", 1_000, || {
        black_box(k.transformed(t, &mut meter()).unwrap());
    });
}
