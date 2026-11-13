//! A curve kind broken on purpose, for the curve targets' tests (28.4, fault injection: a check
//! that cannot fail proves nothing). `tests/curves.rs` and `tests/curve_offsets.rs` include it.

use geom_curve::map::NurbsForm;
use geom_curve::{CurveError, CurveKind, DerivativeBound, ParamCurve};
use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit};
use geom_run::WorkMeter;

/// A kind `inner` with its bound over any span moved by one when `moved`, and conversions that
/// spend every unit the meter has left before they start when `dear`: then the contract stops on
/// its budget in the transform law, before any law at a parameter has run.
#[derive(Debug)]
pub struct Broken<K> {
    pub inner: K,
    pub moved: bool,
    pub dear: bool,
}

impl<K> Broken<K> {
    /// Spends the meter's budget, when the conversions are dear.
    fn spend(&self, meter: &mut WorkMeter<'_>) -> Result<(), CurveError> {
        if self.dear {
            meter.charge(u64::MAX)?;
        }
        Ok(())
    }
}

impl<K: ParamCurve<D>, const D: usize> ParamCurve<D> for Broken<K> {
    fn domain(&self) -> (f64, f64) {
        self.inner.domain()
    }
    fn point(&self, t: f64, side: Limit) -> Result<[f64; D], CurveError> {
        self.inner.point(t, side)
    }
    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; D]]) -> Result<(), CurveError> {
        self.inner.derivatives(t, side, out)
    }
    fn enclose(
        &self,
        t: f64,
        side: Limit,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; D]>, CurveError> {
        self.inner.enclose(t, side, order, meter)
    }
    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError> {
        let b = self.inner.bound(span, meter)?;
        if !self.moved {
            return Ok(b);
        }
        Ok(Bounds {
            lo: b.lo.map(|x| x + 1.0),
            hi: b.hi.map(|x| x + 1.0),
        })
    }
    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<D>, CurveError> {
        self.inner.derivative_bound(span, order, meter)
    }
    fn breaks(&self) -> Vec<f64> {
        self.inner.breaks()
    }
    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        self.spend(meter)?;
        self.inner.to_nurbs(meter)
    }
    fn split(
        &self,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        self.inner.split(t, meter)
    }
    fn reversed(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        self.inner.reversed(meter)
    }
    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        self.spend(meter)?;
        self.inner.transformed(transform, meter)
    }
}
