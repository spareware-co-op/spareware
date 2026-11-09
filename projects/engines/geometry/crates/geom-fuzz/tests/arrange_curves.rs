//! The `arrange_curves` fuzz target (G196b), run as tests on stable Rust: bolero's test mode, the
//! seeded leg, the input bound, and every kind of curve and verdict reached.
//!
//! - `arrange_curves_fuzz` wraps `geom_fuzz::arrange_curves::harness` in `bolero::check!`: the
//!   crate's corpus and crash files, then [`ITERATIONS`] random byte strings; a failing one's seed
//!   is printed as `BOLERO_RANDOM_SEED=<seed>`. Under
//!   `cargo bolero test arrange_curves_fuzz --package geom-fuzz` it is a libFuzzer target.
//! - `arrange_curves_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the
//!   printed seed (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly, and random bytes reach segments, arcs, ellipses and
//!   circles written as ellipses, drawings that are arranged and drawings refused because an
//!   ellipse meets a circle or another ellipse.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::holds;

use geom_fuzz::arrange_curves::{
    MAX_CURVES, MAX_INPUT_BYTES, Shape, check, decode, harness, refused,
};
use geom_fuzz::seeded;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 4_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 8_000;

/// The longest byte string the seeded leg draws.
const MAX_LEN: u16 = 60;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn arrange_curves_fuzz() {
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
fn arrange_curves_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "arrange_curves fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// The input that reads the most: six arcs and ellipses.
#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = vec![MAX_CURVES];
    for i in 0..MAX_CURVES {
        // Kind 1, an arc, and kind 2, an ellipse, each reading five bytes after its kind.
        data.push(1 + i % 2);
        data.extend_from_slice(&[16 * i, 32, 200, 64 + 8 * i, 2]);
    }
    assert_eq!(data.len(), MAX_INPUT_BYTES);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    assert_eq!(case.curves.len(), usize::from(MAX_CURVES));
    holds(check(&case));
    data.extend_from_slice(&[7; 100]);
    assert_eq!(decode(&data).1, MAX_INPUT_BYTES);
    // The empty input is a case too.
    let (empty, read) = decode(&[]);
    assert_eq!(read, 0);
    holds(check(&empty));
}

/// Random bytes reach every kind of curve, circles written as ellipses, arranged drawings with arcs
/// and ellipses together, and refusals; and every case checks.
#[test]
fn random_bytes_reach_every_kind_and_both_verdicts() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed(
        "arrange_curves reach",
        Seed::DEFAULT,
    ));
    let (mut segments, mut arcs, mut ellipses, mut round) = (0_u32, 0_u32, 0_u32, 0_u32);
    let (mut mixed, mut refusals) = (0_u32, 0_u32);
    for _ in 0..1500 {
        let len = rng.below(std::num::NonZeroU64::new(40).unwrap()) as usize;
        let mut data = vec![0_u8; len];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        let mut kinds = [false; 2];
        for shape in &case.curves {
            match shape {
                Shape::Segment(_) => segments += 1,
                Shape::Arc { .. } => {
                    arcs += 1;
                    kinds[0] = true;
                }
                Shape::Ellipse { p, q, .. } => {
                    ellipses += 1;
                    kinds[1] = true;
                    if p == q {
                        round += 1;
                    }
                }
            }
        }
        holds(check(&case));
        if refused(&case) {
            refusals += 1;
        } else if kinds == [true, true] {
            mixed += 1;
        }
    }
    println!(
        "segments {segments}, arcs {arcs}, ellipses {ellipses} ({round} of ratio one), \
         arranged with arcs and ellipses {mixed}, refused {refusals}"
    );
    assert!(segments > 500 && arcs > 500 && ellipses > 500 && round > 50);
    assert!(mixed > 50 && refusals > 50);
}
