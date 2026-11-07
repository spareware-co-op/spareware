//! Attack tests from SMLib's confirmed defects, batch 2: `geom-math`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: the exact or correctly rounded answer, or a defined
//! refusal, and never a panic, a NaN or a silently wrong value.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::manual_midpoint,
    clippy::type_complexity,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::range_minus_one,
    reason = "tests: native arithmetic, casts and indexing, platform maths on an independent oracle (the formula as it is written), prose that names defect files, and a failure stops the test"
)]

use geom_math::{Box3, F3, FBox3, Transform};
use geom_num::NumError;
use geom_num::grid::P3;

fn p3(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn f3(x: f64, y: f64, z: f64) -> F3 {
    F3::new(x, y, z).unwrap()
}

/// The eight corners of a float box as (x, y, z) arrays.
fn corners(lo: [f64; 3], hi: [f64; 3]) -> Vec<[f64; 3]> {
    (0..8u8)
        .map(|c| {
            [
                if c & 1 == 0 { lo[0] } else { hi[0] },
                if c & 2 == 0 { lo[1] } else { hi[1] },
                if c & 4 == 0 { lo[2] } else { hi[2] },
            ]
        })
        .collect()
}

/// A quarter turn about z with exact entries: x' = -y, y' = x, z' = z.
fn quarter_turn_z() -> Transform {
    Transform::from_rows([
        [0.0, -1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ])
    .unwrap()
}

/// solidcore plan/upstream-issues/WR-8--smtransform-evaluateboundingboxsimple-aligned-branch-is-not.md
/// (b4de2149): the box of a box under a translation was a scaled and shifted copy of its
/// intervals, not the translated box. The input is the defect's: a translation-only transform on
/// an axis-aligned box. The image must hold the exact image of every corner and be no wider than
/// the box, under a translation, a scale with a mirror, and a quarter turn.
#[test]
fn wr_8_float_box_under_a_translation_is_the_translated_box() {
    let lo = [1.0, 3.0, -7.0];
    let hi = [2.0, 5.0, -6.0];
    let b = FBox3::new(f3(lo[0], lo[1], lo[2]), f3(hi[0], hi[1], hi[2])).unwrap();
    // Translation only: the image is the box moved by (10, -20, 30).
    let t = Transform::translation(f3(10.0, -20.0, 30.0));
    let image = b.transformed(&t).unwrap();
    let want_lo = [11.0, -17.0, 23.0];
    let want_hi = [12.0, -15.0, 24.0];
    let axes = image.axes().unwrap();
    for (iv, (l, h)) in axes.iter().zip(want_lo.iter().zip(want_hi.iter())) {
        assert!(
            iv.lo() <= *l && iv.hi() >= *h,
            "image must hold the moved box"
        );
        assert!(
            *l - iv.lo() < 1e-9 && iv.hi() - *h < 1e-9,
            "image must be tight"
        );
    }
    // Scale by (2, 0.5, -1): the exact image of every corner is inside, the width is exact.
    let t = Transform::scale(2.0, 0.5, -1.0).unwrap();
    let image = b.transformed(&t).unwrap();
    for c in corners(lo, hi) {
        let p = t.apply_point(f3(c[0], c[1], c[2])).unwrap();
        assert!(
            image.contains(p),
            "corner {c:?} maps outside the box of the image"
        );
    }
    let [x, y, z] = image.axes().unwrap();
    assert!((x.hi() - x.lo() - 2.0).abs() < 1e-9);
    assert!((y.hi() - y.lo() - 1.0).abs() < 1e-9);
    assert!((z.hi() - z.lo() - 1.0).abs() < 1e-9);
    // A quarter turn about z: x' = -y, y' = x. Corners stay inside, the box does not grow.
    let t = quarter_turn_z();
    let image = b.transformed(&t).unwrap();
    for c in corners(lo, hi) {
        let p = t.apply_point(f3(c[0], c[1], c[2])).unwrap();
        assert!(
            image.contains(p),
            "corner {c:?} maps outside the box of the image"
        );
    }
    let [x, y, _] = image.axes().unwrap();
    assert!(
        (x.hi() - x.lo() - 2.0).abs() < 1e-9,
        "x width {}",
        x.hi() - x.lo()
    );
    assert!(
        (y.hi() - y.lo() - 1.0).abs() < 1e-9,
        "y width {}",
        y.hi() - y.lo()
    );
}

/// solidcore plan/upstream-issues/WR-3--smextent3d-containslineseg3d-accepts-one-end-inside.md
/// (b4de2149): a segment with one end inside the box and the other far outside was reported as
/// inside, because the two end tests were OR-ed. Ours states "the segment is inside" as the box
/// of its two ends being inside the box, which holds only when both ends are.
#[test]
fn wr_3_a_segment_with_one_end_outside_is_not_inside_the_box() {
    let b = Box3::new(p3(0, 0, 0), p3(10, 10, 10)).unwrap();
    let inside = p3(5, 5, 5);
    let far = p3(1_000_000, 5, 5);
    assert!(b.contains(inside) && !b.contains(far));
    let seg = Box3::from_points([inside, far]).unwrap();
    assert!(
        !b.contains_box(&seg),
        "one end outside: the segment is not inside"
    );
    // Both ends inside: inside. Both ends outside, segment crossing through: not inside.
    let seg = Box3::from_points([inside, p3(1, 9, 3)]).unwrap();
    assert!(b.contains_box(&seg));
    let through = Box3::from_points([p3(-5, 5, 5), far]).unwrap();
    assert!(!b.contains_box(&through));
    // The float box says the same.
    let fb = FBox3::from_box3(&b);
    assert!(fb.contains(f3(5.0, 5.0, 5.0)) && !fb.contains(f3(1e6, 5.0, 5.0)));
    let fseg = FBox3::new(f3(5.0, 5.0, 5.0), f3(1e6, 5.0, 5.0)).unwrap();
    assert!(fb.overlaps(&fseg));
    assert_eq!(fb.intersection(&fseg).unwrap().hi().x(), 10.0);
    assert_eq!(fb.union(&fseg).hi().x(), 1e6);
}

/// solidcore plan/upstream-issues/WR-4--smextent3d-approximateunbounded-computes-max-from-the.md
/// and WR-9--smpolarbox-addvector3d-relabels-a-region-as-an-arc-without.md (b4de2149): an extent
/// with an infinite end was handled by replacing its ends one after the other, so the new maximum
/// came out of the already replaced minimum. Ours has no infinite box: an infinite or NaN bound is
/// refused when the point is made, a box wider than the doubles can hold is refused when it is
/// moved, and a box with a collapsed axis is still the box it was made as.
#[test]
fn wr_4_wr_9_unbounded_ends_are_refused_and_collapsed_axes_are_kept() {
    // An infinite min with a finite max: not a point of ours.
    assert_eq!(
        F3::new(f64::NEG_INFINITY, 0.0, 0.0),
        Err(NumError::NotFinite)
    );
    assert_eq!(F3::new(0.0, f64::INFINITY, 0.0), Err(NumError::NotFinite));
    assert_eq!(F3::new(f64::NAN, 0.0, 1.0), Err(NumError::NotFinite));
    // The widest box a double allows: moving it so a bound overflows is a refusal, not infinity.
    let wide = FBox3::new(f3(-f64::MAX, 0.0, 0.0), f3(f64::MAX, 1.0, 1.0)).unwrap();
    let doubled = Transform::scale(2.0, 1.0, 1.0).unwrap();
    assert!(wide.transformed(&doubled).is_err());
    let moved = Transform::translation(f3(f64::MAX, 0.0, 0.0));
    assert!(wide.transformed(&moved).is_err());
    // A half-width scale is fine and still holds the whole box.
    let half = Transform::scale(0.5, 1.0, 1.0).unwrap();
    let image = wide.transformed(&half).unwrap();
    assert!(image.contains(f3(f64::MAX / 2.0, 0.5, 0.5)));
    assert!(image.contains(f3(-f64::MAX / 2.0, 0.5, 0.5)));
    // A collapsed x range (the WR-9 trigger): the box is a flat box, and its image is flat too.
    let flat = FBox3::new(f3(3.0, 0.0, 0.0), f3(3.0, 5.0, 5.0)).unwrap();
    let [x, _, _] = flat.axes().unwrap();
    assert_eq!(x.lo(), 3.0);
    assert_eq!(x.hi(), 3.0);
    let image = flat.transformed(&quarter_turn_z()).unwrap();
    let [_, y, _] = image.axes().unwrap();
    assert!(y.lo() <= 3.0 && y.hi() >= 3.0 && y.hi() - y.lo() < 1e-9);
}

/// solidcore plan/upstream-issues/WR-7--smpseudobox-distancetopoint-returns-a-status-as-a-distance.md
/// (b4de2149): the distance to a point of a box in its inside-out initial state returned the
/// status code 1001 as if it were a distance. Ours has no inside-out box: a box whose minimum
/// exceeds its maximum on any axis is refused, grid or float, and the empty intersection of two
/// boxes is `None`, never a box that reports a number.
#[test]
fn wr_7_an_inside_out_box_is_refused_not_a_state_that_reports_distances() {
    assert_eq!(
        Box3::new(p3(1, 0, 0), p3(0, 1, 1)),
        Err(NumError::EmptyInterval)
    );
    assert_eq!(
        Box3::new(p3(0, 0, 1), p3(1, 1, 0)),
        Err(NumError::EmptyInterval)
    );
    assert_eq!(
        FBox3::new(f3(0.0, 0.0, 0.0), f3(-1.0, 1.0, 1.0)),
        Err(NumError::EmptyInterval)
    );
    let a = Box3::new(p3(0, 0, 0), p3(1, 1, 1)).unwrap();
    let b = Box3::new(p3(5, 5, 5), p3(6, 6, 6)).unwrap();
    assert_eq!(a.intersection(&b), None);
    assert!(!a.overlaps(&b));
    let fa = FBox3::from_box3(&a);
    let fb = FBox3::from_box3(&b);
    assert_eq!(fa.intersection(&fb), None);
    // Boxes that only touch share a point: that is a box of zero width, not nothing.
    let c = Box3::new(p3(1, 1, 1), p3(2, 2, 2)).unwrap();
    let touch = a.intersection(&c).unwrap();
    assert_eq!(touch.min(), touch.max());
}
