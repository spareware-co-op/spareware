//! The arc length of a spline, by fixed-node Gauss–Legendre quadrature with a proven bound on its
//! error (piece G220; chapter 15.3, "Size").
//!
//! What it does: [`spline_length`] integrates the speed |C′(u)| of a spline over each span between
//! distinct knots, cut into equal panels, with the eight-point Gauss–Legendre rule on each panel,
//! and returns the value and a bound on how far it can be from the true length ([`Length`]). The
//! panels are doubled, from one to a span, until the bound is under 2⁻⁴⁰ of the length (or 32
//! panels a span are in use, when the bound found is returned as it is). The constraint
//! [`crate::model::ConstraintKind::SplineLength`] uses the same nodes and weights, fixed at the
//! start of a solve, so a solved length is the one reported here.
//!
//! The bound has three parts, each added in interval arithmetic rounded outward:
//!
//! - **The rule's remainder.** For a panel of width h and the m-point rule, the error is at most
//!   h²ᵐ⁺¹ (m!)⁴ / ((2m + 1) ((2m)!)³) · max |s⁽²ᵐ⁾| (Davis and Rabinowitz), s the speed. The
//!   derivative of s comes from the Taylor series of the square root of |C′|², built from
//!   enclosures of the derivatives of C over the panel (`geom-curve`'s `derivative_bound`): the
//!   coefficient of order 2m of a series whose coefficients are intervals holding the true ones
//!   at every parameter of the panel holds s⁽²ᵐ⁾/(2m)! at every parameter. A panel where the
//!   speed may be zero has no such series, and the spline is [`SketchError::Degenerate`].
//! - **The sum.** Each term is the enclosure of the speed at its node (`geom-curve`'s `enclose`)
//!   times its weight, summed as intervals, so the roundoff of the evaluation is inside.
//! - **The rule's own roundoff.** Nodes and weights are doubles near the exact ones: the weight's
//!   relative error is below 2⁻⁵¹ and a node is within 2⁻⁴⁸·max(1, |a|, |b|) of the exact one, and
//!   the speed moves by at most max |C″| per unit of parameter.
//!
//! The bound is proved, and loose where the turn is tight: the derivative enclosures of the series
//! are boxes, so their dependency swells with the order, and a curve that turns sharply inside a
//! span can keep a bound near a thousandth of its length at 32 panels a span, while the value is
//! far closer (`tests/spline_more.rs`, a loop of a cubic).
//!
//! Guarantees:
//!
//! - **A proof, not an estimate.** The returned error is a bound: the length of the spline the
//!   stored numbers define is within it of the value.
//! - **Fixed work.** The panels and nodes depend on the stored spline alone, in a fixed order.
//! - **No panics, no recursion, no unsafe code.**
//!
//! Method: section 15.3, `projects/engines/geometry/architecture/15-sketches.md`; P. J. Davis and
//! P. Rabinowitz, *Methods of Numerical Integration*, 2nd ed., Academic Press (1984), section 2.7
//! (the Gauss remainder), on `geom_num`'s rules; the Taylor series of a
//! square root is the recurrence s₀ = √q₀, sₖ = (qₖ − Σ sⱼ sₖ₋ⱼ) / (2 s₀) (R. E. Moore, *Methods
//! and Applications of Interval Analysis*, SIAM (1979), <https://doi.org/10.1137/1.9781611970906>,
//! chapter 11, interval Taylor forms). The
//! interval series is written out here because no permissive crate gives a series over outward
//! rounded intervals that the kernel's arithmetic could stand behind (principle 2).
//!
//! Cost: per panel, 2m + 1 enclosures of derivative bounds and eight enclosures of the speed; at
//! most six rounds of doubling.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': a and b a panel's ends, h its width, m the rule's size, q and s the series"
)]

extern crate alloc;

use alloc::vec::Vec;

use geom_curve::{CurveError, DerivativeBound, ParamCurve};
use geom_num::interval::Interval;
use geom_num::quadrature_rules::GAUSS_LEGENDRE;
use geom_num::real::Real;
use geom_nurbs::{Curve, Limit};
use geom_run::{Budget, WorkMeter};

use crate::model::{ItemId, Sketch, SketchError};

/// The Gauss–Legendre points of every panel.
pub const NODES: usize = 8;

/// Doublings of the panel count tried: 1, 2, 4, 8, 16, 32 panels to a span.
const ROUNDS: u32 = 6;

/// The relative bound the panels are doubled to meet: 2⁻⁴⁰ of the length.
const RELATIVE: f64 = 9.094_947_017_729_282e-13;

/// The work one measure may charge: far above what 32 panels on any spline can use.
const WORK: u64 = 1 << 40;

/// The length of a spline and a proven bound on the error of the value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Length {
    /// The quadrature's value.
    pub value: f64,
    /// A bound: the true length is within `error` of `value`.
    pub error: f64,
}

/// A node of the rule and its weight, in the spline's parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Node {
    /// The parameter.
    pub u: f64,
    /// The weight, already times half the panel's width.
    pub weight: f64,
}

