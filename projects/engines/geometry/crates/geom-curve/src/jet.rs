//! Truncated Taylor series in interval arithmetic: the derivatives of a formula built from curves'
//! derivatives, enclosed, without differentiating the formula by hand.
//!
//! What it does: a [`Jet`] of order K holds enclosures of f(t), f′(t)/1!, …, f⁽ᴷ⁾(t)/K! at every
//! parameter t of an interval (a point, for an enclosure; a span, for a bound). Sums, products,
//! quotients and square roots of jets are the jets of the sums, products, quotients and roots, by
//! the recurrences of automatic differentiation, so a jet of an offset curve's formula gives its
//! derivatives to order K from the basis curve's derivatives to order K + 1.
//!
//! Guarantees:
//!
//! - **Certified.** If each input coefficient encloses its function's coefficient at every t of
//!   the interval, so does each output coefficient: every recurrence is a polynomial or a quotient
//!   in the inputs, evaluated in interval arithmetic (inclusion isotonicity).
//! - **Refused where undefined.** A quotient by, or a root of, a jet whose value may be zero (or,
//!   for the root, negative) is an error, never a guess.
//!
//! Method: the Taylor-coefficient recurrences of automatic differentiation (A. Griewank and
//! A. Walther, *Evaluating Derivatives*, 2nd ed., SIAM (2008),
//! <https://doi.org/10.1137/1.9780898717761>, on univariate Taylor propagation), evaluated in intervals as in Taylor-model and
//! interval AD methods (R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to Interval
//! Analysis*, SIAM (2009), <https://doi.org/10.1137/1.9780898717716>).
//!
//! Cost: O(K) for a sum, O(K²) for a product, quotient or root.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the recurrences': f, g the jets, c, q, r their coefficients, k, i, j, n orders"
)]

use alloc::vec::Vec;

use geom_num::interval::Interval;

use crate::error::CurveError;
use crate::form::count;
use crate::tight::{self, point};

/// Enclosures of the Taylor coefficients f⁽ᵏ⁾/k!, k = 0 … K (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Jet {
    c: Vec<Interval>,
}

impl Jet {
    /// The jet of a constant, to order `k`.
    pub(crate) fn constant(x: Interval, k: usize) -> Result<Jet, CurveError> {
        let mut c = alloc::vec![point(0.0)?; k.saturating_add(1)];
        if let Some(first) = c.first_mut() {
            *first = x;
        }
        Ok(Jet { c })
    }

    /// The jet whose derivatives are `d` (d[k] encloses f⁽ᵏ⁾): c[k] = d[k]/k!.
    pub(crate) fn from_derivatives(d: &[Interval]) -> Result<Jet, CurveError> {
        let mut c = Vec::with_capacity(d.len());
        let mut f = point(1.0)?;
        for (k, &x) in d.iter().enumerate() {
            if k > 1 {
                f = tight::mul(f, point(count(k)?)?)?;
            }
            c.push(tight::div(x, f)?);
        }
        Ok(Jet { c })
    }

    /// The order K.
    pub(crate) fn order(&self) -> usize {
        self.c.len().saturating_sub(1)
    }

    /// The jet of f′, one order lower: c′[k] = (k + 1)·c[k + 1].
    pub(crate) fn derivative(&self) -> Result<Jet, CurveError> {
        let mut c = Vec::with_capacity(self.order());
        for (k, &x) in self.c.iter().enumerate().skip(1) {
            c.push(tight::mul(point(count(k)?)?, x)?);
        }
        if c.is_empty() {
            c.push(point(0.0)?);
        }
        Ok(Jet { c })
    }

    /// The jet cut to order `k`.
    pub(crate) fn truncate(&self, k: usize) -> Jet {
        Jet {
            c: self.c.iter().take(k.saturating_add(1)).copied().collect(),
        }
    }

    /// The derivatives f⁽ᵏ⁾ = k!·c[k], enclosed.
    pub(crate) fn derivatives(&self) -> Result<Vec<Interval>, CurveError> {
        let mut out = Vec::with_capacity(self.c.len());
        let mut f = point(1.0)?;
        for (k, &x) in self.c.iter().enumerate() {
            if k > 1 {
                f = tight::mul(f, point(count(k)?)?)?;
            }
            out.push(tight::mul(x, f)?);
        }
        Ok(out)
    }

