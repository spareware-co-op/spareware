//! The bounds fuzz target (G143), run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, and a part cut, filleted and joined that keeps every law.
//!
//! - `bounds_fuzz` wraps `geom_fuzz::bounds::harness` in `bolero::check!`, bolero's test mode:
//!   the target's stored corpus and crash files, then [`ITERATIONS`] random byte strings; a
//!   failing one's seed is printed as `BOLERO_RANDOM_SEED=<seed>`.
//! - `bounds_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
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

use geom_fuzz::bounds::{Case, MAX_INPUT_BYTES, Op, check, decode, harness, name};
use geom_fuzz::seeded;
use geom_num::ladder::PER_OPERATION;
use geom_num::{GridStep, Steps};
use geom_op::bounds::Limits;
use geom_op::{NameId, NameSet, OpId, OpPath, Origin, PieceKey, Record, Role, Side};
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 500;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 500_000;

/// The longest byte string either leg draws: the harness reads at most [`MAX_INPUT_BYTES`], and
/// the rest must change nothing.
const MAX_LEN: u16 = 600;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn bounds_fuzz() {
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
fn bounds_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "bounds fuzz target, seeded",
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
    holds(harness(&[0x11; 600]));
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let data = vec![0x11u8; usize::from(MAX_LEN)];
    assert_eq!(decode(&data).1, MAX_INPUT_BYTES);
}

fn path(x: u8) -> OpPath {
    OpPath::new(OpId::from_bytes([x; 16]))
}

fn names(xs: &[usize]) -> NameSet {
    NameSet::new(xs.iter().map(|x| name(*x))).unwrap()
}

fn record(output: usize, origin: Origin) -> Record {
    Record {
        output: name(output),
        origin,
    }
}

fn generated(output: usize, from: &[usize]) -> Record {
    record(
        output,
        Origin::Generated {
            from: names(from),
            role: Role::new("fillet", []).unwrap(),
        },
    )
}

fn op(x: u8, records: Vec<Record>, own: &[(usize, u64)], limits: Limits) -> Op {
    Op {
        path: path(x),
        records,
        own: own.iter().map(|(e, b)| (name(*e), Steps(*b))).collect(),
        limits,
    }
}

/// A loft of three faces, two with a term of their own.
fn loft() -> Op {
    op(
        1,
        vec![generated(0, &[]), generated(1, &[]), generated(2, &[])],
        &[(0, 3), (1, 5), (2, 0)],
        Limits::DEFAULT,
    )
}

/// The loft, a cut that splits one face and leaves the rest, a fillet that merges two and makes
/// an edge, and the part joined with its first state: every law runs on each.
fn part() -> Case {
    let cut = op(
        2,
        vec![
            record(0, Origin::Unchanged),
            record(
                3,
                Origin::Split {
                    of: name(1),
                    piece: PieceKey::ByTools {
                        tools: names(&[2]),
                        side: Side::Front,
                    },
                },
            ),
            record(2, Origin::Modified { from: names(&[2]) }),
        ],
        &[(3, 2), (2, PER_OPERATION.0)],
        Limits::DEFAULT,
    );
    let fillet = op(
        3,
        vec![
            record(0, Origin::Merged { of: names(&[0, 3]) }),
            generated(4, &[2, 3]),
            record(2, Origin::Unchanged),
        ],
        &[(4, 1), (0, 7)],
        Limits::DEFAULT,
    );
    Case {
        step: GridStep::Q1,
        ops: vec![loft(), cut, fillet],
        partner: 1,
        other: GridStep::Q0,
    }
}

#[test]
fn a_part_keeps_every_law() {
    holds(check(&part()));
    holds(check(&Case {
        other: GridStep::Q1,
        partner: 0,
        ..part()
    }));
}

/// Each fault, alone and beside another, is refused as the laws ask.
#[test]
fn every_fault_is_refused_as_the_laws_ask() {
    let tight = Limits {
        own: Steps(4),
        total: Steps(8),
    };
    let faults: [&[(usize, u64)]; 6] = [
        // Past the step's limit.
        &[(1, 5)],
        // Past the ceiling: 5 inherited and 4 own.
        &[(1, 4)],
        // On an unchanged output.
        &[(0, 1)],
        // On no output.
        &[(3, 1)],
        // Twice on one output.
        &[(2, 1), (2, 2)],
        // A term on no output beside one twice, and one past the limit: the last, on the first
        // entity in name order, is the one named (#1171).
        &[(3, 1), (4, 1), (4, 1), (1, 9)],
    ];
    for own in faults {
        holds(check(&Case {
            ops: vec![
                loft(),
                op(
                    4,
                    vec![
                        record(0, Origin::Unchanged),
                        record(1, Origin::Merged { of: names(&[1, 4]) }),
                        record(2, Origin::Modified { from: names(&[2]) }),
                        generated(4, &[]),
                    ],
                    own,
                    tight,
                ),
            ],
            ..part()
        }));
    }
    // A sum within four of the largest word, twice over, on a ceiling just below it and on the
    // largest word itself (#1170): refused, and never wrapped or saturated.
    for total in [u64::MAX - 1, u64::MAX] {
        let huge = Limits {
            own: Steps(u64::MAX),
            total: Steps(total),
        };
        holds(check(&Case {
            ops: vec![
                op(
                    1,
                    vec![generated(0, &[]), generated(1, &[])],
                    &[(0, u64::MAX - 2), (1, u64::MAX - 3)],
                    huge,
                ),
                op(
                    2,
                    vec![record(2, Origin::Merged { of: names(&[0, 1]) })],
                    &[],
                    huge,
                ),
            ],
            ..part()
        }));
    }
}

/// The names of a case are in name order, as the refusal law reads them.
#[test]
fn the_names_are_in_name_order() {
    let all: Vec<NameId> = (0..5).map(name).collect();
    assert!(all.windows(2).all(|w| w[0] < w[1]));
}
