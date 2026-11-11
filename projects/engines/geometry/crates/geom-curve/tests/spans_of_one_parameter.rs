//! Bounds over a span of one parameter (lo = hi), at a break of the kind and away from one: a
//! vertex of a bulge polyline, a joint of a composite, a station of an offset by distances, a
//! knot of a NURBS curve, and a trim, segment or offset that ends on a break of the curve under
//! it.
//!
//! `ParamCurve`'s rule for spans: a span of one parameter is that parameter from both sides, so
//! at a break the bound holds both one-sided values, at an end of the domain the one side there
//! is, and a bound is always a box (finite, ordered). The tests hold each kind to it with the
//! curve's own one-sided points and derivatives as the reference, and hold the kind contract to
//! catching a kind that breaks it. The first seven are those of an independent audit of the
//! merged kernel (issue 750), six of which failed: a bulge polyline's bound over one vertex was
//! an inverted box, and a composite's and an offset's derivative bound over one break held one
//! side only.

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

use geom_curve::contract::check;
use geom_curve::{
    Axes, Basis, BulgePolyline, Composite, CurveError, CurveKind, DerivativeBound, Ellipse, Frame,
    Law, Line, NurbsForm, Offset, OffsetLaw, ParamCurve, Station, Trimmed,
};
use geom_math::{Axis, GridTransform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Basis as Knots, Bounds, Changed, Curve, Limit};
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

/// The polyline (0, 0), (4, 0), (4, 3), (0, 3) of three straight segments.
fn three_segments() -> BulgePolyline<2> {
    BulgePolyline::new(
        vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
        vec![0.0, 0.0, 0.0],
        None,
        false,
    )
    .unwrap()
}

/// The point from `side`.
fn point_from<const D: usize>(k: &CurveKind<D>, t: f64, side: Limit) -> [f64; D] {
    derivative(k, t, side, 0)
}

/// A trim that ends on a vertex of its polyline: the bound over that end is a box that holds
/// the vertex and the tangent of the segment inside the trim.
#[test]
fn a_trim_that_ends_on_a_vertex_bounds_its_own_end() {
    let t = Trimmed::new(Basis::Bulge(three_segments()), (0.5, 2.0)).unwrap();
    let b = t.bound((2.0, 2.0), &mut meter()).unwrap();
    assert!(holds(&b, &[4.0, 3.0], 1e-12), "{b:?}");
    let k = CurveKind::Trimmed(t);
    let DerivativeBound::Bounded(d) = k.derivative_bound((2.0, 2.0), 1, &mut meter()).unwrap()
    else {
        panic!("a polyline's tangent is bounded");
    };
    assert!(
        holds(&d, &derivative(&k, 2.0, Limit::FromLeft, 1), 1e-12),
        "{d:?}"
    );
}

/// Two trims of one polyline joined at its vertex: the segment before the joint ends on a break
/// of its own curve, and the bound over the joint must hold that segment's tangent from inside
/// it, (4, 0), not the arc's beyond its end.
#[test]
fn a_composite_bounds_a_joint_where_a_segment_ends_on_its_curves_vertex() {
    let c = Composite::new(vec![
        Trimmed::new(Basis::Bulge(polyline()), (0.0, 1.0)).unwrap(),
        Trimmed::new(Basis::Bulge(polyline()), (1.0, 2.0)).unwrap(),
    ])
    .unwrap();
    assert_eq!(c.starts(), &[0.0, 1.0, 2.0]);
    let k = CurveKind::Composite(c);
    assert_eq!(derivative(&k, 1.0, Limit::FromLeft, 1), [4.0, 0.0]);
    for order in 1..=2 {
        both_sides_at(&k, 1.0, order);
    }
}

