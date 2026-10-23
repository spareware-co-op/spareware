//! Hexadecimal floating-point literals, read at compile time.
//!
//! Rust has no hexadecimal float literal, but the methods this module ports publish their
//! constants in C99's hex-float notation (`0x1.921fb54442d18p+1` is the double nearest π). The
//! macro [`hf!`](hf) reads that notation, so every table here reads digit for digit like the
//! CORE-MATH source it came from and can be checked against it by eye or by `diff`.
//!
//! Method: the grammar is C99's *hexadecimal-floating-constant*, ISO/IEC 9899:1999 §6.4.4.2
//! (WG14 working draft N1256, <https://www.open-std.org/jtc1/sc22/wg14/www/docs/n1256.pdf>), without
//! a suffix. Its value is the significand times two to the binary exponent, as IEEE 754-2019
//! §5.12.3 specifies for hexadecimal significands (<https://doi.org/10.1109/IEEESTD.2019.8766229>);
//! here it must be a binary64 number exactly.
//!
//! Guarantees:
//!
//! - **Evaluated by the compiler, always.** `hf!("…")` expands to an inline `const` block,
//!   `const { parse("…") }`, so every literal is read during compilation, whatever the context: a
//!   `const` item, a table, or an expression in a function body. No string is ever parsed at run
//!   time, and the kernel's machine code holds only the resulting constants.
//! - **Exact or it does not compile.** The literal's value must be a binary64 number exactly: a
//!   significand that needs more than 53 bits (or more than the subnormal range allows), or a value
//!   outside the finite range, fails an assertion in [`parse`] or a function it calls. Because
//!   [`parse`] runs inside a `const` block, that assertion is evaluated by the compiler: a bad
//!   literal is a build error,
//!   never a run-time panic. Kernel code reaches [`parse`] only through the macro; only the unit
//!   tests below call it directly.
//! - **No rounding at all.** The significand is accumulated as an integer, then placed in the bit
//!   pattern; no floating-point operation is involved.
//! - **Checked arithmetic.** Every integer step is a checked method, as in the rest of the crate
//!   (`lib.rs`); the assertions before each step keep it in range, and a step that failed anyway
//!   would fail the assertion in [`step`], again a build error. Signed and unsigned values meet
//!   only through `abs_diff`, `unsigned_abs` and lossless widening, so no conversion loses a value.
//!
//! Grammar accepted: an optional sign, `0x` or `0X`, hexadecimal digits with at most one point
//! (at least one digit in all), then `p` or `P`, an optional sign and decimal digits.

/// The binary64 number written in C99 hexadecimal notation, for example `hf!("-0x1.8p-3")`,
/// evaluated at compile time.
///
/// Expands to `const { parse(literal) }`: the compiler reads the literal, and a malformed or
/// inexact one is a build error (see the module documentation).
macro_rules! hf {
    ($s:literal) => {
        const { $crate::math::hexf::parse($s) }
    };
}
pub(crate) use hf;

/// The value of a hex digit, or 16 for any other byte. Each arm's distance from the digit's base
/// is exact (`abs_diff` of a byte inside the arm's range).
const fn digit_value(c: u8) -> u32 {
    match c {
        b'0'..=b'9' => c.abs_diff(b'0') as u32,
        b'a'..=b'f' => c.abs_diff(b'a' - 10) as u32,
        b'A'..=b'F' => c.abs_diff(b'A' - 10) as u32,
        _ => 16,
    }
}

/// The value of a checked step of [`parse`], `None` if it overflowed. The assertions before every
/// step keep it in range, so `None` never comes; if it did, the assertion here fails, and since
/// [`parse`] runs only inside `const { }` (the macro), that is a build error, never a run-time
/// panic. (`?` and `Option::expect` are not available or allowed here: the first is not `const`,
/// and the crate denies the second.)
const fn step(v: Option<i64>) -> i64 {
    assert!(v.is_some(), "hex float out of range");
    match v {
        Some(v) => v,
        None => 0,
    }
}

