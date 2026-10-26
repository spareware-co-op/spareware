//! Certified evaluation: interval enclosures of the exact point and derivatives of a nonrational
//! curve, and the exact point of a nonrational surface, at a parameter; and from them certified
//! distances to data, the errors every fit reports.
//!
//! What it does: [`curve_derivative`] encloses C^(k)(u) and [`surface_point`] S(u, v), for the
//! exact curve or surface that the stored doubles define, evaluated at the exact parameter given.
//! [`distance_bound`] turns an enclosure and a data point into an upper bound on their distance.
//! [`curve_errors`] and [`surface_error`] apply them to every data point of a fit.
//!
//! Guarantees:
//!
//! - **Enclosure.** Every value is computed in `geom-num`'s interval arithmetic, each operation
//!   rounded outward (`crate::arith`), from the exact knots, parameter and control points, so it
//!   holds the exact value; the distance bound is the upper end of an enclosure of the Euclidean
//!   norm, so it is at least the exact distance.
//! - **The book's schemes.** The basis functions come from the triangular scheme of de Boor and
//!   Cox (A2.2), a derivative from the differenced control points of the derivative curve (A3.3,
//!   equation 3.8) and that curve's basis, of degree p − k on the same knots; the same operations
//!   as the double-precision code, in intervals.
//! - **Typed failures.** A rational curve or surface is [`NurbsError::Rational`] (fits make only
//!   nonrational ones); a parameter outside the domain is [`NurbsError::ParameterOutside`]; an
//!   enclosure that leaves the finite doubles is [`NurbsError::NotFinite`].
//!
//! Method: interval arithmetic (R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to
//! Interval Analysis*, SIAM (2009), <https://doi.org/10.1137/1.9780898717716>) on Piegl and
//! Tiller's algorithms A2.2 and A3.3 (The NURBS Book, 2nd ed., sections 2.2 and 3.3,
//! <https://doi.org/10.1007/978-3-642-59223-2>).
//!
//! Cost: O(p² + k·p·D) interval operations for a curve's derivative of order k, O(p² + q² +
//! p·q·D) for a surface point; nothing allocates but the lists of errors, charged first.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the book's: u and v for parameters, p and q for degrees, n for basis values, w for control points"
)]

use alloc::vec::Vec;

use geom_num::interval::Interval;
use geom_run::WorkMeter;

use crate::NurbsError;
use crate::arith::Num;
use crate::basis::{Basis, Limit, WIDTH};
use crate::budget::vec_for;
use crate::curve::Curve;
use crate::index::{add, get, mul, real, set, sub};
use crate::surface::Surface;

/// Enclosures of `N_(span−d+i)` of degree d at `u`, into `n[i]` for i from 0 to d: A2.2 in
/// intervals, on `knots`.
fn funs(
    knots: &[f64],
    span: usize,
    u: f64,
    degree: usize,
    n: &mut [Interval; WIDTH],
) -> Result<(), NurbsError> {
    let zero = Interval::of(0.0)?;
    let mut left = [zero; WIDTH];
    let mut right = [zero; WIDTH];
    let x = Interval::of(u)?;
    set(n, 0, Interval::of(1.0)?)?;
    for k in 1..=degree {
        set(
            &mut left,
            k,
            x.minus(Interval::of(get(knots, sub(add(span, 1)?, k)?)?)?)?,
        )?;
        set(
            &mut right,
            k,
            Interval::of(get(knots, add(span, k)?)?)?.minus(x)?,
        )?;
        let mut saved = zero;
        for l in 0..k {
            let r = get(&right, add(l, 1)?)?;
            let le = get(&left, sub(k, l)?)?;
            let delta = r.plus(le)?;
            let temp = get(n, l)?;
            set(n, l, saved.plus(temp.times(r.over(delta)?)?)?)?;
            saved = temp.times(le.over(delta)?)?;
        }
        set(n, k, saved)?;
    }
    Ok(())
}

