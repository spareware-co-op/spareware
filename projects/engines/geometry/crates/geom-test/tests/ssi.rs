//! The intersection acceptance harness (G160, 12.17): the case schema, the placement and its checks,
//! our reading of coverage, the output contract, every rule of the gate, and the failure catalogue.
//! The scorer itself is Python and runs on CI (`ssi/score.sh`): here its verdict comes from
//! [`scored`], which reads [`cover`] the way the scorer reads coverage.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]

use std::fs;
use std::path::{Path, PathBuf};

use geom_curve::{CurveKind, Line};
use geom_intersect::{
    Coincidence, Coverage, CurveContact, CurveEnd, Intersection, Item, OverlapRegion, ParamBox,
    PointContact, Unresolved,
};
use geom_math::Box3;
use geom_num::interval::Interval;
use geom_num::{Exactness, P3, Steps};
use geom_run::{Budget, WorkMeter};
use geom_test::corpus::{EMBEDDED, Manifest};
use geom_test::ssi::SsiError;
use geom_test::ssi::case::{Case, Class, Contact, Expected, Implicit, Surface};
use geom_test::ssi::catalogue;
use geom_test::ssi::cover::{Seen, compare, effective};
use geom_test::ssi::gate::{
    CEILINGS, Ceiling, Ceilings, Failure, Judging, Mode, Report, Run, Scored, Verdict, allowed,
    band, contact_band, floor_reach, judge, run, stands_for, total_size,
};
use geom_test::ssi::output::{Curve, Flag, Output, SUCCESS, Why};
use geom_test::ssi::place::{Placed, disagreement, holds_contact, place};
use geom_test::ssi::standin::StandIn;

/// How close a run at the reference tolerance must stay.
const ACCURACY: Option<f64> = Some(1e-5);

/// The catalogue case called `id`.
fn named(id: &str) -> Case {
    catalogue::all()
        .into_iter()
        .find(|c| c.id == id)
        .expect("a catalogue case")
}

/// A verdict as the scorer gives it, from the same reading of coverage.
fn scored(case: &Case, out: &Output) -> Scored {
    let seen = compare(case, out, effective(case, ACCURACY));
    let verdict = if !seen.missing.is_empty() {
        Verdict::Incomplete
    } else if seen.spurious > 0 {
        Verdict::WrongTopology
    } else {
        Verdict::Pass
    };
    Scored {
        id: case.id.clone(),
        verdict,
        flags: Vec::new(),
        time_ms: Some(1.5),
    }
}

/// A ceiling nothing here reaches.
fn open() -> Ceiling {
    Ceiling {
        count: 1 << 30,
        size: 1 << 60,
        why: "a test".into(),
    }
}

/// The rules `out` breaks for `case`.
fn broken(
    case: &Case,
    placed: &Placed,
    mode: Mode,
    out: &Output,
    ceiling: &Ceiling,
) -> Vec<String> {
    let scored = scored(case, out);
    let j = Judging {
        case,
        placed,
        mode,
        scored: &scored,
        output: out,
        accuracy: ACCURACY,
    };
    judge(&j, ceiling)
        .expect("a class the corpus has")
        .iter()
        .map(|f| f.name().to_owned())
        .collect()
}

/// `stand_in`'s answer to the catalogue case `id`, judged.
fn stand_in(id: &str, stand_in: StandIn, mode: Mode, ceiling: &Ceiling) -> Vec<String> {
    let case = named(id);
    let placed = place(&case, ACCURACY).expect("placed");
    let out = stand_in.answer(&case, &placed, ACCURACY).expect("answered");
    broken(&case, &placed, mode, &out, ceiling)
}

const SMALL: &str = r#"{
    "id": "tiny", "class": "1 transversal", "title": "two planes",
    "surfaces": [
        {"type": "plane", "origin": [0,0,0], "xaxis": [1,0,0], "yaxis": [0,1,0],
         "u": [-1,1], "v": [-1,1]},
        {"type": "plane", "origin": [0,0,0], "xaxis": [1,0,0], "yaxis": [0,0,1],
         "u": [-1,1], "v": [-1,1]}
    ],
    "truth": [
        {"kind": "plane", "point": [0,0,0], "normal": [0,0,1]},
        {"kind": "heightfield", "coeffs": [[0.0]]}
    ],
    "surface_labels": ["a", "b"],
    "expected": {"branches": [{"closed": false, "kind": "crossing",
                               "points": [[-1,0,0],[1,0,0]]}],
                 "isolated_points": [], "singular_points": [], "coincident_regions": []},
    "cov_tol": 0.001, "notes": "", "formula": "", "guessed": false
}"#;

#[test]
fn a_case_reads_as_the_corpus_writes_it() {
    let case = Case::parse(SMALL).expect("it fits");
    assert_eq!(case.id, "tiny");
    assert_eq!(case.class(), Ok(Class::Transversal));
    assert_eq!(case.truth[1], Implicit::Other);
    assert_eq!(case.truth[1].distance([0.0; 3]), None);
    assert_eq!(case.features().len(), 1);
    assert_eq!(case.features()[0].contact, Contact::Crossing);
    let again = Case::parse(&serde_json::to_string(&named("f_seam_line")).expect("written"));
    assert_eq!(again, Ok(named("f_seam_line")));
}

#[test]
fn a_shape_or_contact_it_does_not_know_is_refused() {
    for text in [
        SMALL.replace("\"plane\", \"origin\"", "\"blob\", \"origin\""),
        SMALL.replace("\"crossing\"", "\"glancing\""),
        SMALL.replace("\"cov_tol\": 0.001,", ""),
    ] {
        assert!(
            matches!(Case::parse(&text), Err(SsiError::Malformed { .. })),
            "{text}"
        );
    }
    let mut case = Case::parse(SMALL).expect("it fits");
    case.class = "0 nothing".into();
    assert!(matches!(case.class(), Err(SsiError::Malformed { .. })));
    let missing = PathBuf::from("no-such-case.json");
    assert!(matches!(Case::read(&missing), Err(SsiError::File { .. })));
}

#[test]
fn every_class_of_the_corpus_has_its_name() {
    let names = [
        ("1 transversal", Class::Transversal),
        ("2 tangent curve", Class::TangentCurve),
        ("3 isolated point", Class::IsolatedPoint),
        ("4 singular", Class::Singular),
        ("5 boundary", Class::Boundary),
        ("6 coincident", Class::Coincident),
        ("7 near-degenerate", Class::NearDegenerate),
        ("8 periodic/seam/pole", Class::Periodic),
        ("9 high degree/rational", Class::HighDegree),
        ("Y23 published parameters", Class::Yang),
        ("Y23 reconstruction", Class::Yang),
    ];
    for (name, class) in names {
        assert_eq!(Class::of(name), Some(class), "{name}");
    }
    assert_eq!(Class::of(""), None);
    // 12.17's list of the classes with no flagged regions.
    let clean: Vec<Class> = Class::ALL.into_iter().filter(|c| !c.may_flag()).collect();
    assert_eq!(
        clean,
        [
            Class::Transversal,
            Class::Boundary,
            Class::Coincident,
            Class::Periodic,
            Class::HighDegree
        ]
    );
}

