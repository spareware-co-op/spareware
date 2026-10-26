//! B-spline bases: a degree and a knot vector, checked once, and the basis functions and their
//! derivatives at a parameter (The NURBS Book, algorithms A2.1 to A2.5).
//!
//! What it does: [`Basis::new`] checks chapter 24.7's rules for a knot vector and keeps it.
//! [`Basis::span`] finds the knot span of a parameter (A2.1); [`Basis::eval`] gives the p + 1
//! functions that do not vanish there (A2.2) and [`Basis::eval_derivs`] their derivatives (A2.3);
//! [`Basis::eval_one`] and [`Basis::eval_one_derivs`] give one function and its derivatives
//! (A2.4, A2.5). At a knot of reduced continuity a function's derivatives differ from the left and
//! from the right, so every evaluation takes a [`Limit`]; at an end of the domain only the side
//! inside it exists, and that side is used whatever the limit asks.
//!
//! Guarantees, with u = 2^−53 and `γ_n` = n·u / (1 − n·u) (Higham's notation):
//!
//! - **Well formed or refused.** A basis has a degree from 1 to [`MAX_DEGREE`], at least 2p + 2
//!   knots, every knot finite and none smaller than the one before, no value repeated more than
//!   p + 1 times, a finite difference between the last knot and the first, and a domain
//!   [`u_p`, `u_(m−p)`] that is more than one value (chapter 24.7). Unclamped knot vectors are
//!   accepted, so periodic curves can be stored as they are (chapter 7).
//! - **Parameters inside the domain only.** A parameter that is NaN, infinite or outside the domain
//!   is [`NurbsError::ParameterOutside`]; `NLib` clamps it to the nearest span in silence. A domain
//!   end is in the last (or first) span with more than one value, found exactly, where `NLib` steps
//!   over equal knots within 10^−12.
//! - **Basis values, near exact.** The values of [`Basis::eval`] and [`Basis::eval_one`] lie in
//!   [0, 1], and each is within `γ_(6p)` · N of the exact value N for the knots and parameter given:
//!   every step of the scheme adds or multiplies quantities that are not negative, and the knot
//!   difference that divides each step is at least the term it divides, so each level adds at most
//!   six roundings, all relative. On a clamped knot vector the values at the ends of the domain
//!   are exactly 1 and 0. [`Basis::eval_derivs`] gives the same bits for the values as
//!   [`Basis::eval`].
//! - **Derivatives, bounded by their magnitudes.** Each derivative of order k ≥ 1 is within
//!   `γ_(6p+2)` · M of the exact derivative, where M is the same scheme run on magnitudes: every
//!   difference of two terms replaced by the sum of their absolute values, with the exact knots
//!   and parameter. Each level of the derivative scheme adds at most four roundings to the terms it
//!   combines and the sums and the final factor p!/(p − k)! at most 2k more, so 6p + 2 covers every
//!   order (the tests check this bound against exact rationals). A derivative that overflows,
//!   which a span far shorter than its neighbours can cause, is [`NurbsError::NotFinite`]; the
//!   functions never return a value that is not finite.
//! - **Deterministic.** Only IEEE 754 addition, subtraction, multiplication and division in the
//!   fixed order written here; no fused multiply-add, no platform maths.
//!
//! Method: the triangular schemes of de Boor and Cox for the basis functions, and Piegl and
//! Tiller's algorithms A2.1 to A2.5 for spans, values and derivatives, as `NLib` implements them,
//! with `NLib`'s division order: each quotient of a knot difference by the sum it belongs to is
//! taken before it multiplies a basis value, so no quotient exceeds 1.
//!
//! - C. de Boor, "On calculating with B-splines", *Journal of Approximation Theory* 6 (1972),
//!   50–62, <https://doi.org/10.1016/0021-9045(72)90080-9>.
//! - M. G. Cox, "The numerical evaluation of B-splines", *IMA Journal of Applied Mathematics* 10
//!   (1972), 134–149, <https://doi.org/10.1093/imamat/10.2.134>.
//! - L. Piegl and W. Tiller, *The NURBS Book*, 2nd ed., Springer (1997), sections 2.2 to 2.5,
//!   <https://doi.org/10.1007/978-3-642-59223-2>.
//! - N. J. Higham, *Accuracy and Stability of Numerical Algorithms*, 2nd ed., SIAM (2002),
//!   lemma 3.1 and section 3.4, <https://doi.org/10.1137/1.9780898718027>.
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_BasisBasic.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated to safe `no_std` Rust from `N_BasisFindSpan`,
//!   `N_BasisEval`, `N_BasisDerivs`, `N_BasisIEval` and `N_BasisIDerivs`; a parameter outside
//!   the domain is an error instead of being clamped, and the domain's ends find their span
//!   exactly instead of stepping over knots within 10^−12; the span search stops by its loop
//!   invariant, without `NLib`'s guard against a repeated midpoint; the end shortcuts of
//!   `N_BasisEval` are dropped, because its scheme gives the same exact values there, and those
//!   of `N_BasisIEval` apply only to a clamped end, where they are exact; the zero-divisor guard
//!   (10^−24) is dropped, because a valid basis never divides by a difference that is not
//!   positive; the factor p!/(p − k)! is a double rather than an integer, which overflowed past
//!   degree 12; the checks of `N_KnotVectorIsValid` are those of chapter 24.7, which also accept
//!   unclamped knots and interior knots repeated p + 1 times; every result that could overflow is
//!   checked.
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: [`Basis::new`] is linear in the number of knots and keeps them (one allocation).
//! [`Basis::span`] is a binary search, O(log m). [`Basis::eval`] and [`Basis::eval_one`] take
//! O(p²) operations, [`Basis::eval_derivs`] O(p² + n·p) for n derivatives, and
//! [`Basis::eval_one_derivs`] O(p² + n²); none of them allocates, and their scratch arrays, sized
//! for [`MAX_DEGREE`], take at most about 14 KiB of stack.

