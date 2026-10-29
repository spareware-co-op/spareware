//! The curve-on-surface kind: a curve in a surface's parameter space (a pcurve), the surface held
//! by reference, and its 3D curve as a certified NURBS cache.
//!
//! What it does: [`OnSurface`] stores the pcurve (a trimmed 2D curve in the surface's (u, v)
//! space), an opaque [`SurfaceRef`] into the geometry pool that holds the surface (chapter 8.1:
//! geometry attaches by index into a body's pools), and the 3D curve as a NURBS cache on the
//! pcurve's parameter with a certified bound on its distance from S(pcurve(t)). `geom-curve` sits
//! below `geom-surface` and cannot see surfaces (chapter 27), so, as chapter 12 lays out for the
//! intersection curve, it evaluates, bounds, splits and converts the kind through its cache
//! alone: the kind's curve is the cache, and [`OnSurface::error`] says how far the surface's own
//! curve may be from it. The surface crates build the cache and its bound, and evaluate S on the
//! pcurve when they need the exact curve.
//!
//! Guarantees:
//!
//! - **The contract on the cache.** Every evaluation, bound, split and transform is the NURBS
//!   kind's on the cache, certified for the cache; the error carries over: a split adds its
//!   deviation to each half's error, and a transform scales the error by an upper bound on the
//!   transform's stretch (its linear part's Frobenius norm) and adds its deviation.
//! - **Exact pcurve.** The pcurve is never approximated here: a split trims it, a reversal
//!   reverses it, a transform leaves it (the surface moves with the pool, its parameters do not).
//!
//! Method: the pcurve and its 3D cache of chapter 8 (`projects/engines/geometry/architecture/08-topology.md`,
//! fins and their pcurves) and the cache-only evaluation chapter 12 gives the intersection curve
//! (`projects/engines/geometry/architecture/12-intersections.md`).
//!
//! Cost: the NURBS kind's costs on the cache.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': c the cache, p the pcurve, f the stretch, a, b and e entries"
)]

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Curve, Limit};
use geom_run::WorkMeter;

use crate::affine::Entries;
use crate::error::CurveError;
use crate::map::{NurbsForm, ParamMap};
use crate::param::{CurveKind, DerivativeBound, ParamCurve};
use crate::tight::{self, point};
use crate::trimmed::Trimmed;

/// An opaque reference to a surface in the geometry pool that holds this curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SurfaceRef(pub u64);

/// A curve on a surface (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct OnSurface<const D: usize> {
    pcurve: Trimmed<2>,
    surface: SurfaceRef,
    cache: Curve<D>,
    error: f64,
}

impl<const D: usize> OnSurface<D> {
    /// The curve on `surface` along `pcurve`, with its 3D `cache` on the pcurve's parameter and a
    /// certified `error`, the largest distance between the cache and S(pcurve(t)).
    ///
    /// # Errors
    ///
    /// [`CurveError::Dimension`] unless D is 3; [`CurveError::NotFinite`] for an error that is not
    /// finite or is negative; [`CurveError::SpanOutside`] unless the cache's domain is the
    /// pcurve's interval.
    pub fn new(
        pcurve: Trimmed<2>,
        surface: SurfaceRef,
        cache: Curve<D>,
        error: f64,
    ) -> Result<OnSurface<D>, CurveError> {
        if D != 3 {
            return Err(CurveError::Dimension { dimension: D });
        }
        if !(error.is_finite() && error >= 0.0) {
            return Err(CurveError::NotFinite);
        }
        if ParamCurve::domain(&cache) != pcurve.interval() {
            return Err(CurveError::SpanOutside);
        }
        Ok(OnSurface {
            pcurve,
            surface,
            cache,
            error,
        })
    }

    /// The pcurve.
    #[must_use]
    pub fn pcurve(&self) -> &Trimmed<2> {
        &self.pcurve
    }

    /// The surface's reference.
    #[must_use]
    pub fn surface(&self) -> SurfaceRef {
        self.surface
    }