/// [`step`] for the 128-bit significand.
const fn step_u128(v: Option<u128>) -> u128 {
    assert!(v.is_some(), "hex float significand out of range");
    match v {
        Some(v) => v,
        None => 0,
    }
}

/// The binary64 number written in C99 hexadecimal notation.
///
/// For compile-time evaluation through [`hf!`](hf) only: see the module documentation. Asserts
/// that the literal is well formed and exactly representable.
pub(crate) const fn parse(s: &str) -> f64 {
    let mut rest = s.as_bytes();

    let negative = matches!(rest, [b'-', ..]);
    if let [b'-' | b'+', tail @ ..] = rest {
        rest = tail;
    }

    assert!(
        matches!(rest, [b'0', b'x' | b'X', ..]),
        "a hex float starts with 0x"
    );
    if let [_, _, tail @ ..] = rest {
        rest = tail;
    }

    let (significand, point_exponent, rest) = significand(rest);
    let exponent = step(point_exponent.checked_add(binary_exponent(rest)));
    let magnitude = magnitude(significand, exponent);
    let bits = if negative {
        magnitude | (1u64 << 63)
    } else {
        magnitude
    };
    f64::from_bits(bits)
}

/// The hexadecimal digits at the start of `rest`, with at most one point: the significand as an
/// integer, the binary exponent the point gives it (−4 for each digit after the point), and what
/// follows. Up to 30 digits (120 bits) are accumulated exactly. Exponents are kept as `i64`, far
/// wider than they need.
const fn significand(rest: &[u8]) -> (u128, i64, &[u8]) {
    let mut rest = rest;
    let mut significand: u128 = 0;
    let mut exponent: i64 = 0;
    let mut digits: i64 = 0;
    let mut seen_point = false;
    loop {
        match rest {
            [b'.', tail @ ..] => {
                assert!(!seen_point, "a hex float has at most one point");
                seen_point = true;
                rest = tail;
            }
            [c, tail @ ..] if digit_value(*c) < 16 => {
                digits = step(digits.checked_add(1));
                assert!(digits <= 30, "too many hex digits");
                // At most 30 digits: below 2^120, so neither step overflows.
                significand = step_u128(match significand.checked_mul(16) {
                    Some(v) => v.checked_add(digit_value(*c) as u128),
                    None => None,
                });
                if seen_point {
                    exponent = step(exponent.checked_sub(4));
                }
                rest = tail;
            }
            _ => break,
        }
    }
    assert!(digits > 0, "a hex float needs at least one digit");
    (significand, exponent, rest)
}

/// The binary exponent that ends the literal: `p` or `P`, an optional sign, then up to 5 decimal
/// digits, and nothing after them.
const fn binary_exponent(rest: &[u8]) -> i64 {
    let mut rest = rest;
    assert!(
        matches!(rest, [b'p' | b'P', ..]),
        "a hex float needs a binary exponent"
    );
    if let [_, tail @ ..] = rest {
        rest = tail;
    }
    let negative = matches!(rest, [b'-', ..]);
    if let [b'-' | b'+', tail @ ..] = rest {
        rest = tail;
    }
    let mut written: i64 = 0;
    let mut digits: i64 = 0;
    while let [c @ b'0'..=b'9', tail @ ..] = rest {
        digits = step(digits.checked_add(1));
        assert!(digits <= 5, "exponent too long");
        // At most 5 decimal digits: below 10^5, so neither step overflows.
        written = step(match written.checked_mul(10) {
            Some(v) => v.checked_add(c.abs_diff(b'0') as i64),
            None => None,
        });
        rest = tail;
    }
    assert!(digits > 0, "the binary exponent needs digits");
    assert!(rest.is_empty(), "unexpected characters after the exponent");
    step(if negative {
        written.checked_neg()
    } else {
        Some(written)
    })
}

