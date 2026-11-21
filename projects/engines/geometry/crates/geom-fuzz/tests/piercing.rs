//! The piercing fuzz target (G164), run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, and what it reaches.
//!
//! - `piercing_fuzz` wraps `geom_fuzz::piercing::harness` in `bolero::check!`: the crate's corpus and
//!   crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`.
//! - `piercing_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly, and the cases reach every kind of surface.
//! - Issue 995's segments along planes off the grid are answered by the closed form, each held to
//!   its answer and its count, and so are its segments touching a cone's start circle (group Q)
//!   and issue 1112's segments along a cone's ruling.

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

use geom_fuzz::piercing::{Case, Drawn, MAX_INPUT_BYTES, answer_of, check, decode, harness};
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
fn piercing_fuzz() {
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
fn piercing_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "piercing fuzz target, seeded",
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

/// An input, the (u, v) enclosures of the overlap it answers (`None` for no item), and its count.
type Pinned = (&'static [u8], Option<[(f64, f64); 2]>, u128);

/// The inputs of issue 995's group P stored in the target's `crashes` folder, each with the answer
/// one call of `curve_surface` gives and the work it takes: a segment along y lying in, or parallel
/// to, a plane whose first vector holds a half. The first three are the timeouts of the 14 November
/// campaign, which ran the engine to the budget (1,048,577 units each); the next three are slow
/// units of a search on the Phase 5 train, parallel and 0.48, 8.3 and 3.97 off the plane (913,098,
/// 720,686 and 919,038 units to say nothing); the seventh lies in its plane (1,017,940 units for one
/// flagged item); the eighth, (0, 0, 0) to (0, 140, 0) in the plane on (64, 0, -10.5) and
/// (0, 1, 0), is the timeout of Phase 6's run on the Phase 5 train (pull request 866), on a commit
/// before 32.85, when this target's budget was 2^24. Lying in a plane and running along it are
/// exact signs now (decision 32.85): a segment in its plane is one overlap by identity over all of
/// it, t in [0, 1], at the enclosures of (u, v) given, and one along it off the plane is nothing.
const LINES_ALONG_PLANES: [Pinned; 8] = [
    (
        &[
            0x00, 0x00, 0x20, 0x00, 0x00, 0xe7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0xa9, 0x00,
            0x09,
        ],
        Some([
            (-5e-324, 5e-324),
            (-100.000_000_000_000_24, 128.000_000_000_000_34),
        ]),
        1,
    ),
    (
        &[
            0xde, 0x00, 0x03, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0f, 0x61, 0xa8,
            0x2e, 0x60,
        ],
        Some([
            (-5e-324, 5e-324),
            (0.071_005_917_159_763_12, 0.094_674_556_213_018_03),
        ]),
        1,
    ),
    (
        &[
            0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x09, 0x31,
        ],
        Some([
            (-1e-323, 1e-323),
            (-7.105_427_357_601_023e-17, 0.320_000_000_000_000_95),
        ]),
        1,
    ),
    (
        &[
            0x00, 0x00, 0x25, 0x00, 0x00, 0xe7, 0x00, 0xfc, 0xff, 0xff, 0xff, 0x07, 0x13, 0x00,
            0xa2, 0x21, 0x09,
        ],
        None,
        0,
    ),
    (
        &[
            0xde, 0x00, 0x03, 0x00, 0x00, 0x04, 0x00, 0x00, 0x04, 0xff, 0x00, 0x4f, 0x61, 0xa8,
            0x2e, 0x00,
        ],
        None,
        0,
    ),
    (
        &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x01, 0xd5, 0x31,
        ],
        None,
        0,
    ),
    (
        &[
            0x00, 0x00, 0x29, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0x01,
            0x09,
        ],
        Some([
            (-1.67e-321, 1.67e-321),
            (63.999_999_999_999_81, 82.000_000_000_000_24),
        ]),
        1,
    ),
    (
        &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x23, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3f, 0xeb, 0x00,
        ],
        Some([(-5e-324, 5e-324), (-4.4e-323, 140.000_000_000_000_5)]),
        1,
    ),
];

