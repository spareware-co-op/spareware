//! Knot removal of NURBS curves (The NURBS Book, algorithm A5.8), certified: a knot is removed
//! only while the curve provably stays within the tolerance asked for.
//!
//! What it does: [`Curve::remove_knot`] removes the knot u up to `times` times. Each removal
//! solves for the new control points from both ends of the affected run, as A5.8 and `NLib`'s
//! `N_CrvRemoveKnot` do (`P_i` = `α_i`·`Q_i` + (1 − `α_i`)·`Q_(i−1)` solved for Q), and where the two
//! sweeps cross it averages their two estimates with `NLib`'s weight λ = α / (α + β). Whether a
//! removal is kept is not decided from those formulas: the candidate curve is re-inserted (u put
//! back as many times as it was removed, Boehm's algorithm) in interval arithmetic, which gives
//! the input's knots and an enclosure of the control points the candidate has in the input's
//! basis; the two curves differ by Σ `N_l`·(`P̃_l` − `P_l`) on that basis, so the largest enclosed
//! |`P̃_l` − `P_l`| bounds their distance (with the rational form of `crate::ops`). A removal is kept
//! only while that certified bound stays within the tolerance; the bound of the last removal kept
//! is the deviation returned.
//!
//! Guarantees:
//!
//! - **Within the tolerance, certified.** The returned curve differs from the input by at most the
//!   returned deviation everywhere in the domain, and the deviation is at most `tol`. The bound
//!   covers the removal's own change and the rounding of every new control point, because it is
//!   measured on the doubles returned.
//! - **Never worse than asked.** Where `NLib`'s test can pass a knot whose true change exceeds the
//!   tolerance (its basis maxima come from a numerical search, and its rational test uses the
//!   input's weights), this one cannot; it may remove fewer knots than an uncertified test.
//! - **Typed refusals.** A tolerance that is negative or not finite is [`NurbsError::Tolerance`]; u
//!   not an interior knot is [`NurbsError::KnotOutside`]; u repeated p + 1 times (where the curve
//!   may jump) is [`NurbsError::Discontinuous`]. A removal that would make a weight not positive is
//!   not kept.
//! - **Budgeted.** Each attempt is charged to the meter before it is computed.
//!
//! Method: Tiller's knot removal (W. Tiller, "Knot-removal algorithms for NURBS curves and
//! surfaces", *Computer-Aided Design* 24(8) (1992), 445–453,
//! <https://doi.org/10.1016/0010-4485(92)90012-Y>) as Piegl and Tiller's algorithm A5.8 gives it
//! (The NURBS Book, 2nd ed., section 5.4, <https://doi.org/10.1007/978-3-642-59223-2>) and `NLib`
//! implements it, with the change bounded through re-insertion and the partition of unity
//! (properties P2.4 and P4.4 of the book) in interval arithmetic (`crate::arith`).
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvTool.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated to safe `no_std` Rust from `N_CrvRemoveKnot`, one removal
//!   at a time; the decision to keep a removal uses a certified bound from re-insertion in
//!   interval arithmetic in place of `NLib`'s distance test with basis maxima from a numerical
//!   search, its rescaled tolerance for rational curves and its weight range; a new curve is
//!   returned with the deviation instead of being edited in place.
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: each removal takes O(n·D) operations to copy the points, O(p·D) to solve, and O(n + p²)
//! in intervals to certify; at most min(times, multiplicity) removals are tried.

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::arith::{Hom, Num};
use crate::basis::{Basis, Limit, WIDTH};
use crate::budget::vec_for;
use crate::curve::Curve;
use crate::index::{add, get, mul, set, sub};
use crate::insert::{Insertion, insert_kernel};
use crate::ops::{Changed, charge_op, homs, larger, smaller, split, usable_weight};

