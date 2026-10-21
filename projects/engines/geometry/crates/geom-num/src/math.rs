//! The only place maths functions come from.
//!
//! The standard library's `sin`, `cos`, `exp`, `powi` and friends have platform-dependent precision
//! (the Rust documentation calls it non-deterministic), so the same model would give different
//! bits on Windows, macOS and in the browser. Every maths function the kernel uses comes from here
//! instead, from the pure-Rust `libm` crate (MIT): the same code, the same bits, on
//! every target (chapter 21). Where `libm` uses a processor instruction instead of its own code
//! (the square root, and the fused multiply-add on processors that have one), the instruction is
//! one IEEE 754 defines exactly, correctly rounded, so it gives the same bits as the portable code.
//!
//! Guarantees, for every function here and every argument:
//!
//! - **A finite result or an error, never NaN or an infinity.** Each function returns a
//!   [`Real`], or:
//!   - [`NumError::NotFinite`] when an argument is NaN or infinite (checked first), or the result
//!     would be infinite: an overflow, such as `exp(1000)`, or a pole, such as `ln(0)`;
//!   - [`NumError::OutsideDomain`] when the function has no real value there: the square root or
//!     a logarithm of a negative number, the arcsine or arccosine of a number beyond ±1, or a
//!     negative number to a power that is not a whole number. The domain is checked before `libm`
//!     is called, so `libm`'s NaN never appears.
//! - **Accuracy.** The square root and the fused multiply-add are correctly rounded (IEEE 754);
//!   `abs`, `floor`, `ceil` and `round_ties_even` are exact. The others are `libm`'s, which are
//!   accurate to about one unit in the last place but not proven to be; they move to correctly
//!   rounded CORE-MATH ports with proven bounds (build piece G47), and callers will not change.
//!   An enclosure that needs one of them is refused with [`NumError::UnprovenMaths`] until then
//!   ([`crate::interval`]).
//!
//! `sinpi` and `cospi` take angles in half-turns. They reduce the angle exactly before `libm` sees
//! it, so they are exact (0 or ±1) at every multiple of a quarter turn and keep their relative
//! accuracy next to those multiples, where rotations by a near quarter turn need it.
//!
//! Method: `libm` 0.2.16 (MIT, a pure-Rust implementation of the C maths library,
//! <https://docs.rs/libm/0.2.16>), behind the argument and domain checks above; `sinpi`
//! and `cospi` follow IEEE 754-2019's `sinPi` and `cosPi` (section 9.2,
//! <https://doi.org/10.1109/IEEESTD.2019.8766229>) with the reduction each function's
//! documentation derives.
//!
//! Cost: constant per call and no allocation: the argument checks and a few `libm` calls, each a
//! bounded computation.
//!
//! Consulted: FreeBSD `msun` `s_sinpi.c`, BSD-2-Clause,
//!   <https://github.com/freebsd/freebsd-src/blob/c6d7a2478c9eeb38ed1284e4ee1ff64c75ca8b09/lib/msun/src/s_sinpi.c>,
//!   for its reduction of the angle to a remainder in half-turns and a sign before its kernel,
//!   which [`sinpi`] follows. Method only; no code copied.
//! Consulted: FreeBSD `msun` `s_cospi.c`, BSD-2-Clause,
//!   <https://github.com/freebsd/freebsd-src/blob/c6d7a2478c9eeb38ed1284e4ee1ff64c75ca8b09/lib/msun/src/s_cospi.c>,
//!   for the same reduction in [`cospi`]. Method only; no code copied.

use crate::error::NumError;
use crate::real::{Real, zero_to_plus};

/// The argument itself, or `NotFinite` if it is NaN or infinite.
const fn finite(x: f64) -> Result<f64, NumError> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(NumError::NotFinite)
    }
}