    /// The 3D cache.
    #[must_use]
    pub fn cache(&self) -> &Curve<D> {
        &self.cache
    }

    /// The certified distance between the cache and the surface's curve S(pcurve(t)).
    #[must_use]
    pub fn error(&self) -> f64 {
        self.error
    }

    fn with(
        &self,
        pcurve: Trimmed<2>,
        cache: Curve<D>,
        error: f64,
    ) -> Result<OnSurface<D>, CurveError> {
        OnSurface::new(pcurve, self.surface, cache, error)
    }
}

/// The NURBS curve inside a kind that a NURBS operation returned.
fn nurbs<const D: usize>(k: CurveKind<D>) -> Result<Curve<D>, CurveError> {
    match k {
        CurveKind::Nurbs(c) => Ok(c),
        _ => Err(CurveError::Defect),
    }
}

impl<const D: usize> ParamCurve<D> for OnSurface<D> {
    fn domain(&self) -> (f64, f64) {
        self.pcurve.interval()
    }

    fn point(&self, t: f64, side: Limit) -> Result<[f64; D], CurveError> {
        ParamCurve::point(&self.cache, t, side)
    }

    fn derivatives(&self, t: f64, side: Limit, out: &mut [[f64; D]]) -> Result<(), CurveError> {
        ParamCurve::derivatives(&self.cache, t, side, out)
    }

    fn enclose(
        &self,
        t: f64,
        side: Limit,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; D]>, CurveError> {
        ParamCurve::enclose(&self.cache, t, side, order, meter)
    }

    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError> {
        ParamCurve::bound(&self.cache, span, meter)
    }

    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<D>, CurveError> {
        ParamCurve::derivative_bound(&self.cache, span, order, meter)
    }

    fn to_nurbs(&self, _meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        Ok(Changed {
            value: NurbsForm {
                curve: self.cache.clone(),
                map: ParamMap::Same,
            },
            deviation: 0.0,
        })
    }

    fn split(
        &self,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        let (pl, pr) = self.pcurve.halves(t)?;
        let (cl, cr) = ParamCurve::split(&self.cache, t, meter)?;
        let half = |p: Trimmed<2>,
                    c: Changed<CurveKind<D>>|
         -> Result<Changed<CurveKind<D>>, CurveError> {
            let error = tight::add(point(self.error)?, point(c.deviation)?)?.hi();
            Ok(Changed {
                value: CurveKind::OnSurface(self.with(p, nurbs(c.value)?, error)?),
                deviation: c.deviation,
            })
        };
        Ok((half(pl, cl)?, half(pr, cr)?))
    }

    fn reversed(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        // Every leaf kind reverses exactly; a pcurve moved in (u, v) would need the surface to
        // say how far that moves the 3D curve.
        let p = self.pcurve.reverse(meter)?;
        if p.deviation != 0.0 {
            return Err(CurveError::Defect);
        }
        let c = ParamCurve::reversed(&self.cache, meter)?;
        let error = tight::add(point(self.error)?, point(c.deviation)?)?.hi();
        Ok(Changed {
            value: CurveKind::OnSurface(self.with(p.value, nurbs(c.value)?, error)?),
            deviation: c.deviation,
        })
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        let c = ParamCurve::transformed(&self.cache, transform, meter)?;
        // The error stretches by at most the linear part's Frobenius norm.
        let mut f = point(0.0)?;
        for row in Entries::of(transform)?.exact_rows() {
            let [a, b, e, _] = *row;
            for x in [a, b, e] {
                f = tight::add(f, tight::square(x)?)?;
            }
        }
        let stretch = tight::sqrt(f)?;
        let error = tight::add(
            tight::mul(point(self.error)?, stretch)?,
            point(c.deviation)?,
        )?
        .hi();
        Ok(Changed {
            value: CurveKind::OnSurface(self.with(self.pcurve.clone(), nurbs(c.value)?, error)?),
            deviation: c.deviation,
        })
    }
}
