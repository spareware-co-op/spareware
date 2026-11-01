//! The intersection curve kind: a branch where two surfaces meet, held by its definition, with
//! its 3D curve and both pcurves as certified NURBS caches.
//!
//! What it does: [`IntersectionCurve`] stores its definition (chapter 12.12): the two surfaces as
//! opaque [`SurfaceRef`]s into the geometry pool that holds it, and the certified chain of marched
//! points in (u₁, v₁, u₂, v₂), each with its parameter on the curve. Beside it are the caches, all
//! on one parameter t: the 3D curve C(t), and the pcurves p₁(t) and p₂(t) in each surface's
//! parameters. [`Span`] records, for each span of the 3D cache, how far C may be from the true
//! curve there, and how far each mapped pcurve Sᵢ(pᵢ(t)) may be from C(t). `geom-curve` sits below
//! `geom-surface` and cannot see surfaces (chapter 27), so it evaluates, bounds, splits and
//! converts the kind through its 3D cache alone, as for [`crate::OnSurface`]; `geom-intersect`
//! builds the caches and their bounds, and reaches the true curve from the definition.
//!
//! Guarantees:
//!
//! - **The contract on the cache.** Every evaluation, bound, split and transform is the NURBS
//!   kind's on the 3D cache, certified for the cache.
//! - **A bound for every span.** The spans tile the domain, end to end, each with three finite
//!   bounds that are not negative; the curve's [`IntersectionCurve::error`] is the largest. A
//!   split adds its deviation to every bound of the two halves; a reversal mirrors the spans and
//!   adds its deviation; a transform scales every bound by an upper bound on the transform's
//!   stretch (its linear part's Frobenius norm) and adds its deviation. The pcurves are never
//!   approximated here: a split trims them, a reversal reverses them, a transform leaves them
//!   (the surfaces move with the pool, their parameters do not).
//! - **The definition travels.** A split keeps each chain point on the half whose domain holds its
//!   parameter, both halves the one at the cut; a reversal reverses the chain and negates its
//!   parameters.
//!
//! Method: the procedural intersection curve of chapter 12.12
//! (`projects/engines/geometry/architecture/12-intersections.md`), its cache-only contract that of
//! chapter 7 (`projects/engines/geometry/architecture/07-geometry.md`).
//!
//! Cost: the NURBS kind's costs on the caches; a split or reversal is linear in the spans and the
//! chain.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': c the cache, p the pcurves, f the stretch, a, b and e entries"
)]

use alloc::boxed::Box;
use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Curve, Limit};
use geom_run::WorkMeter;

use crate::affine::Entries;
use crate::error::CurveError;
use crate::map::{NurbsForm, ParamMap};
use crate::on_surface::SurfaceRef;
use crate::param::{CurveKind, DerivativeBound, ParamCurve};
use crate::tight::{self, point};
use crate::trimmed::Trimmed;

/// One span of the 3D cache and its certified bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    /// The span of t.
    pub at: (f64, f64),
    /// How far the 3D cache may be from the true curve over the span.
    pub curve: f64,
    /// How far each mapped pcurve, Sᵢ(pᵢ(t)), may be from the 3D cache C(t) over the span.
    pub pcurves: [f64; 2],
}

/// A marched point of the definition: (u₁, v₁, u₂, v₂) enclosed, and its parameter on the curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marched {
    /// The point's enclosure, on the first surface then the second.
    pub at: [Interval; 4],
    /// Its parameter t on the caches.
    pub t: f64,
}

/// A branch where two surfaces meet (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct IntersectionCurve<const D: usize> {
    surfaces: [SurfaceRef; 2],
    chain: Vec<Marched>,
    cache: Curve<D>,
    // Boxed, so that the kind's variant stays the size of the others.
    pcurves: Box<[Trimmed<2>; 2]>,
    spans: Vec<Span>,
}

