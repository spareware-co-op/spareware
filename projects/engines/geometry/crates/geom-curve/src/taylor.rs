//! NURBS forms on the same parameter by Taylor polynomials with a certified remainder: the
//! conversion of the kinds that have no exact rational form (helix, involute), and of the
//! polynomial kinds, for which it is exact up to rounding.
//!
//! What it does: [`same_parameter`] cuts a kind's domain into n equal pieces and, on each piece
//! [a, b] with middle m, takes the Taylor polynomial of degree p at m from the kind's certified
//! derivative enclosures, rewrites it in Bernstein form on [a, b] in interval arithmetic, and
//! replaces its two end coefficients by the kind's enclosed points at a and b, so neighbouring
//! pieces share their joint. The pieces become one NURBS curve of degree p with each interior
//! knot repeated p times (a C⁰ join at the exact curve points).
//!
//! Guarantees:
//!
//! - **Certified.** On each piece the distance from the curve to the stored NURBS is at most
//!   R + E + Z: R = M·ρ^{p+1}/(p+1)!, Taylor's remainder, with M the norm of the kind's bound on
//!   its (p + 1)-th derivative over the piece and ρ the farthest the piece reaches from m; E the
//!   change from replacing the end coefficients (a Bernstein end basis function is at most 1, and
//!   the two together at most 1); Z the rounding of the stored coefficients. A kind whose
//!   (p + 1)-th derivative bound is exactly zero is a polynomial of degree p at most, and then
//!   R and E are 0: the Taylor polynomial is the curve.
//! - **Same parameter.** The NURBS curve at t is within the deviation of the kind at t.
//!
//! Method: Taylor's theorem with the Lagrange remainder in its vector form (the integral form,
//! whose norm is bounded by the derivative's), the power-to-Bernstein change of basis
//! (R. T. Farouki, "The Bernstein polynomial basis: a centennial retrospective", Computer Aided
//! Geometric Design 29 (2012), section 3, <https://doi.org/10.1016/j.cagd.2012.03.001>), and
//! Bezier pieces as a NURBS curve with p-fold knots (L. Piegl and W. Tiller, *The NURBS Book*, 2nd
//! ed., section 2.3, <https://doi.org/10.1007/978-3-642-59223-2>), all in interval arithmetic.
//!
//! Cost: per piece, one enclosure of p derivatives, one derivative bound of order p + 1 and
//! O(p²·D) interval operations; n pieces.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': p the degree, m the middle, a and b a piece's ends, c the Taylor coefficients"
)]

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_nurbs::{Basis, Changed, Curve, Limit};
use geom_run::WorkMeter;

use crate::error::CurveError;
use crate::form::count;
use crate::map::{NurbsForm, ParamMap};
use crate::param::{DerivativeBound, ParamCurve};
use crate::rational::{breaks, pieces};
use crate::tight::{self, point};

/// The binomial coefficient C(n, k) as an exact double (n ≤ 28: at most C(28, 14) = 40 116 600).
fn binomial(n: usize, k: usize) -> Result<f64, CurveError> {
    let mut c: u64 = 1;
    for i in 0..k {
        let top = u64::try_from(n.checked_sub(i).ok_or(CurveError::Defect)?)
            .map_err(|_| CurveError::Defect)?;
        let bottom = u64::try_from(i.checked_add(1).ok_or(CurveError::Defect)?)
            .map_err(|_| CurveError::Defect)?;
        c = c
            .checked_mul(top)
            .and_then(|x| x.checked_div(bottom))
            .ok_or(CurveError::Defect)?;
    }
    let c = u32::try_from(c).map_err(|_| CurveError::Defect)?;
    Ok(f64::from(c))
}

/// k!, enclosed (exact while it is a double).
fn factorial(k: usize) -> Result<Interval, CurveError> {
    let mut f = point(1.0)?;
    for i in 2..=k {
        f = tight::mul(f, point(count(i)?)?)?;
    }
    Ok(f)
}

