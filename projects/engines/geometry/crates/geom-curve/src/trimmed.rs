//! The trimmed curve kind: a basis curve on a parameter interval inside its domain, the basis
//! itself left untouched.
//!
//! What it does: [`Trimmed`] stores a [`Basis`] (any leaf kind: NURBS, analytic or bulge
//! polyline) and an interval [a, b] inside its domain, and is the basis on that interval, on the
//! basis's own parameter. Trimming keeps the basis exactly as stored: a NURBS curve trimmed at a
//! crossing keeps its knots and control points, where cutting it would insert knots and round
//! (chapter 14: "a trimmed spline is section 7's trimmed curve, the original curve on an implicit
//! parameter interval"). Composite and offset curves and curves on surfaces are built on trimmed
//! curves, so the kinds that hold curves hold only leaves: no kind contains itself.
//!
//! Guarantees:
//!
//! - **Exact.** Evaluation, enclosures and bounds are the basis's, inside the interval; a split
//!   changes only the interval; a reversal or transform is the basis's, with its deviation.
//! - **Refused outside.** A parameter or span outside the interval is an error, though the basis
//!   would take it.
//! - **Converted by cutting.** The NURBS form is the basis cut at the interval's ends (exact for
//!   the analytic kinds, by knot insertion for NURBS), then converted; the deviations add.
//!
//! Method: STEP's `trimmed_curve` with parameter trims (ISO 10303-42), on the kinds of chapter 7
//! (`projects/engines/geometry/architecture/07-geometry-types.md`).
//!
//! Cost: the basis's costs; a conversion adds at most two splits.

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit};
use geom_run::WorkMeter;

use crate::basis::Basis;
use crate::error::CurveError;
use crate::form::{check_domain, check_parameter, split_domain};
use crate::map::NurbsForm;
use crate::param::{CurveKind, DerivativeBound, ParamCurve};
use crate::tight::{self, point};

/// A basis curve on a parameter interval (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Trimmed<const D: usize> {
    basis: Basis<D>,
    interval: (f64, f64),
}

impl<const D: usize> Trimmed<D> {
    /// The basis on `interval`, which must lie inside its domain.
    ///
    /// # Errors
    ///
    /// [`CurveError::SpanOutside`] unless the interval is finite, ordered (a < b) and inside the
    /// basis's domain.
    pub fn new(basis: Basis<D>, interval: (f64, f64)) -> Result<Trimmed<D>, CurveError> {
        check_domain(interval)?;
        let (a, b) = basis.domain();
        if !(a <= interval.0 && interval.1 <= b) {
            return Err(CurveError::SpanOutside);
        }
        Ok(Trimmed { basis, interval })
    }

    /// The whole of a basis with a finite domain.
    ///
    /// # Errors
    ///
    /// [`CurveError::Unbounded`] for a ray or an infinite line.
    pub fn whole(basis: Basis<D>) -> Result<Trimmed<D>, CurveError> {
        let d = basis.domain();
        if !(d.0.is_finite() && d.1.is_finite()) {
            return Err(CurveError::Unbounded);
        }
        Trimmed::new(basis, d)
    }

    /// The basis.
    #[must_use]
    pub fn basis(&self) -> &Basis<D> {
        &self.basis
    }

    /// The interval.
    #[must_use]
    pub fn interval(&self) -> (f64, f64) {
        self.interval
    }

