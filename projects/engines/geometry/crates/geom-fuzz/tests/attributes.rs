//! The attributes fuzz target (G90), run as tests on stable Rust: bolero's test mode, the seeded
//! leg, the input bound, and a broken step caught.
//!
//! - `attributes_fuzz` wraps `geom_fuzz::attributes::harness` in `bolero::check!`: the crate's
//!   corpus and crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is
//!   printed as `BOLERO_RANDOM_SEED=<seed>`. Under `cargo bolero test attributes_fuzz --package
//!   geom-fuzz` it is a libFuzzer target.
//! - `attributes_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed
//!   seed (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly; every origin, a transform and copies come up; and a step
//!   that keeps every value, or loses the notes, is caught.

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

use geom_fuzz::attributes::{
    MAX_INPUT_BYTES, MAX_NAMES, MAX_RECORDS, check, check_with, decode, harness,
};
use geom_fuzz::seeded;
use geom_op::Origin;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 5_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 10_000;

/// The longest byte string the seeded leg draws: the harness reads at most [`MAX_INPUT_BYTES`];
/// the rest must change nothing.
const MAX_LEN: u16 = 160;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn attributes_fuzz() {
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
fn attributes_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "attributes fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// The input that reads the most: every name carries both values, and every record reads its
/// three inputs.
#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = vec![0u8; 1 + 12];
    for _ in 0..2 * MAX_NAMES {
        data.extend_from_slice(&[0, 1]);
    }
    data.push(MAX_RECORDS);
    for r in 0..MAX_RECORDS {
        data.extend_from_slice(&[0, r, 0, 1, 2]);
    }
    data.extend_from_slice(&[0, 0]);
    assert_eq!(data.len(), MAX_INPUT_BYTES);
    assert_eq!(decode(&data).1, MAX_INPUT_BYTES);
    data.extend_from_slice(&[5; 30]);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    assert_eq!(case.records.len(), usize::from(MAX_RECORDS));
    holds(check(&case));
    assert_eq!(decode(&[]).1, 0);
    holds(check(&decode(&[]).0));
}

/// A broken step: hands every input's values on unchanged.
#[allow(
    clippy::unnecessary_wraps,
    reason = "it stands in for `geom_name::apply`, whose type it must have"
)]
fn keep(
    _: &geom_topo::definition::Definitions,
    before: &geom_topo::attributes::Attributes,
    _: &geom_op::History,
    _: &geom_name::Step<'_>,
) -> geom_fuzz::attributes::Applied {
    Ok((before.clone(), Vec::new()))
}

/// A broken step: the real one, with its notes lost.
fn quiet(
    d: &geom_topo::definition::Definitions,
    b: &geom_topo::attributes::Attributes,
    h: &geom_op::History,
    s: &geom_name::Step<'_>,
) -> geom_fuzz::attributes::Applied {
    geom_name::apply(d, b, h, s).map(|(a, _)| (a, Vec::new()))
}

#[test]
fn every_origin_comes_up_and_a_broken_step_is_caught() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("attributes outcomes", Seed::DEFAULT));
    let mut origins = [0u32; 6];
    let (mut moved, mut copied, mut kept_all, mut silent) = (0, 0, 0, 0);
    for _ in 0..3000 {
        let mut data = vec![0u8; 130];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        for (_, o) in &case.records {
            let i = match o {
                Origin::Unchanged => 0,
                Origin::Modified { from } if from.names().len() == 1 => 1,
                Origin::Modified { .. } => 2,
                Origin::Split { .. } => 3,
                Origin::Merged { .. } => 4,
                Origin::Generated { .. } => 5,
            };
            origins[i] += 1;
        }
        moved += u32::from(case.transform.is_some());
        copied += u32::from(case.copies);
        holds(check(&case));
        // A step that hands every input's values on unchanged is caught.
        if check_with(&case, keep).is_err() {
            kept_all += 1;
        }
        // A step that loses its notes is caught whenever a value was dropped.
        if check_with(&case, quiet).is_err() {
            silent += 1;
        }
    }
    println!(
        "origins {origins:?}, transforms {moved}, copies {copied}, keep-all caught {kept_all}, silent caught {silent}"
    );
    assert!(origins.iter().all(|&c| c > 100), "{origins:?}");
    assert!(moved > 500 && copied > 500 && kept_all > 500 && silent > 20);
}
