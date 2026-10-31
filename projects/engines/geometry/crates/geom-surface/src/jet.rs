//! Truncated two-variable Taylor series in interval arithmetic: every partial derivative of a
//! formula built from surfaces' partials, enclosed, without differentiating the formula by hand.
//!
//! What it does: a [`Jet`] of total order K holds enclosures of the Taylor coefficients
//! f^(i, j)/(i!·j!), where f^(i, j) = ∂^{i+j}f/∂u^i∂v^j and i + j ≤ K, at every point of a box (a
//! point, for an enclosure; a box, for a bound), listed as [`crate::param::partial_index`] lists
//! partials. Sums, products, quotients and square roots of jets are the jets of the sums,
//! products, quotients and roots, by the two-variable recurrences of automatic differentiation;
//! [`Jet::du`] and [`Jet::dv`] differentiate. A quotient whose value is known from elsewhere (a
//! rational patch's convex hull, a unit vector's [−1, 1]) starts from that value
//! ([`Jet::div_valued`], piece G68), so its derivatives do not inherit the width of the value's
//! quotient of boxes. So the jet of an offset surface's formula,
//! S + d·(`S_u` × `S_v`)/|`S_u` × `S_v`|, gives all of its partials to order K from the base's partials
//! to order K + 1: its higher derivatives come from the formula's exact rules (Leibniz's rule and
//! the quotient and root recurrences), never from a finite difference.
//!
//! Guarantees:
//!
//! - **Certified.** If each input coefficient encloses its function's coefficient at every point
//!   of the box, so does each output coefficient: every recurrence is a polynomial or a quotient
//!   in the inputs, evaluated in outward-rounded interval arithmetic (inclusion isotonicity).
//! - **Refused where undefined.** A quotient by, or a root of, a jet whose value may be zero (or,
//!   for the root, not positive) is [`CurveError::Unbounded`], never a guess; an order above 18,
//!   where the factorials stop being exact doubles, is [`NurbsError::DerivativeOrder`].
//!
//! Method: the Taylor-coefficient recurrences of automatic differentiation in several variables
//! (A. Griewank and A. Walther, *Evaluating Derivatives*, 2nd ed., SIAM (2008), chapter 13,
//! <https://doi.org/10.1137/1.9780898717761>: the Cauchy product of multivariate Taylor series and
//! the division and square-root recurrences it gives), evaluated in intervals as in interval
//! automatic differentiation (R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to
//! Interval Analysis*, SIAM (2009), <https://doi.org/10.1137/1.9780898717716>); the one-variable
//! form is `geom-curve`'s jet, which this extends to two parameters.
//!
//! Cost: O(K²) for a sum; O(K⁴) for a product, quotient or root (each coefficient sums over the
//! coefficients below it); K is at most a few in the kernel's uses.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the recurrences': f and g the jets, a, b, q and r their coefficients, i, j, k and l orders"
)]

use alloc::vec::Vec;

use geom_curve::CurveError;
use geom_curve::tight::{self, point};
use geom_num::interval::Interval;
use geom_nurbs::NurbsError;

use crate::param::partial_index;

/// The highest order a jet holds: n! is a double exactly up to 18! (< 2^53, and every partial
/// product is a whole number below it).
pub(crate) const MAX_ORDER: usize = 18;

/// n!, exact.
///
/// # Errors
///
/// [`NurbsError::DerivativeOrder`] above [`MAX_ORDER`].
fn factorial(n: usize) -> Result<f64, CurveError> {
    if n > MAX_ORDER {
        return Err(NurbsError::DerivativeOrder { order: n }.into());
    }
    let mut f = 1.0;
    for k in 2..=n {
        f *= whole(k)?;
    }
    Ok(f)
}

/// The whole number k as a double (k is an order, a few at most).
fn whole(k: usize) -> Result<f64, CurveError> {
    u32::try_from(k)
        .map(f64::from)
        .map_err(|_| CurveError::Defect)
}

/// The number of coefficients of a jet of total order k: (k + 1)(k + 2)/2.
fn size(k: usize) -> Result<usize, CurveError> {
    partial_index(k.checked_add(1).ok_or(CurveError::Defect)?, 0)
}

/// Enclosures of the Taylor coefficients f^(i, j)/(i!·j!), i + j ≤ K (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Jet {
    order: usize,
    c: Vec<Interval>,
}

impl Jet {
    /// The jet of a constant, to order `k`.
    pub(crate) fn constant(x: Interval, k: usize) -> Result<Jet, CurveError> {
        let mut c = alloc::vec![point(0.0)?; size(k)?];
        if let Some(first) = c.first_mut() {
            *first = x;
        }
        Ok(Jet { order: k, c })
    }