/// Whether `spans` tile `domain` end to end, every bound finite and not negative.
fn tiles(spans: &[Span], (lo, hi): (f64, f64)) -> bool {
    let mut from = lo;
    for s in spans {
        let (a, b) = s.at;
        let fine = |x: f64| x.is_finite() && x >= 0.0;
        let rising = matches!(a.partial_cmp(&b), Some(core::cmp::Ordering::Less));
        if a != from || !rising || !fine(s.curve) || !s.pcurves.iter().all(|&x| fine(x)) {
            return false;
        }
        from = b;
    }
    from == hi && !spans.is_empty()
}

impl<const D: usize> IntersectionCurve<D> {
    /// The intersection of `surfaces` along the marched `chain`, with its 3D `cache`, its two
    /// `pcurves` on the cache's parameter, and the bounds of each of its `spans`.
    ///
    /// # Errors
    ///
    /// [`CurveError::Dimension`] unless D is 3; [`CurveError::SpanOutside`] unless both pcurves
    /// share the cache's domain, the spans tile it, and every chain point's parameter lies in it,
    /// in order; [`CurveError::NotFinite`] for a bound that is not finite or is negative, or a
    /// chain parameter that is not finite.
    pub fn new(
        surfaces: [SurfaceRef; 2],
        chain: Vec<Marched>,
        cache: Curve<D>,
        pcurves: [Trimmed<2>; 2],
        spans: Vec<Span>,
    ) -> Result<IntersectionCurve<D>, CurveError> {
        if D != 3 {
            return Err(CurveError::Dimension { dimension: D });
        }
        let domain = ParamCurve::domain(&cache);
        if pcurves.iter().any(|p| p.interval() != domain) {
            return Err(CurveError::SpanOutside);
        }
        let bad = |x: f64| !(x.is_finite() && x >= 0.0);
        if spans
            .iter()
            .any(|s| bad(s.curve) || s.pcurves.iter().any(|&x| bad(x)))
        {
            return Err(CurveError::NotFinite);
        }
        if !tiles(&spans, domain) {
            return Err(CurveError::SpanOutside);
        }
        if chain.iter().any(|m| !m.t.is_finite()) {
            return Err(CurveError::NotFinite);
        }
        let (lo, hi) = domain;
        let ordered = chain.windows(2).all(|w| match w {
            [a, b] => a.t <= b.t,
            _ => true,
        });
        if !ordered || chain.iter().any(|m| !(lo <= m.t && m.t <= hi)) {
            return Err(CurveError::SpanOutside);
        }
        Ok(IntersectionCurve {
            surfaces,
            chain,
            cache,
            pcurves: Box::new(pcurves),
            spans,
        })
    }

    /// The two surfaces' references.
    #[must_use]
    pub fn surfaces(&self) -> [SurfaceRef; 2] {
        self.surfaces
    }

    /// The marched points of the definition, in order of t.
    #[must_use]
    pub fn chain(&self) -> &[Marched] {
        &self.chain
    }

    /// The 3D cache.
    #[must_use]
    pub fn cache(&self) -> &Curve<D> {
        &self.cache
    }

    /// The pcurves, in the first surface's parameters and then the second's.
    #[must_use]
    pub fn pcurves(&self) -> &[Trimmed<2>; 2] {
        &self.pcurves
    }

    /// The spans and their bounds, in order of t.
    #[must_use]
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// The largest bound of any span: how far the 3D cache may be from the true curve.
    #[must_use]
    pub fn error(&self) -> f64 {
        self.spans.iter().map(|s| s.curve).fold(0.0, tight::larger)
    }
}

/// The NURBS curve inside a kind that a NURBS operation returned.
fn nurbs<const D: usize>(k: CurveKind<D>) -> Result<Curve<D>, CurveError> {
    match k {
        CurveKind::Nurbs(c) => Ok(c),
        _ => Err(CurveError::Defect),
    }
}

/// `x + e`, rounded up.
fn plus(x: f64, e: f64) -> Result<f64, CurveError> {
    Ok(tight::add(point(x)?, point(e)?)?.hi())
}

/// A span with every bound grown by `e`.
fn grown(s: Span, at: (f64, f64), e: f64) -> Result<Span, CurveError> {
    let [p, q] = s.pcurves;
    Ok(Span {
        at,
        curve: plus(s.curve, e)?,
        pcurves: [plus(p, e)?, plus(q, e)?],
    })
}

