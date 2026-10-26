//! Degree elevation of NURBS curves (The NURBS Book, algorithm A5.9): the same curve with degree
//! p + t.
//!
//! What it does: [`Curve::elevate`] walks the knot vector as Bezier extraction does (A5.6),
//! raises each Bezier piece from degree p to p + t with the coefficients
//! C(p, j)·C(t, i − j) / C(p + t, i) (`NLib`'s elevation matrix, filled for the first half of the rows
//! and mirrored), and removes the knots the extraction added, so each distinct knot ends repeated t
//! times more than before (`NLib`'s `N_CrvElevateDegree`, the book's A5.9). The kernel is generic over
//! `crate::arith`'s numbers and runs in doubles and in intervals (`crate::ops`).
//!
//! Guarantees:
//!
//! - **The same curve, within the reported deviation.** Elevation does not change the curve in
//!   real arithmetic; the reported deviation bounds, with certainty, how far rounding moved it.
//!   The removal steps divide by knot differences that can be small, so no a priori bound is
//!   stated; the interval run measures the rounding of each case.
//! - **Exact coefficients.** Every binomial coefficient comes from Pascal's triangle, exact for
//!   degrees up to [`MAX_DEGREE`] (`crate::basis`), so only the reciprocal 1 / C(p + t, i) and two
//!   products round.
//! - **Where it applies.** The curve must be clamped ([`NurbsError::NotClamped`]) with no
//!   interior knot repeated more than p times ([`NurbsError::Discontinuous`]), as `NLib`'s check
//!   requires, and p + t must not exceed [`MAX_DEGREE`] ([`NurbsError::Degree`]). t = 0 returns
//!   the curve unchanged.
//! - **Budgeted.** The result and the interval run are charged to the meter first.
//!
//! Method: Piegl and Tiller's algorithm A5.9 (The NURBS Book, 2nd ed., section 5.5,
//! <https://doi.org/10.1007/978-3-642-59223-2>; L. Piegl and W. Tiller, "Software-engineering
//! approach to degree elevation of B-spline curves", *Computer-Aided Design* 26(1) (1994), 17–28,
//! <https://doi.org/10.1016/0010-4485(94)90004-3>), as `NLib` implements it.
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvTool.cpp>
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_Bezier.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated to safe `no_std` Rust from `N_CrvElevateDegree`,
//!   `N_BezGetDegreeElevationMatrix` and `N_BezElevateDegree`, generic over the number type so
//!   the same steps give the result and its interval enclosure; Pascal's triangle is computed
//!   rather than read from `NLib`'s table of 18 rows; a new curve is returned instead of edited in
//!   place; the degree limit is [`MAX_DEGREE`].
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: O(s·(p + t)·p) operations for s spans, each twice, and O((p + t)·p) for the matrix; the
//! result and its enclosure are allocated once.

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::arith::{Hom, Num};
use crate::basis::{Basis, MAX_DEGREE, WIDTH};
use crate::bezier::span_count;
use crate::budget::vec_for;
use crate::curve::Curve;
use crate::index::{add, get, mul, set, sub};
use crate::ops::{Changed, charge_op, homs, rounding, split};

