//! The kind contract: the laws every curve kind must keep, as a check any kind's tests run
//! (`projects/engines/geometry/AGENTS.md`, "How to add a geometry kind").
//!
//! What it does: [`check`] runs a kind through every law of [`Law`] at the parameters given and
//! returns the first law broken, with where. The laws compare certified objects with each other:
//! two enclosures of one exact value must overlap, so a bound, an enclosure, a conversion, a split,
//! a reversal or a transform that is wrong by more than its own stated error is caught wherever a
//! sample lands on it. Evaluation in doubles is compared with the certified enclosure within a
//! relative 2^−30, which catches a wrong formula, not rounding.
//!
//! Guarantees:
//!
//! - **Sound.** A law reported broken is broken: two certified enclosures of one exact value that
//!   do not overlap (after widening by the stated deviations) prove that one of them is false, and
//!   a refusal the law requires that did not come is a broken refusal.
//! - **Deterministic.** The checks run in the order of the parameters given, each law in the order
//!   of [`Law`]; the same kind and parameters give the same verdict.
//!
//! Method: property checks of the trait's contract (chapter 28.2's contract laws, applied to the
//! behaviour chapter 7 lists): consistency of certified enclosures, as interval methods check
//! inclusion (R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to Interval Analysis*,
//! SIAM (2009), <https://doi.org/10.1137/1.9780898717716>, the inclusion property of chapter 5).
//!
//! Cost: for each parameter, a few enclosures and bounds of the kind, a split, a reversal and a
//! transform: the kind's own costs, a few dozen times.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the contract's: a and b for the domain, t for a parameter, e, p, h, r, m and n for enclosures"
)]

use geom_math::TypedTransform;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Limit};
use geom_run::WorkMeter;

use crate::affine::Entries;
use crate::error::{ContractError, CurveError, Law};
use crate::map::{NurbsForm, ParamMap};
use crate::param::{DerivativeBound, ParamCurve};

/// The relative slack between a double evaluation and the certified enclosure: 2^−30.
const SLACK: f64 = f64::from_bits(0x3E10_0000_0000_0000);

/// A broken law.
fn broken(law: Law, at: f64, error: Option<CurveError>) -> ContractError {
    ContractError { law, at, error }
}

/// Whether two enclosures of one value can hold it, each side widened by `widen`.
fn overlap<const D: usize>(a: &[Interval; D], b: &[Interval; D], widen: f64) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| x.lo() - widen <= y.hi() + widen && y.lo() - widen <= x.hi() + widen)
}

/// Whether an enclosure meets a box.
fn meets<const D: usize>(a: &[Interval; D], b: &Bounds<D>) -> bool {
    a.iter()
        .zip(b.lo.iter().zip(&b.hi))
        .all(|(x, (&lo, &hi))| x.lo() <= hi && lo <= x.hi())
}

/// Whether doubles lie in an enclosure within the relative slack.
fn near<const D: usize>(v: &[f64; D], a: &[Interval; D]) -> bool {
    v.iter().zip(a).all(|(&x, i)| {
        let scale = 1.0 + x.abs() + i.lo().abs() + i.hi().abs();
        i.lo() - SLACK * scale <= x && x <= i.hi() + SLACK * scale
    })
}

