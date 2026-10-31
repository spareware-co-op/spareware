//! Committed planes: exact on their edges, within budget, and as near the ideal as they claim
//! (G142, 13.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and short names, and a failure stops the test"
)]

use geom_math::{Axis, GridTransform};
use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_num::plane::Tpi;
use geom_num::{Exactness, GridStep, Steps};
use geom_num::{NumError, P3, Sign};
use geom_op::{
    Bounds, ErrorKind, History, Limits, NameId, NameSet, OpError, OpErrorDetail, OpId, OpPath,
    Origin, Record,
};
use geom_poly::commit::{
    CommitError, Committed, Ideal, PlaneClass, check_flip, moved_along, through_edge, vertex_slide,
};
use num_bigint::BigInt;
use proptest::prelude::*;

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// A metre at q0, in steps.
const METRE: i64 = 6_400_000;

/// A 10 cm hinge along x at height zero, and a 10 cm face up the slope.
fn hinge() -> (P3, P3, Vec<P3>) {
    let (a, b) = (p(0, 0, 0), p(METRE / 10, 0, 0));
    let extent = vec![
        a,
        b,
        p(0, METRE / 10, METRE / 10),
        p(METRE / 10, METRE / 10, METRE / 10),
    ];
    (a, b, extent)
}

#[test]
fn a_committed_plane_holds_its_edge_exactly() {
    let (a, b, extent) = hinge();
    let tilt = [0.0, 0.973_876_197, 0.227_076_098];
    let got = through_edge(a, b, tilt, &extent).unwrap();
    assert_eq!(got.plane.side(a), Ok(Sign::Zero));
    assert_eq!(got.plane.side(b), Ok(Sign::Zero));
}

#[test]
fn a_short_hinge_keeps_a_plane_close_to_its_ideal() {
    let (a, b, extent) = hinge();
    let tilt = [0.0, 0.973_876_197, 0.227_076_098];
    let got = through_edge(a, b, tilt, &extent).unwrap();
    // The intervals' own width sets the bound: well under a step, far from what would count.
    assert!(got.deviation < 1e-6, "{}", got.deviation);
    assert!(got.deviation > 0.0);
}

#[test]
fn a_hinge_across_the_frame_stays_within_budget() {
    let m = (1i64 << 40) - 1;
    let (a, b) = (p(-m, -m, -m), p(m, m, m));
    let got = through_edge(a, b, [1.0, -1.0, 0.25], &[a, b]).unwrap();
    assert_eq!(got.plane.side(a), Ok(Sign::Zero));
    assert_eq!(got.plane.side(b), Ok(Sign::Zero));
    let far = p(m, -m, 0);
    let wide = through_edge(a, b, [1.0, -1.0, 0.25], &[a, b, far]).unwrap();
    assert!(wide.deviation >= got.deviation);
}

#[test]
fn a_tilt_along_the_edge_has_no_plane() {
    let (a, b, extent) = hinge();
    let refused = |tilt: [f64; 3], why: NumError| {
        assert_eq!(
            through_edge(a, b, tilt, &extent),
            Err(CommitError::Num(why))
        );
    };
    refused([1.0, 0.0, 0.0], NumError::Degenerate);
    refused([0.0, 0.0, 0.0], NumError::Degenerate);
    refused([0.0, 1e-300, 0.0], NumError::Degenerate);
    refused([0.0, 5e-324, 0.0], NumError::Degenerate);
    refused([f64::NAN, 1.0, 0.0], NumError::NotFinite);
    refused([0.0, f64::INFINITY, 0.0], NumError::NotFinite);
    refused([0.0, 1e300, 0.0], NumError::NotFinite);
}

#[test]
fn a_plane_moved_whole_steps_lands_where_it_should() {
    let direction = [0.6, 0.8, 0.0];
    let point = p(100, 200, 300);
    let extent = [point, p(100, 200, 900), p(900, -400, 300)];
    let got = moved_along(direction, point, 25, &extent).unwrap();
    // 25 steps along (3, 4, 0) / 5 is (15, 20, 0).
    let target = p(115, 220, 300);
    let n = got.plane.n();
    let length = (n.iter().map(|v| *v as f64 * *v as f64).sum::<f64>()).sqrt();
    let reading = n
        .iter()
        .zip([115.0, 220.0, 300.0])
        .map(|(c, v)| *c as f64 * v)
        .sum::<f64>()
        + got.plane.d() as f64;
    assert!((reading / length).abs() < 1e-3, "{}", reading / length);
    assert!(got.deviation < 1e-3, "{}", got.deviation);
    assert_ne!(got.plane.side(target), Err(NumError::Degenerate));
}