/// Square root, correctly rounded. `sqrt(−0)` is −0, as IEEE 754 specifies.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument; [`NumError::OutsideDomain`] for
/// `x < 0`.
pub fn sqrt(x: f64) -> Result<Real, NumError> {
    if finite(x)? < 0.0 {
        return Err(NumError::OutsideDomain);
    }
    Real::new(libm::sqrt(x))
}

/// Sine of an angle in radians.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn sin(x: f64) -> Result<Real, NumError> {
    Real::new(libm::sin(finite(x)?))
}

/// Cosine of an angle in radians.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn cos(x: f64) -> Result<Real, NumError> {
    Real::new(libm::cos(finite(x)?))
}

/// Tangent of an angle in radians. Finite for every finite argument: no double is close enough to
/// an odd multiple of π/2 for the tangent to overflow.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument (and, should `libm` ever overflow, for
/// that result too).
pub fn tan(x: f64) -> Result<Real, NumError> {
    Real::new(libm::tan(finite(x)?))
}

/// Arcsine, in radians, in [−π/2, π/2].
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

/// Arccosine, in radians, in [0, π].
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

/// Arctangent, in radians, in [−π/2, π/2].
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn atan(x: f64) -> Result<Real, NumError> {
    Real::new(libm::atan(finite(x)?))
}

/// The angle of the point (x, y) from the x-axis, in radians, in [−π, π].
///
/// A zero `y` keeps its sign in the result, as IEEE 754's `atan2` specifies: (±0, x) gives ±0 for
/// x > 0 or x = +0, and ±π for x < 0 or x = −0. So −π is returned only for y = −0, and (0, 0)
/// gives a zero or ±π by the signs of its zeros rather than failing.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn atan2(y: f64, x: f64) -> Result<Real, NumError> {
    Real::new(libm::atan2(finite(y)?, finite(x)?))
}

/// e to the power x.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or when e^x overflows (x above about
/// 709.78).
pub fn exp(x: f64) -> Result<Real, NumError> {
    Real::new(libm::exp(finite(x)?))
}

/// The domain of the logarithms: `x` itself when it is finite and not negative.
fn log_argument(x: f64) -> Result<f64, NumError> {
    let x = finite(x)?;
    if x < 0.0 {
        Err(NumError::OutsideDomain)
    } else {
        Ok(x)
    }
}

/// Natural logarithm.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or for ±0, whose logarithm is −∞;
/// [`NumError::OutsideDomain`] for `x < 0`.
pub fn ln(x: f64) -> Result<Real, NumError> {
    Real::new(libm::log(log_argument(x)?))
}

/// Base-2 logarithm.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or for ±0, whose logarithm is −∞;
/// [`NumError::OutsideDomain`] for `x < 0`.
pub fn log2(x: f64) -> Result<Real, NumError> {
    Real::new(libm::log2(log_argument(x)?))
}

/// Base-10 logarithm.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or for ±0, whose logarithm is −∞;
/// [`NumError::OutsideDomain`] for `x < 0`.
pub fn log10(x: f64) -> Result<Real, NumError> {
    Real::new(libm::log10(log_argument(x)?))
}

/// x to the power y.
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

/// x to the whole power n.
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

/// The fused multiply-add a · b + c, rounded once, as IEEE 754 specifies: correctly rounded, so
/// the same bits everywhere. Rust never fuses on its own (RFC 3514), so this is the one way the
/// kernel writes one (chapter 21: only when written explicitly).
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or when the result overflows.
pub fn fma(a: f64, b: f64, c: f64) -> Result<Real, NumError> {
    Real::new(libm::fma(finite(a)?, finite(b)?, finite(c)?))
}

/// The length of (x, y), without undue overflow.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or when the length overflows.
pub fn hypot(x: f64, y: f64) -> Result<Real, NumError> {
    Real::new(libm::hypot(finite(x)?, finite(y)?))
}

/// Cube root.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn cbrt(x: f64) -> Result<Real, NumError> {
    Real::new(libm::cbrt(finite(x)?))
}

/// Absolute value. Exact.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn abs(x: f64) -> Result<Real, NumError> {
    Real::new(libm::fabs(finite(x)?))
}