impl<const D: usize> Curve<D> {
    /// The curve with the knot `u` removed as many times as possible, up to `times`, while it
    /// stays within `tol` of this curve (A5.8), and the number of times it was removed.
    ///
    /// # Errors
    ///
    /// [`NurbsError::Tolerance`] if `tol` is negative or not finite; [`NurbsError::KnotOutside`]
    /// unless `u` is a knot strictly inside the domain; [`NurbsError::Discontinuous`] if `u` is
    /// repeated p + 1 times; [`NurbsError::Stopped`] if the meter stops it.
    pub fn remove_knot(
        &self,
        u: f64,
        times: usize,
        tol: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<Curve<D>>, usize), NurbsError> {
        if !(tol.is_finite() && tol >= 0.0) {
            return Err(NurbsError::Tolerance);
        }
        let basis = self.basis();
        let p = basis.degree();
        let (lo, hi) = basis.domain();
        let s = basis.multiplicity(u);
        if u.is_nan() || u <= lo || u >= hi || s == 0 {
            return Err(NurbsError::KnotOutside);
        }
        if s > p {
            return Err(NurbsError::Discontinuous);
        }
        let rational = self.is_rational();
        let p_max = self.p_max_if(rational)?;
        let original = homs::<f64, D>(meter, self.homogeneous(), self.weights())?;
        let mut knots = basis.knots().to_vec();
        let mut points = original.clone();
        let mut removed = 0;
        let mut deviation = 0.0;
        let tries = times.min(s);
        for step in 1..=tries {
            charge_op(meter, points.len(), D, knots.len())?;
            let r = knots
                .iter()
                .rposition(|&k| k == u)
                .ok_or(NurbsError::Defect)?;
            let here = sub(s, removed)?;
            let (k2, q2) = remove_once(meter, &knots, p, &points, u, (r, here))?;
            if rational && !q2.iter().all(|h| usable_weight(h.w)) {
                break;
            }
            let Some(dev) = certify(
                meter,
                &original,
                basis.knots(),
                (&k2, &q2),
                (p, u, step),
                (rational, p_max),
            )?
            else {
                break;
            };
            if dev > tol {
                break;
            }
            knots = k2;
            points = q2;
            removed = step;
            deviation = dev;
        }
        let (pts, weights) = split(meter, &points, rational)?;
        let curve = Curve::from_homogeneous(Basis::new(p, knots)?, pts, weights)?;
        Ok((
            Changed {
                value: curve,
                deviation,
            },
            removed,
        ))
    }
}

/// One removal of `u` from the curve with knots `knots` and points `pts`, where u's last copy is
/// knot `r` and it has multiplicity `s` (A5.8 for one removal, as `NLib` computes it).
#[allow(
    clippy::many_single_char_names,
    reason = "the indices are named as in The NURBS Book's algorithm and NLib's port, so the two can be read side by side"
)]
fn remove_once<const D: usize>(
    meter: &mut WorkMeter<'_>,
    knots: &[f64],
    p: usize,
    pts: &[Hom<f64, D>],
    u: f64,
    (r, s): (usize, usize),
) -> Result<(Vec<f64>, Vec<Hom<f64, D>>), NurbsError> {
    let first = sub(r, p)?;
    let last = sub(r, s)?;
    let off = sub(first, 1)?;
    let zero = Hom::of([0.0; D], 0.0)?;
    let mut rw = [zero; 2 * WIDTH];
    set(&mut rw, 0, get(pts, off)?)?;
    set(&mut rw, sub(add(last, 1)?, off)?, get(pts, add(last, 1)?)?)?;
    let (mut i, mut j) = (first, last);
    let (mut ii, mut jj) = (1_usize, sub(last, off)?);
    let (mut alf, mut bet) = (0.5, 0.5);
    while j > i {
        let ui = get(knots, i)?;
        alf = (get(knots, add(add(i, p)?, 1)?)? - ui) / (u - ui);
        let uj = get(knots, add(add(j, p)?, 1)?)?;
        bet = (uj - get(knots, j)?) / (uj - u);
        let left = Hom::combine(alf, get(pts, i)?, 1.0 - alf, get(&rw, sub(ii, 1)?)?)?;
        set(&mut rw, ii, left)?;
        let right = Hom::combine(bet, get(pts, j)?, 1.0 - bet, get(&rw, add(jj, 1)?)?)?;
        set(&mut rw, jj, right)?;
        i = add(i, 1)?;
        j = sub(j, 1)?;
        ii = add(ii, 1)?;
        jj = sub(jj, 1)?;
    }
    if j < i {
        // The sweeps crossed: average their two estimates of the middle point (NLib's λ).
        let lam = alf / (alf + bet);
        let mid = Hom::combine(
            lam,
            get(&rw, add(jj, 1)?)?,
            1.0 - lam,
            get(&rw, sub(ii, 1)?)?,
        )?;
        set(&mut rw, add(jj, 1)?, mid)?;
    }
    let mut out = vec_for(meter, sub(pts.len(), 1)?)?;
    let fout = sub(mul(r, 2)?, add(s, p)?)? / 2;
    let (mut i, mut j) = (first, last);
    let mut saved = vec_for(meter, pts.len())?;
    saved.extend_from_slice(pts);
    while j > i {
        set(&mut saved, i, get(&rw, sub(i, off)?)?)?;
        set(&mut saved, j, get(&rw, sub(j, off)?)?)?;
        i = add(i, 1)?;
        j = sub(j, 1)?;
    }
    for (index, h) in saved.iter().enumerate() {
        if index != fout {
            out.push(*h);
        }
    }
    let mut k2 = vec_for(meter, sub(knots.len(), 1)?)?;
    for (index, &k) in knots.iter().enumerate() {
        if index != r {
            k2.push(k);
        }
    }
    Ok((k2, out))
}

