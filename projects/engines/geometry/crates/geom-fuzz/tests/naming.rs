//! The persistent names fuzz target (piece G87), as tests: `naming_fuzz` is bolero's test mode,
//! which cargo-bolero and `ClusterFuzzLite` select by name, and `naming_seeded` the seeded leg
//! that runs on every target, WebAssembly included (the crate README sets out the layout). Each
//! hands the fuzzer's raw bytes to `geom_fuzz::naming::harness`. The input bound is reached and
//! never passed, and every kind of name, a stranger, an undecided entity and a tied pair are
//! reached by the seeded leg's inputs.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a broken law must stop the test loudly"
)]

mod common;

use common::holds;

use geom_fuzz::naming::{MAX_INPUT_BYTES, Parent, decode, harness};
use geom_fuzz::seeded;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 20_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 20_000;

/// The longest byte string the seeded leg draws: past the bound, so the bound is met.
const MAX_LEN: u16 = 96;

const _: () = assert!(MAX_LEN as usize > MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn naming_fuzz() {
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
fn naming_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "naming fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    // Every count at its most: 8 names, 12 list entries, 3 sets, 8 entities.
    let mut data = vec![8u8];
    data.extend([1u8; 40]);
    data.push(12);
    data.extend([1u8; 12]);
    data.push(3);
    data.extend([1u8; 3]);
    data.push(8);
    data.extend([1u8; 24]);
    assert_eq!(data.len(), MAX_INPUT_BYTES);
    data.extend([7u8; 50]);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    assert_eq!(
        (
            case.names.len(),
            case.list.len(),
            case.sets.len(),
            case.keys.len()
        ),
        (8, 12, 3, 8)
    );
    holds(harness(&data));
    let mut g = SplitMix64::new(Seed(0x87));
    for _ in 0..2_000 {
        let len = usize::try_from(g.next_u64() % 200).unwrap();
        let data: Vec<u8> = (0..len).map(|_| g.next_u64().to_le_bytes()[0]).collect();
        assert!(decode(&data).1 <= MAX_INPUT_BYTES);
    }
}

#[test]
fn the_seeded_inputs_reach_every_kind_and_the_hard_cases() {
    let mut g = SplitMix64::new(Seed::DEFAULT);
    let (mut kinds, mut strangers, mut undecided, mut ties) = ([0u32; 5], 0u32, 0u32, 0u32);
    for _ in 0..2_000 {
        let data: Vec<u8> = (0..MAX_INPUT_BYTES)
            .map(|_| g.next_u64().to_le_bytes()[0])
            .collect();
        let (case, _) = decode(&data);
        for d in &case.names {
            kinds[usize::from(d.kind % 5)] += 1;
            strangers += u32::from(d.parents.iter().any(|p| matches!(p, Parent::Stranger(_))));
        }
        undecided += u32::from(
            case.keys
                .iter()
                .any(|k| k.least.is_none() || k.vertices.is_none() || k.direction.is_none()),
        );
        ties += u32::from(
            case.keys
                .iter()
                .enumerate()
                .any(|(i, a)| case.keys[i + 1..].contains(a)),
        );
        holds(harness(&data));
    }
    assert!(kinds.iter().all(|&k| k > 0), "{kinds:?}");
    assert!(strangers > 0 && undecided > 0 && ties > 0);
}