#[test]
fn each_closed_form_gives_the_distance_to_its_surface() {
    let z = [0.0, 0.0, 1.0];
    let close =
        |a: Option<f64>, b: f64| assert!((a.expect("analytic") - b).abs() < 1e-12, "{a:?} {b}");
    let plane = Implicit::Plane {
        point: [0.0, 0.0, 1.0],
        normal: [0.0, 0.0, 2.0],
    };
    close(plane.distance([5.0, 5.0, 4.0]), 3.0);
    close(plane.distance([5.0, 5.0, -2.0]), 3.0);
    let sphere = Implicit::Sphere {
        center: [1.0, 0.0, 0.0],
        radius: 2.0,
    };
    close(sphere.distance([1.0, 0.0, 3.0]), 1.0);
    close(sphere.distance([1.0, 0.0, 0.5]), 1.5);
    let cylinder = Implicit::Cylinder {
        point: [0.0; 3],
        axis: z,
        radius: 2.0,
    };
    close(cylinder.distance([3.0, 0.0, 9.0]), 1.0);
    let cone = Implicit::Cone {
        point: [0.0; 3],
        axis: z,
        half_angle_deg: 45.0,
    };
    close(cone.distance([1.0, 0.0, 1.0]), 0.0);
    close(cone.distance([2.0, 0.0, 1.0]), 0.5_f64.sqrt());
    let torus = Implicit::Torus {
        point: [0.0; 3],
        axis: z,
        big: 3.0,
        small: 1.0,
    };
    close(torus.distance([4.0, 0.0, 0.0]), 0.0);
    close(torus.distance([3.0, 0.0, 2.0]), 1.0);
    close(torus.distance([3.0, 0.0, 0.0]), 1.0);
}

#[test]
fn a_case_goes_at_the_smallest_scale_that_keeps_its_truth() {
    // 0.001 at 2^16 steps is 65.5 steps; at 2^15 it is 32.8, under ε_fit.
    let case = Case::parse(SMALL).expect("it fits");
    let placed = place(&case, None).expect("placed");
    assert_eq!(placed.log2, 16);
    assert_eq!(placed.moved, [0.0, 0.0]);
    assert!(placed.extent > 0.0);
    // A stricter accuracy takes a finer scale: 1e-5 at 2^23 steps is 83.9, at 2^22 it is 41.9.
    assert_eq!(place(&case, ACCURACY).expect("placed").log2, 23);
    // A laxer one changes nothing: the case's own tolerance is the stricter.
    assert_eq!(place(&case, Some(1.0)).expect("placed").log2, 16);
}

#[test]
fn rounding_moves_data_to_whole_steps_and_counts_how_far() {
    let mut case = Case::parse(SMALL).expect("it fits");
    let Surface::Plane { origin, .. } = &mut case.surfaces[0] else {
        panic!("a plane");
    };
    *origin = [0.1, 0.0, 0.0];
    let placed = place(&case, None).expect("placed");
    let Surface::Plane { origin, u, .. } = &placed.surfaces[0] else {
        panic!("a plane");
    };
    let scale = placed.scale();
    assert_eq!(origin[0], (0.1 * scale).round_ties_even());
    assert_eq!(placed.moved[0], (origin[0] - 0.1 * scale).abs());
    assert_eq!(placed.moved[1], 0.0);
    // A parameter range is scaled, not rounded.
    assert_eq!(*u, [-scale, scale]);
    assert_eq!(placed.moved(), placed.moved[0]);
    assert_eq!(placed.units(placed.steps([0.3, 0.0, 0.0])), [0.3, 0.0, 0.0]);
    let snapped = placed.snapped([0.1, 0.0, 0.0]);
    assert_eq!(snapped[0] * scale, origin[0]);
}

#[test]
fn a_sample_off_a_surface_refuses_the_placement() {
    let mut case = Case::parse(SMALL).expect("it fits");
    // The second surface is the plane y = 0. Move the first so the truth leaves it.
    let Surface::Plane { origin, .. } = &mut case.surfaces[0] else {
        panic!("a plane");
    };
    *origin = [0.0, 0.0, 1.0];
    let Err(SsiError::Unplaced { id, why }) = place(&case, None) else {
        panic!("it should not place");
    };
    assert_eq!(id, "tiny");
    assert!(why.contains("off a surface"), "{why}");
    // Just inside the tolerance is kept; just outside is not.
    let at = |z: f64| {
        let mut c = Case::parse(SMALL).expect("it fits");
        if let Surface::Plane { origin, .. } = &mut c.surfaces[0] {
            *origin = [0.0, 0.0, z];
        }
        place(&c, None).is_ok()
    };
    assert!(at(0.000_99));
    assert!(!at(0.001_01));
}

#[test]
fn a_tolerance_too_small_for_the_range_is_refused() {
    let mut case = Case::parse(SMALL).expect("it fits");
    case.cov_tol = 1e-13;
    let Err(SsiError::Unplaced { why, .. }) = place(&case, None) else {
        panic!("it should not place");
    };
    assert!(why.contains("no scale"), "{why}");
}

/// A sphere that is not dyadic, with the truth that goes with it.
fn off_grid() -> Case {
    let mut case = named("f1_mirrored_spheres");
    case.expected = Expected::default();
    case.class = "7 near-degenerate".into();
    case
}

#[test]
fn a_stated_conditioning_pushes_the_scale_up() {
    let plain = place(&off_grid(), None).expect("placed");
    assert!(plain.moved() > 0.0);
    let mut case = off_grid();
    case.ill_cond = Some(1e6);
    let tight = place(&case, None).expect("placed");
    assert!(tight.log2 > plain.log2, "{} {}", tight.log2, plain.log2);
    assert!(tight.moved() * 1e6 <= case.cov_tol * tight.scale());
}

#[test]
fn a_stated_feature_size_pushes_the_scale_up() {
    let plain = place(&off_grid(), None).expect("placed");
    let mut case = off_grid();
    case.feature_size = Some(1e-6);
    let tight = place(&case, None).expect("placed");
    assert!(tight.log2 > plain.log2);
    assert!(tight.moved() * 4.0 <= 1e-6 * tight.scale());
}

#[test]
fn a_contact_on_data_off_the_grid_pushes_the_scale_up() {
    let plain = place(&off_grid(), None).expect("placed");
    let mut case = off_grid();
    case.expected.singular_points = vec![[0.0, 0.0, 3.0_f64.sqrt()]];
    let tight = place(&case, None).expect("placed");
    assert!(tight.log2 > plain.log2);
    let allowed = case.cov_tol * tight.scale();
    assert!(holds_contact(allowed, tight.moved(), tight.extent, 2));
}

#[test]
fn a_shallow_crossing_answers_to_its_conditioning_not_to_a_contact() {
    // A touch on data off the grid needs a scale that keeps the contact from spreading. A stated
    // conditioning replaces that with moved × factor, against the tolerance the scorer widens for it.
    let mut touch = off_grid();
    touch.expected.singular_points = vec![[0.0, 0.0, 3.0_f64.sqrt()]];
    let spread = place(&touch, None).expect("placed");
    let mut shallow = touch.clone();
    shallow.ill_cond = Some(2.0);
    let crossing = place(&shallow, None).expect("placed");
    assert!(
        crossing.log2 < spread.log2,
        "{} {}",
        crossing.log2,
        spread.log2
    );

    // At the scorer's accuracy the tolerance widens, so the same factor asks less of the scale.
    let mut steep = off_grid();
    steep.ill_cond = Some(1e6);
    let plain = place(&steep, None).expect("placed");
    let widened = place(&steep, ACCURACY).expect("placed");
    assert!(widened.log2 <= plain.log2);
    assert!(widened.moved() * 1e6 <= effective(&steep, ACCURACY) * widened.scale());
    assert!(widened.moved() * 1e6 > steep.cov_tol * widened.scale());
}

#[test]
fn a_contact_on_exact_data_does_not() {
    // Exact data moves nothing, so a contact there costs no scale.
    let case = named("f16_cylinders_external");
    let placed = place(&case, None).expect("placed");
    assert_eq!(placed.moved(), 0.0);
    assert_eq!(placed.log2, 13);
}

#[test]
fn the_named_high_order_contacts_take_their_order() {
    let mut case = off_grid();
    case.cov_tol = 0.01;
    case.expected.singular_points = vec![[0.0, 0.0, 3.0_f64.sqrt()]];
    let second = place(&case, None).expect("placed");
    assert_eq!(second.order, 2);
    case.id = "c2n_quartic_contact_parabola".into();
    let fourth = place(&case, None).expect("placed");
    assert_eq!(fourth.order, 4);
    assert!(fourth.log2 > second.log2);
    case.id = "y23_fig1f_high_order_contact".into();
    assert_eq!(place(&case, None).expect("placed").order, 3);
}