#[test]
fn moving_nothing_keeps_the_point_within_rounding() {
    let got = moved_along([0.0, 0.0, 1.0], p(5, 6, 7), 0, &[p(5, 6, 7)]).unwrap();
    assert_eq!(got.plane.side(p(5, 6, 7)), Ok(Sign::Zero));
    assert_eq!(got.plane.side(p(0, 0, 9)), Ok(Sign::Positive));
    assert_eq!(got.plane.side(p(0, 0, 2)), Ok(Sign::Negative));
}

#[test]
fn a_move_along_an_axis_is_exact() {
    let got = moved_along([0.0, 0.0, 1.0], p(5, 6, 7), 3, &[p(0, 0, 10)]).unwrap();
    assert_eq!(got.plane.side(p(1, 2, 10)), Ok(Sign::Zero));
    assert_eq!(got.plane.side(p(1, 2, 11)), Ok(Sign::Positive));
    let back = moved_along([0.0, 0.0, 1.0], p(5, 6, 7), -3, &[p(0, 0, 4)]).unwrap();
    assert_eq!(back.plane.side(p(1, 2, 4)), Ok(Sign::Zero));
}

#[test]
fn no_extent_proves_nothing() {
    assert_eq!(
        moved_along([0.0, 0.0, 1.0], p(5, 6, 7), 0, &[]),
        Err(CommitError::Num(NumError::Degenerate))
    );
    assert_eq!(
        moved_along([0.0, 0.0, 0.0], p(5, 6, 7), 0, &[p(1, 1, 1)]),
        Err(CommitError::Num(NumError::Degenerate))
    );
}

#[test]
fn a_sharp_wedge_slides_a_vertex_further_than_a_blunt_one() {
    let plane = |n: [i128; 3]| Plane::new(n, 0).unwrap();
    let (x, y, z) = (plane([1, 0, 0]), plane([0, 1, 0]), plane([0, 0, 1]));
    let square = vertex_slide([&x, &y, &z], [0.5, 0.25, 0.0]).unwrap();
    assert!((square - 0.75).abs() < 1e-12, "{square}");
    let sharp = plane([1000, 1, 0]);
    let wedge = vertex_slide([&x, &sharp, &z], [0.5, 0.25, 0.0]).unwrap();
    assert!(wedge > 100.0, "{wedge}");
    assert_eq!(
        vertex_slide([&x, &x, &z], [0.5, 0.25, 0.0]),
        Err(CommitError::Num(NumError::Degenerate))
    );
}

#[test]
fn a_commit_may_not_change_a_certain_decision() {
    let above = Interval::new(1.0, 2.0).unwrap();
    let tie = Interval::new(-1.0, 1.0).unwrap();
    assert_eq!(check_flip(Sign::Positive, above), Ok(()));
    assert_eq!(check_flip(Sign::Negative, above), Err(CommitError::Flip));
    assert_eq!(check_flip(Sign::Zero, above), Err(CommitError::Flip));
    assert_eq!(check_flip(Sign::Negative, tie), Ok(()));
    assert_eq!(check_flip(Sign::Zero, tie), Ok(()));
}

#[test]
fn a_commit_error_says_what_was_wrong() {
    assert_eq!(
        CommitError::Flip.to_string(),
        "a commit changed a certain decision"
    );
    assert!(
        CommitError::from(NumError::Degenerate)
            .to_string()
            .starts_with("a plane that cannot be committed")
    );
}

#[test]
fn an_ideal_plane_reads_a_signed_distance() {
    let ideal = Ideal::moved([3.0, 4.0, 0.0], p(0, 0, 0), 5).unwrap();
    assert!(ideal.reading(p(3, 4, 9)).unwrap().contains(0.0));
    let r = ideal.reading(p(6, 8, 0)).unwrap();
    assert!(r.lo() > 4.9 && r.hi() < 5.1, "{r:?}");
    let edge = Ideal::through_edge(p(0, 0, 0), p(9, 0, 0), [0.0, 1.0, 0.0]).unwrap();
    assert!(edge.reading(p(4, 7, 0)).unwrap().contains(0.0));
    let up = edge.reading(p(4, 7, 3)).unwrap();
    assert!(up.lo() > 2.9 && up.hi() < 3.1, "{up:?}");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn every_hinge_stays_on_its_edge(
        a in [-(1i64 << 39)..(1 << 39), -(1i64 << 39)..(1 << 39), -(1i64 << 39)..(1 << 39)],
        d in [-(1i64 << 38)..(1 << 38), -(1i64 << 38)..(1 << 38), -(1i64 << 38)..(1 << 38)],
        tilt in [-1.0f64..1.0, -1.0f64..1.0, -1.0f64..1.0],
    ) {
        let (start, end) = (p(a[0], a[1], a[2]), p(a[0] + d[0], a[1] + d[1], a[2] + d[2]));
        match through_edge(start, end, tilt, &[start, end]) {
            Ok(got) => {
                prop_assert_eq!(got.plane.side(start), Ok(Sign::Zero));
                prop_assert_eq!(got.plane.side(end), Ok(Sign::Zero));
                prop_assert!(got.deviation.is_finite());
            }
            Err(e) => prop_assert_eq!(e, CommitError::Num(NumError::Degenerate)),
        }
    }

    #[test]
    fn every_free_plane_lands_within_its_claim(
        direction in [-1.0f64..1.0, -1.0f64..1.0, -1.0f64..1.0],
        point in [-(1i64 << 30)..(1 << 30), -(1i64 << 30)..(1 << 30), -(1i64 << 30)..(1 << 30)],
        distance in -(1i64 << 30)..(1 << 30),
    ) {
        let at = p(point[0], point[1], point[2]);
        let ideal = Ideal::moved(direction, at, distance);
        prop_assume!(ideal.is_ok());
        let got = moved_along(direction, at, distance, &[at]).unwrap();
        prop_assert!(got.deviation < 1.0, "{}", got.deviation);
    }
}

