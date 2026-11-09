//! Extension of NURBS curves and surfaces past one end of their domain, by the natural
//! continuation of the end span: the end span's polynomial carried on past the end, in homogeneous
//! coordinates, so a rational span continues as the same rational curve (a conic stays on its
//! conic).
//!
//! What it does: [`Curve::extend`] and [`Surface::extend`] widen the domain at one end by a
//! parameter length e. The end knot b, repeated p + 1 times in the clamped source, drops to p
//! copies and stays as the joint; every control point of the source is kept bit for bit; and p new
//! control points follow, the Bezier points of the end span's polynomial over [b, b + e], with
//! p + 1 copies of the new end knot b + e. No knot is removed. The new points come from the end
//! span's own Bezier points (the source with the span's start knot c raised to multiplicity p,
//! `crate::insert`'s kernel) by de Casteljau's algorithm at t = 1 + e/(b − c), taking the last
//! point of each level: they are the polynomial's blossoms b(1^(p−i), t^i). A low end is the high
//! end of the mirrored source (its knots negated and reversed, its points reversed), which is
//! exact. A surface runs the curve kernel on every line of its net in the extended direction
//! (`crate::net`), with the same coefficients on every line.
//!
//! Guarantees:
//!
//! - **The source kept.** Every control point and knot of the source is in the result unchanged
//!   (the end knot once fewer), and every span of the source keeps the knots its points are
//!   evaluated with, so over the source's domain the result is the source.
//! - **The continuation, certified.** The extension is the end span's polynomial continued: the
//!   result is C∞ across the joint (the joint knot is of multiplicity p, but the points across it
//!   are one polynomial's). The kernel runs in doubles for the result and in intervals for the
//!   exact construction (`crate::ops`); the reported deviation bounds, with certainty, how far
//!   the result lies from the exact continuation at every parameter, and it is only the new
//!   points' rounding, the source's points entering exactly.
//! - **Refused, never weak.** A source not clamped at that end is [`NurbsError::NotClamped`]; a
//!   length that is not positive and finite, or that does not move the end knot, is
//!   [`NurbsError::KnotOutside`]; a rational continuation whose weight may not be positive is
//!   [`NurbsError::Weight`], since its curve would pass through infinity.
//! - **Budgeted.** The result and its interval run are charged to the meter first.
//!
//! Method: the natural extension of S.-M. Hu, C.-L. Tai and S.-H. Zhang, "An extension algorithm
//! for B-splines by curve unclamping", Computer-Aided Design 34(5), 2002,
//! <https://doi.org/10.1016/S0010-4485(01)00108-7>, which keeps every derivative of the end span,
//! built here without unclamping: the end span's Bezier points by knot insertion (The NURBS Book,
//! 2nd ed., A5.1, <https://doi.org/10.1007/978-3-642-59223-2>) and their continuation by de
//! Casteljau's algorithm past the span, the blossoming of L. Ramshaw, "Blossoms are polar forms",
//! Computer Aided Geometric Design 6(4), 1989, <https://doi.org/10.1016/0167-8396(89)90032-0>.
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/6432dafe7d716a973a653fca72d5d4c7cd7002c8/crates/solidcore/src/surface/extend.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   Changed for Spare Kit: the Natural continuation of `Local::natural` (the end span's Bezier
//!   points from the span's start knot at multiplicity p, de Casteljau at 1 + e/(b − c), the
//!   source kept with its end knot as a joint of multiplicity p) is translated to safe `no_std`
//!   Rust on curves of any dimension and on surfaces line by line, generic over the number type
//!   so its rounding is certified by an interval run in place of solidcore's a priori γ bounds
//!   and local frames; the end span's Bezier points come from the kernel's own ported knot
//!   insertion; the solve for a length across the side, the Tangent, Curvature and Ruled
//!   continuations and the knot fixup are not taken.
//! Consulted: NVIDIA USD `BRep` `NLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/3fd5b9548d19aa8c12c29bc9637a96d42a06f8a5/source/SMLib/src/NL_CrvAdv.cpp>,
//!   for `N_CrvExtendByParamDist`'s `NL_CMAX` extension (infinite continuity, no knot at the
//!   join) and its refusal of weights that are not usable; its removal of the old end knot within
//!   a tolerance is not taken. No code copied.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: one knot insertion of at most p copies and p de Casteljau levels of O(p) combinations per
//! line, each twice (doubles and intervals); the result and its enclosure are allocated once. No
//! recursion.

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::arith::{Hom, Num};
use crate::basis::{Basis, Limit};
use crate::budget::vec_for;
use crate::curve::Curve;
use crate::index::{add, get, set, sub};
use crate::insert::{Insertion, insert_kernel};
use crate::net::{Net, map_lines};
use crate::ops::{Changed, Direction, charge_op, homs, larger, rounding, split};
use crate::surface::Surface;

/// Which end of a domain an extension continues past.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum End {
    /// The start of the domain: the extension runs to smaller parameters.
    Low,
    /// The end of the domain: the extension runs to larger parameters.
    High,
}