/// The span of `u` in `basis` (from the right) and upper bounds on the p + 1 basis functions
/// that do not vanish there, `N_(span−p+i)` at index i; the rest of the array is 0.
pub(crate) fn basis_upper(basis: &Basis, u: f64) -> Result<(usize, [f64; WIDTH]), NurbsError> {
    let span = basis.span(u, Limit::FromRight)?;
    let mut n = [Interval::of(0.0)?; WIDTH];
    funs(basis.knots(), span, u, basis.degree(), &mut n)?;
    let mut out = [0.0; WIDTH];
    for (o, x) in out.iter_mut().zip(&n) {
        *o = x.hi();
    }
    Ok((span, out))
}

/// An enclosure of the k-th derivative of the nonrational `curve` at `u` (the point for k = 0),
/// from the right at a knot.
pub(crate) fn curve_derivative<const D: usize>(
    curve: &Curve<D>,
    u: f64,
    order: usize,
) -> Result<[Interval; D], NurbsError> {
    if curve.is_rational() {
        return Err(NurbsError::Rational);
    }
    let basis = curve.basis();
    let knots = basis.knots();
    let p = basis.degree();
    let span = basis.span(u, Limit::FromRight)?;
    let zero = Interval::of(0.0)?;
    if order > p {
        return Ok([zero; D]);
    }
    let first = sub(span, p)?;
    let pts = curve.homogeneous();
    let mut w = [[zero; D]; WIDTH];
    for (t, slot) in w.iter_mut().enumerate().take(add(p, 1)?) {
        let c = get(pts, add(first, t)?)?;
        for (s, &x) in slot.iter_mut().zip(&c) {
            *s = Interval::of(x)?;
        }
    }
    for d in 1..=order {
        let factor = Interval::of(real(sub(add(p, 1)?, d)?)?)?;
        for t in 0..=sub(p, d)? {
            let i = add(first, t)?;
            let denom = Interval::of(get(knots, add(add(i, p)?, 1)?)?)?
                .minus(Interval::of(get(knots, add(i, d)?)?)?)?;
            let next = get(&w, add(t, 1)?)?;
            let here = get(&w, t)?;
            let mut out = [zero; D];
            for (o, (a, b)) in out.iter_mut().zip(next.iter().zip(&here)) {
                *o = factor.times(a.minus(*b)?)?.over(denom)?;
            }
            set(&mut w, t, out)?;
        }
    }
    let degree = sub(p, order)?;
    let mut n = [zero; WIDTH];
    funs(knots, span, u, degree, &mut n)?;
    let mut sum = [zero; D];
    for t in 0..=degree {
        let nt = get(&n, t)?;
        let wt = get(&w, t)?;
        for (s, x) in sum.iter_mut().zip(&wt) {
            *s = s.plus(nt.times(*x)?)?;
        }
    }
    Ok(sum)
}

/// An enclosure of the nonrational `surface` at (u, v), from the right at knots.
pub(crate) fn surface_point<const D: usize>(
    surface: &Surface<D>,
    u: f64,
    v: f64,
) -> Result<[Interval; D], NurbsError> {
    if surface.is_rational() {
        return Err(NurbsError::Rational);
    }
    let (bu, bv) = (surface.basis_u(), surface.basis_v());
    let (p, q) = (bu.degree(), bv.degree());
    let su = bu.span(u, Limit::FromRight)?;
    let sv = bv.span(v, Limit::FromRight)?;
    let zero = Interval::of(0.0)?;
    let mut nu = [zero; WIDTH];
    let mut nv = [zero; WIDTH];
    funs(bu.knots(), su, u, p, &mut nu)?;
    funs(bv.knots(), sv, v, q, &mut nv)?;
    let net = surface.homogeneous();
    let cols = bv.count();
    let (fu, fv) = (sub(su, p)?, sub(sv, q)?);
    let mut sum = [zero; D];
    for i in 0..=p {
        let mut row = [zero; D];
        for j in 0..=q {
            let c = get(net, add(mul(add(fu, i)?, cols)?, add(fv, j)?)?)?;
            let nj = get(&nv, j)?;
            for (r, &x) in row.iter_mut().zip(&c) {
                *r = r.plus(nj.times(Interval::of(x)?)?)?;
            }
        }
        let ni = get(&nu, i)?;
        for (s, r) in sum.iter_mut().zip(&row) {
            *s = s.plus(ni.times(*r)?)?;
        }
    }
    Ok(sum)
}

