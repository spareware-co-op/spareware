//! Knot insertion and refinement of NURBS curves (The NURBS Book, algorithms A5.1 and A5.4):
//! more knots and control points, the same curve.
//!
//! What it does: [`Curve::insert_knot`] inserts one parameter value r times (Boehm's algorithm,
//! A5.1); [`Curve::refine`] inserts a non-decreasing list of values at once (A5.4). Each new
//! control point is a convex combination of two older ones, α·Q + (1 − α)·R with
//! α = (u − `u_i`) / (`u_(i+p)` − `u_i`) in [0, 1], in homogeneous coordinates for a rational
//! curve. The kernels are generic over `crate::arith`'s numbers and run twice, in doubles for the
//! result and in intervals for its certified rounding (`crate::ops`). Surfaces use the same kernels
//! on each row or column of their nets (`crate::net`).
//!
//! Guarantees:
//!
//! - **The same curve, within the reported deviation.** In real arithmetic insertion does not
//!   change the curve; the reported deviation bounds, with certainty, how far rounding moved it.
//!   An a priori bound also holds: each new homogeneous coordinate is within `γ_(10t)` · M of its
//!   exact value, with t the number of knots inserted and M the largest absolute value of that
//!   coordinate over the input's control points, because each pass of one insertion turns every
//!   error ε into at most (1 + `γ_10`)·ε + `γ_10`·M (the quotient α carries three roundings, 1 − α
//!   four, and the two products and the sum three more).
//! - **Interior knots only, multiplicity at most p.** A value outside the open domain is
//!   [`NurbsError::KnotOutside`], as `NLib` refuses an end parameter; a list that is not finite and
//!   non-decreasing is [`NurbsError::KnotOutside`]; a value that would be repeated more than p
//!   times is [`NurbsError::TooManyKnots`] (`NLib` checks this for single insertion only).
//! - **Budgeted.** The result's points and knots, and the interval run, are charged to the meter
//!   before anything is computed.
//!
//! Method: Boehm's knot insertion and the Oslo-style refinement of Piegl and Tiller, algorithms
//! A5.1 and A5.4 (The NURBS Book, 2nd ed., section 5.2, <https://doi.org/10.1007/978-3-642-59223-2>;
//! W. Boehm, "Inserting new knots into B-spline curves", *Computer-Aided Design* 12(4) (1980),
//! 199–201, <https://doi.org/10.1016/0010-4485(80)90154-2>), as `NLib` implements them.
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvTool.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated to safe `no_std` Rust from `N_CrvInsertKnot` and
//!   `N_CrvRefine`, generic over the number type so the same steps give the result and its
//!   interval enclosure; new curves are returned instead of edited in place; refinement checks
//!   that its knots are finite, ordered, interior and leave every multiplicity at most p, which
//!   `N_CrvRefine` does not; the zero test of `N_CrvRefine` compares the two knots instead of
//!   their computed difference with zero.
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: insertion takes O(n + r·p) operations and refinement O(n + (n + k)·p) for k new knots,
//! each twice; each allocates the result's points and knots and its enclosure.

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_run::WorkMeter;

use crate::arith::{Hom, Num};
use crate::basis::{Basis, Limit, WIDTH};
use crate::budget::vec_for;
use crate::curve::Curve;
use crate::index::{add, get, run_end, set, sub};
use crate::ops::{Changed, charge_op, homs, p_max, rounding, split};
use crate::NurbsError;