#[test]
fn the_band_is_the_bound_of_12_9() {
    // 12.9's example: a δ of one step and curvatures 10 mm apart, 64,000 steps, give 358 steps.
    assert!(holds_contact(358.0, 1.0, 64_000.0, 2));
    assert!(!holds_contact(357.0, 1.0, 64_000.0, 2));
    // Order three: x³ ≥ 6·δ·L².
    assert!(holds_contact(6.0, 1.0, 6.0, 3));
    assert!(!holds_contact(5.9, 1.0, 6.0, 3));
    // Order four: x⁴ ≥ 24·δ·L³, here 96.
    assert!(holds_contact(3.2, 0.5, 2.0, 4));
    assert!(!holds_contact(3.1, 0.5, 2.0, 4));
    // Exactly on the bound holds.
    assert!(holds_contact(2.0, 1.0, 2.0, 2));
    assert!(holds_contact(0.0, 0.0, 9.0, 2));
}

#[test]
fn a_nurbs_surface_moves_as_far_as_its_net() {
    let net = vec![
        [0.1, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.3],
        [1.0, 1.0, 0.0],
    ];
    let mut case = Case::parse(SMALL).expect("it fits");
    case.expected = Expected::default();
    case.surfaces[0] = Surface::Nurbs {
        label: "a".into(),
        degree: [1, 1],
        knots_u: vec![0.0, 0.0, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 1.0, 1.0],
        nu: 2,
        nv: 2,
        points: net.clone(),
        weights: vec![1.0, 2.0, 1.0, 1.0],
    };
    let placed = place(&case, None).expect("placed");
    let scale = placed.scale();
    let shifts = [
        (0.1 * scale).round_ties_even() - 0.1 * scale,
        (0.3 * scale).round_ties_even() - 0.3 * scale,
    ];
    let furthest = if shifts[0].abs() > shifts[1].abs() {
        shifts[0]
    } else {
        shifts[1]
    };
    assert_eq!(placed.moved[0], furthest.abs());
    let Surface::Nurbs {
        points,
        weights,
        knots_u,
        ..
    } = &placed.surfaces[0]
    else {
        panic!("a NURBS surface");
    };
    assert_eq!(points[1], [0.0, scale, 0.0]);
    // Weights and knots are not grid data.
    assert_eq!(*weights, vec![1.0, 2.0, 1.0, 1.0]);
    assert_eq!(*knots_u, vec![0.0, 0.0, 1.0, 1.0]);
    let kinds = placed.kinds("a").expect("the kernel takes it");
    assert_eq!(kinds.len(), 2);
}

#[test]
fn the_kernels_kinds_are_the_cases_surfaces() {
    for case in catalogue::all() {
        let placed = place(&case, ACCURACY).expect("placed");
        let off = disagreement(&case, &placed).expect("evaluated");
        assert!(off <= case.cov_tol / 8.0, "{}: {off:e}", case.id);
    }
}

#[test]
fn a_kernel_that_reads_a_case_differently_shows() {
    let mut case = named("f_seam_line");
    let placed = place(&case, ACCURACY).expect("placed");
    assert!(disagreement(&case, &placed).expect("evaluated") < 1e-3);
    // The closed form moved under the kernel's kinds.
    case.truth[1] = Implicit::Cylinder {
        point: [0.1, 0.0, 0.0],
        axis: [0.0, 0.0, 1.0],
        radius: 1.0,
    };
    assert!(disagreement(&case, &placed).expect("evaluated") > 0.05);
}

#[test]
fn a_surface_the_kernel_refuses_says_so() {
    let mut case = Case::parse(SMALL).expect("it fits");
    case.expected = Expected::default();
    let Surface::Plane { yaxis, .. } = &mut case.surfaces[0] else {
        panic!("a plane");
    };
    *yaxis = [1.0, 0.0, 0.0];
    let placed = place(&case, None).expect("placed");
    assert!(matches!(placed.kinds("tiny"), Err(SsiError::Kernel { .. })));
}

#[test]
fn the_tolerance_widens_as_the_scorers_does() {
    let mut case = Case::parse(SMALL).expect("it fits");
    assert_eq!(effective(&case, ACCURACY), 0.001);
    case.ill_cond = Some(1_000.0);
    assert_eq!(effective(&case, None), 0.001);
    assert_eq!(effective(&case, Some(0.0)), 0.001);
    assert_eq!(effective(&case, Some(1e-5)), 2.0 * 1e-5 * 1_000.0);
    case.ill_cond = Some(1e9);
    assert_eq!(effective(&case, Some(1e-5)), 0.5);
    case.ill_cond = Some(1.0);
    assert_eq!(effective(&case, Some(1e-5)), 0.001);
}

/// The case `tiny`'s answer of `curves`.
fn answer(curves: Vec<Vec<[f64; 3]>>) -> Output {
    Output {
        status: SUCCESS,
        status_name: None,
        time_ms: 0.0,
        curves: curves
            .into_iter()
            .map(|xyz| Curve {
                xyz,
                kind: "CROSSING".into(),
            })
            .collect(),
        points: Vec::new(),
        log2: 0,
        flagged: Vec::new(),
    }
}

