// SPDX-License-Identifier: Apache-2.0
//! The generated sketch families fuzz target (piece G231), as tests: `sketch_corpus_fuzz` is
//! bolero's test mode, and `sketch_corpus_seeded` the seeded leg that runs on every target,
//! WebAssembly included (the crate README sets out the layout). Each hands the fuzzer's bytes to
//! `geom_fuzz::sketch_corpus::harness`.
//!
//! `every_family_keeps_its_laws` runs a night of each family, edited and not.

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

use geom_fuzz::sketch_corpus::{check_case, check_edited, harness};
use geom_test::oracle::families::cases;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 60;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 60;

fn draw(rng: &mut SplitMix64, n: u64) -> u64 {
    NonZeroU64::new(n).map_or(0, |n| rng.below(n))
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn sketch_corpus_fuzz() {
    println!(
        "bolero test mode: corpus and crash files, then {ITERATIONS} random inputs whose seeds bolero picks"
    );
    bolero::check!()
        .with_iterations(ITERATIONS)
        .for_each(|data: &[u8]| holds(harness(data)));
}

#[test]
fn sketch_corpus_seeded() {
    let mut rng = SplitMix64::new(resolve("sketch_corpus bytes", Seed::DEFAULT).unwrap());
    for _ in 0..SEEDED_CASES {
        let len = draw(&mut rng, 24);
        let data: Vec<u8> = (0..len).map(|_| draw(&mut rng, 256) as u8).collect();
        holds(harness(&data));
    }
}

#[test]
fn every_family_keeps_its_laws() {
    let night = cases(
        resolve("sketch_corpus families", Seed::DEFAULT).unwrap(),
        36,
    );
    for case in &night {
        holds(check_case(case));
        for which in 0..3 {
            for by in [0.5, 0.9, 1.0, 1.7, 3.0] {
                holds(check_edited(case, which, by));
            }
        }
    }
}
