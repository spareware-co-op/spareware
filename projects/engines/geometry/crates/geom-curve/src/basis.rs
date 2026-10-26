//! The leaf curve kinds: the kinds that hold no other curve, which the trimmed, composite and
//! offset kinds and curves on surfaces are built on.
//!
//! What it does: [`Basis`] is a closed enum with one variant per leaf kind (NURBS, the analytic
//! kinds, the bulge polyline), dispatching the kind contract to each, with the conversions to and
//! from [`CurveKind`]. The kinds that hold curves hold a `Basis`, never a `CurveKind`, so no kind
//! contains itself: no recursive type, no recursive call, nothing for WebAssembly's stack to
//! bound (chapter 28.8).
//!
//! Guarantees:
//!
//! - **The same behaviour as the kind.** Every method is the leaf kind's own.
//! - **Leaves stay leaves.** Every leaf's reversal and transform is a leaf (a circle may become an
//!   ellipse, a bulge polyline its NURBS form); a split of a bulge polyline is a trimmed one, so
//!   the trimmed kind splits by its interval instead.
//!
//! Method: the storage-and-behaviour split chapter 7 decides, closed enums for storage and traits
//! for behaviour (`projects/engines/geometry/architecture/07-geometry-types.md`); a leaf's breaks
//! are a NURBS curve's knots (where its smoothness may drop, The NURBS Book, 2nd ed., P2.5,
//! <https://doi.org/10.1007/978-3-642-59223-2>) and a polyline's vertices.
//!
//! Cost: one match per call.

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit};
use geom_run::WorkMeter;

use crate::bulge::BulgePolyline;
use crate::conic::{Circle, Ellipse, Hyperbola, Parabola};
use crate::error::CurveError;
use crate::helix::Helix;
use crate::involute::Involute;
use crate::line::Line;
use crate::map::NurbsForm;
use crate::param::{CurveKind, DerivativeBound, ParamCurve};
use crate::polynomial::Polynomial;

/// A leaf curve kind (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub enum Basis<const D: usize> {
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
}

impl<const D: usize> From<Basis<D>> for CurveKind<D> {
    fn from(b: Basis<D>) -> CurveKind<D> {
        match b {
            Basis::Nurbs(c) => CurveKind::Nurbs(c),
            Basis::Line(c) => CurveKind::Line(c),
            Basis::Circle(c) => CurveKind::Circle(c),
            Basis::Ellipse(c) => CurveKind::Ellipse(c),
            Basis::Hyperbola(c) => CurveKind::Hyperbola(c),
            Basis::Parabola(c) => CurveKind::Parabola(c),
            Basis::Helix(c) => CurveKind::Helix(c),
            Basis::Polynomial(c) => CurveKind::Polynomial(c),
            Basis::Involute(c) => CurveKind::Involute(c),
            Basis::Bulge(c) => CurveKind::Bulge(c),
        }
    }
}

impl<const D: usize> TryFrom<CurveKind<D>> for Basis<D> {
    type Error = CurveError;

    /// The leaf inside a kind; a kind that holds curves is not one ([`CurveError::Defect`] where a
    /// leaf was promised).
    fn try_from(k: CurveKind<D>) -> Result<Basis<D>, CurveError> {
        match k {
            CurveKind::Nurbs(c) => Ok(Basis::Nurbs(c)),
            CurveKind::Line(c) => Ok(Basis::Line(c)),
            CurveKind::Circle(c) => Ok(Basis::Circle(c)),
            CurveKind::Ellipse(c) => Ok(Basis::Ellipse(c)),
            CurveKind::Hyperbola(c) => Ok(Basis::Hyperbola(c)),
            CurveKind::Parabola(c) => Ok(Basis::Parabola(c)),
            CurveKind::Helix(c) => Ok(Basis::Helix(c)),
            CurveKind::Polynomial(c) => Ok(Basis::Polynomial(c)),
            CurveKind::Involute(c) => Ok(Basis::Involute(c)),
            CurveKind::Bulge(c) => Ok(Basis::Bulge(c)),
            CurveKind::Trimmed(_)
            | CurveKind::Composite(_)
            | CurveKind::Offset(_)
            | CurveKind::OnSurface(_) => Err(CurveError::Defect),
        }
    }
}

