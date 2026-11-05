//! Attack tests from SMLib's confirmed defects, batch 1: `geom-curve`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: the right box, point or kind, from an independent
//! formula, or a defined refusal, and never a panic, a hang, a NaN or a wrong value.

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
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::manual_midpoint,
    clippy::type_complexity,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::needless_range_loop,
    clippy::range_minus_one,
    reason = "tests: native arithmetic, casts and indexing, platform maths on an independent oracle (the formula as it is written), prose that names defect files, and a failure stops the test"
)]

use std::f64::consts::PI;

use geom_curve::{Axes, Basis, Circle, CurveError, CurveKind, Ellipse, Frame, ParamCurve};
use geom_math::{Axis, GridTransform, TypedTransform};
use geom_nurbs::{Curve, Limit};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 36))
}

const RIGHT: Limit = Limit::FromRight;

fn near(a: [f64; 2], b: [f64; 2], tol: f64) -> bool {
    (a[0] - b[0]).abs() <= tol && (a[1] - b[1]).abs() <= tol
}

/// solidcore plan/upstream-issues/WR-474--smcurve-calculatetightboundingbox-adds-knots-outside-the-interval.md
/// (b4de2149): the tight box of part of a curve took in the knots outside the queried interval,
/// so a sphere cap's seam (30 degrees of a quarter circle, interval [0, 0.341] of a NURBS with
/// knots {0, 1}) had the box of the whole quarter. Ours is asked the same: a quarter circle of
/// radius 50 as a circle kind and as the rational NURBS the kind converts to, over the first 30
/// degrees. The box must hold the arc there and be far from the quarter's other end.
#[test]
fn wr_474_the_box_of_part_of_an_arc_does_not_take_in_the_rest_of_the_arc() {
    let r = 50.0;
    let arc = Circle::<2>::new([0.0, 0.0], r, Frame::standard().unwrap(), (0.0, 0.5)).unwrap();
    // 30 degrees is a sixth of a half-turn: x from 50 cos 30 = 43.30 to 50, y from 0 to 25.
    let b = arc.bound((0.0, 1.0 / 6.0), &mut meter()).unwrap();
    let c30 = r * (PI / 6.0).cos();
    assert!(b.lo[0] <= c30 && b.lo[0] > c30 - 1e-6, "{b:?}");
    assert!(b.hi[0] >= r && b.hi[0] < r + 1e-6);
    assert!(b.lo[1] <= 0.0 && b.lo[1] > -1e-6);
    assert!(b.hi[1] >= 25.0 && b.hi[1] < 25.0 + 1e-6);
    // The whole arc's box still reaches the far end, (0, 50).
    let all = arc.bound((0.0, 0.5), &mut meter()).unwrap();
    assert!(all.lo[0] <= 0.0 && all.hi[1] >= 50.0);

    // The same arc as the rational NURBS of the conversion, a span of its own parameter.
    let nurbs = arc.to_nurbs(&mut meter()).unwrap();
    let curve: Curve<2> = nurbs.value.curve;
    let leaf = Basis::Nurbs(curve.clone());
    let (lo, hi) = leaf.domain();
    let upto = lo + 0.341 * (hi - lo);
    let part = leaf.bound((lo, upto), &mut meter()).unwrap();
    // It holds every sampled point of the span.
    let mut ymax = f64::MIN;
    for k in 0..=200 {
        let u = lo + (upto - lo) * f64::from(k) / 200.0;
        let p = curve.point(u, RIGHT).unwrap();
        ymax = ymax.max(p[1]);
        assert!(part.lo[0] <= p[0] && p[0] <= part.hi[0], "{u}");
        assert!(part.lo[1] <= p[1] && p[1] <= part.hi[1], "{u}");
    }
    // And it is nowhere near the far end of the quarter: its top is within a fifth of the arc's
    // own highest point over the span (the whole quarter's top is 50).
    assert!(
        part.hi[1] <= ymax + 0.2 * r,
        "box top {} over an arc top of {ymax}",
        part.hi[1]
    );
    assert!(part.hi[1] < 0.8 * r);
    assert!(part.lo[0] > 0.5 * r);
}