use alloc::vec::Vec;

use crate::NurbsError;
use crate::index::{add, at, get, mul, real, run_end, set, sub};

/// The highest degree a basis may have. Every binomial coefficient C(n, k) with n ≤ 2 · 28 = 56 is
/// a whole number below 2^53 (the largest, C(56, 28), is 7 648 690 600 760 440), so it is exact as
/// a double: degree elevation and reduction, and the products of two bases of this degree (piece
/// G59), compute their coefficients exactly. Chapter 24.7's registry may set a lower limit.
pub const MAX_DEGREE: usize = 28;

/// Room for the p + 1 values of a basis of degree [`MAX_DEGREE`].
pub(crate) const WIDTH: usize = 29;

const _: () = assert!(WIDTH == MAX_DEGREE + 1);

/// Which one-sided limit to take at a knot, where a function's derivatives may jump.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Limit {
    /// The limit from the right: the span [`u_j`, `u_(j+1)`) holds the parameter. A curve is
    /// continuous from the right in this sense, so this is the usual choice.
    #[default]
    FromRight,
    /// The limit from the left: the span (`u_j`, `u_(j+1)`] holds the parameter.
    FromLeft,
}

/// A B-spline basis: a degree p and a knot vector `u_0` ≤ … ≤ `u_m` that meet chapter 24.7's rules.
/// It has n + 1 = m − p functions, defined on the domain [`u_p`, `u_(m−p)`].
#[derive(Clone, Debug, PartialEq)]
pub struct Basis {
    degree: usize,
    knots: Vec<f64>,
}

