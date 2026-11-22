//! The benchmark of piece G346 (chapter 30: "a certified G2 maximum on one NURBS edge in 10 ms"):
//! the curvature jump along the edge where a bicubic NURBS patch with interior knots in both
//! directions rolls off a flat strip, its maximum certified to 10⁻⁶; and the same edge's G0, G1
//! and G3 maxima, for comparison.
//!
//! Run with `cargo bench -p geom-analysis --bench continuity`. It uses the standard library's
//! clock only (wall-clock time on this machine; the reference machine and instruction counts are
//! geom-bench's, piece G18).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::cast_precision_loss,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_analysis::continuity::{self, Ask};
use geom_curve::{CurveKind, Line};
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::class_a::{Measure, Side};
use geom_surface::{Plane, SurfaceKind};

fn time(name: &str, runs: u32, mut f: impl FnMut() -> usize) {
    let pieces = f();
    let start = Instant::now();
    for _ in 0..runs {
        black_box(f());
    }
    let ms = start.elapsed().as_secs_f64() * 1e3 / f64::from(runs);
    println!("{name:<44} {ms:>10.3} ms  ({pieces} pieces)");
}

fn main() {
    let strip = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-1.0, 0.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    // Bicubic, one interior knot in u and three in v; its first two rows of points lie on the
    // strip's plane, so it meets the strip G1, and the third row's heights vary along the edge.
    let bu = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let bv = Basis::new(
        3,
        vec![0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let heights = [0.3, 0.8, 0.5, 1.1, 0.4, 0.9, 0.6];
    let mut pts = Vec::new();
    for i in 0..5 {
        for (j, h) in heights.iter().enumerate() {
            let z = match i {
                0 | 1 => 0.0,
                2 => *h,
                _ => *h * (f64::from(i) - 1.0),
            };
            pts.push([f64::from(i) / 4.0, j as f64 / 6.0, z]);
        }
    }
    let patch = SurfaceKind::Nurbs(Surface::new(bu, bv, &pts).unwrap());
    let pcurve = CurveKind::Line(Line::new([0.0, 0.0], [0.0, 1.0], (0.0, 1.0)).unwrap());
    let sides = [
        Side {
            surface: &strip,
            pcurve: &pcurve,
            turned: false,
        },
        Side {
            surface: &patch,
            pcurve: &pcurve,
            turned: false,
        },
    ];
    for (name, m) in [
        ("G0 maximum, one NURBS edge, 1e-6", Measure::Gap),
        ("G1 maximum, one NURBS edge, 1e-6", Measure::NormalAngle),
        ("G2 maximum, one NURBS edge, 1e-6", Measure::Curvature),
        ("G3 maximum, one NURBS edge, 1e-6", Measure::CurvatureChange),
    ] {
        time(name, 20, || {
            let mut meter = WorkMeter::new(Budget::UNLIMITED);
            continuity::join(sides, Ask::new(m, 1e-6).unwrap(), &mut meter)
                .unwrap()
                .pieces
        });
    }
}