#[test]
fn coverage_finds_what_is_missing_and_what_is_extra() {
    let case = Case::parse(SMALL).expect("it fits");
    let cov = 0.001;
    assert_eq!(
        compare(
            &case,
            &answer(vec![vec![[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]]]),
            cov
        ),
        Seen::default()
    );
    // A distance measured to the segment, not its ends: the truth's ends lie on the answer's line.
    let long = answer(vec![vec![
        [-2.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
    ]]);
    let seen = compare(&case, &long, cov);
    assert_eq!(seen.missing.len(), 0);
    // That curve runs past the truth.
    assert_eq!(seen.spurious, 1);
    // Half of it: the far end is missing.
    let half = compare(
        &case,
        &answer(vec![vec![[-1.0, 0.0, 0.0], [0.0, 0.0, 0.0]]]),
        cov,
    );
    assert_eq!(half.missing.len(), 1);
    assert_eq!(half.missing[0].feature, 0);
    assert_eq!(half.missing[0].contact, Contact::Crossing);
    assert_eq!(half.missing[0].samples, vec![[1.0, 0.0, 0.0]]);
    // Nothing at all.
    let none = compare(&case, &answer(Vec::new()), cov);
    assert_eq!(none.missing[0].samples.len(), 2);
    // Just within and just past the tolerance, off to one side.
    let near = |d: f64| {
        compare(
            &case,
            &answer(vec![vec![[-1.0, d, 0.0], [1.0, d, 0.0]]]),
            cov,
        )
    };
    assert_eq!(near(0.000_9), Seen::default());
    assert_eq!(near(0.001_1).missing.len(), 1);
    assert_eq!(near(0.001_1).spurious, 1);
    // Only the curve that leaves counts, and a point counts by itself.
    let mut two = answer(vec![
        vec![[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
        vec![[0.0, 3.0, 0.0], [0.0, 4.0, 0.0]],
    ]);
    assert_eq!(compare(&case, &two, cov).spurious, 1);
    two.points.push([9.0, 9.0, 9.0]);
    two.points.push([0.5, 0.0, 0.0]);
    assert_eq!(compare(&case, &two, cov).spurious, 2);
}

#[test]
fn coverage_thins_a_long_curve_as_the_scorer_does() {
    let mut case = Case::parse(SMALL).expect("it fits");
    let dense: Vec<[f64; 3]> = (0..2_500)
        .map(|i| [f64::from(i) / 1_250.0 - 1.0, 0.0, 0.0])
        .collect();
    case.expected.branches[0].points = dense;
    // 2,500 samples thin to every second one.
    let seen = compare(&case, &answer(Vec::new()), 0.001);
    assert_eq!(seen.missing[0].samples.len(), 1_250);
}

#[test]
fn an_isolated_point_is_judged_and_a_singular_point_is_not() {
    let mut case = Case::parse(SMALL).expect("it fits");
    case.expected.branches.clear();
    case.expected.isolated_points = vec![[0.5, 0.0, 0.0]];
    case.expected.singular_points = vec![[0.25, 0.0, 0.0]];
    let seen = compare(&case, &answer(Vec::new()), 0.001);
    assert_eq!(seen.missing.len(), 1);
    assert_eq!(seen.missing[0].contact, Contact::Isolated);
    let mut found = answer(Vec::new());
    found.points.push([0.5, 0.0, 0.0]);
    assert_eq!(compare(&case, &found, 0.001), Seen::default());
    // A lone sample of an answer curve is still measured.
    let lone = compare(&case, &answer(vec![vec![[0.5, 0.0, 0.0]]]), 0.001);
    assert_eq!(lone.missing.len(), 0);
}

#[test]
fn the_output_keeps_the_scorers_names() {
    let mut out = answer(vec![vec![[1.0, 2.0, 3.0]]]);
    out.flagged.push(Flag {
        why: Why::TangentBand,
        lo: [0; 3],
        hi: [1; 3],
    });
    let text = serde_json::to_value(&out).expect("written");
    assert_eq!(text["status"], 1000);
    assert_eq!(text["curves"][0]["type"], "CROSSING");
    assert_eq!(text["curves"][0]["xyz"][0][2], 3.0);
    assert_eq!(text["flagged"][0]["why"], "tangent-band");
    assert!(text.get("status_name").is_none());
    let back: Output = serde_json::from_value(text).expect("read");
    assert_eq!(back, out);
}

#[test]
fn the_digest_is_the_answer_without_the_time() {
    let base = answer(vec![vec![[1.0, 2.0, 3.0]]]);
    let mut timed = base.clone();
    timed.time_ms = 99.0;
    assert_eq!(base.digest(), timed.digest());
    let mut changes = Vec::new();
    let mut moved = base.clone();
    moved.curves[0].xyz[0][2] = 3.000_000_000_000_001;
    changes.push(moved);
    let mut status = base.clone();
    status.status = 7;
    changes.push(status);
    let mut scale = base.clone();
    scale.log2 = 1;
    changes.push(scale);
    let mut point = base.clone();
    point.points.push([0.0; 3]);
    changes.push(point);
    let mut flag = base.clone();
    flag.flagged.push(Flag {
        why: Why::NearMiss,
        lo: [0; 3],
        hi: [1; 3],
    });
    let mut other_flag = flag.clone();
    other_flag.flagged[0].why = Why::NonSmooth;
    let mut other_box = flag.clone();
    other_box.flagged[0].hi[1] = 2;
    changes.extend([flag, other_flag, other_box]);
    // A sample moved from one curve to the next is a different answer.
    let mut split = answer(vec![vec![[1.0, 2.0, 3.0]], Vec::new()]);
    split.time_ms = 0.0;
    changes.push(split);
    let digests: Vec<String> = changes.iter().map(Output::digest).collect();
    for (i, d) in digests.iter().enumerate() {
        assert_ne!(*d, base.digest(), "change {i}");
        for other in &digests[i + 1..] {
            assert_ne!(d, other);
        }
    }
}

/// A box of one case's grid.
fn box3(lo: [i64; 3], hi: [i64; 3]) -> Box3 {
    let p = |c: [i64; 3]| P3::new(c[0], c[1], c[2]).expect("on the grid");
    Box3::new(p(lo), p(hi)).expect("ordered")
}

/// A parameter box, four unknowns.
fn at(u: f64) -> ParamBox {
    let i = |x: f64| Interval::point(x).expect("finite");
    ParamBox::new(&[i(u), i(u), i(u), i(u)]).expect("a box")
}

#[test]
fn the_kernels_result_reaches_the_scorer_as_its_contract_says() {
    let case = Case::parse(SMALL).expect("it fits");
    let placed = place(&case, None).expect("placed");
    let scale = placed.scale();
    let line = |x: f64| {
        CurveKind::Line(Line::new([x, 0.0, 0.0], [scale, 0.0, 0.0], (0.0, 1.0)).expect("a line"))
    };
    let items = vec![
        Item::Point {
            at: at(1.0),
            xyz: box3([6, 8, 10], [8, 10, 12]),
            contact: PointContact::IsolatedTangent,
            class: Exactness::Exact,
        },
        Item::Curve {
            at: at(2.0),
            curve: Box::new(line(-scale)),
            contact: CurveContact::Crossing,
            ends: [CurveEnd::Boundary, CurveEnd::Boundary],
            class: Exactness::Exact,
        },
        Item::Curve {
            at: at(3.0),
            curve: Box::new(line(0.0)),
            contact: CurveContact::Tangent {
                band: Steps(1),
                gap: geom_test_steps(),
            },
            ends: [CurveEnd::Boundary, CurveEnd::Boundary],
            class: Exactness::Exact,
        },
        Item::Overlap {
            at: at(4.0),
            region: OverlapRegion {
                boundary: vec![line(scale)],
            },
            proof: Coincidence::Identical,
            class: Exactness::Exact,
        },
        Item::Flagged {
            at: at(5.0),
            xyz: box3([1, 2, 3], [4, 5, 6]),
            why: Unresolved::HigherOrderContact,
        },
    ];
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let result = Intersection::new(
        items,
        Coverage::CompleteOutsideFlagged,
        Vec::new(),
        &mut meter,
    )
    .expect("in order");
    let out = Output::of_intersection(&result, &placed, "tiny").expect("read");
    assert_eq!(out.status, SUCCESS);
    assert_eq!(out.log2, placed.log2);
    assert_eq!(out.points, vec![[7.0 / scale, 9.0 / scale, 11.0 / scale]]);
    let kinds: Vec<&str> = out.curves.iter().map(|c| c.kind.as_str()).collect();
    assert_eq!(kinds, ["CROSSING", "TANGENT", "COINCIDENT"]);
    let first = &out.curves[0].xyz;
    assert_eq!(first.len(), 1_024);
    assert_eq!(first[0], [-1.0, 0.0, 0.0]);
    assert_eq!(first[first.len() - 1], [0.0, 0.0, 0.0]);
    assert_eq!(
        out.flagged,
        vec![Flag {
            why: Why::HigherOrderContact,
            lo: [1, 2, 3],
            hi: [4, 5, 6],
        }]
    );
}

/// A gap for a tangent contact.
fn geom_test_steps() -> geom_intersect::StepsInterval {
    geom_intersect::StepsInterval::new(Steps(0), Steps(1)).expect("ordered")
}

#[test]
fn every_reason_of_the_kernel_has_its_name() {
    let gap = geom_test_steps();
    let pairs = [
        (
            Unresolved::TangentBand { width: Steps(1) },
            Why::TangentBand,
        ),
        (Unresolved::HigherOrderContact, Why::HigherOrderContact),
        (Unresolved::NearCoincident { gap }, Why::NearCoincident),
        (Unresolved::NearMiss { gap }, Why::NearMiss),
        (
            Unresolved::BoundExceeded { needed: Steps(1) },
            Why::BoundExceeded,
        ),
        (Unresolved::KnownCurveUnproven, Why::KnownCurveUnproven),
        (
            Unresolved::UndecidedAtFloor {
                test: geom_intersect::Test::Exclusion,
            },
            Why::UndecidedAtFloor,
        ),
        (
            Unresolved::NonSmooth {
                at: geom_intersect::Degeneracy::Pole,
            },
            Why::NonSmooth,
        ),
    ];
    for (unresolved, why) in pairs {
        assert_eq!(Why::of(unresolved), why);
    }
}

#[test]
fn a_flag_measures_its_box() {
    let flag = Flag {
        why: Why::NearMiss,
        lo: [0, 0, 0],
        hi: [3, 4, 0],
    };
    assert_eq!(flag.corners(), ([0.0; 3], [3.0, 4.0, 0.0]));
    assert_eq!(flag.diagonal(), 5.0);
    assert_eq!(total_size(&[flag, flag]), 10);
    let tilted = Flag {
        hi: [1, 1, 0],
        ..flag
    };
    assert_eq!(total_size(&[tilted]), 2);
    assert_eq!(total_size(&[]), 0);
}

#[test]
fn the_truth_passes_on_every_catalogue_case_in_both_modes() {
    for case in catalogue::all() {
        let placed = place(&case, ACCURACY).expect("placed");
        let out = StandIn::Truth
            .answer(&case, &placed, ACCURACY)
            .expect("answered");
        for mode in [Mode::Certify, Mode::Default] {
            let broke = broken(&case, &placed, mode, &out, &Ceiling::default());
            assert!(broke.is_empty(), "{}: {broke:?}", case.id);
        }
    }
}

#[test]
fn the_truth_splits_at_a_singular_point() {
    let mut case = Case::parse(SMALL).expect("it fits");
    case.expected.singular_points = vec![[0.5, 0.0, 0.0]];
    let placed = place(&case, ACCURACY).expect("placed");
    let out = StandIn::Truth
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    let curves: Vec<&[[f64; 3]]> = out.curves.iter().map(|c| c.xyz.as_slice()).collect();
    assert_eq!(
        curves,
        [
            &[[-1.0, 0.0, 0.0], [0.5, 0.0, 0.0]][..],
            &[[0.5, 0.0, 0.0], [1.0, 0.0, 0.0]][..]
        ]
    );
    assert!(out.curves.iter().all(|c| c.kind == "CROSSING"));
    // A curve that already ends there stays whole, and so does one that passes too far off.
    for point in [[1.0, 0.0, 0.0], [1.000_5, 0.0, 0.0], [2.0, 0.0, 0.0]] {
        case.expected.singular_points = vec![point];
        let placed = place(&case, ACCURACY).expect("placed");
        let out = StandIn::Truth
            .answer(&case, &placed, ACCURACY)
            .expect("answered");
        assert_eq!(out.curves.len(), 1, "{point:?}");
    }
    // Two points make three pieces, and the nearest segment is the one cut.
    case.expected.branches[0].points = vec![[-1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
    case.expected.singular_points = vec![[-0.25, 0.0, 0.0], [0.75, 0.0, 0.0]];
    let placed = place(&case, ACCURACY).expect("placed");
    let out = StandIn::Truth
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    let curves: Vec<&[[f64; 3]]> = out.curves.iter().map(|c| c.xyz.as_slice()).collect();
    assert_eq!(
        curves,
        [
            &[[-1.0, 0.0, 0.0], [-0.25, 0.0, 0.0]][..],
            &[[-0.25, 0.0, 0.0], [0.0, 0.0, 0.0], [0.75, 0.0, 0.0]][..],
            &[[0.75, 0.0, 0.0], [1.0, 0.0, 0.0]][..]
        ]
    );
}

#[test]
fn a_closed_curve_starts_at_its_singular_point() {
    let mut case = Case::parse(SMALL).expect("it fits");
    let corners = [
        [-1.0, 0.0, 0.0],
        [0.0, 0.5, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, -0.5, 0.0],
        [-1.0, 0.0, 0.0],
    ];
    case.expected.branches[0].points = corners.to_vec();
    case.expected.branches[0].closed = true;
    case.surfaces[1] = case.surfaces[0].clone();
    case.expected.singular_points = vec![[0.5, 0.25, 0.0]];
    let placed = place(&case, ACCURACY).expect("placed");
    let out = StandIn::Truth
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    assert_eq!(out.curves.len(), 1);
    assert_eq!(
        out.curves[0].xyz,
        [
            [0.5, 0.25, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, -0.5, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.5, 0.0],
            [0.5, 0.25, 0.0]
        ]
    );
}

#[test]
fn a_singular_point_cuts_one_curve_where_it_leaves_the_longest_pieces() {
    // Two lines crossing near one's end: cutting the other leaves nothing short.
    let mut case = Case::parse(SMALL).expect("it fits");
    case.surfaces[1] = case.surfaces[0].clone();
    case.expected.branches[0].points = vec![[-1.0, 0.0, 0.0], [0.9, 0.0, 0.0]];
    let other = geom_test::ssi::case::Branch {
        closed: false,
        kind: Contact::Crossing,
        points: vec![[0.8, -1.0, 0.0], [0.8, 1.0, 0.0]],
    };
    case.expected.branches.push(other);
    case.expected.singular_points = vec![[0.8, 0.0, 0.0]];
    let placed = place(&case, ACCURACY).expect("placed");
    let out = StandIn::Truth
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    assert_eq!(out.curves.len(), 3);
    assert_eq!(out.curves[0].xyz.len(), 2, "the first line stays whole");
    assert_eq!(
        out.curves[1].xyz.last(),
        Some(&placed.snapped([0.8, 0.0, 0.0]))
    );
    assert_eq!(
        out.curves[2].xyz.first(),
        Some(&placed.snapped([0.8, 0.0, 0.0]))
    );
}

#[test]
fn a_wrong_answer_fails_on_every_catalogue_case() {
    for stand_in in [StandIn::Shifted, StandIn::Spurious, StandIn::Dropped] {
        for case in catalogue::all() {
            let placed = place(&case, ACCURACY).expect("placed");
            let out = stand_in.answer(&case, &placed, ACCURACY).expect("answered");
            let broke = broken(&case, &placed, Mode::Default, &out, &open());
            if case.class().expect("a class").may_flag() && stand_in != StandIn::Spurious {
                assert!(
                    broke.contains(&"uncovered".to_owned()),
                    "{}: {broke:?}",
                    case.id
                );
            } else {
                assert!(
                    broke.contains(&"verdict".to_owned()),
                    "{}: {broke:?}",
                    case.id
                );
            }
            let certified = broken(&case, &placed, Mode::Certify, &out, &open());
            assert!(certified.contains(&"verdict".to_owned()), "{}", case.id);
        }
    }
}

#[test]
fn a_flagged_answer_passes_where_a_class_may_flag() {
    let ids = [
        "f16_cylinders_external",
        "f1_mirrored_cylinders",
        "f_apex_rulings",
    ];
    for id in ids {
        let broke = stand_in(id, StandIn::Flagged, Mode::Default, &open());
        assert!(broke.is_empty(), "{id}: {broke:?}");
    }
    let case = named("f16_cylinders_external");
    let placed = place(&case, ACCURACY).expect("placed");
    let out = StandIn::Flagged
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    assert!(out.flagged.len() > 1, "a chain of boxes");
    assert_eq!(out.curves.len(), 2, "the curve either side of the stretch");
    assert_eq!(scored(&case, &out).verdict, Verdict::Incomplete);
}

#[test]
fn a_class_that_may_not_flag_is_never_flagged_by_the_stand_in() {
    for case in catalogue::all() {
        if !case.class().expect("a class").may_flag() {
            let placed = place(&case, ACCURACY).expect("placed");
            let out = StandIn::Flagged
                .answer(&case, &placed, ACCURACY)
                .expect("answered");
            assert!(out.flagged.is_empty(), "{}", case.id);
        }
    }
}

#[test]
fn an_enlarged_region_fails() {
    for id in ["f16_cylinders_external", "f1_mirrored_cylinders"] {
        let broke = stand_in(id, StandIn::Enlarged, Mode::Default, &open());
        assert!(broke.contains(&"too-wide".to_owned()), "{id}: {broke:?}");
    }
}

#[test]
fn a_region_with_nothing_in_it_fails() {
    for id in ["f16_cylinders_external", "f1_mirrored_cylinders"] {
        let broke = stand_in(id, StandIn::Misplaced, Mode::Default, &open());
        assert!(broke.contains(&"no-feature".to_owned()), "{id}: {broke:?}");
    }
}

#[test]
fn a_region_whose_reason_does_not_fit_fails() {
    for id in ["f16_cylinders_external", "f1_mirrored_cylinders"] {
        let broke = stand_in(id, StandIn::WrongReason, Mode::Default, &open());
        assert!(broke.contains(&"wrong-kind".to_owned()), "{id}: {broke:?}");
    }
    // A gap covered only by a region that cannot stand for it is uncovered too.
    let broke = stand_in(
        "f16_cylinders_external",
        StandIn::WrongReason,
        Mode::Default,
        &open(),
    );
    assert!(broke.contains(&"uncovered".to_owned()), "{broke:?}");
}

#[test]
fn every_stand_in_breaks_the_rule_it_names() {
    for s in StandIn::ALL {
        assert_eq!(StandIn::named(s.name()), Some(s));
        let broke = stand_in("f16_cylinders_external", s, Mode::Default, &open());
        match s.expected() {
            None => assert!(broke.is_empty(), "{s:?}: {broke:?}"),
            Some("verdict") => assert!(!broke.is_empty(), "{s:?}"),
            Some(rule) => assert!(broke.iter().any(|b| b == rule), "{s:?}: {broke:?}"),
        }
    }
    assert_eq!(StandIn::named("nothing"), None);
}

#[test]
fn certify_mode_allows_no_flag_and_no_gap() {
    let broke = stand_in(
        "f16_cylinders_external",
        StandIn::Flagged,
        Mode::Certify,
        &open(),
    );
    assert!(
        broke.contains(&"flagged-in-certify".to_owned()),
        "{broke:?}"
    );
    assert!(broke.contains(&"verdict".to_owned()), "{broke:?}");
}

#[test]
fn a_clean_class_may_not_flag() {
    let case = named("f_seam_line");
    let placed = place(&case, ACCURACY).expect("placed");
    let mut out = StandIn::Truth
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    // A box right on the seam, the size of the floor: nothing wrong with it but where it is.
    let p = placed.steps([1.0, 0.0, 0.0]).map(f64::round_ties_even);
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a whole number of steps in range"
    )]
    let p = p.map(|x| x as i64);
    out.flagged.push(Flag {
        why: Why::UndecidedAtFloor,
        lo: p,
        hi: p.map(|x| x + 1),
    });
    let broke = broken(&case, &placed, Mode::Default, &out, &open());
    assert_eq!(broke, ["flagged-in-clean"]);
}

#[test]
fn the_ceilings_hold_the_count_and_the_size_apart() {
    let case = named("f16_cylinders_external");
    let placed = place(&case, ACCURACY).expect("placed");
    let out = StandIn::Flagged
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    let count = out.flagged.len() as u64;
    let size = total_size(&out.flagged);
    let at = |count: u64, size: u64| Ceiling {
        count,
        size,
        why: String::new(),
    };
    assert_eq!(
        broken(&case, &placed, Mode::Default, &out, &at(count, size)).len(),
        0
    );
    for (count, size) in [(count - 1, size), (count, size - 1), (0, 0)] {
        let broke = broken(&case, &placed, Mode::Default, &out, &at(count, size));
        assert_eq!(broke, ["over-ceiling"], "{count} {size}");
    }
}

#[test]
fn a_flag_reaches_as_far_as_the_case_allows_and_no_further() {
    let case = named("f16_cylinders_external");
    let placed = place(&case, ACCURACY).expect("placed");
    let floor = floor_reach(&placed);
    let wide = band(&placed);
    assert!(wide > floor);
    assert!(allowed(Why::UndecidedAtFloor, floor, &placed));
    assert!(!allowed(Why::UndecidedAtFloor, floor + 0.01, &placed));
    assert!(!allowed(Why::NonSmooth, wide, &placed));
    for why in [
        Why::TangentBand,
        Why::HigherOrderContact,
        Why::NearMiss,
        Why::NearCoincident,
    ] {
        assert!(allowed(why, wide * 0.999, &placed), "{why:?}");
        assert!(!allowed(why, wide * 1.001, &placed), "{why:?}");
    }
    // The floor's reach is three boxes' diagonal.
    assert!((floor - 3.0 * 3.0_f64.sqrt()).abs() < 1e-12);
    let moved = place(&off_grid(), ACCURACY).expect("placed");
    assert!(moved.moved() > 0.0);
    assert_eq!(floor_reach(&moved), 3.0 * 3.0_f64.sqrt() + moved.moved());
    let exact = (2.0 * (moved.moved() + 1.0) * moved.extent).sqrt();
    assert_eq!(band(&moved), exact);
}

#[test]
fn a_band_is_measured_to_the_line_not_its_samples() {
    // A box halfway between two samples a long way apart reaches as far as its own corners.
    let case = named("f16_cylinders_external");
    let placed = place(&case, ACCURACY).expect("placed");
    let out = StandIn::Flagged
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    let wide = band(&placed);
    assert!(out.flagged.iter().all(|f| f.diagonal() < 2.0 * wide));
    let step = case.expected.branches[0].points[1][2] - case.expected.branches[0].points[0][2];
    assert!(
        step * placed.scale() > wide,
        "samples sparser than the band"
    );
    assert_eq!(
        broken(&case, &placed, Mode::Default, &out, &open()).len(),
        0
    );
}

#[test]
fn a_higher_order_contact_widens_the_band_at_its_order() {
    let mut case = named("f16_cylinders_external");
    let second = place(&case, ACCURACY).expect("placed");
    case.id = "c2n_quartic_contact_parabola".into();
    let fourth = place(&case, ACCURACY).expect("placed");
    let wide = band(&second) * 4.0;
    assert!(!allowed(Why::TangentBand, wide, &second));
    assert!(allowed(Why::TangentBand, wide, &fourth));
    // At order four the band is where x⁴ = 24·δ·L³, δ one step.
    let width = contact_band(&fourth);
    assert_eq!(contact_band(&second), band(&second));
    assert!(holds_contact(width, 1.0, fourth.extent, 4));
    assert!(!holds_contact(width * 0.999, 1.0, fourth.extent, 4));
    assert!(allowed(Why::TangentBand, width * 0.999, &fourth));
    assert!(!allowed(Why::TangentBand, width * 1.001, &fourth));
}

#[test]
fn a_reason_stands_for_the_kinds_it_can_and_no_others() {
    use Contact::{
        Boundary, Coincident, CoincidentBorder, Crossing, Isolated, NearTangent, Singular, Tangent,
    };
    let every = [
        Crossing,
        Tangent,
        CoincidentBorder,
        NearTangent,
        Boundary,
        Isolated,
        Singular,
        Coincident,
    ];
    let table: [(Why, &[Contact]); 8] = [
        (Why::TangentBand, &[Tangent, NearTangent, Isolated]),
        (Why::HigherOrderContact, &[Tangent, Isolated, Singular]),
        (
            Why::NearCoincident,
            &[Coincident, CoincidentBorder, NearTangent],
        ),
        (Why::NearMiss, &[Crossing, Tangent, NearTangent, Isolated]),
        (Why::NonSmooth, &[Singular]),
        (
            Why::KnownCurveUnproven,
            &[
                Crossing,
                Tangent,
                CoincidentBorder,
                NearTangent,
                Boundary,
                Coincident,
            ],
        ),
        (Why::BoundExceeded, &every),
        (Why::UndecidedAtFloor, &every),
    ];
    for (why, kinds) in table {
        for contact in every {
            assert_eq!(
                stands_for(why, contact),
                kinds.contains(&contact),
                "{why:?} {contact:?}"
            );
        }
    }
}

#[test]
fn both_readings_must_agree() {
    let case = named("f_seam_line");
    let placed = place(&case, ACCURACY).expect("placed");
    let truth = StandIn::Truth
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    let dropped = StandIn::Dropped
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    let spurious = StandIn::Spurious
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    let rules = |out: &Output, verdict: Verdict| {
        let scored = Scored {
            id: case.id.clone(),
            verdict,
            flags: Vec::new(),
            time_ms: None,
        };
        let j = Judging {
            case: &case,
            placed: &placed,
            mode: Mode::Default,
            scored: &scored,
            output: out,
            accuracy: ACCURACY,
        };
        judge(&j, &open()).expect("judged")
    };
    let disagree = |f: &Failure| matches!(f, Failure::Disagree { .. });
    assert_eq!(rules(&truth, Verdict::Pass).len(), 0);
    assert!(rules(&dropped, Verdict::Pass).iter().any(disagree));
    assert!(rules(&spurious, Verdict::Pass).iter().any(disagree));
    assert!(rules(&truth, Verdict::Incomplete).iter().any(disagree));
    assert!(!rules(&dropped, Verdict::Incomplete).iter().any(disagree));
    // A verdict about something else asks nothing of the coverage.
    let err = rules(&truth, Verdict::Error);
    assert_eq!(
        err,
        vec![Failure::Verdict {
            verdict: Verdict::Error
        }]
    );
}

#[test]
fn every_other_verdict_fails_both_modes() {
    let case = named("f16_cylinders_external");
    let placed = place(&case, ACCURACY).expect("placed");
    let out = StandIn::Truth
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    for verdict in [
        Verdict::Crash,
        Verdict::Error,
        Verdict::WrongTopology,
        Verdict::Inaccurate,
        Verdict::NotJudged,
    ] {
        for mode in [Mode::Certify, Mode::Default] {
            let scored = Scored {
                id: case.id.clone(),
                verdict,
                flags: Vec::new(),
                time_ms: None,
            };
            let j = Judging {
                case: &case,
                placed: &placed,
                mode,
                scored: &scored,
                output: &out,
                accuracy: ACCURACY,
            };
            let failures = judge(&j, &open()).expect("judged");
            assert_eq!(failures, vec![Failure::Verdict { verdict }]);
        }
    }
}

#[test]
fn something_spurious_beside_a_gap_fails() {
    let case = named("f16_cylinders_external");
    let placed = place(&case, ACCURACY).expect("placed");
    let out = StandIn::Flagged
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    let judged = |flags: &[&str]| {
        let scored = Scored {
            id: case.id.clone(),
            verdict: Verdict::Incomplete,
            flags: flags.iter().map(|f| (*f).to_owned()).collect(),
            time_ms: None,
        };
        let j = Judging {
            case: &case,
            placed: &placed,
            mode: Mode::Default,
            scored: &scored,
            output: &out,
            accuracy: ACCURACY,
        };
        judge(&j, &open()).expect("judged")
    };
    assert_eq!(
        judged(&["branch 0 covered 90%", "topology differs"]).len(),
        0
    );
    for flag in [
        "output curve 1 off the true intersection by up to 3e-3",
        "output point 0 off the true intersection by 3e-3",
        "2 non-finite output sample(s)",
    ] {
        assert_eq!(judged(&[flag]), vec![Failure::Spurious], "{flag}");
    }
}

#[test]
fn an_answer_from_another_scale_fails() {
    let case = named("f_seam_line");
    let placed = place(&case, ACCURACY).expect("placed");
    let mut out = StandIn::Truth
        .answer(&case, &placed, ACCURACY)
        .expect("answered");
    out.log2 += 1;
    assert_eq!(
        broken(&case, &placed, Mode::Default, &out, &open()),
        ["off-scale"]
    );
}

#[test]
fn every_rule_has_its_name() {
    let names = [
        (
            Failure::Verdict {
                verdict: Verdict::Crash,
            },
            "verdict",
        ),
        (Failure::Spurious, "spurious"),
        (Failure::FlaggedInCertify { count: 1 }, "flagged-in-certify"),
        (Failure::FlaggedInClean { count: 1 }, "flagged-in-clean"),
        (Failure::Uncovered { feature: 0 }, "uncovered"),
        (Failure::NoFeature { region: 0 }, "no-feature"),
        (
            Failure::WrongKind {
                region: 0,
                why: Why::NearMiss,
            },
            "wrong-kind",
        ),
        (
            Failure::TooWide {
                region: 0,
                reach: 1.0,
            },
            "too-wide",
        ),
        (Failure::OverCeiling { count: 1, size: 1 }, "over-ceiling"),
        (
            Failure::Disagree {
                scorer: Verdict::Pass,
                missing: 1,
                spurious: 0,
            },
            "disagree",
        ),
        (Failure::OffScale { log2: 1 }, "off-scale"),
    ];
    for (failure, name) in names {
        assert_eq!(failure.name(), name);
    }
}

#[test]
fn the_register_of_ceilings_is_read_and_held() {
    let ceilings = Ceilings::parse(CEILINGS).expect("the register");
    assert_eq!(ceilings.classes.len(), Class::ALL.len());
    // Nothing is flagged yet, so nothing may be.
    assert!(
        ceilings
            .classes
            .values()
            .all(|c| c.count == 0 && c.size == 0)
    );
    let less = CEILINGS.replace("\"transversal\": { \"count\": 0, \"size\": 0, \"why\": \"12.17: this class may not flag\" },", "");
    assert!(
        matches!(Ceilings::parse(&less), Err(SsiError::Malformed { why, .. }) if why.contains("no ceiling"))
    );
    for field in ["\"count\": 1, \"size\": 0", "\"count\": 0, \"size\": 1"] {
        let raised = CEILINGS.replacen("\"count\": 0, \"size\": 0", field, 1);
        assert!(
            matches!(Ceilings::parse(&raised), Err(SsiError::Malformed { why, .. }) if why.contains("may not flag"))
        );
    }
    let open = CEILINGS.replacen(
        "\"tangent-curve\": { \"count\": 0, \"size\": 0",
        "\"tangent-curve\": { \"count\": 5, \"size\": 7",
        1,
    );
    assert!(Ceilings::parse(&open).is_ok());
    assert!(Ceilings::parse("{").is_err());
}

#[test]
fn raising_a_ceiling_never_lowers_it() {
    let mut ceilings = Ceilings::default();
    ceilings.raise(Class::Yang, 3, 10, "first");
    ceilings.raise(Class::Yang, 2, 20, "second");
    let c = &ceilings.classes[&Class::Yang];
    assert_eq!((c.count, c.size, c.why.as_str()), (3, 20, "second"));
    ceilings.raise(Class::Yang, 4, 5, "third");
    assert_eq!(ceilings.classes[&Class::Yang].count, 4);
    assert_eq!(ceilings.classes[&Class::Yang].size, 20);
}

/// A folder of its own under the target directory.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("geom-test-ssi-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("cases")).expect("made");
    fs::create_dir_all(dir.join("raw")).expect("made");
    dir
}

/// A run of `stand_in` over `cases`, judged.
fn judged_run(dir: &Path, cases: &[Case], stand_in: StandIn, ceilings: &Ceilings) -> Report {
    let mut lines = Vec::new();
    for case in cases {
        let placed = place(case, ACCURACY).expect("placed");
        let out = stand_in.answer(case, &placed, ACCURACY).expect("answered");
        fs::write(
            dir.join("cases").join(format!("{}.json", case.id)),
            serde_json::to_string(case).expect("written"),
        )
        .expect("wrote");
        fs::write(
            dir.join("raw").join(format!("{}.json", case.id)),
            serde_json::to_string(&out).expect("written"),
        )
        .expect("wrote");
        lines.push(scored(case, &out));
    }
    let scores = dir.join("scores.json");
    fs::write(&scores, serde_json::to_string(&lines).expect("written")).expect("wrote");
    let (cases_dir, raw) = (dir.join("cases"), dir.join("raw"));
    run(&Run {
        cases: &cases_dir,
        raw: &raw,
        scores: &scores,
        engine: stand_in.name(),
        mode: Mode::Default,
        tolerance: 1e-6,
        ceilings,
        credit: "the credit",
    })
    .expect("judged")
}

#[test]
fn a_report_publishes_every_case() {
    let dir = scratch("report");
    let mut sourced = named("f16_cylinders_external");
    sourced.source = Some("Yang, Jia & Yan 2023".into());
    let cases = [sourced, named("f_seam_line")];
    let mut ceilings = Ceilings::parse(CEILINGS).expect("the register");
    ceilings.raise(Class::TangentCurve, 1 << 20, 1 << 50, "a test");
    let report = judged_run(&dir, &cases, StandIn::Flagged, &ceilings);
    assert_eq!(report.engine, "flagged");
    assert_eq!(report.mode, Mode::Default);
    assert_eq!(report.credit, "the credit");
    assert_eq!(report.rows.len(), 2);
    assert_eq!(report.failed(), 0);
    let row = &report.rows[0];
    assert_eq!(row.class, Class::TangentCurve);
    assert_eq!(row.verdict, Verdict::Incomplete);
    assert_eq!(row.time_ms, Some(1.5));
    assert!(row.flags > 1);
    assert!(row.size > 0);
    assert!(row.log2 > 0);
    assert_eq!(row.source.as_deref(), Some("Yang, Jia & Yan 2023"));
    assert_eq!(report.rows[1].source, None);
    assert_eq!(report.rows[1].flags, 0);
    assert!(!report.broke("verdict"));

    // The same run again is the same answer, bit for bit; a changed one is named.
    let again = judged_run(&dir, &cases, StandIn::Flagged, &ceilings);
    assert_eq!(report.differs_from(&again).len(), 0);
    let other = judged_run(&dir, &cases, StandIn::Truth, &ceilings);
    assert_eq!(report.differs_from(&other), ["f16_cylinders_external"]);
    let mut lost = again.clone();
    lost.rows.remove(1);
    assert_eq!(report.differs_from(&lost), ["f_seam_line"]);

    // Over the register's ceilings the same run fails, and says which rule.
    let held = judged_run(
        &dir,
        &cases,
        StandIn::Flagged,
        &Ceilings::parse(CEILINGS).expect("read"),
    );
    assert_eq!(held.failed(), 1);
    assert!(held.broke("over-ceiling"));
    assert!(!held.broke("too-wide"));
    fs::remove_dir_all(&dir).expect("removed");
}

#[test]
fn a_runner_that_wrote_nothing_is_a_failure_with_its_verdict() {
    let dir = scratch("crash");
    let case = named("f_seam_line");
    let ceilings = Ceilings::parse(CEILINGS).expect("the register");
    judged_run(
        &dir,
        core::slice::from_ref(&case),
        StandIn::Truth,
        &ceilings,
    );
    fs::remove_file(dir.join("raw").join("f_seam_line.json")).expect("removed");
    let scores = dir.join("scores.json");
    let line = Scored {
        id: case.id.clone(),
        verdict: Verdict::Crash,
        flags: Vec::new(),
        time_ms: None,
    };
    fs::write(&scores, serde_json::to_string(&[line]).expect("written")).expect("wrote");
    let (cases, raw) = (dir.join("cases"), dir.join("raw"));
    let mut job = Run {
        cases: &cases,
        raw: &raw,
        scores: &scores,
        engine: "a test",
        mode: Mode::Default,
        tolerance: 1e-6,
        ceilings: &ceilings,
        credit: "",
    };
    let report = run(&job).expect("judged");
    assert!(report.broke("verdict"));
    assert_eq!(report.rows[0].digest, None);
    // An answer that says it failed has no digest either.
    let mut failed = Output {
        status: 5,
        ..answer(Vec::new())
    };
    failed.log2 = place(&case, ACCURACY).expect("placed").log2;
    fs::write(
        raw.join("f_seam_line.json"),
        serde_json::to_string(&failed).expect("written"),
    )
    .expect("wrote");
    assert_eq!(run(&job).expect("judged").rows[0].digest, None);
    // What cannot be read stops the run: it is not a verdict.
    fs::write(raw.join("f_seam_line.json"), "{").expect("wrote");
    assert!(matches!(run(&job), Err(SsiError::Malformed { .. })));
    let nowhere = dir.join("no-scores.json");
    job.scores = &nowhere;
    assert!(matches!(run(&job), Err(SsiError::File { .. })));
    fs::write(&nowhere, "{").expect("wrote");
    assert!(matches!(run(&job), Err(SsiError::Malformed { .. })));
    fs::remove_dir_all(&dir).expect("removed");
}

#[test]
fn the_credit_travels_with_the_corpus() {
    let manifest = Manifest::parse(EMBEDDED).expect("the register");
    let entry = manifest
        .corpora
        .iter()
        .find(|c| c.id == "ssi-corpus")
        .expect("listed");
    let credit = entry.credit.as_deref().expect("credited");
    assert!(credit.contains("Yang, Jia and Yan 2023"), "{credit}");
    assert!(credit.contains("CC BY"), "{credit}");
}

#[test]
fn the_catalogue_is_our_own_closed_forms() {
    let all = catalogue::all();
    assert_eq!(all.len(), 15);
    let mut ids: Vec<&str> = all.iter().map(|c| c.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), all.len());
    for case in &all {
        assert!(case.class().is_ok(), "{}", case.id);
        assert!(
            case.notes.starts_with("Rebuilds, with our own inputs: "),
            "{}",
            case.id
        );
        assert!(!case.formula.is_empty(), "{}", case.id);
        assert!(!case.title.is_empty(), "{}", case.id);
        let size = case.cov_tol * 1_024.0;
        for feature in case.features() {
            assert!(!feature.points.is_empty(), "{}", case.id);
            for p in feature.points {
                for truth in &case.truth {
                    let off = truth.distance(*p).expect("a closed form");
                    assert!(
                        off <= 1e-11 * (1.0 + size),
                        "{}: {p:?} is {off:e} off",
                        case.id
                    );
                }
            }
        }
    }
}

#[test]
fn the_catalogue_covers_the_lists_it_rebuilds() {
    let all = catalogue::all();
    let says = |text: &str| all.iter().filter(|c| c.notes.contains(text)).count();
    assert_eq!(says("issue #16"), 3);
    assert_eq!(says("issue #1,"), 3);
    assert_eq!(says("within 5 and 2 degrees"), 2);
    assert_eq!(says("one sampled candidate"), 1);
    assert_eq!(says("1/1000"), 2);
    assert_eq!(says("truncated traces"), 1);
    assert_eq!(says("seam-healing"), 2);
    assert_eq!(says("first branch"), 1);
    for case in &all {
        let closed = case.expected.branches.iter().filter(|b| b.closed);
        for branch in closed {
            let (a, b) = (branch.points[0], branch.points[branch.points.len() - 1]);
            let close = (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9 * (1.0 + case.cov_tol * 1_024.0));
            assert!(close, "{}", case.id);
        }
    }
}

#[test]
fn the_catalogue_writes_the_corpus_schema() {
    let dir = scratch("catalogue");
    catalogue::write(&dir.join("cases")).expect("wrote");
    for case in catalogue::all() {
        let read = Case::read(&dir.join("cases").join(format!("{}.json", case.id))).expect("read");
        assert_eq!(read, case);
    }
    assert!(matches!(
        catalogue::write(&dir.join("no-such-folder")),
        Err(SsiError::File { .. })
    ));
    fs::remove_dir_all(&dir).expect("removed");
}
