//! The intersect fuzz target (G158), run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, and what it reaches.
//!
//! - `intersect_fuzz` wraps `geom_fuzz::intersect::harness` in `bolero::check!`: the crate's corpus
//!   and crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`. Under `cargo bolero test intersect_fuzz --package geom-fuzz` it is
//!   a libFuzzer target.
//! - `intersect_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly, and the cases reach both verdicts, every kind of operand
//!   and every number of unknowns.

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

use geom_fuzz::intersect::{
    Case, Drawn, MAX_INPUT_BYTES, MAX_ITEMS, Shape, check, decode, harness,
};
use geom_fuzz::seeded;
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 2_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 5_000;

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
fn intersect_fuzz() {
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
fn intersect_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "intersect fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

/// The input that reads the most: [`MAX_ITEMS`] items.
#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = vec![0; 8 + 3];
    data.push(MAX_ITEMS);
    data.extend_from_slice(&[0; 12 * 9]);
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

/// Items that fall apart along the first unknown, except one that overlaps across another.
fn spread(starts: &[u8]) -> Case {
    Case {
        shapes: [
            Shape {
                kind: 0,
                at: [0, 0, 0],
            },
            Shape {
                kind: 2,
                at: [1, 2, 3],
            },
        ],
        unknowns: 2,
        certified: false,
        work: 200,
        items: starts
            .iter()
            .map(|&s| Drawn {
                kind: 0,
                spans: [(s, 0), (s, 0), (0, 0), (0, 0)],
            })
            .collect(),
    }
}

#[test]
fn both_verdicts_pass_the_oracle() {
    // Disjoint points.
    holds(check(&spread(&[1, 3, 5])));
    // Two the same: refused, and the oracle agrees, so the target finds nothing.
    holds(check(&spread(&[1, 1])));
    // A flagged item under a certified topology is refused too.
    let mut case = spread(&[1]);
    case.items[0].kind = 3;
    case.certified = true;
    holds(check(&case));
}

#[test]
fn the_cases_reach_every_shape_of_answer() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("intersect outcomes", Seed::DEFAULT));
    let mut unknowns = [0; 5];
    let mut kinds = [0; 4];
    let mut shapes = [0; 4];
    for _ in 0..2000 {
        let mut data = [0u8; 120];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        holds(check(&case));
        unknowns[usize::from(case.unknowns)] += 1;
        for item in &case.items {
            kinds[usize::from(item.kind % 4)] += 1;
        }
        for shape in &case.shapes {
            shapes[usize::from(shape.kind % 4)] += 1;
        }
    }
    println!("{unknowns:?} {kinds:?} {shapes:?}");
    assert!(unknowns[2..].iter().all(|&n| n > 300));
    assert!(kinds.iter().all(|&n| n > 300));
    assert!(shapes.iter().all(|&n| n > 300));
}
