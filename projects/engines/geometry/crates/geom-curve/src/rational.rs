//! The exact NURBS form of a conic: an ellipse (circles and arcs included) or a hyperbola as
//! rational quadratic pieces, with a certified bound on the rounding of its control points and
//! weights.
//!
//! What it does: [`conic`] cuts the domain into pieces of at most a quarter turn (an ellipse,
//! t in half-turns) or a parameter width of 1 (a hyperbola), and writes piece [t₀, t₁] as the
//! rational quadratic with control points C(t₀), the meeting point of the end tangents, C(t₁), and
//! weights 1, 1, λ, where, with h = (t₁ − t₀)/2 and σ = tan πh (ellipse) or tanh h (hyperbola),
//! λ = 1 + σ² (ellipse) or 1 − σ² (hyperbola) and the middle control point is
//! C(t₀) + σ·C′(t₀)/π (ellipse) or C(t₀) + σ·C′(t₀) (hyperbola). That is the conic exactly, on the
//! tangent half-angle parameter of [`ParamMap`]. Odd pieces run the other way (weights λ, 1, 1,
//! the middle point from the far end), and each piece's weights are scaled so neighbours share
//! their joint's weight: the weights then alternate and stay bounded. A whole quarter turn has
//! σ = 1 and λ = 2, so a circle on an axis-aligned frame with an exact center and radius converts
//! with weights 1, 1, 2, 1, 1, 1, 2, 1, 1 and every control point exact: deviation 0.
//!
//! Guarantees:
//!
//! - **Certified.** Every control point and weight is first enclosed exactly, then stored as a
//!   double. The stored piece differs from the exact one by at most
//!   max |Pᵢ − Qᵢ| + 2ε/(1 − ε)·max |Qᵢ − Q₀| (stored points P, exact points Q, weights within a
//!   relative ε): a rational Bezier point is Σ Rᵢ·Pᵢ with Rᵢ ≥ 0 summing to 1, and a relative
//!   change ε in the weights changes each Rᵢ by at most 2ε/(1 − ε) of itself.
//! - **Exact where the doubles are.** With the enclosures exact points, the deviation is 0.
//!
//! Method: the conic as a rational quadratic Bezier (L. Piegl and W. Tiller, *The NURBS Book*,
//! 2nd ed., sections 7.2 and 7.3, <https://doi.org/10.1007/978-3-642-59223-2>), with the
//! weights in the non-symmetric standard form (1, 1, λ) reached from the symmetric (1, cos α, 1)
//! by a rational reparametrisation (section 7.2's weight invariant w₀w₂/w₁² = λ = 1/cos² α), so a
//! quarter circle's weights are exact doubles.
//!
//! Cost: O(n) for n pieces (at most 5 for an ellipse, 1024 for a hyperbola), a few dozen interval
//! operations each.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the construction's: a and b the domain, n the pieces, h the half-width, g the tangent, k a piece"
)]

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_num::math;
use geom_nurbs::{Basis, Changed, Curve};
use geom_run::WorkMeter;

use crate::error::CurveError;
use crate::form::{Form, Shape, count};
use crate::map::{NurbsForm, ParamMap, tan_pi, tanh};
use crate::tight::{self, point};

/// The breaks of `domain` into `n` pieces: ends exact, interior points rounded, strictly
/// increasing.
pub(crate) fn breaks((a, b): (f64, f64), n: usize) -> Result<Vec<f64>, CurveError> {
    let nf = count(n)?;
    let mut out = Vec::with_capacity(n.saturating_add(1));
    out.push(a);
    for i in 1..n {
        let t = a + (b - a) * (count(i)? / nf);
        let last = out.last().copied().ok_or(CurveError::Defect)?;
        if !(last < t && t < b) {
            return Err(CurveError::Degenerate);
        }
        out.push(t);
    }
    out.push(b);
    Ok(out)
}

/// The number of pieces of `length` at most `width` each: at least 1.
pub(crate) fn pieces(length: f64, width: f64) -> Result<usize, CurveError> {
    let n = math::ceil(length / width)?.get();
    if n.is_nan() || n >= 4_294_967_296.0 {
        return Err(CurveError::NotFinite);
    }
    if n < 1.0 {
        return Ok(1);
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "n is a whole number in [1, 2^32)"
    )]
    let k = n as u64;
    usize::try_from(k).map_err(|_| CurveError::NotFinite)
}

/// The vector g with C′(t) = π·g (an ellipse) or C′(t) = g (a hyperbola), enclosed.
fn tangent<const D: usize>(form: &Form<D>, t: f64) -> Result<[Interval; D], CurveError> {
    let at = point(t)?;
    let (f1, f2) = match form.shape {
        Shape::Trig => (at.sinpi()?.neg(), at.cospi()?),
        Shape::Hyperbolic => {
            let (e, ei) = (at.exp()?, at.neg().exp()?);
            let half = point(0.5)?;
            (
                tight::mul(tight::sub(e, ei)?, half)?,
                tight::mul(tight::add(e, ei)?, half)?,
            )
        }
        _ => return Err(CurveError::Defect),
    };
    let [_, u, v, _] = &form.exact;
    let mut out = [point(0.0)?; D];
    for (o, (&ui, &vi)) in out.iter_mut().zip(u.iter().zip(v)) {
        *o = tight::add(tight::mul(f1, ui)?, tight::mul(f2, vi)?)?;
    }
    Ok(out)
}

/// Doubles at the middles of an enclosed vector.
fn middles<const D: usize>(v: &[Interval; D]) -> [f64; D] {
    v.map(tight::middle)
}

