//! The exact model shared by the test files: doubles read as rationals from their bits, rounding of
//! rationals to integers and to doubles, and signs of expressions with one square root. None of it
//! calls the code under test.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    clippy::single_match_else,
    clippy::unreadable_literal,
    reason = "test cases and oracles, named as in the formulas they check; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(clippy::expect_used)]

use std::cmp::Ordering;

use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use proptest::test_runner::Config;

/// The configuration a property runs with: `cases` random cases.
pub fn config(cases: u32) -> Config {
    Config {
        cases,
        ..Config::default()
    }
}

/// The bits of the fraction field of a double.
#[allow(
    dead_code,
    reason = "some of the test files that share this module do not use it"
)]
pub const FRACTION_MASK: u64 = (1 << 52) - 1;

/// An integer as a rational.
#[allow(
    dead_code,
    reason = "some of the test files that share this module do not use it"
)]
pub fn int(v: i128) -> BigRational {
    BigRational::from_integer(BigInt::from(v))
}

/// 2^k exactly, for any integer k.
#[allow(
    dead_code,
    reason = "some of the test files that share this module do not use it"
)]
pub fn pow2(k: i32) -> BigRational {
    let p = BigInt::one() << k.unsigned_abs();
    if k >= 0 {
        BigRational::from_integer(p)
    } else {
        BigRational::new(BigInt::one(), p)
    }
}

/// 2^k as a double, for a normal power of two (−1022 ≤ k ≤ 1023), built from its bits: exact and
/// the same on every target, which the platform's `powi` does not promise (chapter 21).
#[allow(
    dead_code,
    reason = "some of the test files that share this module do not use it"
)]
pub fn pow2f(k: i32) -> f64 {
    assert!((-1022..=1023).contains(&k), "2^{k} is not a normal double");
    let biased = u64::try_from(k + 1023).expect("a normal exponent is positive when biased");
    f64::from_bits(biased << 52)
}

/// The exact value of a finite double, read from its sign, exponent and fraction bits.
#[allow(
    dead_code,
    reason = "some of the test files that share this module do not use it"
)]
pub fn exact(x: f64) -> BigRational {
    assert!(x.is_finite(), "exact() of a non-finite double {x}");
    let bits = x.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & FRACTION_MASK;
    let (significand, exponent) = if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), biased - 1075)
    };
    let magnitude = BigRational::from_integer(BigInt::from(significand)) * pow2(exponent);
    if bits >> 63 == 1 {
        -magnitude
    } else {
        magnitude
    }
}

/// The nearest integer to a rational, ties to even.
#[allow(
    dead_code,
    reason = "some of the test files that share this module do not use it"
)]
pub fn round_half_even(x: &BigRational) -> BigInt {
    let floor = x.floor().to_integer();
    let frac = x - BigRational::from_integer(floor.clone());
    let half = BigRational::new(BigInt::one(), BigInt::from(2));
    match frac.cmp(&half) {
        Ordering::Less => floor,
        Ordering::Greater => floor + 1,
        Ordering::Equal => {
            if floor.is_even() {
                floor
            } else {
                floor + 1
            }
        }
    }
}

/// A big integer as an `i128` (it must fit).
#[allow(
    dead_code,
    reason = "some of the test files that share this module do not use it"
)]
pub fn to_i128(v: &BigInt) -> i128 {
    v.to_i128().expect("fits i128")
}

/// Whether a rational is an integer.
#[allow(
    dead_code,
    reason = "some of the test files that share this module do not use it"
)]
pub fn is_integer(x: &BigRational) -> bool {
    x.is_integer()
}

/// The next double above a finite double.
#[allow(
    dead_code,
    reason = "some of the test files that share this module do not use it"
)]
pub fn step_up(x: f64) -> f64 {
    if x == 0.0 {
        f64::from_bits(1)
    } else if x > 0.0 {
        f64::from_bits(x.to_bits() + 1)
    } else {
        f64::from_bits(x.to_bits() - 1)
    }
}

/// The next double below a finite double.
#[allow(
    dead_code,
    reason = "some of the test files that share this module do not use it"
)]
pub fn step_down(x: f64) -> f64 {
    -step_up(-x)
}

/// Whether `d` is the double nearest the rational `x`, ties to even (an even significand).
#[allow(
    dead_code,
    reason = "some of the test files that share this module do not use it"
)]
pub fn is_nearest_double(d: f64, x: &BigRational) -> bool {
    let e = exact(d);
    let two = int(2);
    let below = (exact(step_down(d)) + &e) / &two;
    let above = (exact(step_up(d)) + &e) / &two;
    let even = d.to_bits() & 1 == 0;
    let lower_ok = if even { *x >= below } else { *x > below };
    let upper_ok = if even { *x <= above } else { *x < above };
    lower_ok && upper_ok
}

/// The sign of `a − b·√n` for rationals a, b and a positive integer n: exact, by comparing squares.
#[allow(
    dead_code,
    reason = "some of the test files that share this module do not use it"
)]
pub fn sign_minus_sqrt(a: &BigRational, b: &BigRational, n: u64) -> Ordering {
    let zero = BigRational::zero();
    let rn = int(i128::from(n));
    match (a.cmp(&zero), b.cmp(&zero)) {
        (Ordering::Equal, Ordering::Equal) => Ordering::Equal,
        (Ordering::Greater | Ordering::Equal, Ordering::Less | Ordering::Equal) => {
            Ordering::Greater
        }
        (Ordering::Less | Ordering::Equal, Ordering::Greater | Ordering::Equal) => Ordering::Less,
        // Both positive: compare a² with b²n.
        (Ordering::Greater, Ordering::Greater) => (a * a).cmp(&(b * b * &rn)),
        // Both negative: a − b√n > 0 exactly when |a| < |b|√n.
        (Ordering::Less, Ordering::Less) => (b * b * &rn).cmp(&(a * a)),
    }
}

/// |x − y| ≤ bound·|y|, for a relative error check (y = 0 requires x = 0).
#[allow(
    dead_code,
    reason = "some of the test files that share this module do not use it"
)]
pub fn within_relative(x: &BigRational, y: &BigRational, bound: &BigRational) -> bool {
    (x - y).abs() <= bound * y.abs()
}