/// The certified distance between the input curve (knots `knots`, points `original`) and the
/// candidate (`candidate`, with u removed `times` times), by re-inserting u in intervals; `None`
/// if the re-insertion does not give back the input's knots (which would be a defect).
fn certify<const D: usize>(
    meter: &mut WorkMeter<'_>,
    original: &[Hom<f64, D>],
    knots: &[f64],
    candidate: (&[f64], &[Hom<f64, D>]),
    (p, u, times): (usize, f64, usize),
    (rational, p_max): (bool, f64),
) -> Result<Option<f64>, NurbsError> {
    let (k2, q2) = candidate;
    let reduced = Basis::new(p, k2.to_vec())?;
    let span = reduced.span(u, Limit::FromRight)?;
    let s = reduced.multiplicity(u);
    let mut enclosed = vec_for(meter, q2.len())?;
    for h in q2 {
        enclosed.push(Hom::<Interval, D>::of(h.c, h.w)?);
    }
    let job = Insertion { u, span, s, times };
    let (back, pw) = insert_kernel(meter, k2, p, &enclosed, &job)?;
    if back.as_slice() != knots || pw.len() != original.len() {
        return Ok(None);
    }
    let w_min = q2.iter().fold(f64::INFINITY, |m, h| smaller(m, h.w));
    Ok(Some(difference_bound(
        original, &pw, rational, p_max, w_min,
    )?))
}

/// An upper bound on the distance between two shapes on one basis whose control points differ by
/// `enclosure − exact`: the largest |ΔP| for a nonrational shape, and for a rational one
/// (max |Δ(w·P)| + `p_max`·max |Δw|) / `w_min`, with `w_min` a lower bound on the second shape's
/// weight function (`crate::ops`).
pub(crate) fn difference_bound<const D: usize>(
    exact: &[Hom<f64, D>],
    enclosure: &[Hom<Interval, D>],
    rational: bool,
    p_max: f64,
    w_min: f64,
) -> Result<f64, NurbsError> {
    let magnitude = |e: Interval, x: f64| -> Result<f64, NurbsError> {
        let d = e.minus(Interval::of(x)?)?;
        let (a, b) = (-d.lo(), d.hi());
        Ok(larger(a, b))
    };
    let (mut point_dev, mut weight_dev) = (0.0, 0.0);
    for (x, e) in exact.iter().zip(enclosure) {
        let mut mags = [Interval::of(0.0)?; D];
        for (m, (&ed, &xd)) in mags.iter_mut().zip(e.c.iter().zip(&x.c)) {
            *m = Interval::of(magnitude(ed, xd)?)?;
        }
        let norm = Interval::norm(&mags)
            .map_err(|_| NurbsError::NotFinite)?
            .hi();
        point_dev = larger(point_dev, norm);
        let wd = magnitude(e.w, x.w)?;
        weight_dev = larger(weight_dev, wd);
    }
    if !rational {
        return Ok(point_dev);
    }
    if !usable_weight(w_min) {
        return Err(NurbsError::NotFinite);
    }
    let top =
        Interval::of(point_dev)?.plus(Interval::of(p_max)?.times(Interval::of(weight_dev)?)?)?;
    Ok(top.over(Interval::of(w_min)?)?.hi())
}
