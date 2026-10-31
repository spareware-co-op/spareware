//! The arenas fuzz target (G80), run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, and a broken publish caught.
//!
//! - `arenas_fuzz` wraps `geom_fuzz::arenas::harness` in `bolero::check!`: the crate's corpus and
//!   crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`. Under `cargo bolero test arenas_fuzz --package geom-fuzz` it is
//!   a libFuzzer target.
//! - `arenas_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly; every outcome of publishing (a body, each kind of fault)
//!   is reached; and a publish that refuses every body, or accepts a ragged one, is caught.

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

use geom_fuzz::arenas::{
    Case, MAX_EDITS, MAX_INPUT_BYTES, MAX_ROWS, check, check_with, decode, harness,
};
use geom_fuzz::seeded;
use geom_test::{Seed, SplitMix64};
use geom_topo::{Body, TopoError};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 5_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 10_000;

/// The longest byte string the seeded leg draws: the harness reads at most [`MAX_INPUT_BYTES`];
/// the rest must change nothing.
const MAX_LEN: u16 = 1200;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn arenas_fuzz() {
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
fn arenas_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "arenas fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// The input that reads the most: every arena at [`MAX_ROWS`], every link read from the input,
/// an uneven column, the outside region read from the input, and [`MAX_EDITS`] edits.
#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = vec![1];
    data.extend_from_slice(&[MAX_ROWS; 11]);
    for _ in 0..22 * usize::from(MAX_ROWS) {
        data.push(4);
        data.extend_from_slice(&[0; 8]);
    }
    data.extend_from_slice(&[0; 5]); // region kinds
    data.extend_from_slice(&[0, 3]); // an uneven column, longer
    data.push(4); // the outside region, read from the input
    data.extend_from_slice(&[0; 8]);
    data.push(MAX_EDITS);
    for _ in 0..MAX_EDITS {
        data.extend_from_slice(&[0, 0, 4]);
        data.extend_from_slice(&[0; 8]);
    }
    assert_eq!(data.len(), MAX_INPUT_BYTES);
    let (_, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    data.extend_from_slice(&[7; 100]);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    holds(check(&case));
    // The empty input is a case too.
    let (empty, read) = decode(&[]);
    assert_eq!(read, 0);
    holds(check(&empty));
}

/// The result of publishing a decoded case, by the builder.
fn outcome(case: &Case) -> &'static str {
    let seen = std::cell::Cell::new("body");
    let _ = check_with(case, |b, kind, outside, meter| {
        let r = b.publish(kind, outside, meter);
        if let Err(e) = &r {
            seen.set(match e {
                TopoError::Ragged(_) => "ragged",
                TopoError::Dangling { .. } => "dangling",
                TopoError::Outside => "outside",
                _ => "other",
            });
        }
        r
    });
    seen.get()
}

#[test]
fn every_outcome_is_reached_and_a_broken_publish_is_caught() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("arenas outcomes", Seed::DEFAULT));
    let mut seen = std::collections::BTreeMap::new();
    let mut refused_all = 0;
    for _ in 0..4000 {
        let len = rng.below(std::num::NonZeroU64::new(400).unwrap()) as usize;
        let mut data = vec![0u8; len];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        *seen.entry(outcome(&case)).or_insert(0) += 1;
        // A publish that refuses every body is caught whenever the rules allow one.
        let refuse = |_: geom_topo::Builder,
                      _,
                      _,
                      _: &mut geom_run::WorkMeter<'_>|
         -> Result<Body, TopoError> { Err(TopoError::Outside) };
        if check_with(&case, refuse).is_err() {
            refused_all += 1;
        }
    }
    println!("outcomes: {seen:?}; a refusing publish caught {refused_all} times");
    for kind in ["body", "ragged", "dangling", "outside"] {
        assert!(
            seen.get(kind).copied().unwrap_or(0) > 20,
            "{kind}: {seen:?}"
        );
    }
    assert_eq!(seen.get("other"), None);
    assert!(refused_all > 20);
}