/// solidcore plan/upstream-issues/HANG-23--polar-set-up-of-a-circle-with-a-far-step-interval-shifts-one.md
/// (b4de2149): the polar set-up of a circle whose parameter interval lies far from zero moved one
/// turn per iteration, and never ended past 2^61 degrees. Ours is given arcs far from zero. A
/// domain is a stated interval of at most one turn, so the far arc is evaluated where it is:
/// at once, at the right point, with a span as wide as the doubles there allow; a domain too
/// wide for the doubles there is refused.
#[test]
fn hang_23_a_circle_arc_far_from_zero_is_evaluated_where_it_is() {
    let r = 7.0;
    let frame = Frame::<2>::standard().unwrap();
    // 1e15 and 2^52 are even whole numbers of half-turns: the point at the start is (r, 0).
    for start in [1e15, 4_503_599_627_370_496.0, 1e9, 3.0e5] {
        let arc = Circle::<2>::new([1.0, -2.0], r, frame, (start, start + 1.0)).unwrap();
        assert_eq!(arc.domain(), (start, start + 1.0));
        let p = arc.point(start, RIGHT).unwrap();
        assert!(near(p, [1.0 + r, -2.0], 1e-12 * r), "{start}: {p:?}");
        // One half-turn on, the point is opposite, (-r, 0) from the center.
        let q = arc.point(start + 1.0, Limit::FromLeft).unwrap();
        assert!(near(q, [1.0 - r, -2.0], 1e-12 * r), "{start}: {q:?}");
        // Every point is at the radius from the center, and the box holds them.
        let b = arc.bound((start, start + 1.0), &mut meter()).unwrap();
        for k in 0..=16 {
            let t = start + f64::from(k) / 16.0;
            let p = arc.point(t, RIGHT).unwrap();
            let d = ((p[0] - 1.0).powi(2) + (p[1] + 2.0).powi(2)).sqrt();
            assert!((d - r).abs() <= 1e-12 * r, "{start} + {k}/16: {d}");
            assert!(b.lo[0] <= p[0] && p[0] <= b.hi[0] && b.lo[1] <= p[1] && p[1] <= b.hi[1]);
        }
        // The NURBS form of the far arc: a conversion with a finite deviation, or a refusal.
        match arc.to_nurbs(&mut meter()) {
            Ok(n) => assert!(n.deviation.is_finite()),
            // Where the doubles are a whole unit apart (2^52) the half-turn cannot be cut in two
            // at a representable parameter: refused, not converted wrongly.
            Err(e) => assert!(
                matches!(
                    e,
                    CurveError::Degenerate | CurveError::SpanOutside | CurveError::NotFinite
                ),
                "{start:e}: {e:?}"
            ),
        }
    }
    // A domain more than one turn wide, or whose ends cannot be told apart, is refused.
    let wide = Circle::<2>::new([0.0, 0.0], 1.0, frame, (1e15, 1e15 + 3.0));
    assert_eq!(wide.err(), Some(CurveError::SpanOutside));
    for far in [9_223_372_036_854_775_807.0, 1e300, f64::MAX] {
        let r = Circle::<2>::new([0.0, 0.0], 1.0, frame, (far, far + 1.0));
        assert_eq!(r.err(), Some(CurveError::SpanOutside), "{far:e}");
        let r = Circle::<2>::new([0.0, 0.0], 1.0, frame, (far, f64::INFINITY));
        assert_eq!(r.err(), Some(CurveError::SpanOutside));
    }
    assert_eq!(
        Circle::<2>::new([0.0, 0.0], 1.0, frame, (f64::NAN, 1.0)).err(),
        Some(CurveError::SpanOutside)
    );
}

