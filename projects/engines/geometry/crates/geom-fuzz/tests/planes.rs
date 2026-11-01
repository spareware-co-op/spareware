//! The planes fuzz target (G123), run as tests on stable Rust: bolero's test mode, the seeded leg
//! and the input bound.
//!
//! - `planes_fuzz` wraps `geom_fuzz::planes::harness` in `bolero::check!`; under
//!   `cargo bolero test planes_fuzz --package geom-fuzz` it is a libFuzzer target.
//! - `planes_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.

#![allow(clippy::arithmetic_side_effects, reason = "test generators")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::holds;

use geom_fuzz::planes::{Case, MAX_INPUT_BYTES, PLANES, check, decode, harness};
use geom_fuzz::seeded;
use geom_num::P3;
use geom_num::plane::Plane;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 500;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 2_000;

/// The longest byte string the seeded leg draws.
const MAX_LEN: u16 = 500;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn planes_fuzz() {
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
fn planes_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "planes fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn any_bytes_are_a_case() {
    assert_eq!(decode(&[]).0.planes, []);
    holds(harness(&[]));
    holds(harness(&[0xff; 64]));
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let data = vec![0x11u8; usize::from(MAX_LEN)];
    assert_eq!(decode(&data).1, MAX_INPUT_BYTES);
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// Planes that meet in points, so every law runs on real orders.
#[test]
fn a_box_of_planes_keeps_every_law() {
    let planes = vec![
        Plane::through(p(0, 0, 0), p(1, 0, 0), p(0, 1, 0)).unwrap(),
        Plane::through(p(0, 0, 0), p(0, 1, 0), p(0, 0, 1)).unwrap(),
        Plane::through(p(5, 0, 0), p(5, 1, 0), p(5, 0, 1)).unwrap(),
        Plane::through(p(0, 3, 0), p(0, 3, 1), p(1, 3, 0)).unwrap(),
        Plane::through(p(0, 0, 0), p(1, 1, 0), p(0, 0, 1)).unwrap(),
    ];
    assert_eq!(planes.len(), PLANES);
    holds(check(&Case { planes }));
}
