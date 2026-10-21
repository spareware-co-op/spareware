//! The only place maths functions come from.
//!
//! The standard library's `sin`, `cos`, `exp` and friends have platform-dependent precision (the Rust
//! documentation calls it non-deterministic), so the same model would give different bits on
//! Windows, macOS and in the browser. Every maths function the kernel uses comes from here instead,
//! from the pure-Rust `libm` crate (MIT OR Apache-2.0): the same code, the same bits, on every target
//! (chapter 21). Square root is correctly rounded by IEEE 754 and identical everywhere.
//!
//! `sinpi` and `cospi` take angles in half-turns and are exact at multiples of a quarter turn, so a
//! rotation by 90 degrees gives exactly 0 and ±1 rather than a tiny rounding residue.
//!
//! A later piece moves the transcendental functions to correctly rounded CORE-MATH ports, which are
//! identical by specification; callers will not change.

/// Square root, correctly rounded.
#[must_use]
pub fn sqrt(x: f64) -> f64 {
    libm::sqrt(x)
}

/// Sine of an angle in radians.
#[must_use]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Cosine of an angle in radians.
#[must_use]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// Tangent of an angle in radians.
#[must_use]
pub fn tan(x: f64) -> f64 {
    libm::tan(x)
}

/// Arcsine, in radians.
#[must_use]
pub fn asin(x: f64) -> f64 {
    libm::asin(x)
}

/// Arccosine, in radians.
#[must_use]
pub fn acos(x: f64) -> f64 {
    libm::acos(x)
}

/// Arctangent, in radians.
#[must_use]
pub fn atan(x: f64) -> f64 {
    libm::atan(x)
}

/// The angle of the point (x, y) from the x-axis, in radians, in (−π, π].
#[must_use]
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

/// e to the power x.
#[must_use]
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// Natural logarithm.
#[must_use]
pub fn ln(x: f64) -> f64 {
    libm::log(x)
}

/// Base-2 logarithm.
#[must_use]
pub fn log2(x: f64) -> f64 {
    libm::log2(x)
}

/// Base-10 logarithm.
#[must_use]
pub fn log10(x: f64) -> f64 {
    libm::log10(x)
}

/// x to the power y.
#[must_use]
pub fn pow(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

/// The length of (x, y), without undue overflow.
#[must_use]
pub fn hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

/// Cube root.
#[must_use]
pub fn cbrt(x: f64) -> f64 {
    libm::cbrt(x)
}

/// Absolute value.
#[must_use]
pub fn abs(x: f64) -> f64 {
    libm::fabs(x)
}

/// Largest whole number not above x.
#[must_use]
pub fn floor(x: f64) -> f64 {
    libm::floor(x)
}

/// Smallest whole number not below x.
#[must_use]
pub fn ceil(x: f64) -> f64 {
    libm::ceil(x)
}

/// Nearest whole number, ties to even.
#[must_use]
pub fn round_ties_even(x: f64) -> f64 {
    libm::rint(x)
}

/// The angle x half-turns reduced exactly to the range [−1, 1] half-turns (that is, [−π, π]).
///
/// Exact: `x * 0.5` and the rounding are exact, and `x − 2n` is exact because it is the difference of
/// two numbers within a factor of two of each other, or `n` is zero (Sterbenz's lemma).
fn reduce_half_turns(x: f64) -> f64 {
    x - 2.0 * libm::rint(x * 0.5)
}

/// Sine of an angle given in half-turns: `sinpi(x) = sin(πx)`. Exact (0 or ±1) at every multiple of a
/// quarter turn (`x` a multiple of 0.5). Returns +0 rather than −0 at those points, so results are the
/// same bits whatever the input's history.
#[must_use]
pub fn sinpi(x: f64) -> f64 {
    let r = reduce_half_turns(x);
    if r == 0.0 || r == 1.0 || r == -1.0 {
        0.0
    } else if r == 0.5 {
        1.0
    } else if r == -0.5 {
        -1.0
    } else {
        libm::sin(core::f64::consts::PI * r)
    }
}

/// Cosine of an angle given in half-turns: `cospi(x) = cos(πx)`. Exact (0 or ±1) at every multiple of
/// a quarter turn.
#[must_use]
pub fn cospi(x: f64) -> f64 {
    let r = reduce_half_turns(x);
    if r == 0.0 {
        1.0
    } else if r == 1.0 || r == -1.0 {
        -1.0
    } else if r == 0.5 || r == -0.5 {
        0.0
    } else {
        libm::cos(core::f64::consts::PI * r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_turns_are_exact() {
        for (x, s, c) in [
            (0.0, 0.0, 1.0),
            (0.5, 1.0, 0.0),
            (1.0, 0.0, -1.0),
            (1.5, -1.0, 0.0),
            (2.0, 0.0, 1.0),
            (-0.5, -1.0, 0.0),
            (-1.0, 0.0, -1.0),
            (1_000_000.5, 1.0, 0.0),
            (-1_000_001.0, 0.0, -1.0),
        ] {
            assert_eq!(sinpi(x).to_bits(), f64::to_bits(s), "sinpi({x})");
            assert_eq!(cospi(x).to_bits(), f64::to_bits(c), "cospi({x})");
        }
    }

    #[test]
    fn other_angles_are_close() {
        let x = 1.0 / 6.0;
        assert!((sinpi(x) - 0.5).abs() < 1e-15);
        assert!((cospi(x) - sqrt(3.0) / 2.0).abs() < 1e-15);
    }
}
