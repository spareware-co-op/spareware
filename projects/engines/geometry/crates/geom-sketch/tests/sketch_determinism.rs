//! The replay hash of piece G217: sketches of a fixed corpus, written as text, assembled and hashed
//! with SHA-256, must give the same digest on every target (chapters 21 and 28.13; the golden store
//! of piece G19; the board's standard bar: a piece's cases join the replay corpus).
//!
//! The corpus: 64 chains of points from a fixed linear congruential sequence, joined by distances,
//! point-line distances on a stored side, and fixed values. Each is written in `sketch v1`, read
//! back, assembled, and its residuals and Jacobian recorded as their bits, so a target that rounds
//! one step differently, or orders one item differently, changes the digest. The digest lives in the
//! store; a change is a bug, or a new definition with a golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::similar_names,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_num::{GridStep, P3};
use geom_sketch::equation::Assembly;
use geom_sketch::model::{
    Constraint, ConstraintKind, Entity, EntityKind, Extent, ItemId, Side, Sketch, Standing,
};
use geom_sketch::plane::{Lattice, PlaneFrame, SketchPlane};
use geom_sketch::text::{parse, write};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-sketch/assembly";

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }
    fn unit(&mut self) -> f64 {
        (self.next() as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
}

fn chain(rng: &mut Lcg, size: u64) -> Sketch {
    let lattice = Lattice::new(P3::new(0, 0, 0).unwrap(), [1, 0, 0], [0, 1, 0]).unwrap();
    let mut s = Sketch::new(SketchPlane::new(PlaneFrame::Lattice(lattice), GridStep::Q0));
    let point = |i: u64| ItemId::new(1, i);
    let line = |i: u64| ItemId::new(2, i);
    for i in 0..size {
        s.add_entity(Entity {
            id: point(i),
            kind: EntityKind::Point {
                x: rng.unit() * 1000.0,
                y: rng.unit() * 1000.0,
            },
            construction: false,
        })
        .unwrap();
    }
    for i in 1..size {
        s.add_entity(Entity {
            id: line(i),
            kind: EntityKind::Line {
                start: point(i - 1),
                end: point(i),
                extent: Extent::Segment,
            },
            construction: false,
        })
        .unwrap();
    }
    for i in 1..size {
        let side = if i % 2 == 0 { Side::Left } else { Side::Right };
        for (key, kind) in [
            (
                0,
                ConstraintKind::Distance {
                    a: point(i - 1),
                    b: point(i),
                    distance: 1.0 + rng.unit().abs() * 100.0,
                },
            ),
            (
                1,
                ConstraintKind::PointLine {
                    point: point(i % size),
                    line: line(1 + (i * 3) % (size - 1)),
                    distance: 1.0 + rng.unit().abs(),
                    side,
                },
            ),
            (
                2,
                ConstraintKind::Fix {
                    entity: point(i),
                    slot: (i % 2) as u8,
                    value: rng.unit(),
                },
            ),
        ] {
            s.add_constraint(Constraint {
                id: ItemId::new(3 + i, key),
                kind,
                standing: Standing::Driving,
                priority: (i % 3) as u8,
            })
            .unwrap();
        }
    }
    s
}

fn corpus() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut rng = Lcg(0x0217);
    for round in 0..64 {
        let sketch = chain(&mut rng, 2 + round % 13);
        let text = write(&sketch).unwrap();
        bytes.extend_from_slice(text.as_bytes());
        let back = parse(&text).unwrap();
        let mut assembly = Assembly::new(&back).unwrap();
        assembly.evaluate(&back).unwrap();
        for value in assembly.residuals() {
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        let jacobian = assembly.jacobian();
        for j in 0..jacobian.cols() {
            let (rows, values) = jacobian.column(j).unwrap();
            for (row, value) in rows.iter().zip(values) {
                bytes.extend_from_slice(&(*row as u64).to_le_bytes());
                bytes.extend_from_slice(&value.to_bits().to_le_bytes());
            }
        }
    }
    bytes
}

#[test]
fn sketch_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let bytes = corpus();
        (sha256(&bytes), bytes)
    });
    println!(
        "geom-sketch corpus: {} bytes, golden hash {}",
        checked.value.len(),
        hex(sha256(&checked.value))
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target writes, orders or computes differently \
             (compare this log's digest with another target's), or a definition changed in place, \
             which chapter 26.2 forbids"
        )
    });
}
