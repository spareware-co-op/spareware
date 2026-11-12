//! The `PruneHistory` fuzz target (piece G109), as tests: `prune_fuzz` is bolero's test mode,
//! which cargo-bolero and `ClusterFuzzLite` select by name, and `prune_seeded` the seeded leg that
//! runs on every target, WebAssembly included (the crate README sets out the layout). Each hands
//! the fuzzer's raw bytes to `geom_fuzz::prune::harness`. The case's input bound is reached and
//! never passed, and the bytes after it reach the record's reader.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a broken law must stop the test loudly"
)]

mod common;

use common::holds;

use geom_fuzz::prune::{MAX_INPUT_BYTES, decode, harness};
use geom_fuzz::seeded;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 1_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 1_000;

/// The longest byte string the seeded leg draws: the case and a short payload after it.
const MAX_LEN: u16 = 160;

const _: () = assert!(MAX_LEN as usize > MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn prune_fuzz() {
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
fn prune_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "prune fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    // Six first faces, four steps, then both masks, the step and the edit; then a tail.
    let mut data = vec![5u8, 0, 4];
    data.extend([0u8; 59]);
    let (case, read) = decode(&data).unwrap();
    assert_eq!(read, MAX_INPUT_BYTES);
    assert!(case.tail.is_empty());
    data.extend([0x83, 0x81, 0x50]);
    let (case, read) = decode(&data).unwrap();
    assert_eq!((read, case.tail.as_slice()), (MAX_INPUT_BYTES, &[0x83, 0x81, 0x50][..]));
    holds(harness(&data));
    let mut g = SplitMix64::new(Seed(0x109));
    for _ in 0..500 {
        let len = usize::try_from(g.next_u64() % 120).unwrap();
        let data: Vec<u8> = (0..len).map(|_| g.next_u64().to_le_bytes()[0]).collect();
        let (case, read) = decode(&data).unwrap();
        assert!(read <= MAX_INPUT_BYTES);
        assert_eq!(read + case.tail.len(), data.len());
    }
}

#[test]
fn the_seeded_inputs_hold_names_and_reach_the_budget() {
    let mut g = SplitMix64::new(Seed::DEFAULT);
    let (mut held, mut steps) = (0u32, 0u32);
    for _ in 0..300 {
        let data: Vec<u8> = (0..MAX_INPUT_BYTES)
            .map(|_| g.next_u64().to_le_bytes()[0])
            .collect();
        let (case, _) = decode(&data).unwrap();
        held += u32::from(!case.held.names().is_empty());
        steps += u32::from(!case.element.history.is_empty());
        holds(harness(&data));
    }
    assert!(held > 100 && steps > 100, "{held} {steps}");
}
