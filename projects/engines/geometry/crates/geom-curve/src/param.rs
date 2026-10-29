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

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit};
use geom_run::WorkMeter;

use crate::error::CurveError;

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

    /// A box holding the curve at every parameter of `span`, inside the domain.
    ///
    /// # Errors
    ///
    /// [`CurveError::SpanOutside`]; [`CurveError::NotFinite`]; [`CurveError::Stopped`].
    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError>;

    /// A bound on the derivative of `order` at every parameter of `span`, from both sides at a
    /// break.
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

    /// The curve as a NURBS curve with the same parameter, and a certified bound on the distance
    /// between them at every parameter (0 when exact).
    ///
    /// # Errors
    ///
    /// As [`ParamCurve::bound`].
    fn to_nurbs(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<geom_nurbs::Curve<D>>, CurveError>;

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

/// Every curve kind, stored as itself (chapter 7): one variant per kind built. Later pieces add
/// the analytic kinds (G61), composite, trimmed, bulge, offset and pcurve kinds (G62) and spirals
/// (G63).
#[derive(Clone, Debug, PartialEq)]
pub enum CurveKind<const D: usize> {
    /// A NURBS curve, rational or not, clamped or not.
    Nurbs(geom_nurbs::Curve<D>),
}

impl<const D: usize> ParamCurve<D> for CurveKind<D> {
    fn domain(&self) -> (f64, f64) {
        match self {
            CurveKind::Nurbs(c) => ParamCurve::domain(c),
        }
    }

    fn point(&self, t: f64, side: Limit) -> Result<[f64; D], CurveError> {
        match self {
            CurveKind::Nurbs(c) => ParamCurve::point(c, t, side),
        }
    }

    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; D]]) -> Result<(), CurveError> {
        match self {
            CurveKind::Nurbs(c) => ParamCurve::derivatives(c, t, side, out),
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
        }
    }

    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError> {
        match self {
            CurveKind::Nurbs(c) => ParamCurve::bound(c, span, meter),
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
        }
    }

    fn to_nurbs(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<geom_nurbs::Curve<D>>, CurveError> {
        match self {
            CurveKind::Nurbs(c) => c.to_nurbs(meter),
        }
    }

    fn split(
        &self,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        match self {
            CurveKind::Nurbs(c) => ParamCurve::split(c, t, meter),
        }
    }

    fn reversed(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        match self {
            CurveKind::Nurbs(c) => c.reversed(meter),
        }
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        match self {
            CurveKind::Nurbs(c) => c.transformed(transform, meter),
        }
    }
}
