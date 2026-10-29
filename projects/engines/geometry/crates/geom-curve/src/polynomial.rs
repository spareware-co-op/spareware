//! The polynomial curve kind: P(t) = Σⱼ aⱼ·t^j, a vector coefficient per power.
//!
//! What it does: [`Polynomial`] stores the coefficients a₀ … aₙ (degree 1 to `geom-nurbs`'s
//! `MAX_DEGREE`) and a finite domain, as IFC 4.3's `IfcPolynomialCurve` stores one per coordinate.
//! It evaluates by Horner's rule, bounds a span by the centred form about its middle, converts to
//! a single Bezier piece of its own degree on the same parameter, and transforms by mapping a₀ as
//! a point and the others as vectors, which is exact for every affine map.
//!
//! Guarantees:
//!
//! - **Certified.** Enclosures and bounds hold the exact values, in interval arithmetic that is
//!   exact where it can be (`tight`); a derivative of order above the degree is exactly zero.
//! - **Exact NURBS form up to rounding.** The Bezier form of the same polynomial, with only the
//!   rounding of its control points in the deviation (`crate::taylor`, whose remainder is zero for
//!   a polynomial).
//! - **Exact split and reversal.** A split changes the domain only; reversal negates the odd
//!   coefficients and the domain.
//!
//! Method: Horner's rule and its derivatives (D. E. Knuth, *The Art of Computer Programming*,
//! vol. 2, 3rd ed., section 4.6.4), and the centred form of a polynomial over an interval
//! (R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to Interval Analysis*, SIAM (2009),
//! <https://doi.org/10.1137/1.9780898717716>, the centred or Taylor form).
//!
//! Cost: O(n·D) per evaluation and derivative order; O(n²·D) per bound.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': a the coefficients, n the degree, k an order, t a parameter, m the middle, x the offset from it"
)]

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit, MAX_DEGREE, NurbsError};
use geom_run::WorkMeter;

use crate::error::CurveError;
use crate::form::{self, count};
use crate::map::NurbsForm;
use crate::moved::Motion;
use crate::param::{CurveKind, DerivativeBound, ParamCurve};
use crate::taylor;
use crate::tight::{self, point};

/// A polynomial curve (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Polynomial<const D: usize> {
    coefficients: Vec<[f64; D]>,
    domain: (f64, f64),
}

/// j·(j − 1)·…·(j − k + 1) in doubles.
fn falling(j: usize, k: usize) -> Result<f64, CurveError> {
    let mut f = 1.0;
    for i in 0..k {
        f *= count(j.checked_sub(i).ok_or(CurveError::Defect)?)?;
    }
    Ok(f)
}

/// j·(j − 1)·…·(j − k + 1) enclosed.
fn falling_enclosed(j: usize, k: usize) -> Result<Interval, CurveError> {
    let mut f = point(1.0)?;
    for i in 0..k {
        f = tight::mul(
            f,
            point(count(j.checked_sub(i).ok_or(CurveError::Defect)?)?)?,
        )?;
    }
    Ok(f)
}

impl<const D: usize> Polynomial<D> {
    /// The polynomial with coefficients a₀ … aₙ over `domain`.
    ///
    /// # Errors
    ///
    /// [`CurveError::Nurbs`] with [`NurbsError::Degree`] unless 1 ≤ n ≤ `MAX_DEGREE`;
    /// [`CurveError::NotFinite`]; [`CurveError::SpanOutside`] unless the domain is finite and
    /// ordered.
    pub fn new(
        coefficients: Vec<[f64; D]>,
        domain: (f64, f64),
    ) -> Result<Polynomial<D>, CurveError> {
        let degree = coefficients.len().saturating_sub(1);
        if degree == 0 || degree > MAX_DEGREE {
            return Err(CurveError::Nurbs(NurbsError::Degree { degree }));
        }
        if !coefficients.iter().flatten().all(|x| x.is_finite()) {
            return Err(CurveError::NotFinite);
        }
        form::check_domain(domain)?;
        Ok(Polynomial {
            coefficients,
            domain,
        })
    }

    /// The coefficients a₀ … aₙ.
    #[must_use]
    pub fn coefficients(&self) -> &[[f64; D]] {
        &self.coefficients
    }

    /// The degree n.
    #[must_use]
    pub fn degree(&self) -> usize {
        self.coefficients.len().saturating_sub(1)
    }

    /// P⁽ᵏ⁾(t) in doubles, by Horner's rule.
    fn value(&self, t: f64, k: usize) -> Result<[f64; D], CurveError> {
        let mut s = [0.0; D];
        for (j, a) in self.coefficients.iter().enumerate().skip(k).rev() {
            let f = falling(j, k)?;
            for (x, &ai) in s.iter_mut().zip(a) {
                *x = *x * t + f * ai;
            }
        }
        if s.iter().all(|x| x.is_finite()) {
            Ok(s)
        } else {
            Err(CurveError::NotFinite)
        }
    }

    /// P⁽ᵏ⁾ at every parameter of `t`, enclosed by Horner's rule in intervals.
    fn horner(&self, t: Interval, k: usize) -> Result<[Interval; D], CurveError> {
        let zero = point(0.0)?;
        let mut s = [zero; D];
        for (j, a) in self.coefficients.iter().enumerate().skip(k).rev() {
            let f = falling_enclosed(j, k)?;
            for (x, &ai) in s.iter_mut().zip(a) {
                *x = tight::add(tight::mul(*x, t)?, tight::mul(f, point(ai)?)?)?;
            }
        }
        Ok(s)
    }

