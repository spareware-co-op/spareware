//! NURBS surfaces in two and three dimensions: construction under chapter 24.7's rules, points and
//! partial derivatives (The NURBS Book, algorithms A3.5, A3.6, A4.3 and A4.4).
//!
//! What it does: a [`Surface`] holds two bases, one per parameter direction, and a net of
//! (n + 1) × (m + 1) control points stored row by row (row i follows u, column j follows v), with
//! weights if it is rational, in homogeneous form w·P as for curves. [`Surface::point`] sums, for
//! each row of the p + 1 rows that matter, the q + 1 points against the v basis, then the rows
//! against the u basis (A3.5, as `NLib` does it), and divides by the weights summed the same way
//! for a rational surface (A4.3). [`Surface::derivs`] does the same with basis derivatives (A3.6)
//! and applies Leibniz's rule to w·S = A for a rational surface (A4.4), for every order k ≤ du in
//! u and l ≤ dv in v.
//!
//! Guarantees, with u = 2^−53, `γ_n` = n·u / (1 − n·u), coordinates taken one at a time, `MU` and
//! `MV` the basis magnitudes of [`crate::basis`] (the values themselves for order 0), D = k + l and
//! w the exact weights:
//!
//! - **Well formed or refused.** A surface has exactly one control point (and one weight) per pair
//!   of basis functions, every coordinate finite, every weight positive and finite, and every
//!   weighted coordinate finite.
//! - **Points.** A nonrational point is within `γ_(7p+7q+2)` · `Σ_i` `NU_i` `Σ_j` `NV_j`·|`P_ij`|; a
//!   rational one within `γ_(14p+14q+5)` times the same sum over |w·P| divided by Σ `NU_i` `NV_j`
//!   `w_ij`. Each inner sum carries `γ_(7q+1)`, and each product with a u basis value and the
//!   outer sum add `γ_(6p)` and p + 1 roundings.
//! - **Derivatives.** A nonrational derivative is within `γ_(7p+7q+6)` · `Σ_i` `MU_(k,i)` `Σ_j`
//!   `MV_(l,j)`·|`P_ij`|. A rational one is within `γ_e` · `MS_(k,l)`, e = (D + 1)(14p + 14q +
//!   2D + 12), where `MS` is A4.4's recurrence run on magnitudes (every subtraction an addition,
//!   with the magnitude sums in place of A and of the weight derivatives); each step of the
//!   recurrence adds the error of the sums, two products per term, its sums and subtractions and the
//!   division to the largest error of the lower orders, and a chain of steps from order (0, 0) has
//!   at most D of them. The value of order (0, 0) has the same bits as [`Surface::point`].
//! - **Typed failures.** A parameter outside its domain is [`NurbsError::ParameterOutside`], a
//!   result that overflows [`NurbsError::NotFinite`], an order above [`MAX_DEGREE`]
//!   [`NurbsError::DerivativeOrder`].
//! - **Deterministic.** Fixed summation order (v inside u, by increasing index), no fused
//!   multiply-add, no platform maths.
//!
//! Method: Piegl and Tiller's algorithms A3.5, A3.6, A4.3 and A4.4 (The NURBS Book, 2nd ed.,
//! sections 3.4, 3.5 and 4.4 to 4.5, <https://doi.org/10.1007/978-3-642-59223-2>), with `NLib`'s
//! order of summation for points; the error bounds follow Higham (*Accuracy and Stability of
//! Numerical Algorithms*, 2nd ed., SIAM (2002), chapter 3, <https://doi.org/10.1137/1.9780898718027>).
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_SrfNurb.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated to safe `no_std` Rust from `N_SrfEvalPtImpl` and
//!   `N_SrfDerivs`, for any dimension D instead of `NLib`'s homogeneous 4D points with its "no z"
//!   and "no w" markers; derivatives summed in the same nested order as points (v inside u),
//!   where `N_ComputeDerivativeMatrixSpec` sums products of the two bases' derivatives in one
//!   double loop, so the value of order (0, 0) equals the point; rational derivatives by the
//!   book's A4.4 on the stored homogeneous points, where `NLib` forms rational basis derivatives
//!   against points divided by their weights; parameters outside the domains and orders above
//!   the maximum are typed errors; each overflow is checked.
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: construction is linear in the control points and makes one allocation (two if rational).
//! [`Surface::point`] takes O(p² + q² + p·q·D) operations; [`Surface::derivs`]
//! O(p² + q² + du·dv·(p·q·D + du·dv·D)); neither allocates.

