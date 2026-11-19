// SPDX-License-Identifier: Apache-2.0
//! The replay hash of piece G220a: spline joins solved from a fixed corpus of pulled-off starts,
//! the joins measured at the solution, their senses, and fit-point splines settled, hashed with
//! SHA-256, must give the same digest on every target (chapters 21 and 28.13; the golden store of
//! piece G19).
//!
//! The corpus: 48 cut cubics with the right half's inner points moved by an amount from a fixed
//! linear congruential sequence, each with a join of one of the three orders; each is written in
//! `sketch v1`, read back, solved, and its solved values' bits, the report's counts, the measured
//! gaps and the join's sense recorded. And 8 fit-point splines through six points with the middle
//! one dragged, settled, with their rounds, whether they settled, and every control point's bits.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::manual_midpoint,
    clippy::needless_range_loop,
    clippy::type_complexity,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::float_arithmetic,
    clippy::float_cmp,
    reason = "the corpus generator; the rules are for kernel code"
)]

use std::fmt::Write;

use geom_run::{Budget, WorkMeter};
use geom_sketch::fit::settle;
use geom_sketch::model::ItemId;
use geom_sketch::smooth::{join_sense, solve_and_measure};
use geom_sketch::text::{parse, write};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-sketch/solved-splines";

const HEAD: &str = "sketch v1
plane lattice 0 0 0 1 0 0 0 1 0 q0
";
const BEZIER: &str = "0 0 0 0 1 1 1 1";

