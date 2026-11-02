//! The general fuzz target (G169), run as tests on stable Rust: bolero's test mode, the seeded leg,
//! the input bound, what it reaches, and that a wrong answer is found.
//!
//! - `general_fuzz` wraps `geom_fuzz::general::harness` in `bolero::check!`: the crate's corpus and
//!   crash files, then [`ITERATIONS`] random byte strings; a failing one's seed is printed as
//!   `BOLERO_RANDOM_SEED=<seed>`. Under `cargo bolero test general_fuzz --package geom-fuzz` it is
//!   a libFuzzer target.
//! - `general_seeded` runs the same harness on [`SEEDED_CASES`] byte strings from the printed seed
//!   (`SPAREKIT_SEED` replays it), the same on every target.
//! - The input bound is reached exactly, the cases reach every surface and shape of answer, and
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

use geom_curve::Frame;
use geom_fuzz::general::{
    Case, MAX_INPUT_BYTES, Outcome, check, decode, harness, judge, judge_contact, judge_march,
    surface,
};
use geom_fuzz::seeded;
use geom_intersect::contact::Reading;
use geom_intersect::general::{Settled, Subdivided, subdivide};
use geom_intersect::march::march;
use geom_intersect::result::{Coincidence, CurveContact, PointContact, StepsInterval, Unresolved};
use geom_num::grid::Steps;
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{ImplicitBound, ParamSurface, Sphere, SurfaceKind};
use geom_test::{Seed, SplitMix64};

/// Random byte strings per `cargo test` run of the bolero leg. A subdivision takes tens of
/// milliseconds unoptimised, so this is far below the other targets'.
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
fn general_fuzz() {
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
fn general_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "general fuzz target, seeded",
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
    holds(check(case).map(|_| ()));
    // The empty input is a case too.
    let (empty, read) = decode(&[]);
    assert_eq!(read, 0);
    holds(check(empty).map(|_| ()));
}

#[test]
fn the_cases_reach_every_surface_and_answer() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("general outcomes", Seed::DEFAULT));
    let mut surfaces = [0; 4];
    let (mut settled, mut empty) = (0, 0);
    for _ in 0..60 {
        let mut data = [0u8; MAX_INPUT_BYTES];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        match check(case).unwrap() {
            Outcome::Settled => settled += 1,
            Outcome::Empty => empty += 1,
            Outcome::Flagged | Outcome::Refused => {}
        }
        surfaces[usize::from(case.surface % 4)] += 1;
    }
    println!("{surfaces:?} {settled} {empty}");
    assert!(surfaces.iter().all(|&n| n > 3));
    assert!(settled > 5, "{settled}");
}

/// A sphere through a plane patch, and the true answer.
fn pair() -> (SurfaceKind, SurfaceKind, Sphere, Subdivided) {
    let case = Case {
        surface: 0,
        size: 7,
        second: 0,
        at: [8; 3],
        work: 255,
    };
    let plane = surface(case).unwrap();
    let frame = Frame::standard().unwrap();
    let sphere = Sphere::new([0.0; 3], 2.5, frame, ((0.0, 2.0), (-0.45, 0.45))).unwrap();
    let other = SurfaceKind::Sphere(sphere.clone());
    let found = subdivide(
        &plane,
        &other,
        1e-2,
        &mut WorkMeter::new(Budget::of_work(1 << 22)),
    )
    .unwrap();
    (plane, other, sphere, found)
}

#[test]
fn a_true_answer_passes() {
    let (plane, other, sphere, found) = pair();
    assert!(!found.settled.is_empty(), "{found:?}");
    let implicit: &dyn ImplicitBound = &sphere;
    judge(&plane, &other, implicit, &found).unwrap();
}

#[test]
fn a_lost_pair_is_a_finding() {
    let (plane, other, sphere, mut found) = pair();
    found.settled.clear();
    let implicit: &dyn ImplicitBound = &sphere;
    let law = judge(&plane, &other, implicit, &found).unwrap_err();
    assert!(law.law.contains("crossing"), "{law}");
}