fn curve_error(id: ItemId, e: &CurveError) -> SketchError {
    match e {
        CurveError::Stopped(_) => SketchError::Stopped,
        _ => SketchError::WrongKind(id),
    }
}

fn interval_of(lo: f64, hi: f64) -> Result<Interval, SketchError> {
    Ok(Interval::new(lo, hi)?)
}

/// The distinct knots in the domain, as spans.
fn spans(curve: &Curve<2>) -> Vec<(f64, f64)> {
    let (lo, hi) = curve.basis().domain();
    let mut cuts: Vec<f64> = Vec::new();
    for k in curve.basis().knots() {
        if *k >= lo && *k <= hi && cuts.last() != Some(k) {
            cuts.push(*k);
        }
    }
    cuts.windows(2)
        .filter_map(|w| Some((*w.first()?, *w.get(1)?)))
        .collect()
}

/// `n` equal panels of `span`, the last ending exactly at its end.
fn panels(span: (f64, f64), n: u32) -> Vec<(f64, f64)> {
    let (a, b) = span;
    let step = (b - a) / f64::from(n);
    let mut out = Vec::new();
    let mut from = a;
    for i in 1..=n {
        let to = if i == n { b } else { a + step * f64::from(i) };
        out.push((from, to));
        from = to;
    }
    out
}

/// Interval of the factorial of `k`, exact for k at most 18.
fn factorial(k: usize) -> Result<Interval, SketchError> {
    let mut f = Interval::point(1.0)?;
    for i in 2..=k {
        let i = u32::try_from(i).map_err(|_| SketchError::Internal)?;
        f = f.mul(Interval::point(f64::from(i))?)?;
    }
    Ok(f)
}

/// The largest |value| an interval holds.
fn magnitude(x: Interval) -> f64 {
    larger(x.lo().abs(), x.hi().abs())
}

/// The larger of two finite doubles, in the kernel's total order (chapter 21).
pub(crate) fn larger(a: f64, b: f64) -> f64 {
    match (Real::new(a), Real::new(b)) {
        (Ok(a), Ok(b)) => a.total_max(b).get(),
        _ => f64::NAN,
    }
}

/// The top coefficient of the Taylor series of √(x² + y²), from series of x and y whose
/// coefficients hold the true ones at every parameter of a panel.
fn root_top(a: &[[Interval; 2]], id: ItemId) -> Result<Interval, SketchError> {
    let top = a.len().checked_sub(1).ok_or(SketchError::Internal)?;
    let mut q: Vec<Interval> = Vec::new();
    for k in 0..=top {
        let mut sum = Interval::point(0.0)?;
        for i in 0..=k {
            let j = k.checked_sub(i).ok_or(SketchError::Internal)?;
            let (p, r) = (
                a.get(i).ok_or(SketchError::Internal)?,
                a.get(j).ok_or(SketchError::Internal)?,
            );
            let [px, py] = *p;
            let [rx, ry] = *r;
            sum = sum.add(px.mul(rx)?)?.add(py.mul(ry)?)?;
        }
        q.push(sum);
    }
    let [x0, y0] = *a.first().ok_or(SketchError::Internal)?;
    let q0 = x0.square()?.add(y0.square()?)?;
    *q.first_mut().ok_or(SketchError::Internal)? = q0;
    if q0.lo() <= 0.0 {
        return Err(SketchError::Degenerate(id));
    }
    let s0 = q0.sqrt()?;
    let two_s0 = s0.add(s0)?;
    let mut s: Vec<Interval> = Vec::new();
    s.push(s0);
    for k in 1..=top {
        let mut rest = *q.get(k).ok_or(SketchError::Internal)?;
        for j in 1..k {
            let m = k.checked_sub(j).ok_or(SketchError::Internal)?;
            rest = rest.sub(
                s.get(j)
                    .ok_or(SketchError::Internal)?
                    .mul(*s.get(m).ok_or(SketchError::Internal)?)?,
            )?;
        }
        s.push(rest.div(two_s0)?);
    }
    s.pop().ok_or(SketchError::Internal)
}

/// Encloses the derivative of `order` of the curve over a panel, divided by (order − 1)!, as the
/// coefficient of order − 1 of the velocity's series.
fn coefficient(
    curve: &Curve<2>,
    panel: (f64, f64),
    order: usize,
    meter: &mut WorkMeter<'_>,
    id: ItemId,
) -> Result<[Interval; 2], SketchError> {
    let DerivativeBound::Bounded(b) = curve
        .derivative_bound(panel, order, meter)
        .map_err(|e| curve_error(id, &e))?
    else {
        return Err(SketchError::Degenerate(id));
    };
    let f = factorial(order.checked_sub(1).ok_or(SketchError::Internal)?)?;
    Ok([
        interval_of(b.lo[0], b.hi[0])?.div(f)?,
        interval_of(b.lo[1], b.hi[1])?.div(f)?,
    ])
}