/// An upper bound on |q − x| for every x in the enclosure `c`.
pub(crate) fn distance_bound<const D: usize>(
    q: [f64; D],
    c: [Interval; D],
) -> Result<f64, NurbsError> {
    let zero = Interval::of(0.0)?;
    let mut diff = [zero; D];
    for (d, (&a, b)) in diff.iter_mut().zip(q.iter().zip(&c)) {
        *d = Interval::of(a)?.minus(*b)?;
    }
    let bound = Interval::norm(&diff)
        .map_err(|_| NurbsError::NotFinite)?
        .hi();
    if bound.is_finite() {
        Ok(bound)
    } else {
        Err(NurbsError::NotFinite)
    }
}

/// Charges the meter for `count` certified evaluations of degrees up to (p, q) in dimension D.
fn charge_evals(
    meter: &mut WorkMeter<'_>,
    count: usize,
    (p, q): (usize, usize),
    dim: usize,
) -> Result<(), NurbsError> {
    let wide = |k: usize| u64::try_from(k).map_err(|_| NurbsError::Defect);
    let each = add(
        add(mul(p, p)?, mul(q, q)?)?,
        mul(mul(add(p, 1)?, add(q, 1)?)?, dim)?,
    )?;
    meter.charge(wide(count)?.saturating_mul(wide(each)?))?;
    Ok(())
}

/// For each data point `q_k`, a certified upper bound on |`q_k` − C(`u_k`)|.
pub(crate) fn curve_errors<const D: usize>(
    meter: &mut WorkMeter<'_>,
    curve: &Curve<D>,
    q: &[[f64; D]],
    u: &[f64],
) -> Result<Vec<f64>, NurbsError> {
    if q.len() != u.len() {
        return Err(NurbsError::Defect);
    }
    charge_evals(meter, q.len(), (curve.degree(), 0), D)?;
    let mut out = vec_for(meter, q.len())?;
    for (&qk, &uk) in q.iter().zip(u) {
        out.push(distance_bound(qk, curve_derivative(curve, uk, 0)?)?);
    }
    Ok(out)
}

/// The largest of `errors`, 0 for none.
pub(crate) fn largest(errors: &[f64]) -> f64 {
    errors.iter().fold(0.0, |m, &e| if e > m { e } else { m })
}

/// A certified upper bound on |`Q_kl` − S(`u_k`, `v_l`)| over a grid of `u.len()` rows of
/// `v.len()` points, row by row.
pub(crate) fn surface_error<const D: usize>(
    meter: &mut WorkMeter<'_>,
    surface: &Surface<D>,
    q: &[[f64; D]],
    (u, v): (&[f64], &[f64]),
) -> Result<f64, NurbsError> {
    if q.len() != mul(u.len(), v.len())? {
        return Err(NurbsError::Defect);
    }
    let degrees = (surface.basis_u().degree(), surface.basis_v().degree());
    charge_evals(meter, q.len(), degrees, D)?;
    let mut worst = 0.0;
    for (k, &uk) in u.iter().enumerate() {
        for (l, &vl) in v.iter().enumerate() {
            let qkl = get(q, add(mul(k, v.len())?, l)?)?;
            let e = distance_bound(qkl, surface_point(surface, uk, vl)?)?;
            if e > worst {
                worst = e;
            }
        }
    }
    Ok(worst)
}