/// The answer's items as `Debug` prints them: the overlap over all of the segment at `uv`, or none.
fn along(uv: Option<[(f64, f64); 2]>) -> String {
    let Some([(u0, u1), (v0, v1)]) = uv else {
        return "[]".to_owned();
    };
    format!(
        "[Overlap {{ at: ParamBox {{ spans: [Interval {{ lo: 0.0, hi: 1.0 }}, Interval {{ lo: \
         {u0:?}, hi: {u1:?} }}, Interval {{ lo: {v0:?}, hi: {v1:?} }}, Interval {{ lo: 0.0, hi: \
         0.0 }}], unknowns: 3 }}, region: OverlapRegion {{ boundary: [] }}, proof: Identical, \
         class: Certified {{ bound: Steps(1), step: Q0 }} }}]"
    )
}

#[test]
fn a_segment_along_a_plane_off_the_grid_is_a_closed_form() {
    for (data, uv, work) in LINES_ALONG_PLANES {
        let (case, _) = decode(data);
        let (found, used) = answer_of(&case).unwrap_or_else(|finding| panic!("{finding}"));
        let found = found.unwrap_or_else(|| panic!("stopped on the budget: {data:02x?}"));
        assert_eq!(format!("{:?}", found.items()), along(uv), "{data:02x?}");
        assert_eq!(used, work, "{data:02x?}");
        holds(check(&case));
    }
}

/// Issue 1112's inputs, stored in the target's `crashes` folder, each with the answer one call of
/// `curve_surface` gives and the work it takes: a segment along a ruling of a cone about the z
/// axis, (0, 4, −4) to (0, 8, 4) on the cone through (0, −4, 12) of start radius 16 and slope 0.5;
/// three on cones of slope zero, whose rulings run along the axis; and (0, 0, 4) to the apex at
/// (0, 4, −4) of the cone through (0, 4, 0) of start radius 2 and slope 0.5. The line's quadratic
/// leads with A = 0, whose enclosure `quadratic_with` refused, and the engine ran along it: 927,183,
/// 826,784, 946,148 and 832,796 units for one flagged item, and 1,048,577 for the budget's stop.
/// Whether a line is parallel to a ruling is an exact sign now, and one with B and C zero lies
/// along it (decision 32.107): one overlap by identity over all of it, t in [0, 1], at the
/// enclosures of (u, v) given. On the cones of slope zero the ruling is on the cone's join, u = 0,
/// held across it from just short of 2 (decision 32.104).
const SEGMENTS_ALONG_A_RULING: [Pinned; 5] = [
    (
        &[
            0xf6, 0x00, 0x01, 0xff, 0x00, 0x02, 0x01, 0xff, 0x00, 0xff, 0x03, 0x0f, 0x00, 0x18,
            0x00, 0x26,
        ],
        Some([
            (0.499_999_999_999_999_7, 0.500_000_000_000_000_3),
            (-16.000_000_000_000_018, -7.999_999_999_999_988),
        ]),
        1,
    ),
    (
        &[
            0x00, 0xff, 0xff, 0x00, 0xff, 0xff, 0xff, 0xff, 0xfe, 0xff, 0xff, 0x03, 0xf8, 0x00,
            0x00, 0x00,
        ],
        Some([
            (1.999_999_999_999_999_8, 2.000_000_000_000_000_4),
            (-3.996_802_888_650_565e-15, 4.000_000_000_000_004),
        ]),
        1,
    ),
    (
        &[
            0x00, 0xff, 0xff, 0x20, 0xff, 0xff, 0xff, 0xff, 0xfe, 0xff, 0xff, 0x03, 0xf8, 0x00,
            0x00, 0x00,
        ],
        Some([
            (1.999_999_999_999_999_8, 2.000_000_000_000_000_4),
            (-1.705_302_565_824_241e-13, 132.000_000_000_000_14),
        ]),
        1,
    ),
    (
        &[
            0x00, 0xff, 0xff, 0x00, 0xff, 0xff, 0xfe, 0x03, 0xfe, 0xff, 0xfd, 0x03, 0xf8, 0x00,
            0x43,
        ],
        Some([
            (1.999_999_999_999_999_8, 2.000_000_000_000_000_4),
            (3.999_999_999_999_988_5, 12.000_000_000_000_009),
        ]),
        1,
    ),
    (
        &[
            0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0xff, 0x0f, 0x00, 0x01, 0x00, 0x01,
        ],
        Some([
            (1.499_999_999_999_999_6, 1.500_000_000_000_000_4),
            (-4.000_000_000_000_009, 4.000_000_000_000_004),
        ]),
        1,
    ),
];

