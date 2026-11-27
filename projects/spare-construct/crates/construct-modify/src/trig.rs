//! The half-tangent of an angle, in whole numbers: the nearest whole number to `tan(θ/2)` times
//! 2^[`PRECISION`], for a turn `θ` by any angle. [`crate::exact::Turn`] makes the turn it holds from
//! it: the turn whose cosine and sine are `(1 - t^2) / (1 + t^2)` and `2t / (1 + t^2)` for that
//! half-tangent `t`, fractions whose squares add to exactly one, so what is held is a true rotation,
//! and it differs from the angle asked by less than 2^-159 of a radian (the decision note
//! `notes/CON-A3-F4-P1-modify-tools.md`, decision 2).
//!
//! How, all on whole numbers of any size (num-bigint), in fixed point with [`WORK`] bits after the
//! point, every division rounding down:
//! - pi by Machin's formula, `pi = 16 atan(1/5) - 4 atan(1/239)` (J. Machin, 1706), each arctangent
//!   by its series `atan(1/k) = sum of (-1)^n / ((2n + 1) k^(2n + 1))`. Each term is one floor
//!   division of the last power (a floor of a floor by a whole number is the floor of the whole
//!   division) and one more by `2n + 1`, so each is under 2 of the last bits low, and the series
//!   stops when a power is zero, its tail then under one: 41 and 12 terms, so pi is within
//!   16 x 83 + 4 x 25 = 1,428 of the last bits.
//! - the half angle `x`, `|x| <= pi/8`, as pi times a fraction of at most 1/8: within 2^9 of them.
//! - its sine and cosine by their Taylor series, each term from the last, under 3 of the last bits
//!   each over fewer than 20 terms; with `x`'s own error, each within 2^10.
//! - the tangent, sine over cosine (the cosine is above 0.92 here): within 2^11 of the last bits.
//!
//! So the working's tangent is within 2^11 of [`WORK`]'s last bits, under 2^-21 of the last bit of
//! [`PRECISION`], and the half-tangent given is within half a last bit and that: under 2^-160. The
//! turn held, `2 atan(t)`, is within twice that of the angle asked, under 2^-159 of a radian.
//!
//! Why this is written here and not taken from a library: the project's rule is a bound proved in
//! whole numbers (no floating point in anything stored or compared), and no permissively licensed
//! library gives a tangent as an exact fraction with such a bound. num-bigint (MIT OR Apache-2.0)
//! does the arithmetic; the series are the textbook ones.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "whole numbers of any size cannot overflow, and every divisor here is above zero"
)]

use crate::exact::FULL;
use num_bigint::BigInt;
use num_traits::{One, Zero};
use std::sync::OnceLock;

/// Bits after the point of a half-tangent: `tan(θ/2)` is held as `n / 2^PRECISION`.
pub const PRECISION: u32 = 160;

/// Bits the working carries past [`PRECISION`], so that its own error (under 2^11 of its last
/// bits, the module header) stays under 2^-21 of the last bit given.
const GUARD: u32 = 32;

/// Bits after the point of the working.
pub const WORK: u32 = PRECISION + GUARD;

/// One, in the working's fixed point.
fn one() -> BigInt {
    BigInt::one() << WORK
}

/// `atan(1/k)` in the working's fixed point, by its series.
fn arctan_inverse(k: u32) -> BigInt {
    let k = BigInt::from(k);
    let kk = &k * &k;
    let mut power = one() / &k;
    let mut odd = BigInt::one();
    let mut sum = BigInt::zero();
    let mut add = true;
    while !power.is_zero() {
        let term = &power / &odd;
        if add {
            sum += term;
        } else {
            sum -= term;
        }
        add = !add;
        power /= &kk;
        odd += 2;
    }
    sum
}

/// Pi in the working's fixed point, by Machin's formula; worked out once.
fn pi() -> &'static BigInt {
    static PI: OnceLock<BigInt> = OnceLock::new();
    PI.get_or_init(|| arctan_inverse(5) * 16 - arctan_inverse(239) * 4)
}

/// The sine and cosine of `x` given in the working's fixed point, `0 <= x <= pi/8`, by their
/// series.
fn sin_cos(x: &BigInt) -> (BigInt, BigInt) {
    let x2 = (x * x) >> WORK;
    let series = |first: BigInt, mut k: BigInt| {
        let mut sum = first.clone();
        let mut term = first;
        let mut add = false;
        loop {
            // The next term is the last times x^2 over the next two whole numbers.
            let divisor = &k * (&k + 1);
            term = ((&term * &x2) >> WORK) / divisor;
            if term.is_zero() {
                return sum;
            }
            if add {
                sum += &term;
            } else {
                sum -= &term;
            }
            add = !add;
            k += 2;
        }
    };
    // sin x = x - x^3/3! + x^5/5! ...; cos x = 1 - x^2/2! + x^4/4! ...
    (
        series(x.clone(), BigInt::from(2)),
        series(one(), BigInt::one()),
    )
}

/// `value / 2^bits` to the nearest whole number, a half rounding up, for a `value` not below zero.
fn nearest_shift(value: &BigInt, bits: u32) -> BigInt {
    (value + (BigInt::one() << (bits - 1))) >> bits
}

/// The nearest whole number to `tan(x) 2^PRECISION` for the half angle `x = pi r / (den FULL)`,
/// `r` angle units over `den`, `0 <= r / den <=` an eighth of a turn's units, so `0 <= x <= pi/8`.
#[must_use]
pub fn half_tangent(r: u128, den: u128) -> BigInt {
    let parts = BigInt::from(den) * BigInt::from(FULL);
    let x = (pi() * BigInt::from(r)) / parts;
    let (sin, cos) = sin_cos(&x);
    let tangent = (sin << WORK) / cos;
    nearest_shift(&tangent, GUARD)
}

/// The nearest whole number to `|cross| / (root + dot) 2^PRECISION`, `root` the square root of
/// `product`, for `dot >= 0` and `product >= 1`: the half-tangent of the turn between two
/// directions whose dot and cross products are `dot` and `cross` and whose lengths multiply to
/// `root`. The root is the floor of its value in the working's fixed point (`BigInt::sqrt`), so the
/// denominator is within one of its last bits, under 2^-192 of itself, and the half-tangent, at most
/// one, within 2 of the working's last bits.
#[must_use]
pub fn half_tangent_between(cross: &BigInt, dot: &BigInt, product: &BigInt) -> BigInt {
    let root = (product << (2 * WORK)).sqrt();
    let denominator = root + (dot << WORK);
    let magnitude = num_traits::Signed::abs(cross);
    let tangent = (magnitude << (2 * WORK)) / denominator;
    nearest_shift(&tangent, GUARD)
}
