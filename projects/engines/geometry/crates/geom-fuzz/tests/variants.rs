//! The variants fuzz target (piece G42), as tests: `variants_fuzz` is bolero's test mode, which
//! cargo-bolero, `ClusterFuzzLite` and Kani select by name, and `variants_seeded` the seeded leg
//! that runs on every target, WebAssembly included (the crate README sets out the layout). Each
//! hands the fuzzer's raw bytes to `geom_fuzz::variants::harness`, which checks every variant of
//! every generated exact path against the big-integer value of its description.
//!
//! Fewer cases than the predicates target's: each case evaluates every registered description in
//! big integers and calls 33 exact paths.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
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
use geom_fuzz::variants::{MAX_INPUT_BYTES, harness};
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 2_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 4_000;

/// The longest byte string the seeded leg draws. The harness reads at most
/// [`MAX_INPUT_BYTES`] (620) bytes (derived in `geom_fuzz::variants`); the 80 more let some
/// strings carry bytes nobody reads, which must change nothing.
const MAX_LEN: u16 = 700;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn variants_fuzz() {
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
fn variants_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "variants fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}
