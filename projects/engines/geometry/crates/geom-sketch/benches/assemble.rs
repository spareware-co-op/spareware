//! The benchmark of piece G217 (chapter 30): 10,000 constraints assembled, the board's "in 2 ms".
//!
//! Run with `cargo bench -p geom-sketch --bench assemble`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18). The sketch is a chain of 5,001 points joined by distances, with a fixed value on each
//! other coordinate: the shape a long dimensioned outline gives.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_num::{GridStep, P3};
use geom_sketch::equation::Assembly;
use geom_sketch::model::{
    Constraint, ConstraintKind, Entity, EntityKind, ItemId, Sketch, Standing,
};
use geom_sketch::plane::{Lattice, PlaneFrame, SketchPlane};

fn chain() -> Sketch {
    let lattice = Lattice::new(P3::new(0, 0, 0).unwrap(), [1, 0, 0], [0, 1, 0]).unwrap();
    let mut s = Sketch::new(SketchPlane::new(PlaneFrame::Lattice(lattice), GridStep::Q0));
    let at = |i: u64| ItemId::new(1, i);
    for i in 0..=5_000 {
        s.add_entity(Entity {
            id: at(i),
            kind: EntityKind::Point {
                x: i as f64 * 3.0,
                y: (i % 7) as f64 * 2.0,
            },
            construction: false,
        })
        .unwrap();
    }
    for i in 0..5_000 {
        for (key, kind) in [
            (
                0,
                ConstraintKind::Distance {
                    a: at(i),
                    b: at(i + 1),
                    distance: 3.0,
                },
            ),
            (
                1,
                ConstraintKind::Fix {
                    entity: at(i),
                    slot: 1,
                    value: 0.0,
                },
            ),
        ] {
            s.add_constraint(Constraint {
                id: ItemId::new(2 + i, key),
                kind,
                standing: Standing::Driving,
                priority: 0,
            })
            .unwrap();
        }
    }
    s
}

fn time(name: &str, runs: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let ms = start.elapsed().as_secs_f64() * 1e3 / f64::from(runs);
    println!("{name:<52} {ms:>8.2} ms");
}

fn main() {
    let s = chain();
    assert_eq!(s.constraints().len(), 10_000);
    time("the pattern of 10,000 constraints", 10, || {
        black_box(Assembly::new(black_box(&s)).unwrap());
    });
    let mut a = Assembly::new(&s).unwrap();
    time("10,000 constraints assembled", 100, || {
        a.evaluate(black_box(&s)).unwrap();
    });
    black_box(a.residuals());
}
