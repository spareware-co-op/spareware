//! Interval operations that stay exact when they can: the analytic kinds' arithmetic.
//!
//! What it does: each function is the `geom-num` interval operation of the same name, except that
//! when both operands are single doubles and the exact result is a double too, it returns that
//! double as a point interval instead of widening it. So a circle of integer radius on a grid
//! center, evaluated at a quarter turn, is enclosed by its exact point, and two constructions of
//! one exact value compare equal: a conversion or transform that is exact reports a deviation of
//! 0, which widened intervals never could.
//!
//! Guarantees:
//!
//! - **Certified.** A point result is the exact value, decided by an error-free transformation
//!   (`geom-num`'s `two_sum` and `two_prod`, whose error is finite and zero exactly when the
//!   operation was exact); every other result is the widened interval of `geom-num`, which encloses
//!   the exact range.
//! - **Deterministic.** The same operands give the same bits on every target.
//!
//! Method: error-free transformations (T. J. Dekker, "A floating-point technique for extending
//! the available precision", Numerische Mathematik 18 (1971),
//! <https://doi.org/10.1007/BF01397083>; J. R. Shewchuk, "Adaptive precision floating-point
//! arithmetic and fast robust geometric predicates", Discrete & Computational Geometry 18 (1997),
//! <https://doi.org/10.1007/PL00009321>) as exactness tests, with `geom-num`'s interval arithmetic
//! otherwise.
//!
//! Cost: a few floating-point operations per call more than the interval operation.

use geom_num::eft::{two_prod, two_sum};
use geom_num::interval::Interval;
use geom_num::math;

use crate::error::CurveError;

/// The single double an interval holds, if it is a point.
fn single(a: Interval) -> Option<f64> {
    (a.lo() == a.hi()).then_some(a.lo())
}

/// The point interval at `x`.
///
/// # Errors
///
/// [`CurveError::NotFinite`] if a bound overflows (or, for a quotient or root, if the divisor holds zero or the operand holds a negative number).
pub fn point(x: f64) -> Result<Interval, CurveError> {
    Ok(Interval::point(x)?)
}

/// The exact product `x · y` as a double, if it is one.
fn exact_product(x: f64, y: f64) -> Option<f64> {
    let (p, e) = two_prod(x, y);
    (p.is_finite() && e == 0.0).then_some(p)
}

/// `a + b`.
///
/// # Errors
///
/// [`CurveError::NotFinite`] if a bound overflows (or, for a quotient or root, if the divisor holds zero or the operand holds a negative number).
pub fn add(a: Interval, b: Interval) -> Result<Interval, CurveError> {
    if let (Some(x), Some(y)) = (single(a), single(b)) {
        let (s, e) = two_sum(x, y);
        if s.is_finite() && e == 0.0 {
            return point(s);
        }
    }
    Ok(a.add(b)?)
}

/// `a − b`.
///
/// # Errors
///
/// [`CurveError::NotFinite`] if a bound overflows (or, for a quotient or root, if the divisor holds zero or the operand holds a negative number).
pub fn sub(a: Interval, b: Interval) -> Result<Interval, CurveError> {
    add(a, b.neg())
}

/// `a · b`.
///
/// # Errors
///
/// [`CurveError::NotFinite`] if a bound overflows (or, for a quotient or root, if the divisor holds zero or the operand holds a negative number).
pub fn mul(a: Interval, b: Interval) -> Result<Interval, CurveError> {
    if let (Some(x), Some(y)) = (single(a), single(b))
        && let Some(p) = exact_product(x, y)
    {
        return point(p);
    }
    Ok(a.mul(b)?)
}

/// `a / b`.
///
/// # Errors
///
/// [`CurveError::NotFinite`] if `b` contains zero or the quotient overflows.
pub fn div(a: Interval, b: Interval) -> Result<Interval, CurveError> {
    if let (Some(x), Some(y)) = (single(a), single(b))
        && y != 0.0
    {
        let q = x / y;
        if q.is_finite() && exact_product(q, y) == Some(x) {
            return point(q);
        }
    }
    Ok(a.div(b)?)
}

/// `√a`, for `a` ≥ 0.
///
/// # Errors
///
/// [`CurveError::NotFinite`] if `a` holds a negative number.
pub fn sqrt(a: Interval) -> Result<Interval, CurveError> {
    if let Some(x) = single(a)
        && x >= 0.0
    {
        let s = math::sqrt(x)?.get();
        if exact_product(s, s) == Some(x) {
            return point(s);
        }
    }
    Ok(a.sqrt()?)
}

/// `a²`, tight: the exact square of a point when it is a double, else the interval square (never
/// negative).
///
/// # Errors
///
/// [`CurveError::NotFinite`] if a bound overflows (or, for a quotient or root, if the divisor holds zero or the operand holds a negative number).
pub fn square(a: Interval) -> Result<Interval, CurveError> {
    if let Some(x) = single(a)
        && let Some(p) = exact_product(x, x)
    {
        return point(p);
    }
    Ok(a.square()?)
}

/// The Euclidean norm of a vector of intervals.
///
/// The sum of squares is never negative, but widening can push its enclosure's lower end below
/// zero; it is cut at zero, which this function, holding the proof, may do.
///
/// # Errors
///
/// [`CurveError::NotFinite`] if a bound overflows (or, for a quotient or root, if the divisor holds zero or the operand holds a negative number).
pub fn norm(v: &[Interval]) -> Result<Interval, CurveError> {
    let mut s = point(0.0)?;
    for &x in v {
        s = add(s, square(x)?)?;
    }
    if s.lo() < 0.0 {
        s = Interval::new(0.0, s.hi())?;
    }
    sqrt(s)
}

/// An upper bound on |x| for every x in `a`.
#[must_use]
pub fn magnitude(a: Interval) -> f64 {
    let (lo, hi) = (a.lo().abs(), a.hi().abs());
    if lo > hi { lo } else { hi }
}

/// The double nearest the middle of `a`: the point itself for a point.
#[must_use]
pub fn middle(a: Interval) -> f64 {
    f64::midpoint(a.lo(), a.hi())
}

/// An upper bound on the distance between the vectors `a` and `b`, each enclosed.
///
/// # Errors
///
/// [`CurveError::NotFinite`] if a bound overflows (or, for a quotient or root, if the divisor holds zero or the operand holds a negative number).
pub fn distance(a: &[Interval], b: &[Interval]) -> Result<f64, CurveError> {
    let mut d = alloc::vec::Vec::with_capacity(a.len());
    for (&x, &y) in a.iter().zip(b) {
        d.push(sub(x, y)?);
    }
    Ok(norm(&d)?.hi())
}

/// An upper bound on the distance between doubles `a` and the exact vector enclosed by `b`.
///
/// # Errors
///
/// [`CurveError::NotFinite`] if a bound overflows (or, for a quotient or root, if the divisor holds zero or the operand holds a negative number).
pub fn distance_to(a: &[f64], b: &[Interval]) -> Result<f64, CurveError> {
    let mut v = alloc::vec::Vec::with_capacity(a.len());
    for &x in a {
        v.push(point(x)?);
    }
    distance(&v, b)
}
