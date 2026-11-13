//! The operation contract's fuzz target (piece G82), as tests: `contract_fuzz` is bolero's test
//! mode, which cargo-bolero and `ClusterFuzzLite` select by name, and `contract_seeded` the seeded
//! leg that runs on every target, WebAssembly included (the crate README sets out the layout).
//! Each hands the fuzzer's raw bytes to `geom_fuzz::contract::harness`. The case's input bound is
//! reached and never passed, and the seeded inputs reach errors, warnings and tool descriptions.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a broken law must stop the test loudly"
)]

mod common;

use common::holds;

use geom_fuzz::contract::{MAX_INPUT_BYTES, decode, harness};
use geom_fuzz::seeded;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 1_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 1_000;

/// The longest byte string the seeded leg draws: the case and a few bytes past it.
const MAX_LEN: u16 = 400;

const _: () = assert!(MAX_LEN as usize > MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn contract_fuzz() {
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
fn contract_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "contract fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// The input that reads the most: every count at its most, every label a step's name, and every
/// message four bytes of text.
fn longest() -> Vec<u8> {
    let path = || {
        let mut p = vec![0u8, 0, 4];
        p.extend([0u8; 4 * 6]);
        p
    };
    let mut data = path();
    data.extend(path());
    data.push(2);
    for _ in 0..2 {
        data.extend([0u8; 10]);
        data.push(3);
        data.extend([0u8; 6]);
        data.push(3);
        data.extend([0u8; 30]);
        data.push(3);
        data.extend([0u8; 30]);
        data.push(2);
        data.extend([0u8; 4]);
    }
    data.push(2);
    for _ in 0..2 {
        data.push(0);
        data.push(3);
        data.extend([0u8; 6]);
        data.push(2);
        data.extend([0u8; 20]);
        data.push(0);
    }
    data.extend([0u8; 28]);
    data
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = longest();
    assert_eq!(data.len(), MAX_INPUT_BYTES);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    assert_eq!((case.errors.len(), case.warnings.len()), (2, 2));
    assert!(case.paths.iter().all(|p| p.labels.len() == 4));
    assert!(case.errors.iter().all(|e| {
        e.entities.len() == 3
            && e.evidence.len() == 3
            && e.remedies.len() == 3
            && e.message.is_some()
    }));
    holds(harness(&data));
    data.extend([0xFF; 40]);
    let (_, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    holds(harness(&data));
    let mut g = SplitMix64::new(Seed(0x82));
    for _ in 0..500 {
        let len = usize::try_from(g.next_u64() % 400).unwrap();
        let data: Vec<u8> = (0..len).map(|_| g.next_u64().to_le_bytes()[0]).collect();
        let (_, read) = decode(&data);
        assert!(read <= MAX_INPUT_BYTES && read <= data.len());
    }
}

#[test]
fn the_seeded_inputs_reach_errors_warnings_and_tools() {
    let mut g = SplitMix64::new(Seed::DEFAULT);
    let (mut errors, mut warnings, mut chosen, mut steps) = (0u32, 0u32, 0u32, 0u32);
    for _ in 0..300 {
        let data: Vec<u8> = (0..MAX_INPUT_BYTES)
            .map(|_| g.next_u64().to_le_bytes()[0])
            .collect();
        let (case, _) = decode(&data);
        errors += u32::from(!case.errors.is_empty());
        warnings += u32::from(!case.warnings.is_empty());
        chosen += u32::from(case.chosen != 0);
        steps += u32::from(case.paths.iter().any(|p| {
            p.labels
                .iter()
                .any(|l| matches!(l, geom_fuzz::contract::LabelDraw::Step(_)))
        }));
        holds(harness(&data));
    }
    assert!(
        errors > 150 && warnings > 150 && chosen > 250 && steps > 150,
        "{errors} {warnings} {chosen} {steps}"
    );
}
