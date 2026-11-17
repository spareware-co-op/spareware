//! The booleans fuzz target, run as tests on stable Rust: bolero's test mode, the seeded leg, the
//! input bound, and boxes that overlap, touch, stand apart and wall rooms reached.
//!
//! - `booleans_fuzz` wraps `geom_fuzz::booleans::harness` in `bolero::check!`: the target's stored
//!   corpus and crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is
//!   printed as `BOLERO_RANDOM_SEED=<seed>`.
//! - `booleans_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their inputs.
#![allow(
    clippy::arithmetic_side_effects,
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

use geom_fuzz::booleans::{Case, MAX_BOXES, MAX_INPUT_BYTES, check, decode, harness};
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 20;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 40;

/// The longest byte string either leg draws: the harness reads at most [`MAX_INPUT_BYTES`], and
/// the rest must change nothing.
const MAX_LEN: u16 = 40;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics there,
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn booleans_fuzz() {
    println!(
        "bolero test mode: corpus and crash files, then {ITERATIONS} random inputs whose seeds \\
         bolero draws from the operating system; a failing input's seed is printed as \\
         BOLERO_RANDOM_SEED=<seed>, which replays it"
    );
    bolero::check!()
        .with_iterations(ITERATIONS)
        .with_max_len(usize::from(MAX_LEN))
        .for_each(|data: &[u8]| holds(harness(data)));
}

#[test]
fn booleans_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "booleans fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    // Three boxes: a scale byte, a count byte that gives the most boxes, six bytes a box, then
    // the half-space, the rod, the move and the seeds.
    let mut data = vec![0_u8, MAX_BOXES - 2];
    data.extend((0..6 * MAX_BOXES).map(|i| i % 5));
    data.extend([1, 2, 0, 1, 3, 0x29, 7, 200]);
    assert_eq!(data.len(), MAX_INPUT_BYTES);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    assert_eq!(case.boxes.len(), usize::from(MAX_BOXES));
    assert_eq!(case.cut, (1, 3));
    assert_eq!(case.rod, (0, [1, 3]));
    assert_eq!(case.moved, (1, 2, -1));
    assert_eq!(case.seeds, [7, 200]);

    let mut longer = data.clone();
    longer.extend([0xa5; 16]);
    assert_eq!(decode(&longer), (case, MAX_INPUT_BYTES));
}

#[test]
fn every_byte_string_decodes_and_short_ones_hold() {
    holds(harness(&[]));
    holds(harness(&[3]));
    holds(harness(&[255; 8]));
}

#[test]
fn boxes_that_overlap_touch_stand_apart_and_wall_rooms_hold() {
    // Overlapping, with the rod through both and seeds either side.
    holds(check(&Case {
        scale: 2,
        boxes: vec![([0, 0, 0], [3, 3, 3]), ([1, 1, 1], [4, 4, 4])],
        cut: (2, 2),
        rod: (0, [1, 1]),
        moved: (0, 0, 1),
        seeds: [0, 9],
    }));
    // Three boxes sharing faces: the fast path's whole shared face, and a wall across the room.
    holds(check(&Case {
        scale: 6,
        boxes: vec![
            ([0, 0, 0], [2, 4, 4]),
            ([2, 0, 0], [4, 4, 4]),
            ([1, 1, 1], [3, 3, 3]),
        ],
        cut: (0, 1),
        rod: (2, [0, 3]),
        moved: (1, 1, -1),
        seeds: [1, 2],
    }));
    // Apart, near the edge of the frame, the tool moved away.
    holds(check(&Case {
        scale: 1 << 36,
        boxes: vec![([0, 0, 0], [1, 1, 1]), ([3, 3, 3], [4, 4, 4])],
        cut: (1, 3),
        rod: (1, [0, 0]),
        moved: (0, 2, 1),
        seeds: [0, 40],
    }));
    // A wall that splits the room in two, seeds on either side.
    holds(check(&Case {
        scale: 2,
        boxes: vec![([2, 0, 0], [3, 4, 4]), ([0, 0, 0], [1, 1, 1])],
        cut: (0, 3),
        rod: (0, [2, 2]),
        moved: (0, 1, 1),
        seeds: [0, 60],
    }));
}