/// The exact NURBS form of the conic `form` over `domain` (module documentation).
///
/// # Errors
///
/// [`CurveError::Defect`] unless the shape is `Trig` or `Hyperbolic`; [`CurveError::Degenerate`]
/// if the domain is too narrow to cut; [`CurveError::NotFinite`]; [`CurveError::Stopped`].
#[allow(
    clippy::too_many_lines,
    reason = "one construction: breaks, then each piece's points, weights and deviation"
)]
pub(crate) fn conic<const D: usize>(
    form: &Form<D>,
    domain: (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Changed<NurbsForm<D>>, CurveError> {
    let hyperbolic = match form.shape {
        Shape::Trig => false,
        Shape::Hyperbolic => true,
        _ => return Err(CurveError::Defect),
    };
    let (a, b) = domain;
    let n = pieces(b - a, if hyperbolic { 1.0 } else { 0.5 })?;
    meter.charge(u64::try_from(n).map_err(|_| CurveError::Defect)?)?;
    let cuts = breaks(domain, n)?;
    let one = point(1.0)?;
    let half = point(0.5)?;
    // The exact points at the breaks, and their stored doubles.
    let mut at = Vec::with_capacity(cuts.len());
    for &t in &cuts {
        at.push(form.enclose(point(t)?, 0)?);
    }
    let mut points: Vec<[f64; D]> = Vec::with_capacity(n.saturating_mul(2).saturating_add(1));
    let mut weights: Vec<f64> = Vec::with_capacity(points.capacity());
    let mut knots: Vec<f64> = Vec::with_capacity(n.saturating_mul(2).saturating_add(4));
    knots.extend([0.0, 0.0, 0.0]);
    let mut joint = 1.0;
    let mut deviation = point(0.0)?;
    for k in 0..n {
        let (Some(&t0), Some(&t1), Some(q0), Some(q2)) = (
            cuts.get(k),
            cuts.get(k.saturating_add(1)),
            at.get(k),
            at.get(k.saturating_add(1)),
        ) else {
            return Err(CurveError::Defect);
        };
        let forward = k & 1 == 0;
        let h = tight::mul(tight::sub(point(t1)?, point(t0)?)?, half)?;
        let sigma = if hyperbolic { tanh(h)? } else { tan_pi(h)? };
        let s2 = tight::square(sigma)?;
        let lambda = if hyperbolic {
            tight::sub(one, s2)?
        } else {
            tight::add(one, s2)?
        };
        // The middle control point: from the start on an even piece, from the end on an odd one.
        let (from, base, sign) = if forward {
            (t0, q0, one)
        } else {
            (t1, q2, one.neg())
        };
        let g = tangent(form, from)?;
        let step = tight::mul(sign, sigma)?;
        let mut q1 = [point(0.0)?; D];
        for ((o, &bi), &gi) in q1.iter_mut().zip(base).zip(&g) {
            *o = tight::add(bi, tight::mul(step, gi)?)?;
        }
        // Weights scaled so the first is the joint's stored weight.
        let (first, last) = if forward {
            (one, lambda)
        } else {
            (lambda, one)
        };
        let scale = tight::div(point(joint)?, first)?;
        let w1 = scale;
        let w2 = tight::mul(scale, last)?;
        let (s1, s2w) = (tight::middle(w1), tight::middle(w2));
        if !(s1 > 0.0 && s2w > 0.0 && s1.is_finite() && s2w.is_finite()) {
            return Err(CurveError::NotFinite);
        }
        // The relative error of the stored weights.
        let mut eps = 0.0;
        for (s, w) in [(s1, w1), (s2w, w2)] {
            let r = tight::sub(tight::div(point(s)?, w)?, one)?;
            let m = tight::magnitude(r);
            if m > eps {
                eps = m;
            }
        }
        if eps.is_nan() || eps >= 0.5 {
            return Err(CurveError::NotFinite);
        }
        // Stored points: the joints computed once, the middle point's doubles.
        let p0 = middles(q0);
        let p1 = middles(&q1);
        let p2 = middles(q2);
        let mut moved = 0.0;
        for (p, q) in [(&p0, q0), (&p1, &q1), (&p2, q2)] {
            let d = tight::distance_to(p, q)?;
            if d > moved {
                moved = d;
            }
        }
        let mut piece = point(moved)?;
        if eps > 0.0 {
            let mut reach = 0.0;
            for q in [&q1, q2] {
                let d = tight::distance(q, q0)?;
                if d > reach {
                    reach = d;
                }
            }
            let e = point(eps)?;
            let factor = tight::div(tight::mul(point(2.0)?, e)?, tight::sub(one, e)?)?;
            piece = tight::add(piece, tight::mul(factor, point(reach)?)?)?;
        }
        if piece.hi() > deviation.hi() {
            deviation = point(piece.hi())?;
        }
        if k == 0 {
            points.push(p0);
            weights.push(joint);
        }
        points.push(p1);
        weights.push(s1);
        points.push(p2);
        weights.push(s2w);
        joint = s2w;
        let kf = count(k.saturating_add(1))?;
        if k.saturating_add(1) == n {
            knots.extend([kf, kf, kf]);
        } else {
            knots.extend([kf, kf]);
        }
    }
    let curve = Curve::new_rational(Basis::new(2, knots)?, &points, &weights)?;
    let map = if hyperbolic {
        ParamMap::Hyperbolic(cuts)
    } else {
        ParamMap::Circular(cuts)
    };
    Ok(Changed {
        value: NurbsForm { curve, map },
        deviation: deviation.hi(),
    })
}