use alloc::vec::Vec;

use crate::NurbsError;
use crate::basis::{Basis, Limit, MAX_DEGREE, WIDTH};
use crate::curve::{check_weights, finite};
use crate::index::{add, at, get, mul, set, sub};
use crate::point::Point;

/// A NURBS surface of dimension `D`: two bases and a net of control points, weighted if rational.
#[derive(Clone, Debug, PartialEq)]
pub struct Surface<const D: usize> {
    u: Basis,
    v: Basis,
    /// The net, row by row: point (i, j) at i · (m + 1) + j; w·P for a rational surface.
    points: Vec<[f64; D]>,
    weights: Option<Vec<f64>>,
}

impl<const D: usize> Surface<D> {
    /// The nonrational surface on bases `u` and `v` with this net, row by row.
    ///
    /// # Errors
    ///
    /// [`NurbsError::PointCount`] unless the net has one point per pair of basis functions;
    /// [`NurbsError::PointNotFinite`] for the first point with a coordinate that is not finite.
    pub fn new<P: Point<D>>(u: Basis, v: Basis, points: &[P]) -> Result<Self, NurbsError> {
        let points = points.iter().map(|p| p.coords()).collect();
        Self::from_homogeneous(u, v, points, None)
    }

    /// The rational surface on bases `u` and `v` with this net and these weights, row by row;
    /// the points are stored as w·P, each coordinate rounded once.
    ///
    /// # Errors
    ///
    /// As for [`crate::Curve::new_rational`], with the net's row-major indices.
    pub fn new_rational<P: Point<D>>(
        u: Basis,
        v: Basis,
        points: &[P],
        weights: &[f64],
    ) -> Result<Self, NurbsError> {
        let expected = mul(u.count(), v.count())?;
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
        Self::from_homogeneous(u, v, homogeneous, Some(weights.to_vec()))
    }

    /// The surface on bases `u` and `v` with homogeneous net `points` (w·P) and `weights`, or the
    /// nonrational surface with net `points` if `weights` is `None`, row by row. Nothing is
    /// rounded.
    ///
    /// # Errors
    ///
    /// As for [`crate::Curve::from_homogeneous`], with the net's row-major indices.
    pub fn from_homogeneous(
        u: Basis,
        v: Basis,
        points: Vec<[f64; D]>,
        weights: Option<Vec<f64>>,
    ) -> Result<Self, NurbsError> {
        let expected = mul(u.count(), v.count())?;
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
        Ok(Surface {
            u,
            v,
            points,
            weights,
        })
    }

    /// The basis in u.
    #[must_use]
    pub fn basis_u(&self) -> &Basis {
        &self.u
    }

    /// The basis in v.
    #[must_use]
    pub fn basis_v(&self) -> &Basis {
        &self.v
    }

    /// The net as stored, row by row: w·P for a rational surface, P otherwise.
    #[must_use]
    pub fn homogeneous(&self) -> &[[f64; D]] {
        &self.points
    }

    /// The weights, row by row, if the surface is rational.
    #[must_use]
    pub fn weights(&self) -> Option<&[f64]> {
        self.weights.as_deref()
    }

    /// Whether the surface is rational.
    #[must_use]
    pub fn is_rational(&self) -> bool {
        self.weights.is_some()
    }

