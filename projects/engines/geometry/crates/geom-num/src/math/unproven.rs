//! Deterministic maths functions **without** a proven error bound.
//!
//! Every function here comes from the pure-Rust `libm` crate 0.2.16 (MIT OR Apache-2.0,
//! <https://github.com/rust-lang/compiler-builtins/tree/master/libm>), mostly ports of FreeBSD's
//! msun (<https://github.com/freebsd/freebsd-src/tree/main/lib/msun>), the methods of Sun's fdlibm.
//! They are deterministic: the same code gives the same bits on every target and thread count
//! (chapter 21), unlike the standard library's, whose precision the Rust documentation calls
//! non-deterministic. But their error bounds are documented, not proven (evidence note 05, section
//! 7; chapter 16.2), so they must never feed a certified result.
//!
//! They keep the guarantees of [`super`]: each returns a [`Real`] or an error, never NaN or an
//! infinity. [`NumError::NotFinite`] for a NaN or infinite argument (checked first) or an infinite
//! result (an overflow, or a pole such as `log2(0)`); [`NumError::OutsideDomain`] where the function
//! has no real value, checked before `libm` is called, so `libm`'s NaN never appears.
//!
//! Use the proven functions in [`super`] wherever one exists. Use one of these only where no
//! proven function exists yet and the caller has asked for an approximate result: chapter 12.3's
//! rule makes an operation that rests on an unproven function fail with `UnprovenMaths` unless
//! asked, and label everything that rests on it Approximate, its deviation measured against a
//! correctly rounded reference rather than proven.
//!
//! Names match the proven functions where both exist (`atan2`, `exp`, `ln`, `sinpi`, `cospi`), so a
//! call site's path, `math::unproven::…` or `math::…`, says which kind it uses. There is no proven
//! radian `sin` or `cos` yet: give angles in half-turns and use the proven `sinpi` and `cospi`.

use super::{finite, log_argument};
use crate::error::NumError;
use crate::real::{Real, zero_to_plus};

/// Sine of an angle in radians. Deterministic, error bound not proven. For a proven result give
/// the angle in half-turns and use [`super::sinpi`].
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn sin(x: f64) -> Result<Real, NumError> {
    Real::new(libm::sin(finite(x)?))
}

/// Cosine of an angle in radians. Deterministic, error bound not proven. For a proven result give
/// the angle in half-turns and use [`super::cospi`].
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn cos(x: f64) -> Result<Real, NumError> {
    Real::new(libm::cos(finite(x)?))
}

/// Tangent of an angle in radians. Deterministic, error bound not proven. Finite for every finite
/// argument: no double is close enough to an odd multiple of π/2 for the tangent to overflow.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument (and, should `libm` ever overflow, for
/// that result too).
pub fn tan(x: f64) -> Result<Real, NumError> {
    Real::new(libm::tan(finite(x)?))
}

/// Arcsine, in radians, in [−π/2, π/2]. Deterministic, error bound not proven.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument; [`NumError::OutsideDomain`] for
/// `|x| > 1`.
pub fn asin(x: f64) -> Result<Real, NumError> {
    let x = finite(x)?;
    if !(-1.0..=1.0).contains(&x) {
        return Err(NumError::OutsideDomain);
    }
    Real::new(libm::asin(x))
}

/// Arccosine, in radians, in [0, π]. Deterministic, error bound not proven.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument; [`NumError::OutsideDomain`] for
/// `|x| > 1`.
pub fn acos(x: f64) -> Result<Real, NumError> {
    let x = finite(x)?;
    if !(-1.0..=1.0).contains(&x) {
        return Err(NumError::OutsideDomain);
    }
    Real::new(libm::acos(x))
}

/// Arctangent, in radians, in [−π/2, π/2]. Deterministic, error bound not proven; for an angle
/// from a point use the proven [`super::atan2()`].
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn atan(x: f64) -> Result<Real, NumError> {
    Real::new(libm::atan(finite(x)?))
}

/// The angle of the point (x, y) from the x-axis, in radians, in [−π, π], with the signs of zero
/// of [`super::atan2()`]. Deterministic, error bound not proven; prefer [`super::atan2()`].
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn atan2(y: f64, x: f64) -> Result<Real, NumError> {
    Real::new(libm::atan2(finite(y)?, finite(x)?))
}

