//! The edit fuzz target (G206), run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, and every turn and mirror sense reached.
//!
//! - `edit_fuzz` wraps `geom_fuzz::edit::harness` in `bolero::check!`: the crate's corpus and
//!   crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`. Under `cargo bolero test edit_fuzz --package geom-fuzz` it is a
//!   libFuzzer target.
//! - `edit_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly, and every shape and every kind of classification is
//!   reached.

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

use geom_fuzz::edit::{MAX_INPUT_BYTES, MAX_VERTICES, check, decode, harness};
use geom_fuzz::seeded;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 20_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 50_000;

/// The longest byte string the seeded leg draws.
const MAX_LEN: u16 = 90;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn edit_fuzz() {
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
fn edit_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "edit fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// The input that reads the most: a full set of vertices, a bulge for each segment of a closed
/// polyline, and a placement.
#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = vec![MAX_VERTICES - 2, 1, 0];
    data.extend_from_slice(&[3; 2 * MAX_VERTICES as usize]);
    data.extend_from_slice(&[1; MAX_VERTICES as usize]);
    data.extend_from_slice(&[1, 1, 1, 2, 2]);
    assert_eq!(data.len(), MAX_INPUT_BYTES);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    assert_eq!(case.vertices.len(), usize::from(MAX_VERTICES));
    assert!(case.closed);
    holds(check(&case));
    data.extend_from_slice(&[7; 100]);
    assert_eq!(decode(&data).1, MAX_INPUT_BYTES);
    // The empty input is a case too.
    let (empty, read) = decode(&[]);
    assert_eq!(read, 0);
    holds(check(&empty));
}

/// Random bytes reach every quarter turn, both mirror senses, closed and open polylines, and a
/// good number of placements that succeed.
#[test]
fn every_turn_and_both_senses_are_reached() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("edit reach", Seed::DEFAULT));
    let mut turns = [0_u32; 4];
    let mut mirrors = [0_u32; 2];
    let mut closed = 0_u32;
    let mut placed = 0_u32;
    for _ in 0..4000 {
        let len = rng.below(std::num::NonZeroU64::new(40).unwrap()) as usize;
        let mut data = vec![0_u8; len];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        turns[usize::from(case.turns)] += 1;
        mirrors[usize::from(case.mirror)] += 1;
        holds(check(&case));
        if case.closed {
            closed += 1;
        }
        if case.scale != 0 && case.magnitude < 20 {
            placed += 1;
        }
    }
    println!("turns {turns:?}, mirrors {mirrors:?}, closed {closed}, placed {placed}");
    assert!(turns.iter().all(|&n| n > 100), "{turns:?}");
    assert!(mirrors.iter().all(|&n| n > 100), "{mirrors:?}");
    assert!(closed > 100 && placed > 100);
}
