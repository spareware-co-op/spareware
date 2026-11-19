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
//!   its answer and its count.

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
