//! The crossings fuzz target (G163), run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, and what it reaches.
//!
//! - `crossings_fuzz` wraps `geom_fuzz::crossings::harness` in `bolero::check!`: the crate's corpus and
//!   crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`. Under `cargo bolero test crossings_fuzz --package geom-fuzz` it is a
//!   libFuzzer target.
//! - `crossings_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly, and the cases reach every kind of curve, crossings, misses
//!   and a shared carrier.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test generators; the rules are for kernel code"
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

use geom_fuzz::crossings::{Case, Drawn, MAX_INPUT_BYTES, check, decode, harness};
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 500;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 1_500;

/// The longest byte string the seeded leg draws: the harness reads at most [`MAX_INPUT_BYTES`];
/// the rest must change nothing.
const MAX_LEN: u16 = 40;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn crossings_fuzz() {
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
fn crossings_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "crossings fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = vec![3; MAX_INPUT_BYTES];
    let (_, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    data.extend_from_slice(&[7; 10]);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    holds(check(&case));
    // The empty input is a case too.
    let (empty, read) = decode(&[]);
    assert_eq!(read, 0);
    holds(check(&empty));
}

fn pair(a: Drawn, b: Drawn) {
    holds(check(&Case {
        drawn: [a, b],
        work: 3,
    }));
}

#[test]
fn every_kind_meets_every_kind() {
    let at = [10, 20, 30, 40, 50, 60];
    let other = [220, 5, 40, 3, 90, 200];
    for k in 0..4 {
        for j in 0..4 {
            pair(Drawn { kind: k, at }, Drawn { kind: j, at: other });
            pair(Drawn { kind: k, at }, Drawn { kind: j, at });
        }
    }
}
