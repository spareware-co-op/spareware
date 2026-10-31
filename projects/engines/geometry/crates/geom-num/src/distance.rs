//! Exact distance comparisons on grid points: whether a gap is within a clearance (19.10).
//!
//! What it does: each function answers with the sign of an exactly computed squared gap less c²:
//! [`point_point`], [`point_line`], [`point_plane`] and [`line_line`]. [`product_difference`] gives
//! the sign of a · b − c · d of dot products, which is how the closest feature of a triangle is
//! told. No square root is ever taken: both sides are squared, so the comparison stays on integers.
//!
//! Guarantees:
//!
//! - **Exact, and within 256 bits** (19.10). Every coordinate difference has |x| ≤ D = 2^41 − 2 and
//!   a clearance is at most [`MAX_CLEARANCE`] = 2^42, and each function's budget is below. No step
//!   rounds, and a step that did not fit would be reported, not answered.
//! - **Never a guess.** `Zero` means the gap is exactly c; a line or plane that does not exist is
//!   [`NumError::Degenerate`], decided exactly.
//! - **Unmoved by order.** Swapping the two points of [`point_point`] or the two lines of
//!   [`line_line`] changes nothing, and neither does moving every input by one grid vector.
//!
//! Bit budget, with D = 2^41 − 2, in `i128` up to a dot or cross product and in [`I256`] (limit
//! 2^255) from there:
//!
//! - a dot of differences is at most 3D² < 2^84;
//! - a component of a cross product of differences is at most 2D² < 2^83, and its length squared
//!   at most 12D⁴ < 2^168;
//! - a triple product is at most 3 · D · 2D² = 6D³ < 2^126, and its square below 2^252;
//! - c² ≤ 2^84, so c² times any squared length above is below 2^252;
//! - two products of dots are below 9D⁴ < 2^168 each.
//!
//! The extremes are tested with every coordinate at ±(2^40 − 1).
//!
//! Method: chapter 19.10's table
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`): |ap|²·|ab|² − (ap·ab)²
//! is Lagrange's identity for |ap × ab|², so a point's distance from a line is compared by squares
//! without a square root, and likewise (n · w)²/|n|² for a plane and for two lines.
//!
//! Cost: constant; at most a few multiplications in 256 bits. No allocation.

#![allow(
    clippy::many_single_char_names,
    reason = "a, b, d and p are points and n, u, v and w vectors, as in the formulas"
)]

use crate::error::NumError;
use crate::grid::P3;
use crate::sign::Sign;
use crate::wide::I256;

/// The largest clearance: wider than the frame's diagonal, √3 · 2^41 < 2^42.
pub const MAX_CLEARANCE: u64 = 1 << 42;

/// A difference of grid points, its components at most 2^41 − 2.
type Run = [i128; 3];

/// `b − a`. Coordinates are below 2^40, so each difference fits.
fn run(a: P3, b: P3) -> Run {
    let part = |p: i64, q: i64| i128::from(q).wrapping_sub(i128::from(p));
    [
        part(a.x.get(), b.x.get()),
        part(a.y.get(), b.y.get()),
        part(a.z.get(), b.z.get()),
    ]
}

/// u · v: at most 3D² < 2^84 for differences, and below 2^126 for a difference and a cross
/// product, so wrapping steps are exact.
fn dot(u: Run, v: Run) -> i128 {
    let [a, b, c] = u;
    let [x, y, z] = v;
    a.wrapping_mul(x)
        .wrapping_add(b.wrapping_mul(y))
        .wrapping_add(c.wrapping_mul(z))
}

/// u × v of differences: each component at most 2D² < 2^83, so wrapping steps are exact.
fn cross(u: Run, v: Run) -> Run {
    let [a, b, c] = u;
    let [x, y, z] = v;
    let term =
        |p: i128, q: i128, r: i128, s: i128| p.wrapping_mul(q).wrapping_sub(r.wrapping_mul(s));
    [term(b, z, c, y), term(c, x, a, z), term(a, y, b, x)]
}

/// a · b in 256 bits, for values below 2^127.
fn times(a: i128, b: i128) -> Result<I256, NumError> {
    I256::from_i128(a)
        .checked_mul(&I256::from_i128(b))
        .ok_or(NumError::BudgetExceeded)
}

/// c², at most 2^84, or the clearance refused.
fn squared(clearance: u64) -> Result<i128, NumError> {
    if clearance > MAX_CLEARANCE {
        return Err(NumError::OutOfRange {
            value: i128::from(clearance),
        });
    }
    let c = i128::from(clearance);
    Ok(c.wrapping_mul(c))
}

/// The sign of `gap − c² · length`: the comparison every function here ends in.
fn against(gap: &I256, length: i128, clearance: u64) -> Result<Sign, NumError> {
    let limit = times(squared(clearance)?, length)?;
    let difference = gap.checked_sub(&limit).ok_or(NumError::BudgetExceeded)?;
    Ok(difference.signum())
}

/// The sign of |pq|² − c²: `Negative` when `p` and `q` are closer than `clearance`.
///
/// # Errors
///
/// [`NumError::OutOfRange`] for a clearance over [`MAX_CLEARANCE`].
pub fn point_point(p: P3, q: P3, clearance: u64) -> Result<Sign, NumError> {
    let u = run(p, q);
    against(&I256::from_i128(dot(u, u)), 1, clearance)
}

/// The sign of the squared distance from `p` to the line through `a` and `b`, less c², scaled by
/// |ab|²: |ap|²·|ab|² − (ap·ab)² − c²·|ab|².
///
/// # Errors
///
/// [`NumError::Degenerate`] when `a` and `b` are one point, and [`NumError::OutOfRange`] for a
/// clearance over [`MAX_CLEARANCE`].
pub fn point_line(a: P3, b: P3, p: P3, clearance: u64) -> Result<Sign, NumError> {
    let (ab, ap) = (run(a, b), run(a, p));
    let length = dot(ab, ab);
    if length == 0 {
        return Err(NumError::Degenerate);
    }
    let along = dot(ap, ab);
    let gap = times(dot(ap, ap), length)?
        .checked_sub(&times(along, along)?)
        .ok_or(NumError::BudgetExceeded)?;
    against(&gap, length, clearance)
}

/// The sign of the squared distance from `p` to the plane through `a`, `b` and `d`, less c², scaled
/// by |n|² for its normal n: (n · (p − a))² − c²·|n|².
///
/// # Errors
///
/// [`NumError::Degenerate`] when the three points are collinear, and [`NumError::OutOfRange`] for a
/// clearance over [`MAX_CLEARANCE`].
pub fn point_plane(a: P3, b: P3, d: P3, p: P3, clearance: u64) -> Result<Sign, NumError> {
    let n = cross(run(a, b), run(a, d));
    triple_against(n, run(a, p), clearance)
}

/// The sign of the squared distance between the lines through `p0`, `p1` and through `q0`, `q1`,
/// less c², scaled by |u × v|² for their directions u and v: (w·(u × v))² − c²·|u × v|². What it
/// says of the two segments holds where their closest points are inside both.
///
/// # Errors
///
/// [`NumError::Degenerate`] when the lines are parallel or a line's two points are one, and
/// [`NumError::OutOfRange`] for a clearance over [`MAX_CLEARANCE`].
pub fn line_line(p0: P3, p1: P3, q0: P3, q1: P3, clearance: u64) -> Result<Sign, NumError> {
    let n = cross(run(p0, p1), run(q0, q1));
    triple_against(n, run(p0, q0), clearance)
}

/// The sign of (n · w)² − c²·|n|².
fn triple_against(n: Run, w: Run, clearance: u64) -> Result<Sign, NumError> {
    if n == [0; 3] {
        return Err(NumError::Degenerate);
    }
    let [x, y, z] = n;
    let triple = dot(n, w);
    let squares = times(x, x)?
        .checked_add(&times(y, y)?)
        .and_then(|s| s.checked_add(&times(z, z).ok()?))
        .ok_or(NumError::BudgetExceeded)?;
    let limit = I256::from_i128(squared(clearance)?)
        .checked_mul(&squares)
        .ok_or(NumError::BudgetExceeded)?;
    let difference = times(triple, triple)?
        .checked_sub(&limit)
        .ok_or(NumError::BudgetExceeded)?;
    Ok(difference.signum())
}

/// The sign of a · b − c · d, for dot products of differences, each below 2^84, as the tests of a
/// point's closest feature on a triangle use them.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for values past those bounds, which grid points cannot give.
pub fn product_difference(a: i128, b: i128, c: i128, d: i128) -> Result<Sign, NumError> {
    let difference = times(a, b)?
        .checked_sub(&times(c, d)?)
        .ok_or(NumError::BudgetExceeded)?;
    Ok(difference.signum())
}

/// (b − a) · (c − a): the dot product [`product_difference`] takes.
#[must_use]
pub fn dot_from(a: P3, b: P3, c: P3) -> i128 {
    dot(run(a, b), run(a, c))
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// A small coordinate: each comparison below is the same formula at every scale.
    fn small() -> i64 {
        let v: i8 = kani::any();
        kani::assume(v.unsigned_abs() <= 2);
        i64::from(v)
    }

    fn point() -> P3 {
        P3::new(small(), small(), small()).unwrap()
    }

    fn clearance() -> u64 {
        let c: u8 = kani::any();
        kani::assume(c <= 4);
        u64::from(c)
    }

    /// The same difference and products in `i64`, far from overflow at this size.
    fn small_run(a: P3, b: P3) -> [i64; 3] {
        [
            b.x.get() - a.x.get(),
            b.y.get() - a.y.get(),
            b.z.get() - a.z.get(),
        ]
    }

    fn small_dot(u: [i64; 3], v: [i64; 3]) -> i64 {
        let ([a, b, c], [x, y, z]) = (u, v);
        a * x + b * y + c * z
    }

    fn small_cross(u: [i64; 3], v: [i64; 3]) -> [i64; 3] {
        let ([a, b, c], [x, y, z]) = (u, v);
        [b * z - c * y, c * x - a * z, a * y - b * x]
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn point_point_has_the_sign_of_its_gap() {
        let (p, q, c) = (point(), point(), clearance());
        let u = small_run(p, q);
        let c = i64::try_from(c).unwrap_or(0);
        assert!(point_point(p, q, c.cast_unsigned()) == Ok(Sign::of_i64(small_dot(u, u) - c * c)));
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn point_line_has_the_sign_of_its_gap() {
        let (a, b, p, c) = (point(), point(), point(), clearance());
        let (ab, ap) = (small_run(a, b), small_run(a, p));
        let length = small_dot(ab, ab);
        kani::assume(length != 0);
        let along = small_dot(ap, ab);
        let c = i64::try_from(c).unwrap_or(0);
        let gap = small_dot(ap, ap) * length - along * along - c * c * length;
        assert!(point_line(a, b, p, c.cast_unsigned()) == Ok(Sign::of_i64(gap)));
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn point_plane_has_the_sign_of_its_gap() {
        let (a, b, d, p, c) = (point(), point(), point(), point(), clearance());
        let n = small_cross(small_run(a, b), small_run(a, d));
        kani::assume(n != [0; 3]);
        let h = small_dot(n, small_run(a, p));
        let c = i64::try_from(c).unwrap_or(0);
        let gap = h * h - c * c * small_dot(n, n);
        assert!(point_plane(a, b, d, p, c.cast_unsigned()) == Ok(Sign::of_i64(gap)));
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn line_line_has_the_sign_of_its_gap() {
        let (p0, p1, q0, q1, c) = (point(), point(), point(), point(), clearance());
        let n = small_cross(small_run(p0, p1), small_run(q0, q1));
        kani::assume(n != [0; 3]);
        let h = small_dot(n, small_run(p0, q0));
        let c = i64::try_from(c).unwrap_or(0);
        let gap = h * h - c * c * small_dot(n, n);
        assert!(line_line(p0, p1, q0, q1, c.cast_unsigned()) == Ok(Sign::of_i64(gap)));
    }
}