/// Whether `a` is the nearest whole number to √`square`.
fn nearest_root(a: &BigInt, square: &BigInt) -> bool {
    let two = BigInt::from(2);
    let low = &two * a - 1;
    let high = &two * a + 1;
    let four = square * 4;
    &low * &low <= four && four <= &high * &high
}

#[test]
fn a_moved_plane_takes_the_nearest_whole_offset() {
    let point = p(10, -20, 30);
    let cases: [([f64; 3], i64); 6] = [
        ([1.0, 1.0, 0.0], 1),
        ([1.0, 2.0, 3.0], 7),
        ([-3.0, 5.0, 7.0], -12_345),
        ([0.3, -0.4, 0.5], 1 << 40),
        ([0.3, -0.4, 0.5], -(1 << 40)),
        ([1.0, 1.0, 1.0], 3),
    ];
    for (direction, distance) in cases {
        let c = moved_along(direction, point, distance, &[point]).unwrap();
        let n = c.plane.n().map(BigInt::from);
        let on_point = &n[0] * 10 + &n[1] * -20 + &n[2] * 30;
        let moved = -(BigInt::from(c.plane.d()) + on_point);
        let square = (&n[0] * &n[0] + &n[1] * &n[1] + &n[2] * &n[2]) * distance * distance;
        assert!(
            nearest_root(&moved.magnitude().clone().into(), &square),
            "{direction:?}"
        );
        assert_eq!(
            moved.sign() == num_bigint::Sign::Minus,
            distance < 0,
            "{direction:?}"
        );
    }
}

#[test]
fn a_move_past_the_budget_is_refused() {
    let point = p(0, 0, 0);
    assert_eq!(
        moved_along([1.0, 1.0, 1.0], point, i64::MAX, &[point]),
        Err(CommitError::Num(NumError::BudgetExceeded))
    );
}

#[test]
fn a_whole_length_move_is_exact() {
    // |(3, 4, 0)| is whole, so the offset is too: 5·k·δ on a normal of (3k, 4k, 0).
    let c = moved_along([3.0, 4.0, 0.0], p(0, 0, 0), 1_000, &[p(0, 0, 0)]).unwrap();
    let [x, y, z] = c.plane.n();
    assert_eq!(z, 0);
    assert_eq!(x * 4, y * 3);
    assert_eq!(c.plane.d(), -(x / 3 * 5 * 1_000));
}

#[test]
fn a_class_is_one_normal_for_every_plane_in_it() {
    let class = PlaneClass::holding([1_000, 0, 0], [0.0, 0.8, 0.6]).unwrap();
    let (a, b) = (p(0, 0, 0), p(5, 900, 7));
    let extent = [p(0, 0, 0), p(1_000, 0, 0)];
    let (one, two) = (
        class.plane(a, 0, &extent).unwrap(),
        class.plane(b, 0, &extent).unwrap(),
    );
    assert_eq!(one.plane.n(), class.normal());
    assert_eq!(two.plane.n(), class.normal());
    // Each holds its own eave exactly, however long.
    assert_eq!(one.plane.side(p(1_000_000, 0, 0)).unwrap(), Sign::Zero);
    assert_eq!(two.plane.side(p(1_000_005, 900, 7)).unwrap(), Sign::Zero);
    assert_ne!(one.plane.side(b).unwrap(), Sign::Zero);
}