/// e to the power x. Deterministic, error bound not proven; prefer [`super::exp()`].
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or when e^x overflows (x above about
/// 709.78).
pub fn exp(x: f64) -> Result<Real, NumError> {
    Real::new(libm::exp(finite(x)?))
}

/// Natural logarithm. Deterministic, error bound not proven; prefer [`super::ln`].
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or for ±0, whose logarithm is −∞;
/// [`NumError::OutsideDomain`] for `x < 0`.
pub fn ln(x: f64) -> Result<Real, NumError> {
    Real::new(libm::log(log_argument(x)?))
}

/// Base-2 logarithm. Deterministic, error bound not proven.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or for ±0, whose logarithm is −∞;
/// [`NumError::OutsideDomain`] for `x < 0`.
pub fn log2(x: f64) -> Result<Real, NumError> {
    Real::new(libm::log2(log_argument(x)?))
}

/// Base-10 logarithm. Deterministic, error bound not proven.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or for ±0, whose logarithm is −∞;
/// [`NumError::OutsideDomain`] for `x < 0`.
pub fn log10(x: f64) -> Result<Real, NumError> {
    Real::new(libm::log10(log_argument(x)?))
}

/// x to the power y. Deterministic, error bound not proven.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or when the power is infinite: an
/// overflow, or zero to a negative power. [`NumError::OutsideDomain`] for `x < 0` with a `y` that
/// is not a whole number, where the power has no real value.
pub fn pow(x: f64, y: f64) -> Result<Real, NumError> {
    let (x, y) = (finite(x)?, finite(y)?);
    if x < 0.0 && libm::trunc(y) != y {
        return Err(NumError::OutsideDomain);
    }
    Real::new(libm::pow(x, y))
}

/// x to the whole power n. Deterministic, error bound not proven.
///
/// Computed as `pow(x, n)`, with `n` converted to a double exactly (every `i32` is one), so it has
/// `pow`'s accuracy. Not by repeated multiplication, whose rounding error grows with the number of
/// products; and not `f64::powi`, whose result differs by platform (chapter 21).
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite `x`, or when the power is infinite: an overflow,
/// or zero to a negative power.
pub fn powi(x: f64, n: i32) -> Result<Real, NumError> {
    pow(x, f64::from(n))
}

/// The length of (x, y), without undue overflow. Deterministic; `libm` documents its `hypot` as a
/// CORE-MATH port, but this crate has not verified that claim, so the bound is not relied on.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or when the length overflows.
pub fn hypot(x: f64, y: f64) -> Result<Real, NumError> {
    Real::new(libm::hypot(finite(x)?, finite(y)?))
}

/// Cube root. Deterministic; `libm` documents its `cbrt` as a CORE-MATH port, but this crate has
/// not verified that claim, so the bound is not relied on.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn cbrt(x: f64) -> Result<Real, NumError> {
    Real::new(libm::cbrt(finite(x)?))
}

/// The finite angle x half-turns reduced exactly to the range [−1, 1] half-turns (that is,
/// [−π, π]).
///
/// Exact: `x * 0.5` and the rounding are exact (the halving is exact unless x is subnormal, and
/// then |x/2| < 1/2 either way, so the nearest whole number is 0), and `x − 2n` is exact because it
/// is the difference of two numbers within a factor of two of each other, or `n` is zero
/// (Sterbenz's lemma).
fn reduce_half_turns(x: f64) -> f64 {
    x - 2.0 * libm::rint(x * 0.5)
}

/// Sine of an angle given in half-turns, `sin(πx)`, by `libm::sin(π·r)` after an exact reduction.
/// Exact (0 or ±1) at every multiple of a quarter turn, where a zero is +0. Deterministic, error
/// bound not proven; prefer the correctly rounded [`super::sinpi`].
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn sinpi(x: f64) -> Result<Real, NumError> {
    let r = reduce_half_turns(finite(x)?);
    let s = if r == 0.0 || r == 1.0 || r == -1.0 {
        0.0
    } else if r == 0.5 {
        1.0
    } else if r == -0.5 {
        -1.0
    } else {
        libm::sin(core::f64::consts::PI * r)
    };
    Real::new(zero_to_plus(s))
}

/// Cosine of an angle given in half-turns, `cos(πx)`, by `libm::cos(π·r)` after an exact
/// reduction. Exact (0 or ±1) at every multiple of a quarter turn, where a zero is +0.
/// Deterministic, error bound not proven; prefer the correctly rounded [`super::cospi`].
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn cospi(x: f64) -> Result<Real, NumError> {
    let r = reduce_half_turns(finite(x)?);
    let c = if r == 0.0 {
        1.0
    } else if r == 1.0 || r == -1.0 {
        -1.0
    } else if r == 0.5 || r == -0.5 {
        0.0
    } else {
        libm::cos(core::f64::consts::PI * r)
    };
    Real::new(zero_to_plus(c))
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
    use super::*;

    /// The value, for a result that must exist.
    fn ok(r: Result<Real, NumError>) -> f64 {
        r.unwrap().get()
    }

    #[test]
    fn quarter_turns_are_exact() {
        for (x, s, c) in [
            (0.0, 0.0, 1.0),
            (-0.0, 0.0, 1.0),
            (0.5, 1.0, 0.0),
            (1.0, 0.0, -1.0),
            (1.5, -1.0, 0.0),
            (2.0, 0.0, 1.0),
            (-0.5, -1.0, 0.0),
            (-1.0, 0.0, -1.0),
            (1_000_000.5, 1.0, 0.0),
            (-1_000_001.0, 0.0, -1.0),
        ] {
            assert_eq!(ok(sinpi(x)).to_bits(), f64::to_bits(s), "sinpi({x})");
            assert_eq!(ok(cospi(x)).to_bits(), f64::to_bits(c), "cospi({x})");
        }
    }

    #[test]
    fn other_angles_are_close() {
        let x = 1.0 / 6.0;
        assert!((ok(sinpi(x)) - 0.5).abs() < 1e-15);
        assert!((ok(cospi(x)) - libm::sqrt(3.0) / 2.0).abs() < 1e-15);
    }
}