    /// The jet of order `k` whose partials are `d` (listed by `partial_index`; d must hold at least
    /// the partials to order k): c^(i, j) = d^(i, j)/(i!·j!).
    pub(crate) fn from_partials(d: &[Interval], k: usize) -> Result<Jet, CurveError> {
        let mut c = Vec::with_capacity(size(k)?);
        for n in 0..=k {
            for j in 0..=n {
                let i = n.checked_sub(j).ok_or(CurveError::Defect)?;
                let x = *d.get(partial_index(i, j)?).ok_or(CurveError::Defect)?;
                let f = tight::mul(point(factorial(i)?)?, point(factorial(j)?)?)?;
                c.push(tight::div(x, f)?);
            }
        }
        Ok(Jet { order: k, c })
    }

    /// The partials f^(i, j) = i!·j!·c^(i, j), enclosed, listed by `partial_index`.
    pub(crate) fn partials(&self) -> Result<Vec<Interval>, CurveError> {
        let mut out = Vec::with_capacity(self.c.len());
        for n in 0..=self.order {
            for j in 0..=n {
                let i = n.checked_sub(j).ok_or(CurveError::Defect)?;
                let f = tight::mul(point(factorial(i)?)?, point(factorial(j)?)?)?;
                out.push(tight::mul(self.get(i, j)?, f)?);
            }
        }
        Ok(out)
    }

    /// The coefficient c^(i, j).
    pub(crate) fn get(&self, i: usize, j: usize) -> Result<Interval, CurveError> {
        self.c
            .get(partial_index(i, j)?)
            .copied()
            .ok_or(CurveError::Defect)
    }

    /// The jet cut to order `k` (at most its own).
    pub(crate) fn truncate(&self, k: usize) -> Result<Jet, CurveError> {
        let order = k.min(self.order);
        Ok(Jet {
            order,
            c: self.c.iter().take(size(order)?).copied().collect(),
        })
    }

    /// The jet of ∂f/∂u (`along_u`) or ∂f/∂v, one order lower (a jet of order 0 gives the zero jet
    /// of order 0): c′^(i, j) = (i + 1)·c^(i + 1, j), or (j + 1)·c^(i, j + 1).
    fn derivative(&self, along_u: bool) -> Result<Jet, CurveError> {
        let Some(order) = self.order.checked_sub(1) else {
            return Jet::constant(point(0.0)?, 0);
        };
        let mut c = Vec::with_capacity(size(order)?);
        for n in 0..=order {
            for j in 0..=n {
                let i = n.checked_sub(j).ok_or(CurveError::Defect)?;
                let up = |x: usize| x.checked_add(1).ok_or(CurveError::Defect);
                let (factor, x) = if along_u {
                    (up(i)?, self.get(up(i)?, j)?)
                } else {
                    (up(j)?, self.get(i, up(j)?)?)
                };
                c.push(tight::mul(point(whole(factor)?)?, x)?);
            }
        }
        Ok(Jet { order, c })
    }

    /// ∂f/∂u (module documentation).
    pub(crate) fn du(&self) -> Result<Jet, CurveError> {
        self.derivative(true)
    }

    /// ∂f/∂v (module documentation).
    pub(crate) fn dv(&self) -> Result<Jet, CurveError> {
        self.derivative(false)
    }

    /// The common order of two jets: they must have the same.
    fn same(&self, g: &Jet) -> Result<usize, CurveError> {
        if self.order == g.order {
            Ok(self.order)
        } else {
            Err(CurveError::Defect)
        }
    }

    /// f + g.
    pub(crate) fn add(&self, g: &Jet) -> Result<Jet, CurveError> {
        let order = self.same(g)?;
        let mut c = Vec::with_capacity(self.c.len());
        for (&a, &b) in self.c.iter().zip(&g.c) {
            c.push(tight::add(a, b)?);
        }
        Ok(Jet { order, c })
    }

    /// f − g.
    pub(crate) fn sub(&self, g: &Jet) -> Result<Jet, CurveError> {
        let order = self.same(g)?;
        let mut c = Vec::with_capacity(self.c.len());
        for (&a, &b) in self.c.iter().zip(&g.c) {
            c.push(tight::sub(a, b)?);
        }
        Ok(Jet { order, c })
    }

    /// x·f for an enclosed constant x.
    pub(crate) fn scale(&self, x: Interval) -> Result<Jet, CurveError> {
        let mut c = Vec::with_capacity(self.c.len());
        for &a in &self.c {
            c.push(tight::mul(x, a)?);
        }
        Ok(Jet {
            order: self.order,
            c,
        })
    }

    /// Σ a^(i, j)·b^(k − i, l − j) over i ≤ k and j ≤ l, leaving out the terms `skip` names (the
    /// first factor's (i, j)).
    fn convolve(
        a: &Jet,
        b: &[Interval],
        (k, l): (usize, usize),
        skip: impl Fn(usize, usize) -> bool,
    ) -> Result<Interval, CurveError> {
        let mut s = point(0.0)?;
        for i in 0..=k {
            for j in 0..=l {
                if skip(i, j) {
                    continue;
                }
                let (ki, lj) = (
                    k.checked_sub(i).ok_or(CurveError::Defect)?,
                    l.checked_sub(j).ok_or(CurveError::Defect)?,
                );
                let y = *b.get(partial_index(ki, lj)?).ok_or(CurveError::Defect)?;
                s = tight::add(s, tight::mul(a.get(i, j)?, y)?)?;
            }
        }
        Ok(s)
    }