    fn get(&self, k: usize) -> Result<Interval, CurveError> {
        self.c.get(k).copied().ok_or(CurveError::Defect)
    }

    /// f + g.
    pub(crate) fn add(&self, g: &Jet) -> Result<Jet, CurveError> {
        let mut c = Vec::with_capacity(self.c.len());
        for (&a, &b) in self.c.iter().zip(&g.c) {
            c.push(tight::add(a, b)?);
        }
        Ok(Jet { c })
    }

    /// f − g.
    pub(crate) fn sub(&self, g: &Jet) -> Result<Jet, CurveError> {
        let mut c = Vec::with_capacity(self.c.len());
        for (&a, &b) in self.c.iter().zip(&g.c) {
            c.push(tight::sub(a, b)?);
        }
        Ok(Jet { c })
    }

    /// x·f for an enclosed constant x.
    pub(crate) fn scale(&self, x: Interval) -> Result<Jet, CurveError> {
        let mut c = Vec::with_capacity(self.c.len());
        for &a in &self.c {
            c.push(tight::mul(x, a)?);
        }
        Ok(Jet { c })
    }

    /// f·g: c[k] = Σᵢ f[i]·g[k − i].
    pub(crate) fn mul(&self, g: &Jet) -> Result<Jet, CurveError> {
        let n = self.c.len();
        let mut c = Vec::with_capacity(n);
        for k in 0..n {
            let mut s = point(0.0)?;
            for i in 0..=k {
                let j = k.checked_sub(i).ok_or(CurveError::Defect)?;
                s = tight::add(s, tight::mul(self.get(i)?, g.get(j)?)?)?;
            }
            c.push(s);
        }
        Ok(Jet { c })
    }

    /// f / g: q[k] = (f[k] − Σ_{i=1..k} g[i]·q[k − i]) / g[0].
    ///
    /// # Errors
    ///
    /// [`CurveError::Unbounded`] if g's value may be zero.
    pub(crate) fn div(&self, g: &Jet) -> Result<Jet, CurveError> {
        let g0 = g.get(0)?;
        if g0.lo() <= 0.0 && g0.hi() >= 0.0 {
            return Err(CurveError::Unbounded);
        }
        let n = self.c.len();
        let mut q: Vec<Interval> = Vec::with_capacity(n);
        for k in 0..n {
            let mut s = self.get(k)?;
            for i in 1..=k {
                let j = k.checked_sub(i).ok_or(CurveError::Defect)?;
                let qj = q.get(j).copied().ok_or(CurveError::Defect)?;
                s = tight::sub(s, tight::mul(g.get(i)?, qj)?)?;
            }
            q.push(tight::div(s, g0)?);
        }
        Ok(Jet { c: q })
    }

    /// √f: r[0] = √f[0], r[k] = (f[k] − Σ_{i=1..k−1} r[i]·r[k − i]) / (2·r[0]).
    ///
    /// # Errors
    ///
    /// [`CurveError::Unbounded`] unless f's value is certainly positive.
    pub(crate) fn sqrt(&self) -> Result<Jet, CurveError> {
        let f0 = self.get(0)?;
        if f0.lo() <= 0.0 {
            return Err(CurveError::Unbounded);
        }
        let r0 = tight::sqrt(f0)?;
        let two_r0 = tight::mul(point(2.0)?, r0)?;
        let n = self.c.len();
        let mut r: Vec<Interval> = Vec::with_capacity(n);
        r.push(r0);
        for k in 1..n {
            let mut s = self.get(k)?;
            for i in 1..k {
                let j = k.checked_sub(i).ok_or(CurveError::Defect)?;
                let (ri, rj) = (
                    r.get(i).copied().ok_or(CurveError::Defect)?,
                    r.get(j).copied().ok_or(CurveError::Defect)?,
                );
                s = tight::sub(s, tight::mul(ri, rj)?)?;
            }
            r.push(tight::div(s, two_r0)?);
        }
        Ok(Jet { c: r })
    }
}