/// A segment that starts or ends on a vertex of its polyline is read from inside the segment:
/// the bound over the joint holds the two tangents that meet there and not the polyline's
/// tangent from beyond the trim, which is no part of the composite.
#[test]
fn a_composite_reads_a_segments_end_from_inside_the_segment() {
    // The polyline's first segment, then a line: the joint is the polyline's vertex 1, and
    // beyond the trim the polyline turns to (0, 3).
    let line = Line::new([4.0, 0.0], [1.0, 1.0], (0.0, 2.0)).unwrap();
    let ends_on_it = CurveKind::Composite(
        Composite::new(vec![
            Trimmed::new(Basis::Bulge(three_segments()), (0.0, 1.0)).unwrap(),
            Trimmed::whole(Basis::Line(line)).unwrap(),
        ])
        .unwrap(),
    );
    // A line, then the polyline's second segment: before the trim the polyline runs along
    // (4, 0).
    let line = Line::new([4.0, -2.0], [0.0, 1.0], (0.0, 2.0)).unwrap();
    let starts_on_it = CurveKind::Composite(
        Composite::new(vec![
            Trimmed::whole(Basis::Line(line)).unwrap(),
            Trimmed::new(Basis::Bulge(three_segments()), (1.0, 2.0)).unwrap(),
        ])
        .unwrap(),
    );
    for (k, joint, beyond) in [
        (ends_on_it, 1.0, [0.0, 3.0]),
        (starts_on_it, 2.0, [4.0, 0.0]),
    ] {
        both_sides_at(&k, joint, 1);
        let DerivativeBound::Bounded(b) =
            k.derivative_bound((joint, joint), 1, &mut meter()).unwrap()
        else {
            panic!("lines have bounded tangents");
        };
        assert!(!holds(&b, &beyond, 1e-9), "{b:?} holds {beyond:?}");
    }
}

/// A whole polyline as one segment of a composite: the polyline's vertex is a break strictly
/// inside the segment, the composite lists it, and the bound over it holds both tangents.
#[test]
fn a_composite_bounds_a_vertex_inside_a_segment() {
    let after = Line::new([4.0, 3.0], [1.0, 0.0], (0.0, 2.0)).unwrap();
    let c = Composite::new(vec![
        Trimmed::whole(Basis::Bulge(polyline())).unwrap(),
        Trimmed::whole(Basis::Line(after)).unwrap(),
    ])
    .unwrap();
    assert_eq!(ParamCurve::breaks(&c), vec![0.0, 1.0, 2.0, 4.0]);
    let k = CurveKind::Composite(c);
    let (l, r) = (
        derivative(&k, 1.0, Limit::FromLeft, 1),
        derivative(&k, 1.0, Limit::FromRight, 1),
    );
    assert!((l[0] - r[0]).abs() + (l[1] - r[1]).abs() > 1.0);
    for order in 1..=2 {
        both_sides_at(&k, 1.0, order);
    }
    let b = k.bound((1.0, 1.0), &mut meter()).unwrap();
    assert!(holds(&b, &[4.0, 0.0], 1e-12), "{b:?}");
}

/// A joint with a gap, as a transform or a stored composite may leave one: the curve has two
/// points there, half a unit apart, and the bound over the joint holds both.
#[test]
fn a_composite_bounds_both_ends_of_a_joint_with_a_gap() {
    let a = Line::new([0.0, 0.0], [1.0, 0.0], (0.0, 4.0)).unwrap();
    let b = Line::new([4.0, 0.5], [0.0, 1.0], (0.0, 3.0)).unwrap();
    let c = Composite::from_starts(
        vec![
            Trimmed::whole(Basis::Line(a)).unwrap(),
            Trimmed::whole(Basis::Line(b)).unwrap(),
        ],
        vec![0.0, 4.0, 7.0],
    )
    .unwrap();
    assert!(c.gaps()[0] >= 0.5);
    let bound = c.bound((4.0, 4.0), &mut meter()).unwrap();
    assert!(holds(&bound, &[4.0, 0.0], 1e-12), "{bound:?}");
    assert!(holds(&bound, &[4.0, 0.5], 1e-12), "{bound:?}");
}

/// Nothing changes where a joint has no gap: its two ends are the same exact point, and the
/// bound over the joint is the box of the segment after it there, bit for bit, as it was.
#[test]
fn a_joint_without_a_gap_keeps_its_one_point() {
    let a = Line::new([0.0, 0.0], [1.0, 0.0], (0.0, 4.0)).unwrap();
    let b = Line::new([4.0, 0.0], [0.0, 1.0], (0.0, 3.0)).unwrap();
    let after = Trimmed::whole(Basis::Line(b)).unwrap();
    let c = Composite::new(vec![Trimmed::whole(Basis::Line(a)).unwrap(), after.clone()]).unwrap();
    assert_eq!(c.gaps(), &[0.0]);
    assert_eq!(
        c.bound((4.0, 4.0), &mut meter()).unwrap(),
        after.bound((0.0, 0.0), &mut meter()).unwrap()
    );
}

/// The ellipse of the audit's offset test.
fn ellipse() -> Trimmed<2> {
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
    Trimmed::whole(Basis::Ellipse(e)).unwrap()
}