/// Largest whole number not above x. Exact.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn floor(x: f64) -> Result<Real, NumError> {
    Real::new(libm::floor(finite(x)?))
}

/// Smallest whole number not below x. Exact.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn ceil(x: f64) -> Result<Real, NumError> {
    Real::new(libm::ceil(finite(x)?))
}

/// Nearest whole number, ties to even. Exact.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn round_ties_even(x: f64) -> Result<Real, NumError> {
    Real::new(libm::rint(finite(x)?))
}

/// The finite angle `x` half-turns reduced exactly to r in [−1, 1] half-turns (that is, [−π, π]),
/// with x − r a whole number of full turns.
///
/// Exact: n = `rint(x · 0.5)` is a whole number with |x/2 − n| ≤ 1/2, so |x − 2n| ≤ 1. The halving
/// is exact unless x is subnormal, and then |x/2| < 1/2 either way, so n = 0. `2n` is exact (a
/// doubling), and `x − 2n` is exact: for n = 0 it is x, and otherwise x and 2n are within a factor
/// of two of each other (x lies in [2n − 1, 2n + 1] and |2n| ≥ 2), so Sterbenz's lemma applies.
fn reduce_half_turns(x: f64) -> f64 {
    x - 2.0 * libm::rint(x * 0.5)
}

/// sin(πt) for |t| ≤ 1/4 and its companion cos(πt): the two `libm` calls everything below reduces
/// to. The argument π·t is rounded once.
fn sin_pi_small(t: f64) -> f64 {
    libm::sin(core::f64::consts::PI * t)
}

/// cos(πt) for |t| ≤ 1/4.
fn cos_pi_small(t: f64) -> f64 {
    libm::cos(core::f64::consts::PI * t)
}

/// Sine of an angle given in half-turns: `sinpi(x) = sin(πx)`. Exact (0 or ±1) at every multiple
/// of a quarter turn (`x` a multiple of 0.5), where a zero is +0, so results are the same bits
/// whatever the input's history.
///
/// Method: IEEE 754-2019's `sinPi` (section 9.2), with the reduction FreeBSD's `msun` uses for its
/// `sinpi` (BSD-2-Clause; method only). The angle is reduced to r in [−1, 1] half-turns
/// (`reduce_half_turns`, exact), then by sin(−r) = −sin(r) to a = |r| in [0, 1], then by one
/// exact subtraction to |t| ≤ 1/4:
///
/// - a ≤ 1/4: sin(πa);
/// - 1/4 < a ≤ 3/4: sin(πa) = cos(π(a − 1/2)), and a − 1/2 is exact by Sterbenz's lemma
///   (1/4 ≤ a ≤ 1);
/// - a > 3/4: sin(πa) = sin(π(1 − a)), and 1 − a is exact by Sterbenz's lemma (1/2 ≤ a ≤ 1).
///
/// Why: multiplying a reduced angle near ±1 or ±1/2 by π first and reducing afterwards, as `libm`
/// would, loses the small difference that is the whole result: sin(π(1 − 2^−40)) is about
/// 2.9 · 10^−12, and the error of π·(1 − 2^−40) alone is about 10^−16, a relative error of
/// 4 · 10^−5. Reduced first, the difference is exact, and the only errors left are relative ones:
/// the argument π·t, rounded once with π itself rounded, is within (2^−53 + 2^−54) relative of πt,
/// which moves sin(πt) by at most that relative amount (θ·cot θ ≤ 1) and cos(πt) by less
/// (θ·tan θ ≤ π/4 for θ ≤ π/4); `libm`'s sine and cosine add about one unit in the last place, not
/// proven (build piece G47). The tests hold the result within 3 units in the last place of a series
/// oracle at every k/2 ± 2^−j.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn sinpi(x: f64) -> Result<Real, NumError> {
    let r = reduce_half_turns(finite(x)?);
    let a = libm::fabs(r);
    let s = if a <= 0.25 {
        sin_pi_small(a)
    } else if a <= 0.75 {
        cos_pi_small(a - 0.5)
    } else {
        sin_pi_small(1.0 - a)
    };
    let signed = if r < 0.0 { -s } else { s };
    Real::new(zero_to_plus(signed))
}