    /// The point S(u, v) (A3.5, A4.3).
    ///
    /// # Errors
    ///
    /// [`NurbsError::ParameterOutside`] if `u` or `v` is not in its domain;
    /// [`NurbsError::NotFinite`] if the point overflows.
    pub fn point(&self, u: f64, v: f64, limits: (Limit, Limit)) -> Result<[f64; D], NurbsError> {
        let (p, q) = (self.u.degree(), self.v.degree());
        let su = self.u.span(u, limits.0)?;
        let sv = self.v.span(v, limits.1)?;
        let mut nu = [0.0_f64; WIDTH];
        let mut nv = [0.0_f64; WIDTH];
        self.u.funs(su, u, &mut nu)?;
        self.v.funs(sv, v, &mut nv)?;
        let (sum, weight) = self.blend(sub(su, p)?, sub(sv, q)?, &nu, &nv)?;
        if self.weights.is_some() {
            finite(sum.map(|x| x / weight))
        } else {
            finite(sum)
        }
    }

    /// `Σ_i` `nu_i` · (`Σ_j` `nv_j` · `P_(first_u+i, first_v+j)`), and the same sum of the weights.
    fn blend(
        &self,
        first_u: usize,
        first_v: usize,
        nu: &[f64],
        nv: &[f64],
    ) -> Result<([f64; D], f64), NurbsError> {
        let (p, q) = (self.u.degree(), self.v.degree());
        let cols = self.v.count();
        let mut sum = [0.0_f64; D];
        let mut weight = 0.0;
        for i in 0..=p {
            let row = mul(add(first_u, i)?, cols)?;
            let mut inner = [0.0_f64; D];
            let mut inner_w = 0.0;
            for j in 0..=q {
                let nj = get(nv, j)?;
                let index = add(row, add(first_v, j)?)?;
                for (id, pd) in inner.iter_mut().zip(get(&self.points, index)?) {
                    *id += nj * pd;
                }
                if let Some(ws) = &self.weights {
                    inner_w += nj * get(ws, index)?;
                }
            }
            let ni = get(nu, i)?;
            for (sd, id) in sum.iter_mut().zip(inner) {
                *sd += ni * id;
            }
            weight += ni * inner_w;
        }
        Ok((sum, weight))
    }

    /// The point and its partial derivatives at (u, v): `out[k·(dv+1) + l]` is ∂^(k+l)S / ∂u^k ∂v^l
    /// for k from 0 to `du` and l from 0 to `dv` (A3.6, A4.4).
    ///
    /// # Errors
    ///
    /// [`NurbsError::BufferLength`] unless `out` has (du + 1)(dv + 1) elements;
    /// [`NurbsError::DerivativeOrder`] if `du` or `dv` is above [`MAX_DEGREE`];
    /// [`NurbsError::ParameterOutside`] if `u` or `v` is not in its domain;
    /// [`NurbsError::NotFinite`] if a derivative overflows.
    pub fn derivs(
        &self,
        u: f64,
        v: f64,
        limits: (Limit, Limit),
        orders: (usize, usize),
        out: &mut [[f64; D]],
    ) -> Result<(), NurbsError> {
        let (du, dv) = orders;
        for order in [du, dv] {
            if order > MAX_DEGREE {
                return Err(NurbsError::DerivativeOrder { order });
            }
        }
        let cols = add(dv, 1)?;
        let needed = mul(add(du, 1)?, cols)?;
        if out.len() != needed {
            return Err(NurbsError::BufferLength {
                given: out.len(),
                needed,
            });
        }
        let (p, q) = (self.u.degree(), self.v.degree());
        let (tu, tv) = (if du < p { du } else { p }, if dv < q { dv } else { q });
        let su = self.u.span(u, limits.0)?;
        let sv = self.v.span(v, limits.1)?;
        let mut ndu = [0.0_f64; WIDTH * WIDTH];
        let mut ndv = [0.0_f64; WIDTH * WIDTH];
        self.u.ders(su, u, tu, &mut ndu)?;
        self.v.ders(sv, v, tv, &mut ndv)?;
        let (wu, wv) = (add(p, 1)?, add(q, 1)?);
        // wders[k·WIDTH + l]: the (k, l) derivative of the weight function (zero above tu, tv).
        let mut wders = [0.0_f64; WIDTH * WIDTH];
        for row in out.iter_mut() {
            *row = [0.0; D];
        }
        for k in 0..=tu {
            let u_values = ndu
                .get(mul(k, wu)?..add(mul(k, wu)?, wu)?)
                .ok_or(NurbsError::Defect)?;
            for l in 0..=tv {
                let v_terms = ndv
                    .get(mul(l, wv)?..add(mul(l, wv)?, wv)?)
                    .ok_or(NurbsError::Defect)?;
                let (sum, weight) = self.blend(sub(su, p)?, sub(sv, q)?, u_values, v_terms)?;
                set(out, add(mul(k, cols)?, l)?, sum)?;
                set(&mut wders, add(mul(k, WIDTH)?, l)?, weight)?;
            }
        }
        if self.weights.is_some() {
            leibniz2(out, &wders, (du, dv), (tu, tv))?;
        }
        for row in out.iter() {
            finite(*row)?;
        }
        Ok(())
    }
}