/// The bits of `significand · 2^exponent` without its sign, which must be a binary64 number
/// exactly.
const fn magnitude(significand: u128, exponent: i64) -> u64 {
    if significand == 0 {
        return 0;
    }
    // 2^top ≤ significand < 2^(top+1): 127 − leading zeros, 0 to 127.
    let top = step(127i64.checked_sub(significand.leading_zeros() as i64));
    let unbiased = step(exponent.checked_add(top)); // 2^unbiased ≤ value < 2^(unbiased+1)
    assert!(unbiased <= 1023, "hex float overflows binary64");
    // Bits of the significand below this weight must be zero: 2^(unbiased − 52) for a normal
    // result, 2^−1074 for a subnormal one.
    let lsb = if unbiased >= -1022 {
        step(unbiased.checked_sub(52))
    } else {
        -1074
    };
    // How many low bits of `significand` fall below the last place.
    let drop = step(lsb.checked_sub(exponent));
    let kept = if drop > 0 {
        assert!(drop < 128, "hex float underflows binary64");
        // The low `drop` bits, below 128 of them, as a mask.
        let lost = significand & !(u128::MAX << drop);
        assert!(lost == 0, "hex float is not exactly a binary64 number");
        significand >> drop
    } else {
        // The leading bit moves up to 2^52 at most: by fewer than 128 places.
        assert!(drop > -128, "hex float significand too short");
        significand << drop.unsigned_abs()
    };
    // kept < 2^53: its leading bit is at 2^52 for a normal result, below for a subnormal one. So
    // its low 64 bits, read from its bytes, are all of it.
    assert!(kept >> 53 == 0, "hex float significand misplaced");
    let [k0, k1, k2, k3, k4, k5, k6, k7, ..] = kept.to_le_bytes();
    let kept = u64::from_le_bytes([k0, k1, k2, k3, k4, k5, k6, k7]);
    if unbiased >= -1022 {
        // kept has its leading bit at 2^52: replace it by the biased exponent, unbiased + 1023,
        // which is 1 to 2046 here (abs_diff from −1023 is exact).
        (unbiased.abs_diff(-1023) << 52) | (kept & ((1u64 << 52) - 1))
    } else {
        kept // subnormal: the significand in units of 2^−1074, exponent field 0
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    // `hf!` is in textual scope: it is defined above, in this file.
    use super::parse;

    // In const items, as in the kernel tables.
    const PI: f64 = hf!("0x1.921fb54442d18p+1");
    const NEG_ZERO: f64 = hf!("-0x0p+0");
    const TINY: f64 = hf!("0x1p-1074");
    const SUB: f64 = hf!("0x0.8p-1022");
    const MAX: f64 = hf!("0x1.fffffffffffffp+1023");
    const MIN_NORMAL: f64 = hf!("0x1p-1022");
    const UNNORMALISED: f64 = hf!("0xc.90fdaa22168cp-2");
    const SHORT: f64 = hf!("0x1p+0");
    const UPPER: f64 = hf!("0X1.8P-1");
    const LEADING_POINT: f64 = hf!("0x.8p1");

    #[test]
    fn reads_c99_hex_floats_exactly() {
        assert_eq!(PI, core::f64::consts::PI);
        assert_eq!(NEG_ZERO.to_bits(), (-0.0f64).to_bits());
        assert_eq!(TINY.to_bits(), 1);
        assert_eq!(SUB.to_bits(), 1u64 << 51);
        assert_eq!(MAX, f64::MAX);
        assert_eq!(MIN_NORMAL, f64::MIN_POSITIVE);
        assert_eq!(UNNORMALISED, core::f64::consts::PI);
        assert_eq!(SHORT, 1.0);
        assert_eq!(UPPER, 0.75);
        assert_eq!(LEADING_POINT, 1.0);
        // The macro in an expression is still a compile-time constant.
        assert_eq!(hf!("-0x1.5555555555555p-3"), -1.0 / 6.0);
        // The reader called at run time, which only these tests do.
        assert_eq!(parse("-0x1.5555555555555p-3"), -1.0 / 6.0);
        assert_eq!(parse("0x1.0p0"), 1.0);
    }
}
