//! The exact plan arithmetic of joins at any angle (job 6.1): where two layer faces cross, a face's
//! offset from its wall's line, and a point set back along a wall, each worked exactly in whole
//! numbers of any size (num-bigint) and rounded once to the nearest grid step, a half away from
//! zero. So every corner is within half a step (78.125 nm) of the true point on each axis, and a
//! corner of walls along the plan's axes, or of walls on whole ratios such as 3-4-5 where the
//! true point is a grid point, is exact.
//!
//! - [`across`]: `R(o)`, the offset of a face `o` steps to the right of a line along `d`: the true
//!   `o * (dy, -dx) / |d|`, each coordinate rounded once. It is construct-regen's
//!   `wall_plan::Plan::across` (the one rounding a layered wall's faces take), the same rounding
//!   of the same true value, so the faces the join rule meets are the faces the builders build; a
//!   test there pins that the two agree.
//! - [`back`]: the true `t * d / |d|`, `t` steps along a line, each coordinate rounded once: how
//!   far a wrapped end's layers are set back.
//! - [`meet`]: where two lines cross, the true point rounded once: two layer faces' corner.
//!
//! The sizes: a coordinate is within `2^40` steps ([`super::plan::LIMIT`]), a direction within
//! `2^41`, so a cross product is under `2^83` and every numerator here under `2^126`; num-bigint
//! holds them without a bound to keep, and every answer is checked back into the grid.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "arbitrary-precision whole numbers, which never overflow; each division checks its               divisor beside it, and every answer is checked back into the grid"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "geometry's own names: points p, q, directions d, e, coordinates x, y"
)]

use super::plan::{LIMIT, P};
use num_bigint::{BigInt, BigUint, Sign};
use num_integer::Integer;
use num_traits::{Signed, ToPrimitive, Zero};

/// `n / sqrt(dd)` to the nearest whole number, a half away from zero: the largest `m` with
/// `(2m - 1)^2 * dd <= 4 n^2`, found as `floor((isqrt(floor(4 n^2 / dd)) + 1) / 2)`.
fn over_root(n: &BigInt, dd: &BigUint) -> Option<i64> {
    if dd.is_zero() {
        return None;
    }
    let size = n.magnitude();
    let four: BigUint = size * size * 4_u32;
    let v = (four / dd).sqrt();
    let m: BigUint = (v + 1_u32) / 2_u32;
    let m = BigInt::from_biguint(Sign::Plus, m);
    let m = if n.is_negative() { -m } else { m };
    within(&m)
}

/// A whole number back in the grid's range.
fn within(value: &BigInt) -> Option<i64> {
    let v = value.to_i64()?;
    (v.unsigned_abs() <= LIMIT.unsigned_abs()).then_some(v)
}

/// `a / b` to the nearest whole number, a half away from zero.
fn nearest(a: &BigInt, b: &BigInt) -> Option<i64> {
    if b.is_zero() {
        return None;
    }
    let (size_a, size_b) = (a.abs(), b.abs());
    let twice_b: BigInt = &size_b * 2;
    let num: BigInt = size_a * 2 + &size_b;
    let q = num.div_floor(&twice_b);
    let q = if a.is_negative() == b.is_negative() {
        q
    } else {
        -q
    };
    within(&q)
}

fn big(v: i64) -> BigInt {
    BigInt::from(v)
}

/// The square of a direction's length.
fn length_squared([dx, dy]: P) -> BigUint {
    let (x, y) = (big(dx), big(dy));
    (&x * &x + &y * &y).magnitude().clone()
}

/// `R(o)`: the offset of a face `o` steps to the right of a line along `d`, each coordinate the
/// nearest step to the true `o * (dy, -dx) / |d|`, a half away from zero. `None` for a direction
/// with no length or an offset beyond the grid.
#[must_use]
pub(super) fn across(o: i64, d: P) -> Option<P> {
    let [dx, dy] = d;
    let dd = length_squared(d);
    Some([
        over_root(&(big(o) * big(dy)), &dd)?,
        over_root(&(-(big(o) * big(dx))), &dd)?,
    ])
}

