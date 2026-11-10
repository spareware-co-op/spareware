//! Audit of splitting a NURBS curve at an interior knot of full multiplicity.
//!
//! `geom-nurbs` accepts "interior knots repeated p + 1 times" (a curve that may jump there), and
//! `ParamCurve::split` promises the two parts for any parameter strictly inside the domain. At
//! such a knot the curve is two curves already: the parts are its control points before and after
//! the knot, with no arithmetic at all. The reference is the curve's own evaluation from each
//! side.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    reason = "tests: native arithmetic and indexing, and a failure stops the test"
)]

use geom_curve::{Basis, ParamCurve, Trimmed};
use geom_nurbs::{Basis as Knots, Curve, Limit};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Two quadratic arcs on [0, 1] and [1, 2], the knot 1 repeated three times: the first ends at
/// (2, 0), the second starts at (5, 5).
fn two_arcs() -> Curve<2> {
    let basis = Knots::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0]).unwrap();
    Curve::new(
        basis,
        &[[0.0, 0.0], [1.0, 1.0], [2.0, 0.0], [5.0, 5.0], [6.0, 6.0], [7.0, 5.0]],
    )
    .unwrap()
}

#[test]
fn a_curve_with_a_full_multiplicity_knot_evaluates_from_each_side() {
    let c = two_arcs();
    assert_eq!(ParamCurve::point(&c, 1.0, Limit::FromLeft).unwrap(), [2.0, 0.0]);
    assert_eq!(ParamCurve::point(&c, 1.0, Limit::FromRight).unwrap(), [5.0, 5.0]);
}

#[test]
fn a_curve_splits_at_a_full_multiplicity_knot() {
    let c = two_arcs();
    let parts = ParamCurve::split(&c, 1.0, &mut meter());
    let (left, right) = parts.unwrap_or_else(|e| panic!("split(1) failed: {e:?}"));
    assert_eq!(left.value.domain(), (0.0, 1.0));
    assert_eq!(right.value.domain(), (1.0, 2.0));
    assert_eq!(left.value.point(1.0, Limit::FromLeft).unwrap(), [2.0, 0.0]);
    assert_eq!(right.value.point(1.0, Limit::FromRight).unwrap(), [5.0, 5.0]);
    assert_eq!(left.deviation, 0.0);
    assert_eq!(right.deviation, 0.0);
}

/// A trimmed curve ending at that knot converts by cutting its basis there.
#[test]
fn a_curve_trimmed_at_a_full_multiplicity_knot_converts() {
    let t = Trimmed::new(Basis::Nurbs(two_arcs()), (0.25, 1.0)).unwrap();
    let n = t.to_nurbs(&mut meter());
    let n = n.unwrap_or_else(|e| panic!("to_nurbs of the trimmed curve failed: {e:?}"));
    let p = n.value.curve.point(1.0, Limit::FromLeft).unwrap();
    assert!((p[0] - 2.0).abs() + p[1].abs() <= n.deviation + 1e-12);
}
