// SPDX-License-Identifier: Apache-2.0
//! The sketch free-motions fuzz target (piece G224), as tests: `sketch_motions_fuzz` is bolero's
//! test mode, and `sketch_motions_seeded` the seeded leg that runs on every target, WebAssembly
//! included (the crate README sets out the layout). Each hands the fuzzer's bytes to
//! `geom_fuzz::sketch_motions::harness`.
//!
//! `chosen_sketches_keep_every_law` adds sketches with a known number of freedoms.

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

use geom_fuzz::sketch_motions::{check_sketch, harness};
use geom_sketch::text::parse;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 2_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 2_000;

fn draw(rng: &mut SplitMix64, n: u64) -> u64 {
    NonZeroU64::new(n).map_or(0, |n| rng.below(n))
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics,               which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn sketch_motions_fuzz() {
    println!(
        "bolero test mode: corpus and crash files, then {ITERATIONS} random inputs whose seeds          bolero picks"
    );
    bolero::check!()
        .with_iterations(ITERATIONS)
        .for_each(|data: &[u8]| holds(harness(data)));
}

#[test]
fn sketch_motions_seeded() {
    let mut rng = SplitMix64::new(resolve("sketch_motions bytes", Seed::DEFAULT).unwrap());
    for _ in 0..SEEDED_CASES {
        let len = draw(&mut rng, 64);
        let data: Vec<u8> = (0..len).map(|_| draw(&mut rng, 256) as u8).collect();
        holds(harness(&data));
    }
}

#[test]
fn chosen_sketches_keep_every_law() {
    for lines in [
        "",
        "entity 1.0 point 0 0
entity 1.1 point 300 400
",
        "entity 1.0 point 0 0
entity 1.1 point 300 400
constraint 2.0 driving 0 fix 1.0 0 0
         constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 distance 1.0 1.1 500
",
        "entity 1.0 point 0 0
entity 1.1 point 300 400
constraint 2.0 driving 0 distance 1.0 1.1 500
         constraint 2.1 driving 1 distance 1.0 1.1 700
",
    ] {
        let sketch = parse(&format!(
            "sketch v1
plane lattice 0 0 0 1 0 0 0 1 0 q0
{lines}"
        ))
        .unwrap();
        holds(check_sketch(&sketch));
    }
}
