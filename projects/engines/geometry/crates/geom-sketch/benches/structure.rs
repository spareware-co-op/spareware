//! The benchmark of piece G221 (chapter 30): the structural analysis of 100,000 equations, the
//! board's "in 50 ms".
//!
//! Run with `cargo bench -p geom-sketch --bench structure`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18). The graph has the shape a large sketch gives: a chain where each equation reads three
//! neighbouring variables and a few columns are coupled, from a fixed linear congruential sequence.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_run::{Budget, WorkMeter};
use geom_sketch::structure::{Graph, analyse};

fn sketchlike(n: usize) -> Graph {
    let mut state = 7u64;
    let mut below = |k: usize| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) % k as u64) as usize
    };
    let rows: Vec<[usize; 3]> = (0..n)
        .map(|i| {
            [
                i,
                i.saturating_sub(1 + below(3)),
                i.saturating_sub(1 + below(9)),
            ]
        })
        .collect();
    Graph::new(n, rows.iter().map(|r| &r[..])).unwrap()
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
    let g = sketchlike(100_000);
    time("structural analysis of 100,000 equations", 10, || {
        black_box(analyse(black_box(&g), &mut WorkMeter::new(Budget::UNLIMITED)).unwrap());
    });
}
