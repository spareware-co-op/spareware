//! The fuzz target of containers and trust on open (piece G108), as tests: `open_fuzz` is
//! bolero's test mode, which cargo-bolero and `ClusterFuzzLite` select by name, and `open_seeded`
//! the seeded leg that runs on every target, WebAssembly included (the crate README sets out the
//! layout). Each hands the fuzzer's raw bytes to `geom_fuzz::open::harness`. The corpus starts
//! from the container of `geom-format`'s trust-on-open tests, a stamped cube and an unstamped
//! sheet, written as a file, behind each edit.

#![allow(clippy::panic, reason = "a broken law must stop the test loudly")]

mod common;

use common::holds;

use geom_fuzz::open::harness;
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 2_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 2_000;

/// The longest byte string the seeded leg draws; the harness reads all of it.
const MAX_LEN: u16 = 512;

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn open_fuzz() {
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
fn open_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "open fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}