/// An offset of the ellipse by distances at `stations` (parameter, lateral offset).
fn by_distances(stations: &[(f64, f64)]) -> Offset<2> {
    by_distances_from(ellipse(), stations)
}

/// An offset of `basis` by distances at `stations` (parameter, lateral offset).
fn by_distances_from(basis: Trimmed<2>, stations: &[(f64, f64)]) -> Offset<2> {
    let stations = stations
        .iter()
        .map(|&(at, lateral)| Station {
            at,
            lateral,
            vertical: 0.0,
        })
        .collect();
    Offset::new(basis, OffsetLaw::Distances(stations)).unwrap()
}

/// A station on an end of the domain is not a break of the curve: there is one side there, and
/// the bound over that end holds its derivative and not the one from beyond the end, where the
/// lateral offset is held constant (the offset by the station's distance alone).
#[test]
fn a_station_on_an_end_of_the_domain_is_bounded_from_inside() {
    let o = CurveKind::Offset(by_distances(&[(0.0, 0.1), (0.5, -0.2), (1.25, 0.3)]));
    for (end, side, lateral) in [(0.0, Limit::FromRight, 0.1), (1.25, Limit::FromLeft, 0.3)] {
        let DerivativeBound::Bounded(b) = o.derivative_bound((end, end), 1, &mut meter()).unwrap()
        else {
            panic!("the offset's tangent is bounded at {end}");
        };
        assert!(holds(&b, &derivative(&o, end, side, 1), 1e-9), "{b:?}");
        let constant = CurveKind::Offset(by_distances(&[(0.5, lateral)]));
        let beyond = derivative(&constant, end, side, 1);
        assert!(!holds(&b, &beyond, 1e-9), "{b:?} holds {beyond:?}");
    }
}

/// The lateral offset itself does not jump at a station, so the points over one station keep
/// the one piece they had: the box is the one an offset gives whose first station is that one,
/// where there is no piece before it to add.
#[test]
fn the_points_over_one_station_keep_their_one_piece() {
    let o = by_distances(&[(0.25, 0.1), (0.5, -0.2), (1.0, 0.3)]);
    let from_here = by_distances(&[(0.5, -0.2), (1.0, 0.3)]);
    let b = o.bound((0.5, 0.5), &mut meter()).unwrap();
    assert_eq!(b, from_here.bound((0.5, 0.5), &mut meter()).unwrap());
    let k = CurveKind::Offset(o);
    assert!(holds(&b, &point_from(&k, 0.5, Limit::FromRight), 1e-9));
    assert!(holds(&b, &point_from(&k, 0.5, Limit::FromLeft), 1e-9));
}

/// The same on an offset of the x axis, where the box's y is the lateral offset's own enclosure
/// and nothing of the basis rounds over it: a second piece, read from the left through the
/// stations before this one, would widen that enclosure by a rounding, and the box with it.
#[test]
fn the_points_over_one_station_of_a_line_keep_their_one_piece() {
    let axis = || {
        let l = Line::new([0.0, 0.0], [1.0, 0.0], (0.0, 1.25)).unwrap();
        Trimmed::whole(Basis::Line(l)).unwrap()
    };
    let o = by_distances_from(axis(), &[(0.25, 0.1), (0.5, -0.2), (1.0, 0.3)]);
    let from_here = by_distances_from(axis(), &[(0.5, -0.2), (1.0, 0.3)]);
    let b = o.bound((0.5, 0.5), &mut meter()).unwrap();
    assert_eq!(b, from_here.bound((0.5, 0.5), &mut meter()).unwrap());
    let k = CurveKind::Offset(o);
    assert!(holds(&b, &point_from(&k, 0.5, Limit::FromRight), 1e-9));
    assert!(holds(&b, &point_from(&k, 0.5, Limit::FromLeft), 1e-9));
}