impl<const D: usize> Basis<D> {
    /// The parameters strictly inside `interval` where the leaf may not be smooth: a NURBS
    /// curve's distinct interior knots, a bulge polyline's vertices; none for the analytic kinds.
    pub(crate) fn breaks(&self, (lo, hi): (f64, f64)) -> Vec<f64> {
        let mut out: Vec<f64> = match self {
            Basis::Nurbs(c) => c.basis().knots().to_vec(),
            Basis::Bulge(p) => {
                let (a, _) = ParamCurve::domain(p);
                (0..=p.segments())
                    .filter_map(|i| u32::try_from(i).ok())
                    .map(|i| a + f64::from(i))
                    .collect()
            }
            _ => Vec::new(),
        };
        out.retain(|&x| lo < x && x < hi);
        out.dedup();
        out
    }
}

impl<const D: usize> ParamCurve<D> for Basis<D> {
    fn domain(&self) -> (f64, f64) {
        match self {
            Basis::Nurbs(c) => ParamCurve::domain(c),
            Basis::Line(c) => ParamCurve::domain(c),
            Basis::Circle(c) => ParamCurve::domain(c),
            Basis::Ellipse(c) => ParamCurve::domain(c),
            Basis::Hyperbola(c) => ParamCurve::domain(c),
            Basis::Parabola(c) => ParamCurve::domain(c),
            Basis::Helix(c) => ParamCurve::domain(c),
            Basis::Polynomial(c) => ParamCurve::domain(c),
            Basis::Involute(c) => ParamCurve::domain(c),
            Basis::Bulge(c) => ParamCurve::domain(c),
        }
    }

