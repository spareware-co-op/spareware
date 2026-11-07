//! The project fuzz target (G162), run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, what it reaches, and that a wrong answer is found.
//!
//! - `project_fuzz` wraps `geom_fuzz::project::harness` in `bolero::check!`: the crate's corpus and
//!   crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`. Under `cargo bolero test project_fuzz --package geom-fuzz` it is
//!   a libFuzzer target.
//! - `project_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly, the cases reach every kind and every shape of answer, and
//!   each law fires on an answer changed to break it.

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

use geom_fuzz::project::{
    Case, MAX_INPUT_BYTES, Outcome, Shape, check, decode, harness, judge_curve, judge_surface,
    shape,
};
use geom_fuzz::seeded;
use geom_intersect::{Continuum, Projection, project_curve, project_surface};
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg. A search takes milliseconds
/// unoptimised, so this is a tenth of the other targets'.
const ITERATIONS: usize = 200;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 500;

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
fn project_fuzz() {
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
fn project_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "project fuzz target, seeded",
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

/// A case of `kind` with the point at `point`.
fn case(kind: u8, point: [u8; 3]) -> Case {
    Case {
        kind,
        size: 4,
        trim: 7,
        width: 7,
        point,
        work: 255,
    }
}

/// The byte whose coordinate is `v`, in half units.
fn coordinate(v: i16) -> u8 {
    u8::try_from(v + 8).unwrap()
}

#[test]
fn every_shape_of_answer_is_reached_on_purpose() {
    // A circle from its axis: the whole circle.
    let axis = [coordinate(0), coordinate(0), coordinate(3)];
    assert_eq!(check(&case(1, axis)).unwrap(), Outcome::Set);
    // A sphere from its centre: the whole sphere, and a cylinder from its axis: a parallel.
    let centre = [coordinate(0); 3];
    assert_eq!(check(&case(4, centre)).unwrap(), Outcome::Set);
    assert_eq!(check(&case(5, axis)).unwrap(), Outcome::Set);
    // A plane and a line each have one foot.
    let off = [coordinate(1), coordinate(2), coordinate(5)];
    assert_eq!(check(&case(3, off)).unwrap(), Outcome::One);
    assert_eq!(check(&case(0, off)).unwrap(), Outcome::One);
}

#[test]
fn the_cases_reach_every_kind_and_shape_of_answer() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("project outcomes", Seed::DEFAULT));
    let mut kinds = [0; 8];
    let (mut one, mut set, mut tie, mut open, mut refused) = (0, 0, 0, 0, 0);
    for _ in 0..400 {
        let mut data = [0u8; MAX_INPUT_BYTES];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        match check(&case).unwrap() {
            Outcome::One => one += 1,
            Outcome::Set => set += 1,
            Outcome::Tie => tie += 1,
            Outcome::NonIsolated => open += 1,
            Outcome::Refused => refused += 1,
        }
        kinds[usize::from(case.kind % 8)] += 1;
    }
    println!("{kinds:?} {one} {set} {tie} {open} {refused}");
    assert!(kinds.iter().all(|&n| n > 20));
    assert!(one > 100, "{set} {open} {refused}");
    // Sets take a point exactly on an axis: `every_shape_of_answer_is_reached_on_purpose` has them.
    assert!(tie > 0);
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Each law fires on a true answer changed to break it.
#[test]
fn a_changed_curve_answer_is_a_finding() {
    let c = case(1, [coordinate(3), coordinate(1), coordinate(2)]);
    let Shape::Curve(curve) = shape(&c).unwrap() else {
        panic!("a circle is a curve");
    };
    let p = c.at();
    let found = project_curve(&curve, &p, 1e-6, &mut meter()).unwrap();
    judge_curve(&curve, &p, &found).unwrap();
    let Projection::Points(points) = found else {
        panic!("one foot");
    };

    let mut far = points.clone();
    far[0].distance = Interval::point(far[0].distance.lo() + 1.0).unwrap();
    let law = judge_curve(&curve, &p, &Projection::Points(far)).unwrap_err();
    assert!(law.law.contains("nearer"), "{law}");

    let mut near = points.clone();
    near[0].distance = Interval::point(near[0].distance.lo() - 1.0).unwrap();
    let law = judge_curve(&curve, &p, &Projection::Points(near)).unwrap_err();
    assert!(law.law.contains("distance it reports"), "{law}");

    let mut moved = points.clone();
    moved[0].point[0] = Interval::point(moved[0].point[0].lo() + 1.0).unwrap();
    let law = judge_curve(&curve, &p, &Projection::Points(moved)).unwrap_err();
    assert!(law.law.contains("on the kind"), "{law}");

    let mut outside = points.clone();
    outside[0].at[0] = Interval::point(50.0).unwrap();
    let law = judge_curve(&curve, &p, &Projection::Points(outside)).unwrap_err();
    assert!(law.law.contains("in the domain"), "{law}");

    let two = vec![points[0], points[0]];
    let mut reversed = two;
    reversed[1].at[0] = Interval::point(points[0].at[0].lo() / 2.0).unwrap();
    let law = judge_curve(&curve, &p, &Projection::Points(reversed)).unwrap_err();
    assert!(law.law.contains("parameter order"), "{law}");

    let law = judge_curve(&curve, &p, &Projection::Points(Vec::new())).unwrap_err();
    assert!(law.law.contains("has a nearest point"), "{law}");
}

#[test]
fn a_changed_surface_answer_is_a_finding() {
    let c = case(4, [coordinate(3), coordinate(1), coordinate(2)]);
    let Shape::Surface(surface) = shape(&c).unwrap() else {
        panic!("a sphere is a surface");
    };
    let p = c.at();
    let found = project_surface(&surface, &p, 1e-6, &mut meter()).unwrap();
    judge_surface(&surface, &p, &found).unwrap();
    let wrong = Projection::Set {
        set: Continuum::Surface,
        distance: Interval::point(0.25).unwrap(),
    };
    let law = judge_surface(&surface, &p, &wrong).unwrap_err();
    assert!(law.law.contains("typed set"), "{law}");
}
