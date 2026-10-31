//! The box tree's and cell index's fuzz target, run as tests on stable Rust: bolero's test mode,
//! the seeded leg, the input bound, and touching, nested and coincident boxes.
//!
//! - `boxtree_fuzz` wraps `geom_fuzz::boxtree::harness` in `bolero::check!`. Under `cargo test`
//!   it replays the files in `tests/__fuzz__/boxtree/corpus` and `crashes`, if any, then runs
//!   [`ITERATIONS`] random byte strings; bolero prints a failing one's seed as
//!   `BOLERO_RANDOM_SEED=<seed>`, which replays it. Under
//!   `cargo bolero test boxtree_fuzz --package geom-fuzz` the same test is a libFuzzer target.
//! - `boxtree_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed
//!   seed (`SPAREKIT_SEED` replays it), the same on every target.
//! - `touching_nested_and_coincident_boxes_equal_brute_force`: the queries on chosen cases.

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
use geom_fuzz::boxtree::{Case, MAX_BOXES, MAX_INPUT_BYTES, MAX_POINTS, check, decode, harness};
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the fuzz target in bolero's test mode.
const ITERATIONS: usize = 2_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 2_000;

/// The longest byte string the fuzz target and the seeded leg try. Most cases read far fewer than
/// [`MAX_INPUT_BYTES`] (3118) bytes; the longest strings reach the far coordinates.
const MAX_LEN: u16 = 3200;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[test]
fn boxtree_fuzz() {
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
fn boxtree_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "boxtree fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let max = u8::try_from(MAX_BOXES).unwrap();
    let mut bytes = vec![1, max, max, max];
    let far = |b: &mut Vec<u8>, n: usize| {
        for _ in 0..n {
            b.push(255);
            b.extend([0x5A; 8]);
        }
    };
    far(&mut bytes, (3 * MAX_BOXES + 1) * 6);
    bytes.push(u8::try_from(MAX_POINTS).unwrap());
    far(&mut bytes, MAX_POINTS * 3 + 3);
    bytes.extend([9; 8]);
    bytes.extend([1; 40]);
    assert_eq!(decode(&bytes).1, MAX_INPUT_BYTES);
    assert_eq!(decode(&[]).1, 0);
}

#[test]
fn touching_nested_and_coincident_boxes_equal_brute_force() {
    let b = |lo: [i64; 3], hi: [i64; 3]| (lo, hi);
    for space in [false, true] {
        let case = Case {
            space,
            boxes: vec![
                b([0, 0, 0], [2, 2, 2]),
                b([2, 2, 2], [4, 4, 4]),
                b([0, 0, 0], [2, 2, 2]),
                b([1, 1, 1], [1, 1, 1]),
                b([5, 5, 5], [6, 6, 6]),
            ],
            others: vec![b([2, 0, 0], [3, 1, 1]), b([-3, -3, -3], [-1, -1, -1])],
            moved: vec![b([5, 5, 5], [7, 7, 7])],
            query: b([2, 2, 2], [2, 2, 2]),
            points: vec![[0, 0, 0], [3, 4, 0], [0, 0, 5], [3, 4, 0]],
            center: [0, 0, 0],
            radius: 5,
        };
        holds(check(&case));
    }
}