impl<const D: usize> Curve<D> {
    /// The same curve with degree p + `t` (A5.9).
    ///
    /// # Errors
    ///
    /// [`NurbsError::Degree`] if p + t exceeds [`MAX_DEGREE`]; [`NurbsError::NotClamped`] unless
    /// the curve is clamped; [`NurbsError::Discontinuous`] if an interior knot is repeated p + 1
    /// times; [`NurbsError::Stopped`] if the meter stops it; [`NurbsError::NotFinite`] if a point
    /// overflows.
    pub fn elevate(
        &self,
        t: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<Curve<D>>, NurbsError> {
        let basis = self.basis();
        let p = basis.degree();
        let q = add(p, t)?;
        if q > MAX_DEGREE {
            return Err(NurbsError::Degree { degree: q });
        }
        check_elevation(basis)?;
        if t == 0 {
            return Ok(Changed {
                value: self.clone(),
                deviation: 0.0,
            });
        }
        let s = span_count(basis);
        let points = add(self.homogeneous().len(), mul(t, s)?)?;
        let knots = add(basis.knots().len(), mul(t, add(s, 1)?)?)?;
        charge_op(meter, points, D, knots)?;
        let rational = self.is_rational();
        let input_f = homs::<f64, D>(meter, self.homogeneous(), self.weights())?;
        let input_i = homs::<Interval, D>(meter, self.homogeneous(), self.weights())?;
        let (uq, result) = elevate_kernel(meter, basis, &input_f, t)?;
        let (_, enclosure) = elevate_kernel(meter, basis, &input_i, t)?;
        let deviation = rounding(&result, &enclosure, rational, self.p_max_if(rational)?)?;
        let (pts, weights) = split(meter, &result, rational)?;
        Ok(Changed {
            value: Curve::from_homogeneous(Basis::new(q, uq)?, pts, weights)?,
            deviation,
        })
    }
}

/// The conditions of A5.9 on the knots: clamped, and no interior knot repeated p + 1 times.
pub(crate) fn check_elevation(basis: &Basis) -> Result<(), NurbsError> {
    if !basis.is_clamped() {
        return Err(NurbsError::NotClamped);
    }
    let (lo, hi) = basis.domain();
    let p = basis.degree();
    if basis
        .knots()
        .chunk_by(|a, b| a == b)
        .any(|run| run.first().is_some_and(|&k| lo < k && k < hi) && run.len() > p)
    {
        return Err(NurbsError::Discontinuous);
    }
    Ok(())
}

/// Pascal's triangle up to row [`MAX_DEGREE`], row-major with rows of [`WIDTH`]; exact.
pub(crate) fn pascal() -> Result<[f64; WIDTH * WIDTH], NurbsError> {
    let mut tri = [0.0_f64; WIDTH * WIDTH];
    for n in 0..WIDTH {
        let row = mul(n, WIDTH)?;
        set(&mut tri, row, 1.0)?;
        for k in 1..=n {
            let above = sub(row, WIDTH)?;
            let left = get(&tri, add(above, sub(k, 1)?)?)?;
            let right = if k < n { get(&tri, add(above, k)?)? } else { 0.0 };
            set(&mut tri, add(row, k)?, left + right)?;
        }
    }
    Ok(tri)
}

/// C(n, k) from the triangle.
pub(crate) fn binomial(tri: &[f64], n: usize, k: usize) -> Result<f64, NurbsError> {
    get(tri, add(mul(n, WIDTH)?, k)?)
}

/// `NLib`'s degree elevation matrix for degree p raised by t, row-major with rows of [`WIDTH`]:
/// entry (i, j) is C(p, j)·C(t, i − j) / C(p + t, i), computed as (1 / C(p + t, i))·C(p, j)·C(t, i − j)
/// for i ≤ (p + t) / 2 and mirrored, (i, j) = (p + t − i, p − j), below.
fn elevation_matrix<N: Num>(p: usize, t: usize) -> Result<[N; WIDTH * WIDTH], NurbsError> {
    let tri = pascal()?;
    let q = add(p, t)?;
    let zero = N::of(0.0)?;
    let mut rm = [zero; WIDTH * WIDTH];
    let at_ = |i: usize, j: usize| -> Result<usize, NurbsError> { add(mul(i, WIDTH)?, j) };
    set(&mut rm, 0, N::of(1.0)?)?;
    set(&mut rm, at_(q, p)?, N::of(1.0)?)?;
    let half = q / 2;
    for i in 1..=half {
        let inv = N::of(1.0)?.over(N::of(binomial(&tri, q, i)?)?)?;
        let lo = i.saturating_sub(t);
        let hi = if i > p { p } else { i };
        for j in lo..=hi {
            let v = inv
                .times(N::of(binomial(&tri, p, j)?)?)?
                .times(N::of(binomial(&tri, t, sub(i, j)?)?)?)?;
            set(&mut rm, at_(i, j)?, v)?;
        }
    }
    for i in add(half, 1)?..q {
        let lo = i.saturating_sub(t);
        let hi = if i > p { p } else { i };
        for j in lo..=hi {
            let v = get(&rm, at_(sub(q, i)?, sub(p, j)?)?)?;
            set(&mut rm, at_(i, j)?, v)?;
        }
    }
    Ok(rm)
}

/// The working state of A5.9: the Bezier points of the current piece (`bw`), the points saved for
/// the next one (`nw`), the elevated piece (`dw`) and the insertion coefficients.
struct Pieces<N, const D: usize> {
    bw: [Hom<N, D>; WIDTH],
    nw: [Hom<N, D>; WIDTH],
    dw: [Hom<N, D>; WIDTH],
    alfs: [N; WIDTH],
    omas: [N; WIDTH],
}

/// A5.9 on the control points `pw` of a clamped `basis`: the elevated knots and points.
#[allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "the indices are named as in The NURBS Book's algorithm and NLib's port, and the loop is kept whole, so the two can be read side by side"
)]
pub(crate) fn elevate_kernel<N: Num, const D: usize>(
    meter: &mut WorkMeter<'_>,
    basis: &Basis,
    pw: &[Hom<N, D>],
    t: usize,
) -> Result<(Vec<f64>, Vec<Hom<N, D>>), NurbsError> {
    let up = basis.knots();
    let p = basis.degree();
    let q = add(p, t)?;
    let mp = sub(up.len(), 1)?;
    let s = span_count(basis);
    let nq = add(sub(pw.len(), 1)?, mul(t, s)?)?;
    let mq = add(mp, mul(t, add(s, 1)?)?)?;
    let rm = elevation_matrix::<N>(p, t)?;
    let zero = Hom::of([0.0; D], 0.0)?;
    let mut uq = vec_for(meter, add(mq, 1)?)?;
    uq.resize(add(mq, 1)?, 0.0);
    let mut qw = vec_for(meter, add(nq, 1)?)?;
    qw.resize(add(nq, 1)?, zero);
    let mut st = Pieces {
        bw: [zero; WIDTH],
        nw: [zero; WIDTH],
        dw: [zero; WIDTH],
        alfs: [N::of(0.0)?; WIDTH],
        omas: [N::of(0.0)?; WIDTH],
    };
    let one = N::of(1.0)?;
    let mut a = p;
    let mut b = add(p, 1)?;
    // r = −1 before the first piece: `first` stands for that, and `r` is read only after it.
    let mut r = 0_usize;
    let mut first_piece = true;
    let mut cind = 1_usize;
    let mut kind = add(q, 1)?;
    for i in 0..=q {
        set(&mut uq, i, get(up, a)?)?;
    }
    set(&mut qw, 0, get(pw, 0)?)?;
    for i in 0..=p {
        set(&mut st.bw, i, get(pw, i)?)?;
    }
    while b < mp {
        let i0 = b;
        while b < mp && get(up, b)? == get(up, add(b, 1)?)? {
            b = add(b, 1)?;
        }
        let mlt = add(sub(b, i0)?, 1)?;
        let oldr = if first_piece { 0 } else { r };
        // The last run, the end knots, has p + 1 knots and r = −1, which behaves as 0 here.
        r = p.saturating_sub(mlt);
        let lbz = if oldr > 0 { add(oldr, 2)? / 2 } else { 1 };
        let rbz = if r > 0 { sub(q, add(r, 1)? / 2)? } else { q };
        let (ua, ub) = (get(up, a)?, get(up, b)?);
        if r > 0 {
            let num = N::of(ub)?.minus(N::of(ua)?)?;
            for k in (add(mlt, 1)?..=p).rev() {
                let den = N::of(get(up, add(a, k)?)?)?.minus(N::of(ua)?)?;
                let alf = num.over(den)?;
                let slot = sub(sub(k, mlt)?, 1)?;
                set(&mut st.alfs, slot, alf)?;
                set(&mut st.omas, slot, one.minus(alf)?)?;
            }
            for j in 1..=r {
                let save = sub(r, j)?;
                let sj = add(mlt, j)?;
                for k in (sj..=p).rev() {
                    let next = Hom::combine(
                        get(&st.alfs, sub(k, sj)?)?,
                        get(&st.bw, k)?,
                        get(&st.omas, sub(k, sj)?)?,
                        get(&st.bw, sub(k, 1)?)?,
                    )?;
                    set(&mut st.bw, k, next)?;
                }
                set(&mut st.nw, save, get(&st.bw, p)?)?;
            }
        }
        // Raise the piece: dw[i] = Σ_j rm[i][j]·bw[j], from lbz to q.
        for i in lbz..=q {
            let lo = i.saturating_sub(t);
            let hi = if i > p { p } else { i };
            let mut acc = zero;
            for j in lo..=hi {
                let coef = get(&rm, add(mul(i, WIDTH)?, j)?)?;
                acc = acc.add_scaled(coef, get(&st.bw, j)?)?;
            }
            set(&mut st.dw, i, acc)?;
        }
        if oldr > 1 {
            remove_shared_knot(&mut qw, &mut st.dw, &uq, (ua, ub), (kind, cind, lbz, q, oldr))?;
        }
        if !first_piece {
            for _ in 0..sub(q, oldr)? {
                set(&mut uq, kind, ua)?;
                kind = add(kind, 1)?;
            }
        }
        for i in lbz..=rbz {
            set(&mut qw, cind, get(&st.dw, i)?)?;
            cind = add(cind, 1)?;
        }
        if b < mp {
            for i in 0..r {
                set(&mut st.bw, i, get(&st.nw, i)?)?;
            }
            for i in r..=p {
                set(&mut st.bw, i, get(pw, add(sub(b, p)?, i)?)?)?;
            }
            a = b;
            b = add(b, 1)?;
        } else {
            for i in 0..=q {
                set(&mut uq, add(kind, i)?, ub)?;
            }
        }
        first_piece = false;
    }
    Ok((uq, qw))
}

