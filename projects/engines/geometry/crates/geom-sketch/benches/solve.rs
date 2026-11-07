//! The benchmark of piece G222 (chapters 15.6 and 30): a family sketch and a floor outline.
//!
//! Run with `cargo bench -p geom-sketch --bench solve`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18). The family sketch is 40 rectangles of five rules each, 200 constraints; the outline
//! is 5,000 segments, each with its own two points, joined by coincidences, with a direction and a
//! length on each. Section 15.6's targets are under 1 ms and under 50 ms.

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

/// 1,250 teeth of four segments: right, up, right, down.
fn outline() -> Sketch {
    let mut text = HEAD.to_owned();
    let steps = [(300.0, 0.0), (0.0, 200.0), (300.0, 0.0), (0.0, -200.0)];
    let (mut x, mut y) = (0.0, 0.0);
    for i in 0..5_000_usize {
        let (dx, dy) = steps[i % 4];
        writeln!(text, "entity 1.{} point {} {}", 2 * i, x + 0.25, y - 0.25).unwrap();
        x += dx;
        y += dy;
        writeln!(
            text,
            "entity 1.{} point {} {}",
            2 * i + 1,
            x - 0.25,
            y + 0.25
        )
        .unwrap();
    }
    for i in 0..5_000_usize {
        writeln!(
            text,
            "entity 2.{i} line 1.{} 1.{} segment",
            2 * i,
            2 * i + 1
        )
        .unwrap();
    }
    writeln!(text, "constraint 3.0 driving 0 fix 1.0 0 0").unwrap();
    writeln!(text, "constraint 3.1 driving 0 fix 1.0 1 0").unwrap();
    for i in 0..5_000_usize {
        let (dx, _) = steps[i % 4];
        let across = dx != 0.0;
        let tag = if across { "horizontal" } else { "vertical" };
        writeln!(text, "constraint 4.{i} driving 0 {tag} 2.{i}").unwrap();
        let (part, size) = if across {
            ("across", 300.0)
        } else if i % 4 == 1 {
            ("upward", 200.0)
        } else {
            ("upward", -200.0)
        };
        let (a, b) = (2 * i, 2 * i + 1);
        writeln!(text, "constraint 5.{i} driving 0 {part} 1.{a} 1.{b} {size}").unwrap();
        if i + 1 < 5_000 {
            writeln!(
                text,
                "constraint 6.{i} driving 0 coincident 1.{b} 1.{}",
                b + 1
            )
            .unwrap();
        }
    }
    parse(&text).unwrap()
}

fn time(name: &str, runs: u32, sketch: &Sketch) {
    let mut last = None;
    let start = Instant::now();
    for _ in 0..runs {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        last = Some(solve(black_box(sketch), &mut meter).unwrap());
    }
    let ms = start.elapsed().as_secs_f64() * 1e3 / f64::from(runs);
    let report = last.unwrap().report;
    println!(
        "{name:<40} {ms:>8.2} ms, {} stages, {} iterations",
        report.stages, report.iterations
    );
}

fn main() {
    let (family, outline) = (family(), outline());
    assert_eq!(family.constraints().len(), 200);
    time("a 200-constraint family sketch", 100, &family);
    time("a 5,000-segment floor outline", 5, &outline);
}
