//! Audit of the NURBS curve kind on unclamped knot vectors.
//!
//! `CurveKind::Nurbs` is documented as "a NURBS curve, rational or not, clamped or not", and
//! `geom-nurbs` accepts unclamped knots "so periodic curves can be stored as they are". The kind
//! contract asks every kind for certified enclosures and bounds; the NURBS kind's header promises
//! them ("every enclosure and bound holds the exact value") and names one refusal, a parameter or
//! span outside the domain. The reference here is the curve's own evaluation in doubles, which
//! works on unclamped knots.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "tests: native arithmetic and indexing, the formulas' names, and a failure stops the test"
)]

use geom_curve::{Basis, CurveKind, DerivativeBound, ParamCurve, Trimmed};
use geom_nurbs::{Basis as Knots, Curve, Limit};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A closed uniform cubic B-spline through a square's corners, stored periodically: uniform
/// knots 0, 1, …, 10 and the first three control points repeated at the end. Its domain is [3, 7].
fn periodic() -> Curve<2> {
    let knots: Vec<f64> = (0..=10).map(f64::from).collect();
    let basis = Knots::new(3, knots).unwrap();
    let corners = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
    let pts: Vec<[f64; 2]> = (0..7).map(|i| corners[i % 4]).collect();
    Curve::new(basis, &pts).unwrap()
}

#[test]
fn an_unclamped_nurbs_curve_evaluates_in_doubles() {
    let c = periodic();
    assert_eq!(ParamCurve::domain(&c), (3.0, 7.0));
    // A uniform cubic B-spline's point at a knot is (P0 + 4·P1 + P2)/6.
    let p = ParamCurve::point(&c, 3.0, Limit::FromRight).unwrap();
    assert!((p[0] - 20.0 / 6.0).abs() < 1e-12 && (p[1] - 4.0 / 6.0).abs() < 1e-12);
}

#[test]
fn an_unclamped_nurbs_curve_is_enclosed() {
    let c = periodic();
    let p = ParamCurve::point(&c, 4.5, Limit::FromRight).unwrap();
    let e = ParamCurve::enclose(&c, 4.5, Limit::FromRight, 0, &mut meter());
    let e = e.unwrap_or_else(|err| panic!("enclose(4.5) of an unclamped curve failed: {err:?}"));
    for (x, i) in p.iter().zip(&e[0]) {
        assert!(i.lo() - 1e-12 <= *x && *x <= i.hi() + 1e-12);
    }
}

#[test]
fn an_unclamped_nurbs_curve_is_bounded() {
    let c = periodic();
    let b = ParamCurve::bound(&c, (3.5, 6.0), &mut meter());
    let b = b.unwrap_or_else(|err| panic!("bound((3.5, 6)) of an unclamped curve failed: {err:?}"));
    for i in 0..=20 {
        let t = 3.5 + 2.5 * f64::from(i) / 20.0;
        let p = ParamCurve::point(&c, t, Limit::FromRight).unwrap();
        for d in 0..2 {
            assert!(b.lo[d] - 1e-12 <= p[d] && p[d] <= b.hi[d] + 1e-12);
        }
    }
    let d = ParamCurve::derivative_bound(&c, (3.5, 6.0), 1, &mut meter());
    assert!(
        matches!(d, Ok(DerivativeBound::Bounded(_))),
        "derivative_bound of an unclamped curve: {d:?}"
    );
}

/// The kinds that hold curves take any leaf: a trimmed periodic curve must convert to NURBS.
#[test]
fn a_trimmed_unclamped_nurbs_curve_converts() {
    let t = Trimmed::new(Basis::Nurbs(periodic()), (3.5, 6.0)).unwrap();
    let k = CurveKind::Trimmed(t);
    let n = k.to_nurbs(&mut meter());
    let n = n.unwrap_or_else(|err| panic!("to_nurbs of a trimmed unclamped curve failed: {err:?}"));
    let p = k.point(5.0, Limit::FromRight).unwrap();
    let q = n.value.curve.point(5.0, Limit::FromRight).unwrap();
    assert!((p[0] - q[0]).abs() + (p[1] - q[1]).abs() <= 2.0 * n.deviation + 1e-12);
}
