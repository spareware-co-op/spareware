//! The DEFLATE reader's fuzz target (piece G414b), as tests: `inflate_fuzz` is bolero's test mode,
//! which cargo-bolero, `ClusterFuzzLite` and Kani select by name, and `inflate_seeded` the seeded
//! leg that runs on every target, WebAssembly included (the crate README sets out the layout).
//! Each hands the fuzzer's raw bytes to `geom_fuzz::inflate::harness`, which reads them with
//! `geom-exchange`'s inflate and with `miniz_oxide`.
//!
//! `seeded_streams` adds a leg random bytes rarely reach: streams `miniz_oxide` compresses at every
//! level, then cut, extended or with bits flipped, so the decoder's dynamic tables, long matches and
//! stored blocks run on every pull request too.

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

use common::holds;

use geom_fuzz::inflate::harness;
use geom_fuzz::seeded;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 20_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 20_000;

/// The longest byte string the fuzzing legs draw.
const MAX_LEN: u16 = 1_024;

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn inflate_fuzz() {
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
fn inflate_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "inflate fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn seeded_streams() {
    let seed = resolve("inflate fuzz target, streams", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let cases = 2_000;
    for _ in 0..cases {
        let len = (rng.next_u64() % 3_000) as usize;
        let alphabet = 1 + rng.next_u64() % 256;
        let data: Vec<u8> = (0..len)
            .map(|_| (rng.next_u64() % alphabet) as u8)
            .collect();
        let level = (rng.next_u64() % 11) as u8;
        let mut s = miniz_oxide::deflate::compress_to_vec(&data, level);
        match rng.next_u64() % 4 {
            0 => {}
            1 => {
                let keep = (rng.next_u64() % (s.len() as u64 + 1)) as usize;
                s.truncate(keep);
            }
            2 => s.push(rng.next_u64() as u8),
            _ => {
                if !s.is_empty() {
                    let at = (rng.next_u64() % s.len() as u64) as usize;
                    s[at] ^= 1 << (rng.next_u64() % 8);
                }
            }
        }
        holds(harness(&s));
    }
}
