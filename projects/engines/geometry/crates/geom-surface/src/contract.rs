//! The kind contract for surfaces: the laws every surface kind must keep, as a check any kind's
//! tests run (`projects/engines/geometry/AGENTS.md`, "How to add a geometry kind").
//!
//! What it does: [`check`] runs a kind through the laws of `geom-curve`'s [`Law`], read for
//! surfaces, at the parameter pairs given, and returns the first law broken. As for curves, the
//! laws compare certified objects: enclosures of one exact value must overlap after widening by the
//! stated deviations; the normal bound must keep the sign of every coordinate it decides; doubles
//! must lie in the certified enclosures within a relative 2^−30 (a wrong formula, not rounding).
//!
//! Guarantees:
//!
//! - **Sound.** A law reported broken is broken (two certified enclosures of one value that miss
//!   each other, a sign the normal bound decides that the exact normal contradicts, or a refusal
//!   that did not come).
//! - **Deterministic.** Parameters in the order given, laws in a fixed order.
//!
//! Method: as `geom-curve`'s contract (chapter 28.2's contract laws, applied to the behaviour of
//! chapter 7), on the inclusion property of interval arithmetic (Moore, Kearfott and Cloud,
//! <https://doi.org/10.1137/1.9780898717716>).
//!
//! Cost: for each parameter pair, a few enclosures and bounds, a split, a reversal and a transform.

#![allow(
    clippy::many_single_char_names,
    reason = "the contract's names: u and v for parameters, e, p, h, r, m and n for enclosures"
)]

use geom_curve::affine::Entries;
use geom_curve::tight;
use geom_curve::{ContractError, CurveError, DerivativeBound, Law, ParamMap};
use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Direction, Limit};
use geom_run::WorkMeter;

use crate::param::{ParamSurface, SurfaceForm};

/// The relative slack between doubles and the certified enclosure: 2^−30.
const SLACK: f64 = f64::from_bits(0x3E10_0000_0000_0000);

fn broken(law: Law, at: f64, error: Option<CurveError>) -> ContractError {
    ContractError { law, at, error }
}

fn overlap(a: &[Interval; 3], b: &[Interval; 3], widen: f64) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| x.lo() - widen <= y.hi() + widen && y.lo() - widen <= x.hi() + widen)
}

fn meets(a: &[Interval; 3], b: &Bounds<3>) -> bool {
    a.iter()
        .zip(b.lo.iter().zip(&b.hi))
        .all(|(x, (&lo, &hi))| x.lo() <= hi && lo <= x.hi())
}

fn near(v: &[f64; 3], a: &[Interval; 3]) -> bool {
    v.iter().zip(a).all(|(&x, i)| {
        let scale = 1.0 + x.abs() + i.lo().abs() + i.hi().abs();
        i.lo() - SLACK * scale <= x && x <= i.hi() + SLACK * scale
    })
}

/// The cross product of two enclosed vectors.
fn cross(a: &[Interval; 3], b: &[Interval; 3]) -> Result<[Interval; 3], CurveError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    Ok([
        a1.mul(b2)?.sub(a2.mul(b1)?)?,
        a2.mul(b0)?.sub(a0.mul(b2)?)?,
        a0.mul(b1)?.sub(a1.mul(b0)?)?,
    ])
}

