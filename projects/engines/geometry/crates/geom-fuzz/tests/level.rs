//! The level fuzz target (G168), run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, what it reaches, and that a wrong answer is found.
//!
//! - `level_fuzz` wraps `geom_fuzz::level::harness` in `bolero::check!`: the crate's corpus and
//!   crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`. Under `cargo bolero test level_fuzz --package geom-fuzz` it is a
//!   libFuzzer target.
//! - `level_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly, the cases reach every surface, function and shape of
//!   answer, and each law fires on an answer changed to break it.

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

use geom_curve::Frame;
use geom_fuzz::level::{Case, MAX_INPUT_BYTES, Outcome, check, decode, harness, judge, surface};
use geom_fuzz::seeded;
use geom_intersect::{Branch, LevelCurves, OnImplicit, trace};
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{ImplicitBound, Sphere};
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg. A trace takes tens of milliseconds
/// unoptimised, so this is far below the other targets'.
const ITERATIONS: usize = 16;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 40;

/// The longest byte string the seeded leg draws: the harness reads at most [`MAX_INPUT_BYTES`];
/// the rest must change nothing.
const MAX_LEN: u16 = 24;

const _: () = assert!(MAX_LEN as usize >= MAX_INPUT_BYTES);

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn level_fuzz() {
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
fn level_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "level fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}

#[test]
fn the_input_bound_is_reached_and_never_passed() {
    let mut data = vec![1; MAX_INPUT_BYTES];
    let (_, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    data.extend_from_slice(&[7; 100]);
    let (case, read) = decode(&data);
    assert_eq!(read, MAX_INPUT_BYTES);
    holds(check(&case).map(|_| ()));
    // The empty input is a case too.
    let (empty, read) = decode(&[]);
    assert_eq!(read, 0);
    holds(check(&empty).map(|_| ()));
}

#[test]
fn the_cases_reach_every_surface_function_and_answer() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("level outcomes", Seed::DEFAULT));
    let (mut surfaces, mut levels) = ([0; 5], [0; 6]);
    let (mut open, mut closed, mut empty) = (0, 0, 0);
    for _ in 0..60 {
        let mut data = [0u8; MAX_INPUT_BYTES];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        match check(&case).unwrap() {
            Outcome::Open => open += 1,
            Outcome::Closed => closed += 1,
            Outcome::Empty => empty += 1,
            Outcome::Flagged | Outcome::Refused => {}
        }
        surfaces[usize::from(case.surface % 5)] += 1;
        levels[usize::from(case.level % 6)] += 1;
    }
    println!("{surfaces:?} {levels:?} {open} {closed} {empty}");
    assert!(surfaces.iter().all(|&n| n > 3));
    assert!(levels.iter().all(|&n| n > 3));
    assert!(open > 5, "{open}");
    assert!(empty > 0, "{empty}");
}

/// A sphere cutting the plane patch in a circle, and the true answer.
fn circle() -> (geom_surface::SurfaceKind, Sphere, LevelCurves) {
    let case = Case {
        surface: 0,
        size: 8,
        level: 0,
        at: [16; 3],
        trim: 7,
        work: 255,
    };
    let plane = surface(&case).unwrap();
    let frame = Frame::standard().unwrap();
    let sphere = Sphere::new([0.0; 3], 2.5, frame, ((0.0, 2.0), (-0.5, 0.5))).unwrap();
    let implicit: &dyn ImplicitBound = &sphere;
    let found = trace(
        &plane,
        &OnImplicit(implicit),
        1e-5,
        1e-3,
        &mut WorkMeter::new(Budget::of_work(1 << 22)),
    )
    .unwrap();
    (plane, sphere, found)
}

#[test]
fn a_true_answer_passes() {
    let (plane, sphere, found) = circle();
    assert_eq!(found.branches.len(), 1, "{found:?}");
    let implicit: &dyn ImplicitBound = &sphere;
    judge(&plane, &OnImplicit(implicit), &found).unwrap();
}

#[test]
fn a_missing_branch_is_a_finding() {
    let (plane, sphere, mut found) = circle();
    found.branches.clear();
    let implicit: &dyn ImplicitBound = &sphere;
    let law = judge(&plane, &OnImplicit(implicit), &found).unwrap_err();
    assert!(law.law.contains("sign change"), "{law}");
}

#[test]
fn a_vertex_out_of_the_domain_is_a_finding() {
    let (plane, sphere, mut found) = circle();
    let far = Interval::point(500.0).unwrap();
    let branch: &mut Branch = &mut found.branches[0];
    branch.vertices[0] = [far, far];
    let implicit: &dyn ImplicitBound = &sphere;
    let law = judge(&plane, &OnImplicit(implicit), &found).unwrap_err();
    assert!(law.law.contains("in the domain"), "{law}");
}

#[test]
fn a_branch_past_its_ceiling_is_a_finding() {
    let (plane, sphere, mut found) = circle();
    found.branches[0].deviation = 1.0;
    let implicit: &dyn ImplicitBound = &sphere;
    let law = judge(&plane, &OnImplicit(implicit), &found).unwrap_err();
    assert!(law.law.contains("ceiling"), "{law}");
}
