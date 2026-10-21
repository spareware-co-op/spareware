//! Tests of `geom_math::dd` against exact rational arithmetic (`num-rational`).
//!
//! What this file proves:
//!
//! - Accuracy: for random double-double operands of many magnitudes, including sums that cancel
//!   almost completely, every operation's result is within a relative 2^−100 of the exact result
//!   of the same operation on the operands' exact values, and within that operation's documented
//!   bound (2u² to 7u², u = 2^−53; 2^−100 = 64u²), so the published and derived bounds are checked
//!   too. The square root is checked by squaring exact bounds.
//! - Every result is normalized: `hi` is `hi + lo` rounded to nearest.
//! - Exact parts are exact: `from_i64`, `from_i128` below 2^106, `exact_product` inside its
//!   precondition (and `None` outside it), negation.
//! - `round_shifted` rounds the exact value `hi + lo − offset` to the nearest integer, ties to
//!   even, and reports exactness, for random values, constructed ties and near-ties on either
//!   side, values at ±(2^40 − 1), and offsets up to ±2^62.
//! - Errors: overflow is `NotFinite`, division by zero `Degenerate`, a negative square root
//!   `NegativeSqrt`.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(clippy::unwrap_used)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]

mod common;

use common::{
    config, exact, int, is_integer, pow2, pow2f, round_half_even, to_i128, within_relative,
};
use geom_math::DD;
use geom_num::NumError;
use geom_num::grid::COORD_MAX;
use num_rational::BigRational;
use proptest::prelude::*;

/// The exact value of a double-double.
fn value(d: DD) -> BigRational {
    exact(d.hi()) + exact(d.lo())
}

/// A double with a random 53-bit significand and an exponent in `range`.
fn double(range: std::ops::RangeInclusive<i32>) -> BoxedStrategy<f64> {
    (any::<bool>(), range, any::<u64>())
        .prop_map(|(neg, e, bits)| {
            let m = (bits & ((1u64 << 52) - 1)) | (1u64 << 52);
            let v = m as f64 * pow2f(e - 52);
            if neg { -v } else { v }
        })
        .boxed()
}

/// A normalized double-double with leading part exponent in `range` and a random trailing part
/// (zero, or up to half an ulp of the leading part).
fn dd(range: std::ops::RangeInclusive<i32>) -> BoxedStrategy<DD> {
    (
        double(range),
        prop_oneof![1 => Just(0.0f64), 4 => -1.0f64..1.0],
    )
        .prop_map(|(hi, f)| DD::from_sum(hi, hi * f * pow2f(-53)).unwrap())
        .boxed()
}

fn rel() -> BigRational {
    pow2(-100)
}

/// a·u² + b·u³ with u = 2^−53: the form of each operation's documented bound.
fn bound(a: i128, b: i128) -> BigRational {
    int(a) * pow2(-106) + int(b) * pow2(-159)
}

/// Whether r is within a relative `beta` of the square root of x: √x(1 − β) ≤ r ≤ √x(1 + β).
fn root_within(r: &BigRational, x: &BigRational, beta: &BigRational) -> bool {
    let one = int(1);
    let lo = r / (&one + beta);
    let hi = r / (&one - beta);
    &lo * &lo <= *x && *x <= &hi * &hi
}

fn normalized(d: DD) -> bool {
    d.hi() + d.lo() == d.hi()
}

