//! The commits fuzz target (G142), run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, and a roof's hinge, a free plane and a hip roof's class that keep every law.
//!
//! - `commits_fuzz` wraps `geom_fuzz::commits::harness` in `bolero::check!`, bolero's test mode:
//!   the target's stored corpus and crash files, then [`ITERATIONS`] random byte strings; a
//!   failing one's seed is printed as `BOLERO_RANDOM_SEED=<seed>`.
//! - `commits_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
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

use geom_fuzz::commits::{Case, MAX_INPUT_BYTES, check, decode, harness};
use geom_fuzz::seeded;
use geom_math::{Axis, GridTransform};
use geom_num::P3;
use geom_num::interval::Interval;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 500;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 120_000;

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
fn commits_fuzz() {
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
fn commits_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "commits fuzz target, seeded",
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

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// A metre at q0, in steps.
const METRE: i64 = 6_400_000;

/// A 30 degree roof on a 10 m hinge, a plane moved along a sloping normal, and a quarter turn with
/// a shift: every commit is made and every law runs on it.
fn roof() -> Case {
    let (a, b) = (p(0, 0, 0), p(10 * METRE, 0, 0));
    Case {
        a,
        b,
        tilt: [0.0, 0.866_025_403_784_438_6, 0.5],
        direction: [0.3, -0.4, 0.5],
        point: p(METRE, 2 * METRE, 3 * METRE),
        distance: 12_345,
        extent: vec![
            a,
            b,
            p(0, 5 * METRE, 3 * METRE),
            p(10 * METRE, 5 * METRE, 3 * METRE),
        ],
        symmetry: Some(
            GridTransform::from_parts(
                [Axis::Y, Axis::X, Axis::Z],
                [true, false, false],
                [7, -9, 11],
            )
            .unwrap(),
        ),
        probe: p(METRE, METRE, METRE),
        interval: Some(Interval::new(0.0, 0.0).unwrap()),
    }
}

#[test]
fn a_roof_keeps_every_law() {
    holds(check(&roof()));
}

/// Hostile doubles, an edge of no length and no extent: each is refused, by its type.
#[test]
fn bad_inputs_are_refused_by_type() {
    for tilt in [
        [f64::NAN, 1.0, 0.0],
        [0.0, f64::INFINITY, 0.0],
        [0.0, 0.0, 0.0],
        [5e-324, 0.0, 0.0],
        [1e300, 0.0, 1.0],
        [2.0, 0.0, 0.0],
    ] {
        holds(check(&Case {
            tilt,
            direction: tilt,
            ..roof()
        }));
    }
    let case = roof();
    holds(check(&Case {
        b: case.a,
        ..roof()
    }));
    holds(check(&Case {
        extent: Vec::new(),
        distance: 0,
        ..roof()
    }));
    holds(check(&Case {
        distance: i64::MAX,
        ..roof()
    }));
}
