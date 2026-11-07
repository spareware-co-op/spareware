//! The benchmark of piece G227 (chapters 15.10 and 30): the chained Krawczyk certificates of a
//! 200-constraint family, the board's "in 2 ms".
//!
//! Run with `cargo bench -p geom-sketch --bench certify`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18). The family is the solver's own (`benches/solve.rs`): 40 rectangles of five rules each,
//! 200 constraints, solved and committed once; the timed call is the whole of `certify`, from the
//! structural analysis and the equations to every block's proof.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::fmt::Write;
use std::hint::black_box;
use std::time::Instant;

use geom_run::{Budget, WorkMeter};
use geom_sketch::certify::{Certificate, certify};
use geom_sketch::commit::commit;
use geom_sketch::model::Sketch;
use geom_sketch::solve::solve;
use geom_sketch::text::parse;

const HEAD: &str = "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n";

fn family() -> Sketch {
    let mut text = HEAD.to_owned();
    for i in 0..40_u32 {
        let (a, b) = (10 + 2 * i, 11 + 2 * i);
        let at = f64::from(i) * 1000.0;
        for (key, (x, y)) in [(0.5, 0.5), (401.0, 3.0), (399.0, 298.0), (2.0, 301.0)]
            .iter()
            .enumerate()
        {
            writeln!(text, "entity {a}.{key} point {} {y}", at + x).unwrap();
        }
        for (key, (p, q)) in [(0, 1), (1, 2), (3, 2), (0, 3)].iter().enumerate() {
            writeln!(text, "entity {b}.{key} line {a}.{p} {a}.{q} segment").unwrap();
        }
    }
    for i in 0..40_u32 {
        let (a, b) = (10 + 2 * i, 11 + 2 * i);
        let at = f64::from(i) * 1000.0;
        let c = 100 + i;
        writeln!(text, "constraint {c}.0 driving 0 horizontal {b}.0").unwrap();
        writeln!(text, "constraint {c}.1 driving 0 vertical {b}.1").unwrap();
        writeln!(text, "constraint {c}.2 driving 0 parallel {b}.0 {b}.2").unwrap();
        writeln!(text, "constraint {c}.3 driving 0 fix {a}.0 0 {at}").unwrap();
        writeln!(text, "constraint {c}.4 driving 0 distance {a}.0 {a}.1 400").unwrap();
    }
    parse(&text).unwrap()
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
    let solved = solve(&family(), &mut WorkMeter::new(Budget::UNLIMITED))
        .unwrap()
        .sketch;
    assert_eq!(solved.constraints().len(), 200);
    let committed = commit(&solved, &mut WorkMeter::new(Budget::UNLIMITED)).unwrap();
    let first = certify(&solved, &committed, &mut WorkMeter::new(Budget::UNLIMITED)).unwrap();
    assert!(
        first
            .blocks
            .iter()
            .all(|b| matches!(b.certificate, Certificate::Proven { .. }))
    );
    time("certificates of a 200-constraint family", 20, || {
        black_box(
            certify(
                black_box(&solved),
                black_box(&committed),
                &mut WorkMeter::new(Budget::UNLIMITED),
            )
            .unwrap(),
        );
    });
}
