//! Piece G414b's benchmark: the DEFLATE reader's throughput on a mesh document laid out as 3MF lays
//! out its model part, compressed as a zip writer would (`miniz_oxide` at level 6, zlib's default),
//! the commonest large entry an importer inflates; and, beside it, `miniz_oxide` 0.9.1's own
//! decoder on the same stream, so the cost of the port's checks is measured, not guessed.
//!
//! Run with `cargo bench -p geom-exchange --bench inflate`. It uses the standard library's clock
//! only, like the other benchmarks beside it. Coordinates come from the kernel's one generator,
//! `geom_num::rng` (Philox4x32-10), with a fixed seed and stream, so every run measures the same
//! document.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::unwrap_used,
    reason = "a benchmark is development code, like a test; the rules are for kernel code"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "a benchmark measures wall-clock time by design (Instant::now and elapsed); the time is printed, never fed into anything the kernel computes"
)]

use std::fmt::Write as _;
use std::hint::black_box;
use std::time::Instant;

use geom_exchange::inflate::{InflateBudget, inflate_slice};
use geom_num::rng::{Philox, Seed, StreamId};
use geom_run::{Budget, WorkMeter};

const VERTICES: u64 = 200_000;
const TRIANGLES: u64 = 400_000;
const RUNS: u32 = 5;

fn document() -> String {
    let mut stream = Philox::new(Seed(0x6414_B0B1), StreamId(0)).stream();
    let mut doc = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\">\n\
         <resources><object id=\"1\" type=\"model\"><mesh>\n<vertices>\n",
    );
    for _ in 0..VERTICES {
        let mut c = || stream.below(2_000_000).unwrap() as f64 / 1_000.0 - 1_000.0;
        let (x, y, z) = (c(), c(), c());
        writeln!(doc, "<vertex x=\"{x}\" y=\"{y}\" z=\"{z}\"/>").unwrap();
    }
    doc.push_str("</vertices>\n<triangles>\n");
    for _ in 0..TRIANGLES {
        let mut v = || stream.below(VERTICES).unwrap();
        let (a, b, c) = (v(), v(), v());
        writeln!(doc, "<triangle v1=\"{a}\" v2=\"{b}\" v3=\"{c}\"/>").unwrap();
    }
    doc.push_str("</triangles>\n</mesh></object></resources>\n</model>\n");
    doc
}

/// The best of [`RUNS`] timings of `f`, in seconds.
fn best(mut f: impl FnMut()) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..RUNS {
        let start = Instant::now();
        f();
        best = best.min(start.elapsed().as_secs_f64());
    }
    best
}

fn main() {
    let doc = document();
    let stream = miniz_oxide::deflate::compress_to_vec(doc.as_bytes(), 6);
    let bytes = doc.len() as f64;
    let budget = InflateBudget {
        output: doc.len(),
        ..InflateBudget::DEFAULT
    };
    let ours = best(|| {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let out = inflate_slice(&stream, &budget, &mut meter).unwrap();
        assert_eq!(out.len(), doc.len());
        black_box(out);
    });
    let theirs = best(|| {
        let out = miniz_oxide::inflate::decompress_to_vec_with_limit(&stream, doc.len()).unwrap();
        black_box(out);
    });
    println!(
        "3MF mesh part, {:.1} MB from {:.1} MB: inflate {:>7.1} MB/s, miniz_oxide 0.9.1 {:>7.1} MB/s, \
         {:.2}x its time (best of {RUNS})",
        bytes / 1e6,
        stream.len() as f64 / 1e6,
        bytes / 1e6 / ours,
        bytes / 1e6 / theirs,
        ours / theirs,
    );
}
