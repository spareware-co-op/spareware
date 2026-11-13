//! A surface kind broken on purpose, for the surface targets' tests (28.4, fault injection: a
//! check that cannot fail proves nothing). `tests/surfaces.rs` and `tests/surface_offsets.rs`
//! include it.

use geom_curve::{CurveError, DerivativeBound};
use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Direction, Limit};
use geom_run::WorkMeter;
use geom_surface::{ParamBox, ParamSurface, SurfaceForm, SurfaceKind};

/// A kind `inner` with its bound over any box moved a million units from the truth when `moved`,
/// and conversions that spend every unit the meter has left before they start when `dear`: then
/// the contract stops on its budget in the transform law, before any law at a pair has run.
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

impl<K: ParamSurface> ParamSurface for Broken<K> {
    fn domain(&self) -> ParamBox {
        self.inner.domain()
    }
    fn point(&self, u: f64, v: f64, s: (Limit, Limit)) -> Result<[f64; 3], CurveError> {
        self.inner.point(u, v, s)
    }
    fn partials(&self, u: f64, v: f64, s: (Limit, Limit)) -> Result<[[f64; 3]; 3], CurveError> {
        self.inner.partials(u, v, s)
    }
    fn enclose(
        &self,
        u: f64,
        v: f64,
        s: (Limit, Limit),
        m: &mut WorkMeter<'_>,
    ) -> Result<[[Interval; 3]; 3], CurveError> {
        self.inner.enclose(u, v, s, m)
    }
    fn bound(&self, r: ParamBox, m: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        let b = self.inner.bound(r, m)?;
        if !self.moved {
            return Ok(b);
        }
        Ok(Bounds {
            lo: b.lo.map(|x| x + 1e6),
            hi: b.hi.map(|x| x + 1e6),
        })
    }
    fn derivative_bound(
        &self,
        r: ParamBox,
        d: Direction,
        m: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<3>, CurveError> {
        self.inner.derivative_bound(r, d, m)
    }
    fn partials_over(
        &self,
        r: ParamBox,
        s: (Limit, Limit),
        k: usize,
        m: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 3]>, CurveError> {
        self.inner.partials_over(r, s, k, m)
    }
    fn breaks(&self) -> (Vec<f64>, Vec<f64>) {
        self.inner.breaks()
    }
    fn normal_bound(&self, r: ParamBox, m: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        self.inner.normal_bound(r, m)
    }
    fn to_nurbs(&self, m: &mut WorkMeter<'_>) -> Result<Changed<SurfaceForm>, CurveError> {
        self.spend(m)?;
        self.inner.to_nurbs(m)
    }
    fn split(
        &self,
        d: Direction,
        t: f64,
        m: &mut WorkMeter<'_>,
    ) -> Result<(Changed<SurfaceKind>, Changed<SurfaceKind>), CurveError> {
        self.inner.split(d, t, m)
    }
    fn reversed(
        &self,
        d: Direction,
        m: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        self.inner.reversed(d, m)
    }
    fn transformed(
        &self,
        t: &TypedTransform,
        m: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        self.spend(m)?;
        self.inner.transformed(t, m)
    }
}