impl Basis {
    /// The basis of degree `degree` on `knots`.
    ///
    /// # Errors
    ///
    /// The first rule broken, in this order: [`NurbsError::Degree`], [`NurbsError::KnotCount`],
    /// [`NurbsError::KnotNotFinite`], [`NurbsError::KnotDecreasing`],
    /// [`NurbsError::Multiplicity`], [`NurbsError::KnotRange`], [`NurbsError::EmptyDomain`].
    pub fn new(degree: usize, knots: Vec<f64>) -> Result<Basis, NurbsError> {
        if degree == 0 || degree > MAX_DEGREE {
            return Err(NurbsError::Degree { degree });
        }
        let order = add(degree, 1)?;
        let needed = mul(order, 2)?;
        if knots.len() < needed {
            return Err(NurbsError::KnotCount {
                knots: knots.len(),
                needed,
            });
        }
        if let Some(index) = knots.iter().position(|k| !k.is_finite()) {
            return Err(NurbsError::KnotNotFinite { index });
        }
        if let Some(index) = knots.windows(2).position(|w| w.get(1) < w.first()) {
            return Err(NurbsError::KnotDecreasing { index: add(index, 1)? });
        }
        let mut index = 0;
        while index < knots.len() {
            let end = run_end(&knots, index)?;
            let multiplicity = sub(end, index)?;
            if multiplicity > order {
                return Err(NurbsError::Multiplicity {
                    index,
                    multiplicity,
                });
            }
            index = end;
        }
        let first = get(&knots, 0)?;
        let last = get(&knots, sub(knots.len(), 1)?)?;
        if !(last - first).is_finite() {
            return Err(NurbsError::KnotRange);
        }
        let basis = Basis { degree, knots };
        let (lo, hi) = basis.domain();
        if lo >= hi {
            return Err(NurbsError::EmptyDomain);
        }
        Ok(basis)
    }

    /// The degree p.
    #[must_use]
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// The knots `u_0`, …, `u_m`.
    #[must_use]
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// The number of basis functions, n + 1 = m − p.
    #[must_use]
    pub fn count(&self) -> usize {
        // `new` checked m + 1 ≥ 2p + 2, so this never saturates.
        self.knots
            .len()
            .saturating_sub(self.degree)
            .saturating_sub(1)
    }

    /// The domain [`u_p`, `u_(m−p)`].
    #[must_use]
    pub fn domain(&self) -> (f64, f64) {
        let lo = self.knots.get(self.degree).copied().unwrap_or(f64::NAN);
        let hi = self
            .knots
            .get(self.count())
            .copied()
            .unwrap_or(f64::NAN);
        (lo, hi)
    }

    /// Whether each end knot is repeated p + 1 times, so the curve interpolates its end control
    /// points.
    #[must_use]
    pub fn is_clamped(&self) -> bool {
        let p = self.degree;
        let m = self.knots.len().saturating_sub(1);
        self.knots.first() == self.knots.get(p)
            && self.knots.get(m.saturating_sub(p)) == self.knots.last()
    }

    /// How many knots equal `u`.
    #[must_use]
    pub fn multiplicity(&self, u: f64) -> usize {
        self.knots.iter().filter(|&&k| k == u).count()
    }

    /// The index j of the knot span that holds `u`, with `u_j` < `u_(j+1)`: the span
    /// [`u_j`, `u_(j+1)`) for [`Limit::FromRight`], (`u_j`, `u_(j+1)`] for [`Limit::FromLeft`], and at
    /// the ends of the domain the first or last span that is more than one value (A2.1).
    ///
    /// # Errors
    ///
    /// [`NurbsError::ParameterOutside`] if `u` is not in the domain.
    pub fn span(&self, u: f64, limit: Limit) -> Result<usize, NurbsError> {
        let (lo, hi) = self.domain();
        if u.is_nan() || u < lo || u > hi {
            return Err(NurbsError::ParameterOutside);
        }
        let u_ = &self.knots;
        let p = self.degree;
        let top = self.count();
        if u == hi {
            // The last span with u_j < u_(j+1); u_p < u_(m−p), so the loop stops at p or above.
            let mut j = sub(top, 1)?;
            while get(u_, j)? == get(u_, add(j, 1)?)? {
                j = sub(j, 1)?;
            }
            return Ok(j);
        }
        if u == lo {
            let mut j = p;
            while get(u_, add(j, 1)?)? == get(u_, j)? {
                j = add(j, 1)?;
            }
            return Ok(j);
        }
        // Binary search keeping u_low ≤ u < u_high (from the right) or u_low < u ≤ u_high (from
        // the left); u is strictly inside the domain, so both hold at the start.
        let mut low = p;
        let mut high = top;
        while sub(high, low)? > 1 {
            let mid = add(low, sub(high, low)? / 2)?;
            let below = match limit {
                Limit::FromRight => u < get(u_, mid)?,
                Limit::FromLeft => u <= get(u_, mid)?,
            };
            if below {
                high = mid;
            } else {
                low = mid;
            }
        }
        Ok(low)
    }

