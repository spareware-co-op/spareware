//! The placed fuzz target (G145), run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, and hand cases over all 48 signed permutations, a general placement, a scale,
//! an index past the table and packed bytes past 47.
//!
//! - `placed_fuzz` wraps `geom_fuzz::placed::harness` in `bolero::check!`, bolero's test mode:
//!   the target's stored corpus and crash files, then [`ITERATIONS`] random byte strings; a
//!   failing one's seed is printed as `BOLERO_RANDOM_SEED=<seed>`.
//! - `placed_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
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

use geom_fuzz::placed::{Case, General, MAX_INPUT_BYTES, Probe, Shape, check, decode, harness};
use geom_fuzz::seeded;
use geom_make::{BoxInput, PrismInput};
use geom_math::{Axis, Rotation};
use geom_num::{P3, Steps};
use geom_run::LogicalSize;
use geom_test::Seed;
use geom_topo::placed::Placed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 100;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 6_000;

/// The longest byte string either leg draws: the harness reads at most [`MAX_INPUT_BYTES`], and
/// the rest must change nothing.
const MAX_LEN: u16 = 400;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn placed_fuzz() {
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
fn placed_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "placed fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn any_bytes_are_a_case() {
    holds(harness(&[]));
    holds(harness(&[0xff; 64]));
    holds(harness(&[0x11; 400]));
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let data = vec![0x11u8; usize::from(MAX_LEN)];
    assert_eq!(decode(&data).1, MAX_INPUT_BYTES);
}

#[test]
fn the_record_is_at_most_88_bytes() {
    assert!(size_of::<Placed>() <= 88, "{}", size_of::<Placed>());
    assert_eq!(<Placed as LogicalSize>::LOGICAL_BYTES, 88);
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// A probe with each coordinate's rule and no word.
fn probe(shell: u32, rules: [u8; 3]) -> Probe {
    Probe {
        shell,
        picks: rules.map(|r| (r, 0)),
    }
}

/// Probes on every face, edge and corner of a box, one step either side of each and at its
/// middle, and on the shell past the body's.
fn probes() -> Vec<Probe> {
    vec![
        probe(0, [1, 3, 3]),
        probe(0, [3, 5, 3]),
        probe(0, [3, 3, 1]),
        probe(0, [1, 1, 3]),
        probe(0, [5, 5, 5]),
        probe(0, [0, 3, 3]),
        probe(0, [2, 3, 6]),
        probe(1, [3, 3, 3]),
    ]
}

fn a_box() -> Shape {
    Shape::Box(BoxInput {
        min: p(2, -3, 5),
        size: [Steps(10), Steps(20), Steps(30)],
    })
}

/// A pentagonal prism, whose faces are slanted to two axes.
fn a_pentagon() -> Shape {
    Shape::Prism(PrismInput {
        base: vec![p(0, 0, 1), p(3, 0, 1), p(4, 2, 1), p(2, 4, 1), p(0, 3, 1)],
        height: Steps(3),
    })
}

/// A slanted prism.
fn a_slant() -> Shape {
    Shape::Prism(PrismInput {
        base: vec![p(0, 0, 1), p(30, 0, 1), p(40, 20, 1), p(10, 20, 1)],
        height: Steps(15),
    })
}

fn case(shape: Shape, axes: u8) -> Case {
    Case {
        shape,
        axes,
        shift: [7, -11, 13],
        packed: axes,
        probes: probes(),
        general: General::Rotate(Axis::Z, Rotation::direction(4, 3).unwrap()),
        accept: Some(64),
        index: 0,
    }
}

#[test]
fn every_permutation_keeps_every_law() {
    for shape in [a_box(), a_pentagon(), a_slant()] {
        for axes in 0..48 {
            holds(check(&case(shape.clone(), axes)));
        }
    }
}

#[test]
fn a_general_placement_keeps_every_law_whatever_it_accepts() {
    for general in [
        General::Rotate(Axis::X, Rotation::quarter_turns(1)),
        General::Rotate(Axis::Y, Rotation::direction(5, 12).unwrap()),
        General::Rotate(Axis::Z, Rotation::direction(1, 2).unwrap()),
        General::Exact,
    ] {
        for accept in [None, Some(1), Some(64), Some(1 << 30)] {
            holds(check(&Case {
                general,
                accept,
                ..case(a_pentagon(), 9)
            }));
        }
    }
}

#[test]
fn a_scale_an_index_past_the_table_and_a_packed_byte_past_47_are_refused() {
    holds(check(&Case {
        general: General::Scale(2, 1),
        ..case(a_box(), 0)
    }));
    for index in [1, u32::MAX] {
        holds(check(&Case {
            index,
            ..case(a_box(), 17)
        }));
    }
    for packed in [48, 200, 255] {
        holds(check(&Case {
            packed,
            ..case(a_box(), 31)
        }));
    }
}

#[test]
fn a_placement_past_the_grid_is_refused_like_its_materialisation() {
    let m = (1_i64 << 40) - 1;
    holds(check(&Case {
        shift: [m, 0, 0],
        ..case(a_box(), 0)
    }));
    holds(check(&Case {
        shift: [-m, m, -m],
        ..case(a_slant(), 47)
    }));
}
