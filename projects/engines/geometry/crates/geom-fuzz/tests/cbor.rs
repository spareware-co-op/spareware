//! The CBOR fuzz target (piece G91), as tests: `cbor_fuzz` is bolero's test mode, which
//! cargo-bolero, `ClusterFuzzLite` and Kani select by name, and `cbor_seeded` the seeded leg that
//! runs on every target, WebAssembly included (the crate README sets out the layout). Each hands
//! the fuzzer's raw bytes to `geom_fuzz::cbor::harness`, which reads them as `geom-format`'s
//! deterministic CBOR and as its text form.
//!
//! `seeded_text_cases` adds a leg random bytes rarely reach: byte strings built from the text
//! form's own characters, so the parser's paths run on every pull request too.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use core::num::NonZeroU64;

use common::holds;

use geom_fuzz::cbor::{check_text, harness};
use geom_fuzz::seeded;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 20_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 20_000;

/// The longest byte string the seeded leg draws; the harness reads all of it.
const MAX_LEN: u16 = 512;

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn cbor_fuzz() {
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
fn cbor_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "cbor fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn seeded_text_cases() {
    let seed = resolve("cbor fuzz target, text", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let tokens = [
        "[",
        "]",
        "{",
        "}",
        "(",
        ")",
        ",",
        ":",
        " ",
        "\n",
        "/c/",
        "\"",
        "\\",
        "\\u00e9",
        "\\ud834",
        "\\udd1e",
        "h'",
        "'",
        "0",
        "1",
        "9",
        "-",
        "0x",
        "1.8",
        "p",
        "+",
        "-1074",
        "1023",
        "f",
        "2(",
        "3(",
        "79(",
        "86(",
        "true",
        "false",
        "null",
        "a",
        "é",
        "18446744073709551616",
    ];
    for _ in 0..20_000 {
        let n = rng.below(NonZeroU64::new(24).unwrap());
        let text: String = (0..n)
            .map(|_| tokens[rng.below(NonZeroU64::new(tokens.len() as u64).unwrap()) as usize])
            .collect();
        holds(check_text(&text));
    }
}