#[test]
fn a_hip_roof_keeps_its_apex_under_the_footprints_symmetries() {
    // A 30 degree slope on a square footprint: four planes, one apex, which only a shared commit
    // makes a true tie.
    let h = METRE;
    let slope = [0.0, 3.0_f64.sqrt() / 2.0, 0.5];
    let class = PlaneClass::holding([1_000, 0, 0], slope).unwrap();
    let turn = GridTransform::quarter_turns(Axis::Z, 1);
    let mut corner = p(-h, -h, 0);
    let mut side = class;
    let mut planes = Vec::new();
    for _ in 0..4 {
        planes.push(side.plane(corner, 0, &[corner]).unwrap());
        side = side.image(&turn);
        corner = turn.apply(corner).unwrap();
    }
    let apex = Tpi::new(planes[0].plane, planes[1].plane, planes[2].plane).unwrap();
    assert_eq!(planes[3].plane.side_of_tpi(&apex).unwrap(), Sign::Zero);
    // Each image's normal is the last one's, turned: nothing rounded again.
    let [x, y, z] = class.normal();
    assert_eq!(class.image(&turn).normal(), [-y, x, z]);
    // A mirror carries the second plane onto the fourth, its reflection.
    let mirror = GridTransform::mirror(Axis::X);
    let second = class.image(&turn);
    let fourth = second.image(&turn).image(&turn);
    assert_eq!(second.image(&mirror).normal(), fourth.normal());
}

#[test]
fn an_image_strays_from_its_ideal_as_far_as_the_plane_it_came_from() {
    let class = PlaneClass::holding([1_000, 0, 0], [0.0, 0.8, 0.6]).unwrap();
    let extent = [
        p(0, 0, 0),
        p(1_000, 0, 0),
        p(0, 800, 600),
        p(1_000, 800, 600),
    ];
    let base = class.plane(extent[0], 0, &extent).unwrap();
    for symmetry in [
        GridTransform::quarter_turns(Axis::Z, 1),
        GridTransform::quarter_turns(Axis::X, 3),
        GridTransform::mirror(Axis::Y),
    ] {
        let moved: Vec<P3> = extent.iter().map(|q| symmetry.apply(*q).unwrap()).collect();
        let image = class.image(&symmetry).plane(moved[0], 0, &moved).unwrap();
        assert!(
            image.deviation <= base.deviation * 2.0 + 1e-3,
            "{}",
            image.deviation
        );
        for q in &moved {
            assert_eq!(
                image.plane.side(*q).unwrap(),
                base.plane.side(extent[0]).unwrap()
            );
        }
    }
}

#[test]
fn a_commit_is_certified_however_small_its_deviation() {
    let c = Committed {
        plane: Plane::new([1, 0, 0], 0).unwrap(),
        deviation: 1e-6,
    };
    assert_eq!(c.term(), Steps(1));
    assert_eq!(
        c.class(GridStep::Q1),
        Exactness::Certified {
            bound: Steps(1),
            step: GridStep::Q1
        }
    );
    let wide = Committed {
        deviation: 3.0,
        ..c
    };
    assert_eq!(wide.term(), Steps(3));
    let wider = Committed {
        deviation: 3.25,
        ..c
    };
    assert_eq!(wider.term(), Steps(4));
}

#[test]
fn a_flip_is_exactness_not_accepted_by_the_name_of_commit_flip() {
    let error = OpError::new(
        OpPath::new(OpId::from_bytes([1; 16])),
        OpErrorDetail::from(CommitError::Flip),
    );
    assert_eq!(error.kind(), ErrorKind::ExactnessNotAccepted);
    assert_eq!(
        OpErrorDetail::from(CommitError::Num(NumError::Degenerate)).kind(),
        ErrorKind::Degenerate
    );
}

#[test]
fn a_commits_term_adds_along_the_face_it_made() {
    let (a, b, extent) = hinge();
    let committed = through_edge(a, b, [0.0, 0.7, 0.7], &extent).unwrap();
    let face = NameId::from_bytes([1; 16]);
    let step = OpPath::new(OpId::from_bytes([2; 16]));
    let empty = NameSet::new([]).unwrap();
    let made = History::new(
        [Record {
            output: face,
            origin: Origin::Unchanged,
        }],
        empty.clone(),
        [],
    )
    .unwrap();
    let start = Bounds::new(GridStep::Q0);
    // Passed through unchanged, a term is a defect: it has to be a modified or a new face.
    assert!(
        start
            .after(&step, &made, &[(face, committed.term())], Limits::DEFAULT)
            .is_err()
    );
    let drafted = History::new(
        [Record {
            output: face,
            origin: Origin::Modified {
                from: NameSet::new([face]).unwrap(),
            },
        }],
        empty,
        [],
    )
    .unwrap();
    let bounds = start
        .after(
            &step,
            &drafted,
            &[(face, committed.term())],
            Limits::DEFAULT,
        )
        .unwrap();
    assert_eq!(bounds.of(face), committed.term());
    assert_eq!(bounds.class(), committed.class(GridStep::Q0));
}
