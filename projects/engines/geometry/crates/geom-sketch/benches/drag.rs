//! The benchmark of piece G228 (chapters 15.9 and 30): a frame of a drag in a 1,000-entity sketch,
//! the board's "4 ms frame at 1,000 entities".
//!
//! Run with `cargo bench -p geom-sketch --bench drag`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18). The sketch is the commit benchmark's: 125 four-sided shapes of four points and four
//! lines, a quarter of them rectangles and the rest parallelograms. A corner of one rectangle is
//! dragged along a path of 200 frames; the figure is the mean of one frame, from the cursor to the
//! returned sketch.

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
use geom_sketch::drag::{Dragger, Status};
use geom_sketch::model::{
    Constraint, ConstraintKind, Entity, EntityKind, Extent, ItemId, Sketch, Standing,
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
        let p = |k: u64| ItemId::new(1, i * 4 + k);
        let l = |k: u64| ItemId::new(2, i * 4 + k);
        for (k, (a, b)) in [(0, 1), (1, 2), (3, 2), (0, 3)].into_iter().enumerate() {
            s.add_entity(Entity {
                id: l(k as u64),
                kind: EntityKind::Line {
                    start: p(a),
                    end: p(b),
                    extent: Extent::Segment,
                },
                construction: false,
            })
            .unwrap();
        }
        let kinds = if level {
            [
                ConstraintKind::Horizontal { line: l(0) },
                ConstraintKind::Vertical { line: l(1) },
                ConstraintKind::Horizontal { line: l(2) },
                ConstraintKind::Vertical { line: l(3) },
            ]
        } else {
            [
                ConstraintKind::Parallel { a: l(0), b: l(2) },
                ConstraintKind::Parallel { a: l(1), b: l(3) },
                ConstraintKind::Collinear { a: l(0), b: l(0) },
                ConstraintKind::Perpendicular { a: l(0), b: l(0) },
            ]
        };
        for (n, kind) in kinds.into_iter().enumerate() {
            if !level && n >= 2 {
                continue;
            }
            rules += 1;
            s.add_constraint(rule(rules, kind)).unwrap();
        }
    }
    s
}

fn main() {
    let sketch = shapes(125);
    assert_eq!(sketch.entities().len(), 1000);
    let corner = ItemId::new(1, 4 * 40 + 2);
    let mut drag =
        Dragger::begin(&sketch, &[corner], &mut WorkMeter::new(Budget::UNLIMITED)).unwrap();
    let frames = 200_u32;
    let mut held = 0;
    let start = Instant::now();
    for i in 0..frames {
        let t = f64::from(i);
        let cursor = [[40_000.0 + 400.0 + t * 0.9, 300.0 + t * 0.6]];
        let frame = drag
            .frame(&cursor, &mut WorkMeter::new(Budget::UNLIMITED))
            .unwrap();
        if frame.status == Status::Held {
            held += 1;
        }
        black_box(&frame);
    }
    let ms = start.elapsed().as_secs_f64() * 1e3 / f64::from(frames);
    assert_eq!(held, frames);
    println!("a drag frame in a 1,000-entity sketch {ms:>8.2} ms");
}