/// The knot removal step of A5.9: removes the knot `ua` that joins the previous piece to this one
/// `oldr − 1` times, updating the points already written (`qw`, below `cind`) and the elevated
/// piece (`dw`).
#[allow(
    clippy::many_single_char_names,
    reason = "the indices are named as in The NURBS Book's algorithm and NLib's port"
)]
fn remove_shared_knot<N: Num, const D: usize>(
    qw: &mut [Hom<N, D>],
    dw: &mut [Hom<N, D>],
    uq: &[f64],
    (ua, ub): (f64, f64),
    (kind, cind, lbz, q, oldr): (usize, usize, usize, usize, usize),
) -> Result<(), NurbsError> {
    let one = N::of(1.0)?;
    let mut first = sub(kind, 2)?;
    let mut last = kind;
    let den = N::of(ub)?.minus(N::of(ua)?)?;
    let bet = N::of(ub)?.minus(N::of(get(uq, sub(kind, 1)?)?)?)?.over(den)?;
    let omb = one.minus(bet)?;
    for tr in 1..oldr {
        let mut i = first;
        let mut j = last;
        // kj = j − kind + 1, which stays at least 0 (see the module's method).
        let mut kj = sub(add(j, 1)?, kind)?;
        while sub(j, i)? > tr {
            if i < cind {
                let ui = N::of(get(uq, i)?)?;
                let alf = N::of(ub)?.minus(ui)?.over(N::of(ua)?.minus(ui)?)?;
                let oma = one.minus(alf)?;
                let next = Hom::combine(alf, get(qw, i)?, oma, get(qw, sub(i, 1)?)?)?;
                set(qw, i, next)?;
            }
            if j >= lbz {
                let next = if sub(j, tr)? <= sub(add(kind, oldr)?, q)? {
                    let gam = N::of(ub)?
                        .minus(N::of(get(uq, sub(j, tr)?)?)?)?
                        .over(den)?;
                    let omg = one.minus(gam)?;
                    Hom::combine(gam, get(dw, kj)?, omg, get(dw, add(kj, 1)?)?)?
                } else {
                    Hom::combine(bet, get(dw, kj)?, omb, get(dw, add(kj, 1)?)?)?
                };
                set(dw, kj, next)?;
            }
            i = add(i, 1)?;
            j = sub(j, 1)?;
            kj = kj.saturating_sub(1);
        }
        first = sub(first, 1)?;
        last = add(last, 1)?;
    }
    Ok(())
}
