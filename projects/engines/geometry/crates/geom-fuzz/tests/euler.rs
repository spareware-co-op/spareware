//! The Euler operators' fuzz target (piece G81), as tests: `euler_fuzz` is bolero's test mode,
//! which cargo-bolero and `ClusterFuzzLite` select by name, and `euler_seeded` the seeded leg that
//! runs on every target, WebAssembly included (the crate README sets out the layout). Each hands
//! the fuzzer's raw bytes to `geom_fuzz::euler::harness`. The input bound is reached and never
//! passed, and the seeded leg's inputs apply every operator.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a broken law must stop the test loudly"
)]

mod common;

use common::holds;

use geom_fuzz::euler::{MAX_INPUT_BYTES, MAX_STEPS, decode, harness};
use geom_fuzz::seeded;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 5_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 5_000;

/// The longest byte string the seeded leg draws: past the bound, so the bound is met.
const MAX_LEN: u16 = 160;

const _: () = assert!(MAX_LEN as usize > MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn euler_fuzz() {
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
fn euler_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "euler fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let data = vec![13u8; MAX_INPUT_BYTES + 40];
    let (case, read) = decode(&data);
    assert_eq!((case.steps.len(), read), (MAX_STEPS, MAX_INPUT_BYTES));
    holds(harness(&data));
    assert_eq!(decode(&[1, 2, 3]).0.steps.len(), 0);
}

/// Wires first, then random steps: every operator applies at least once over the seeded inputs.
#[test]
fn the_seeded_inputs_apply_every_operator() {
    use geom_fuzz::euler::applied;
    let mut g = SplitMix64::new(Seed::DEFAULT);
    let mut seen = [0u32; 13];
    for _ in 0..3_000 {
        let mut data: Vec<u8> = (0..MAX_INPUT_BYTES)
            .map(|_| g.next_u64().to_le_bytes()[0])
            .collect();
        // Start with a wire, so the edits have rows to act on.
        data[0] = 0;
        data[1] = 0;
        for (op, n) in applied(&decode(&data).0).into_iter().enumerate() {
            seen[op] += n;
        }
        holds(harness(&data));
    }
    assert!(seen.iter().all(|&n| n > 0), "{seen:?}");
}
