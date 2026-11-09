//! The benchmark of piece G253 (chapter 30): extending a 1,000-span bicubic NURBS surface, one
//! timing each: the natural continuation alone (geom-nurbs), and the full extension with its
//! normal certificate, past a side across the 1,000 spans and past one along them; and a
//! cylinder widened by its definition.
//!
//! Run with `cargo bench -p geom-surface --bench extend`. It uses the standard library's clock
//! only (wall-clock time on this machine; the reference machine and instruction counts are
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

use geom_curve::Frame;
use geom_nurbs::{Basis, Direction, End, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::extend::extend;
use geom_surface::{Cylinder, Reach, Side, SurfaceKind};

fn time(name: &str, runs: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
    println!("{name:<52} {us:>12.3} µs");
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A clamped bicubic graph over 1,000 spans in u and one in v, with fixed waves.
fn long_patch() -> Surface<3> {
    let spans = 1000_u32;
    let mut ku = vec![0.0; 4];
    for i in 1..spans {
        ku.push(f64::from(i));
    }
    ku.extend([f64::from(spans); 4]);
    let bu = Basis::new(3, ku).unwrap();
    let bv = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut pts = Vec::new();
    for i in 0..bu.count() {
        for j in 0..bv.count() {
            let (x, y) = (i as f64, j as f64);
            pts.push([x, y, 0.2 * ((x + 2.0 * y) % 3.0 - 1.0)]);
        }
    }
    Surface::new(bu, bv, &pts).unwrap()
}

fn main() {
    let s = long_patch();
    let kind = SurfaceKind::Nurbs(s.clone());
    time("natural continuation past u = max (4 lines)", 200, || {
        black_box(
            s.extend(Direction::U, End::High, 2.0, &mut meter())
                .unwrap(),
        );
    });
    time(
        "natural continuation past v = max (1,003 lines)",
        50,
        || {
            black_box(
                s.extend(Direction::V, End::High, 0.5, &mut meter())
                    .unwrap(),
            );
        },
    );
    time("extend past u = max, certified", 20, || {
        black_box(extend(&kind, Side::UMax, 2.0, Reach::Exactly, &mut meter()).unwrap());
    });
    time("extend past v = max, certified", 5, || {
        black_box(extend(&kind, Side::VMax, 0.5, Reach::Exactly, &mut meter()).unwrap());
    });
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let cyl = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 1.0, frame, ((0.0, 1.0), (0.0, 1.0))).unwrap(),
    );
    time("cylinder by its definition", 10_000, || {
        black_box(extend(&cyl, Side::VMax, 3.0, Reach::Exactly, &mut meter()).unwrap());
    });
}
