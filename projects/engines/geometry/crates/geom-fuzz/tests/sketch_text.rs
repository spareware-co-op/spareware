//! The sketch text fuzz target (piece G217), as tests: `sketch_text_fuzz` is bolero's test mode, and
//! `sketch_text_seeded` the seeded leg that runs on every target, WebAssembly included (the crate
//! README sets out the layout). Each hands the fuzzer's bytes to `geom_fuzz::sketch_text::harness`.
//!
//! `seeded_lines` adds a leg random bytes rarely reach: texts built from the grammar's own words, so
//! the parser's paths run on every pull request too.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use core::num::NonZeroU64;

use common::holds;

use geom_fuzz::sketch_text::{check_text, harness};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 20_000;

/// Texts in the seeded leg.
const SEEDED_CASES: u32 = 20_000;

/// The words the grammar is made of, and some that are nearly its words.
const WORDS: [&str; 44] = [
    "entity",
    "constraint",
    "point",
    "line",
    "circle",
    "arc",
    "ellipse",
    "spline",
    "parameter",
    "scalar",
    "placement",
    "reference",
    "construction",
    "segment",
    "ray",
    "infinite",
    "driving",
    "coincident",
    "horizontal",
    "vertical",
    "distance",
    "point-line",
    "fix",
    "left",
    "right",
    "0",
    "1",
    "2",
    "-1",
    "0.5",
    "1e308",
    "1e-320",
    "nan",
    "inf",
    "1.0",
    "1.1",
    "2.0",
    "2.1",
    "3.0",
    "18446744073709551616",
    "1.",
    ".1",
    "1.0.0",
    "q0",
];

fn draw(rng: &mut SplitMix64, n: u64) -> u64 {
    NonZeroU64::new(n).map_or(0, |n| rng.below(n))
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn sketch_text_fuzz() {
    println!(
        "bolero test mode: corpus and crash files, then {ITERATIONS} random inputs whose seeds \
         bolero picks"
    );
    bolero::check!()
        .with_iterations(ITERATIONS)
        .for_each(|data: &[u8]| holds(harness(data)));
}

#[test]
fn sketch_text_seeded() {
    let mut rng = SplitMix64::new(resolve("sketch_text bytes", Seed::DEFAULT).unwrap());
    for _ in 0..SEEDED_CASES {
        let len = draw(&mut rng, 128);
        let data: Vec<u8> = (0..len).map(|_| draw(&mut rng, 256) as u8).collect();
        holds(harness(&data));
    }
}

#[test]
fn seeded_lines() {
    let mut rng = SplitMix64::new(resolve("sketch_text lines", Seed::DEFAULT).unwrap());
    let mut accepted = 0;
    for _ in 0..SEEDED_CASES {
        let mut text = String::from("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n");
        // A point or two, so lines that name them can parse.
        text.push_str("entity 1.0 point 0 0\nentity 1.1 point 3 4\n");
        for _ in 0..draw(&mut rng, 5) {
            text.push_str(if draw(&mut rng, 2) == 0 {
                "entity"
            } else {
                "constraint"
            });
            for _ in 0..draw(&mut rng, 8) {
                text.push(' ');
                text.push_str(WORDS[draw(&mut rng, WORDS.len() as u64) as usize]);
            }
            text.push('\n');
        }
        holds(check_text(&text));
        if geom_sketch::text::parse(&text).is_ok() {
            accepted += 1;
        }
    }
    assert!(accepted > 100, "only {accepted} texts parsed");
}