/// The value and bound of one round, with its nodes.
fn round(
    curve: &Curve<2>,
    cuts: &[(f64, f64)],
    id: ItemId,
) -> Result<(Vec<Node>, Length), SketchError> {
    let rule = GAUSS_LEGENDRE.get(NODES - 1).ok_or(SketchError::Internal)?;
    if rule.nodes.len() != NODES {
        return Err(SketchError::Internal);
    }
    let mut meter = WorkMeter::new(Budget::of_work(WORK));
    let m = NODES;
    let twice = m.checked_mul(2).ok_or(SketchError::Internal)?;
    // (m!)⁴ / ((2m + 1) ((2m)!)³), then times (2m)! for the series' scale: (m!)⁴ / ((2m+1) ((2m)!)²).
    let m_fact = factorial(m)?;
    let two_m_fact = factorial(twice)?;
    let constant = m_fact.square()?.square()?.div(
        Interval::point(f64::from(
            u32::try_from(twice.checked_add(1).ok_or(SketchError::Internal)?)
                .map_err(|_| SketchError::Internal)?,
        ))?
        .mul(two_m_fact.square()?)?,
    )?;
    let mut total = Interval::point(0.0)?;
    let mut extra = Interval::point(0.0)?;
    let mut nodes: Vec<Node> = Vec::new();
    for &(a, b) in cuts {
        let h = b - a;
        let half = h * 0.5;
        let mid = a + half;
        let mut here = Interval::point(0.0)?;
        for (t, w) in rule.nodes.iter().zip(rule.weights) {
            let u = (mid + half * t).clamp(a, b);
            let weight = half * w;
            let enclosed = curve
                .enclose(u, Limit::FromRight, 1, &mut meter)
                .map_err(|e| curve_error(id, &e))?;
            let velocity = enclosed.get(1).ok_or(SketchError::Internal)?;
            let speed = Interval::norm(velocity)?;
            here = here.add(Interval::point(weight)?.mul(speed)?)?;
            nodes.push(Node { u, weight });
        }
        total = total.add(here)?;
        let mut series: Vec<[Interval; 2]> = Vec::new();
        for order in 1..=twice.checked_add(1).ok_or(SketchError::Internal)? {
            series.push(coefficient(curve, (a, b), order, &mut meter, id)?);
        }
        let top = root_top(&series, id)?;
        let width = Interval::point(b)?.sub(Interval::point(a)?)?;
        let mut power = width;
        for _ in 0..twice {
            power = power.mul(width)?;
        }
        let remainder = power.mul(constant)?.mul(Interval::point(magnitude(top))?)?;
        extra = extra.add(remainder)?;
        // The rule's own roundoff.
        let c2 = coefficient(curve, (a, b), 2, &mut meter, id)?;
        let accel = Interval::norm(&[
            Interval::point(magnitude(c2[0]))?,
            Interval::point(magnitude(c2[1]))?,
        ])?;
        let weight_roundoff = here.mul(Interval::point(f64::from_bits(0x3cc0_0000_0000_0000))?)?; // 2⁻⁵¹
        let reach = larger(larger(a.abs(), b.abs()), 1.0);
        let node_roundoff = width.mul(accel)?.mul(Interval::point(
            reach * f64::from_bits(0x3cf0_0000_0000_0000),
        )?)?; // 2⁻⁴⁸
        extra = extra.add(weight_roundoff)?.add(node_roundoff)?;
    }
    let value = total.lo() * 0.5 + total.hi() * 0.5;
    let spread = larger(
        Interval::point(total.hi())?
            .sub(Interval::point(value)?)?
            .hi(),
        Interval::point(value)?
            .sub(Interval::point(total.lo())?)?
            .hi(),
    );
    let error = extra.add(Interval::point(spread)?)?.hi();
    Ok((nodes, Length { value, error }))
}

/// The nodes and length of `spline`, with the panels doubled until the bound is small.
pub(crate) fn analyse(sketch: &Sketch, spline: ItemId) -> Result<(Vec<Node>, Length), SketchError> {
    let curve = sketch.spline_curve(spline)?;
    let base = spans(&curve);
    let mut count: u32 = 1;
    let mut last = None;
    for _ in 0..ROUNDS {
        let mut cuts = Vec::new();
        for span in &base {
            cuts.extend(panels(*span, count));
        }
        match round(&curve, &cuts, spline) {
            Ok((nodes, length)) => {
                let enough = length.error <= length.value * RELATIVE;
                last = Some((nodes, length));
                if enough {
                    break;
                }
            }
            // A box over a wide panel can hold a zero velocity the curve does not reach: narrower
            // panels are tried before the speed is called zero.
            Err(SketchError::Degenerate(_)) => last = None,
            Err(e) => return Err(e),
        }
        count = count.checked_mul(2).ok_or(SketchError::Internal)?;
    }
    last.ok_or(SketchError::Degenerate(spline))
}

/// The arc length of the spline `spline` at the sketch's values, with a proven bound on the error
/// of the value.
///
/// # Errors
///
/// [`SketchError::WrongKind`] for what is not a spline `geom-nurbs` accepts;
/// [`SketchError::Degenerate`] where the spline's speed may be zero, so no bound can be proved;
/// [`SketchError::Numeric`]; [`SketchError::Stopped`].
pub fn spline_length(sketch: &Sketch, spline: ItemId) -> Result<Length, SketchError> {
    analyse(sketch, spline).map(|(_, length)| length)
}
