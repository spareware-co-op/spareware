//! The benchmark of piece G62 (chapter 30): the bulge polyline, trimmed, composite and offset
//! kinds' main operations, one timing each.
//!
//! Run with `cargo bench -p geom-curve --bench compound`. It uses the standard library's clock only
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
    Basis, BulgePolyline, Circle, Composite, CurveKind, Frame, Line, Offset, OffsetLaw, ParamCurve,
    Trimmed,
};
use geom_nurbs::{Basis as Knots, Curve, Limit};
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
    let poly = BulgePolyline::new(
        vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
        vec![0.0, 0.5, 1.0, -2.0],
        Some(vec![(0.5, 1.0); 4]),
        true,
    )
    .unwrap();
    let mut k = vec![0.0; 4];
    k.extend((1..8).map(f64::from));
    k.extend([8.0; 4]);
    let basis = Knots::new(3, k).unwrap();
    let pts: Vec<[f64; 3]> = (0..basis.count())
        .map(|i| {
            let x = f64::from(u32::try_from(i).unwrap());
            [x, (x * 0.7) % 3.0, (x * 0.3) % 2.0]
        })
        .collect();
    let spline = Curve::new(basis, &pts).unwrap();
    let trimmed = Trimmed::new(Basis::Nurbs(spline), (1.25, 6.5)).unwrap();
    let line = Line::new([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
    let circle = Circle::full([1.0, 1.0, 0.0], 1.0, Frame::standard().unwrap()).unwrap();
    let knots = Knots::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let tail = Curve::new(knots, &[[2.0, 1.0, 0.0], [2.0, 3.0, 1.0], [0.0, 3.0, 2.0]]).unwrap();
    let composite = Composite::new(vec![
        Trimmed::whole(Basis::Line(line)).unwrap(),
        Trimmed::new(Basis::Circle(circle), (1.5, 2.0)).unwrap(),
        Trimmed::whole(Basis::Nurbs(tail)).unwrap(),
    ])
    .unwrap();
    let base = Circle::new([0.0, 0.0], 5.0, Frame::standard().unwrap(), (0.0, 1.0)).unwrap();
    let offset = Offset::new(
        Trimmed::whole(Basis::Circle(base)).unwrap(),
        OffsetLaw::Planar { distance: 1.25 },
    )
    .unwrap();
    let kinds2: Vec<(&str, CurveKind<2>)> = vec![
        ("bulge polyline, 4 segments", CurveKind::Bulge(poly)),
        ("offset of a half circle", CurveKind::Offset(offset)),
    ];
    let kinds3: Vec<(&str, CurveKind<3>)> = vec![
        ("trimmed cubic, 8 spans", CurveKind::Trimmed(trimmed)),
        (
            "composite: line, arc, quadratic",
            CurveKind::Composite(composite),
        ),
    ];
    for (name, k) in &kinds2 {
        bench(name, k);
    }
    for (name, k) in &kinds3 {
        bench(name, k);
    }
}

fn bench<const D: usize>(name: &str, k: &CurveKind<D>) {
    let (a, b) = k.domain();
    let t = a + (b - a) * 0.37;
    println!("{name}:");
    time("  enclose (point and two derivatives)", 10_000, || {
        black_box(k.enclose(t, Limit::FromRight, 2, &mut meter()).unwrap());
    });
    time("  bound over half the domain", 10_000, || {
        black_box(k.bound((a, a + (b - a) * 0.5), &mut meter()).unwrap());
    });
    time("  to_nurbs", 100, || {
        black_box(k.to_nurbs(&mut meter()).unwrap());
    });
    time("  split in the middle", 1_000, || {
        black_box(k.split(f64::midpoint(a, b), &mut meter()).unwrap());
    });
}
