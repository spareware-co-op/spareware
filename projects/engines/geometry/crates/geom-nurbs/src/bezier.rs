//! Bezier extraction of NURBS curves (The NURBS Book, algorithm A5.6): a clamped curve as one
//! Bezier curve per knot span.
//!
//! What it does: [`Curve::to_bezier`] walks the knot vector from left to right. At the end of each
//! span it inserts the end knot until it is repeated p times, which leaves the span's p + 1
//! control points as the Bezier points of that piece, and carries the points it shares with the
//! next span forward (`NLib`'s `N_CrvDecomposeBez`, the book's A5.6). Each piece is returned as a
//! curve of degree p on the knots [a, …, a, b, …, b], rational if the input is. The kernel is
//! generic over `crate::arith`'s numbers and runs in doubles and in intervals (`crate::ops`).
//!
//! Guarantees:
//!
//! - **The same curve, piece by piece, within the reported deviation.** Piece i equals the input on
//!   its span in real arithmetic; the reported deviation bounds, with certainty, how far rounding
//!   moved any piece. A priori, each homogeneous coordinate is within `γ_(10t)` · M of its exact
//!   value, with t the number of insertions made (at most p − 1 per interior knot) and M the
//!   largest absolute value of that coordinate over the input's control points: every new point
//!   is a convex combination, as in `crate::insert`.
//! - **Clamped input only.** Extraction starts from a clamped end, as A5.6 and `NLib` assume; an
//!   unclamped curve is [`NurbsError::NotClamped`]. Interior knots repeated p + 1 times (a
//!   discontinuous curve) are allowed: the pieces then do not share an end point.
//! - **Budgeted.** The pieces and the interval run are charged to the meter first.
//!
//! Method: Piegl and Tiller's algorithm A5.6 (The NURBS Book, 2nd ed., section 5.4,
//! <https://doi.org/10.1007/978-3-642-59223-2>), knot insertion until every knot has multiplicity p
//! (Boehm 1980, <https://doi.org/10.1016/0010-4485(80)90154-2>), as `NLib` implements it.
//!
//! Source: PORT from NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvTool.cpp>
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: translated to safe `no_std` Rust from `N_CrvDecomposeBez`, generic over
//!   the number type so the same steps give the pieces and their interval enclosure; an unclamped
//!   curve is refused instead of `NLib`'s skipping of extra end knots as bad data; pieces are
//!   returned as curves with their own knots.
//! SPDX-License-Identifier: MPL-2.0 AND Apache-2.0
//!
//! Cost: O(s·p²) operations for s spans, each twice; one allocation per piece and its enclosure.

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::arith::{Hom, Num};
use crate::basis::{Basis, WIDTH};
use crate::budget::vec_for;
use crate::curve::Curve;
use crate::index::{add, get, mul, set, sub};
use crate::ops::{Changed, charge_op, homs, larger, rounding, split};

/// One Bezier piece: the span [a, b] and its p + 1 homogeneous points.
pub(crate) type Piece<N, const D: usize> = (f64, f64, Vec<Hom<N, D>>);

