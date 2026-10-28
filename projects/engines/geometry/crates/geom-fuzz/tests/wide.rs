//! The wide-integer fuzz target, run as tests on stable Rust: bolero's test mode, the seeded leg,
//! and every operation at every width's limits.
//!
//! - `wide_fuzz` wraps `geom_fuzz::wide::harness` in `bolero::check!`. Under `cargo test` it
//!   replays the files in `tests/__fuzz__/wide/corpus` and `crashes`, if any, then runs
//!   [`ITERATIONS`] random byte strings; bolero prints a failing one's seed as
//!   `BOLERO_RANDOM_SEED=<seed>`, which replays it. Under
//!   `cargo bolero test wide_fuzz --package geom-fuzz` the same test is a libFuzzer target (the
//!   crate header has the command), and `kernel-fuzz.yml`'s campaign runs it for 25 hours.
//! - `wide_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), so every pull request also runs a fixed set of cases, the same
//!   on every target.
//! - `every_operation_at_every_widths_limits` checks every operation, at every width of
//!   `geom_fuzz::wide::WIDTHS`, on every pair of the values at and next to the width's limits
//!   (−2^(64L−1), −2^(64L−1) + 1, −2^(64L−2), −1, 0, 1, 2^(64L−2), 2^(64L−1) − 2, 2^(64L−1) − 1),
//!   and on every `resize` target: the board's extreme-value tests (G26), against the same oracle.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::holds;

use geom_fuzz::seeded;
use geom_fuzz::wide::{Case, MAX_INPUT_BYTES, Op, WIDTHS, check, harness};
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 10_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 20_000;

/// The longest byte string the seeded leg draws: the harness reads at most [`MAX_INPUT_BYTES`]
/// (219); the rest must change nothing.
const MAX_LEN: u16 = 256;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn wide_fuzz() {
    println!(
        "bolero test mode: corpus and crash files, then {ITERATIONS} random inputs whose seeds \
         bolero draws from the operating system; a failing input's seed is printed as \
         BOLERO_RANDOM_SEED=<seed>, which replays it"
    );
    bolero::check!()
        .with_iterations(ITERATIONS)
        .with_max_len(usize::from(MAX_LEN))
        .for_each(|data: &[u8]| holds(harness(data)));
}

#[test]
fn wide_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "wide fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// The limbs of `v` in `limbs` limbs, for `v` in −2^(64L−1) ..= 2^(64L−1) − 1 given as its top
/// limb's value and a fill for the limbs below: the extremes and their neighbours.
fn limit_values(limbs: usize) -> Vec<Vec<u64>> {
    let with = |low: u64, middle: u64, top: u64| -> Vec<u64> {
        (0..limbs)
            .map(|i| match i {
                0 => low,
                i if i + 1 == limbs => top,
                _ => middle,
            })
            .collect()
    };
    vec![
        with(0, 0, 1 << 63),                      // −2^(64L−1)
        with(1, 0, 1 << 63),                      // −2^(64L−1) + 1
        with(0, 0, 0xc000_0000_0000_0000),        // −2^(64L−2)
        with(u64::MAX, u64::MAX, u64::MAX),       // −1
        with(0, 0, 0),                            // 0
        with(1, 0, 0),                            // 1
        with(0, 0, 1 << 62),                      // 2^(64L−2)
        with(u64::MAX - 1, u64::MAX, !(1 << 63)), // 2^(64L−1) − 2
        with(u64::MAX, u64::MAX, !(1 << 63)),     // 2^(64L−1) − 1
    ]
}

/// Every operation at every width, on every pair of limit values, against the oracle.
#[test]
fn every_operation_at_every_widths_limits() {
    let ops = [
        Op::Add,
        Op::Sub,
        Op::Neg,
        Op::Mul,
        Op::Compare,
        Op::Sign,
        Op::Resize,
        Op::I128,
    ];
    let mut checked = 0_u32;
    for limbs in WIDTHS {
        let values = limit_values(limbs);
        for a in &values {
            for b in &values {
                for op in ops {
                    let targets: &[usize] = if op == Op::Resize { &WIDTHS } else { &[2] };
                    for &target in targets {
                        let case = Case {
                            limbs,
                            op,
                            target,
                            a: a.clone(),
                            b: b.clone(),
                        };
                        check(&case).unwrap_or_else(|f| panic!("{f}"));
                        checked += 1;
                    }
                }
            }
        }
    }
    // 7 widths × 81 pairs × (7 operations + 7 resize targets).
    assert_eq!(checked, 7 * 81 * 14);
}