impl<const D: usize> ParamCurve<D> for IntersectionCurve<D> {
    fn domain(&self) -> (f64, f64) {
        ParamCurve::domain(&self.cache)
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
        let (cl, cr) = ParamCurve::split(&self.cache, t, meter)?;
        // The pcurves are trimmed, never cut: a cut would move them in (u, v), which only the
        // surfaces can turn into a distance.
        let [p, q] = &*self.pcurves;
        let (pl, pr) = p.halves(t)?;
        let (ql, qr) = q.halves(t)?;
        let mut left = Vec::new();
        let mut right = Vec::new();
        for s in &self.spans {
            let (a, b) = s.at;
            if b <= t {
                left.push(grown(*s, s.at, cl.deviation)?);
            } else if t <= a {
                right.push(grown(*s, s.at, cr.deviation)?);
            } else {
                left.push(grown(*s, (a, t), cl.deviation)?);
                right.push(grown(*s, (t, b), cr.deviation)?);
            }
        }
        let chain_left = self.chain.iter().filter(|m| m.t <= t).copied().collect();
        let chain_right = self.chain.iter().filter(|m| t <= m.t).copied().collect();
        let half = |c: Changed<CurveKind<D>>,
                    p: Trimmed<2>,
                    q: Trimmed<2>,
                    chain: Vec<Marched>,
                    spans: Vec<Span>|
         -> Result<Changed<CurveKind<D>>, CurveError> {
            Ok(Changed {
                value: CurveKind::Intersection(IntersectionCurve::new(
                    self.surfaces,
                    chain,
                    nurbs(c.value)?,
                    [p, q],
                    spans,
                )?),
                deviation: c.deviation,
            })
        };
        Ok((
            half(cl, pl, ql, chain_left, left)?,
            half(cr, pr, qr, chain_right, right)?,
        ))
    }

    fn reversed(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        let c = ParamCurve::reversed(&self.cache, meter)?;
        // Every leaf kind reverses exactly; a pcurve moved in (u, v) would need the surface to
        // say how far that moves the curve.
        let [p, q] = &*self.pcurves;
        let (p, q) = (p.reverse(meter)?, q.reverse(meter)?);
        if p.deviation != 0.0 || q.deviation != 0.0 {
            return Err(CurveError::Defect);
        }
        let mut spans = Vec::with_capacity(self.spans.len());
        for s in self.spans.iter().rev() {
            let (a, b) = s.at;
            spans.push(grown(*s, (-b, -a), c.deviation)?);
        }
        let chain = self
            .chain
            .iter()
            .rev()
            .map(|m| Marched { at: m.at, t: -m.t })
            .collect();
        Ok(Changed {
            value: CurveKind::Intersection(IntersectionCurve::new(
                self.surfaces,
                chain,
                nurbs(c.value)?,
                [p.value, q.value],
                spans,
            )?),
            deviation: c.deviation,
        })
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        let c = ParamCurve::transformed(&self.cache, transform, meter)?;
        // Every bound is a distance in space, so it stretches by at most the linear part's
        // Frobenius norm.
        let mut f = point(0.0)?;
        for row in Entries::of(transform)?.exact_rows() {
            let [a, b, e, _] = *row;
            for x in [a, b, e] {
                f = tight::add(f, tight::square(x)?)?;
            }
        }
        let stretch = tight::sqrt(f)?;
        let scale = |x: f64| -> Result<f64, CurveError> {
            Ok(tight::add(tight::mul(point(x)?, stretch)?, point(c.deviation)?)?.hi())
        };
        let mut spans = Vec::with_capacity(self.spans.len());
        for s in &self.spans {
            let [p, q] = s.pcurves;
            spans.push(Span {
                at: s.at,
                curve: scale(s.curve)?,
                pcurves: [scale(p)?, scale(q)?],
            });
        }
        Ok(Changed {
            value: CurveKind::Intersection(IntersectionCurve::new(
                self.surfaces,
                self.chain.clone(),
                nurbs(c.value)?,
                (*self.pcurves).clone(),
                spans,
            )?),
            deviation: c.deviation,
        })
    }
}
