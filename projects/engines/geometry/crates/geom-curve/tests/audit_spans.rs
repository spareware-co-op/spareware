//! Audit of bounds over a span of one parameter, where that parameter is a break of the kind: a
//! vertex of a bulge polyline, a joint of a composite, a station of an offset by distances.
//!
//! `ParamCurve::bound` promises "a box holding the curve at every parameter of `span`", and
//! `ParamCurve::derivative_bound` "a bound on the derivative of `order` at every parameter of
//! `span`, from both sides at a break". The NURBS kind keeps both for a span of one parameter (it
//! bounds "both one-sided limits: every piece that touches it"); the tests here hold the other
//! kinds to the same reading, with the curve's own one-sided derivatives as the reference.

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

use geom_curve::{
    Axes, Basis, BulgePolyline, Composite, CurveKind, DerivativeBound, Ellipse, Frame, Line, Offset,
    OffsetLaw, ParamCurve, Station, Trimmed,
};
use geom_nurbs::{Basis as Knots, Bounds, Curve, Limit};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Whether `b` is a box (finite, ordered) that holds `p` within `slack`.
fn holds<const D: usize>(b: &Bounds<D>, p: &[f64; D], slack: f64) -> bool {
    b.lo.iter()
        .zip(&b.hi)
        .zip(p)
        .all(|((l, h), x)| l.is_finite() && h.is_finite() && l - slack <= *x && *x <= h + slack)
}

/// The derivative of `order` at `t` from `side`.
fn derivative<const D: usize>(k: &CurveKind<D>, t: f64, side: Limit, order: usize) -> [f64; D] {
    let mut d = [[0.0; D]; 4];
    k.derivatives(t, side, &mut d).unwrap();
    d[order]
}

/// Both one-sided derivatives of `order` at the break `t` must lie in the bound over (t, t).
fn both_sides_at<const D: usize>(k: &CurveKind<D>, t: f64, order: usize) {
    let bound = match k.derivative_bound((t, t), order, &mut meter()).unwrap() {
        DerivativeBound::Bounded(b) => b,
        DerivativeBound::Unbounded => return,
    };
    for side in [Limit::FromRight, Limit::FromLeft] {
        let v = derivative(k, t, side, order);
        assert!(
            holds(&bound, &v, 1e-9),
            "derivative_bound(({t}, {t}), {order}) = {bound:?} does not hold the derivative {v:?} from {side:?}"
        );
    }
}

/// The square with corners (0, 0), (4, 0), (4, 3) as a bulge polyline with one arc.
fn polyline() -> BulgePolyline<2> {
    BulgePolyline::new(
        vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]],
        vec![0.0, 0.5],
        None,
        false,
    )
    .unwrap()
}

/// A span that is one interior vertex: the box must hold that vertex. The polyline's first
/// segment ends at (4, 0) at parameter 1, exactly.
#[test]
fn a_bulge_polyline_bounds_a_span_that_is_one_vertex() {
    let p = polyline();
    let vertex = p.point(1.0, Limit::FromRight).unwrap();
    assert_eq!(vertex, [4.0, 0.0]);
    let b = p.bound((1.0, 1.0), &mut meter()).unwrap();
    assert!(
        holds(&b, &vertex, 1e-12),
        "bound((1, 1)) = {b:?} does not hold the vertex {vertex:?}"
    );
}

/// The same through a trimmed polyline, which passes the span to its basis.
#[test]
fn a_trimmed_bulge_polyline_bounds_a_span_that_is_one_vertex() {
    let t = Trimmed::new(Basis::Bulge(polyline()), (0.25, 1.75)).unwrap();
    let b = t.bound((1.0, 1.0), &mut meter()).unwrap();
    assert!(
        holds(&b, &[4.0, 0.0], 1e-12),
        "bound((1, 1)) = {b:?} does not hold the vertex (4, 0)"
    );
}

/// The tangent turns at the vertex: from (4, 0) along the first segment to the arc's start
/// tangent. The bound over the vertex alone must hold both, and must be a box.
#[test]
fn a_bulge_polyline_bounds_both_tangents_at_a_vertex() {
    let k = CurveKind::Bulge(polyline());
    for order in 1..=2 {
        both_sides_at(&k, 1.0, order);
    }
}

/// The reference reading: a polyline as a degree 1 NURBS curve bounds both tangents at its
/// corner. (This one passes: it states what the others are held to.)
#[test]
fn the_nurbs_kind_bounds_both_tangents_at_a_corner() {
    let basis = Knots::new(1, vec![0.0, 0.0, 1.0, 2.0, 2.0]).unwrap();
    let c = Curve::<2>::new(basis, &[[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]]).unwrap();
    both_sides_at(&CurveKind::Nurbs(c), 1.0, 1);
}

/// The same corner as a composite of two lines, joined at parameter 4: the tangents are (1, 0)
/// before the joint and (0, 1) after it.
#[test]
fn a_composite_bounds_both_tangents_at_a_joint() {
    let a = Line::new([0.0, 0.0], [1.0, 0.0], (0.0, 4.0)).unwrap();
    let b = Line::new([4.0, 0.0], [0.0, 1.0], (0.0, 3.0)).unwrap();
    let c = Composite::new(vec![
        Trimmed::whole(Basis::Line(a)).unwrap(),
        Trimmed::whole(Basis::Line(b)).unwrap(),
    ])
    .unwrap();
    assert_eq!(c.starts(), &[0.0, 4.0, 7.0]);
    let k = CurveKind::Composite(c);
    assert_eq!(derivative(&k, 4.0, Limit::FromLeft, 1), [1.0, 0.0]);
    assert_eq!(derivative(&k, 4.0, Limit::FromRight, 1), [0.0, 1.0]);
    both_sides_at(&k, 4.0, 1);
}

/// An offset whose lateral distance changes slope at a station: its tangent jumps there, and the
/// bound over the station alone must hold the tangent from each side.
#[test]
fn an_offset_by_distances_bounds_both_tangents_at_a_station() {
    let e = Ellipse::new(
        [1.0, 2.0],
        Axes::Frame {
            frame: Frame::standard().unwrap(),
            a: 6.0,
            b: 3.0,
        },
        (0.0, 1.25),
    )
    .unwrap();
    let o = Offset::new(
        Trimmed::whole(Basis::Ellipse(e)).unwrap(),
        OffsetLaw::Distances(vec![
            Station {
                at: 0.25,
                lateral: 0.1,
                vertical: 0.0,
            },
            Station {
                at: 0.5,
                lateral: -0.2,
                vertical: 0.0,
            },
            Station {
                at: 1.0,
                lateral: 0.3,
                vertical: 0.0,
            },
        ]),
    )
    .unwrap();
    let k = CurveKind::Offset(o);
    // The tangents differ at the station by the jump of the lateral slope, about 1.2 here.
    let (l, r) = (
        derivative(&k, 0.5, Limit::FromLeft, 1),
        derivative(&k, 0.5, Limit::FromRight, 1),
    );
    assert!((l[0] - r[0]).abs() + (l[1] - r[1]).abs() > 0.5);
    both_sides_at(&k, 0.5, 1);
}
