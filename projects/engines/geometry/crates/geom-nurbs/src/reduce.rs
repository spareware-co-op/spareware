//! Degree reduction of NURBS curves (The NURBS Book, algorithm A5.11), certified: the curve of
//! degree p − 1 is returned only when it provably lies within the tolerance of the input.
//!
//! What it does: [`Curve::reduce_degree`] walks the knot vector as Bezier extraction does (A5.6),
//! reduces each Bezier piece from degree p to p − 1 (the book's equations 5.41 to 5.44: points
//! solved from both ends, the middle one averaged for odd degree), and removes one copy of each
//! knot again, averaging the two estimates where the removal sweeps cross with `NLib`'s weight
//! λ = α / (α + β) (`NLib`'s `N_CrvReduceDegreeOnce`). Every distinct knot loses one copy, so knots
//! of multiplicity 1 disappear. Whether the result is kept is not decided by `NLib`'s error estimate
//! but certified: the candidate is raised back to degree p (A5.9) and the knots it lost are
//! inserted (A5.4), both exact operations run in interval arithmetic, which gives the input's
//! knots and an enclosure of the control points the candidate has on the input's basis; their
//! largest distance from the input's points bounds the distance between the curves
//! (`crate::remove`'s difference bound, with the rational form of `crate::ops`).
//!
//! Guarantees:
//!
//! - **Within the tolerance, certified.** A returned curve differs from the input by at most the
//!   returned deviation everywhere in the domain, and the deviation is at most `tol`; the bound
//!   includes the rounding of every returned control point. Otherwise the result is
//!   [`NurbsError::OutsideTolerance`], also when a weight of the candidate is not positive.
//! - **Where it applies.** The curve has degree 2 or more ([`NurbsError::Degree`]), is clamped
//!   ([`NurbsError::NotClamped`]) and has no interior knot repeated p + 1 times
//!   ([`NurbsError::Discontinuous`]), as A5.11 and `NLib` assume; the tolerance is finite and not
//!   negative ([`NurbsError::Tolerance`]).
//! - **Budgeted.** The candidate and its certification are charged to the meter first.
//!
//! Method: Piegl and Tiller's algorithm A5.11 with Bezier degree reduction by equations 5.41 to
//! 5.44 (The NURBS Book, 2nd ed., section 5.6, <https://doi.org/10.1007/978-3-642-59223-2>; L.
//! Piegl and W. Tiller, "Algorithm for degree reduction of B-spline curves", *Computer-Aided
//! Design* 27(2) (1995), 101–110, <https://doi.org/10.1016/0010-4485(95)92147-K>), as `NLib`
//! implements it, certified by exact elevation and refinement in interval arithmetic
//! (`crate::elevate`, `crate::insert`, `crate::arith`).
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvTool.cpp>
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_Bezier.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated to safe `no_std` Rust from `N_CrvReduceDegreeOnce`,
//!   `N_BezDegreeReduceCoefs` and `N_BezReduceDegree`; `NLib`'s error estimate (Bezier error terms
//!   and basis maxima from a numerical search) and its weight range are replaced by the certified
//!   bound above, so the square root of its odd-degree estimate is not needed; a new curve is
//!   returned with its deviation instead of a flag.
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: O(s·p²) operations for s spans to reduce, and an elevation and a refinement of the
//! result in intervals to certify, O(s·p²) more; the candidate and the enclosures are allocated
//! once each.

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::arith::Hom;
use crate::basis::{Basis, WIDTH};
use crate::bezier::span_count;
use crate::budget::vec_for;
use crate::curve::Curve;
use crate::elevate::{check_elevation, elevate_kernel};
use crate::index::{add, get, mul, real, set, sub};
use crate::insert::refine_kernel;
use crate::ops::{Changed, charge_op, homs, split};
use crate::remove::difference_bound;

