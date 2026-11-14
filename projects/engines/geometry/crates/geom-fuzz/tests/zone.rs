//! The zone fuzz target (G199, with arcs from G199a), run as tests on stable Rust: bolero's test
//! mode, the seeded leg, the input bound, and crossings and arcs reached.
//!
//! - `zone_fuzz` wraps `geom_fuzz::zone::harness` in `bolero::check!`: the crate's corpus and
//!   crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`. Under `cargo bolero test zone_fuzz --package geom-fuzz` it is a
//!   libFuzzer target.
//! - `zone_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
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

use geom_fuzz::seeded;
use geom_fuzz::zone::{
    MAX_ARCS, MAX_INPUT_BYTES, MAX_PROBES, MAX_SEGMENTS, check, decode, harness,
};
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg. Fewer than G199's 2,000 since arcs
/// came in (G199a): a case with arcs asks dozens of exact questions on one-root points, each tens of
/// microseconds in a debug build.
const ITERATIONS: usize = 1_000;

/// Byte strings in the seeded leg; fewer than G199's 4,000 for the same reason.
const SEEDED_CASES: u32 = 1_000;

/// The longest byte string the seeded leg draws.
const MAX_LEN: u16 = 100;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn zone_fuzz() {
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
fn zone_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "zone fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// The input that reads the most: a full set of segments, probes and arcs.
#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = vec![MAX_SEGMENTS];
    data.extend_from_slice(&[0, 0, 7, 7, 0, 7, 7, 0, 3, 0, 3, 7, 0, 3, 7, 3]);
    data.extend_from_slice(&[1, 1, 6, 2, 5, 5, 6, 6, 0, 0, 0, 0, 0, 0, 0, 0]);
    data.extend_from_slice(&[0, MAX_PROBES]);
    data.extend_from_slice(&[3, 4, 1, 5, 5, 2, 9, 2, 0, 1, 1, 1]);
    data.push(MAX_ARCS);
    data.extend_from_slice(&[6, 3, 2, 3, 4, 1, 1, 5, 5, 8]);
    assert_eq!(data.len(), MAX_INPUT_BYTES);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    assert_eq!(case.segments.len(), usize::from(MAX_SEGMENTS));
    assert_eq!(case.arcs.len(), usize::from(MAX_ARCS));
    holds(check(&case));
    // A byte string that stops before the arcs reads as it did before arcs came in.
    let (old, read) = decode(&data[..47]);
    assert_eq!(read, 47);
    assert_eq!(
        (old.segments, old.probes, old.arcs.len()),
        (case.segments, case.probes, 0)
    );
    data.extend_from_slice(&[7; 100]);
    assert_eq!(decode(&data).1, MAX_INPUT_BYTES);
    // The empty input is a case too.
    let (empty, read) = decode(&[]);
    assert_eq!(read, 0);
    holds(check(&empty));
}

/// Random bytes make drawings with crossings and arcs in good number, and every one checks.
#[test]
fn random_bytes_make_drawings_that_cross() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("zone reach", Seed::DEFAULT));
    let mut crossing = 0_u32;
    let mut sizes = [0_u32; 9];
    let mut arcs = [0_u32; MAX_ARCS as usize + 1];
    for _ in 0..1000 {
        let len = rng.below(std::num::NonZeroU64::new(90).unwrap()) as usize;
        let mut data = vec![0_u8; len];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        sizes[case.segments.len()] += 1;
        arcs[case.arcs.len()] += 1;
        holds(check(&case));
        if case.segments.len() + case.arcs.len() >= 2 {
            crossing += 1;
        }
    }
    println!("sizes {sizes:?}, arcs {arcs:?}, drawings of two or more {crossing}");
    assert!(sizes.iter().all(|&n| n > 50), "{sizes:?}");
    assert!(arcs.iter().all(|&n| n > 50), "{arcs:?}");
    assert!(crossing > 600);
}
