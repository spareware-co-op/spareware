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

use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_num::{NumError, P3, Sign};
use geom_poly::commit::{CommitError, Ideal, check_flip, moved_along, through_edge, vertex_slide};
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
