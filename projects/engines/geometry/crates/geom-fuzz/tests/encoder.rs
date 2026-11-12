//! The canonical encoder's fuzz target (piece G86), as tests: `encoder_fuzz` is bolero's test
//! mode, which cargo-bolero and `ClusterFuzzLite` select by name, and `encoder_seeded` the seeded
//! leg that runs on every target, WebAssembly included (the crate README sets out the layout).
//! Each hands the fuzzer's raw bytes to `geom_fuzz::encoder::harness`.
//!
//! `the_check_catches_what_is_not_canonical` is `geom-op`'s own check of the harness, moved with
//! it: a stream that writes a known item reads back, and bytes that are not canonical are caught.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a broken law must stop the test loudly"
)]

mod common;

use common::holds;

use geom_fuzz::encoder::{drive, harness, rewritten};
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 5_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 5_000;

/// The longest byte string the seeded leg draws; the harness reads all of it.
const MAX_LEN: u16 = 1_500;

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn encoder_fuzz() {
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
fn encoder_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "encoder fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_check_catches_what_is_not_canonical() {
    // An array of two (op 7, length 2), then two empty texts (op 4, length 0).
    let finished = drive(&[7, 2, 4, 0, 4, 0]).unwrap();
    assert_eq!(finished, [vec![0x82, 0x60, 0x60]]);
    assert_eq!(rewritten(&finished[0]).as_deref(), Ok(&finished[0][..]));
    // A head in two bytes where one does, a double wider than it needs, a map key out of order, a
    // byte after the item, a negative head in two bytes, and a map key that is not unsigned.
    for bad in [
        &[0x18, 0x05][..],
        &[0xfa, 0x3f, 0x80, 0x00, 0x00],
        &[0xa2, 0x02, 0x00, 0x01, 0x00],
        &[0x01, 0x01],
        &[0x20 | 0x18, 0x02],
        &[0xa1, 0x20, 0x00],
    ] {
        let again = rewritten(bad);
        assert!(
            again.as_deref() != Ok(bad),
            "{bad:02x?} passed as canonical"
        );
    }
}