impl<const D: usize> Curve<D> {
    /// The curve with `u` inserted `times` times (A5.1).
    ///
    /// # Errors
    ///
    /// [`NurbsError::KnotOutside`] unless `u` is strictly inside the domain;
    /// [`NurbsError::TooManyKnots`] if `u` would be repeated more than p times;
    /// [`NurbsError::Stopped`] if the meter stops it; [`NurbsError::NotFinite`] if a new point
    /// overflows.
    pub fn insert_knot(
        &self,
        u: f64,
        times: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<Curve<D>>, NurbsError> {
        let basis = self.basis();
        let (lo, hi) = basis.domain();
        if u.is_nan() || u <= lo || u >= hi {
            return Err(NurbsError::KnotOutside);
        }
        let p = basis.degree();
        let s = basis.multiplicity(u);
        if add(s, times)? > p {
            return Err(NurbsError::TooManyKnots);
        }
        let span = basis.span(u, Limit::FromRight)?;
        let count = self.homogeneous().len();
        charge_op(meter, add(count, times)?, D, add(basis.knots().len(), times)?)?;
        let rational = self.is_rational();
        let input_f = homs::<f64, D>(meter, self.homogeneous(), self.weights())?;
        let input_i = homs::<Interval, D>(meter, self.homogeneous(), self.weights())?;
        let job = Insertion { u, span, s, times };
        let (knots, result) = insert_kernel(meter, basis.knots(), p, &input_f, &job)?;
        let (_, enclosure) = insert_kernel(meter, basis.knots(), p, &input_i, &job)?;
        let deviation = rounding(&result, &enclosure, rational, self.p_max_if(rational)?)?;
        let (points, weights) = split(meter, &result, rational)?;
        let curve = Curve::from_homogeneous(Basis::new(p, knots)?, points, weights)?;
        Ok(Changed {
            value: curve,
            deviation,
        })
    }

    /// The curve with the knots `x` inserted (A5.4). `x` is non-decreasing; a value may repeat.
    ///
    /// # Errors
    ///
    /// [`NurbsError::KnotOutside`] unless every value is finite, strictly inside the domain and
    /// not smaller than the one before; [`NurbsError::TooManyKnots`] if a value would be repeated
    /// more than p times; [`NurbsError::Stopped`] and [`NurbsError::NotFinite`] as for
    /// [`Curve::insert_knot`].
    pub fn refine(
        &self,
        x: &[f64],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<Curve<D>>, NurbsError> {
        let basis = self.basis();
        check_refinement(basis, x)?;
        if x.is_empty() {
            return Ok(Changed {
                value: self.clone(),
                deviation: 0.0,
            });
        }
        let count = self.homogeneous().len();
        charge_op(meter, add(count, x.len())?, D, add(basis.knots().len(), x.len())?)?;
        let rational = self.is_rational();
        let input_f = homs::<f64, D>(meter, self.homogeneous(), self.weights())?;
        let input_i = homs::<Interval, D>(meter, self.homogeneous(), self.weights())?;
        let (knots, result) = refine_kernel(meter, basis, &input_f, x)?;
        let (_, enclosure) = refine_kernel(meter, basis, &input_i, x)?;
        let deviation = rounding(&result, &enclosure, rational, self.p_max_if(rational)?)?;
        let (points, weights) = split(meter, &result, rational)?;
        let curve = Curve::from_homogeneous(Basis::new(basis.degree(), knots)?, points, weights)?;
        Ok(Changed {
            value: curve,
            deviation,
        })
    }

    /// The bound on |C| a rational deviation needs, or 0 for a nonrational curve.
    pub(crate) fn p_max_if(&self, rational: bool) -> Result<f64, NurbsError> {
        if rational {
            p_max(self.homogeneous(), self.weights())
        } else {
            Ok(0.0)
        }
    }
}

/// The checks of [`Curve::refine`]: finite, non-decreasing, strictly inside the domain, and no
/// value repeated more than p times with the basis's own knots.
pub(crate) fn check_refinement(basis: &Basis, x: &[f64]) -> Result<(), NurbsError> {
    let (lo, hi) = basis.domain();
    if x.iter().any(|&v| v.is_nan() || v <= lo || v >= hi) {
        return Err(NurbsError::KnotOutside);
    }
    if x.windows(2).any(|w| w.get(1) < w.first()) {
        return Err(NurbsError::KnotOutside);
    }
    let mut start = 0;
    while start < x.len() {
        let end = run_end(x, start)?;
        let value = get(x, start)?;
        if add(sub(end, start)?, basis.multiplicity(value))? > basis.degree() {
            return Err(NurbsError::TooManyKnots);
        }
        start = end;
    }
    Ok(())
}

/// One knot insertion: the value, its span (from the right), its multiplicity and how many
/// times to insert it.
pub(crate) struct Insertion {
    pub(crate) u: f64,
    pub(crate) span: usize,
    pub(crate) s: usize,
    pub(crate) times: usize,
}

/// A5.1 on control points `pw` with knots `knots`: the new knots and control points.
#[allow(
    clippy::many_single_char_names,
    reason = "the indices are named as in The NURBS Book's algorithm and NLib's port, so the two can be read side by side"
)]
pub(crate) fn insert_kernel<N: Num, const D: usize>(
    meter: &mut WorkMeter<'_>,
    knots: &[f64],
    p: usize,
    pw: &[Hom<N, D>],
    job: &Insertion,
) -> Result<(Vec<f64>, Vec<Hom<N, D>>), NurbsError> {
    let Insertion { u, span, s, times } = *job;
    let r = times;
    let n = sub(pw.len(), 1)?;
    let mut uq = vec_for(meter, add(knots.len(), r)?)?;
    uq.extend_from_slice(knots.get(..=span).ok_or(NurbsError::Defect)?);
    uq.extend(core::iter::repeat_n(u, r));
    uq.extend_from_slice(knots.get(add(span, 1)?..).ok_or(NurbsError::Defect)?);
    let zero = Hom::of([0.0; D], 0.0)?;
    let mut qw = vec_for(meter, add(pw.len(), r)?)?;
    qw.resize(add(pw.len(), r)?, zero);
    let mut rw = [zero; WIDTH];
    let mut k = sub(span, p)?;
    let width = sub(p, s)?;
    for i in 0..=width {
        set(&mut rw, i, get(pw, add(k, i)?)?)?;
    }
    for i in 0..=k {
        set(&mut qw, i, get(pw, i)?)?;
    }
    for i in sub(span, s)?..=n {
        set(&mut qw, add(i, r)?, get(pw, i)?)?;
    }
    let one = N::of(1.0)?;
    let un = N::of(u)?;
    for j in 1..=r {
        k = add(sub(span, p)?, j)?;
        for i in 0..=sub(width, j)? {
            let left = N::of(get(knots, add(k, i)?)?)?;
            let right = N::of(get(knots, add(add(i, span)?, 1)?)?)?;
            let alf = un.minus(left)?.over(right.minus(left)?)?;
            let oma = one.minus(alf)?;
            let next = Hom::combine(alf, get(&rw, add(i, 1)?)?, oma, get(&rw, i)?)?;
            set(&mut rw, i, next)?;
        }
        set(&mut qw, k, get(&rw, 0)?)?;
        set(
            &mut qw,
            sub(sub(add(span, r)?, j)?, s)?,
            get(&rw, sub(width, j)?)?,
        )?;
    }
    for i in add(k, 1)?..sub(span, s)? {
        set(&mut qw, i, get(&rw, sub(i, k)?)?)?;
    }
    Ok((uq, qw))
}

