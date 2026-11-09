//! Piece G413's benchmark: wall-clock time per number for the exchange basics an importer and an
//! exporter run on every value of a file: decimal and double lengths to grid in metres, inches and
//! US survey feet, grid values written back as decimals, and reals read and written shortest.
//!
//! Run with `cargo bench -p geom-exchange`. It uses the standard library's clock only, like
//! geom-math's frame benchmark, and moves to `geom-bench` with the others. Inputs come from the
//! kernel's one generator, `geom_num::rng` (Philox4x32-10), with a fixed seed and stream, so every
//! run measures the same work.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::unwrap_used,
    clippy::cast_lossless,
    clippy::indexing_slicing,
    reason = "a benchmark is development code, like a test; the rules are for kernel code"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "a benchmark measures wall-clock time by design (Instant::now and elapsed); the time is printed, never fed into anything the kernel computes"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_exchange::unit::{self, NamedLength};
use geom_exchange::{grid, real, write};
use geom_math::{Decimal, LinearUnit};
use geom_num::grid::GridStep;
use geom_num::rng::{Philox, Seed, StreamId};

const VALUES: usize = 100_000;

fn time<T>(name: &str, n: usize, mut f: impl FnMut(usize) -> T) {
    let start = Instant::now();
    for i in 0..n {
        black_box(f(i));
    }
    let ns = start.elapsed().as_nanos() as f64 / n as f64;
    println!("{name:<44} {ns:>10.1} ns per value");
}

fn main() {
    let mut stream = Philox::new(Seed(0x6413_BE7C), StreamId(0)).stream();
    // Decimals of up to 12 significant digits with three to six places: survey and model values.
    let decimals: Vec<Decimal> = (0..VALUES)
        .map(|_| {
            let m = stream.below(1_000_000_000_000).unwrap() as i128 - 500_000_000_000;
            let e = -(3 + stream.below(4).unwrap() as i32);
            Decimal::new(m, e).unwrap()
        })
        .collect();
    let doubles: Vec<f64> = decimals
        .iter()
        .map(|d| real::nearest(*d).unwrap())
        .collect();
    let counts: Vec<i64> = (0..VALUES)
        .map(|_| stream.below(1 << 40).unwrap() as i64 - (1 << 39))
        .collect();
    let mm = unit::metre(Some(unit::SiPrefix::Milli)).unwrap();
    let inch = NamedLength::Inch.unit().unwrap();
    let survey = NamedLength::UsSurveyFoot.unit().unwrap();
    let q0 = GridStep::Q0;
    for (label, u) in [
        ("m", LinearUnit::METRE),
        ("mm", mm),
        ("in", inch),
        ("US ft", survey),
    ] {
        time(&format!("decimal length to grid ({label})"), VALUES, |i| {
            grid::length(decimals[i], u, q0)
        });
        time(&format!("double length to grid ({label})"), VALUES, |i| {
            grid::length_of_double(doubles[i], u, q0)
        });
        time(&format!("grid value written ({label})"), VALUES, |i| {
            write::length(counts[i], u, q0)
        });
    }
    time("real written shortest", VALUES, |i| {
        real::shortest(doubles[i])
    });
    let texts: Vec<String> = doubles
        .iter()
        .map(|x| real::shortest(*x).unwrap().to_string())
        .collect();
    time("real read", VALUES, |i| real::read(&texts[i]));
}
