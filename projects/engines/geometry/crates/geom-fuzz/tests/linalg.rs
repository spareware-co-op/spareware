//! The dense and spectral decompositions' fuzz target, run as tests on stable Rust: bolero's test
//! mode, the seeded leg, the input bound, and chosen matrices.
//!
//! - `linalg_fuzz` wraps `geom_fuzz::linalg::harness` in `bolero::check!`. Under `cargo test` it
//!   replays the files in `tests/__fuzz__/linalg/corpus` and `crashes`, if any, then runs
//!   [`ITERATIONS`] random byte strings; bolero prints a failing one's seed as
//!   `BOLERO_RANDOM_SEED=<seed>`, which replays it. Under
//!   `cargo bolero test linalg_fuzz --package geom-fuzz` the same test is a libFuzzer target.
//! - `linalg_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - `chosen_matrices_keep_every_law`: singular, symmetric, graded and rectangular matrices.

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
use geom_fuzz::linalg::{Case, MAX_DIM, MAX_INPUT_BYTES, check, decode, harness};
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the fuzz target in bolero's test mode.
const ITERATIONS: usize = 2_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 2_000;

/// The longest byte string the fuzz target and the seeded leg try. The harness reads at most
/// [`MAX_INPUT_BYTES`] (227) bytes; the rest let some strings carry bytes nobody reads.
const MAX_LEN: u16 = 256;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[test]
fn linalg_fuzz() {
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
fn linalg_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "linalg fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut bytes = vec![u8::try_from(MAX_DIM - 1).unwrap(); 2];
    for _ in 0..MAX_DIM * MAX_DIM {
        bytes.push(0x0F);
        bytes.extend([0x3F, 0xF0, 0, 0, 0, 0, 0, 0]);
    }
    bytes.extend([1; 30]);
    assert_eq!(decode(&bytes).1, MAX_INPUT_BYTES);
    assert_eq!(decode(&[]).1, 0);
}

#[test]
fn chosen_matrices_keep_every_law() {
    let cases = [
        (2, 2, vec![1.0, 2.0, 2.0, 4.0]),
        (3, 3, vec![4.0, 1.0, 2.0, 1.0, 3.0, 0.5, 2.0, 0.5, 5.0]),
        (2, 3, vec![1.0, 0.0, 1e-200, 0.0, 1e200, 1.0]),
        (3, 1, vec![0.0, 0.0, 0.0]),
        (1, 1, vec![-7.0]),
    ];
    for (rows, cols, entries) in cases {
        holds(check(&Case {
            rows,
            cols,
            entries,
        }));
    }
}