    /// The basis cut to the interval, as a leaf of its own, and the deviation of the cuts. A bulge
    /// polyline splits only into trimmed polylines, so its exact NURBS form (on the same
    /// parameter) is cut instead. Everything here stays among the leaves, so no call reaches a
    /// kind that holds curves.
    pub(crate) fn cut(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<Basis<D>>, CurveError> {
        let (a, b) = self.basis.domain();
        let (lo, hi) = self.interval;
        let mut deviation = point(0.0)?;
        let mut piece = match &self.basis {
            Basis::Bulge(p) if lo > a || hi < b => {
                let n = p.to_nurbs(meter)?;
                deviation = point(n.deviation)?;
                Basis::Nurbs(n.value.curve)
            }
            other => other.clone(),
        };
        if lo > a {
            let (_, right) = piece.split(lo, meter)?;
            deviation = tight::add(deviation, point(right.deviation)?)?;
            piece = Basis::try_from(right.value)?;
        }
        if hi < b {
            let (left, _) = piece.split(hi, meter)?;
            deviation = tight::add(deviation, point(left.deviation)?)?;
            piece = Basis::try_from(left.value)?;
        }
        Ok(Changed {
            value: piece,
            deviation: deviation.hi(),
        })
    }

    /// The trimmed curve traversed the other way.
    pub(crate) fn reverse(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<Trimmed<D>>, CurveError> {
        let r = self.basis.reversed(meter)?;
        let basis = Basis::try_from(r.value)?;
        let (lo, hi) = self.interval;
        Ok(Changed {
            value: Trimmed::new(basis, (-hi, -lo))?,
            deviation: r.deviation,
        })
    }

    /// The trimmed curve's image.
    pub(crate) fn moved(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<Trimmed<D>>, CurveError> {
        let r = self.basis.transformed(transform, meter)?;
        let basis = Basis::try_from(r.value)?;
        Ok(Changed {
            value: Trimmed::new(basis, self.interval)?,
            deviation: r.deviation,
        })
    }

    /// The two halves at `t`, strictly inside the interval, the basis shared.
    pub(crate) fn halves(&self, t: f64) -> Result<(Trimmed<D>, Trimmed<D>), CurveError> {
        let (left, right) = split_domain(self.interval, t)?;
        Ok((
            Trimmed {
                basis: self.basis.clone(),
                interval: left,
            },
            Trimmed {
                basis: self.basis.clone(),
                interval: right,
            },
        ))
    }

    /// The parameters strictly inside the interval where the basis may not be smooth (a NURBS
    /// basis's distinct interior knots, a bulge polyline's vertices), increasing; none for the
    /// analytic kinds. Surfaces built on the curve read them (`geom-surface`'s swept kinds).
    #[must_use]
    pub fn breaks(&self) -> Vec<f64> {
        self.basis.breaks(self.interval)
    }
}

impl<const D: usize> ParamCurve<D> for Trimmed<D> {
    fn domain(&self) -> (f64, f64) {
        self.interval
    }

    fn point(&self, t: f64, side: Limit) -> Result<[f64; D], CurveError> {
        check_parameter(self.interval, t)?;
        self.basis.point(t, side)
    }

    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; D]]) -> Result<(), CurveError> {
        check_parameter(self.interval, t)?;
        self.basis.derivatives(t, side, out)
    }

    fn enclose(
        &self,
        t: f64,
        side: Limit,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; D]>, CurveError> {
        check_parameter(self.interval, t)?;
        self.basis.enclose(t, side, order, meter)
    }

    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError> {
        crate::nurbs::check_span(span, self.interval)?;
        self.basis.bound(span, meter)
    }

    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<D>, CurveError> {
        crate::nurbs::check_span(span, self.interval)?;
        self.basis.derivative_bound(span, order, meter)
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        let piece = self.cut(meter)?;
        let n = piece.value.to_nurbs(meter)?;
        Ok(Changed {
            value: n.value,
            deviation: tight::add(point(piece.deviation)?, point(n.deviation)?)?.hi(),
        })
    }

    fn split(
        &self,
        t: f64,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        let (left, right) = self.halves(t)?;
        let part = |piece| Changed {
            value: CurveKind::Trimmed(piece),
            deviation: 0.0,
        };
        Ok((part(left), part(right)))
    }

    fn reversed(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        let r = self.reverse(meter)?;
        Ok(Changed {
            value: CurveKind::Trimmed(r.value),
            deviation: r.deviation,
        })
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        let r = self.moved(transform, meter)?;
        Ok(Changed {
            value: CurveKind::Trimmed(r.value),
            deviation: r.deviation,
        })
    }
}
