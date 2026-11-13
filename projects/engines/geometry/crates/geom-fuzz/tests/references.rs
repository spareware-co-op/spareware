//! The reference resolution fuzz target (piece G89), as tests: `references_fuzz` is bolero's test
//! mode, which cargo-bolero and `ClusterFuzzLite` select by name, and `references_seeded` the
//! seeded leg that runs on every target, WebAssembly included (the crate README sets out the
//! layout). Each hands the fuzzer's raw bytes to `geom_fuzz::references::harness`. The input bound
//! is reached and never passed, and the seeded leg's inputs reach every outcome of 20.4's table.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a broken law must stop the test loudly"
)]

mod common;

use std::collections::BTreeSet;

use common::holds;

use geom_fuzz::references::{MAX_INPUT_BYTES, decode, expected, harness};
use geom_fuzz::seeded;
use geom_name::{Resolution, UnresolvedReason};
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 5_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 5_000;

/// The longest byte string the seeded leg draws: past the bound, so the bound is met.
const MAX_LEN: u16 = 80;

const _: () = assert!(MAX_LEN as usize > MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn references_fuzz() {
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
fn references_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "references fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    // Six first faces, four steps, then every byte of the reference.
    let mut data = vec![5u8, 0, 4];
    data.extend([0u8; 63]);
    let (_, read) = decode(&data).unwrap();
    assert_eq!(read, MAX_INPUT_BYTES);
    let mut g = SplitMix64::new(Seed(0x89));
    for _ in 0..2_000 {
        let len = usize::try_from(g.next_u64() % 120).unwrap();
        let data: Vec<u8> = (0..len).map(|_| g.next_u64().to_le_bytes()[0]).collect();
        assert!(decode(&data).unwrap().1 <= MAX_INPUT_BYTES);
    }
}

/// The name of an outcome, for counting which the seeded inputs reach.
fn outcome(r: &Resolution) -> String {
    match r {
        Resolution::Exact { .. } => "Exact".into(),
        Resolution::FollowedHistory { .. } => "FollowedHistory".into(),
        Resolution::Ambiguous { .. } => "Ambiguous".into(),
        Resolution::WeakChanged { .. } => "WeakChanged".into(),
        Resolution::Deleted { .. } => "Deleted".into(),
        Resolution::HistoryPruned { .. } => "HistoryPruned".into(),
        Resolution::Unresolved { reason } => format!("{reason:?}"),
    }
}

#[test]
fn the_seeded_inputs_reach_every_outcome() {
    let mut g = SplitMix64::new(Seed::DEFAULT);
    let mut seen = BTreeSet::new();
    for _ in 0..20_000 {
        let data: Vec<u8> = (0..MAX_INPUT_BYTES)
            .map(|_| g.next_u64().to_le_bytes()[0])
            .collect();
        let (case, _) = decode(&data).unwrap();
        seen.insert(outcome(&expected(&case)));
        holds(harness(&data));
    }
    let reasons = [
        UnresolvedReason::ElementGone,
        UnresolvedReason::LinkMissing,
        UnresolvedReason::LinkOlder,
        UnresolvedReason::NoCarrier,
        UnresolvedReason::NameUnknown,
        UnresolvedReason::HistoryBroken,
    ];
    for want in [
        "Exact",
        "FollowedHistory",
        "Ambiguous",
        "WeakChanged",
        "Deleted",
        "HistoryPruned",
    ]
    .into_iter()
    .map(String::from)
    .chain(reasons.iter().map(|r| format!("{r:?}")))
    {
        assert!(seen.contains(&want), "{want} not reached: {seen:?}");
    }
}