impl<const D: usize> Curve<D> {
    /// The curve as Bezier pieces, one per knot span with more than one value, in order (A5.6).
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotClamped`] unless the curve is clamped; [`NurbsError::Stopped`] if the
    /// meter stops it; [`NurbsError::NotFinite`] if a point overflows.
    pub fn to_bezier(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<Vec<Curve<D>>>, NurbsError> {
        let basis = self.basis();
        if !basis.is_clamped() {
            return Err(NurbsError::NotClamped);
        }
        let p = basis.degree();
        let spans = span_count(basis);
        let order = add(p, 1)?;
        charge_op(meter, mul(spans, order)?, D, mul(spans, mul(order, 2)?)?)?;
        let rational = self.is_rational();
        let input_f = homs::<f64, D>(meter, self.homogeneous(), self.weights())?;
        let input_i = homs::<Interval, D>(meter, self.homogeneous(), self.weights())?;
        let pieces = decompose(meter, basis, &input_f)?;
        let enclosures = decompose(meter, basis, &input_i)?;
        let p_max = self.p_max_if(rational)?;
        let mut deviation = 0.0;
        let mut out = vec_for(meter, pieces.len())?;
        for ((a, b, pts), (_, _, enc)) in pieces.iter().zip(&enclosures) {
            let dev = rounding(pts, enc, rational, p_max)?;
            deviation = larger(deviation, dev);
            let mut knots = vec_for(meter, mul(order, 2)?)?;
            knots.extend(core::iter::repeat_n(*a, order));
            knots.extend(core::iter::repeat_n(*b, order));
            let (points, weights) = split(meter, pts, rational)?;
            out.push(Curve::from_homogeneous(
                Basis::new(p, knots)?,
                points,
                weights,
            )?);
        }
        Ok(Changed {
            value: out,
            deviation,
        })
    }
}

/// The number of knot spans of the domain with more than one value.
pub(crate) fn span_count(basis: &Basis) -> usize {
    let (lo, hi) = basis.domain();
    let mut count = 0_usize;
    let mut last = lo;
    for &k in basis.knots() {
        if k > last && k <= hi {
            count = count.saturating_add(1);
            last = k;
        }
    }
    count
}

/// A5.6 on the control points `pw` of a clamped `basis`: the Bezier pieces in order.
#[allow(
    clippy::many_single_char_names,
    reason = "the indices are named as in The NURBS Book's algorithm and NLib's port, so the two can be read side by side"
)]
pub(crate) fn decompose<N: Num, const D: usize>(
    meter: &mut WorkMeter<'_>,
    basis: &Basis,
    pw: &[Hom<N, D>],
) -> Result<Vec<Piece<N, D>>, NurbsError> {
    let up = basis.knots();
    let p = basis.degree();
    let m = sub(up.len(), 1)?;
    let mut out = vec_for(meter, span_count(basis))?;
    let zero = Hom::of([0.0; D], 0.0)?;
    let mut qw = [zero; WIDTH];
    let mut nqw = [zero; WIDTH];
    let mut alfs = [N::of(0.0)?; WIDTH];
    let mut omas = [N::of(0.0)?; WIDTH];
    let one = N::of(1.0)?;
    let mut is = p;
    let mut ie = add(p, 1)?;
    for i in 0..=p {
        set(&mut qw, i, get(pw, i)?)?;
    }
    while ie < m {
        let i0 = ie;
        while ie < m && get(up, ie)? == get(up, add(ie, 1)?)? {
            ie = add(ie, 1)?;
        }
        let mlt = add(sub(ie, i0)?, 1)?;
        if mlt < p {
            let r = sub(p, mlt)?;
            let num = N::of(get(up, ie)?)?.minus(N::of(get(up, is)?)?)?;
            for i in (add(mlt, 1)?..=p).rev() {
                let den = N::of(get(up, add(is, i)?)?)?.minus(N::of(get(up, is)?)?)?;
                let alf = num.over(den)?;
                let slot = sub(sub(i, mlt)?, 1)?;
                set(&mut alfs, slot, alf)?;
                set(&mut omas, slot, one.minus(alf)?)?;
            }
            for j in 1..=r {
                let s = add(mlt, j)?;
                let save = sub(r, j)?;
                for k in (s..=p).rev() {
                    let next = Hom::combine(
                        get(&alfs, sub(k, s)?)?,
                        get(&qw, k)?,
                        get(&omas, sub(k, s)?)?,
                        get(&qw, sub(k, 1)?)?,
                    )?;
                    set(&mut qw, k, next)?;
                }
                if ie < m {
                    set(&mut nqw, save, get(&qw, p)?)?;
                }
            }
        }
        let mut piece = vec_for(meter, add(p, 1)?)?;
        piece.extend_from_slice(qw.get(..=p).ok_or(NurbsError::Defect)?);
        out.push((get(up, is)?, get(up, ie)?, piece));
        if ie < m {
            // Points the insertions did not set come from the input.
            let start = sub(p, mlt).unwrap_or(0);
            for i in start..=p {
                set(&mut nqw, i, get(pw, add(sub(ie, p)?, i)?)?)?;
            }
            qw = nqw;
        }
        is = ie;
        ie = add(ie, 1)?;
    }
    Ok(out)
}