proptest! {
    #![proptest_config(config(4096))]

    #[test]
    fn add_and_sub_are_accurate(a in dd(-300..=300), b in dd(-300..=300)) {
        let s = a.add(b).unwrap();
        prop_assert!(normalized(s));
        prop_assert!(within_relative(&value(s), &(value(a) + value(b)), &rel()));
        prop_assert!(within_relative(&value(s), &(value(a) + value(b)), &bound(3, 13)));
        let d = a.sub(b).unwrap();
        prop_assert!(within_relative(&value(d), &(value(a) - value(b)), &rel()));
        prop_assert!(within_relative(&value(d), &(value(a) - value(b)), &bound(3, 13)));
        prop_assert_eq!(value(-a), -value(a));
    }

    #[test]
    fn cancelling_sums_are_accurate(a in dd(-200..=200), k in -60i32..=0, f in -1.0f64..1.0) {
        // b ≈ −a, so the sum keeps only a's last bits and the perturbation.
        let p = DD::from_sum(a.hi() * f * pow2f(k - 40), 0.0).unwrap();
        let b = (-a).add(p).unwrap();
        let s = a.add(b).unwrap();
        prop_assert!(within_relative(&value(s), &(value(a) + value(b)), &rel()));
        prop_assert!(within_relative(&value(s), &(value(a) + value(b)), &bound(3, 13)));
    }

    #[test]
    fn products_are_accurate(a in dd(-250..=250), b in dd(-250..=250), y in double(-250..=250)) {
        let p = a.mul(b).unwrap();
        prop_assert!(normalized(p));
        prop_assert!(within_relative(&value(p), &(value(a) * value(b)), &rel()));
        prop_assert!(within_relative(&value(p), &(value(a) * value(b)), &bound(7, 0)));
        let q = a.mul_f64(y).unwrap();
        prop_assert!(within_relative(&value(q), &(value(a) * exact(y)), &rel()));
        prop_assert!(within_relative(&value(q), &(value(a) * exact(y)), &bound(2, 0)));
        let s = a.add_f64(y).unwrap();
        prop_assert!(within_relative(&value(s), &(value(a) + exact(y)), &rel()));
        prop_assert!(within_relative(&value(s), &(value(a) + exact(y)), &bound(2, 5)));
    }

    #[test]
    fn division_is_accurate(a in dd(-250..=250), b in dd(-250..=250)) {
        let q = a.div(b).unwrap();
        prop_assert!(normalized(q));
        prop_assert!(within_relative(&value(q), &(value(a) / value(b)), &rel()));
        prop_assert!(within_relative(&value(q), &(value(a) / value(b)), &bound(4, 60)));
    }

    #[test]
    fn square_root_is_accurate(a in dd(-600..=600)) {
        let a = if a.hi() < 0.0 { -a } else { a };
        let r = a.sqrt().unwrap();
        prop_assert!(normalized(r));
        let v = value(r);
        let one = int(1);
        let lo = &v * (&one - rel());
        let hi = &v * (&one + rel());
        let x = value(a);
        prop_assert!(&lo * &lo <= x && x <= &hi * &hi);
        prop_assert!(root_within(&v, &x, &bound(5, 0)));
    }

    #[test]
    fn small_integers_and_their_roots(n in 1u64..(1 << 62)) {
        let a = DD::from_i64(i64::try_from(n).unwrap());
        prop_assert_eq!(value(a), int(i128::from(n)));
        let r = a.sqrt().unwrap();
        let v = value(r);
        let x = int(i128::from(n));
        let lo = &v * (int(1) - rel());
        let hi = &v * (int(1) + rel());
        prop_assert!(&lo * &lo <= x && x <= &hi * &hi);
        prop_assert!(root_within(&v, &x, &bound(5, 0)));
    }

    #[test]
    fn integers_convert_exactly(v in any::<i64>(), w in -(1i128 << 106) + 1..(1i128 << 106)) {
        let d = DD::from_i64(v);
        prop_assert!(normalized(d));
        prop_assert_eq!(value(d), int(i128::from(v)));
        let e = DD::from_i128(w).unwrap();
        prop_assert!(normalized(e));
        prop_assert_eq!(value(e), int(w));
    }

    #[test]
    fn exact_products(a in double(-500..=500), b in double(-500..=500)) {
        let ea = ((a.to_bits() >> 52) & 0x7ff) as i32 - 1023;
        let eb = ((b.to_bits() >> 52) & 0x7ff) as i32 - 1023;
        let ok = (-970..=1021).contains(&(ea + eb));
        match DD::exact_product(a, b) {
            Some(p) => {
                prop_assert!(ok);
                prop_assert_eq!(value(p), exact(a) * exact(b));
                prop_assert_eq!(p.hi(), a * b);
            }
            None => prop_assert!(!ok),
        }
    }

    #[test]
    fn rounding_is_exact_and_ties_go_to_even(
        base in prop_oneof![
            -(1i64 << 60)..(1i64 << 60),
            -100i64..100,
            Just(COORD_MAX),
            Just(-COORD_MAX)
        ],
        frac in prop_oneof![Just(0.0f64), Just(0.5), Just(-0.5), -1.0f64..1.0, Just(0.25)],
        nudge in prop_oneof![Just(0.0f64), -1.0f64..1.0],
        offset in prop_oneof![Just(0i128), -(1i128 << 62)..(1i128 << 62)],
    ) {
        // value = base + frac + tiny nudge, held exactly as a double-double where possible.
        let hi = DD::from_i64(base).add_f64(frac).unwrap();
        let tiny = nudge * pow2f(-80);
        let d = hi.add_f64(tiny).unwrap();
        let x = value(d) - int(offset);
        let r = d.round_shifted(offset).unwrap();
        prop_assert_eq!(r.value(), to_i128(&round_half_even(&x)));
        prop_assert_eq!(r.is_exact(), is_integer(&x));
    }
}

