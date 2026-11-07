// SPDX-License-Identifier: Apache-2.0
//! The spline rules fuzz target (piece G220a), as tests: `sketch_splines_fuzz` is bolero's test
//! mode, and `sketch_splines_seeded` the seeded leg that runs on every target, WebAssembly included
//! (the crate README sets out the layout). Each hands the fuzzer's bytes to
//! `geom_fuzz::sketch_splines::harness`.
//!
//! `chosen_sketches_keep_every_law` adds sketches with a known answer.

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

use geom_fuzz::sketch_splines::{check_sketch, harness};
use geom_sketch::text::parse;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 300;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 300;

fn draw(rng: &mut SplitMix64, n: u64) -> u64 {
    NonZeroU64::new(n).map_or(0, |n| rng.below(n))
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn sketch_splines_fuzz() {
    println!(
        "bolero test mode: corpus and crash files, then {ITERATIONS} random inputs whose seeds bolero picks"
    );
    bolero::check!()
        .with_iterations(ITERATIONS)
        .for_each(|data: &[u8]| holds(harness(data)));
}

#[test]
fn sketch_splines_seeded() {
    let mut rng = SplitMix64::new(resolve("sketch_splines bytes", Seed::DEFAULT).unwrap());
    for _ in 0..SEEDED_CASES {
        let len = draw(&mut rng, 64);
        let data: Vec<u8> = (0..len).map(|_| draw(&mut rng, 256) as u8).collect();
        holds(harness(&data));
    }
}

#[test]
fn chosen_sketches_keep_every_law() {
    // Bytes that make a gentle pair of cubics with rules of every kind, and the empty input.
    for data in [
        vec![],
        vec![128; 40],
        (0..64_u8)
            .map(|i| i.wrapping_mul(7).wrapping_add(90))
            .collect::<Vec<u8>>(),
        (0..64_u8)
            .map(|i| 255_u8.wrapping_sub(i.wrapping_mul(3)))
            .collect::<Vec<u8>>(),
    ] {
        holds(harness(&data));
    }
    let sketch = parse(
        "sketch v1
plane lattice 0 0 0 1 0 0 0 1 0 q0
entity 1.0 point 0 0
entity 1.1 point 0 0
entity 1.2 point 0 0
entity 1.3 point 0 0
entity 2.0 spline 3 4 1.0 1.1 1.2 1.3 8 0 0 0 0 1 1 1 1 4 1 1 1 1
",
    )
    .unwrap();
    holds(check_sketch(&sketch));
}
