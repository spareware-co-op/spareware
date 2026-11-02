//! The typed transforms' fuzz target, run as tests on stable Rust: bolero's test mode, the seeded
//! leg, the input bound, and the laws on chosen cases.
//!
//! - `transforms_fuzz` wraps `geom_fuzz::transforms::harness` in `bolero::check!`. Under
//!   `cargo test` it replays the files in `tests/__fuzz__/transforms/corpus` and `crashes`, if
//!   any, then runs [`ITERATIONS`] random byte strings; bolero prints a failing one's seed as
//!   `BOLERO_RANDOM_SEED=<seed>`, which replays it. Under
//!   `cargo bolero test transforms_fuzz --package geom-fuzz` the same test is a libFuzzer target.
//! - `transforms_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed
//!   seed (`SPAREKIT_SEED` replays it), the same on every target.
//! - `the_laws_hold_at_the_frames_limits` and `a_wrong_case_is_a_finding`: the laws on chosen
//!   cases, and a case built to break one (28.4, fault injection).

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their inputs.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test inputs; the rules are for kernel code"
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
use geom_fuzz::transforms::{Case, MAX_INPUT_BYTES, TEXT, check, decode, harness};
use geom_num::grid::COORD_MAX;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the fuzz target in bolero's test mode.
const ITERATIONS: usize = 10_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 10_000;

/// The longest byte string the fuzz target and the seeded leg try. The harness reads at most
/// [`MAX_INPUT_BYTES`] (97) bytes; the rest let some strings carry bytes nobody reads.
const MAX_LEN: u16 = 128;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn transforms_fuzz() {
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
fn transforms_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "transforms fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let word = |b: &mut Vec<u8>| {
        b.push(7);
        b.extend([0x11; 8]);
    };
    let mut bytes = Vec::new();
    for _ in 0..3 {
        word(&mut bytes);
    }
    bytes.extend([1; 4 + 9]);
    for _ in 0..3 {
        word(&mut bytes);
    }
    bytes.extend([3, 1, 2, 3, 4, TEXT as u8]);
    bytes.extend([b'1'; TEXT]);
    bytes.extend([9; 30]);
    assert_eq!(decode(&bytes).1, MAX_INPUT_BYTES);
    assert_eq!(decode(&[]).1, 0);
}

fn case(point: [i64; 3], m: [[i64; 3]; 3], d: i64, q: [i64; 4], text: &str) -> Case {
    Case {
        point,
        grid: [[0x15, 200], [0x02, 3]],
        m,
        t: [1, -1, 3],
        d,
        q,
        text: text.as_bytes().to_vec(),
    }
}

#[test]
fn the_laws_hold_at_the_frames_limits() {
    let m = [[3, 0, 0], [0, -4, 1], [0, 0, 2]];
    for point in [[0, 0, 0], [COORD_MAX / 4, -7, 5], [1, 2, 3], [-3, 2, -1]] {
        // Halves and quarters, ties included.
        for d in [1, 2, 4, 3] {
            holds(check(&case(point, m, d, [1, 2, 2, 0], "-12.375e3")));
        }
    }
}

#[test]
fn a_wrong_case_is_a_finding() {
    // A decimal that does not parse is legal; the laws hold on every value above. A case cannot
    // make the kernel wrong, so the check is shown to fail by handing it an image it did not
    // compute: the rounding law on a transform whose exact image needs rounding, compared with a
    // point moved by one.
    let c = case(
        [1, 2, 3],
        [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        2,
        [1, 0, 0, 0],
        "x",
    );
    holds(check(&c));
    assert!(geom_fuzz::transforms::check_rounding([2, 1, 1], false, [1, 2, 3], 2).is_err());
    assert!(geom_fuzz::transforms::check_rounding([0, 0, 2], false, [1, 1, 4], 2).is_ok());
}