    /// The p + 1 basis functions that do not vanish at `u`, `N_(j−p)`, …, `N_j`, into `out`, and the
    /// span j (A2.2).
    ///
    /// # Errors
    ///
    /// [`NurbsError::BufferLength`] unless `out` has p + 1 elements;
    /// [`NurbsError::ParameterOutside`] as for [`Basis::span`].
    pub fn eval(&self, u: f64, limit: Limit, out: &mut [f64]) -> Result<usize, NurbsError> {
        let needed = add(self.degree, 1)?;
        if out.len() != needed {
            return Err(NurbsError::BufferLength {
                given: out.len(),
                needed,
            });
        }
        let span = self.span(u, limit)?;
        self.funs(span, u, out)?;
        Ok(span)
    }

    /// A2.2 at a known span: `N_(span−p+i)` into `n[i]`, for i from 0 to p.
    pub(crate) fn funs(&self, span: usize, u: f64, n: &mut [f64]) -> Result<(), NurbsError> {
        let u_ = &self.knots;
        let mut left = [0.0_f64; WIDTH];
        let mut right = [0.0_f64; WIDTH];
        set(n, 0, 1.0)?;
        for k in 1..=self.degree {
            set(&mut left, k, u - get(u_, sub(add(span, 1)?, k)?)?)?;
            set(&mut right, k, get(u_, add(span, k)?)? - u)?;
            let mut saved = 0.0;
            for l in 0..k {
                let r = get(&right, add(l, 1)?)?;
                let le = get(&left, sub(k, l)?)?;
                // Both differences are at least zero and r > 0 or le > 0 (the span is more than
                // one value), so delta > 0 and each quotient lies in [0, 1].
                let delta = r + le;
                let temp = get(n, l)?;
                set(n, l, saved + temp * (r / delta))?;
                saved = temp * (le / delta);
            }
            set(n, k, saved)?;
        }
        Ok(())
    }

    /// The basis functions that do not vanish at `u` and their derivatives up to order `order`,
    /// into `out` row by row: `out[k·(p+1) + i]` is the k-th derivative of `N_(j−p+i)`, and j, the
    /// span, is returned (A2.3). Rows above p are zero.
    ///
    /// # Errors
    ///
    /// [`NurbsError::BufferLength`] unless `out` has (order + 1)(p + 1) elements;
    /// [`NurbsError::ParameterOutside`] as for [`Basis::span`]; [`NurbsError::NotFinite`] if a
    /// derivative overflows.
    pub fn eval_derivs(
        &self,
        u: f64,
        limit: Limit,
        order: usize,
        out: &mut [f64],
    ) -> Result<usize, NurbsError> {
        let needed = mul(add(order, 1)?, add(self.degree, 1)?)?;
        if out.len() != needed {
            return Err(NurbsError::BufferLength {
                given: out.len(),
                needed,
            });
        }
        let span = self.span(u, limit)?;
        self.ders(span, u, order, out)?;
        Ok(span)
    }

