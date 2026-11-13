//! The benchmark of piece G226 (chapters 15.10 and 30): the checks after the commit of a
//! 1,000-entity sketch, the board's "in 2 ms".
//!
//! Run with `cargo bench -p geom-sketch --bench check`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18). The sketch is the one of `benches/commit.rs`: 125 four-sided shapes of four points and
//! four lines, 1,000 entities, a quarter of them rectangles and the rest parallelograms, with a
//! length stated on every side (500 dimensions the commit can only bound), so the checks enclose 500
//! residuals as well as confirming the exact relations. The equations are built once, as a caller
//! checking commit after commit would, and only the check is timed.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_num::{GridStep, P3};
use geom_run::{Budget, WorkMeter};
use geom_sketch::check::Checker;
use geom_sketch::commit::commit;
use geom_sketch::model::{
    Amount, Constraint, ConstraintKind, Entity, EntityKind, Extent, ItemId, Sketch, Standing,
};
use geom_sketch::plane::{Lattice, PlaneFrame, SketchPlane};

fn shapes(count: u64) -> Sketch {
    let lattice = Lattice::new(P3::new(0, 0, 0).unwrap(), [1, 0, 0], [0, 1, 0]).unwrap();
    let mut s = Sketch::new(SketchPlane::new(PlaneFrame::Lattice(lattice), GridStep::Q0));
    let rule = |key: u64, kind| Constraint {
        id: ItemId::new(3, key),
        kind,
        standing: Standing::Driving,
        priority: 0,
    };
    let mut rules = 0;
    for i in 0..count {
        let at = (i as f64) * 1000.0;
        let level = i % 4 == 0;
        let skew = if level { 0.0 } else { 130.4 };
        let corners = [
            (at + 0.3, 0.4),
            (at + 400.6, if level { 0.2 } else { 100.7 }),
            (at + 400.2 + skew, 300.5),
            (at + 0.1 + skew, if level { 300.9 } else { 200.2 }),
        ];
        for (k, (x, y)) in corners.iter().enumerate() {
            s.add_entity(Entity {
                id: ItemId::new(1, i * 4 + k as u64),
                kind: EntityKind::Point { x: *x, y: *y },
                construction: false,
            })
            .unwrap();
        }
        let l = |k: u64| ItemId::new(2, i * 4 + k);
        let sides = [(0, 1), (1, 2), (3, 2), (0, 3)];
        for (k, (a, b)) in sides.into_iter().enumerate() {
            s.add_entity(Entity {
                id: l(k as u64),
                kind: EntityKind::Line {
                    start: ItemId::new(1, i * 4 + a),
                    end: ItemId::new(1, i * 4 + b),
                    extent: Extent::Segment,
                },
                construction: false,
            })
            .unwrap();
        }
        let kinds = if level {
            vec![
                ConstraintKind::Horizontal { line: l(0) },
                ConstraintKind::Vertical { line: l(1) },
                ConstraintKind::Horizontal { line: l(2) },
                ConstraintKind::Vertical { line: l(3) },
            ]
        } else {
            vec![
                ConstraintKind::Parallel { a: l(0), b: l(2) },
                ConstraintKind::Parallel { a: l(1), b: l(3) },
            ]
        };
        for kind in kinds {
            rules += 1;
            s.add_constraint(rule(rules, kind)).unwrap();
        }
        for (k, (a, b)) in sides.into_iter().enumerate() {
            let (p, q) = (
                corners[usize::try_from(a).unwrap()],
                corners[usize::try_from(b).unwrap()],
            );
            let length = ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
            rules += 1;
            s.add_constraint(rule(
                rules,
                ConstraintKind::Length {
                    line: l(k as u64),
                    length: Amount::Value(length),
                },
            ))
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
    let sketch = shapes(125);
    assert_eq!(sketch.entities().len(), 1000);
    let committed = commit(&sketch, &mut WorkMeter::new(Budget::UNLIMITED)).unwrap();
    let checker = Checker::new(&sketch).unwrap();
    let done = checker
        .check(
            &sketch,
            &committed,
            Checker::fit(),
            &mut WorkMeter::new(Budget::UNLIMITED),
        )
        .unwrap();
    assert_eq!(done.holds.len(), sketch.constraints().len());
    time("checks of a 1,000-entity commit", 50, || {
        black_box(
            checker
                .check(
                    black_box(&sketch),
                    black_box(&committed),
                    Checker::fit(),
                    &mut WorkMeter::new(Budget::UNLIMITED),
                )
                .unwrap(),
        );
    });
}
