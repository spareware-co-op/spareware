//! Checks of the test oracle itself (`math_reference.rs`): its constants against published
//! digits, its values against independently known correctly rounded values, its rounding to
//! binary64 at ties, subnormals and overflow, and a rough cross-check against `libm`.

// Test code may use native arithmetic and casts (Cargo.toml).
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]

#[path = "math_reference.rs"]
mod reference;

use core::cmp::Ordering;

use num_bigint::BigInt;
use reference::*;

/// The oracle's π and ln 2 contain their published digits.
#[test]
fn constants_match_published_digits() {
    // π: 3.243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89 (hex), the digits
    // that Blowfish's P-array is made of.
    let pi_hex = "3243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89";
    let m = BigInt::parse_bytes(pi_hex.as_bytes(), 16).unwrap();
    let low = Dy {
        m: m.clone(),
        e: -4 * (pi_hex.len() as i64 - 1),
    };
    let high = Dy { m: m + 1, e: low.e };
    assert!(pi().lo.cmp(&low) != Ordering::Less && pi().hi.cmp(&high) != Ordering::Greater);
    // ln 2 = 0.B17217F7D1CF79ABC9E3B39803F2F6AF40F343267298B62D8A0D175B8BAAFA2B... (hex)
    let ln2_hex = "B17217F7D1CF79ABC9E3B39803F2F6AF40F343267298B62D";
    let m = BigInt::parse_bytes(ln2_hex.as_bytes(), 16).unwrap();
    let low = Dy {
        m: m.clone(),
        e: -4 * ln2_hex.len() as i64,
    };
    let high = Dy { m: m + 1, e: low.e };
    assert!(ln2().lo.cmp(&low) != Ordering::Less && ln2().hi.cmp(&high) != Ordering::Greater);
    // The cached intervals are about as tight as their precision.
    let w = pi().hi.sub(&pi().lo);
    assert!(w.is_zero() || w.exponent() < -(CONST_BITS as i64) + 8);
    assert_ne!(
        atan_eighths(8).lo.cmp(&pi().scale(-2).hi),
        Ordering::Greater
    );
    assert_ne!(atan_eighths(8).hi.cmp(&pi().scale(-2).lo), Ordering::Less);
}

/// Correctly rounded values known independently: Rust's own constants are the doubles nearest
/// the mathematical constants.
#[test]
fn known_values() {
    use core::f64::consts::*;
    assert_same(exp(1.0), E, "exp(1)");
    assert_same(ln(2.0), LN_2, "ln(2)");
    assert_same(ln(10.0), LN_10, "ln(10)");
    assert_same(atan2(1.0, 1.0), FRAC_PI_4, "atan2(1, 1)");
    assert_same(
        atan2(1.0, -1.0),
        hx("0x1.2d97c7f3321d2p+1"),
        "atan2(1, −1) = 3π/4",
    );
    assert_same(3.0 * FRAC_PI_4, hx("0x1.2d97c7f3321d2p+1"), "3π/4 rounded");
    assert_same(sinpi(0.25), FRAC_1_SQRT_2, "sinpi(1/4)");
    assert_same(cospi(0.25), FRAC_1_SQRT_2, "cospi(1/4)");
    assert_same(sin(1.0), hx("0x1.aed548f090ceep-1"), "sin(1)");
    assert_same(cos(1.0), hx("0x1.14a280fb5068cp-1"), "cos(1)");
    assert_same(
        sin(core::f64::consts::PI),
        hx("0x1.1a62633145c07p-53"),
        "sin(π rounded)",
    );
    assert_same(exp(-746.0), 0.0, "exp underflow");
    assert_same(exp(710.0), f64::INFINITY, "exp overflow");
    assert_same(
        sinpi(1.0 / 3.0),
        libm::sqrt(3.0) / 2.0,
        "sinpi(1/3) vs √3/2",
    );
}

/// The oracle agrees with an independent implementation to within an ulp on random inputs
/// (a check for gross mistakes such as a wrong sign or quadrant, not of the last bit).
#[test]
fn agrees_with_libm_roughly() {
    let mut rng = Rng(0x5eed_0001);
    for _ in 0..300 {
        let x = rng.uniform(-40.0, 40.0);
        assert!(ulps(sin(x), libm::sin(x)) <= 1, "sin({x})");
        assert!(ulps(cos(x), libm::cos(x)) <= 1, "cos({x})");
        assert!(ulps(exp(x), libm::exp(x)) <= 1, "exp({x})");
        let y = rng.uniform(-40.0, 40.0);
        assert!(ulps(atan2(y, x), libm::atan2(y, x)) <= 1, "atan2({y}, {x})");
        let z = rng.log_uniform(-1074 + 52, 1023).abs();
        assert!(ulps(ln(z), libm::log(z)) <= 1, "ln({z})");
        let w = rng.uniform(-4.0, 4.0);
        let pw = core::f64::consts::PI * w;
        assert!((sinpi(w) - libm::sin(pw)).abs() <= 1e-14, "sinpi({w})");
        assert!((cospi(w) - libm::cos(pw)).abs() <= 1e-14, "cospi({w})");
    }
}

#[test]
fn rounding_to_double_handles_ties_subnormals_and_overflow() {
    let one = Dy::int(1);
    let tie = one.add(&Dy::pow2(-53)); // halfway between 1 and 1 + 2^−52: ties to even (1)
    assert_same(tie.to_f64(), 1.0, "tie down");
    let tie_up = Dy::int(1).add(&Dy::pow2(-52)).add(&Dy::pow2(-53));
    assert_same(tie_up.to_f64(), 1.0 + 2.0 * f64::EPSILON, "tie up to even");
    assert_same(
        Dy::pow2(-1075).to_f64(),
        0.0,
        "half the smallest subnormal ties to 0",
    );
    assert_same(
        Dy::pow2(-1075).add(&Dy::pow2(-1200)).to_f64(),
        f64::from_bits(1),
        "above",
    );
    assert_same(
        Dy::pow2(-1075).neg().to_f64(),
        -0.0,
        "negative tiny keeps its sign",
    );
    assert_same(Dy::pow2(1024).to_f64(), f64::INFINITY, "overflow");
    let max = Dy::from_f64(f64::MAX);
    assert_same(
        max.add(&Dy::pow2(969)).to_f64(),
        f64::MAX,
        "below the overflow threshold",
    );
    // MAX + ulp/2 is a tie between MAX (odd significand) and 2^1024: to even, so overflow.
    assert_same(
        max.add(&Dy::pow2(970)).to_f64(),
        f64::INFINITY,
        "tie at the threshold",
    );
    assert_same(
        hx("0x0.fffffffffffffp-1022"),
        f64::from_bits(0x000f_ffff_ffff_ffff),
        "hx sub",
    );
    assert_same(hx("-0x0p+0"), -0.0, "hx −0");
    for &v in &[
        1.0,
        -2.5,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::MAX,
        -1e-300,
    ] {
        assert_same(Dy::from_f64(v).to_f64(), v, "round trip");
    }
}
