//! The cache fuzz target (G105), run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, and a wrong forgetting rule caught.
//!
//! - `cache_fuzz` wraps `geom_fuzz::cache::harness` in `bolero::check!`: the crate's corpus and
//!   crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`. Under `cargo bolero test cache_fuzz --package geom-fuzz` it is a
//!   libFuzzer target.
//! - `cache_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly; inserts, refused entries, gets and forgets all come up;
//!   and a store checked against the rule "forget the newest" fails.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
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

use geom_fuzz::cache::{MAX_INPUT_BYTES, MAX_OPS, Op, check, check_with, decode, harness};
use geom_fuzz::seeded;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 5_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 10_000;

/// The longest byte string the seeded leg draws: the harness reads at most [`MAX_INPUT_BYTES`];
/// the rest must change nothing.
const MAX_LEN: u16 = 128;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn cache_fuzz() {
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
fn cache_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "cache fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// The input that reads the most: [`MAX_OPS`] calls of four bytes each.
#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = vec![31, MAX_OPS];
    for i in 0..MAX_OPS {
        data.extend_from_slice(&[i % 4, i, 7, 0x35]);
    }
    assert_eq!(data.len(), MAX_INPUT_BYTES);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    assert_eq!(case.ops.len(), usize::from(MAX_OPS));
    data.extend_from_slice(&[1; 20]);
    assert_eq!(decode(&data), (case.clone(), MAX_INPUT_BYTES));
    holds(check(&case));
    assert_eq!(decode(&[]).1, 0);
    holds(check(&decode(&[]).0));
}

#[test]
fn every_kind_of_call_comes_up_and_a_wrong_rule_is_caught() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("cache outcomes", Seed::DEFAULT));
    let (mut inserts, mut refused, mut gets, mut forgets, mut newest_caught) = (0, 0, 0, 0, 0);
    for _ in 0..3000 {
        let mut data = vec![0u8; MAX_INPUT_BYTES];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        for op in &case.ops {
            match *op {
                Op::Insert { bytes, .. } => {
                    inserts += 1;
                    refused += u32::from(bytes > case.bound);
                }
                Op::Get { .. } => gets += 1,
                Op::Forget { .. } => forgets += 1,
            }
        }
        holds(check(&case));
        // A store held to "forget the newest first" disagrees whenever forgetting happens.
        if check_with(&case, |list| list.len().saturating_sub(1)).is_err() {
            newest_caught += 1;
        }
    }
    println!(
        "inserts {inserts} (refused {refused}), gets {gets}, forgets {forgets}, wrong rule caught \
         {newest_caught}"
    );
    assert!(inserts > 1000 && refused > 100 && gets > 1000 && forgets > 1000);
    assert!(newest_caught > 300, "{newest_caught}");
}
