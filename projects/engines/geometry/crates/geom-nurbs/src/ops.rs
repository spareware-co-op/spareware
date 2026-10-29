//! What every knot operation returns, and the certified bound on how far its result lies from the
//! curve or surface it was given.
//!
//! What it does: a knot operation returns a [`Changed`] value: the new curve, curves or surface,
//! and `deviation`, an upper bound on the distance between the new shape and the old one at every
//! parameter of the domain. For an operation that is exact in real arithmetic (insertion,
//! refinement, Bezier extraction, degree elevation) the deviation is only the rounding of the new
//! control points; for an approximation (knot removal, degree reduction) it also holds the
//! approximation's own error. [`rounding`] turns the interval run of an operation (`crate::arith`)
//! into the rounding part: chapter 7's rule that a conversion "returns the NURBS, whether it is
//! exact, and its maximum error".
//!
//! Guarantees:
//!
//! - **Certified.** For a nonrational result with control points Q̂ whose exact counterparts are Q,
//!   the shapes differ by Σ `N_i`·(`Q̂_i` − `Q_i`), at most `max_i` |`Q̂_i` − `Q_i`| because the basis
//!   functions are not negative and sum to 1; the interval run bounds each |`Q̂_i` − `Q_i`| by the
//!   Euclidean norm of its interval's widths, rounded up. For a rational result, S = A / W and
//!   Ŝ − S = ((Â − A) − S·(Ŵ − W)) / Ŵ, so |Ŝ − S| ≤ (max |Δ(w·Q)| + `P_max`·max |Δw|) / min ŵ,
//!   where `P_max` bounds |S| (the largest distance of an input control point from the origin, by
//!   the convex hull property) and min ŵ is the smallest weight of the result, which bounds Ŵ
//!   below. Every step is computed in interval arithmetic and its upper bound taken.
//! - **Refused, never weak.** A result with a weight that is not positive is
//!   [`NurbsError::Weight`]; a bound that is not finite is [`NurbsError::NotFinite`].
//!
//! Method: the partition of unity and the convex hull property of B-splines (The NURBS Book, 2nd
//! ed., Springer (1997), properties P2.4, P3.4 and P4.4, <https://doi.org/10.1007/978-3-642-59223-2>),
//! with interval arithmetic for every bound (`crate::arith`).
//!
//! Cost: linear in the number of control points; [`homs`] and [`split`] make one allocation each.

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::arith::{Hom, Num};
use crate::budget::{charge, vec_for};

/// A knot operation's result and a certified upper bound on how far it lies from its input.
#[derive(Clone, Debug, PartialEq)]
pub struct Changed<T> {
    /// The new curve, curves or surface.
    pub value: T,
    /// An upper bound on the distance between the new shape and the old one, over the whole
    /// domain: the rounding of the new control points, plus the approximation of an operation that
    /// approximates.
    pub deviation: f64,
}

/// A parameter direction of a surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    /// The first parameter, u: the rows of the net.
    U,
    /// The second parameter, v: the columns of the net.
    V,
}

/// Charges for a knot operation with `points` result points of dimension `dim` and `knots` knots:
/// the result in doubles and the interval run beside it, which holds two doubles per number.
pub(crate) fn charge_op(
    meter: &mut WorkMeter<'_>,
    points: usize,
    dim: usize,
    knots: usize,
) -> Result<(), NurbsError> {
    // A result point and its enclosure: 3 · (dim + 1) doubles, charged as three weighted points.
    charge(meter, points.saturating_mul(3), dim, true, knots)
}

/// The control points as homogeneous points in `N`, with weight 1 for a nonrational shape.
pub(crate) fn homs<N: Num, const D: usize>(
    meter: &mut WorkMeter<'_>,
    points: &[[f64; D]],
    weights: Option<&[f64]>,
) -> Result<Vec<Hom<N, D>>, NurbsError> {
    let mut out = vec_for(meter, points.len())?;
    for (i, &p) in points.iter().enumerate() {
        let w = match weights {
            Some(ws) => ws.get(i).copied().ok_or(NurbsError::Defect)?,
            None => 1.0,
        };
        out.push(Hom::of(p, w)?);
    }
    Ok(out)
}