/// The offset of a polyline parts at a vertex: the normal turns there, so the offset has two
/// points, (4, 0.5) and (3.5, 0) here, and the bound over the vertex holds both.
#[test]
fn an_offset_of_a_polyline_bounds_both_points_at_a_vertex() {
    let basis = Trimmed::whole(Basis::Bulge(three_segments())).unwrap();
    let o = Offset::new(basis, OffsetLaw::Planar { distance: 0.5 }).unwrap();
    let k = CurveKind::Offset(o);
    let (l, r) = (
        point_from(&k, 1.0, Limit::FromLeft),
        point_from(&k, 1.0, Limit::FromRight),
    );
    assert!((l[0] - 4.0).abs() + (l[1] - 0.5).abs() < 1e-12, "{l:?}");
    assert!((r[0] - 3.5).abs() + r[1].abs() < 1e-12, "{r:?}");
    let b = k.bound((1.0, 1.0), &mut meter()).unwrap();
    assert!(holds(&b, &l, 1e-9) && holds(&b, &r, 1e-9), "{b:?}");
    both_sides_at(&k, 1.0, 1);
}

/// A NURBS curve parts at a knot repeated degree + 1 times inside its domain: two arcs that do
/// not meet. The bound over that knot holds the end of the one and the start of the other.
#[test]
fn a_nurbs_curve_bounds_both_points_at_a_knot_that_parts_it() {
    let basis = Knots::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0]).unwrap();
    let c = Curve::<2>::new(
        basis,
        &[
            [0.0, 0.0],
            [1.0, 1.0],
            [2.0, 0.0],
            [5.0, 5.0],
            [6.0, 6.0],
            [7.0, 5.0],
        ],
    )
    .unwrap();
    let k = CurveKind::Nurbs(c);
    assert_eq!(point_from(&k, 1.0, Limit::FromLeft), [2.0, 0.0]);
    assert_eq!(point_from(&k, 1.0, Limit::FromRight), [5.0, 5.0]);
    let b = k.bound((1.0, 1.0), &mut meter()).unwrap();
    assert!(
        holds(&b, &[2.0, 0.0], 1e-12) && holds(&b, &[5.0, 5.0], 1e-12),
        "{b:?}"
    );
    both_sides_at(&k, 1.0, 1);
}

/// Nothing changes at a knot that does not part the curve: the bound over it is the box of the
/// point from the right, bit for bit, as it was. Here a knot repeated degree times, a corner the
/// curve passes through.
#[test]
fn a_nurbs_curve_keeps_its_one_point_at_a_knot_that_does_not_part_it() {
    let basis = Knots::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 3.0, 3.0, 3.0]).unwrap();
    let c = Curve::<2>::new_rational(
        basis,
        &[[0.1, 0.3], [1.7, 1.1], [2.3, 0.7], [5.9, 5.3], [6.1, 4.7]],
        &[1.0, 0.7, 1.3, 0.9, 1.1],
    )
    .unwrap();
    let e = c.enclose(1.0, Limit::FromRight, 0, &mut meter()).unwrap();
    let b = ParamCurve::bound(&c, (1.0, 1.0), &mut meter()).unwrap();
    assert_eq!(b.lo, e[0].map(Interval::lo));
    assert_eq!(b.hi, e[0].map(Interval::hi));
}

/// What a kind did wrong over a span of one parameter before issue 750 was fixed.
#[derive(Clone, Copy, PartialEq)]
enum Fault {
    /// Nothing: the kind as it is.
    None,
    /// The bound on the points is the empty box, +∞ to −∞.
    Inverted,
    /// The derivative bound is the derivative from the right alone.
    OneSided,
}

/// A kind with one of the old faults put back, over spans of one parameter only.
struct Faulty {
    inner: CurveKind<2>,
    fault: Fault,
}

impl ParamCurve<2> for Faulty {
    fn domain(&self) -> (f64, f64) {
        self.inner.domain()
    }
    fn point(&self, t: f64, side: Limit) -> Result<[f64; 2], CurveError> {
        self.inner.point(t, side)
    }
    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; 2]]) -> Result<(), CurveError> {
        self.inner.derivatives(t, side, out)
    }
    fn enclose(
        &self,
        t: f64,
        side: Limit,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 2]>, CurveError> {
        self.inner.enclose(t, side, order, meter)
    }
    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<2>, CurveError> {
        if self.fault == Fault::Inverted && span.0 == span.1 {
            return Ok(Bounds {
                lo: [f64::INFINITY; 2],
                hi: [f64::NEG_INFINITY; 2],
            });
        }
        self.inner.bound(span, meter)
    }
    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<2>, CurveError> {
        if self.fault == Fault::OneSided && span.0 == span.1 {
            let e = self.inner.enclose(span.0, Limit::FromRight, order, meter)?;
            return Ok(DerivativeBound::Bounded(Bounds {
                lo: e[order].map(Interval::lo),
                hi: e[order].map(Interval::hi),
            }));
        }
        self.inner.derivative_bound(span, order, meter)
    }
    fn breaks(&self) -> Vec<f64> {
        self.inner.breaks()
    }
    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<2>>, CurveError> {
        self.inner.to_nurbs(meter)
    }
    fn split(
        &self,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<2>>, Changed<CurveKind<2>>), CurveError> {
        self.inner.split(t, meter)
    }
    fn reversed(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<2>>, CurveError> {
        self.inner.reversed(meter)
    }
    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<2>>, CurveError> {
        self.inner.transformed(transform, meter)
    }
}