/// Cosine of an angle given in half-turns: `cospi(x) = cos(πx)`. Exact (0 or ±1) at every multiple
/// of a quarter turn, where a zero is +0.
///
/// Method: IEEE 754-2019's `cosPi` (section 9.2), with the reduction of FreeBSD's `msun` `cospi`
/// (BSD-2-Clause; method only), as for [`sinpi`]: r in [−1, 1] half-turns, then by cos(−r) = cos(r)
/// to a = |r|, then one exact subtraction to |t| ≤ 1/4:
///
/// - a ≤ 1/4: cos(πa);
/// - 1/4 < a ≤ 3/4: cos(πa) = sin(π(1/2 − a)), with 1/2 − a exact by Sterbenz's lemma;
/// - a > 3/4: cos(πa) = −cos(π(1 − a)), with 1 − a exact by Sterbenz's lemma.
///
/// The accuracy argument is [`sinpi`]'s.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn cospi(x: f64) -> Result<Real, NumError> {
    let a = libm::fabs(reduce_half_turns(finite(x)?));
    let c = if a <= 0.25 {
        cos_pi_small(a)
    } else if a <= 0.75 {
        sin_pi_small(0.5 - a)
    } else {
        -cos_pi_small(1.0 - a)
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
            (-1.5, 1.0, 0.0),
            (1_000_000.5, 1.0, 0.0),
            (-1_000_001.0, 0.0, -1.0),
            (9_007_199_254_740_992.0, 0.0, 1.0),
            (4_503_599_627_370_497.0, 0.0, -1.0),
            (f64::MAX, 0.0, 1.0),
        ] {
            assert_eq!(ok(sinpi(x)).to_bits(), f64::to_bits(s), "sinpi({x})");
            assert_eq!(ok(cospi(x)).to_bits(), f64::to_bits(c), "cospi({x})");
        }
    }

    #[test]
    fn other_angles_are_close() {
        let x = 1.0 / 6.0;
        assert!((ok(sinpi(x)) - 0.5).abs() < 1e-15);
        assert!((ok(cospi(x)) - ok(sqrt(3.0)) / 2.0).abs() < 1e-15);
        // The quarter-turn boundaries of the reduction, from both sides.
        let r = core::f64::consts::FRAC_1_SQRT_2;
        for x in [0.25, 0.75, -0.25, -0.75] {
            assert!((ok(sinpi(x)).abs() - r).abs() < 2e-16, "sinpi({x})");
            assert!((ok(cospi(x)).abs() - r).abs() < 2e-16, "cospi({x})");
        }
    }

    /// The low half of π: π − `PI`, to double precision.
    const PI_LO: f64 = 1.224_646_799_147_353_2e-16;

    /// 2^−j for 0 ≤ j ≤ 1022, from its bits.
    fn pow2_neg(j: i32) -> f64 {
        f64::from_bits(u64::try_from(1023 - j).unwrap() << 52)
    }

    /// sin(π·2^−j) and cos(π·2^−j), to within about half a unit in the last place, for j ≥ 3: the
    /// argument x = π·2^−j in double-double (`PI` and `PI_LO` scaled by a power of two, exactly),
    /// then the Taylor series, whose correction terms are small enough relative to the leading
    /// term that their own rounding does not matter.
    fn oracle(j: i32) -> (f64, f64) {
        let eps = pow2_neg(j);
        let (x_hi, x_lo) = (core::f64::consts::PI * eps, PI_LO * eps);
        let x2 = x_hi * x_hi;
        // sin x = x + x · Σ_{k ≥ 1} (−x²)^k / (2k + 1)!, cos x = 1 + Σ_{k ≥ 1} (−x²)^k / (2k)!.
        let (mut s_term, mut c_term, mut s_sum, mut c_sum) = (1.0, 1.0, 0.0, 0.0);
        for k in 1..=12 {
            let k = f64::from(k);
            s_term *= -x2 / ((2.0 * k) * (2.0 * k + 1.0));
            c_term *= -x2 / ((2.0 * k - 1.0) * (2.0 * k));
            s_sum += s_term;
            c_sum += c_term;
        }
        (x_hi + (x_lo + x_hi * s_sum), 1.0 + c_sum)
    }

    /// Units in the last place between two doubles of the same sign.
    fn ulps(a: f64, b: f64) -> u64 {
        assert_eq!(a.is_sign_negative(), b.is_sign_negative(), "{a:e} {b:e}");
        a.to_bits().abs_diff(b.to_bits())
    }

    /// Next to every multiple of a quarter turn, k/2 ± 2^−j, `sinpi` and `cospi` are within 3
    /// units in the last place of the series oracle, both where the result is tiny (next to a
    /// zero) and where it is next to ±1. Reducing after multiplying by π, as before, missed by
    /// millions of units there (a relative error of 4.6 · 10^−5 at 1 − 2^−40).
    #[test]
    fn near_quarter_turns_the_relative_error_stays_small() {
        let mut worst = 0;
        for j in 3..=60 {
            let eps = pow2_neg(j);
            let (s, c) = oracle(j);
            for k in [-7i32, -4, -3, -2, -1, 0, 1, 2, 3, 4, 7, 1001] {
                let half = f64::from(k) * 0.5;
                for (sign, x) in [(1.0, half + eps), (-1.0, half - eps)] {
                    if (x - half) * sign != eps {
                        continue; // k/2 ± 2^−j is not a double: past 53 bits.
                    }
                    // k = 2m: sin(πm ± πε) = ±(−1)^m sin(πε), cos = (−1)^m cos(πε).
                    // k = 2m + 1: sin = (−1)^m cos(πε), cos = ∓(−1)^m sin(πε).
                    let m = k.div_euclid(2);
                    let parity = if m.rem_euclid(2) == 0 { 1.0 } else { -1.0 };
                    let (want_s, want_c) = if k.rem_euclid(2) == 0 {
                        (sign * parity * s, parity * c)
                    } else {
                        (parity * c, -sign * parity * s)
                    };
                    let (got_s, got_c) = (ok(sinpi(x)), ok(cospi(x)));
                    let (ds, dc) = (ulps(got_s, want_s), ulps(got_c, want_c));
                    assert!(
                        ds <= 3,
                        "sinpi({x:e}) = {got_s:e}, want {want_s:e}: {ds} ulps"
                    );
                    assert!(
                        dc <= 3,
                        "cospi({x:e}) = {got_c:e}, want {want_c:e}: {dc} ulps"
                    );
                    worst = worst.max(ds).max(dc);
                }
            }
        }
        assert!(worst <= 3, "{worst}");
    }

    /// Every function refuses NaN and both infinities in every argument, and never returns them.
    #[test]
    fn non_finite_arguments_are_refused() {
        let unary: [fn(f64) -> Result<Real, NumError>; 18] = [
            sqrt,
            sin,
            cos,
            tan,
            asin,
            acos,
            atan,
            exp,
            ln,
            log2,
            log10,
            cbrt,
            abs,
            floor,
            ceil,
            round_ties_even,
            sinpi,
            cospi,
        ];
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for f in unary {
                assert_eq!(f(bad), Err(NumError::NotFinite), "{bad}");
            }
            for (x, y) in [(bad, 1.0), (1.0, bad)] {
                assert_eq!(atan2(x, y), Err(NumError::NotFinite));
                assert_eq!(pow(x, y), Err(NumError::NotFinite));
                assert_eq!(hypot(x, y), Err(NumError::NotFinite));
                assert_eq!(fma(x, y, 1.0), Err(NumError::NotFinite));
                assert_eq!(fma(1.0, x, y), Err(NumError::NotFinite));
            }
            assert_eq!(powi(bad, 2), Err(NumError::NotFinite));
        }
    }

    /// Where `libm` would give NaN from finite arguments, the function reports `OutsideDomain`;
    /// where it would give an infinity, `NotFinite`.
    #[test]
    fn domains_poles_and_overflow_are_reported() {
        let outside = Err(NumError::OutsideDomain);
        let infinite = Err(NumError::NotFinite);
        let tiny = f64::from_bits(1);
        assert_eq!(sqrt(-1.0), outside);
        assert_eq!(sqrt(-tiny), outside);
        assert_eq!(ok(sqrt(-0.0)).to_bits(), (-0.0f64).to_bits());
        for x in [1.0 + f64::EPSILON, -2.0, f64::MAX, f64::MIN] {
            assert_eq!(asin(x), outside, "asin({x})");
            assert_eq!(acos(x), outside, "acos({x})");
        }
        assert_eq!(ok(asin(-1.0)), -core::f64::consts::FRAC_PI_2);
        assert_eq!(ok(acos(-1.0)), core::f64::consts::PI);
        for f in [ln, log2, log10] {
            assert_eq!(f(-1.0), outside);
            assert_eq!(f(-tiny), outside);
            assert_eq!(f(0.0), infinite);
            assert_eq!(f(-0.0), infinite);
            assert_eq!(ok(f(1.0)).to_bits(), 0.0f64.to_bits());
        }
        assert_eq!(pow(-8.0, 1.0 / 3.0), outside);
        assert_eq!(pow(-tiny, 0.5), outside);
        assert_eq!(ok(pow(-8.0, 3.0)), -512.0);
        assert_eq!(ok(pow(-2.0, -2.0)), 0.25);
        assert_eq!(pow(0.0, -1.0), infinite);
        assert_eq!(pow(-0.0, -1.0), infinite);
        assert_eq!(pow(10.0, 400.0), infinite);
        assert_eq!(exp(1000.0), infinite);
        assert_eq!(ok(exp(-1000.0)), 0.0);
        assert_eq!(hypot(f64::MAX, f64::MAX), infinite);
        assert_eq!(fma(f64::MAX, 2.0, 0.0), infinite);
        assert_eq!(powi(2.0, 1024), infinite);
        assert_eq!(powi(0.0, -1), infinite);
    }

    /// `powi` is `pow` with an exact exponent, and `fma` rounds once.
    #[test]
    fn powi_and_fma() {
        assert_eq!(ok(powi(-3.0, 3)), -27.0);
        assert_eq!(ok(powi(-0.5, 0)), 1.0);
        for n in [-30, -7, -1, 0, 1, 2, 5, 31] {
            assert_eq!(powi(1.1, n), pow(1.1, f64::from(n)), "{n}");
        }
        // (1 + 2^−52)(1 − 2^−52) − 1 = −2^−104 exactly, which only a single rounding keeps.
        let e = f64::EPSILON;
        assert_eq!(ok(fma(1.0 + e, 1.0 - e, -1.0)), -(e * e));
    }

    /// The signs of `atan2` at zero `y`, as documented.
    #[test]
    fn atan2_keeps_the_sign_of_a_zero_y() {
        let pi = core::f64::consts::PI;
        assert_eq!(ok(atan2(0.0, 1.0)).to_bits(), 0.0f64.to_bits());
        assert_eq!(ok(atan2(-0.0, 1.0)).to_bits(), (-0.0f64).to_bits());
        assert_eq!(ok(atan2(-0.0, 0.0)).to_bits(), (-0.0f64).to_bits());
        assert_eq!(ok(atan2(0.0, -1.0)), pi);
        assert_eq!(ok(atan2(-0.0, -1.0)), -pi);
        assert_eq!(ok(atan2(-0.0, -0.0)), -pi);
        assert_eq!(ok(atan2(0.0, -0.0)), pi);
    }
}