/// The knots of the mirrored shape: negated and reversed. Exact, and its own inverse.
fn mirrored(knots: &[f64]) -> Vec<f64> {
    knots.iter().rev().map(|x| -x).collect()
}

/// The new end knot of an extension by `by` past the end `b` of the high end: a double strictly
/// past `b`.
fn new_end(b: f64, by: f64) -> Result<f64, NurbsError> {
    let end = b + by;
    if by.is_finite() && by > 0.0 && end.is_finite() && end > b {
        Ok(end)
    } else {
        Err(NurbsError::KnotOutside)
    }
}

/// Whether `knots` (degree `p`) end in p + 1 copies of the last knot.
fn clamped_high(knots: &[f64], p: usize) -> bool {
    let last = knots.last();
    last.is_some()
        && knots
            .iter()
            .rev()
            .take(p.saturating_add(1))
            .all(|k| Some(k) == last)
        && knots.len() > p.saturating_add(1)
}

/// The natural continuation of the high end of the curve with `knots` (degree `p`, clamped at its
/// high end) and control points `pw` to the new end knot `end`: the new knots and points.
#[allow(
    clippy::many_single_char_names,
    reason = "the names are the construction's: p the degree, b and c the end span's knots, t the de Casteljau parameter"
)]
pub(crate) fn natural_kernel<N: Num, const D: usize>(
    meter: &mut WorkMeter<'_>,
    knots: &[f64],
    p: usize,
    pw: &[Hom<N, D>],
    end: f64,
) -> Result<(Vec<f64>, Vec<Hom<N, D>>), NurbsError> {
    let len = knots.len();
    let b = get(knots, sub(len, 1)?)?;
    // c: the knot before the end's p + 1 copies, the end span's start.
    let c = get(knots, sub(len, add(p, 2)?)?)?;
    if c >= b {
        return Err(NurbsError::Defect);
    }
    // The end span's Bezier points: the last p + 1 points once c is repeated p times.
    let s = knots.iter().filter(|&&k| k == c).count();
    let bezier_source = if s < p {
        let basis = Basis::new(p, knots.to_vec())?;
        let job = Insertion {
            u: c,
            span: basis.span(c, Limit::FromRight)?,
            s,
            times: sub(p, s)?,
        };
        insert_kernel(meter, knots, p, pw, &job)?.1
    } else {
        pw.to_vec()
    };
    let start = sub(bezier_source.len(), add(p, 1)?)?;
    let mut level = vec_for(meter, add(p, 1)?)?;
    level.extend_from_slice(bezier_source.get(start..).ok_or(NurbsError::Defect)?);
    // de Casteljau past the span: t = 1 + e/(b − c), e = end − b exactly as the knots hold it.
    let one = N::of(1.0)?;
    let t = one.plus(
        N::of(end)?
            .minus(N::of(b)?)?
            .over(N::of(b)?.minus(N::of(c)?)?)?,
    )?;
    let omt = one.minus(t)?;
    let mut points = vec_for(meter, add(pw.len(), p)?)?;
    points.extend_from_slice(pw);
    for r in 1..=p {
        let top = sub(p, r)?;
        for i in 0..=top {
            let next = Hom::combine(omt, get(&level, i)?, t, get(&level, add(i, 1)?)?)?;
            set(&mut level, i, next)?;
        }
        points.push(get(&level, top)?);
    }
    let mut new_knots = vec_for(meter, add(len, p)?)?;
    new_knots.extend_from_slice(knots.get(..sub(len, 1)?).ok_or(NurbsError::Defect)?);
    new_knots.extend(core::iter::repeat_n(end, add(p, 1)?));
    Ok((new_knots, points))
}

/// The natural continuation at `which` end: mirrored in and out for a low end.
fn natural_at<N: Num, const D: usize>(
    meter: &mut WorkMeter<'_>,
    knots: &[f64],
    p: usize,
    pw: &[Hom<N, D>],
    which: End,
    end: f64,
) -> Result<(Vec<f64>, Vec<Hom<N, D>>), NurbsError> {
    match which {
        End::High => natural_kernel(meter, knots, p, pw, end),
        End::Low => {
            let flipped: Vec<Hom<N, D>> = pw.iter().rev().copied().collect();
            let (k, q) = natural_kernel(meter, &mirrored(knots), p, &flipped, -end)?;
            Ok((mirrored(&k), q.into_iter().rev().collect()))
        }
    }
}

/// The checks of an extension of `basis` at `which` end by `by`: clamped there, and the new end
/// knot, a double strictly past the old end.
fn check_extension(basis: &Basis, which: End, by: f64) -> Result<f64, NurbsError> {
    let p = basis.degree();
    let knots = basis.knots();
    let (lo, hi) = basis.domain();
    match which {
        End::High => {
            if !clamped_high(knots, p) {
                return Err(NurbsError::NotClamped);
            }
            new_end(hi, by)
        }
        End::Low => {
            if !clamped_high(&mirrored(knots), p) {
                return Err(NurbsError::NotClamped);
            }
            Ok(-new_end(-lo, by)?)
        }
    }
}

