// SPDX-License-Identifier: Apache-2.0
//! The replay hash of piece G220: spline joins of a fixed corpus, written as text, assembled and
//! hashed with SHA-256, must give the same digest on every target (chapters 21 and 28.13; the golden
//! store of piece G19; the board's standard bar: a piece's cases join the replay corpus).
//!
//! The corpus: 64 pairs of cubic splines on points from a fixed linear congruential sequence, each
//! pair with a join of every order and a spline end along a line. Each is written in `sketch v1`, read
//! back, assembled, and its residuals and Jacobian recorded as their bits, with the measured
//! smoothness of the join, so a target that rounds one step differently changes the digest. The
//! digest lives in the store; a change is a bug, or a new definition with a golden of its own
//! (chapter 26.2).

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
use geom_sketch::model::{ItemId, Smoothness, SplineEnd};
use geom_sketch::smooth::continuity;
use geom_sketch::text::{parse, write};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-sketch/splines";

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

const RULES: [&str; 5] = [
    "spline-join 2.0 finish 2.1 start g1",
    "spline-join 2.0 finish 2.1 start g2",
    "spline-join 2.0 finish 2.1 start g3",
    "spline-join 2.0 start 2.1 finish g3",
    "spline-tangent-line 2.1 finish 2.2",
];

fn drawing(rng: &mut Lcg) -> String {
    let mut text = String::from("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n");
    // Eight control points marching right with a random rise, a join point shared, and a line.
    for k in 0..8_u64 {
        let x = (k as f64) * 10.0 + rng.unit() * 3.0;
        let y = rng.unit() * 20.0;
        writeln!(text, "entity 1.{k} point {x} {y}").unwrap();
    }
    writeln!(
        text,
        "entity 1.8 point 0 -5\nentity 1.9 point 80 {}",
        rng.unit() * 20.0
    )
    .unwrap();
    text.push_str("entity 2.0 spline 3 4 1.0 1.1 1.2 1.3 8 0 0 0 0 1 1 1 1 4 1 1 1 1\n");
    text.push_str("entity 2.1 spline 3 4 1.4 1.5 1.6 1.7 8 0 0 0 0 1 1 1 1 4 1 1 1 1\n");
    text.push_str("entity 2.2 line 1.8 1.9 segment\n");
    text
}

fn corpus() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut rng = Lcg(0x0220);
    for round in 0..64 {
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
        let measured = continuity(
            &back,
            (ItemId::new(2, 0), SplineEnd::Finish),
            (ItemId::new(2, 1), SplineEnd::Start),
            Smoothness::CurvatureRate,
        )
        .unwrap();
        for value in [
            measured.tangent,
            measured.curvature.unwrap(),
            measured.rate.unwrap(),
        ] {
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
    }
    bytes
}

#[test]
fn spline_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let bytes = corpus();
        (sha256(&bytes), bytes)
    });
    println!(
        "geom-sketch spline corpus: {} bytes, golden hash {}",
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
