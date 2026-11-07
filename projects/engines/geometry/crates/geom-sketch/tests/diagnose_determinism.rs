//! The replay hash of piece G224: a fixed corpus of sketches, diagnosed, with the report text and
//! every pivot's size, rank count and group, hashed with SHA-256, must give the same digest on every
//! target (chapters 15.8, 21 and 28.13; the golden store of piece G19).
//!
//! The corpus: 48 chains of points from a fixed linear congruential sequence, each held at its first
//! point, dimensioned along both axes on every link, and given one extra distance that is sometimes
//! the chain's own (redundant) and sometimes not (a conflict); the worked sketches of the unit tests
//! (a free pair, a triangle, three collinear points under a triangle's distances). The digest lives
//! in the store; a change is a bug, or a new definition with a golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::fmt::Write;

use geom_run::{Budget, WorkMeter};
use geom_sketch::diagnose::{Verdict, diagnose};
use geom_sketch::text::parse;
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-sketch/diagnose";

const HEAD: &str = "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n";

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }
}

fn chain(rng: &mut Lcg, points: usize) -> String {
    let mut text = HEAD.to_owned();
    for i in 0..points {
        writeln!(
            text,
            "entity 1.{i} point {} {}",
            i as f64 * 100.0,
            (rng.next() % 40) as f64
        )
        .unwrap();
    }
    writeln!(text, "constraint 2.0 driving 0 fix 1.0 0 0").unwrap();
    writeln!(text, "constraint 2.1 driving 0 fix 1.0 1 0").unwrap();
    for i in 1..points {
        writeln!(
            text,
            "constraint 3.{i} driving 0 across 1.{} 1.{i} 100",
            i - 1
        )
        .unwrap();
        writeln!(
            text,
            "constraint 4.{i} driving 1 upward 1.{} 1.{i} 20",
            i - 1
        )
        .unwrap();
    }
    if points > 2 {
        // The chain runs 100 right and 20 up per link: the straight distance is its own, or not.
        let n = (points - 1) as f64;
        let own = (n * n * 100.0 * 100.0 + n * n * 20.0 * 20.0).sqrt();
        let asked = if rng.next() % 2 == 0 { own } else { own + 50.0 };
        writeln!(
            text,
            "constraint 5.0 driving 2 distance 1.0 1.{} {asked}",
            points - 1
        )
        .unwrap();
    }
    text
}

const WORKED: [&str; 3] = [
    "entity 1.0 point 0 0\nentity 1.1 point 5 5\n",
    "entity 1.0 point 0 0
entity 1.1 point 3000 0
entity 1.2 point 3000 4000
constraint 3.0 driving 0 fix 1.0 0 0
constraint 3.1 driving 0 fix 1.0 1 0
constraint 3.2 driving 0 distance 1.0 1.1 3000
constraint 3.3 driving 0 distance 1.1 1.2 4000
constraint 3.4 driving 0 distance 1.0 1.2 5000
constraint 3.5 driving 0 fix 1.1 1 0
",
    "entity 1.0 point 0 0
entity 1.1 point 3000 0
entity 1.2 point 6000 0
constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 fix 1.1 1 0
constraint 2.3 driving 0 distance 1.0 1.1 3000
constraint 2.4 driving 0 distance 1.1 1.2 3000
constraint 2.5 driving 0 distance 1.0 1.2 6000
",
];

fn corpus() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut rng = Lcg(0x0224);
    let mut texts: Vec<String> = WORKED.iter().map(|w| format!("{HEAD}{w}")).collect();
    for round in 0..48 {
        texts.push(chain(&mut rng, 2 + round % 9));
    }
    for text in texts {
        let sketch = parse(&text).unwrap();
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let report = diagnose(&sketch, 10_000.0, &mut meter).unwrap();
        bytes.extend_from_slice(report.text().as_bytes());
        for p in &report.analysis.pivots {
            bytes.extend_from_slice(&(p.equation as u64).to_le_bytes());
            bytes.extend_from_slice(&p.size.to_bits().to_le_bytes());
            bytes.push(match p.verdict {
                Verdict::Independent => 0,
                Verdict::NearSingular => 1,
                Verdict::Dependent => 2,
            });
        }
        for g in report.redundant.iter().chain(&report.conflicting) {
            bytes.extend_from_slice(&g.defect.to_bits().to_le_bytes());
            for e in &g.equations {
                bytes.extend_from_slice(&(*e as u64).to_le_bytes());
            }
        }
    }
    bytes
}

#[test]
fn diagnose_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let bytes = corpus();
        (sha256(&bytes), bytes)
    });
    println!(
        "geom-sketch diagnose corpus: {} bytes, golden hash {}",
        checked.value.len(),
        hex(sha256(&checked.value))
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target computes, orders or prints differently, \
             or a definition changed in place, which chapter 26.2 forbids"
        )
    });
}
