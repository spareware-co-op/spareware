//! The behaviour every curve kind has (chapter 7): evaluation, derivatives, certified enclosures,
//! conservative bounds on the value and its derivatives over any span, conversion to NURBS with an
//! error, split, reverse and transform; and the closed enum that stores every kind.
//!
//! What it does: [`ParamCurve`] is the trait chapter 12.1 names for this behaviour, implemented by
//! each kind and by [`CurveKind`], the storage enum with one variant per kind, which dispatches to
//! it. Operations that make a new curve (split, reverse, transform) return a [`CurveKind`], because
//! a transform may change a kind (a non-uniform scale turns a circle into an ellipse, chapter 7),
//! wrapped in `geom-nurbs`'s [`Changed`]: the result and a certified bound on how far it lies from
//! the exact result of the operation on the exact input, 0 when it is exact.
//!
//! Guarantees:
//!
//! - **Certified, or refused.** Every bound and enclosure holds the exact value for the curve the
//!   stored data define; every [`Changed`] deviation bounds the distance between the returned curve
//!   and the exact result. A kind that cannot bound a derivative over a span says
//!   [`DerivativeBound::Unbounded`] rather than guess.
//! - **One family for 2D and 3D.** The trait is generic over the dimension D, so planar and
//!   spatial curves share every kind and every rule (chapter 7, principle 8).
//! - **Closed and exhaustive.** [`CurveKind`] lists every kind built; matching on it is exhaustive,
//!   and adding a kind is a new variant (the playbook in `projects/engines/geometry/AGENTS.md`).
//! - **Typed transforms.** A transform is a `geom-math` [`TypedTransform`]: exact, rational or
//!   certified, and the result's deviation accounts for its entries as the variant holds them.
//!
//! Method: the operations of chapter 7 and the trait names proposed in chapter 12.1
//! (`projects/engines/geometry/architecture/12-intersections.md`); the storage-enum-and-behaviour-
//! trait split chapter 7 decides ("Storage uses closed enums ... Behaviour uses traits").
//!
//! Cost: none here; each kind's implementation states its own.
//!
//! Tier: Preview since 2026-10-31.

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit};
use geom_run::WorkMeter;

use crate::bulge::BulgePolyline;
use crate::composite::Composite;
use crate::conic::{Circle, Ellipse, Hyperbola, Parabola};
use crate::error::CurveError;
use crate::helix::Helix;
use crate::intersection::IntersectionCurve;
use crate::involute::Involute;
use crate::line::Line;
use crate::map::NurbsForm;
use crate::offset::Offset;
use crate::on_surface::OnSurface;
use crate::polynomial::Polynomial;
use crate::transition::Transition;
use crate::trimmed::Trimmed;

/// A bound on a derivative over a span: a box, or none where the derivative is not bounded there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DerivativeBound<const D: usize> {
    /// Every value of the derivative over the span lies in this box.
    Bounded(Bounds<D>),
    /// The derivative is not bounded over the span (a cusp of a procedural kind, a singular
    /// point); callers take another route there.
    Unbounded,
}

