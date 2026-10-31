//! The NURBS fuzz target, run as tests on stable Rust: bolero's test mode, the seeded leg, the
//! input bound, the four laws reached, and a broken evaluation caught.
//!
//! - `nurbs_fuzz` wraps `geom_fuzz::nurbs::harness` in `bolero::check!`. Under `cargo test` it
//!   replays the files in `tests/__fuzz__/nurbs/corpus` and `crashes`, if any, then runs
//!   [`ITERATIONS`] random byte strings; bolero prints a failing one's seed as
//!   `BOLERO_RANDOM_SEED=<seed>`, which replays it. Under
//!   `cargo bolero test nurbs_fuzz --package geom-fuzz` the same test is a libFuzzer target.
//! - `nurbs_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - `clamped_and_uniform_rational_curves_keep_every_law` checks hand-made cases of each shape.
//! - `a_broken_evaluation_is_a_finding`: a point moved by one is caught (28.4, fault injection).

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
use geom_fuzz::nurbs::{MAX_INPUT_BYTES, MAX_POINTS, check, check_with, decode, harness};
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the fuzz target in bolero's test mode.
const ITERATIONS: usize = 2_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 2_000;

/// The longest byte string the fuzz target and the seeded leg try. The harness reads at most
/// [`MAX_INPUT_BYTES`] (306) bytes; the rest let some strings carry bytes nobody reads, which
/// must change nothing.
const MAX_LEN: u16 = 320;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[test]
fn nurbs_fuzz() {
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
fn nurbs_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "nurbs fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// A case: in space or not, of degree `degree` with `count` points, rational or not, clamped or
/// not, steps of 1/2 with a repeat, every coordinate and weight a whole number from the rule
/// bytes, the parameter and the knot at a third and two thirds.
fn input(space: bool, degree: u8, count: u8, rational: bool, clamped: bool) -> Vec<u8> {
    let mut bytes = vec![
        u8::from(space),
        degree - 1,
        count - degree - 1,
        u8::from(rational),
        u8::from(clamped),
    ];
    for i in 0..MAX_POINTS + 3 {
        // 0x20: a step of 2/4; every third 0x00, a repeat where allowed.
        bytes.push(if i % 3 == 2 { 0x00 } else { 0x20 });
    }
    for i in 0..MAX_POINTS * 4 {
        // Whole numbers 1 to 9 (rule 3, k in the high bits), varied.
        bytes.push((((i * 5) % 9 + 1) as u8) << 4 | 3);
    }
    bytes.extend([85, 170]);
    bytes
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut bytes = vec![0; 5];
    bytes.extend([0x20; MAX_POINTS + 3]);
    for _ in 0..MAX_POINTS * 4 {
        bytes.push(0x0F);
        bytes.extend([0x3F, 0xF0, 0, 0, 0, 0, 0, 0]);
    }
    bytes.extend([1, 2]);
    bytes.extend([7; 40]);
    assert_eq!(decode(&bytes).1, MAX_INPUT_BYTES);
    assert_eq!(decode(&[]).1, 0);
}

#[test]
fn clamped_and_uniform_rational_curves_keep_every_law() {
    for space in [false, true] {
        for degree in 1..=4u8 {
            for rational in [false, true] {
                for clamped in [false, true] {
                    let count = (degree + 3).min(MAX_POINTS as u8);
                    let case = decode(&input(space, degree, count, rational, clamped)).0;
                    assert_eq!(usize::from(degree), case.degree);
                    holds(check(&case));
                }
            }
        }
    }
}

#[test]
fn a_broken_evaluation_is_a_finding() {
    let case = decode(&input(false, 3, 6, true, true)).0;
    holds(check_with(&case, 0.0));
    let found = check_with(&case, 1.0).unwrap_err();
    assert!(found.law.contains("within its stated bound"), "{found}");
}