/// Runs `kind` through every law of the contract at the parameters `params` (each inside its
/// domain), with `transform` for the transform law (one acting in the plane for a 2D kind). The
/// NURBS law compares the kind at t with its NURBS form at the parameter its map gives; a kind
/// whose domain is not finite must refuse the conversion with [`CurveError::Unbounded`].
///
/// # Errors
///
/// The first [`ContractError`] found: the law, the parameter, and the error of a call the law
/// needed, if one failed.
#[allow(
    clippy::too_many_lines,
    reason = "one law after another, in the order of Law, each a few lines"
)]
pub fn check<K: ParamCurve<D>, const D: usize>(
    kind: &K,
    params: &[f64],
    transform: &TypedTransform,
    meter: &mut WorkMeter<'_>,
) -> Result<(), ContractError> {
    let (a, b) = kind.domain();
    let call = |law: Law, at: f64| move |e: CurveError| broken(law, at, Some(e));
    // A ray's or infinite line's domain has an infinite end; the laws use finite stand-ins 1024
    // beyond the parameters given (or the finite end).
    let (first, last) = params.iter().fold((0.0_f64, 0.0_f64), |(lo, hi), &t| {
        (if t < lo { t } else { lo }, if t > hi { t } else { hi })
    });
    let lo = if a.is_finite() { a } else { first - 1024.0 };
    let hi = if b.is_finite() { b } else { last + 1024.0 };
    // Refusals.
    let outside = if b.is_finite() {
        b + (b - lo) + 1.0
    } else if a.is_finite() {
        a - (hi - a) - 1.0
    } else {
        f64::NAN
    };
    let beyond = if a.is_finite() {
        (a - 1.0, hi)
    } else if b.is_finite() {
        (lo, b + 1.0)
    } else {
        (lo, f64::INFINITY)
    };
    if kind.point(outside, Limit::FromRight) != Err(CurveError::ParameterOutside)
        || kind.bound(beyond, meter) != Err(CurveError::SpanOutside)
        || kind.split(a, meter).err() != Some(CurveError::ParameterOutside)
    {
        return Err(broken(Law::RefusesOutside, outside, None));
    }
    let whole = kind
        .bound((lo, hi), meter)
        .map_err(call(Law::BoundHoldsPoints, lo))?;
    let entries = Entries::of(transform).map_err(call(Law::TransformMovesTheCurve, lo))?;
    let mid = lo + (hi - lo) * 0.5;
    let (left, right) = kind
        .split(mid, meter)
        .map_err(call(Law::SplitKeepsTheCurve, mid))?;
    let reversed = kind
        .reversed(meter)
        .map_err(call(Law::ReverseKeepsTheCurve, a))?;
    let moved = kind
        .transformed(transform, meter)
        .map_err(call(Law::TransformMovesTheCurve, a))?;
    // A ray or infinite line has no NURBS form, and must say so.
    let nurbs = match kind.to_nurbs(meter) {
        Ok(n) => Some(n),
        Err(CurveError::Unbounded) if !(a.is_finite() && b.is_finite()) => None,
        Err(e) => return Err(broken(Law::NurbsWithinItsError, lo, Some(e))),
    };
    for &t in params {
        let e = kind
            .enclose(t, Limit::FromRight, 2, meter)
            .map_err(call(Law::EvaluationIsEnclosed, t))?;
        // Evaluation in doubles against the certified enclosure.
        let mut d = [[0.0; D]; 3];
        kind.derivatives(t, Limit::FromRight, &mut d)
            .map_err(call(Law::EvaluationIsEnclosed, t))?;
        for (v, enc) in d.iter().zip(&e) {
            if !near(v, enc) {
                return Err(broken(Law::EvaluationIsEnclosed, t, None));
            }
        }
        let p = e
            .first()
            .copied()
            .ok_or(broken(Law::EvaluationIsEnclosed, t, None))?;
        // Bounds over the whole domain and over a span around t.
        let span = (f64::midpoint(lo, t), f64::midpoint(t, hi));
        let local = kind
            .bound(span, meter)
            .map_err(call(Law::BoundHoldsPoints, t))?;
        if !meets(&p, &whole) || !meets(&p, &local) {
            return Err(broken(Law::BoundHoldsPoints, t, None));
        }
        for (k, dk) in e.iter().enumerate().skip(1) {
            match kind
                .derivative_bound(span, k, meter)
                .map_err(call(Law::DerivativeBoundHoldsDerivatives, t))?
            {
                DerivativeBound::Bounded(bx) if !meets(dk, &bx) => {
                    return Err(broken(Law::DerivativeBoundHoldsDerivatives, t, None));
                }
                _ => {}
            }
        }
        // The NURBS conversion, at the point its parameter map gives.
        if let Some(nurbs) = &nurbs {
            let n = nurbs_at(&nurbs.value, t, meter).map_err(call(Law::NurbsWithinItsError, t))?;
            if !overlap(&n, &p, nurbs.deviation) {
                return Err(broken(Law::NurbsWithinItsError, t, None));
            }
        }
        // The split's halves, on their sides of the middle.
        let half = if t <= mid { &left } else { &right };
        let side = if t <= mid {
            Limit::FromLeft
        } else {
            Limit::FromRight
        };
        let h = half
            .value
            .enclose(t, side, 0, meter)
            .map_err(call(Law::SplitKeepsTheCurve, t))?;
        let pe = kind
            .enclose(t, side, 0, meter)
            .map_err(call(Law::SplitKeepsTheCurve, t))?;
        if !h
            .first()
            .zip(pe.first())
            .is_some_and(|(x, y)| overlap(x, y, half.deviation))
        {
            return Err(broken(Law::SplitKeepsTheCurve, t, None));
        }
        // The reversed curve at −t, approached from the other side.
        let r = reversed
            .value
            .enclose(-t, Limit::FromLeft, 0, meter)
            .map_err(call(Law::ReverseKeepsTheCurve, t))?;
        if !r
            .first()
            .is_some_and(|x| overlap(x, &p, reversed.deviation))
        {
            return Err(broken(Law::ReverseKeepsTheCurve, t, None));
        }
        // The transformed curve against the image of the enclosure.
        let image = image::<D>(&entries, &p).map_err(call(Law::TransformMovesTheCurve, t))?;
        let m = moved
            .value
            .enclose(t, Limit::FromRight, 0, meter)
            .map_err(call(Law::TransformMovesTheCurve, t))?;
        if !m
            .first()
            .is_some_and(|x| overlap(x, &image, moved.deviation))
        {
            return Err(broken(Law::TransformMovesTheCurve, t, None));
        }
        // Determinism: the same call again, bit for bit.
        let again = kind
            .enclose(t, Limit::FromRight, 2, meter)
            .map_err(call(Law::Deterministic, t))?;
        if again != e {
            return Err(broken(Law::Deterministic, t, None));
        }
    }
    if kind.bound((lo, hi), meter) != Ok(whole) {
        return Err(broken(Law::Deterministic, lo, None));
    }
    Ok(())
}