#[test]
fn rounding_examples() {
    let r = |hi: f64, lo: f64, off: i128| DD::from_sum(hi, lo).unwrap().round_shifted(off).unwrap();
    assert_eq!(
        (r(2.5, 0.0, 0).value(), r(2.5, 0.0, 0).is_exact()),
        (2, false)
    );
    assert_eq!(r(3.5, 0.0, 0).value(), 4);
    assert_eq!(r(-2.5, 0.0, 0).value(), -2);
    assert_eq!(r(-3.5, 0.0, 0).value(), -4);
    assert_eq!(r(2.5, 1e-30, 0).value(), 3);
    assert_eq!(r(2.5, -1e-30, 0).value(), 2);
    // The offset shifts the parity: 2.5 − 1 = 1.5 rounds to 2.
    assert_eq!(r(2.5, 0.0, 1).value(), 2);
    // A tie carried in the trailing part of a large value.
    let big = pow2f(60);
    assert_eq!(r(big, 0.5, 0).value(), (1i128 << 60));
    assert_eq!(r(big, 0.5, 1).value(), (1i128 << 60));
    assert_eq!(r(big, -0.5, 0).value(), (1i128 << 60));
    assert_eq!(r(big + 2.0 * 256.0, 0.5, 0).value(), (1i128 << 60) + 512);
    let whole = r(big, 1.0, 0);
    assert_eq!((whole.value(), whole.is_exact()), ((1i128 << 60) + 1, true));
    let top = r(COORD_MAX as f64, 0.0, 0);
    assert_eq!((top.value(), top.is_exact()), (i128::from(COORD_MAX), true));
    assert_eq!(DD::from_f64(pow2f(101)).unwrap().round_shifted(0), None);
}

#[test]
fn errors() {
    let max = DD::from_f64(f64::MAX).unwrap();
    assert_eq!(max.add(max), Err(NumError::NotFinite));
    assert_eq!(max.mul_f64(2.0), Err(NumError::NotFinite));
    assert_eq!(DD::from_i64(1).div(DD::ZERO), Err(NumError::Degenerate));
    assert_eq!(DD::from_i64(-4).sqrt(), Err(NumError::NegativeSqrt));
    assert_eq!(DD::ZERO.sqrt(), Ok(DD::ZERO));
    assert_eq!(DD::from_f64(f64::NAN), Err(NumError::NotFinite));
    assert_eq!(DD::from_sum(1.0, f64::INFINITY), Err(NumError::NotFinite));
    assert_eq!(DD::from_i128(1i128 << 106), None);
    assert_eq!(DD::exact_product(f64::MIN_POSITIVE / 4.0, 1.0), None);
    assert_eq!(DD::exact_product(0.0, f64::MAX), Some(DD::ZERO));
    assert_eq!(DD::from_i64(i64::MAX).hi(), pow2f(63));
    assert_eq!(DD::from_i64(i64::MAX).lo(), -1.0);
    assert_eq!(DD::from_i64(7).to_f64(), 7.0);
}