/// The behaviour of a curve kind of dimension D.
///
/// # Spans
///
/// [`ParamCurve::bound`] and [`ParamCurve::derivative_bound`] take a span (lo, hi) inside the
/// domain [a, b], a ≤ lo ≤ hi ≤ b; lo = hi is a span like any other, the one parameter. A break
/// is a parameter where the curve, or the derivative asked for, has two one-sided values: a
/// vertex of a polyline, a joint of a composite, a station of an offset by distances, a knot of
/// reduced continuity, a break of a curve the kind is built on. Every kind owes the same thing:
///
/// - **A span with lo < hi is the curve restricted to it.** The bound holds every parameter
///   strictly inside from both sides, lo from the right and hi from the left: the one-sided
///   values a walk along the span meets, and not the value beyond an end that is a break.
/// - **A span of one parameter is that parameter from both sides.** At a break the bound holds
///   both one-sided values, because a caller that has narrowed a span to one parameter may have
///   come from either side; at an end of the domain, the one side there is; away from a break
///   the two sides are one value, and the bound is a box around it. So a span of one parameter
///   is not the limit of the spans that end on it: it can hold more than they do.
/// - **A bound is a box.** Every lower end is finite and at most its upper end. A kind that
///   cannot give one returns an error, or [`DerivativeBound::Unbounded`] for a derivative; it
///   never returns an empty or inverted box, and never a box that leaves out a value above.
/// - **A kind built on another curve may hold more at its own ends.** Where a trim, a segment
///   or an offset ends on a break of the curve under it, its bound over that end may also hold
///   that curve's value from beyond the end: a box that holds more is still a bound.
///
/// [`ParamCurve::enclose`], [`ParamCurve::point`] and [`ParamCurve::derivatives`] take one
/// parameter and one side, and give that side's value at a break; at an end of the domain they
/// are asked from the side inside it.
///
/// `geom_curve::contract::check` holds every kind to this at each parameter it is given and at
/// the breaks the kind lists ([`ParamCurve::breaks`]), under the laws
/// [`crate::Law::BoundHoldsPoints`] and [`crate::Law::DerivativeBoundHoldsDerivatives`]. A kind
/// with breaks must list them, or the contract cannot visit them.
pub trait ParamCurve<const D: usize> {
    /// The parameter domain [a, b].
    fn domain(&self) -> (f64, f64);

    /// The point at `t`, in doubles; at a parameter where the curve is not smooth, the limit from
    /// `side`.
    ///
    /// # Errors
    ///
    /// [`CurveError::ParameterOutside`] if `t` is not in the domain; [`CurveError::NotFinite`].
    fn point(&self, t: f64, side: Limit) -> Result<[f64; D], CurveError>;

    /// The points at every parameter of `t`, into flat arrays (chapter 23.6): coordinate k of the
    /// point at `t[i]` goes to `out[k·n + i]`, n = `t.len()` (all x values, then all y values, …),
    /// and `results[i]` says whether it succeeded. Each point is [`ParamCurve::point`]'s at
    /// `t[i]` and `side`, bit for bit; a parameter that fails gets [`ParamCurve::point`]'s error in
    /// `results` and zeros in `out`, and fails no other ([`crate::flat::fill`]).
    ///
    /// # Errors
    ///
    /// [`CurveError::ArrayLength`] unless `results` holds n entries and `out` D·n values; then
    /// nothing is written.
    fn points(
        &self,
        t: &[f64],
        side: Limit,
        out: &mut [f64],
        results: &mut [Result<(), CurveError>],
    ) -> Result<(), CurveError> {
        crate::flat::fill(out, results, t.iter().map(|&ti| self.point(ti, side)))
    }