#[test]
fn an_odd_count_is_a_finding() {
    let (plane, other, sphere, mut found) = pair();
    found.settled[0].starts.pop();
    let implicit: &dyn ImplicitBound = &sphere;
    let law = judge(&plane, &other, implicit, &found).unwrap_err();
    assert!(law.law.contains("even"), "{law}");
}

#[test]
fn a_start_point_off_a_surface_is_a_finding() {
    let (plane, other, sphere, mut found) = pair();
    let far = Interval::point(0.25).unwrap();
    found.settled[0].starts[0][2] = far;
    found.settled[0].starts[0][3] = far;
    let implicit: &dyn ImplicitBound = &sphere;
    let law = judge(&plane, &other, implicit, &found).unwrap_err();
    assert!(law.law.contains("both surfaces"), "{law}");
}

#[test]
fn a_march_is_held_to_its_laws() {
    let (plane, other, _, found) = pair();
    let mut meter = WorkMeter::new(Budget::of_work(1 << 22));
    let mut traced = march(&plane, &other, &found, 1e-2, 1e-1, &mut meter).unwrap();
    judge_march(&plane, &other, &traced).unwrap();
    traced.branches[0].deviation = 1.0;
    let law = judge_march(&plane, &other, &traced).unwrap_err();
    assert!(law.law.contains("ceiling"), "{law}");
    traced.branches[0].deviation = 0.0;
    let far = Interval::point(0.25).unwrap();
    traced.branches[0].vertices[0][2] = far;
    traced.branches[0].vertices[0][3] = far;
    let law = judge_march(&plane, &other, &traced).unwrap_err();
    assert!(law.law.contains("both surfaces"), "{law}");
}

#[test]
fn an_analysis_is_held_to_its_laws() {
    let (first, second, _, _) = pair();
    let (a, b) = (first.domain(), second.domain());
    let point = |x: f64| Interval::point(x).unwrap();
    let mid = |(lo, hi): (f64, f64)| point(lo / 2.0 + hi / 2.0);
    let at = [mid(a.0), mid(a.1), mid(b.0), mid(b.1)];
    let near = Interval::new(-1.0, 1.0).unwrap();
    let judged = |r: Reading| judge_contact(&first, &second, &[r]);
    let higher = Unresolved::HigherOrderContact;
    let gap = StepsInterval::new(Steps(0), Steps(1)).unwrap();
    let fine = [
        Reading::Undecided {
            first: a,
            second: b,
            why: higher,
        },
        // A contact read from its branches is a box, and its middle need not be on either surface.
        Reading::Point {
            at,
            gap: near,
            contact: PointContact::Singular { branches: 2 },
        },
        Reading::Overlap {
            first: a,
            second: b,
            coincidence: Coincidence::Identical,
        },
    ];
    for reading in fine {
        assert!(judged(reading).is_ok());
    }
    let beyond = |r: ((f64, f64), (f64, f64))| ((r.0.0, r.0.1 + 1.0), r.1);
    let broken = [
        Reading::Point {
            at,
            gap: point(1.0),
            contact: PointContact::Singular { branches: 2 },
        },
        Reading::Point {
            at,
            gap: near,
            contact: PointContact::IsolatedTangent,
        },
        Reading::Point {
            at,
            gap: near,
            contact: PointContact::Singular { branches: 4 },
        },
        Reading::Crossing(Settled {
            first: a,
            second: b,
            starts: vec![at],
        }),
        Reading::Curve {
            first: a,
            second: b,
            contact: CurveContact::Tangent {
                band: Steps(2),
                gap,
            },
        },
        Reading::Overlap {
            first: a,
            second: b,
            coincidence: Coincidence::Within(Steps(2)),
        },
        Reading::Undecided {
            first: beyond(a),
            second: b,
            why: higher,
        },
        Reading::Undecided {
            first: a,
            second: beyond(b),
            why: higher,
        },
    ];
    for reading in broken {
        assert!(judged(reading).is_err());
    }
}