/// Rational partial derivatives from the homogeneous ones (A4.4), in place: on entry `out[k][l]`
/// holds the (k, l) derivative of A = Σ N·w·P and `wders[k][l]` that of the weight function; on
/// exit `out[k][l]` holds the (k, l) derivative of S = A / w.
fn leibniz2<const D: usize>(
    out: &mut [[f64; D]],
    wders: &[f64],
    orders: (usize, usize),
    tops: (usize, usize),
) -> Result<(), NurbsError> {
    let (du, dv) = orders;
    let cols = add(dv, 1)?;
    let w00 = get(wders, 0)?;
    let bins = pascal()?;
    let bin = |n: usize, k: usize| -> Result<f64, NurbsError> { get(&bins, add(mul(n, WIDTH)?, k)?) };
    let wd = |k: usize, l: usize| -> Result<f64, NurbsError> {
        if k > tops.0 || l > tops.1 {
            Ok(0.0)
        } else {
            get(wders, add(mul(k, WIDTH)?, l)?)
        }
    };
    let at_ = |k: usize, l: usize| -> Result<usize, NurbsError> { add(mul(k, cols)?, l) };
    for k in 0..=du {
        for l in 0..=dv {
            let mut val = get(out, at_(k, l)?)?;
            for j in 1..=l {
                let f = bin(l, j)? * wd(0, j)?;
                let lower = get(out, at_(k, sub(l, j)?)?)?;
                for (vd, ld) in val.iter_mut().zip(lower) {
                    *vd -= f * ld;
                }
            }
            for i in 1..=k {
                let f = bin(k, i)? * wd(i, 0)?;
                let lower = get(out, at_(sub(k, i)?, l)?)?;
                for (vd, ld) in val.iter_mut().zip(lower) {
                    *vd -= f * ld;
                }
                let mut v2 = [0.0_f64; D];
                for j in 1..=l {
                    let g = bin(l, j)? * wd(i, j)?;
                    let lower = get(out, at_(sub(k, i)?, sub(l, j)?)?)?;
                    for (v2d, ld) in v2.iter_mut().zip(lower) {
                        *v2d += g * ld;
                    }
                }
                let b = bin(k, i)?;
                for (vd, v2d) in val.iter_mut().zip(v2) {
                    *vd -= b * v2d;
                }
            }
            for vd in &mut val {
                *vd /= w00;
            }
            *at(out, at_(k, l)?)? = val;
        }
    }
    Ok(())
}

/// Pascal's triangle up to row [`MAX_DEGREE`], row-major with rows of [`WIDTH`]: every entry is a
/// whole number at most C(28, 14), so every sum here is exact.
fn pascal() -> Result<[f64; WIDTH * WIDTH], NurbsError> {
    let mut t = [0.0_f64; WIDTH * WIDTH];
    for n in 0..WIDTH {
        let row = mul(n, WIDTH)?;
        set(&mut t, row, 1.0)?;
        for k in 1..=n {
            let above = sub(row, WIDTH)?;
            let left = get(&t, add(above, sub(k, 1)?)?)?;
            let right = if k < n { get(&t, add(above, k)?)? } else { 0.0 };
            set(&mut t, add(row, k)?, left + right)?;
        }
    }
    Ok(t)
}
