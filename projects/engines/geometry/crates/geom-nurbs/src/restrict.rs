//! Bernstein forms cut down to a sub-span and evaluated at a parameter, in interval arithmetic: the
//! de Casteljau steps the kind contract's bounds and enclosures need (piece G60).
//!
//! What it does: [`BernsteinCurve::restrict`] gives the Bernstein form of a piece over a sub-span
//! [lo, hi] of its domain, and [`BernsteinPatch::restrict`] of a patch over a sub-box, by de
//! Casteljau subdivision at the sub-span's ends, each a local parameter (t − a)/(b − a) enclosed in
//! an interval. [`BernsteinCurve::value_at`] and [`BernsteinPatch::value_at`] give the homogeneous
//! value and the weight at a parameter, the last point of a subdivision there.
//!
//! Guarantees:
//!
//! - **Enclosures of the exact.** Subdivision is linear in the coefficients with weights 1 − s and
//!   s; computed with s an interval holding the exact local parameter, and every operation rounded
//!   outward, each new coefficient holds the exact coefficient of the exact sub-piece (inclusion
//!   monotonicity), so the convex hull bounds of `crate::bernstein` hold for the sub-span.
//! - **Homogeneous.** Rational forms are cut with their weights as a last coordinate, so the
//!   weights of the result are the exact weights of the sub-piece's homogeneous form.
//! - **Typed refusals.** A sub-span outside the domain, unordered, or a single value, is
//!   [`NurbsError::KnotOutside`]; a parameter outside it [`NurbsError::ParameterOutside`].
//!
//! Method: de Casteljau's algorithm and subdivision (The NURBS Book, 2nd ed., section 1.3 and
//! algorithm A1.5, <https://doi.org/10.1007/978-3-642-59223-2>), in interval arithmetic.
//!
//! Cost: O(p²·D) interval operations per subdivision; a patch O(p·q·(p + q)·D); one allocation per
//! row of coefficients.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the book's: s for a local parameter, a and b for a domain, p and q for degrees, w for weights"
)]

use alloc::vec::Vec;

use geom_num::interval::Interval;

use crate::NurbsError;
use crate::arith::Num;
use crate::bernstein::{BernsteinCurve, BernsteinPatch, iv};
use crate::index::{add, get, mul};

/// Coefficients with their weights (1 for a nonrational form).
type Rows<const D: usize> = Vec<([Interval; D], Interval)>;

/// The coefficients of the parts left and right of the local parameter `s`.
fn subdivide<const D: usize>(
    c: &[([Interval; D], Interval)],
    s: Interval,
) -> Result<(Rows<D>, Rows<D>), NurbsError> {
    let one_s = iv(1.0)?.minus(s)?;
    let mut level: Rows<D> = c.to_vec();
    let mut left = Vec::with_capacity(c.len());
    let mut right = Vec::with_capacity(c.len());
    left.push(*level.first().ok_or(NurbsError::Defect)?);
    right.push(*level.last().ok_or(NurbsError::Defect)?);
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().saturating_sub(1));
        for pair in level.windows(2) {
            let [(p0, w0), (p1, w1)] = pair else {
                return Err(NurbsError::Defect);
            };
            let mut p = *p0;
            for (x, (a, b)) in p.iter_mut().zip(p0.iter().zip(p1)) {
                *x = one_s.times(*a)?.plus(s.times(*b)?)?;
            }
            next.push((p, one_s.times(*w0)?.plus(s.times(*w1)?)?));
        }
        left.push(*next.first().ok_or(NurbsError::Defect)?);
        right.push(*next.last().ok_or(NurbsError::Defect)?);
        level = next;
    }
    right.reverse();
    Ok((left, right))
}

/// The coefficients and the weights, apart.
fn split<const D: usize>(c: &[([Interval; D], Interval)]) -> (Vec<[Interval; D]>, Vec<Interval>) {
    let mut points = Vec::with_capacity(c.len());
    let mut weights = Vec::with_capacity(c.len());
    for (p, w) in c {
        points.push(*p);
        weights.push(*w);
    }
    (points, weights)
}

/// The local parameter (t − a)/(b − a), enclosed.
fn local(t: f64, (a, b): (f64, f64)) -> Result<Interval, NurbsError> {
    iv(t)?.minus(iv(a)?)?.over(iv(b)?.minus(iv(a)?)?)
}

/// Coefficients `c` over [a, b] cut down to [lo, hi].
fn cut<const D: usize>(
    c: &[([Interval; D], Interval)],
    (a, b): (f64, f64),
    (lo, hi): (f64, f64),
) -> Result<Rows<D>, NurbsError> {
    let mut out: Rows<D> = c.to_vec();
    if hi < b {
        out = subdivide(&out, local(hi, (a, b))?)?.0;
    }
    if lo > a {
        out = subdivide(&out, local(lo, (a, hi))?)?.1;
    }
    Ok(out)
}

/// A sub-span strictly ordered inside [a, b].
fn check_sub((lo, hi): (f64, f64), (a, b): (f64, f64)) -> Result<(), NurbsError> {
    if lo.is_finite() && hi.is_finite() && a <= lo && lo < hi && hi <= b {
        Ok(())
    } else {
        Err(NurbsError::KnotOutside)
    }
}

