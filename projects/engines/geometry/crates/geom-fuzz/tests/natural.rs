//! The natural-quadrics fuzz target (G165), run as tests on stable Rust: bolero's test mode, the
//! seeded leg, the input bound, and what it reaches.
//!
//! - `natural_fuzz` wraps `geom_fuzz::natural::harness` in `bolero::check!`: the crate's corpus and
//!   crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`.
//! - `natural_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly, and the cases reach every pair of kinds.

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

use geom_fuzz::natural::{Case, Drawn, MAX_INPUT_BYTES, check, decode, harness, work_of};
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 300;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 1_000;

/// The longest byte string the seeded leg draws: the harness reads at most [`MAX_INPUT_BYTES`];
/// the rest must change nothing.
const MAX_LEN: u16 = 40;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn natural_fuzz() {
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
fn natural_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "natural quadrics fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = vec![3; MAX_INPUT_BYTES];
    let (_, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    data.extend_from_slice(&[7; 10]);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    holds(check(&case));
    // The empty input is a case too.
    let (empty, read) = decode(&[]);
    assert_eq!(read, 0);
    holds(check(&empty));
}

/// Issue 974's four inputs, stored in the target's `crashes` folder, each with the most work one
/// call of `natural` may take on it. In all four the circle where the surfaces meet is an end
/// circle of one of them, so that edge lies wholly on the other surface. Before the closed form for
/// such a circle the engine bisected round all of it: over 4,194,304 units (the budget's stop),
/// 1,376,154, 1,287,083 and 1,164,683. Now 46, 2,524, 38 and 38: the second still runs its other
/// edges, which cross the other surface, through the engine.
const EDGES_ON_THE_OTHER: [(&[u8], u128); 4] = [
    (&[0x7e, 0x00, 0x00, 0x00, 0x95, 0x08, 0x08, 0xf7], 64),
    (&[0x7e, 0x00, 0x00, 0x40, 0x95, 0x08, 0x08, 0xf7], 4_096),
    (&[0x93, 0x03, 0x02, 0x4b, 0x00, 0x00, 0x03], 64),
    (&[0x3e, 0x00, 0x00, 0xb5, 0xce], 64),
];

#[test]
fn an_edge_lying_on_the_other_surface_costs_a_closed_form() {
    for (data, most) in EDGES_ON_THE_OTHER {
        let (case, _) = decode(data);
        let work = work_of(&case).unwrap_or_else(|finding| panic!("{finding}"));
        assert!(work <= most, "{work} units, over {most}, on {data:02x?}");
        holds(harness(data));
    }
}

/// Issue 974's inputs that the closed form does not take, stored in the target's `crashes` folder,
/// each with the work one call of `natural` takes on it today: a count held from above, so that it
/// can only get cheaper. Each still pays the engine for an edge, whose own work is given first:
///
/// - a sphere against a cylinder about a line through its centre, whose end circle is a great
///   circle of the sphere: no closed form places a circle on a sphere (19,090 of 19,133);
/// - a cylinder whose end circle lies in a plane whose domain holds half of it (136,698 of
///   136,758);
/// - the same with a cone, as a search of this target found it eleven times in 55 minutes: the
///   cone's end circle of radius 600 lies in a plane whose domain holds an arc of it (1,252,043
///   of 1,252,098, the cheapest of the eleven; the dearest, at radius 856, takes 3,046,840);
/// - a cone against a plane that holds its axis, an edge of the plane passing through the apex,
///   where the line meets the cone at one point that is no simple root (483,244 of 484,099);
/// - two cones about one line on the axis off the grid, no edge of either on the other: an end
///   circle in the plane of the other's apex is clear of that cone by 150, and the engine takes
///   353,774 and 441,124 units to say so (800,571 in all);
/// - the issue's fifth input, a sphere of radius 256 against a cylinder of radius 1 about a line
///   through its centre, whose end circle stands 0.002 outside the sphere all round (782,639 of
///   782,683).
///
/// Not stored: an end circle on the axis off the grid that lies in a plane. `quadrics::edges`
/// rounds its centre off the plane, the engine runs round it, and the call stops on the budget. A
/// stop's count is the budget's own, which no test can hold tighter.
const STILL_THE_ENGINES: [(&[u8], u128); 6] = [
    (
        &[
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x4b, 0x00, 0x00, 0x00,
        ],
        19_133,
    ),
    (
        &[
            0x02, 0x00, 0x00, 0x4b, 0x04, 0x00, 0x00, 0x00, 0x4b, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        136_758,
    ),
    (&[0xf7, 0xff, 0x83, 0xb5, 0x06, 0x60, 0xce, 0x60], 1_252_098),
    (
        &[
            0x03, 0x00, 0x00, 0x4b, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
        ],
        484_099,
    ),
    (
        &[
            0x03, 0x2d, 0x3c, 0x00, 0x09, 0x03, 0x00, 0x03, 0x00, 0x00, 0x00, 0x09, 0x03, 0x00,
        ],
        800_571,
    ),
    (
        &[0xfd, 0xde, 0x00, 0xf5, 0xff, 0xc2, 0x25, 0x02, 0xde],
        782_683,
    ),
];

#[test]
fn what_the_closed_form_leaves_costs_no_more_than_today() {
    for (data, most) in STILL_THE_ENGINES {
        let (case, _) = decode(data);
        let work = work_of(&case).unwrap_or_else(|finding| panic!("{finding}"));
        assert!(work <= most, "{work} units, over {most}, on {data:02x?}");
    }
}

#[test]
fn every_pair_of_kinds_is_met() {
    let shapes = [
        [10, 20, 30, 40, 0, 1],
        [12, 20, 30, 40, 1, 2],
        [10, 20, 30, 41, 2, 4],
        [10, 18, 30, 7, 3, 6],
    ];
    for first in 0..4 {
        for second in 0..4 {
            for at in shapes {
                holds(check(&Case {
                    first: Drawn {
                        kind: first,
                        at: shapes[0],
                    },
                    second: Drawn { kind: second, at },
                    work: 3,
                }));
            }
        }
    }
}
