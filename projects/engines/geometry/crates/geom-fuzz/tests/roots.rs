//! The root engine's fuzz target, run as tests on stable Rust: bolero's test mode, the seeded
//! leg, the input bound, and chosen polynomials.
//!
//! - `roots_fuzz` wraps `geom_fuzz::roots::harness` in `bolero::check!`. Under `cargo test` it
//!   replays the files in `tests/__fuzz__/roots/corpus` and `crashes`, if any, then runs
//!   [`ITERATIONS`] random byte strings; bolero prints a failing one's seed as
//!   `BOLERO_RANDOM_SEED=<seed>`, which replays it. Under
//!   `cargo bolero test roots_fuzz --package geom-fuzz` the same test is a libFuzzer target.
//! - `roots_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - `simple_repeated_and_end_roots_keep_every_law`: chosen polynomials.

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
use geom_fuzz::roots::{Case, MAX_INPUT_BYTES, MAX_ROOTS, check, decode, harness};
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the fuzz target in bolero's test mode.
const ITERATIONS: usize = 2_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 2_000;

/// The longest byte string the fuzz target and the seeded leg try. The harness reads at most
/// [`MAX_INPUT_BYTES`] (10) bytes; the rest let some strings carry bytes nobody reads.
const MAX_LEN: u16 = 32;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn roots_fuzz() {
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
fn roots_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "roots fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut bytes = vec![u8::try_from(MAX_ROOTS - 1).unwrap()];
    bytes.extend([10; MAX_ROOTS]);
    bytes.extend([0, 64, 1, 3]);
    bytes.extend([7; 10]);
    assert_eq!(decode(&bytes).1, MAX_INPUT_BYTES);
    assert_eq!(decode(&[]).1, 0);
}

#[test]
fn simple_repeated_and_end_roots_keep_every_law() {
    let cases = [
        vec![-1.0, -0.5, 0.0, 0.5, 1.0],
        vec![1.0, 1.0, -2.0],
        vec![-4.0, 4.0],
        vec![0.25],
    ];
    for roots in cases {
        for hansen in [false, true] {
            holds(check(&Case {
                roots: roots.clone(),
                domain: (-4.0, 4.0),
                hansen,
                half: 0.25,
            }));
        }
    }
}

#[test]
fn the_engine_answers_so_the_law_is_not_empty() {
    use geom_fuzz::roots::{Product, WORK};
    use geom_num::interval::Interval;
    use geom_num::isolate::{Options, isolate};
    use geom_run::{Budget, WorkMeter};
    let roots = [-1.0, 0.5, 2.25];
    let mut meter = WorkMeter::new(Budget::of_work(WORK));
    let got = isolate(
        &Product(&roots),
        [Interval::new(-4.0, 4.0).unwrap()],
        Options::DEFAULT,
        &mut meter,
    )
    .unwrap();
    assert_eq!(got.zeros.len(), 3, "{got:?}");
}
