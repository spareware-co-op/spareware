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

use geom_fuzz::natural::{
    Case, Drawn, MAX_INPUT_BYTES, answer_of, check, decode, harness, work_of,
};
use geom_fuzz::seeded;
use geom_intersect::Item;
use geom_test::Seed;
use geom_test::golden::{hex, sha256};

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
/// - a cone against a plane that holds its axis, an edge of the plane passing through the apex,
///   where the line meets the cone at one point that is no simple root (483,244 of 484,099).
///
/// Not stored: an end circle on the axis off the grid that lies in a plane. `quadrics::edges`
/// rounds its centre off the plane, the engine runs round it, and the call stops on the budget. A
/// stop's count is the budget's own, which no test can hold tighter.
const STILL_THE_ENGINES: [(&[u8], u128); 2] = [
    (
        &[
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x4b, 0x00, 0x00, 0x00,
        ],
        19_133,
    ),
    (
        &[
            0x03, 0x00, 0x00, 0x4b, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
        ],
        484_099,
    ),
];

/// The issue's fifth input, stored in the target's `crashes` folder, with the work one call of
/// `natural` takes on it now: a sphere of radius 256 against a cylinder of radius 1 about a line
/// through its centre, whose end circle stands 0.002 outside the sphere all the way round. The
/// engine took 782,639 units to say so (782,683 in all, three such calls for one run of the
/// target, which stopped a search on its slow-input line). A circle about a line through a
/// sphere's centre is now on the sphere or clear of it by one exact sign (decision 32.74), and
/// the answer is the same in every field but the work it records.
const AN_END_CIRCLE_OFF_A_SPHERE: [(&[u8], u128); 1] =
    [(&[0xfd, 0xde, 0x00, 0xf5, 0xff, 0xc2, 0x25, 0x02, 0xde], 23)];