#[test]
fn a_segment_along_a_cone_s_ruling_is_a_closed_form() {
    for (data, uv, work) in SEGMENTS_ALONG_A_RULING {
        let (case, _) = decode(data);
        let (found, used) = answer_of(&case).unwrap_or_else(|finding| panic!("{finding}"));
        let found = found.unwrap_or_else(|| panic!("stopped on the budget: {data:02x?}"));
        assert_eq!(format!("{:?}", found.items()), along(uv), "{data:02x?}");
        assert_eq!(used, work, "{data:02x?}");
        holds(check(&case));
    }
}

/// Issue 995's group Q, stored in the target's `crashes` folder (the first five; a sixth input
/// of the search reads the same 16 bytes as the first): the segment (−4, −4, −4) to (−3, −4, −4)
/// against the cone about the z axis through (−4, −260, −4) of start radius 256 and slope 7/16
/// (the first four) or 9/16 (the fifth). The segment's start is exactly 256 from the axis, in the
/// plane of the start circle, so it touches the cone there: one tangent point at t = 0. The apex
/// is 4096/7 or 4096/9 below the centre, off the grid, and the engine bisected round the touch
/// for 387,549 and 597,383 units to one flagged item over t from 0 to about 0.6; the exact sign of
/// a cone of any start radius (decision 32.88) makes it a closed form. The last, of slope 1/8, has
/// its apex on the grid and was a closed form already (decision 32.87): it is held unchanged.
/// Each with the upper bound of its point's enclosure of t, and its count.
const A_SEGMENT_TOUCHING_A_START_CIRCLE: [(&[u8], f64, u128); 6] = [
    (
        &[
            0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xbf, 0xff, 0xff, 0xff, 0xc1,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x03, 0xff, 0x03, 0x00,
        ],
        1.018e-320,
        4,
    ),
    (
        &[
            0x0a, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xbf, 0xff, 0xff, 0xff, 0xff,
            0x15, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25,
        ],
        1.018e-320,
        4,
    ),
    (
        &[
            0x0a, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xbf, 0xff, 0xff, 0xff, 0xff,
            0xe7, 0x00, 0x00, 0x00, 0x25,
        ],
        1.018e-320,
        4,
    ),
    (
        &[
            0x8a, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xbf, 0xff, 0xff, 0xff, 0xff,
            0xe7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25,
        ],
        1.018e-320,
        4,
    ),
    (
        &[
            0xdc, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xbf, 0xff, 0xff, 0x01, 0x00,
            0x00, 0xfe, 0xff, 0xfd,
        ],
        1.1443e-320,
        4,
    ),
    (
        &[
            0x0a, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xbf, 0xff, 0xff, 0xfa, 0xff,
            0x15, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25,
        ],
        8.913e-321,
        4,
    ),
];

/// The answer's items as `Debug` prints them: the one tangent point at the segment's start, at
/// the start circle's u = 0.5, with t from 0 to `t`.
fn touching(t: f64) -> String {
    format!(
        "[Point {{ at: ParamBox {{ spans: [Interval {{ lo: 0.0, hi: {t:?} }}, Interval {{ lo: \
         0.4999999999999997, hi: 0.5000000000000003 }}, Interval {{ lo: -4.4e-323, hi: 4.4e-323 \
         }}, Interval {{ lo: 0.0, hi: 0.0 }}], unknowns: 3 }}, xyz: Box3 {{ min: P3 {{ x: \
         Coord(-5), y: Coord(-4), z: Coord(-4) }}, max: P3 {{ x: Coord(-3), y: Coord(-4), z: \
         Coord(-4) }} }}, contact: IsolatedTangent, class: Certified {{ bound: Steps(1), step: Q0 \
         }} }}]"
    )
}

#[test]
fn a_segment_touching_a_start_circle_is_one_point_in_closed_form() {
    for (data, t, work) in A_SEGMENT_TOUCHING_A_START_CIRCLE {
        let (case, _) = decode(data);
        let (found, used) = answer_of(&case).unwrap_or_else(|finding| panic!("{finding}"));
        let found = found.unwrap_or_else(|| panic!("stopped on the budget: {data:02x?}"));
        assert_eq!(format!("{:?}", found.items()), touching(t), "{data:02x?}");
        assert_eq!(used, work, "{data:02x?}");
        holds(check(&case));
    }
}

#[test]
fn every_kind_of_surface_is_met() {
    let at = [10, 20, 30, 40, 50, 60];
    let other = [220, 5, 40, 3, 90, 200];
    for kind in 0..2 {
        for surface in 0..6 {
            holds(check(&Case {
                curve: Drawn { kind, at },
                surface: Drawn {
                    kind: surface,
                    at: other,
                },
                work: 3,
            }));
        }
    }
}