/// A parameter inside [a, b].
fn check_in(t: f64, (a, b): (f64, f64)) -> Result<(), NurbsError> {
    if a <= t && t <= b {
        Ok(())
    } else {
        Err(NurbsError::ParameterOutside)
    }
}

/// The coefficients with their weights.
fn with_weights<const D: usize>(
    points: &[[Interval; D]],
    weights: Option<&[Interval]>,
) -> Result<Rows<D>, NurbsError> {
    let one = iv(1.0)?;
    let mut out = Vec::with_capacity(points.len());
    for (i, p) in points.iter().enumerate() {
        let w = match weights {
            Some(ws) => get(ws, i)?,
            None => one,
        };
        out.push((*p, w));
    }
    Ok(out)
}

impl<const D: usize> BernsteinCurve<D> {
    /// The Bernstein form of this piece over the sub-span [lo, hi] of its domain, lo < hi.
    ///
    /// # Errors
    ///
    /// [`NurbsError::KnotOutside`] unless a ≤ lo < hi ≤ b; [`NurbsError::NotFinite`].
    pub fn restrict(&self, span: (f64, f64)) -> Result<BernsteinCurve<D>, NurbsError> {
        check_sub(span, self.domain())?;
        let c = cut(
            &with_weights(self.points(), self.weights())?,
            self.domain(),
            span,
        )?;
        let (points, weights) = split(&c);
        BernsteinCurve::new(span, points, self.is_rational().then_some(weights))
    }

    /// The homogeneous value (w·C for a rational piece) and the weight at `t`, enclosed.
    ///
    /// # Errors
    ///
    /// [`NurbsError::ParameterOutside`] unless t is in the domain; [`NurbsError::NotFinite`].
    pub fn value_at(&self, t: f64) -> Result<([Interval; D], Interval), NurbsError> {
        check_in(t, self.domain())?;
        let c = with_weights(self.points(), self.weights())?;
        let (left, _) = subdivide(&c, local(t, self.domain())?)?;
        left.last().copied().ok_or(NurbsError::Defect)
    }
}

impl<const D: usize> BernsteinPatch<D> {
    /// Row i (fixed u index) or column j of the coefficients with their weights.
    fn line(&self, along_v: bool, k: usize) -> Result<Rows<D>, NurbsError> {
        let (p, q) = self.degrees();
        let one = iv(1.0)?;
        let n = if along_v { q } else { p };
        let mut out = Vec::with_capacity(add(n, 1)?);
        for m in 0..=n {
            let (i, j) = if along_v { (k, m) } else { (m, k) };
            let idx = add(mul(i, add(q, 1)?)?, j)?;
            let w = match self.weights() {
                Some(ws) => get(ws, idx)?,
                None => one,
            };
            out.push((get(self.points(), idx)?, w));
        }
        Ok(out)
    }

    /// The Bernstein form of this patch over the sub-box `u` × `v` of its domain, each side
    /// strictly ordered.
    ///
    /// # Errors
    ///
    /// [`NurbsError::KnotOutside`] unless each side lies strictly ordered inside the domain;
    /// [`NurbsError::NotFinite`].
    pub fn restrict(&self, u: (f64, f64), v: (f64, f64)) -> Result<BernsteinPatch<D>, NurbsError> {
        let (du, dv) = self.domain();
        check_sub(u, du)?;
        check_sub(v, dv)?;
        let (p, q) = self.degrees();
        // Every row (fixed u index) cut in v, then every column of the result cut in u.
        let mut rows = Vec::with_capacity(add(p, 1)?);
        for i in 0..=p {
            rows.push(cut(&self.line(true, i)?, dv, v)?);
        }
        let mut grid: Vec<([Interval; D], Interval)> =
            Vec::with_capacity(mul(add(p, 1)?, add(q, 1)?)?);
        let mut cols = Vec::with_capacity(add(q, 1)?);
        for j in 0..=q {
            let column: Rows<D> = rows.iter().map(|r| get(r, j)).collect::<Result<_, _>>()?;
            cols.push(cut(&column, du, u)?);
        }
        for i in 0..=p {
            for col in &cols {
                grid.push(get(col, i)?);
            }
        }
        let (points, weights) = split(&grid);
        BernsteinPatch::new(
            (u, v),
            (p, q),
            points,
            self.is_rational().then_some(weights),
        )
    }

    /// The homogeneous value (w·S for a rational patch) and the weight at (u, v), enclosed.
    ///
    /// # Errors
    ///
    /// [`NurbsError::ParameterOutside`] unless (u, v) is in the domain; [`NurbsError::NotFinite`].
    pub fn value_at(&self, u: f64, v: f64) -> Result<([Interval; D], Interval), NurbsError> {
        let (du, dv) = self.domain();
        check_in(u, du)?;
        check_in(v, dv)?;
        let (p, _) = self.degrees();
        let sv = local(v, dv)?;
        let mut column = Vec::with_capacity(add(p, 1)?);
        for i in 0..=p {
            let (left, _) = subdivide(&self.line(true, i)?, sv)?;
            column.push(*left.last().ok_or(NurbsError::Defect)?);
        }
        let (left, _) = subdivide(&column, local(u, du)?)?;
        left.last().copied().ok_or(NurbsError::Defect)
    }
}