    /// A2.3 at a known span; `out` holds at least (order + 1)(p + 1) values.
    pub(crate) fn ders(
        &self,
        span: usize,
        u: f64,
        order: usize,
        out: &mut [f64],
    ) -> Result<(), NurbsError> {
        let p = self.degree;
        let w = add(p, 1)?;
        // ndu[j][r]: below the diagonal the knot differences, on and above it the basis values
        // of each degree (NLib's layout).
        let mut ndu = [0.0_f64; WIDTH * WIDTH];
        let at_ = |j: usize, r: usize| -> Result<usize, NurbsError> { add(mul(j, WIDTH)?, r) };
        let u_ = &self.knots;
        let mut left = [0.0_f64; WIDTH];
        let mut right = [0.0_f64; WIDTH];
        set(&mut ndu, 0, 1.0)?;
        for j in 1..=p {
            set(&mut left, j, u - get(u_, sub(add(span, 1)?, j)?)?)?;
            set(&mut right, j, get(u_, add(span, j)?)? - u)?;
            let mut saved = 0.0;
            for r in 0..j {
                let ri = get(&right, add(r, 1)?)?;
                let le = get(&left, sub(j, r)?)?;
                let delta = ri + le;
                set(&mut ndu, at_(j, r)?, delta)?;
                let prev = get(&ndu, at_(r, sub(j, 1)?)?)?;
                set(&mut ndu, at_(r, j)?, saved + prev * (ri / delta))?;
                saved = prev * (le / delta);
            }
            set(&mut ndu, at_(j, j)?, saved)?;
        }
        let rows = add(order, 1)?;
        let total = mul(rows, w)?;
        for v in out.get_mut(..total).ok_or(NurbsError::Defect)? {
            *v = 0.0;
        }
        for j in 0..=p {
            set(out, j, get(&ndu, at_(j, p)?)?)?;
        }
        let top = if order < p { order } else { p };
        for r in 0..=p {
            derivative_column(&ndu, p, r, top, w, out)?;
        }
        // The factor p!/(p − k)!: exact while below 2^53, and within (k − 1) roundings beyond.
        let mut factor = real(p)?;
        for k in 1..=top {
            for j in 0..w {
                *at(out, add(mul(k, w)?, j)?)? *= factor;
            }
            factor *= real(sub(p, k)?)?;
        }
        if out
            .get(..total)
            .ok_or(NurbsError::Defect)?
            .iter()
            .all(|v| v.is_finite())
        {
            Ok(())
        } else {
            Err(NurbsError::NotFinite)
        }
    }

    /// The basis function `N_i` at `u` (A2.4).
    ///
    /// # Errors
    ///
    /// [`NurbsError::Index`] if `i` is not below [`Basis::count`];
    /// [`NurbsError::ParameterOutside`] as for [`Basis::span`].
    pub fn eval_one(&self, i: usize, u: f64, limit: Limit) -> Result<f64, NurbsError> {
        let limit = self.check_one(i, u, limit)?;
        let p = self.degree;
        let (lo, hi) = self.domain();
        // NLib's end values, kept where they are exact: at a clamped end only the end function
        // is not zero, and it is 1.
        if u == lo && get(&self.knots, 0)? == lo {
            return Ok(if i == 0 { 1.0 } else { 0.0 });
        }
        let last = sub(self.count(), 1)?;
        if u == hi && get(&self.knots, sub(self.knots.len(), 1)?)? == hi {
            return Ok(if i == last { 1.0 } else { 0.0 });
        }
        let mut na = [0.0_f64; WIDTH];
        if !self.degree_zero(i, u, limit, &mut na)? {
            return Ok(0.0);
        }
        let u_ = &self.knots;
        let ui = get(u_, i)?;
        for k in 1..=p {
            let n0 = get(&na, 0)?;
            let mut saved = if n0 == 0.0 {
                0.0
            } else {
                ((u - ui) * n0) / (get(u_, add(i, k)?)? - ui)
            };
            for j in 0..sub(add(p, 1)?, k)? {
                let ur = get(u_, add(add(i, j)?, add(k, 1)?)?)?;
                let ul = get(u_, add(add(i, j)?, 1)?)?;
                let next = get(&na, add(j, 1)?)?;
                if next == 0.0 {
                    set(&mut na, j, saved)?;
                    saved = 0.0;
                } else {
                    let temp = next / (ur - ul);
                    set(&mut na, j, saved + (ur - u) * temp)?;
                    saved = (u - ul) * temp;
                }
            }
        }
        get(&na, 0)
    }