/// The kind's point at `t`, enclosed.
fn point_at<K: ParamCurve<D>, const D: usize>(
    kind: &K,
    t: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<[Interval; D], CurveError> {
    kind.enclose(t, Limit::FromRight, 0, meter)?
        .first()
        .copied()
        .ok_or(CurveError::Defect)
}

/// The Bernstein coefficients on [a, b] of the Taylor polynomial of degree p at m, with
/// coefficients c[k] (the k-th derivative over k!): t − m = α + β·v with α = a − m, β = b − a, so
/// the power coefficients in v are Σₖ c[k]·C(k, j)·β^j·α^{k−j}, and the Bernstein ones
/// Σ_{j≤i} C(i, j)/C(p, j) times those.
fn bernstein<const D: usize>(
    c: &[[Interval; D]],
    alpha: Interval,
    beta: Interval,
) -> Result<Vec<[Interval; D]>, CurveError> {
    let p = c.len().checked_sub(1).ok_or(CurveError::Defect)?;
    let zero = point(0.0)?;
    let mut alpha_pow = Vec::with_capacity(c.len());
    let mut beta_pow = Vec::with_capacity(c.len());
    let (mut x, mut y) = (point(1.0)?, point(1.0)?);
    for _ in 0..=p {
        alpha_pow.push(x);
        beta_pow.push(y);
        x = tight::mul(x, alpha)?;
        y = tight::mul(y, beta)?;
    }
    let mut power = Vec::with_capacity(c.len());
    for j in 0..=p {
        let mut s = [zero; D];
        for (k, ck) in c.iter().enumerate().skip(j) {
            let ap = *alpha_pow
                .get(k.checked_sub(j).ok_or(CurveError::Defect)?)
                .ok_or(CurveError::Defect)?;
            let bp = *beta_pow.get(j).ok_or(CurveError::Defect)?;
            let f = tight::mul(tight::mul(point(binomial(k, j)?)?, bp)?, ap)?;
            for (si, &cki) in s.iter_mut().zip(ck) {
                *si = tight::add(*si, tight::mul(f, cki)?)?;
            }
        }
        power.push(s);
    }
    let mut out = Vec::with_capacity(c.len());
    for i in 0..=p {
        let mut s = [zero; D];
        for (j, bj) in power.iter().enumerate().take(i.saturating_add(1)) {
            // Multiplied before divided, so a quotient that is a double comes out exact.
            let (num, den) = (point(binomial(i, j)?)?, point(binomial(p, j)?)?);
            for (si, &bji) in s.iter_mut().zip(bj) {
                *si = tight::add(*si, tight::div(tight::mul(num, bji)?, den)?)?;
            }
        }
        out.push(s);
    }
    Ok(out)
}

/// An upper bound on the norm of the (p + 1)-th derivative over `span`, and whether it is exactly
/// zero.
fn derivative_norm<K: ParamCurve<D>, const D: usize>(
    kind: &K,
    span: (f64, f64),
    order: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<(f64, bool), CurveError> {
    let DerivativeBound::Bounded(b) = kind.derivative_bound(span, order, meter)? else {
        return Err(CurveError::Defect);
    };
    let zero = b.lo.iter().chain(&b.hi).all(|&x| x == 0.0);
    let mut m = Vec::with_capacity(D);
    for (&lo, &hi) in b.lo.iter().zip(&b.hi) {
        m.push(point(tight::larger(lo.abs(), hi.abs()))?);
    }
    Ok((tight::norm(&m)?.hi(), zero))
}

/// The kind as NURBS of degree `degree` on the same parameter, from pieces at most `width` long
/// (module documentation).
///
/// # Errors
///
/// [`CurveError::Unbounded`] if the domain is not finite; [`CurveError::Degenerate`] if it is too
/// narrow to cut; the kind's own errors; [`CurveError::Stopped`].
pub(crate) fn same_parameter<K: ParamCurve<D>, const D: usize>(
    kind: &K,
    degree: usize,
    width: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Changed<NurbsForm<D>>, CurveError> {
    let (a, b) = kind.domain();
    if !(a.is_finite() && b.is_finite()) {
        return Err(CurveError::Unbounded);
    }
    let n = pieces(b - a, width)?;
    let cuts = breaks((a, b), n)?;
    from_cuts(kind, degree, &cuts, meter)
}

/// The cuts of `domain`: each interval between consecutive `breaks` (sorted, inside the domain)
/// cut into `per` equal pieces, the breaks kept exactly.
pub(crate) fn cuts_between(
    (a, b): (f64, f64),
    inner: &[f64],
    per: usize,
) -> Result<Vec<f64>, CurveError> {
    let mut ends = Vec::with_capacity(inner.len().saturating_add(2));
    ends.push(a);
    for &x in inner {
        if a < x && x < b && ends.last().is_some_and(|&l| l < x) {
            ends.push(x);
        }
    }
    ends.push(b);
    let mut out: Vec<f64> = Vec::new();
    for w in ends.windows(2) {
        let (Some(&x), Some(&y)) = (w.first(), w.get(1)) else {
            return Err(CurveError::Defect);
        };
        let piece = breaks((x, y), per).or_else(|_| breaks((x, y), 1))?;
        if out.is_empty() {
            out.extend(piece);
        } else {
            out.extend(piece.into_iter().skip(1));
        }
    }
    Ok(out)
}

/// The kind as NURBS of degree `degree` on the same parameter, cut at `cuts` (its ends included,
/// strictly increasing), each piece a Taylor polynomial with its certified remainder.
///
/// # Errors
///
/// As [`same_parameter`].
#[allow(
    clippy::too_many_lines,
    reason = "one construction: each piece's coefficients, remainder and rounding"
)]
pub(crate) fn from_cuts<K: ParamCurve<D>, const D: usize>(
    kind: &K,
    degree: usize,
    cuts: &[f64],
    meter: &mut WorkMeter<'_>,
) -> Result<Changed<NurbsForm<D>>, CurveError> {
    let n = cuts.len().checked_sub(1).ok_or(CurveError::Defect)?;
    let p = degree;
    let count_points = n.checked_mul(p).and_then(|x| x.checked_add(1));
    let Some(count_points) = count_points else {
        return Err(CurveError::NotFinite);
    };
    meter.charge(u64::try_from(count_points).map_err(|_| CurveError::Defect)?)?;
    let mut ends = Vec::with_capacity(cuts.len());
    for &t in cuts {
        ends.push(point_at(kind, t, meter)?);
    }
    let mut points: Vec<[f64; D]> = Vec::with_capacity(count_points);
    let mut knots = Vec::with_capacity(count_points.saturating_add(p).saturating_add(1));
    let mut deviation = point(0.0)?;
    let order = p.checked_add(1).ok_or(CurveError::Defect)?;
    let reach_factorial = factorial(order)?;
    for k in 0..n {
        let (Some(&t0), Some(&t1), Some(e0), Some(e1)) = (
            cuts.get(k),
            cuts.get(k.saturating_add(1)),
            ends.get(k),
            ends.get(k.saturating_add(1)),
        ) else {
            return Err(CurveError::Defect);
        };
        let m = f64::midpoint(t0, t1);
        let derivs = kind.enclose(m, Limit::FromRight, p, meter)?;
        let mut c = Vec::with_capacity(derivs.len());
        for (j, d) in derivs.iter().enumerate() {
            let f = factorial(j)?;
            let mut ck = *d;
            for x in &mut ck {
                *x = tight::div(*x, f)?;
            }
            c.push(ck);
        }
        let alpha = tight::sub(point(t0)?, point(m)?)?;
        let beta = tight::sub(point(t1)?, point(t0)?)?;
        let coeffs = bernstein(&c, alpha, beta)?;
        let (bound, exact) = derivative_norm(kind, (t0, t1), order, meter)?;
        let mut piece = point(0.0)?;
        if !exact {
            let rho = {
                let far = tight::add(alpha, beta)?;
                let (x, y) = (tight::magnitude(alpha), tight::magnitude(far));
                point(tight::larger(x, y))?
            };
            let mut r = point(bound)?;
            for _ in 0..order {
                r = tight::mul(r, rho)?;
            }
            r = tight::div(r, reach_factorial)?;
            let (Some(first), Some(last)) = (coeffs.first(), coeffs.last()) else {
                return Err(CurveError::Defect);
            };
            let d0 = tight::distance(first, e0)?;
            let d1 = tight::distance(last, e1)?;
            piece = tight::add(r, point(tight::larger(d0, d1))?)?;
        }
        // The stored coefficients: the shared ends, the middles of the interior ones.
        let s0 = e0.map(tight::middle);
        let s1 = e1.map(tight::middle);
        let mut rounding = tight::distance_to(&s0, e0)?;
        rounding = tight::larger(rounding, tight::distance_to(&s1, e1)?);
        if k == 0 {
            points.push(s0);
            knots.extend(core::iter::repeat_n(t0, order));
        }
        for q in coeffs
            .iter()
            .skip(1)
            .take(p.checked_sub(1).ok_or(CurveError::Defect)?)
        {
            let s = q.map(tight::middle);
            rounding = tight::larger(rounding, tight::distance_to(&s, q)?);
            points.push(s);
        }
        points.push(s1);
        knots.extend(core::iter::repeat_n(
            t1,
            if k.saturating_add(1) == n { order } else { p },
        ));
        piece = tight::add(piece, point(rounding)?)?;
        deviation = point(tight::larger(deviation.hi(), piece.hi()))?;
    }
    let curve = Curve::new(Basis::new(p, knots)?, &points)?;
    Ok(Changed {
        value: NurbsForm {
            curve,
            map: ParamMap::Same,
        },
        deviation: deviation.hi(),
    })
}