/// Runs `kind` through every law at the parameter pairs `params` (each inside its domain), with
/// `transform` for the transform law. Split and reversal are checked in u.
///
/// # Errors
///
/// The first [`ContractError`] found, its `at` the pair's u.
#[allow(
    clippy::too_many_lines,
    reason = "one law after another, in the order of Law, each a few lines"
)]
pub fn check<K: ParamSurface>(
    kind: &K,
    params: &[(f64, f64)],
    transform: &TypedTransform,
    meter: &mut WorkMeter<'_>,
) -> Result<(), ContractError> {
    let ((a, b), (c, d)) = kind.domain();
    let call = |law: Law, at: f64| move |e: CurveError| broken(law, at, Some(e));
    let sides = (Limit::FromRight, Limit::FromRight);
    // One domain length and one unit past the end.
    let beyond = b + (b - a);
    let outside = beyond + 1.0;
    if kind.point(outside, c, sides) != Err(CurveError::ParameterOutside)
        || kind.bound(((a - 1.0, b), (c, d)), meter) != Err(CurveError::SpanOutside)
        || kind.split(Direction::U, a, meter).err() != Some(CurveError::ParameterOutside)
    {
        return Err(broken(Law::RefusesOutside, outside, None));
    }
    let whole_box = ((a, b), (c, d));
    let whole = kind
        .bound(whole_box, meter)
        .map_err(call(Law::BoundHoldsPoints, a))?;
    let normals = kind
        .normal_bound(whole_box, meter)
        .map_err(call(Law::NormalBoundHoldsDirections, a))?;
    let entries = Entries::of(transform).map_err(call(Law::TransformMovesTheCurve, a))?;
    let mid = f64::midpoint(a, b);
    let (left, right) = kind
        .split(Direction::U, mid, meter)
        .map_err(call(Law::SplitKeepsTheCurve, mid))?;
    let reversed = kind
        .reversed(Direction::U, meter)
        .map_err(call(Law::ReverseKeepsTheCurve, a))?;
    let moved = kind
        .transformed(transform, meter)
        .map_err(call(Law::TransformMovesTheCurve, a))?;
    let nurbs = kind
        .to_nurbs(meter)
        .map_err(call(Law::NurbsWithinItsError, a))?;
    for &(u, v) in params {
        let e = kind
            .enclose(u, v, sides, meter)
            .map_err(call(Law::EvaluationIsEnclosed, u))?;
        let dbl = kind
            .partials(u, v, sides)
            .map_err(call(Law::EvaluationIsEnclosed, u))?;
        if !dbl.iter().zip(&e).all(|(x, y)| near(x, y)) {
            return Err(broken(Law::EvaluationIsEnclosed, u, None));
        }
        let [p, su, sv] = e;
        let region = (
            (f64::midpoint(a, u), f64::midpoint(u, b)),
            (f64::midpoint(c, v), f64::midpoint(v, d)),
        );
        let local = kind
            .bound(region, meter)
            .map_err(call(Law::BoundHoldsPoints, u))?;
        if !meets(&p, &whole) || !meets(&p, &local) {
            return Err(broken(Law::BoundHoldsPoints, u, None));
        }
        for (dir, dv) in [(Direction::U, su), (Direction::V, sv)] {
            if let DerivativeBound::Bounded(bx) = kind
                .derivative_bound(region, dir, meter)
                .map_err(call(Law::DerivativeBoundHoldsDerivatives, u))?
                && !meets(&dv, &bx)
            {
                return Err(broken(Law::DerivativeBoundHoldsDerivatives, u, None));
            }
        }
        // The normal bound decides a sign only where the exact normal has it.
        let n = cross(&su, &sv).map_err(call(Law::NormalBoundHoldsDirections, u))?;
        for (x, (&lo, &hi)) in n.iter().zip(normals.lo.iter().zip(&normals.hi)) {
            if (lo > 0.0 && x.hi() <= 0.0) || (hi < 0.0 && x.lo() >= 0.0) {
                return Err(broken(Law::NormalBoundHoldsDirections, u, None));
            }
        }
        let ne = nurbs_at(&nurbs.value, u, v, meter).map_err(call(Law::NurbsWithinItsError, u))?;
        if !overlap(&ne, &p, nurbs.deviation) {
            return Err(broken(Law::NurbsWithinItsError, u, None));
        }
        let (half, side) = if u <= mid {
            (&left, Limit::FromLeft)
        } else {
            (&right, Limit::FromRight)
        };
        let h = half
            .value
            .enclose(u, v, (side, Limit::FromRight), meter)
            .map_err(call(Law::SplitKeepsTheCurve, u))?;
        let pe = kind
            .enclose(u, v, (side, Limit::FromRight), meter)
            .map_err(call(Law::SplitKeepsTheCurve, u))?;
        if !h
            .first()
            .zip(pe.first())
            .is_some_and(|(x, y)| overlap(x, y, half.deviation))
        {
            return Err(broken(Law::SplitKeepsTheCurve, u, None));
        }
        let r = reversed
            .value
            .enclose(-u, v, (Limit::FromLeft, Limit::FromRight), meter)
            .map_err(call(Law::ReverseKeepsTheCurve, u))?;
        if !r
            .first()
            .is_some_and(|x| overlap(x, &p, reversed.deviation))
        {
            return Err(broken(Law::ReverseKeepsTheCurve, u, None));
        }
        let image = entries
            .image_enclosed(p)
            .map_err(call(Law::TransformMovesTheCurve, u))?;
        let m = moved
            .value
            .enclose(u, v, sides, meter)
            .map_err(call(Law::TransformMovesTheCurve, u))?;
        if !m
            .first()
            .is_some_and(|x| overlap(x, &image, moved.deviation))
        {
            return Err(broken(Law::TransformMovesTheCurve, u, None));
        }
        if kind.enclose(u, v, sides, meter) != Ok(e) {
            return Err(broken(Law::Deterministic, u, None));
        }
    }
    if kind.bound(whole_box, meter) != Ok(whole) {
        return Err(broken(Law::Deterministic, a, None));
    }
    Ok(())
}

/// The NURBS form's point at the kind's (u, v), enclosed: at the point itself when both maps are
/// the identity, else the surface's bound over the box the maps' enclosures make.
fn nurbs_at(
    form: &SurfaceForm,
    u: f64,
    v: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<[Interval; 3], CurveError> {
    if (&form.u, &form.v) == (&ParamMap::Same, &ParamMap::Same) {
        let sides = (Limit::FromRight, Limit::FromRight);
        let [p, _, _] = ParamSurface::enclose(&form.surface, u, v, sides, meter)?;
        return Ok(p);
    }
    let ((a, b), (c, d)) = ParamSurface::domain(&form.surface);
    let cut = |x: Interval, (lo, hi): (f64, f64)| -> (f64, f64) {
        let l = tight::larger(x.lo(), lo);
        (l, tight::larger(tight::smaller(x.hi(), hi), l))
    };
    let (iu, iv) = (form.u.enclose(u)?, form.v.enclose(v)?);
    let bx = ParamSurface::bound(&form.surface, (cut(iu, (a, b)), cut(iv, (c, d))), meter)?;
    let mut out = [Interval::point(0.0)?; 3];
    for (o, (&l, &h)) in out.iter_mut().zip(bx.lo.iter().zip(&bx.hi)) {
        *o = Interval::new(l, h)?;
    }
    Ok(out)
}