    /// The basis function `N_i` at `u` and its derivatives, into `out`: `out[k]` is the k-th
    /// derivative, for k from 0 to `out.len() − 1` (A2.5). Orders above p are zero.
    ///
    /// # Errors
    ///
    /// [`NurbsError::BufferLength`] if `out` is empty; [`NurbsError::Index`] and
    /// [`NurbsError::ParameterOutside`] as for [`Basis::eval_one`]; [`NurbsError::NotFinite`] if
    /// a derivative overflows.
    pub fn eval_one_derivs(
        &self,
        i: usize,
        u: f64,
        limit: Limit,
        out: &mut [f64],
    ) -> Result<(), NurbsError> {
        if out.is_empty() {
            return Err(NurbsError::BufferLength {
                given: 0,
                needed: 1,
            });
        }
        let limit = self.check_one(i, u, limit)?;
        for v in out.iter_mut() {
            *v = 0.0;
        }
        let p = self.degree;
        // nt[j][k], j the function's offset and k its degree (NLib's table).
        let mut nt = [0.0_f64; WIDTH * WIDTH];
        let at_ = |j: usize, k: usize| -> Result<usize, NurbsError> { add(mul(j, WIDTH)?, k) };
        let mut col = [0.0_f64; WIDTH];
        if !self.degree_zero(i, u, limit, &mut col)? {
            return Ok(());
        }
        for j in 0..=p {
            set(&mut nt, at_(j, 0)?, get(&col, j)?)?;
        }
        let u_ = &self.knots;
        let ui = get(u_, i)?;
        for k in 1..=p {
            let n0 = get(&nt, at_(0, sub(k, 1)?)?)?;
            let mut saved = if n0 == 0.0 {
                0.0
            } else {
                ((u - ui) * n0) / (get(u_, add(i, k)?)? - ui)
            };
            for j in 0..sub(add(p, 1)?, k)? {
                let ur = get(u_, add(add(i, j)?, add(k, 1)?)?)?;
                let ul = get(u_, add(add(i, j)?, 1)?)?;
                let next = get(&nt, at_(add(j, 1)?, sub(k, 1)?)?)?;
                if next == 0.0 {
                    set(&mut nt, at_(j, k)?, saved)?;
                    saved = 0.0;
                } else {
                    let temp = next / (ur - ul);
                    set(&mut nt, at_(j, k)?, saved + (ur - u) * temp)?;
                    saved = (u - ul) * temp;
                }
            }
        }
        set(out, 0, get(&nt, at_(0, p)?)?)?;
        let top = sub(out.len(), 1)?;
        let top = if top < p { top } else { p };
        let mut nd = [0.0_f64; WIDTH];
        for k in 1..=top {
            for j in 0..=k {
                set(&mut nd, j, get(&nt, at_(j, sub(p, k)?)?)?)?;
            }
            for l in 1..=k {
                let n0 = get(&nd, 0)?;
                let mut saved = if n0 == 0.0 {
                    0.0
                } else {
                    n0 / (get(u_, add(sub(add(i, p)?, k)?, l)?)? - ui)
                };
                let factor = real(add(sub(p, k)?, l)?)?;
                for j in 0..sub(add(k, 1)?, l)? {
                    let ur = get(u_, add(add(sub(add(p, l)?, k)?, add(i, j)?)?, 1)?)?;
                    let ul = get(u_, add(add(i, j)?, 1)?)?;
                    let next = get(&nd, add(j, 1)?)?;
                    if next == 0.0 {
                        set(&mut nd, j, factor * saved)?;
                        saved = 0.0;
                    } else {
                        let temp = next / (ur - ul);
                        set(&mut nd, j, factor * (saved - temp))?;
                        saved = temp;
                    }
                }
            }
            set(out, k, get(&nd, 0)?)?;
        }
        if out.iter().all(|v| v.is_finite()) {
            Ok(())
        } else {
            Err(NurbsError::NotFinite)
        }
    }