/// The true `t * d / |d|`, `t` steps along a line along `d`, each coordinate the nearest step.
#[must_use]
pub(super) fn back(t: i64, d: P) -> Option<P> {
    let [dx, dy] = d;
    let dd = length_squared(d);
    Some([
        over_root(&(big(t) * big(dx)), &dd)?,
        over_root(&(big(t) * big(dy)), &dd)?,
    ])
}

/// Where the line through `p` along `d` crosses the line through `q` along `e`, the true point's
/// coordinates each the nearest step; `None` for lines that do not cross (parallel) or a crossing
/// beyond the grid.
#[must_use]
pub(super) fn meet(p: P, d: P, q: P, e: P) -> Option<P> {
    let ([px, py], [dx, dy], [qx, qy], [ex, ey]) = (p, d, q, e);
    let den = big(dx) * big(ey) - big(dy) * big(ex);
    if den.is_zero() {
        return None;
    }
    let (wx, wy) = (big(qx) - big(px), big(qy) - big(py));
    let num = &wx * big(ey) - &wy * big(ex);
    let x = big(px) * &den + big(dx) * &num;
    let y = big(py) * &den + big(dy) * &num;
    Some([nearest(&x, &den)?, nearest(&y, &den)?])
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    reason = "tests"
)]
mod tests {
    use super::*;

    /// On whole ratios the answers are exact; along an axis an offset is itself.
    #[test]
    fn whole_ratios_and_axes_are_exact() {
        assert_eq!(across(500, [3000, 4000]), Some([400, -300]));
        assert_eq!(across(-500, [3000, 4000]), Some([-400, 300]));
        assert_eq!(across(80_000, [10_000, 0]), Some([0, -80_000]));
        assert_eq!(across(3, [0, -7]), Some([-3, 0]));
        assert_eq!(back(10, [6, 8]), Some([6, 8]));
        assert_eq!(meet([0, 0], [1, 0], [5, -3], [0, 1]), Some([5, 0]));
        assert_eq!(meet([0, 0], [3, 4], [0, 50], [4, -3]), Some([24, 32]));
        // (4.8, 6.4) to the nearest step.
        assert_eq!(meet([0, 0], [3, 4], [0, 10], [4, -3]), Some([5, 6]));
        assert_eq!(meet([0, 0], [1, 0], [0, 5], [2, 0]), None);
    }

    /// A half rounds away from zero; every answer is within half a step of the true value.
    #[test]
    fn each_coordinate_is_the_nearest_step() {
        // (1, 1) at offset 1: (0.7071, -0.7071) -> (1, -1).
        assert_eq!(across(1, [1, 1]), Some([1, -1]));
        // On (6, 8), R(1) is (0.8, -0.6): (1, -1).
        assert_eq!(across(1, [6, 8]), Some([1, -1]));
        // 5 / 2 = 2.5 -> 3, -5 / 2 -> -3.
        assert_eq!(nearest(&big(5), &big(2)), Some(3));
        assert_eq!(nearest(&big(-5), &big(2)), Some(-3));
        assert_eq!(nearest(&big(5), &big(-2)), Some(-3));
        for d in [[7, 3], [-5, 11], [123_456_789, 98_765], [1, -1_000_000_000]] {
            let length = ((d[0] as f64).powi(2) + (d[1] as f64).powi(2)).sqrt();
            for t in [1_i64, 3, 80_000, -737_280, 64_000_000] {
                let [x, y] = back(t, d).unwrap();
                assert!((x as f64 - t as f64 * d[0] as f64 / length).abs() <= 0.5 + 1e-6);
                assert!((y as f64 - t as f64 * d[1] as f64 / length).abs() <= 0.5 + 1e-6);
            }
        }
        // Two lines at an angle off every whole ratio: within half a step of the true crossing.
        let (p, d, q, e) = ([0, 0], [7, 3], [1000, -50], [-2, 9]);
        let [x, y] = meet(p, d, q, e).unwrap();
        let den = (d[0] * e[1] - d[1] * e[0]) as f64;
        let s = ((q[0] - p[0]) * e[1] - (q[1] - p[1]) * e[0]) as f64 / den;
        assert!((x as f64 - s * d[0] as f64).abs() <= 0.5);
        assert!((y as f64 - s * d[1] as f64).abs() <= 0.5);
    }
}