/// An upper bound on |S| over a shape whose exact homogeneous control points `enclosure` holds:
/// the largest distance of a control point from the origin (the convex hull property, which
/// needs every weight positive: a weight that may not be is [`NurbsError::Weight`]).
fn enclosed_p_max<const D: usize>(enclosure: &[Hom<Interval, D>]) -> Result<f64, NurbsError> {
    let mut best = 0.0;
    for (index, h) in enclosure.iter().enumerate() {
        if h.w.lo().is_nan() || h.w.lo() <= 0.0 {
            return Err(NurbsError::Weight { index });
        }
        let mut coords = [Interval::of(0.0)?; D];
        for (c, &x) in coords.iter_mut().zip(&h.c) {
            *c = x.over(h.w)?;
        }
        let norm = Interval::norm(&coords)
            .map_err(|_| NurbsError::NotFinite)?
            .hi();
        best = larger(best, norm);
    }
    Ok(best)
}

impl<const D: usize> Curve<D> {
    /// The curve continued past `which` end of its domain by the parameter length `by`: the end
    /// span's polynomial carried on, the source's points and knots kept (module documentation).
    /// The deviation bounds the result's distance from the exact continuation at every
    /// parameter.
    ///
    /// # Errors
    ///
    /// [`NurbsError::NotClamped`] unless the curve is clamped at that end;
    /// [`NurbsError::KnotOutside`] for a length that is not positive and finite or does not move
    /// the end knot; [`NurbsError::Weight`] for a rational continuation whose weight may not be
    /// positive; [`NurbsError::Stopped`] if the meter stops it; [`NurbsError::NotFinite`] if a new
    /// point overflows.
    pub fn extend(
        &self,
        which: End,
        by: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<Curve<D>>, NurbsError> {
        let basis = self.basis();
        let end = check_extension(basis, which, by)?;
        let p = basis.degree();
        let count = add(self.homogeneous().len(), p)?;
        charge_op(meter, count, D, add(basis.knots().len(), p)?)?;
        let rational = self.is_rational();
        let input_f = homs::<f64, D>(meter, self.homogeneous(), self.weights())?;
        let input_i = homs::<Interval, D>(meter, self.homogeneous(), self.weights())?;
        let (knots, result) = natural_at(meter, basis.knots(), p, &input_f, which, end)?;
        let (_, enclosure) = natural_at(meter, basis.knots(), p, &input_i, which, end)?;
        let pm = if rational {
            enclosed_p_max(&enclosure)?
        } else {
            0.0
        };
        let deviation = rounding(&result, &enclosure, rational, pm)?;
        let (points, weights) = split(meter, &result, rational)?;
        let curve = Curve::from_homogeneous(Basis::new(p, knots)?, points, weights)?;
        Ok(Changed {
            value: curve,
            deviation,
        })
    }
}

impl<const D: usize> Surface<D> {
    /// The surface continued past `which` end of its domain in direction `dir` by the parameter
    /// length `by`: every line of the net in that direction continued as [`Curve::extend`]
    /// continues a curve, with the same knots.
    ///
    /// # Errors
    ///
    /// As [`Curve::extend`], for the basis of direction `dir`.
    pub fn extend(
        &self,
        dir: Direction,
        which: End,
        by: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<Surface<D>>, NurbsError> {
        let basis = self.basis_of(dir).clone();
        let end = check_extension(&basis, which, by)?;
        let p = basis.degree();
        let len = add(basis.count(), p)?;
        charge_op(
            meter,
            self.net_size(dir, len)?,
            D,
            add(basis.knots().len(), p)?,
        )?;
        let knots = basis.knots();
        let net_f: Net<f64, D> = self.net(meter)?;
        let net_i: Net<Interval, D> = self.net(meter)?;
        let (new_knots, result) = map_lines(meter, &net_f, dir, |m, line| {
            natural_at(m, knots, p, line, which, end)
        })?;
        let (_, enclosure) = map_lines(meter, &net_i, dir, |m, line| {
            natural_at(m, knots, p, line, which, end)
        })?;
        let rational = self.is_rational();
        let pm = if rational {
            enclosed_p_max(&enclosure.points)?
        } else {
            0.0
        };
        let deviation = rounding(&result.points, &enclosure.points, rational, pm)?;
        let (points, weights) = split(meter, &result.points, rational)?;
        let new_basis = Basis::new(p, new_knots)?;
        let (bu, bv) = match dir {
            Direction::U => (new_basis, self.basis_v().clone()),
            Direction::V => (self.basis_u().clone(), new_basis),
        };
        Ok(Changed {
            value: Surface::from_homogeneous(bu, bv, points, weights)?,
            deviation,
        })
    }
}
