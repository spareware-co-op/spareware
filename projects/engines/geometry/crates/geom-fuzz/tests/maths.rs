//! The proven maths functions' fuzz target, run as tests on stable Rust: bolero's test mode, the
//! seeded leg, the input bound, chosen values, and a result one double off caught.
//!
//! - `maths_fuzz` wraps `geom_fuzz::maths::harness` in `bolero::check!`. Under `cargo test` it
//!   replays the files in `tests/__fuzz__/maths/corpus` and `crashes`, if any, then runs
//!   [`ITERATIONS`] random byte strings; bolero prints a failing one's seed as
//!   `BOLERO_RANDOM_SEED=<seed>`, which replays it. Under
//!   `cargo bolero test maths_fuzz --package geom-fuzz` the same test is a libFuzzer target.
//! - `maths_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.

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
use geom_fuzz::maths::{Case, Function, MAX_INPUT_BYTES, check, check_with, decode, harness};
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the fuzz target in bolero's test mode.
const ITERATIONS: usize = 2_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 2_000;

/// The longest byte string the fuzz target and the seeded leg try. The harness reads at most
/// [`MAX_INPUT_BYTES`] (20) bytes; the rest let some strings carry bytes nobody reads.
const MAX_LEN: u16 = 32;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn maths_fuzz() {
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
fn maths_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "maths fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut bytes = vec![4];
    for _ in 0..2 {
        bytes.push(0x0F);
        bytes.extend([0x3F, 0xF0, 0, 0, 0, 0, 0, 0]);
    }
    bytes.push(0);
    bytes.extend([7; 10]);
    assert_eq!(decode(&bytes).1, MAX_INPUT_BYTES);
    assert_eq!(decode(&[]).1, 0);
}

fn case(function: Function, a: f64, b: f64) -> Case {
    Case { function, a, b }
}

#[test]
fn chosen_values_are_correctly_rounded_and_one_double_off_is_caught() {
    let cases = [
        case(Function::Sinpi, 0.25, 0.0),
        case(Function::Cospi, 1.0 / 3.0, 0.0),
        case(Function::Exp, 1.0, 0.0),
        case(Function::Exp, -12.5, 0.0),
        case(Function::Ln, 2.0, 0.0),
        case(Function::Ln, 1e300, 0.0),
        case(Function::Atan2, 1.0, 1.0),
        case(Function::Atan2, -3.0, -4.0),
    ];
    for c in cases {
        holds(check(&c));
        assert!(check_with(&c, 1).is_err(), "{c:?} one double up");
        assert!(check_with(&c, -1).is_err(), "{c:?} one double down");
    }
}