/// A quarter turn in the plane, for the contract's transform law.
fn quarter() -> TypedTransform {
    TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1))
}

/// The two lines of the audit's composite, meeting at a right angle at parameter 4.
fn corner() -> CurveKind<2> {
    let a = Line::new([0.0, 0.0], [1.0, 0.0], (0.0, 4.0)).unwrap();
    let b = Line::new([4.0, 0.0], [0.0, 1.0], (0.0, 3.0)).unwrap();
    CurveKind::Composite(
        Composite::new(vec![
            Trimmed::whole(Basis::Line(a)).unwrap(),
            Trimmed::whole(Basis::Line(b)).unwrap(),
        ])
        .unwrap(),
    )
}

/// The kind contract visits every break a kind lists, so the kinds of this file pass it at
/// parameters that name no break, and each of the two old faults is caught by its law at the
/// break, a parameter the test never gave.
#[test]
fn the_contract_holds_every_kind_to_the_rule_at_its_breaks() {
    let kinds: Vec<(&str, CurveKind<2>, f64, [f64; 2])> = vec![
        ("polyline", CurveKind::Bulge(polyline()), 1.0, [0.25, 1.75]),
        ("composite", corner(), 4.0, [1.0, 6.0]),
        (
            "offset by distances",
            CurveKind::Offset(by_distances(&[(0.25, 0.1), (0.5, -0.2), (1.0, 0.3)])),
            0.25,
            [0.125, 0.75],
        ),
        // Stations on both ends of the domain: an end has one side, and asking it from outside
        // would ask for the slope beyond the end, which the bound rightly leaves out.
        (
            "offset by distances to both ends",
            CurveKind::Offset(by_distances(&[(0.0, 0.1), (0.5, -0.2), (1.25, 0.3)])),
            0.5,
            [0.25, 0.75],
        ),
        (
            "trimmed polyline",
            CurveKind::Trimmed(Trimmed::new(Basis::Bulge(three_segments()), (0.125, 2.0)).unwrap()),
            1.0,
            [0.25, 1.75],
        ),
    ];
    for (name, kind, at, smooth) in kinds {
        assert!(kind.breaks().contains(&at), "{name} lists its break");
        let good = Faulty {
            inner: kind.clone(),
            fault: Fault::None,
        };
        check(&good, &smooth, &quarter(), &mut meter()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let inverted = Faulty {
            inner: kind.clone(),
            fault: Fault::Inverted,
        };
        let e = check(&inverted, &[], &quarter(), &mut meter()).unwrap_err();
        assert_eq!(e.law, Law::BoundHoldsPoints, "{name}: {e}");
        let one_sided = Faulty {
            inner: kind,
            fault: Fault::OneSided,
        };
        let e = check(&one_sided, &[], &quarter(), &mut meter()).unwrap_err();
        assert_eq!(
            (e.law, e.at),
            (Law::DerivativeBoundHoldsDerivatives, at),
            "{name}: {e}"
        );
    }
}

/// Away from a break the rule asks only for a box around the one value: a smooth kind passes
/// the contract over spans of one parameter, and an inverted box is caught there too, at the
/// first parameter given.
#[test]
fn the_contract_asks_for_a_box_at_every_parameter() {
    let kind = CurveKind::Trimmed(ellipse());
    assert_eq!(kind.breaks(), Vec::<f64>::new());
    let params = [0.0, 0.5, 1.25];
    let good = Faulty {
        inner: kind.clone(),
        fault: Fault::None,
    };
    check(&good, &params, &quarter(), &mut meter()).unwrap();
    let inverted = Faulty {
        inner: kind,
        fault: Fault::Inverted,
    };
    let e = check(&inverted, &params, &quarter(), &mut meter()).unwrap_err();
    assert_eq!((e.law, e.at), (Law::BoundHoldsPoints, 0.0), "{e}");
}