/// A5.4 on control points `pw` of `basis`: the knots with `x` inserted, and the new points.
#[allow(
    clippy::many_single_char_names,
    reason = "the indices are named as in The NURBS Book's algorithm and NLib's port, so the two can be read side by side"
)]
pub(crate) fn refine_kernel<N: Num, const D: usize>(
    meter: &mut WorkMeter<'_>,
    basis: &Basis,
    pw: &[Hom<N, D>],
    x: &[f64],
) -> Result<(Vec<f64>, Vec<Hom<N, D>>), NurbsError> {
    let up = basis.knots();
    let p = basis.degree();
    let m = sub(up.len(), 1)?;
    let n = sub(pw.len(), 1)?;
    let r = sub(x.len(), 1)?;
    let a = basis.span(get(x, 0)?, Limit::FromRight)?;
    let b = add(basis.span(get(x, r)?, Limit::FromRight)?, 1)?;
    let mut uq = vec_for(meter, add(up.len(), x.len())?)?;
    uq.resize(add(up.len(), x.len())?, 0.0);
    let zero = Hom::of([0.0; D], 0.0)?;
    let mut qw = vec_for(meter, add(pw.len(), x.len())?)?;
    qw.resize(add(pw.len(), x.len())?, zero);
    for j in 0..=a {
        set(&mut uq, j, get(up, j)?)?;
    }
    for j in add(b, p)?..=m {
        set(&mut uq, add(add(j, r)?, 1)?, get(up, j)?)?;
    }
    for j in 0..=sub(a, p)? {
        set(&mut qw, j, get(pw, j)?)?;
    }
    for j in sub(b, 1)?..=n {
        set(&mut qw, add(add(j, r)?, 1)?, get(pw, j)?)?;
    }
    let one = N::of(1.0)?;
    let mut i = sub(add(b, p)?, 1)?;
    let mut k = add(add(b, p)?, r)?;
    for j in (0..=r).rev() {
        let xj = get(x, j)?;
        while xj <= get(up, i)? && i > a {
            let from = sub(sub(i, p)?, 1)?;
            set(&mut qw, sub(sub(k, p)?, 1)?, get(pw, from)?)?;
            set(&mut uq, k, get(up, i)?)?;
            k = sub(k, 1)?;
            i = sub(i, 1)?;
        }
        let copy = get(&qw, sub(k, p)?)?;
        set(&mut qw, sub(sub(k, p)?, 1)?, copy)?;
        for l in 1..=p {
            let ind = add(sub(k, p)?, l)?;
            let top = get(&uq, add(k, l)?)?;
            if top == xj {
                let copy = get(&qw, ind)?;
                set(&mut qw, sub(ind, 1)?, copy)?;
            } else {
                let low = get(up, add(sub(i, p)?, l)?)?;
                let alf = N::of(top)?
                    .minus(N::of(xj)?)?
                    .over(N::of(top)?.minus(N::of(low)?)?)?;
                let oma = one.minus(alf)?;
                let next = Hom::combine(alf, get(&qw, sub(ind, 1)?)?, oma, get(&qw, ind)?)?;
                set(&mut qw, sub(ind, 1)?, next)?;
            }
        }
        set(&mut uq, k, xj)?;
        k = k.saturating_sub(1);
    }
    Ok((uq, qw))
}