    /// The checks of A2.4 and A2.5, and the limit to use: at the start of the domain only the
    /// limit from the right exists, at the end only the limit from the left.
    fn check_one(&self, i: usize, u: f64, limit: Limit) -> Result<Limit, NurbsError> {
        let count = self.count();
        if i >= count {
            return Err(NurbsError::Index { index: i, count });
        }
        let (lo, hi) = self.domain();
        if u.is_nan() || u < lo || u > hi {
            return Err(NurbsError::ParameterOutside);
        }
        Ok(if u == lo {
            Limit::FromRight
        } else if u == hi {
            Limit::FromLeft
        } else {
            limit
        })
    }

    /// The degree-zero functions `N_(i+j,0)`, j from 0 to p, at `u` from the side `limit`, into
    /// `col`; false when `N_i` vanishes at `u` (outside its support), and `col` is then unused.
    fn degree_zero(
        &self,
        i: usize,
        u: f64,
        limit: Limit,
        col: &mut [f64],
    ) -> Result<bool, NurbsError> {
        let u_ = &self.knots;
        let p = self.degree;
        let first = get(u_, i)?;
        let end = get(u_, add(add(i, p)?, 1)?)?;
        let inside = |from: f64, to: f64| match limit {
            Limit::FromRight => from <= u && u < to,
            Limit::FromLeft => from < u && u <= to,
        };
        if !inside(first, end) {
            return Ok(false);
        }
        for j in 0..=p {
            let from = get(u_, add(i, j)?)?;
            let to = get(u_, add(add(i, j)?, 1)?)?;
            set(col, j, if inside(from, to) { 1.0 } else { 0.0 })?;
        }
        Ok(true)
    }
}

/// Row k of A2.3's derivative table for the function r, k from 1 to `top`: the coefficients
/// `a_(k,j)` of the differences of lower-degree functions, and their sum against those functions.
fn derivative_column(
    ndu: &[f64],
    p: usize,
    r: usize,
    top: usize,
    w: usize,
    out: &mut [f64],
) -> Result<(), NurbsError> {
    let at_ = |j: usize, c: usize| -> Result<usize, NurbsError> { add(mul(j, WIDTH)?, c) };
    // Two rows of coefficients, the current one at offset s1 and the next at s2.
    let mut coef = [0.0_f64; 2 * WIDTH];
    let (mut s1, mut s2) = (0, WIDTH);
    set(&mut coef, 0, 1.0)?;
    for k in 1..=top {
        let mut sum = 0.0;
        let pk = sub(p, k)?;
        let below = add(pk, 1)?;
        if r >= k {
            let rk = sub(r, k)?;
            let v = get(&coef, s1)? / get(ndu, at_(below, rk)?)?;
            set(&mut coef, s2, v)?;
            sum = v * get(ndu, at_(rk, pk)?)?;
        }
        let j1 = if add(r, 1)? >= k { 1 } else { sub(k, r)? };
        let j2 = if r <= below { sub(k, 1)? } else { sub(p, r)? };
        for j in j1..=j2 {
            // r + j ≥ k, since j ≥ j1.
            let rkj = sub(add(r, j)?, k)?;
            let v = (get(&coef, add(s1, j)?)? - get(&coef, add(s1, sub(j, 1)?)?)?)
                / get(ndu, at_(below, rkj)?)?;
            set(&mut coef, add(s2, j)?, v)?;
            sum += v * get(ndu, at_(rkj, pk)?)?;
        }
        if r <= pk {
            let v = -get(&coef, add(s1, sub(k, 1)?)?)? / get(ndu, at_(below, r)?)?;
            set(&mut coef, add(s2, k)?, v)?;
            sum += v * get(ndu, at_(r, pk)?)?;
        }
        set(out, add(mul(k, w)?, r)?, sum)?;
        core::mem::swap(&mut s1, &mut s2);
    }
    Ok(())
}
