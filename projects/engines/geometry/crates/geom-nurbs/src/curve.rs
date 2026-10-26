//! NURBS curves in two and three dimensions: construction under chapter 24.7's rules, points and
//! derivatives (The NURBS Book, algorithms A3.1, A3.2, A4.1 and A4.2), and the control points of
//! derivative curves (A3.3).
//!
//! What it does: a [`Curve`] holds a [`Basis`] and n + 1 control points of dimension D, with
//! weights if it is rational. A rational curve stores its points in homogeneous form, w·P, as the
//! algorithms of The NURBS Book use them, so a curve built from homogeneous points keeps them
//! exactly and knot operations never divide by a weight. [`Curve::point`] sums the basis functions
//! that do not vanish times their control points (A3.1), and for a rational curve divides by the
//! summed weights (A4.1). [`Curve::derivs`] sums the basis derivatives against the points (A3.2);
//! for a rational curve it applies Leibniz's rule to w·C = A (A4.2). [`Curve::derivative_curve`]
//! builds the k-th derivative of a nonrational curve as a curve of degree p − k by differencing
//! the control points (A3.3).
//!
//! Guarantees, with u = 2^−53, `γ_n` = n·u / (1 − n·u), coordinates taken one at a time, N and M
//! the basis values and the derivative magnitudes of [`crate::basis`], and w the exact weights:
//!
//! - **Well formed or refused.** A curve has exactly as many control points (and weights) as its
//!   basis has functions, every coordinate finite, every weight positive and finite, and every
//!   weighted coordinate w·P finite.
//! - **Points.** A nonrational point is within `γ_(7p+1)` · Σ `N_j`·|`P_j`| of the exact point: the
//!   basis values carry `γ_(6p)` each, and one product and p sums add p + 1 roundings. A rational
//!   point is within `γ_(14p+3)` · (Σ `N_j`·|`w_j`·`P_j`|) / (Σ `N_j`·`w_j`): the sums carry `γ_(7p+1)`,
//!   the weight sum relative to itself because every term is positive, and the division one more.
//!   On a clamped curve the ends are the end control points exactly (for a rational curve, the
//!   quotient w·P / w, rounded once).
//! - **Derivatives.** A nonrational derivative of order k ≥ 1 is within `γ_(7p+3)` · Σ `M_j`·|`P_j`|.
//!   A rational derivative of order k is within `γ_e` · `MC_k`, e = (k + 1)(14p + 8 + k), where
//!   `MC_k` = (Σ `M_(k,j)`·|`w_j`·`P_j`| + `Σ_(i=1..k)` C(k, i) · (Σ `M_(i,j)`·`w_j`) · `MC_(k−i)`) / Σ `N_j`·`w_j`
//!   is Leibniz's recurrence run on magnitudes: each step adds the error of the sums, two products,
//!   k subtractions and the division to the error of the lower orders. The value of order 0 has
//!   the same bits as [`Curve::point`].
//! - **Derivative curves.** The k-th derivative curve's control points are each within
//!   `γ_(4k)` of the same differences run on magnitudes (each difference of two points replaced
//!   by the sum of their absolute values); each level adds a rounded knot difference, a quotient,
//!   two products and a sum. Its value at a parameter is the k-th derivative of the curve.
//! - **Typed failures.** A result that overflows is [`NurbsError::NotFinite`]; a derivative curve
//!   across a knot where a lower derivative jumps is [`NurbsError::Discontinuous`], where `NLib`
//!   reports a numerical error; derivative curves of rational curves are
//!   [`NurbsError::Rational`] (their symbolic form needs products of B-splines, piece G59).
//! - **Deterministic.** Fixed summation order (by increasing control point index), no fused
//!   multiply-add, no platform maths.
//!
//! Method: Piegl and Tiller's algorithms A3.1 to A3.3, A4.1 and A4.2 (The NURBS Book, 2nd ed.,
//! sections 3.2, 3.3 and 4.2 to 4.3, <https://doi.org/10.1007/978-3-642-59223-2>), as `NLib`
//! implements the points and derivative curves; the error bounds follow Higham's rounding-error
//! analysis (*Accuracy and Stability of Numerical Algorithms*, 2nd ed., SIAM (2002), chapter 3,
//! <https://doi.org/10.1137/1.9780898718027>).
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvNurb.cpp>
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvAdv.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated to safe `no_std` Rust from `N_CrvEval`, `N_CrvDerivs` and
//!   `N_CrvNonRatGetDerivCrvsAll`, for any dimension D instead of `NLib`'s homogeneous 4D points
//!   with its "no z" and "no w" markers; rational derivatives by the book's A4.2 on the stored
//!   homogeneous points, where `N_CrvDerivs` forms rational basis derivatives against points
//!   divided by their weights; a parameter outside the domain and a derivative order above the
//!   maximum are typed errors; derivative curves keep the knots `u_k` to `u_(m−k)`, so unclamped
//!   curves work too, where `NLib` rebuilds clamped ends; each overflow is checked.
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: construction is linear in the control points and makes one allocation (two if
//! rational). [`Curve::point`] takes O(p²) operations and [`Curve::derivs`] O(p² + k·p·D + k²·D)
//! for k derivatives; neither allocates. [`Curve::derivative_curve`] takes O(k·n·D) operations and
//! one allocation for the points and one for the knots, charged to the meter first.

use alloc::vec::Vec;

use geom_run::WorkMeter;

use crate::basis::{Basis, Limit, MAX_DEGREE, WIDTH};
use crate::budget::{charge, vec_for};
use crate::index::{add, at, get, mul, real, set, sub};
use crate::point::Point;
use crate::NurbsError;

/// A NURBS curve of dimension `D`: a basis and its control points, weighted if rational.
#[derive(Clone, Debug, PartialEq)]
pub struct Curve<const D: usize> {
    basis: Basis,
    /// The control points, as w·P for a rational curve.
    points: Vec<[f64; D]>,
    weights: Option<Vec<f64>>,
}

impl<const D: usize> Curve<D> {
    /// The nonrational curve on `basis` with these control points.
    ///
    /// # Errors
    ///
    /// [`NurbsError::PointCount`] unless there is one point per basis function;
    /// [`NurbsError::PointNotFinite`] for the first point with a coordinate that is not finite.
    pub fn new<P: Point<D>>(basis: Basis, points: &[P]) -> Result<Self, NurbsError> {
        let points = points.iter().map(|p| p.coords()).collect();
        Self::from_homogeneous(basis, points, None)
    }

    /// The rational curve on `basis` with these control points and weights; the points are
    /// stored as w·P, each coordinate rounded once.
    ///
    /// # Errors
    ///
    /// [`NurbsError::PointCount`] unless there is one point and one weight per basis function;
    /// [`NurbsError::Weight`] for the first weight that is not positive and finite;
    /// [`NurbsError::PointNotFinite`] for the first point with a coordinate, or a weighted
    /// coordinate, that is not finite.
    pub fn new_rational<P: Point<D>>(
        basis: Basis,
        points: &[P],
        weights: &[f64],
    ) -> Result<Self, NurbsError> {
        let expected = basis.count();
        for len in [points.len(), weights.len()] {
            if len != expected {
                return Err(NurbsError::PointCount {
                    points: len,
                    expected,
                });
            }
        }
        check_weights(weights)?;
        let mut homogeneous = Vec::with_capacity(points.len());
        for (index, (p, &w)) in points.iter().zip(weights).enumerate() {
            let c = p.coords();
            if !c.iter().all(|x| x.is_finite()) {
                return Err(NurbsError::PointNotFinite { index });
            }
            homogeneous.push(c.map(|x| x * w));
        }
        Self::from_homogeneous(basis, homogeneous, Some(weights.to_vec()))
    }

    /// The curve on `basis` with homogeneous control points `points` (w·P) and `weights`, or the
    /// nonrational curve with control points `points` if `weights` is `None`. Nothing is rounded.
    ///
    /// # Errors
    ///
    /// [`NurbsError::PointCount`] unless there is one point (and one weight) per basis function;
    /// [`NurbsError::Weight`] for the first weight that is not positive and finite;
    /// [`NurbsError::PointNotFinite`] for the first point with a coordinate that is not finite.
    pub fn from_homogeneous(
        basis: Basis,
        points: Vec<[f64; D]>,
        weights: Option<Vec<f64>>,
    ) -> Result<Self, NurbsError> {
        let expected = basis.count();
        if points.len() != expected {
            return Err(NurbsError::PointCount {
                points: points.len(),
                expected,
            });
        }
        if let Some(w) = &weights {
            if w.len() != expected {
                return Err(NurbsError::PointCount {
                    points: w.len(),
                    expected,
                });
            }
            check_weights(w)?;
        }
        if let Some(index) = points
            .iter()
            .position(|p| !p.iter().all(|x| x.is_finite()))
        {
            return Err(NurbsError::PointNotFinite { index });
        }
        Ok(Curve {
            basis,
            points,
            weights,
        })
    }

    /// The basis.
    #[must_use]
    pub fn basis(&self) -> &Basis {
        &self.basis
    }

    /// The degree p.
    #[must_use]
    pub fn degree(&self) -> usize {
        self.basis.degree()
    }

    /// The control points as stored: w·P for a rational curve, P otherwise.
    #[must_use]
    pub fn homogeneous(&self) -> &[[f64; D]] {
        &self.points
    }

    /// The weights, if the curve is rational.
    #[must_use]
    pub fn weights(&self) -> Option<&[f64]> {
        self.weights.as_deref()
    }

    /// Whether the curve is rational.
    #[must_use]
    pub fn is_rational(&self) -> bool {
        self.weights.is_some()
    }

    /// Control point i, `P_i`: for a rational curve w·P / w, each coordinate rounded once.
    ///
    /// # Errors
    ///
    /// [`NurbsError::Index`] if `i` is not below the number of control points.
    pub fn control_point(&self, i: usize) -> Result<[f64; D], NurbsError> {
        let count = self.points.len();
        let p = self
            .points
            .get(i)
            .copied()
            .ok_or(NurbsError::Index { index: i, count })?;
        Ok(match &self.weights {
            Some(w) => {
                let wi = get(w, i)?;
                p.map(|x| x / wi)
            }
            None => p,
        })
    }

    /// The point C(u) (A3.1, A4.1).
    ///
    /// # Errors
    ///
    /// [`NurbsError::ParameterOutside`] if `u` is not in the domain; [`NurbsError::NotFinite`] if
    /// the point overflows.
    pub fn point(&self, u: f64, limit: Limit) -> Result<[f64; D], NurbsError> {
        let p = self.degree();
        let span = self.basis.span(u, limit)?;
        let mut values = [0.0_f64; WIDTH];
        self.basis.funs(span, u, &mut values)?;
        let first = sub(span, p)?;
        let mut sum = [0.0_f64; D];
        let mut weight = 0.0;
        for j in 0..=p {
            let nj = get(&values, j)?;
            let index = add(first, j)?;
            for (sd, pd) in sum.iter_mut().zip(get(&self.points, index)?) {
                *sd += nj * pd;
            }
            if let Some(ws) = &self.weights {
                weight += nj * get(ws, index)?;
            }
        }
        if self.weights.is_some() {
            for sd in &mut sum {
                *sd /= weight;
            }
        }
        finite(sum)
    }

    /// The point and its derivatives at `u`: `out[k]` is the k-th derivative C^(k)(u), for k
    /// from 0 to `out.len() − 1` (A3.2, A4.2).
    ///
    /// # Errors
    ///
    /// [`NurbsError::BufferLength`] if `out` is empty; [`NurbsError::DerivativeOrder`] if it asks
    /// for an order above [`MAX_DEGREE`]; [`NurbsError::ParameterOutside`] if `u` is not in the
    /// domain; [`NurbsError::NotFinite`] if a derivative overflows.
    pub fn derivs(&self, u: f64, limit: Limit, out: &mut [[f64; D]]) -> Result<(), NurbsError> {
        let order = out.len().checked_sub(1).ok_or(NurbsError::BufferLength {
            given: 0,
            needed: 1,
        })?;
        if order > MAX_DEGREE {
            return Err(NurbsError::DerivativeOrder { order });
        }
        let p = self.degree();
        let top = if order < p { order } else { p };
        let span = self.basis.span(u, limit)?;
        let mut nders = [0.0_f64; WIDTH * WIDTH];
        self.basis.ders(span, u, top, &mut nders)?;
        let width = add(p, 1)?;
        let first = sub(span, p)?;
        let mut wders = [0.0_f64; WIDTH];
        for row in out.iter_mut() {
            *row = [0.0; D];
        }
        for k in 0..=top {
            let mut sum = [0.0_f64; D];
            let mut weight = 0.0;
            for j in 0..=p {
                let nk = get(&nders, add(mul(k, width)?, j)?)?;
                let index = add(first, j)?;
                for (sd, pd) in sum.iter_mut().zip(get(&self.points, index)?) {
                    *sd += nk * pd;
                }
                if let Some(ws) = &self.weights {
                    weight += nk * get(ws, index)?;
                }
            }
            set(out, k, sum)?;
            set(&mut wders, k, weight)?;
        }
        if self.weights.is_some() {
            leibniz(out, &wders, top)?;
        }
        for row in out.iter() {
            finite(*row)?;
        }
        Ok(())
    }

    /// The k-th derivative of this nonrational curve, as a curve of degree p − k on the knots
    /// `u_k`, …, `u_(m−k)` (A3.3).
    ///
    /// # Errors
    ///
    /// [`NurbsError::Rational`] for a rational curve; [`NurbsError::Degree`] if k ≥ p (a curve has
    /// degree 1 or more; k = 0 gives the curve itself); [`NurbsError::Discontinuous`] if a lower derivative jumps
    /// at a knot; [`NurbsError::NotFinite`] if a control point overflows;
    /// [`NurbsError::Stopped`] if the meter stops it.
    pub fn derivative_curve(
        &self,
        k: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Curve<D>, NurbsError> {
        if self.is_rational() {
            return Err(NurbsError::Rational);
        }
        let p = self.degree();
        if k == 0 {
            charge(meter, self.points.len(), D, false, self.basis.knots().len())?;
            return Ok(self.clone());
        }
        if k >= p {
            return Err(NurbsError::Degree {
                degree: p.saturating_sub(k),
            });
        }
        let u_ = self.basis.knots();
        let count = self.points.len();
        let knots = sub(u_.len(), mul(k, 2)?)?;
        charge(meter, count, D, false, knots)?;
        let mut tw = vec_for(meter, count)?;
        tw.extend_from_slice(&self.points);
        for level in 1..=k {
            let factor = real(add(sub(p, level)?, 1)?)?;
            for i in 0..=sub(sub(count, 1)?, level)? {
                let den = get(u_, add(add(i, p)?, 1)?)? - get(u_, add(i, level)?)?;
                if den == 0.0 {
                    return Err(NurbsError::Discontinuous);
                }
                let alf = factor / den;
                let next = get(&tw, add(i, 1)?)?;
                let here = at(&mut tw, i)?;
                for (h, n) in here.iter_mut().zip(next) {
                    *h = alf * n + -alf * *h;
                }
            }
        }
        tw.truncate(sub(count, k)?);
        let mut kv = vec_for(meter, knots)?;
        kv.extend_from_slice(u_.get(k..k.saturating_add(knots)).ok_or(NurbsError::Defect)?);
        let basis = Basis::new(sub(p, k)?, kv)?;
        if !tw.iter().all(|q| q.iter().all(|x| x.is_finite())) {
            return Err(NurbsError::NotFinite);
        }
        Curve::from_homogeneous(basis, tw, None)
    }
}

/// Rational derivatives from the homogeneous ones (A4.2), in place: on entry `out[k]` holds the
/// k-th derivative of A = Σ N·w·P and `wders[k]` that of w = Σ N·w (zero above `top`); on exit
/// `out[k]` holds C^(k) = (A^(k) − `Σ_(i=1..k)` C(k, i)·w^(i)·C^(k−i)) / w.
fn leibniz<const D: usize>(
    out: &mut [[f64; D]],
    wders: &[f64],
    top: usize,
) -> Result<(), NurbsError> {
    let w0 = get(wders, 0)?;
    // Row k of Pascal's triangle, exact: every entry is at most C(28, 14).
    let mut bin = [0.0_f64; WIDTH];
    set(&mut bin, 0, 1.0)?;
    for k in 0..out.len() {
        if k > 0 {
            for i in (1..k).rev() {
                let below = get(&bin, sub(i, 1)?)?;
                *at(&mut bin, i)? += below;
            }
            set(&mut bin, k, 1.0)?;
        }
        let mut v = get(out, k)?;
        let last = if k < top { k } else { top };
        for i in 1..=last {
            let f = get(&bin, i)? * get(wders, i)?;
            let lower = get(out, sub(k, i)?)?;
            for (vd, ld) in v.iter_mut().zip(lower) {
                *vd -= f * ld;
            }
        }
        for vd in &mut v {
            *vd /= w0;
        }
        set(out, k, v)?;
    }
    Ok(())
}

/// The first weight that is not positive and finite, as an error.
pub(crate) fn check_weights(weights: &[f64]) -> Result<(), NurbsError> {
    match weights.iter().position(|&w| !(w.is_finite() && w > 0.0)) {
        Some(index) => Err(NurbsError::Weight { index }),
        None => Ok(()),
    }
}

/// `c` if every coordinate is finite.
pub(crate) fn finite<const D: usize>(c: [f64; D]) -> Result<[f64; D], NurbsError> {
    if c.iter().all(|x| x.is_finite()) {
        Ok(c)
    } else {
        Err(NurbsError::NotFinite)
    }
}
