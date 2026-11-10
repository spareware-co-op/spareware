//! The batch fuzz target (G104), run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, and a broken driver caught.
//!
//! - `batch_fuzz` wraps `geom_fuzz::batch::harness` in `bolero::check!`: the crate's corpus and
//!   crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`. Under `cargo bolero test batch_fuzz --package geom-fuzz` it is a
//!   libFuzzer target.
//! - `batch_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly; repeated keys, item failures, own budgets and batch totals
//!   all come up; and a driver that drops a result, or ignores the total, is caught.

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

use geom_fuzz::batch::{
    MAX_INPUT_BYTES, MAX_ITEMS, MAX_PARAMS, check, check_with, decode, harness,
};
use geom_fuzz::seeded;
use geom_run::Exec;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 5_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 10_000;

/// The longest byte string the seeded leg draws: the harness reads at most [`MAX_INPUT_BYTES`];
/// the rest must change nothing.
const MAX_LEN: u16 = 96;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn batch_fuzz() {
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
fn batch_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "batch fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// The input that reads the most: [`MAX_ITEMS`] items and [`MAX_PARAMS`] parameters.
#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = vec![MAX_ITEMS, 10, 0];
    for i in 0..MAX_ITEMS {
        data.extend_from_slice(&[i, 3]);
    }
    data.push(MAX_PARAMS);
    for i in 0..MAX_PARAMS {
        data.extend_from_slice(&[i * 8, 255, i]);
    }
    assert_eq!(data.len(), MAX_INPUT_BYTES);
    assert_eq!(decode(&data).1, MAX_INPUT_BYTES);
    data.extend_from_slice(&[9; 30]);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    assert_eq!(case.items.len(), usize::from(MAX_ITEMS));
    holds(check(&case));
    assert_eq!(decode(&[]).1, 0);
    holds(check(&decode(&[]).0));
}

#[test]
fn every_rule_comes_up_and_a_broken_driver_is_caught() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("batch outcomes", Seed::DEFAULT));
    let (mut repeated, mut totals, mut dropped, mut ignored) = (0, 0, 0, 0);
    for _ in 0..3000 {
        let mut data = vec![0u8; 60];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        let mut keys: Vec<_> = case.items.iter().map(|i| i.key).collect();
        keys.sort_unstable();
        keys.dedup();
        if keys.len() < case.items.len() {
            repeated += 1;
        } else if case.total.is_some() {
            totals += 1;
        }
        // A driver that drops the last result is caught whenever the batch runs.
        let drop_last = |b: &geom_op::Batch<'_>, items: &[geom_fuzz::batch::Item]| {
            let mut r = b.run(&Exec::serial(), items, |i| i.key, |_, i, _| Ok(i.work))?;
            r.pop();
            Ok(r)
        };
        if !case.items.is_empty()
            && keys.len() == case.items.len()
            && check_with(&case, drop_last).is_err()
        {
            dropped += 1;
        }
        // A driver that runs every item alone, ignoring the total, is caught when the total cuts.
        let alone = |b: &geom_op::Batch<'_>, items: &[geom_fuzz::batch::Item]| {
            Ok(items
                .iter()
                .map(|i| {
                    b.single(i.key, i, |_, i, m: &mut geom_run::WorkMeter<'_>| {
                        for _ in 0..i.work {
                            m.charge(1).map_err(|_| {
                                geom_op::OpError::new(
                                    b.path().clone(),
                                    geom_op::RunDetail::Cancelled,
                                )
                            })?;
                        }
                        if i.fails {
                            return Err(geom_op::OpError::new(
                                b.path().item(i.key),
                                geom_num::NumError::Degenerate,
                            ));
                        }
                        Ok(i.work)
                    })
                })
                .collect())
        };
        if check_with(&case, alone).is_err() {
            ignored += 1;
        }
    }
    println!(
        "repeated {repeated}, totals {totals}, dropped caught {dropped}, total ignored caught {ignored}"
    );
    assert!(repeated > 100 && totals > 100 && dropped > 100 && ignored > 20);
}