fn id(sequence: u64, key: u64) -> ItemId {
    ItemId::new(sequence, key)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn point(text: &mut String, key: u64, x: f64, y: f64) {
    writeln!(text, "entity 1.{key} point {x} {y}").unwrap();
}

fn cubic(text: &mut String, index: u64, first: u64) {
    writeln!(
        text,
        "entity 2.{index} spline 3 4 1.{first} 1.{} 1.{} 1.{} 8 {BEZIER} 4 1 1 1 1",
        first + 1,
        first + 2,
        first + 3
    )
    .unwrap();
}

fn fix(text: &mut String, key: u64, point: u64, x: f64, y: f64) {
    writeln!(
        text,
        "constraint 3.{key} driving 0 fix 1.{point} 0 {x}\nconstraint 3.{} driving 0 fix 1.{point} 1 {y}",
        key + 1
    )
    .unwrap();
}

/// A cubic cut at its middle by de Casteljau's rule: the left half on points 1.0 to 1.3 and the
/// right on 1.4 to 1.7, both ends of the cut at 1.3 and 1.4.
fn halves() -> ([[f64; 2]; 4], [[f64; 2]; 4]) {
    let p = [[0.0, 0.0], [8.0, 16.0], [24.0, 20.0], [32.0, 0.0]];
    let mid = |a: [f64; 2], b: [f64; 2]| [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let (q0, q1, q2) = (mid(p[0], p[1]), mid(p[1], p[2]), mid(p[2], p[3]));
    let (r0, r1) = (mid(q0, q1), mid(q1, q2));
    let s = mid(r0, r1);
    ([p[0], q0, r0, s], [s, r1, q2, p[3]])
}

/// The two halves with the right one's inner points moved off the cut by `shift`, the left half and
/// the right half's far end held, the cut points made one and a join of `order` stated.
fn pulled(shift: f64, order: &str) -> String {
    let (l, r) = halves();
    let mut text = HEAD.to_owned();
    for (k, c) in l.iter().enumerate() {
        point(&mut text, k as u64, c[0], c[1]);
    }
    for (k, c) in r.iter().enumerate() {
        let (dx, dy) = if k == 1 || k == 2 {
            (shift, -shift)
        } else {
            (0.0, 0.0)
        };
        point(&mut text, 4 + k as u64, c[0] + dx, c[1] + dy);
    }
    cubic(&mut text, 0, 0);
    cubic(&mut text, 1, 4);
    let mut key = 0;
    for k in 0..4 {
        fix(&mut text, key, k, l[k as usize][0], l[k as usize][1]);
        key += 2;
    }
    fix(&mut text, key, 7, r[3][0], r[3][1]);
    writeln!(text, "constraint 4.0 driving 0 coincident 1.3 1.4").unwrap();
    writeln!(
        text,
        "constraint 4.1 driving 0 spline-join 2.0 finish 2.1 start {order}"
    )
    .unwrap();
    text
}

/// Fit points on a gentle curve, a fit-point spline through them by `author`, and the sketch that
/// holds the end fit points and the middle one at `(mx, my)`.
fn fitted(mx: f64, my: f64) -> (geom_sketch::model::Sketch, ItemId) {
    let data = [
        [0.0, 0.0],
        [10.0, 6.0],
        [22.0, 9.0],
        [31.0, 7.0],
        [40.0, 1.0],
        [52.0, -3.0],
    ];
    let mut text = HEAD.to_owned();
    for (k, p) in data.iter().enumerate() {
        point(&mut text, k as u64, p[0], p[1]);
    }
    let mut sketch = parse(&text).unwrap();
    let fit_points: Vec<ItemId> = (0..6).map(|k| id(1, k)).collect();
    let made = geom_sketch::fit::author(&sketch, &fit_points, 3, id(2, 0), 1, 100).unwrap();
    for e in made.entities {
        sketch.add_entity(e).unwrap();
    }
    for c in made.constraints {
        sketch.add_constraint(c).unwrap();
    }
    let mut holds = String::new();
    let mut key = 0;
    for (k, x, y) in [(0, 0.0, 0.0), (2, mx, my), (5, 52.0, -3.0)] {
        fix(&mut holds, key, k, x, y);
        key += 2;
    }
    let mut full = geom_sketch::text::write(&sketch).unwrap();
    full.push_str(&holds);
    (parse(&full).unwrap(), id(2, 0))
}

struct Lcg(u64);

impl Lcg {
    fn unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
}

fn bits(bytes: &mut Vec<u8>, value: f64) {
    bytes.extend_from_slice(&value.to_bits().to_le_bytes());
}

fn corpus() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut rng = Lcg(8714);
    for round in 0..48 {
        let order = ["g1", "g2", "g3"][round % 3];
        let text = write(&parse(&pulled(0.5 + 1.5 * rng.unit().abs(), order)).unwrap()).unwrap();
        bytes.extend_from_slice(text.as_bytes());
        let sketch = parse(&text).unwrap();
        let (solved, joins) = solve_and_measure(&sketch, &mut meter()).unwrap();
        for v in solved.sketch.variables().unwrap() {
            bits(&mut bytes, v.value);
        }
        let r = &solved.report;
        for n in [
            r.iterations,
            u64::from(r.steps),
            r.stages as u64,
            r.freedoms as u64,
        ] {
            bytes.extend_from_slice(&n.to_le_bytes());
        }
        bits(&mut bytes, r.largest);
        for j in joins {
            bits(&mut bytes, j.continuity.tangent);
            bits(&mut bytes, j.continuity.curvature.unwrap_or(f64::NAN));
            bits(&mut bytes, j.continuity.rate.unwrap_or(f64::NAN));
        }
        let sense = join_sense(&solved.sketch, ItemId::new(4, 1)).unwrap();
        bytes.push(match sense {
            None => 0,
            Some(geom_sketch::equation::Sense::Positive) => 1,
            Some(geom_sketch::equation::Sense::Negative) => 2,
        });
    }
    for round in 0..8 {
        let (sketch, spline) = fitted(20.0 + 3.0 * f64::from(round), 12.0 + 2.0 * rng.unit());
        let settled = settle(&sketch, spline, &mut meter()).unwrap();
        bytes.extend_from_slice(&u64::from(settled.rounds).to_le_bytes());
        bytes.push(u8::from(settled.settled));
        bits(&mut bytes, settled.moved);
        for v in settled.solved.sketch.variables().unwrap() {
            bits(&mut bytes, v.value);
        }
    }
    bytes
}

#[test]
fn solved_splines_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let bytes = corpus();
        (sha256(&bytes), bytes)
    });
    println!(
        "geom-sketch solved-spline corpus: {} bytes, golden hash {}",
        checked.value.len(),
        hex(sha256(&checked.value))
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target writes, orders or computes differently              (compare this log's digest with another target's), or a definition changed in place,              which chapter 26.2 forbids"
        )
    });
}