    /// P⁽ᵏ⁾ over the span [lo, hi], enclosed by the centred form about its middle m:
    /// Σᵢ P⁽ᵏ⁺ⁱ⁾(m)/i!·(T − m)^i, by Horner's rule in T − m.
    fn centred(&self, (lo, hi): (f64, f64), k: usize) -> Result<[Interval; D], CurveError> {
        let zero = point(0.0)?;
        let n = self.degree();
        if k > n {
            return Ok([zero; D]);
        }
        if lo == hi {
            return self.horner(point(lo)?, k);
        }
        let m = f64::midpoint(lo, hi);
        let x = tight::sub(Interval::new(lo, hi)?, point(m)?)?;
        let at = point(m)?;
        let mut s = [zero; D];
        let mut factorial = point(1.0)?;
        let mut terms = Vec::with_capacity(n.saturating_sub(k).saturating_add(1));
        for i in 0..=n.saturating_sub(k) {
            if i > 1 {
                factorial = tight::mul(factorial, point(count(i)?)?)?;
            }
            let mut d = self.horner(at, k.checked_add(i).ok_or(CurveError::Defect)?)?;
            for v in &mut d {
                *v = tight::div(*v, factorial)?;
            }
            terms.push(d);
        }
        for d in terms.iter().rev() {
            for (v, &di) in s.iter_mut().zip(d) {
                *v = tight::add(tight::mul(*v, x)?, di)?;
            }
        }
        Ok(s)
    }

    /// The box of `centred`.
    fn hull(
        &self,
        span: (f64, f64),
        k: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Bounds<D>, CurveError> {
        crate::nurbs::check_span(span, self.domain)?;
        meter.charge(u64::try_from(self.coefficients.len()).map_err(|_| CurveError::Defect)?)?;
        let e = self.centred(span, k)?;
        Ok(Bounds {
            lo: e.map(Interval::lo),
            hi: e.map(Interval::hi),
        })
    }
}

impl<const D: usize> ParamCurve<D> for Polynomial<D> {
    fn domain(&self) -> (f64, f64) {
        self.domain
    }

    fn point(&self, t: f64, _side: Limit) -> Result<[f64; D], CurveError> {
        form::check_parameter(self.domain, t)?;
        self.value(t, 0)
    }

    fn derivatives(&self, t: f64, _side: Limit, out: &mut [[f64; D]]) -> Result<(), CurveError> {
        form::check_parameter(self.domain, t)?;
        for (k, o) in out.iter_mut().enumerate() {
            *o = self.value(t, k)?;
        }
        Ok(())
    }

    fn enclose(
        &self,
        t: f64,
        _side: Limit,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; D]>, CurveError> {
        form::check_parameter(self.domain, t)?;
        meter.charge(
            u64::try_from(order)
                .map_err(|_| CurveError::Defect)?
                .saturating_add(1),
        )?;
        let at = point(t)?;
        let mut out = Vec::with_capacity(order.saturating_add(1));
        for k in 0..=order {
            out.push(self.horner(at, k)?);
        }
        Ok(out)
    }

    fn bound(&self, span: (f64, f64), meter: &mut WorkMeter<'_>) -> Result<Bounds<D>, CurveError> {
        self.hull(span, 0, meter)
    }

    fn derivative_bound(
        &self,
        span: (f64, f64),
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<D>, CurveError> {
        Ok(DerivativeBound::Bounded(self.hull(span, order, meter)?))
    }

    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<NurbsForm<D>>, CurveError> {
        taylor::same_parameter(self, self.degree(), f64::INFINITY, meter)
    }

    fn split(
        &self,
        t: f64,
        _meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<CurveKind<D>>, Changed<CurveKind<D>>), CurveError> {
        let (left, right) = form::split_domain(self.domain, t)?;
        let part = |domain| Changed {
            value: CurveKind::Polynomial(Polynomial {
                domain,
                ..self.clone()
            }),
            deviation: 0.0,
        };
        Ok((part(left), part(right)))
    }

    fn reversed(&self, _meter: &mut WorkMeter<'_>) -> Result<Changed<CurveKind<D>>, CurveError> {
        let (a, b) = self.domain;
        let coefficients = self
            .coefficients
            .iter()
            .enumerate()
            .map(|(j, c)| if j & 1 == 1 { c.map(|x| -x) } else { *c })
            .collect();
        Ok(Changed {
            value: CurveKind::Polynomial(Polynomial::new(coefficients, (-b, -a))?),
            deviation: 0.0,
        })
    }

    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<CurveKind<D>>, CurveError> {
        let m = Motion::of::<D>(transform)?;
        meter.charge(u64::try_from(self.coefficients.len()).map_err(|_| CurveError::Defect)?)?;
        let (a, b) = self.domain;
        let reach = point(tight::larger(a.abs(), b.abs()))?;
        let mut coefficients = Vec::with_capacity(self.coefficients.len());
        let mut deviation = point(0.0)?;
        let mut power = point(1.0)?;
        for (j, c) in self.coefficients.iter().enumerate() {
            let is_point = j == 0;
            let moved = if is_point {
                m.point(*c)?
            } else {
                m.vector(*c)?
            };
            let mut exact = [point(0.0)?; D];
            for (e, &x) in exact.iter_mut().zip(c) {
                *e = point(x)?;
            }
            let image = m.enclosed(&exact, is_point)?;
            let d = tight::distance_to(&moved, &image)?;
            if d != 0.0 {
                deviation = tight::add(deviation, tight::mul(power, point(d)?)?)?;
            }
            power = tight::mul(power, reach)?;
            coefficients.push(moved);
        }
        Ok(Changed {
            value: CurveKind::Polynomial(Polynomial::new(coefficients, self.domain)?),
            deviation: deviation.hi(),
        })
    }
}