/// Issue 974's inputs whose end circle lies in a plane whose domain holds a part of it, stored
/// in the target's `crashes` folder, each with the work one call of `natural` takes on it now
/// that the arcs the domain holds have a closed form (decision 32.69): a cylinder's end circle
/// of radius 5, half held (136,758 units before); a cone's of radius 600, the cheapest of the
/// eleven a search of this target found in 55 minutes (1,252,098 before); and a cone's of
/// radius 856, the dearest of them (3,046,840 before).
const ARCS_A_PLANE_HOLDS: [(&[u8], u128); 3] = [
    (
        &[
            0x02, 0x00, 0x00, 0x4b, 0x04, 0x00, 0x00, 0x00, 0x4b, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        70,
    ),
    (&[0xf7, 0xff, 0x83, 0xb5, 0x06, 0x60, 0xce, 0x60], 62),
    (
        &[
            0x63, 0x33, 0xb7, 0xb4, 0xff, 0xb4, 0xff, 0xfc, 0xfc, 0x6b, 0xff,
        ],
        62,
    ),
];

/// Issue 995's group 3, stored in the target's `crashes` folder: two cones of slope 2 on parallel,
/// distinct axes along (0.6, 0.8, 0), each end circle of one (radius 344 to 856) taken against
/// the other, with the work one call of `natural` takes on it now and the curves of its answer.
/// The engine ran round each circle, 258,258 to 1,759,429 units a circle for nothing or two
/// points: 4,151,095, 4,193,766, 3,736,405, 3,748,566 and 3,106,700 units a call. A circle across
/// a parallel axis is now its plane past the domain, or two circles in one plane told apart or
/// crossing by enclosures (decision 32.86): 1,377 units a call, and 3,984 where two circles each
/// cross at two points (648 units a circle, its box in space for each point). Where nothing is
/// found the answer is the same in every field but the work; where points are, each crossing's box
/// lies inside the engine's, and the curves' ends, cut at those boxes' middles, move by under 4e-10.
///
/// The last is issue 974's input of two cones about one line on the axis off the grid, once on
/// the engine's list: an end circle in the plane of the other's apex, clear of that cone by 150,
/// took the engine 353,774 and 441,124 units (800,571 in all). The circle `quadrics::edges` builds
/// stands a rounding off that plane, where the cut is a circle about the axis that an enclosure
/// holds clear of zero, and two circles about one centre of radii apart meet nothing: 1,697.
const ENDS_ACROSS_A_PARALLEL_CONE: [(&[u8], u128, usize); 6] = [
    (
        &[
            0x33, 0x0b, 0x00, 0xfe, 0xff, 0xff, 0x0e, 0xff, 0xff, 0xff, 0xff, 0x00, 0x6b, 0xff,
        ],
        1_377,
        1,
    ),
    (
        &[
            0x7f, 0x03, 0x00, 0x00, 0x80, 0xff, 0xff, 0xff, 0xff, 0x00, 0xff, 0xba, 0x03, 0xff,
        ],
        1_377,
        1,
    ),
    (
        &[
            0x2b, 0x03, 0xe6, 0xf6, 0xd8, 0x03, 0xf6, 0xe7, 0xe7, 0xe7, 0xe7, 0xe7, 0x03, 0xf6,
            0xe7, 0xe7, 0xe7,
        ],
        1_377,
        1,
    ),
    (
        &[
            0x93, 0x03, 0x00, 0x03, 0x02, 0xff, 0x2f, 0x4b, 0x41, 0xff, 0xfe, 0xff, 0xff, 0xf7,
        ],
        3_984,
        2,
    ),
    (
        &[
            0x63, 0x33, 0xb7, 0x33, 0xb7, 0xff, 0xff, 0xff, 0x7f, 0xb7, 0xd4, 0xb4, 0x6b, 0xff,
        ],
        3_984,
        2,
    ),
    (
        &[
            0x03, 0x2d, 0x3c, 0x00, 0x09, 0x03, 0x00, 0x03, 0x00, 0x00, 0x00, 0x09, 0x03, 0x00,
        ],
        1_697,
        1,
    ),
];

#[test]
fn an_end_circle_across_a_parallel_cone_costs_a_closed_form() {
    for (data, most, curves) in ENDS_ACROSS_A_PARALLEL_CONE {
        let (case, _) = decode(data);
        let work = work_of(&case).unwrap_or_else(|finding| panic!("{finding}"));
        assert!(work <= most, "{work} units, over {most}, on {data:02x?}");
        let answer = answer_of(&case)
            .unwrap_or_else(|finding| panic!("{finding}"))
            .unwrap_or_else(|| panic!("no answer on {data:02x?}"));
        let items = answer.items();
        assert!(
            items.len() == curves && items.iter().all(|i| matches!(i, Item::Curve { .. })),
            "{items:?} on {data:02x?}"
        );
        holds(harness(data));
    }
}

/// Issue 995's group R, stored in the target's `crashes` folder: two cones of slope 2 on parallel
/// axes along (0.6, 0.8, 0), of start radius 224 and 0, whose centres differ by 8 square to the
/// axis, so each end circle lies in the plane of an end of the other's domain. The other cone cuts
/// that plane in a circle of radius 600, 8 from the end circle of radius 376 or 824: one inside the
/// other, apart by 216. 32.86 left a plane whose enclosure reaches an end to the engine, which ran
/// round each of the four circles for nothing: 460,442 to 942,607 units a circle, 2,756,742 a call.
/// Two circles apart for every height the plane's enclosure holds now meet nothing at an end too:
/// 1,377 units a call.
///
/// With it, the SHA-256 of the Debug text of the answer's items, coverage and diagnostics, every
/// field but its work, taken on the engine's answer before that change: the answer is the same.
const ENDS_IN_THE_PLANE_OF_AN_END: [(&[u8], u128, &str); 1] = [(
    &[
        0x73, 0xff, 0xff, 0xff, 0xdf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfd, 0xdf, 0x3f, 0xce,
    ],
    1_377,
    "677f58ccfc1d3e64b339aba50ee9418553d9c0c7a233bd8d78ae9dab7e5b0ea6",
)];

#[test]
fn an_end_circle_in_the_plane_of_the_others_end_costs_a_closed_form() {
    for (data, most, digest) in ENDS_IN_THE_PLANE_OF_AN_END {
        let (case, _) = decode(data);
        let work = work_of(&case).unwrap_or_else(|finding| panic!("{finding}"));
        let answer = answer_of(&case)
            .unwrap_or_else(|finding| panic!("{finding}"))
            .unwrap_or_else(|| panic!("no answer on {data:02x?}"));
        let text = format!(
            "{:?} {:?} {:?}",
            answer.items(),
            answer.coverage(),
            answer.diagnostics()
        );
        let said = hex(sha256(text.as_bytes()));
        assert!(
            work <= most && said == digest,
            "{work} units (at most {most}), answer {said}, on {data:02x?}"
        );
        holds(harness(data));
    }
}

#[test]
fn what_the_closed_form_leaves_costs_no_more_than_today() {
    let held = STILL_THE_ENGINES
        .into_iter()
        .chain(ARCS_A_PLANE_HOLDS)
        .chain(AN_END_CIRCLE_OFF_A_SPHERE);
    for (data, most) in held {
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
