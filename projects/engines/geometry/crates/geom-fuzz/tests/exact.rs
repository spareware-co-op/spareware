//! The exact mesh fuzz target (G146), run as tests on stable Rust: bolero's test mode, the seeded
//! leg, the input bound, and a closed mesh and an open one reached.
//!
//! - `exact_fuzz` wraps `geom_fuzz::exact::harness` in `bolero::check!`: the target's stored corpus
//!   and crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`. Under `cargo bolero test exact_fuzz --package geom-fuzz` it is a
//!   libFuzzer target.
//! - `exact_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their inputs.
#![allow(
    clippy::arithmetic_side_effects,
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

use geom_fuzz::exact::{Case, MAX_FACES, MAX_INPUT_BYTES, check, decode, harness};
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 2_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 4_000;

/// The longest byte string either leg draws: the harness reads at most [`MAX_INPUT_BYTES`], and
/// the rest must change nothing.
const MAX_LEN: u16 = 64;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn exact_fuzz() {
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
fn exact_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "exact fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = vec![0_u8, MAX_FACES];
    data.extend((0..3 * MAX_FACES).map(|i| i % 27));
    assert_eq!(data.len(), MAX_INPUT_BYTES);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    assert_eq!(case.faces.len(), usize::from(MAX_FACES));

    let mut longer = data.clone();
    longer.extend([0xa5; 16]);
    assert_eq!(decode(&longer), (case, MAX_INPUT_BYTES));
}

#[test]
fn every_byte_string_decodes_and_short_ones_hold() {
    holds(harness(&[]));
    holds(harness(&[3]));
    holds(harness(&[255; 8]));
}

#[test]
fn a_closed_mesh_and_an_open_one_hold() {
    // A tetrahedron on the lattice, outward, then one face of it.
    let (a, b, c, d) = (0, 2, 6, 18);
    holds(check(&Case {
        scale: 1,
        faces: vec![[a, c, b], [a, b, d], [b, c, d], [c, a, d]],
    }));
    holds(check(&Case {
        scale: 1 << 38,
        faces: vec![[a, c, b]],
    }));
}