    /// The point and its derivatives at `t` into `out`: `out[k]` is the k-th derivative, for k up to
    /// `out.len()` − 1.
    ///
    /// # Errors
    ///
    /// As [`ParamCurve::point`].
    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; D]]) -> Result<(), CurveError>;

    /// Enclosures of the exact point and derivatives up to `order` at `t` (item k the k-th
    /// derivative): certified evaluation.
    ///
    /// # Errors
    ///
    /// As [`ParamCurve::point`]; [`CurveError::Stopped`].
    fn enclose(
        &self,
        t: f64,
        side: Limit,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; D]>, CurveError>;

    /// A box holding the curve at every parameter of `span`, inside the domain; over a span of
    /// one parameter, the point from both sides where the curve parts there (the trait's rule
    /// for spans).
    ///
    /// # Errors
    ///
    /// [`CurveError::SpanOutside`]; [`CurveError::NotFinite`]; [`CurveError::Stopped`].
    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError>;

    /// A bound on the derivative of `order` at every parameter of `span`, from both sides at a
    /// break inside it, and from both sides at a break that is the whole span (the trait's rule
    /// for spans).
    ///
    /// # Errors
    ///
    /// As [`ParamCurve::bound`].
    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<D>, CurveError>;

    /// The parameters where the curve may have a break (the trait's rule for spans), in
    /// increasing order, some perhaps repeated or outside the domain; none for a kind that is
    /// smooth everywhere, which is what this gives unless the kind says otherwise. The kind
    /// contract reads them, to hold the kind to the rule at each.
    fn breaks(&self) -> Vec<f64> {
        Vec::new()
    }

    /// The curve as a NURBS curve, with the map from the curve's parameter t to the NURBS
    /// curve's u (the same parameter, or a conic's tangent half-angle map), and a certified bound
    /// on |C(t) − N(u(t))| over the domain (0 when exact).
    ///
    /// # Errors
    ///
    /// [`CurveError::Unbounded`] for a ray or infinite line; as [`ParamCurve::bound`].
    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError>;

    /// The curve split at `t`, strictly inside the domain: the part on [a, t] and the part on
    /// [t, b], each with the same parameter as the curve.
    ///
    /// # Errors
    ///
    /// [`CurveError::ParameterOutside`] unless a < t < b; as [`ParamCurve::bound`].
    fn split(
        &self,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError>;

    /// The curve traversed the other way, on [−b, −a]: the reversed curve at −t is the curve at t.
    ///
    /// # Errors
    ///
    /// As [`ParamCurve::bound`].
    fn reversed(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError>;

    /// The image of the curve under `transform`, with the same parameter, possibly of another
    /// kind. A 2D curve takes only a transform that acts in the plane.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotInPlane`]; [`CurveError::Dimension`] unless D is 2 or 3;
    /// [`CurveError::NotFinite`]; [`CurveError::Stopped`].
    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError>;
}

/// Every curve kind, stored as itself (chapter 7): one variant per kind built. The kinds that hold
/// curves (trimmed, composite, offset, on a surface) hold only leaves, [`crate::basis::Basis`], so
/// no kind contains itself.
#[derive(Clone, Debug, PartialEq)]
pub enum CurveKind<const D: usize> {
    /// A NURBS curve, rational or not, clamped or not.
    Nurbs(geom_nurbs::Curve<D>),
    /// A segment, ray or infinite line.
    Line(Line<D>),
    /// A circle or circular arc.
    Circle(Circle<D>),
    /// An ellipse or elliptical arc, by parameter.
    Ellipse(Ellipse<D>),
    /// One branch of a hyperbola, or an arc of it.
    Hyperbola(Hyperbola<D>),
    /// An arc of a parabola.
    Parabola(Parabola<D>),
    /// A helix (3D).
    Helix(Helix<D>),
    /// A polynomial curve.
    Polynomial(Polynomial<D>),
    /// An involute of a circle, or its affine image.
    Involute(Involute<D>),
    /// A bulge polyline (2D), with optional widths.
    Bulge(BulgePolyline<D>),
    /// A leaf kind on a parameter interval.
    Trimmed(Trimmed<D>),
    /// Trimmed curves joined end to end.
    Composite(Composite<D>),
    /// An offset curve, with its NURBS cache.
    Offset(Offset<D>),
    /// A curve on a surface: its pcurve and 3D cache.
    OnSurface(OnSurface<D>),
    /// A transition curve of an alignment (2D): a clothoid or another spiral, or a part of one.
    Transition(Transition<D>),
    /// A branch where two surfaces meet: its definition, and its 3D curve and pcurves as certified
    /// caches (G175).
    Intersection(IntersectionCurve<D>),
}

impl<const D: usize> ParamCurve<D> for CurveKind<D> {
    fn domain(&self) -> (f64, f64) {
        match self {
            CurveKind::Nurbs(c) => ParamCurve::domain(c),
            CurveKind::Line(c) => ParamCurve::domain(c),
            CurveKind::Circle(c) => ParamCurve::domain(c),
            CurveKind::Ellipse(c) => ParamCurve::domain(c),
            CurveKind::Hyperbola(c) => ParamCurve::domain(c),
            CurveKind::Parabola(c) => ParamCurve::domain(c),
            CurveKind::Helix(c) => ParamCurve::domain(c),
            CurveKind::Polynomial(c) => ParamCurve::domain(c),
            CurveKind::Involute(c) => ParamCurve::domain(c),
            CurveKind::Bulge(c) => ParamCurve::domain(c),
            CurveKind::Trimmed(c) => ParamCurve::domain(c),
            CurveKind::Composite(c) => ParamCurve::domain(c),
            CurveKind::Offset(c) => ParamCurve::domain(c),
            CurveKind::OnSurface(c) => ParamCurve::domain(c),
            CurveKind::Intersection(c) => ParamCurve::domain(c),
            CurveKind::Transition(c) => ParamCurve::domain(c),
        }
    }

    fn point(&self, t: f64, side: Limit) -> Result<[f64; D], CurveError> {
        match self {
            CurveKind::Nurbs(c) => ParamCurve::point(c, t, side),
            CurveKind::Line(c) => ParamCurve::point(c, t, side),
            CurveKind::Circle(c) => ParamCurve::point(c, t, side),
            CurveKind::Ellipse(c) => ParamCurve::point(c, t, side),
            CurveKind::Hyperbola(c) => ParamCurve::point(c, t, side),
            CurveKind::Parabola(c) => ParamCurve::point(c, t, side),
            CurveKind::Helix(c) => ParamCurve::point(c, t, side),
            CurveKind::Polynomial(c) => ParamCurve::point(c, t, side),
            CurveKind::Involute(c) => ParamCurve::point(c, t, side),
            CurveKind::Bulge(c) => ParamCurve::point(c, t, side),
            CurveKind::Trimmed(c) => ParamCurve::point(c, t, side),
            CurveKind::Composite(c) => ParamCurve::point(c, t, side),
            CurveKind::Offset(c) => ParamCurve::point(c, t, side),
            CurveKind::OnSurface(c) => ParamCurve::point(c, t, side),
            CurveKind::Intersection(c) => ParamCurve::point(c, t, side),
            CurveKind::Transition(c) => ParamCurve::point(c, t, side),
        }
    }

    // One dispatch on the kind for the whole call, not one per point (chapter 23.6), so the kind's
    // own `points` runs its own `point` in its loop.
    fn points(
        &self,
        t: &[f64],
        side: Limit,
        out: &mut [f64],
        results: &mut [Result<(), CurveError>],
    ) -> Result<(), CurveError> {
        match self {
            CurveKind::Nurbs(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Line(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Circle(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Ellipse(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Hyperbola(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Parabola(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Helix(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Polynomial(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Involute(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Bulge(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Trimmed(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Composite(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Offset(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::OnSurface(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Intersection(c) => ParamCurve::points(c, t, side, out, results),
            CurveKind::Transition(c) => ParamCurve::points(c, t, side, out, results),
        }
    }

    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; D]]) -> Result<(), CurveError> {
        match self {
            CurveKind::Nurbs(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Line(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Circle(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Ellipse(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Hyperbola(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Parabola(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Helix(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Polynomial(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Involute(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Bulge(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Trimmed(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Composite(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Offset(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::OnSurface(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Intersection(c) => ParamCurve::derivatives(c, t, side, out),
            CurveKind::Transition(c) => ParamCurve::derivatives(c, t, side, out),
        }
    }

    fn enclose(
        &self,
        t: f64,
        side: Limit,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; D]>, CurveError> {
        match self {
            CurveKind::Nurbs(c) => c.enclose(t, side, order, meter),
            CurveKind::Line(c) => c.enclose(t, side, order, meter),
            CurveKind::Circle(c) => c.enclose(t, side, order, meter),
            CurveKind::Ellipse(c) => c.enclose(t, side, order, meter),
            CurveKind::Hyperbola(c) => c.enclose(t, side, order, meter),
            CurveKind::Parabola(c) => c.enclose(t, side, order, meter),
            CurveKind::Helix(c) => c.enclose(t, side, order, meter),
            CurveKind::Polynomial(c) => c.enclose(t, side, order, meter),
            CurveKind::Involute(c) => c.enclose(t, side, order, meter),
            CurveKind::Bulge(c) => c.enclose(t, side, order, meter),
            CurveKind::Trimmed(c) => c.enclose(t, side, order, meter),
            CurveKind::Composite(c) => c.enclose(t, side, order, meter),
            CurveKind::Offset(c) => c.enclose(t, side, order, meter),
            CurveKind::OnSurface(c) => c.enclose(t, side, order, meter),
            CurveKind::Intersection(c) => c.enclose(t, side, order, meter),
            CurveKind::Transition(c) => c.enclose(t, side, order, meter),
        }
    }

    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError> {
        match self {
            CurveKind::Nurbs(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Line(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Circle(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Ellipse(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Hyperbola(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Parabola(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Helix(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Polynomial(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Involute(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Bulge(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Trimmed(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Composite(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Offset(c) => ParamCurve::bound(c, span, meter),
            CurveKind::OnSurface(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Intersection(c) => ParamCurve::bound(c, span, meter),
            CurveKind::Transition(c) => ParamCurve::bound(c, span, meter),
        }
    }

    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<D>, CurveError> {
        match self {
            CurveKind::Nurbs(c) => c.derivative_bound(span, order, meter),
            CurveKind::Line(c) => c.derivative_bound(span, order, meter),
            CurveKind::Circle(c) => c.derivative_bound(span, order, meter),
            CurveKind::Ellipse(c) => c.derivative_bound(span, order, meter),
            CurveKind::Hyperbola(c) => c.derivative_bound(span, order, meter),
            CurveKind::Parabola(c) => c.derivative_bound(span, order, meter),
            CurveKind::Helix(c) => c.derivative_bound(span, order, meter),
            CurveKind::Polynomial(c) => c.derivative_bound(span, order, meter),
            CurveKind::Involute(c) => c.derivative_bound(span, order, meter),
            CurveKind::Bulge(c) => c.derivative_bound(span, order, meter),
            CurveKind::Trimmed(c) => c.derivative_bound(span, order, meter),
            CurveKind::Composite(c) => c.derivative_bound(span, order, meter),
            CurveKind::Offset(c) => c.derivative_bound(span, order, meter),
            CurveKind::OnSurface(c) => c.derivative_bound(span, order, meter),
            CurveKind::Intersection(c) => c.derivative_bound(span, order, meter),
            CurveKind::Transition(c) => c.derivative_bound(span, order, meter),
        }
    }

    fn breaks(&self) -> Vec<f64> {
        match self {
            CurveKind::Nurbs(c) => ParamCurve::breaks(c),
            CurveKind::Line(c) => ParamCurve::breaks(c),
            CurveKind::Circle(c) => ParamCurve::breaks(c),
            CurveKind::Ellipse(c) => ParamCurve::breaks(c),
            CurveKind::Hyperbola(c) => ParamCurve::breaks(c),
            CurveKind::Parabola(c) => ParamCurve::breaks(c),
            CurveKind::Helix(c) => ParamCurve::breaks(c),
            CurveKind::Polynomial(c) => ParamCurve::breaks(c),
            CurveKind::Involute(c) => ParamCurve::breaks(c),
            CurveKind::Bulge(c) => ParamCurve::breaks(c),
            CurveKind::Trimmed(c) => ParamCurve::breaks(c),
            CurveKind::Composite(c) => ParamCurve::breaks(c),
            CurveKind::Offset(c) => ParamCurve::breaks(c),
            CurveKind::OnSurface(c) => ParamCurve::breaks(c),
            CurveKind::Intersection(c) => ParamCurve::breaks(c),
            CurveKind::Transition(c) => ParamCurve::breaks(c),
        }
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        match self {
            CurveKind::Nurbs(c) => c.to_nurbs(meter),
            CurveKind::Line(c) => c.to_nurbs(meter),
            CurveKind::Circle(c) => c.to_nurbs(meter),
            CurveKind::Ellipse(c) => c.to_nurbs(meter),
            CurveKind::Hyperbola(c) => c.to_nurbs(meter),
            CurveKind::Parabola(c) => c.to_nurbs(meter),
            CurveKind::Helix(c) => c.to_nurbs(meter),
            CurveKind::Polynomial(c) => c.to_nurbs(meter),
            CurveKind::Involute(c) => c.to_nurbs(meter),
            CurveKind::Bulge(c) => c.to_nurbs(meter),
            CurveKind::Trimmed(c) => c.to_nurbs(meter),
            CurveKind::Composite(c) => c.to_nurbs(meter),
            CurveKind::Offset(c) => c.to_nurbs(meter),
            CurveKind::OnSurface(c) => c.to_nurbs(meter),
            CurveKind::Intersection(c) => c.to_nurbs(meter),
            CurveKind::Transition(c) => c.to_nurbs(meter),
        }
    }

    fn split(
        &self,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        match self {
            CurveKind::Nurbs(c) => ParamCurve::split(c, t, meter),
            CurveKind::Line(c) => ParamCurve::split(c, t, meter),
            CurveKind::Circle(c) => ParamCurve::split(c, t, meter),
            CurveKind::Ellipse(c) => ParamCurve::split(c, t, meter),
            CurveKind::Hyperbola(c) => ParamCurve::split(c, t, meter),
            CurveKind::Parabola(c) => ParamCurve::split(c, t, meter),
            CurveKind::Helix(c) => ParamCurve::split(c, t, meter),
            CurveKind::Polynomial(c) => ParamCurve::split(c, t, meter),
            CurveKind::Involute(c) => ParamCurve::split(c, t, meter),
            CurveKind::Bulge(c) => ParamCurve::split(c, t, meter),
            CurveKind::Trimmed(c) => ParamCurve::split(c, t, meter),
            CurveKind::Composite(c) => ParamCurve::split(c, t, meter),
            CurveKind::Offset(c) => ParamCurve::split(c, t, meter),
            CurveKind::OnSurface(c) => ParamCurve::split(c, t, meter),
            CurveKind::Intersection(c) => ParamCurve::split(c, t, meter),
            CurveKind::Transition(c) => ParamCurve::split(c, t, meter),
        }
    }

    fn reversed(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        match self {
            CurveKind::Nurbs(c) => c.reversed(meter),
            CurveKind::Line(c) => c.reversed(meter),
            CurveKind::Circle(c) => c.reversed(meter),
            CurveKind::Ellipse(c) => c.reversed(meter),
            CurveKind::Hyperbola(c) => c.reversed(meter),
            CurveKind::Parabola(c) => c.reversed(meter),
            CurveKind::Helix(c) => c.reversed(meter),
            CurveKind::Polynomial(c) => c.reversed(meter),
            CurveKind::Involute(c) => c.reversed(meter),
            CurveKind::Bulge(c) => c.reversed(meter),
            CurveKind::Trimmed(c) => c.reversed(meter),
            CurveKind::Composite(c) => c.reversed(meter),
            CurveKind::Offset(c) => c.reversed(meter),
            CurveKind::OnSurface(c) => c.reversed(meter),
            CurveKind::Intersection(c) => c.reversed(meter),
            CurveKind::Transition(c) => c.reversed(meter),
        }
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        match self {
            CurveKind::Nurbs(c) => c.transformed(transform, meter),
            CurveKind::Line(c) => c.transformed(transform, meter),
            CurveKind::Circle(c) => c.transformed(transform, meter),
            CurveKind::Ellipse(c) => c.transformed(transform, meter),
            CurveKind::Hyperbola(c) => c.transformed(transform, meter),
            CurveKind::Parabola(c) => c.transformed(transform, meter),
            CurveKind::Helix(c) => c.transformed(transform, meter),
            CurveKind::Polynomial(c) => c.transformed(transform, meter),
            CurveKind::Involute(c) => c.transformed(transform, meter),
            CurveKind::Bulge(c) => c.transformed(transform, meter),
            CurveKind::Trimmed(c) => c.transformed(transform, meter),
            CurveKind::Composite(c) => c.transformed(transform, meter),
            CurveKind::Offset(c) => c.transformed(transform, meter),
            CurveKind::OnSurface(c) => c.transformed(transform, meter),
            CurveKind::Intersection(c) => c.transformed(transform, meter),
            CurveKind::Transition(c) => c.transformed(transform, meter),
        }
    }
}