/// The result's control points and, for a rational shape, its weights.
pub(crate) type Parts<const D: usize> = (Vec<[f64; D]>, Option<Vec<f64>>);

/// Splits homogeneous result points into control points and weights (dropped if `rational` is
/// false).
pub(crate) fn split<const D: usize>(
    meter: &mut WorkMeter<'_>,
    homs: &[Hom<f64, D>],
    rational: bool,
) -> Result<Parts<D>, NurbsError> {
    let mut points = vec_for(meter, homs.len())?;
    points.extend(homs.iter().map(|h| h.c));
    let weights = if rational {
        let mut w = vec_for(meter, homs.len())?;
        w.extend(homs.iter().map(|h| h.w));
        Some(w)
    } else {
        None
    };
    Ok((points, weights))
}

/// An upper bound on the largest distance of a control point from the origin, |w·P / w|.
pub(crate) fn p_max<const D: usize>(
    points: &[[f64; D]],
    weights: Option<&[f64]>,
) -> Result<f64, NurbsError> {
    let mut best = 0.0;
    for (i, p) in points.iter().enumerate() {
        let w = match weights {
            Some(ws) => Interval::of(ws.get(i).copied().ok_or(NurbsError::Defect)?)?,
            None => Interval::of(1.0)?,
        };
        let mut coords = [Interval::of(0.0)?; D];
        for (c, &x) in coords.iter_mut().zip(p) {
            *c = Interval::of(x)?.over(w)?;
        }
        let norm = Interval::norm(&coords)
            .map_err(|_| NurbsError::NotFinite)?
            .hi();
        best = larger(best, norm);
    }
    Ok(best)
}

/// The certified rounding of a result: how far the shape with control points `result` can lie
/// from the shape with the exact points that `enclosure` holds (see the module's guarantees).
/// Whether `w` can be a weight: finite and above zero. The one test of a weight in the crate.
pub(crate) fn usable_weight(w: f64) -> bool {
    w.is_finite() && w > 0.0
}

/// The larger of two bounds, neither NaN. At a tie either is the same number: every bound here is
/// an interval's upper end, a width, a weight or an absolute value, never −0 (intervals keep a zero
/// end as +0).
pub(crate) fn larger(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// The smaller of two weights or bounds, neither NaN; at a tie either is the same number, as for
/// [`larger`].
pub(crate) fn smaller(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

pub(crate) fn rounding<const D: usize>(
    result: &[Hom<f64, D>],
    enclosure: &[Hom<Interval, D>],
    rational: bool,
    p_max: f64,
) -> Result<f64, NurbsError> {
    let width = |i: Interval| -> Result<Interval, NurbsError> {
        Interval::of(i.hi())?.minus(Interval::of(i.lo())?)
    };
    let mut point_dev = 0.0;
    let mut weight_dev = 0.0;
    let mut w_min = f64::INFINITY;
    if result.len() != enclosure.len() {
        return Err(NurbsError::Defect);
    }
    for (index, (r, e)) in result.iter().zip(enclosure).enumerate() {
        let mut widths = [Interval::of(0.0)?; D];
        for (wd, &ed) in widths.iter_mut().zip(&e.c) {
            *wd = width(ed)?;
        }
        let norm = Interval::norm(&widths)
            .map_err(|_| NurbsError::NotFinite)?
            .hi();
        point_dev = larger(point_dev, norm);
        if rational {
            weight_dev = larger(weight_dev, width(e.w)?.hi());
            if !usable_weight(r.w) {
                return Err(NurbsError::Weight { index });
            }
            w_min = smaller(w_min, r.w);
        }
    }
    if !rational {
        return Ok(point_dev);
    }
    let top =
        Interval::of(point_dev)?.plus(Interval::of(p_max)?.times(Interval::of(weight_dev)?)?)?;
    let bound = top.over(Interval::of(w_min)?)?.hi();
    if bound.is_finite() {
        Ok(bound)
    } else {
        Err(NurbsError::NotFinite)
    }
}