/// The NURBS form's point at the kind's parameter `t`, enclosed: at u itself when the map is the
/// identity, else the curve's bound over the enclosure of u.
fn nurbs_at<const D: usize>(
    form: &NurbsForm<D>,
    t: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<[Interval; D], CurveError> {
    if form.map == ParamMap::Same {
        return ParamCurve::enclose(&form.curve, t, Limit::FromRight, 0, meter)?
            .first()
            .copied()
            .ok_or(CurveError::Defect);
    }
    let u = form.map.enclose(t)?;
    let (c0, c1) = ParamCurve::domain(&form.curve);
    let lo = if u.lo() < c0 { c0 } else { u.lo() };
    let hi = if u.hi() > c1 { c1 } else { u.hi() };
    let b = ParamCurve::bound(&form.curve, (lo, if hi < lo { lo } else { hi }), meter)?;
    let mut out = [Interval::point(0.0)?; D];
    for (o, (&l, &h)) in out.iter_mut().zip(b.lo.iter().zip(&b.hi)) {
        *o = Interval::new(l, h)?;
    }
    Ok(out)
}

/// The image of an enclosed point under the transform's exact entries, in intervals.
fn image<const D: usize>(e: &Entries, p: &[Interval; D]) -> Result<[Interval; D], CurveError> {
    let zero = Interval::point(0.0)?;
    let mut x = [zero; 3];
    for (slot, v) in x.iter_mut().zip(p) {
        *slot = *v;
    }
    let img = e.image_enclosed(x)?;
    let mut out = [zero; D];
    for (o, v) in out.iter_mut().zip(&img) {
        *o = *v;
    }
    Ok(out)
}