/// solidcore plan/upstream-issues/WR-98--smtrimmingtools-mirrorcurve-divides-rational-control-points.md
/// (b4de2149): mirroring a rational curve divided each control point by its weight twice, so
/// the curve was moved and distorted, not mirrored. The input is a rational curve with weights
/// other than 1: the quarter circle of radius 5, and a rational quadratic with uneven weights,
/// mirrored in each axis, and in the line y = x. The mirror image of each point of the curve
/// is the point of the mirrored curve at the same parameter.
#[test]
fn wr_98_mirroring_a_rational_curve_mirrors_it_and_does_not_distort_it() {
    let basis = geom_nurbs::Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let w = 0.5_f64.sqrt();
    let quarter = Curve::<2>::new_rational(
        basis.clone(),
        &[[5.0, 0.0], [5.0, 5.0], [0.0, 5.0]],
        &[1.0, w, 1.0],
    )
    .unwrap();
    let uneven = Curve::<2>::new_rational(
        basis,
        &[[1.0, 2.0], [4.0, 7.0], [9.0, 1.0]],
        &[1.0, 3.5, 0.25],
    )
    .unwrap();
    let mirrors: [(TypedTransform, fn([f64; 2]) -> [f64; 2]); 3] = [
        (TypedTransform::Exact(GridTransform::mirror(Axis::X)), |p| {
            [-p[0], p[1]]
        }),
        (TypedTransform::Exact(GridTransform::mirror(Axis::Y)), |p| {
            [p[0], -p[1]]
        }),
        (
            TypedTransform::Exact(GridTransform::mirror_diagonal(false)),
            |p| [p[1], p[0]],
        ),
    ];
    for curve in [quarter, uneven] {
        let kind = CurveKind::Nurbs(curve.clone());
        for (t, image) in &mirrors {
            let moved = kind.transformed(t, &mut meter()).unwrap();
            for k in 0..=40 {
                let (lo, hi) = curve.basis().domain();
                let u = lo + (hi - lo) * f64::from(k) / 40.0;
                let want = image(curve.point(u, RIGHT).unwrap());
                let got = moved.value.point(u, RIGHT).unwrap();
                let tol = 1e-12 * (1.0 + want[0].abs() + want[1].abs()) + moved.deviation;
                assert!(near(got, want, tol), "u = {u}: {got:?} against {want:?}");
            }
            // Mirroring leaves the weights of the curve alone.
            if let CurveKind::Nurbs(m) = &moved.value {
                assert_eq!(m.weights(), curve.weights());
            }
        }
    }
}

/// solidcore plan/upstream-issues/UB-227--surface-of-revolution-and-extrusion-constructors-evaluate-a.md
/// and WR-159--smvector3d-unitize-turns-a-vector-with-an-overflowing.md (b4de2149): a circle whose
/// axis had a component of +-1e300 had an overflowing squared length, which unitised to a zero
/// vector, so it got no NURBS and its users dereferenced the missing one. The inputs are the
/// defects': a circle (and an ellipse) whose frame normal or reference vector has a component of
/// 1e300 or -1e300. Ours must refuse it, or give the circle: unit axes, each point at the radius
/// from the centre, and a NURBS form.
#[test]
fn ub_227_a_circle_on_an_axis_of_length_1e300_is_refused_or_exact() {
    let r = 3.0;
    let axes = [
        ([1.0, 0.0, 0.0], [0.0, -1e300, 1.0]),
        ([1.0, 0.0, 0.0], [0.0, 1e300, 1.0]),
        ([1e300, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1e300, 1e300, 0.0], [0.0, 0.0, -1e300]),
        ([1.0, 0.0, 0.0], [0.0, 0.0, 1e300]),
        ([f64::MAX, 0.0, 0.0], [0.0, f64::MAX, 0.0]),
    ];
    for (x, n) in axes {
        let frame = match Frame::<3>::new(x, n) {
            Ok(f) => f,
            Err(e) => {
                assert!(
                    matches!(e, CurveError::NotFinite | CurveError::Degenerate),
                    "{x:?} {n:?}: {e:?}"
                );
                continue;
            }
        };
        match Circle::<3>::new([0.0, 0.0, 0.0], r, frame, (0.0, 2.0)) {
            Err(e) => assert!(
                matches!(
                    e,
                    CurveError::NotFinite | CurveError::Degenerate | CurveError::Nurbs(_)
                ),
                "{x:?} {n:?}: {e:?}"
            ),
            Ok(c) => {
                for k in 0..=16 {
                    let t = 2.0 * f64::from(k) / 16.0;
                    let p = c.point(t, RIGHT).unwrap();
                    let d = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
                    assert!((d - r).abs() <= 1e-12 * r, "{x:?} {n:?} t = {t}: {p:?}");
                }
                assert!(c.to_nurbs(&mut meter()).is_ok(), "{x:?} {n:?}");
            }
        }
    }
    // The same through an ellipse's explicit axes: refused, or a curve with finite points.
    let big = Axes::Vectors {
        u: [1e300, 0.0, 0.0],
        v: [0.0, 1e300, 0.0],
    };
    match Ellipse::<3>::new([0.0; 3], big, (0.0, 2.0)) {
        Err(e) => assert!(
            matches!(
                e,
                CurveError::NotFinite | CurveError::Degenerate | CurveError::Nurbs(_)
            ),
            "{e:?}"
        ),
        Ok(e) => {
            let p = e.point(0.5, RIGHT).unwrap();
            assert!(p.iter().all(|x| x.is_finite()));
            assert!((p[1] - 1e300).abs() <= 1e-9 * 1e300);
        }
    }
    let overflow = Axes::Vectors {
        u: [f64::MAX, 0.0, 0.0],
        v: [0.0, f64::MAX, 0.0],
    };
    if let Ok(e) = Ellipse::<3>::new([f64::MAX, 0.0, 0.0], overflow, (0.0, 2.0)) {
        // A point that overflows is refused when asked for, not returned as an infinity.
        match e.point(0.0, RIGHT) {
            Ok(p) => assert!(p.iter().all(|x| x.is_finite())),
            Err(err) => assert_eq!(err, CurveError::NotFinite),
        }
    }
}

