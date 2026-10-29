//! The benchmark of piece G61 (chapter 30): the analytic kinds' main operations, one timing each,
//! in 3D.
//!
//! Run with `cargo bench -p geom-curve --bench analytic`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_curve::{
    Axes, Circle, CurveKind, Ellipse, Frame, Helix, HelixAxes, Hyperbola, Involute, Line, Parabola,
    ParamCurve, Polynomial,
};
use geom_math::{Axis, Transform, TypedTransform};
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};

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
    let frame = Frame::new([3.0, 4.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let c = [1.0, -2.0, 3.5];
    let kinds: Vec<(&str, CurveKind<3>)> = vec![
        (
            "line",
            CurveKind::Line(Line::new(c, [1.0, 0.5, -0.25], (0.0, 4.0)).unwrap()),
        ),
        (
            "circle",
            CurveKind::Circle(Circle::full(c, 5.0, frame).unwrap()),
        ),
        (
            "ellipse",
            CurveKind::Ellipse(
                Ellipse::new(
                    c,
                    Axes::Vectors {
                        u: [4.0, 1.0, 0.5],
                        v: [-1.0, 2.0, 0.25],
                    },
                    (0.0, 2.0),
                )
                .unwrap(),
            ),
        ),
        (
            "hyperbola",
            CurveKind::Hyperbola(
                Hyperbola::new(
                    c,
                    Axes::Frame {
                        frame,
                        a: 2.0,
                        b: 1.0,
                    },
                    (-2.0, 2.0),
                )
                .unwrap(),
            ),
        ),
        (
            "parabola",
            CurveKind::Parabola(Parabola::focal(c, 0.75, frame, (-3.0, 3.0)).unwrap()),
        ),
        (
            "helix, 4 turns",
            CurveKind::Helix(
                Helix::new(
                    c,
                    HelixAxes::Frame {
                        frame,
                        radius: 2.0,
                        pitch: 0.5,
                    },
                    (0.0, 8.0),
                )
                .unwrap(),
            ),
        ),
        (
            "polynomial, degree 5",
            CurveKind::Polynomial(
                Polynomial::new(
                    vec![
                        [1.0, 0.0, 0.0],
                        [0.5, 1.0, 0.0],
                        [0.0, -0.25, 1.0],
                        [0.125, 0.0, 0.5],
                        [0.0, 0.0625, 0.0],
                        [-0.03125, 0.0, 0.0],
                    ],
                    (-1.0, 2.0),
                )
                .unwrap(),
            ),
        ),
        (
            "involute",
            CurveKind::Involute(Involute::new(c, 3.0, frame, (0.0, 2.0)).unwrap()),
        ),
    ];
    let turn = TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap());
    for (name, k) in &kinds {
        let (a, b) = k.domain();
        let t = a + (b - a) * 0.37;
        println!("{name}:");
        time("  point", 100_000, || {
            black_box(k.point(t, Limit::FromRight).unwrap());
        });
        time("  enclose (point and two derivatives)", 100_000, || {
            black_box(k.enclose(t, Limit::FromRight, 2, &mut meter()).unwrap());
        });
        time("  bound over half the domain", 10_000, || {
            black_box(k.bound((a, a + (b - a) * 0.5), &mut meter()).unwrap());
        });
        time("  to_nurbs", 1_000, || {
            black_box(k.to_nurbs(&mut meter()).unwrap());
        });
        time("  transformed (certified rotation)", 10_000, || {
            black_box(k.transformed(&turn, &mut meter()).unwrap());
        });
    }
}