    /// f·g: c^(k, l) = Σ f^(i, j)·g^(k − i, l − j).
    pub(crate) fn mul(&self, g: &Jet) -> Result<Jet, CurveError> {
        let order = self.same(g)?;
        let mut c = Vec::with_capacity(self.c.len());
        for n in 0..=order {
            for l in 0..=n {
                let k = n.checked_sub(l).ok_or(CurveError::Defect)?;
                c.push(Jet::convolve(self, &g.c, (k, l), |_, _| false)?);
            }
        }
        Ok(Jet { order, c })
    }

    /// f / g: q^(k, l) = (f^(k, l) − Σ_{(i, j) ≠ (0, 0)} g^(i, j)·q^(k − i, l − j)) / g^(0, 0),
    /// coefficient by coefficient in the order of `partial_index`, which lists every q the sum
    /// needs before q^(k, l).
    ///
    /// # Errors
    ///
    /// [`CurveError::Unbounded`] if g's value may be zero.
    pub(crate) fn div(&self, g: &Jet) -> Result<Jet, CurveError> {
        self.quotient(g, None)
    }

    /// f / g with the quotient's value known to lie in `value`, an enclosure from elsewhere (a
    /// rational patch's convex hull, or [−1, 1] for a coordinate of a unit vector): as
    /// [`Jet::div`], with q^(0, 0) cut to `value`. Every other coefficient takes q^(0, 0) through
    /// the recurrence, so the cut narrows them all, and each still holds its exact value.
    ///
    /// # Errors
    ///
    /// As [`Jet::div`].
    pub(crate) fn div_valued(&self, g: &Jet, value: Interval) -> Result<Jet, CurveError> {
        self.quotient(g, Some(value))
    }

    /// f / g, q^(0, 0) cut to `value` if given ([`Jet::div`], [`Jet::div_valued`]).
    fn quotient(&self, g: &Jet, value: Option<Interval>) -> Result<Jet, CurveError> {
        let order = self.same(g)?;
        let g0 = g.get(0, 0)?;
        if g0.lo() <= 0.0 && g0.hi() >= 0.0 {
            return Err(CurveError::Unbounded);
        }
        let mut q: Vec<Interval> = Vec::with_capacity(self.c.len());
        for n in 0..=order {
            for l in 0..=n {
                let k = n.checked_sub(l).ok_or(CurveError::Defect)?;
                // q^(k, l) is not yet in q; the sum skips (0, 0), the only term that needs it.
                q.push(point(0.0)?);
                let s = Jet::convolve(g, &q, (k, l), |i, j| i == 0 && j == 0)?;
                let mut x = tight::div(tight::sub(self.get(k, l)?, s)?, g0)?;
                if let (0, 0, Some(v)) = (k, l, value) {
                    x = tight::meet(x, v)?;
                }
                if let Some(slot) = q.last_mut() {
                    *slot = x;
                }
            }
        }
        Ok(Jet { order, c: q })
    }

    /// √f: r^(0, 0) = √f^(0, 0), and r^(k, l) = (f^(k, l) − Σ r^(i, j)·r^(k − i, l − j)) /
    /// (2·r^(0, 0)), the sum over every (i, j) but (0, 0) and (k, l).
    ///
    /// # Errors
    ///
    /// [`CurveError::Unbounded`] unless f's value is certainly positive.
    pub(crate) fn sqrt(&self) -> Result<Jet, CurveError> {
        let f0 = self.get(0, 0)?;
        if f0.lo() <= 0.0 {
            return Err(CurveError::Unbounded);
        }
        let r0 = tight::sqrt(f0)?;
        let two_r0 = tight::mul(point(2.0)?, r0)?;
        let mut r = Jet {
            order: self.order,
            c: Vec::with_capacity(self.c.len()),
        };
        r.c.push(r0);
        for n in 1..=self.order {
            for l in 0..=n {
                let k = n.checked_sub(l).ok_or(CurveError::Defect)?;
                // r^(k, l) is not yet known; the sum skips it and (0, 0), the terms that need it.
                r.c.push(point(0.0)?);
                let s = Jet::convolve(&r, &r.c, (k, l), |i, j| {
                    (i == 0 && j == 0) || (i == k && j == l)
                })?;
                let x = tight::div(tight::sub(self.get(k, l)?, s)?, two_r0)?;
                if let Some(slot) = r.c.last_mut() {
                    *slot = x;
                }
            }
        }
        Ok(r)
    }
}