/// solidcore plan/upstream-issues/WR-490--ellipse-trim-seam-lines-on-trimmed-domain.md and
/// UB-154--sm-getellipsetrimintervals-reads-the-parameter-of-a-failed.md (b4de2149): the trim
/// intervals of an ellipse dropped the half ellipse whose only crossing with a seam lay in a
/// trimmed-off part (WR-490), and read the result of a failed line and plane intersection for a
/// seam line that is zero long (UB-154). Ours is given elliptical and circular arcs that cross
/// the parameter seam (t = 2 is t = 0): the arc must run through the seam point, be cut there
/// into two arcs that join, and its box must hold the seam point and both sides.
#[test]
fn wr_490_and_ub_154_an_arc_across_the_seam_keeps_both_sides_and_splits_exactly() {
    let c = [2.0, -1.0];
    let ell = |dom: (f64, f64)| {
        Ellipse::<2>::new(
            c,
            Axes::Frame {
                frame: Frame::standard().unwrap(),
                a: 6.0,
                b: 2.0,
            },
            dom,
        )
        .unwrap()
    };
    let circ = |dom: (f64, f64)| Circle::<2>::new(c, 3.0, Frame::standard().unwrap(), dom).unwrap();
    let kinds: Vec<(CurveKind<2>, f64, f64)> = vec![
        (CurveKind::Ellipse(ell((1.75, 2.25))), 6.0, 2.0),
        (CurveKind::Circle(circ((1.75, 2.25))), 3.0, 3.0),
        (CurveKind::Ellipse(ell((1.9, 2.9))), 6.0, 2.0),
        (CurveKind::Circle(circ((-0.25, 0.25))), 3.0, 3.0),
    ];
    for (kind, a, b) in kinds {
        let (lo, hi) = kind.domain();
        let at = |t: f64| [c[0] + a * (PI * t).cos(), c[1] + b * (PI * t).sin()];
        // The seam point, t = 2 (or 0), is on the arc and equal from both sides.
        let seam = if lo < 2.0 && hi > 2.0 { 2.0 } else { 0.0 };
        let (left, right) = (
            kind.point(seam, Limit::FromLeft).unwrap(),
            kind.point(seam, RIGHT).unwrap(),
        );
        assert!(near(left, right, 1e-12));
        assert!(near(left, [c[0] + a, c[1]], 1e-12));
        // Every point is the formula's.
        for k in 0..=32 {
            let t = lo + (hi - lo) * f64::from(k) / 32.0;
            assert!(
                near(kind.point(t, RIGHT).unwrap(), at(t), 1e-12 * (a + b)),
                "t = {t}"
            );
        }
        // The box holds the seam point and the extremes of the arc.
        let bx = kind.bound((lo, hi), &mut meter()).unwrap();
        assert!(bx.lo[0] <= c[0] + a && bx.hi[0] >= c[0] + a);
        for k in 0..=32 {
            let p = at(lo + (hi - lo) * f64::from(k) / 32.0);
            assert!(bx.lo[0] <= p[0] && p[0] <= bx.hi[0] && bx.lo[1] <= p[1] && p[1] <= bx.hi[1]);
        }
        // Cut at the seam: two arcs that meet there, together the whole.
        let (first, second) = kind.split(seam, &mut meter()).unwrap();
        assert_eq!(first.value.domain(), (lo, seam));
        assert_eq!(second.value.domain(), (seam, hi));
        let (e1, e2) = (
            first.value.point(seam, Limit::FromLeft).unwrap(),
            second.value.point(seam, RIGHT).unwrap(),
        );
        assert!(near(e1, e2, 1e-12) && near(e1, left, 1e-12));
        // A split at the end or outside the domain is refused, not turned into an empty arc.
        assert!(kind.split(lo, &mut meter()).is_err());
        assert!(kind.split(hi + 1.0, &mut meter()).is_err());
    }
}