    fn point(&self, t: f64, side: Limit) -> Result<[f64; D], CurveError> {
        match self {
            Basis::Nurbs(c) => ParamCurve::point(c, t, side),
            Basis::Line(c) => ParamCurve::point(c, t, side),
            Basis::Circle(c) => ParamCurve::point(c, t, side),
            Basis::Ellipse(c) => ParamCurve::point(c, t, side),
            Basis::Hyperbola(c) => ParamCurve::point(c, t, side),
            Basis::Parabola(c) => ParamCurve::point(c, t, side),
            Basis::Helix(c) => ParamCurve::point(c, t, side),
            Basis::Polynomial(c) => ParamCurve::point(c, t, side),
            Basis::Involute(c) => ParamCurve::point(c, t, side),
            Basis::Bulge(c) => ParamCurve::point(c, t, side),
        }
    }

    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; D]]) -> Result<(), CurveError> {
        match self {
            Basis::Nurbs(c) => ParamCurve::derivatives(c, t, side, out),
            Basis::Line(c) => ParamCurve::derivatives(c, t, side, out),
            Basis::Circle(c) => ParamCurve::derivatives(c, t, side, out),
            Basis::Ellipse(c) => ParamCurve::derivatives(c, t, side, out),
            Basis::Hyperbola(c) => ParamCurve::derivatives(c, t, side, out),
            Basis::Parabola(c) => ParamCurve::derivatives(c, t, side, out),
            Basis::Helix(c) => ParamCurve::derivatives(c, t, side, out),
            Basis::Polynomial(c) => ParamCurve::derivatives(c, t, side, out),
            Basis::Involute(c) => ParamCurve::derivatives(c, t, side, out),
            Basis::Bulge(c) => ParamCurve::derivatives(c, t, side, out),
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
            Basis::Nurbs(c) => c.enclose(t, side, order, meter),
            Basis::Line(c) => c.enclose(t, side, order, meter),
            Basis::Circle(c) => c.enclose(t, side, order, meter),
            Basis::Ellipse(c) => c.enclose(t, side, order, meter),
            Basis::Hyperbola(c) => c.enclose(t, side, order, meter),
            Basis::Parabola(c) => c.enclose(t, side, order, meter),
            Basis::Helix(c) => c.enclose(t, side, order, meter),
            Basis::Polynomial(c) => c.enclose(t, side, order, meter),
            Basis::Involute(c) => c.enclose(t, side, order, meter),
            Basis::Bulge(c) => c.enclose(t, side, order, meter),
        }
    }

    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError> {
        match self {
            Basis::Nurbs(c) => ParamCurve::bound(c, span, meter),
            Basis::Line(c) => ParamCurve::bound(c, span, meter),
            Basis::Circle(c) => ParamCurve::bound(c, span, meter),
            Basis::Ellipse(c) => ParamCurve::bound(c, span, meter),
            Basis::Hyperbola(c) => ParamCurve::bound(c, span, meter),
            Basis::Parabola(c) => ParamCurve::bound(c, span, meter),
            Basis::Helix(c) => ParamCurve::bound(c, span, meter),
            Basis::Polynomial(c) => ParamCurve::bound(c, span, meter),
            Basis::Involute(c) => ParamCurve::bound(c, span, meter),
            Basis::Bulge(c) => ParamCurve::bound(c, span, meter),
        }
    }

    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<D>, CurveError> {
        match self {
            Basis::Nurbs(c) => c.derivative_bound(span, order, meter),
            Basis::Line(c) => c.derivative_bound(span, order, meter),
            Basis::Circle(c) => c.derivative_bound(span, order, meter),
            Basis::Ellipse(c) => c.derivative_bound(span, order, meter),
            Basis::Hyperbola(c) => c.derivative_bound(span, order, meter),
            Basis::Parabola(c) => c.derivative_bound(span, order, meter),
            Basis::Helix(c) => c.derivative_bound(span, order, meter),
            Basis::Polynomial(c) => c.derivative_bound(span, order, meter),
            Basis::Involute(c) => c.derivative_bound(span, order, meter),
            Basis::Bulge(c) => c.derivative_bound(span, order, meter),
        }
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        match self {
            Basis::Nurbs(c) => c.to_nurbs(meter),
            Basis::Line(c) => c.to_nurbs(meter),
            Basis::Circle(c) => c.to_nurbs(meter),
            Basis::Ellipse(c) => c.to_nurbs(meter),
            Basis::Hyperbola(c) => c.to_nurbs(meter),
            Basis::Parabola(c) => c.to_nurbs(meter),
            Basis::Helix(c) => c.to_nurbs(meter),
            Basis::Polynomial(c) => c.to_nurbs(meter),
            Basis::Involute(c) => c.to_nurbs(meter),
            Basis::Bulge(c) => c.to_nurbs(meter),
        }
    }

    fn split(
        &self,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        match self {
            Basis::Nurbs(c) => ParamCurve::split(c, t, meter),
            Basis::Line(c) => ParamCurve::split(c, t, meter),
            Basis::Circle(c) => ParamCurve::split(c, t, meter),
            Basis::Ellipse(c) => ParamCurve::split(c, t, meter),
            Basis::Hyperbola(c) => ParamCurve::split(c, t, meter),
            Basis::Parabola(c) => ParamCurve::split(c, t, meter),
            Basis::Helix(c) => ParamCurve::split(c, t, meter),
            Basis::Polynomial(c) => ParamCurve::split(c, t, meter),
            Basis::Involute(c) => ParamCurve::split(c, t, meter),
            Basis::Bulge(c) => ParamCurve::split(c, t, meter),
        }
    }

    fn reversed(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        match self {
            Basis::Nurbs(c) => c.reversed(meter),
            Basis::Line(c) => c.reversed(meter),
            Basis::Circle(c) => c.reversed(meter),
            Basis::Ellipse(c) => c.reversed(meter),
            Basis::Hyperbola(c) => c.reversed(meter),
            Basis::Parabola(c) => c.reversed(meter),
            Basis::Helix(c) => c.reversed(meter),
            Basis::Polynomial(c) => c.reversed(meter),
            Basis::Involute(c) => c.reversed(meter),
            Basis::Bulge(c) => c.reversed(meter),
        }
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        match self {
            Basis::Nurbs(c) => c.transformed(transform, meter),
            Basis::Line(c) => c.transformed(transform, meter),
            Basis::Circle(c) => c.transformed(transform, meter),
            Basis::Ellipse(c) => c.transformed(transform, meter),
            Basis::Hyperbola(c) => c.transformed(transform, meter),
            Basis::Parabola(c) => c.transformed(transform, meter),
            Basis::Helix(c) => c.transformed(transform, meter),
            Basis::Polynomial(c) => c.transformed(transform, meter),
            Basis::Involute(c) => c.transformed(transform, meter),
            Basis::Bulge(c) => c.transformed(transform, meter),
        }
    }
}