impl<const D: usize> Curve<D> {
    /// The curve with degree p − 1, within `tol` of this one (A5.11).
    ///
    /// # Errors
    ///
    /// [`NurbsError::Tolerance`] if `tol` is negative or not finite; [`NurbsError::Degree`] if p is
    /// 1; [`NurbsError::NotClamped`] and [`NurbsError::Discontinuous`] as for [`Curve::elevate`];
    /// [`NurbsError::OutsideTolerance`] if the reduced curve cannot be certified within `tol`;
    /// [`NurbsError::Stopped`] if the meter stops it.
    pub fn reduce_degree(
        &self,
        tol: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<Curve<D>>, NurbsError> {
        if !(tol.is_finite() && tol >= 0.0) {
            return Err(NurbsError::Tolerance);
        }
        let basis = self.basis();
        let p = basis.degree();
        if p < 2 {
            return Err(NurbsError::Degree { degree: 0 });
        }
        check_elevation(basis)?;
        let q = sub(p, 1)?;
        let rational = self.is_rational();
        // The candidate, then its elevation and refinement in intervals: three curves of about the
        // input's size.
        charge_op(
            meter,
            mul(self.homogeneous().len(), 3)?,
            D,
            mul(basis.knots().len(), 3)?,
        )?;
        let input = homs::<f64, D>(meter, self.homogeneous(), self.weights())?;
        let (uq, candidate) = reduce_kernel(meter, basis, &input)?;
        if rational && !candidate.iter().all(|h| h.w.is_finite() && h.w > 0.0) {
            return Err(NurbsError::OutsideTolerance);
        }
        let reduced = Basis::new(q, uq)?;
        // Certify: back to degree p, then the knots the reduction removed, in intervals.
        let mut enclosed = vec_for(meter, candidate.len())?;
        for h in &candidate {
            enclosed.push(Hom::<Interval, D>::of(h.c, h.w)?);
        }
        let (up_knots, up_points) = elevate_kernel(meter, &reduced, &enclosed, 1)?;
        let raised = Basis::new(p, up_knots)?;
        let missing = missing_knots(meter, basis.knots(), raised.knots())?;
        let (back_knots, back) = if missing.is_empty() {
            (raised.knots().to_vec(), up_points)
        } else {
            refine_kernel(meter, &raised, &up_points, &missing)?
        };
        if back_knots.as_slice() != basis.knots() || back.len() != input.len() {
            return Err(NurbsError::Defect);
        }
        let w_min = candidate
            .iter()
            .fold(f64::INFINITY, |m, h| if h.w < m { h.w } else { m });
        let deviation = difference_bound(&input, &back, rational, self.p_max_if(rational)?, w_min)?;
        if deviation > tol {
            return Err(NurbsError::OutsideTolerance);
        }
        let (points, weights) = split(meter, &candidate, rational)?;
        Ok(Changed {
            value: Curve::from_homogeneous(reduced, points, weights)?,
            deviation,
        })
    }
}

/// The knots of `full` that `part` lacks, with repeats, in order: both are non-decreasing, and
/// `part` holds no value more often than `full`.
fn missing_knots(
    meter: &mut WorkMeter<'_>,
    full: &[f64],
    part: &[f64],
) -> Result<Vec<f64>, NurbsError> {
    let mut out = vec_for(meter, full.len().saturating_sub(part.len()))?;
    let mut j = 0;
    for &k in full {
        if part.get(j).is_some_and(|&x| x == k) {
            j = add(j, 1)?;
        } else {
            out.push(k);
        }
    }
    Ok(out)
}

/// The coefficients of Bezier degree reduction from degree p (`NLib`'s `N_BezDegreeReduceCoefs`):
/// `α_i` = p / (p − i) for i up to (p − 1) / 2 and `β_i` = p / (i + 1) from p − 2 down to
/// (p − 1) / 2, with 1 − `α_i` and 1 − `β_i`.
struct Coefs {
    alf: [f64; WIDTH],
    oma: [f64; WIDTH],
    bet: [f64; WIDTH],
    omb: [f64; WIDTH],
}

fn reduction_coefs(p: usize) -> Result<Coefs, NurbsError> {
    let mut c = Coefs {
        alf: [0.0; WIDTH],
        oma: [0.0; WIDTH],
        bet: [0.0; WIDTH],
        omb: [0.0; WIDTH],
    };
    let r = sub(p, 1)? / 2;
    let pf = real(p)?;
    for i in 1..=r {
        let a = pf / (pf - real(i)?);
        set(&mut c.alf, i, a)?;
        set(&mut c.oma, i, 1.0 - a)?;
    }
    for i in (r..=sub(p, 2)?).rev() {
        let b = pf / (real(i)? + 1.0);
        set(&mut c.bet, i, b)?;
        set(&mut c.omb, i, 1.0 - b)?;
    }
    Ok(c)
}

/// One Bezier piece `pw` (p + 1 points) reduced to degree p − 1 into `qw` (`NLib`'s
/// `N_BezReduceDegree`, without its error estimate).
fn reduce_bezier<const D: usize>(
    pw: &[Hom<f64, D>],
    p: usize,
    c: &Coefs,
    qw: &mut [Hom<f64, D>],
) -> Result<(), NurbsError> {
    set(qw, 0, get(pw, 0)?)?;
    set(qw, sub(p, 1)?, get(pw, p)?)?;
    let r = sub(p, 1)? / 2;
    // For odd p the step at i = r is overwritten by the middle point's average below, so the left
    // sweep may run to r either way.
    for i in 1..=r {
        let v = Hom::combine(
            get(&c.alf, i)?,
            get(pw, i)?,
            get(&c.oma, i)?,
            get(qw, sub(i, 1)?)?,
        )?;
        set(qw, i, v)?;
    }
    for i in (add(r, 1)?..=sub(p, 2)?).rev() {
        let v = Hom::combine(
            get(&c.bet, i)?,
            get(pw, add(i, 1)?)?,
            get(&c.omb, i)?,
            get(qw, add(i, 1)?)?,
        )?;
        set(qw, i, v)?;
    }
    if p % 2 == 1 {
        // The middle point: the average of its estimates from the left and from the right.
        let pl = Hom::combine(
            get(&c.alf, r)?,
            get(pw, r)?,
            get(&c.oma, r)?,
            get(qw, sub(r, 1)?)?,
        )?;
        let pr = Hom::combine(
            get(&c.bet, r)?,
            get(pw, add(r, 1)?)?,
            get(&c.omb, r)?,
            get(qw, add(r, 1)?)?,
        )?;
        set(qw, r, Hom::combine(0.5, pl, 0.5, pr)?)?;
    }
    Ok(())
}

/// The working state of A5.11: the current piece (`bw`), the points kept for the next (`nw`), the
/// reduced piece (`dw`) and the insertion coefficients.
struct Work<const D: usize> {
    bw: [Hom<f64, D>; WIDTH],
    nw: [Hom<f64, D>; WIDTH],
    dw: [Hom<f64, D>; WIDTH],
    alfs: [f64; WIDTH],
    omas: [f64; WIDTH],
}

/// A5.11 on the control points `pw` of a clamped `basis` of degree p ≥ 2: the knots and points of
/// the reduced candidate.
#[allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "the indices are named as in The NURBS Book's algorithm and NLib's port, and the loop is kept whole, so the two can be read side by side"
)]
fn reduce_kernel<const D: usize>(
    meter: &mut WorkMeter<'_>,
    basis: &Basis,
    pw: &[Hom<f64, D>],
) -> Result<(Vec<f64>, Vec<Hom<f64, D>>), NurbsError> {
    let up = basis.knots();
    let p = basis.degree();
    let q = sub(p, 1)?;
    let mp = sub(up.len(), 1)?;
    let s_count = span_count(basis);
    let nq = sub(sub(pw.len(), 1)?, s_count)?;
    let mq = sub(sub(mp, s_count)?, 1)?;
    let coefs = reduction_coefs(p)?;
    let zero = Hom::of([0.0; D], 0.0)?;
    let mut uq = vec_for(meter, add(mq, 1)?)?;
    uq.resize(add(mq, 1)?, 0.0);
    let mut qw = vec_for(meter, add(nq, 1)?)?;
    qw.resize(add(nq, 1)?, zero);
    let mut st = Work {
        bw: [zero; WIDTH],
        nw: [zero; WIDTH],
        dw: [zero; WIDTH],
        alfs: [0.0; WIDTH],
        omas: [0.0; WIDTH],
    };
    let mut a = p;
    let mut b = add(p, 1)?;
    let mut r = 0_usize;
    let mut first_piece = true;
    let mut spn = add(p, 1)?;
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
        let i0 = spn;
        while spn < mp && get(up, spn)? == get(up, add(spn, 1)?)? {
            spn = add(spn, 1)?;
        }
        let mlt = add(sub(spn, i0)?, 1)?;
        b = sub(add(b, mlt)?, 1)?;
        let oldr = if first_piece { 0 } else { r };
        // The end knots have p + 1 copies, r = −1, which behaves as 0 here.
        r = p.saturating_sub(mlt);
        // (oldr + 2)/2 is 1 at oldr = 0.
        let lbz = add(oldr, 2)? / 2;
        let (ua, ub) = (get(up, a)?, get(up, b)?);
        if r > 0 {
            let num = ub - ua;
            for k in (add(mlt, 1)?..=p).rev() {
                let alf = num / (get(up, add(a, k)?)? - ua);
                let slot = sub(sub(k, mlt)?, 1)?;
                set(&mut st.alfs, slot, alf)?;
                set(&mut st.omas, slot, 1.0 - alf)?;
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
        reduce_bezier(&st.bw, p, &coefs, &mut st.dw)?;
        if oldr > 0 {
            cind = remove_join(&mut qw, &mut st.dw, &uq, (ua, ub), (kind, oldr))?;
        }
        if !first_piece {
            for _ in 0..sub(q, oldr)? {
                set(&mut uq, kind, ua)?;
                kind = add(kind, 1)?;
            }
        }
        for i in lbz..=q {
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
            spn = add(spn, 1)?;
        } else {
            for i in 0..=q {
                set(&mut uq, add(kind, i)?, ub)?;
            }
        }
        first_piece = false;
    }
    Ok((uq, qw))
}

/// The knot removal of A5.11: removes the knot `ua` joining the previous reduced piece to this one
/// `oldr` times, updating the points written so far (`qw`) and the reduced piece (`dw`), and
/// averaging where the sweeps cross; returns the index at which the piece's points continue.
#[allow(
    clippy::many_single_char_names,
    reason = "the indices are named as in The NURBS Book's algorithm and NLib's port"
)]
fn remove_join<const D: usize>(
    qw: &mut [Hom<f64, D>],
    dw: &mut [Hom<f64, D>],
    uq: &[f64],
    (ua, ub): (f64, f64),
    (kind, oldr): (usize, usize),
) -> Result<usize, NurbsError> {
    let mut first = kind;
    let mut last = kind;
    let den = ub - ua;
    let mut i = first;
    for k in 0..oldr {
        i = first;
        let mut j = last;
        let mut l = sub(j, kind)?;
        while sub(j, i).unwrap_or(0) > k && j >= i {
            let left = get(uq, sub(i, 1)?)?;
            let alf = (ub - left) / (ua - left);
            let bet = (ub - get(uq, sub(sub(j, k)?, 1)?)?) / den;
            let qi = Hom::combine(alf, get(qw, sub(i, 1)?)?, 1.0 - alf, get(qw, sub(i, 2)?)?)?;
            set(qw, sub(i, 1)?, qi)?;
            let dl = Hom::combine(bet, get(dw, l)?, 1.0 - bet, get(dw, add(l, 1)?)?)?;
            set(dw, l, dl)?;
            i = add(i, 1)?;
            j = sub(j, 1)?;
            l = l.saturating_sub(1);
        }
        if j < i || sub(j, i)? < k {
            // The sweeps crossed: average the two estimates (NLib's λ, from α and β taken here).
            let u2 = get(uq, sub(i, 2)?)?;
            let u1 = get(uq, sub(i, 1)?)?;
            let alf = (ua - u2) / (ub - u2);
            let bet = (ua - u1) / (ub - u1);
            let lam = alf / (alf + bet);
            let mid = Hom::combine(lam, get(dw, add(l, 1)?)?, 1.0 - lam, get(qw, sub(i, 2)?)?)?;
            set(qw, sub(i, 2)?, mid)?;
        }
        first = sub(first, 1)?;
        last = add(last, 1)?;
    }
    sub(i, 1)
}
