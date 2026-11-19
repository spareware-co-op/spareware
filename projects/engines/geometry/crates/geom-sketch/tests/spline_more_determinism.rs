// SPDX-License-Identifier: Apache-2.0
//! The replay hash of the rest of piece G220: rational splines, splines against lines and arcs to
//! every order, arc length with its proven bound, a pinned point and fit-point splines, of a fixed
//! corpus, written as text, assembled and hashed with SHA-256, must give the same digest on every
//! target (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus: 48 drawings of two rational quadratic splines (weights from a fixed linear
//! congruential sequence), an arc and a line, each with one rule of the new kinds; each is written
//! in `sketch v1`, read back, assembled, and its residuals and Jacobian recorded as their bits,
//! with the arc length of both splines, value and error bound. And 16 fit-point splines through
//! seven points, with their control points, parameters and certified error. A target that rounds one
//! step differently changes the digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::fmt::Write;

use geom_sketch::equation::Assembly;
use geom_sketch::fit::interpolate;
use geom_sketch::length::spline_length;
use geom_sketch::model::ItemId;
use geom_sketch::text::{parse, write};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-sketch/splines-more";

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

const RULES: [&str; 8] = [
    "spline-join 2.0 finish 2.1 start g1",
    "spline-join 2.0 finish 2.1 start g3",
    "spline-against 2.0 start 2.2 start g2",
    "spline-against 2.1 finish 2.2 start g3",
    "spline-against 2.1 start 2.3 finish g3",
    "spline-length 2.0 150",
    "spline-length 2.1 90",
    "spline-through 2.1 1.9 0.35",
];

fn drawing(rng: &mut Lcg) -> String {
    let mut text = String::from("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n");
    // Two quadratics marching right, the second starting where the first ends; a line and an arc.
    let xs = [0.0, 30.0, 60.0, 60.0, 90.0, 120.0];
    for (k, x) in xs.iter().enumerate() {
        let y = if k == 3 { 0.0 } else { rng.unit() * 25.0 };
        writeln!(text, "entity 1.{k} point {} {}", x + rng.unit() * 4.0, y).unwrap();
    }
    writeln!(
        text,
        "entity 1.6 point 0 0\nentity 1.7 point -40 {}",
        rng.unit() * 15.0
    )
    .unwrap();
    writeln!(
        text,
        "entity 1.8 point 10 40\nentity 1.9 point 61 {}",
        rng.unit() * 20.0
    )
    .unwrap();
    writeln!(text, "entity 1.10 point 30 -20\nentity 1.11 point 60 0").unwrap();
    let (w1, w2) = (1.0 + rng.unit() * 0.4, 1.0 + rng.unit() * 0.4);
    writeln!(
        text,
        "entity 2.0 spline 2 3 1.0 1.1 1.2 6 0 0 0 1 1 1 3 1 {w1} 1"
    )
    .unwrap();
    writeln!(
        text,
        "entity 2.1 spline 2 3 1.3 1.4 1.5 6 0 0 0 1 1 1 3 1 {w2} 1"
    )
    .unwrap();
    text.push_str("entity 2.2 line 1.6 1.7 segment\n");
    text.push_str("entity 2.3 arc 1.8 20 1.10 1.11\n");
    text
}

fn corpus() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut rng = Lcg(0x0220_0220);
    for round in 0..48 {
        let base = drawing(&mut rng);
        let rule = RULES[round % RULES.len()];
        let text =
            write(&parse(&format!("{base}constraint 3.0 driving 0 {rule}\n")).unwrap()).unwrap();
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
        for k in 0..2 {
            let length = spline_length(&back, ItemId::new(2, k)).unwrap();
            for value in [length.value, length.error] {
                bytes.extend_from_slice(&value.to_bits().to_le_bytes());
            }
        }
    }
    for _ in 0..16 {
        let data: Vec<[f64; 2]> = (0..7)
            .map(|k| [f64::from(k) * 10.0 + rng.unit() * 3.0, rng.unit() * 20.0])
            .collect();
        let fit = interpolate(&data, 3).unwrap();
        for p in &fit.points {
            for v in p {
                bytes.extend_from_slice(&v.to_bits().to_le_bytes());
            }
        }
        for v in fit.knots.iter().chain(&fit.parameters).chain([&fit.error]) {
            bytes.extend_from_slice(&v.to_bits().to_le_bytes());
        }
    }
    bytes
}

#[test]
fn spline_more_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let bytes = corpus();
        (sha256(&bytes), bytes)
    });
    println!(
        "geom-sketch more-spline corpus: {} bytes, golden hash {}",
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
