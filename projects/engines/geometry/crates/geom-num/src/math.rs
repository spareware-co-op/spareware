//! The only place maths functions come from, with proven error bounds.
//!
//! The standard library's `sin`, `cos`, `exp`, `powi` and friends have platform-dependent
//! precision (the Rust documentation calls it non-deterministic), so the same model would give
//! different bits on Windows, macOS and in the browser. Every maths function the kernel uses comes
//! from here instead (chapter 21), and a curved computation that feeds a decision may use one only
//! if its error bound is proven (chapter 12.3, "Maths functions need a proven bound").
//!
//! Guarantees, for every function here and in [`unproven`], and every argument:
//!
//! - **A finite result or an error, never NaN or an infinity.** Each function returns a [`Real`],
//!   or:
//!   - [`NumError::NotFinite`] when an argument is NaN or infinite (checked first), or the result
//!     would be infinite: an overflow, such as `exp(1000)`, or a pole, such as `ln(0)`;
//!   - [`NumError::OutsideDomain`] when the function has no real value there: the square root or
//!     a logarithm of a negative number, the arcsine or arccosine of a number beyond ±1, or a
//!     negative number to a power that is not a whole number. The domain is checked before the
//!     function is evaluated, so no function is evaluated where its value would be NaN.
//! - **Accuracy**, as the table below says: proven for the functions here, documented but not
//!   proven for those in [`unproven`].
//! - **Deterministic.** The same bits on every target, thread count and build: the correctly
//!   rounded results by their specification, the others because they are the same pure-Rust code
//!   everywhere.
//! - **Bounded proofs, and what is only tested.** Every function here returns its value through
//!   [`Real::new`], which Kani proves refuses exactly NaN and the infinities (`real`'s proofs), so
//!   none returns a NaN or an infinity, whatever its argument. Kani also proves (piece G41,
//!   chapter 28.7) that [`abs`], [`floor`], [`ceil`] and [`round_ties_even`] are exact for every
//!   finite double: a whole number (or the magnitude) at the right place, ties to even, and
//!   `NotFinite` for exactly the non-finite arguments. The correctly rounded functions rest on
//!   CORE-MATH's proofs and on IEEE 754 (for [`sqrt`] and [`fma`]), and are tested bit for bit
//!   against a big-number oracle (`tests/math_reference.rs`); their code (argument reduction,
//!   tables, polynomials) is beyond a bounded proof.
//!
//! # Use these
//!
//! | Function | Result | Error bound |
//! | --- | --- | --- |
//! | [`sinpi`], [`cospi`] | `sin(πx)`, `cos(πx)`; exact 0 and ±1 at multiples of ½ | correctly rounded¹ |
//! | [`exp()`] | `e^x` | correctly rounded¹ |
//! | [`ln`] | natural logarithm | correctly rounded¹ |
//! | [`atan2()`] | the angle of `(x, y)`, in `[−π, π]` | correctly rounded² |
//! | [`sqrt`] | square root | correctly rounded (IEEE 754) |
//! | [`fma`] | `a·b + c`, rounded once | correctly rounded (IEEE 754) |
//! | [`abs`], [`floor`], [`ceil`], [`round_ties_even`] | | exact |
//!
//! Their interval versions, for certified enclosures, are
//! [`Interval::sinpi`](crate::interval::Interval::sinpi),
//! [`Interval::cospi`](crate::interval::Interval::cospi),
//! [`Interval::exp`](crate::interval::Interval::exp),
//! [`Interval::ln`](crate::interval::Interval::ln) and
//! [`Interval::atan2`](crate::interval::Interval::atan2): the peaks of `sinpi` and `cospi` are
//! found by exact comparisons, never missed or invented, and `atan2`'s branch cut and the origin
//! are decided exactly.
//!
//! "Correctly rounded" means the result is the binary64 number nearest the exact mathematical
//! value, ties to even: an error of at most half an ulp, and a result fixed bit for bit by the
//! specification, so it is identical on every target and stays identical through any future
//! reimplementation. The five transcendental functions are ports of CORE-MATH
//! (<https://core-math.gitlabpages.inria.fr/>, MIT licence), whose method is Ziv's strategy: a
//! fast evaluation with a proven error bound and a rounding test, then a more accurate one when
//! the test cannot decide. Each module documents its source file and commit, its domain, every
//! change from the C source, and the proof of its bound. Each public function checks its domain
//! first, as above, then returns the port's result unchanged, apart from the sign of a zero
//! `sinpi` ("Zeros", below).
//!
//! ¹ What is proven, and what correct rounding rests on. Each accurate path has a proven error
//! bound, so it rounds correctly, unconditionally, every input whose exact value has fewer than
//! `t₀` identical bits after its round bit (Lemma 2): `t₀` = 44 for `sinpi` and `cospi`, 48 for
//! `exp` (33 for results below 2^−1059), 55 for `ln`. Inputs closer to a rounding boundary are
//! what CORE-MATH's worst-case lists exist to hold; every listed input is checked and correctly
//! rounded. That the lists hold every such input is the statement of CORE-MATH's searches, not a
//! proof made here, and two of the lists qualify it: exp.wc says its main search covers "most
//! inputs", and log.wc that it "should have all" its worst cases. An input the lists missed would
//! still be within `½ + 2^−45.6` ulp (`sinpi`, `cospi`), `½ + 2^−49.2` ulp (`exp`; `½ + 2^−34`
//! below 2^−1059) or `½ + 2^−56.6` ulp (`ln`): those bounds are proven for every input.
//!
//! ² `atan2` has two arguments, so its hard-to-round cases cannot be enumerated at all. Its
//! accurate path's error is proven below 2^−182.63 (relative), every ingredient re-derived here;
//! it is correctly rounded unless the exact angle lies that close to a rounding boundary, which no
//! input has ever been found to do, and its error is proven below `½ + 2^−128` ulp in every case
//! (see its module).
//!
//! **For certified enclosures** (chapter 12.3) correct rounding is not needed: every result above
//! is proven faithful, one of the two doubles either side of the exact value, because it is either
//! decided by a proven rounding test or rounded to nearest from a value within a small fraction of
//! an ulp of the exact one. So `next_down(r) ≤ exact ≤ next_up(r)`, and an interval widened by one
//! step on each side, as [`crate::interval`] does after every operation and the interval versions
//! do, encloses the exact value, for every input.
//!
//! Angles in half-turns ([`sinpi`], [`cospi`]) are the kernel's preferred form: the reduction
//! modulo a full turn is exact, so `sinpi(0.5)` is exactly 1 and `cospi(0.5)` exactly 0, with no
//! rounding residue from π, and the relative accuracy holds right next to every multiple of a
//! quarter turn, where rotations by a near quarter turn need it.
//!
//! Everything else, `tan`, `asin`, `acos`, `atan`, `log2`, `log10`, `pow`, `powi`, `hypot`, `cbrt`
//! and the radian `sin` and `cos`, is in [`unproven`], with `libm`'s own versions of the five
//! proven functions: deterministic, but without a proven bound, so an operation that needs one
//! fails with `UnprovenMaths` unless the caller asks for an approximate result (chapter 12.3).
//! Nothing in this crate calls them.
//!
//! **Zeros.** A zero result of `sinpi` or `cospi` (at a whole or a half turn) is +0, whatever the
//! argument's sign: the crate's one-zero rule ([`crate::eft`], [`crate::interval`]), so the result
//! has the same bits whatever the argument's history. The port follows IEEE 754-2019 §9.2.1, which
//! gives `sinpi(−n) = −0` for a positive integer `n` and `sinpi(−0) = −0`; the public function adds
//! +0, which changes exactly those zeros and nothing else (the proof is on `zero_to_plus` in
//! `real.rs`). Every other function keeps IEEE 754's sign of zero: `sqrt(−0)`, `floor(−0)`,
//! `ceil(−0.5)`, `round_ties_even(−0.25)` and `atan2(−0, x)` for `x > 0` are −0, as IEEE 754 makes
//! them.
//!
//! **NaN.** No public function returns NaN: a NaN or infinite argument, an argument outside the
//! domain and an infinite result are each reported, as above. Inside this module the ports compute
//! on raw doubles; their own results for `±∞` and NaN, and the logarithm's for a negative number
//! (which the public functions never pass them), are always the canonical NaN,
//! `0x7ff8_0000_0000_0000`, never the hardware's default NaN or a propagated payload, whose bits
//! differ between targets (chapter 21). The kernel stays NaN-free the way chapters 16.2 and 21 set
//! out: the argument of a partial operation is checked before the call (scalars are
//! [`crate::real::Real`]s, checked at construction, these functions check their domains, and
//! fields check every partial operation's interval when they are built), or the interval versions
//! are used, which return an error outside the domain, never a NaN.
//!
//! **Speed.** `tests/math_bench.rs` times each function against `libm`'s on the same arguments;
//! piece G47's target is at most twice `libm`'s time.
//!
//! Method: the five transcendental functions are ports of CORE-MATH
//! (<https://core-math.gitlabpages.inria.fr/>), each following Ziv's strategy with its proof in its
//! own module; the square root and the fused multiply-add are IEEE 754-2019's correctly rounded
//! operations (<https://doi.org/10.1109/IEEESTD.2019.8766229>), through `libm` 0.2.16
//! (<https://docs.rs/libm/0.2.16>); `abs` and the roundings are exact operations on the bits.
//!
//! Cost: constant per call and no allocation: each function checks its argument, then runs its
//! port's fast path, and the accurate path only when the fast path's rounding test cannot decide;
//! each module states its own cost. Speed against `libm` is measured as above.
//!
//! # Proofs: the shared lemmas
//!
//! Notation: `u = 2^−53`, the unit roundoff; RN is IEEE rounding to nearest, ties to even, the only
//! rounding Rust has (RFC 3514). For a result `a` in the normal range, `|RN(a) − a| ≤ u·|a|`. Each
//! proof says why no quantity it bounds this way is subnormal.
//!
//! **Lemma 1 (Ziv's rounding test).** Let `v` be the exact value, `h` and `l` doubles with
//! `|v − (h + l)| ≤ E`, and `e > 0` a double with `e·(1 − u) ≥ E + u·|l|`. If
//! `RN(h + RN(l − e)) = RN(h + RN(l + e)) = w`, then `RN(v) = w`.
//! *Proof.* Since `|l ∓ e| ≤ |l| + e`, `RN(l − e) ≤ (l − e) + u·|l − e| ≤ l − e·(1 − u) + u·|l| ≤
//! l − E`, and likewise `RN(l + e) ≥ l + E`; so `h + RN(l − e) ≤ v ≤ h + RN(l + e)`, and RN is
//! monotone. ∎ The factor `1 − u` and the term `u·|l|` account for the rounding of `l ± e` itself,
//! which a bound on `|v − (h + l)|` alone does not cover. A proof that checks the stronger
//! `e·(1 − u) ≥ E + u·(|l| + e)` instead has checked this too. (The near-zero tests of `sinpi` and
//! `cospi`, `RN(RN(l ± e) + h)`, are the same.)
//!
//! **Lemma 2 (worst cases).** Let `v` be a value with fewer than `t` identical bits after its round
//! bit, and `a` an approximation with `|a − v| < 2^−(t+1)·ulp(v)`. Then `RN(a) = RN(v)`.
//! *Proof.* Write `v = (m + f)·ulp(v)` with `m` an integer and `0 ≤ f < 1`. If `|f − ½| <
//! 2^−(t+1)`, the `t` bits of `f` after its round bit are all 0 (round bit 1) or all 1 (round bit
//! 0). So the nearest midpoint is at least `2^−(t+1)·ulp(v)` from `v`, none lies between `a` and
//! `v`, and both round alike. ∎ A path without a rounding test (an accurate path, or a near-zero
//! shortcut) is therefore correctly rounded when its error is below `2^−(t+1)·ulp(v)` (for a
//! normal `v`, `2^−(t+54)·|v|` suffices) and every input with at least `t` identical bits is
//! checked directly. CORE-MATH publishes such inputs per function (its `*.wc` files, from
//! Lefèvre's and its own exhaustive searches with `BaCSeL`); each module states the threshold `t` and
//! the ranges its list covers. Every listed input was checked against the big-number reference of
//! the tests (`tests/math_full_worst_cases.rs`, which reads the lists, `sinpi`'s images of its
//! set (b) under powers of two included; a sample of each list runs in every build).
//!
//! **Lemma 3 (exact transformations).** For doubles `a`, `b` with no overflow: (i) `fast_two_sum(a,
//! b)` returns `s = RN(a + b)` and `e` with `s + e = a + b` exactly when `a = 0`, when `|a| ≥ |b|`
//! (Dekker), or when `a` is an integer multiple of `ulp(b)`: then `a + b` is a multiple of `ulp(b)`
//! below `2^(e_b + 2)`, so `s` is exact or off by at most `ulp(b)`, `s − a` is exact and so is
//! `e`. (ii) `fma(a, b, −RN(a·b)) = a·b − RN(a·b)` exactly when `|a·b| ≥ 2^−969`. (iii) `a − b` is
//! exact when `b/2 ≤ a ≤ 2b` (Sterbenz).
//!
//! **Lemma 4 (double-double products).** For `x = xh + xl`, `c = ch + cl` with `|xl| ≤ λ·|xh|`,
//! `|cl| ≤ μ·|ch|` and `λ, μ ≤ 32u`, CORE-MATH's `muldd` (in its `muldd_acc` and `muldd`
//! variants) returns `h + l` with `|h + l − x·c| ≤ (u² + 3.001u·(λ + μ) + λμ)·|xh·ch|` and
//! `|l| ≤ 1.001·(u + λ + μ)·|xh·ch|`. *Proof.* `ch·xh = RN(ch·xh) + δ` exactly (Lemma 3); the cross
//! terms `ch·xl` and `cl·xh` are rounded (`u·(λ + μ)` together), summed (another `u·(λ + μ)`) and
//! added to `δ` (`u·(u + λ + μ)`); `cl·xl` is dropped (`λμ`); the final fast two-sum is exact. ∎
//!
//! The polynomial approximation errors in the proofs are bounded rigorously, not sampled: by the
//! difference between each coefficient and the target's Taylor coefficient, or, where the
//! polynomial is a minimax fit and those differences cancel, by evaluating the difference
//! polynomial in interval arithmetic on a subdivision of the interval, plus a Taylor remainder.
//! These numbers, and every table's error, are computed with exact arithmetic by each module's unit
//! tests, which name the proof steps they check.

mod atan2;
mod enclose;
mod exp;
mod hexf;
mod log;
mod sincospi;
mod support;
pub mod unproven;

/// The big-number oracle of the integration tests, shared with the unit tests that check the
/// proofs' numeric ingredients (table and polynomial errors) exhaustively.
#[cfg(test)]
#[path = "../tests/math_reference.rs"]
pub(crate) mod reference;

use crate::error::NumError;
use crate::real::{Real, zero_to_plus};

/// The canonical quiet NaN, `0x7ff8_0000_0000_0000`: positive, quiet, zero payload. Every NaN the
/// ports in this module compute is this one, written as its bit pattern because `f64::NAN` does
/// not promise one. No public function returns it.
const NAN: f64 = f64::from_bits(0x7ff8_0000_0000_0000);

/// The argument itself, or `NotFinite` if it is NaN or infinite.
const fn finite(x: f64) -> Result<f64, NumError> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(NumError::NotFinite)
    }
}

/// The domain of the logarithms: `x` itself when it is finite and not negative. Zero passes, and
/// its logarithm, −∞, is reported as `NotFinite` by [`Real::new`].
fn log_argument(x: f64) -> Result<f64, NumError> {
    let x = finite(x)?;
    if x < 0.0 {
        Err(NumError::OutsideDomain)
    } else {
        Ok(x)
    }
}

/// Sine of an angle given in half-turns: `sinpi(x) = sin(πx)`, correctly rounded.
///
/// Error at most half an ulp for every argument (see the module documentation and
/// `sincospi.rs`, CORE-MATH's `sinpi.c`). Exactly 0 or ±1 at every multiple of ½:
/// `sinpi(2n + ½) = 1`, `sinpi(2n − ½) = −1`, and `sinpi(n) = +0`, a zero always +0 (the module
/// documentation, "Zeros"), so results are the same bits whatever the argument's history. No
/// limit on the argument's size: the reduction modulo a whole turn is exact.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn sinpi(x: f64) -> Result<Real, NumError> {
    Real::new(zero_to_plus(sincospi::sinpi(finite(x)?)))
}

/// Cosine of an angle given in half-turns: `cospi(x) = cos(πx)`, correctly rounded.
///
/// Error at most half an ulp for every argument (see the module documentation and
/// `sincospi.rs`, CORE-MATH's `cospi.c`). Exactly 0 or ±1 at every multiple of ½:
/// `cospi(2n) = 1`, `cospi(2n + 1) = −1`, `cospi(n + ½) = +0`.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn cospi(x: f64) -> Result<Real, NumError> {
    Real::new(zero_to_plus(sincospi::cospi(finite(x)?)))
}

/// e to the power x, correctly rounded (see the module documentation and `exp.rs`, CORE-MATH's
/// `exp.c`). Below about −745.13 the result is +0, which is the correctly rounded value there.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or when e^x overflows (from
/// `0x1.62e42fefa39fp+9`, about 709.78, up).
pub fn exp(x: f64) -> Result<Real, NumError> {
    Real::new(exp::exp(finite(x)?))
}

/// Natural logarithm, correctly rounded (see the module documentation and `log.rs`, CORE-MATH's
/// `log.c`). `ln(1) = +0`.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument, or for ±0, whose logarithm is −∞;
/// [`NumError::OutsideDomain`] for `x < 0`.
pub fn ln(x: f64) -> Result<Real, NumError> {
    Real::new(log::ln(log_argument(x)?))
}

/// The angle of the point (x, y) from the x-axis, in radians, in [−π, π], correctly rounded
/// unless the exact angle lies within 2^−182.63 (relative) of a rounding boundary, and within
/// `½ + 2^−128` ulp always (see the module documentation and `atan2.rs`, CORE-MATH's `atan2.c`).
///
/// A zero `y` keeps its sign in the result, as IEEE 754's `atan2` specifies: (±0, x) gives ±0 for
/// x > 0 or x = +0, and ±π for x < 0 or x = −0. So −π is returned only for y = −0, and (0, 0)
/// gives a zero or ±π by the signs of its zeros rather than failing.
///
/// # Errors
///
/// [`NumError::NotFinite`] for a NaN or infinite argument.
pub fn atan2(y: f64, x: f64) -> Result<Real, NumError> {
    Real::new(atan2::atan2(finite(y)?, finite(x)?))
}

/// Square root, correctly rounded (IEEE 754 requires it, and `libm` computes it so on every
/// target). `sqrt(−0)` is −0, as IEEE 754 specifies.
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

/// [`fma`] of arguments already known to be finite, as a double: the eight fused multiply-adds of
/// `Interval::mul_add`, whose operands are interval bounds, finite by construction, so the
/// argument checks of [`fma`] would refuse nothing.
///
/// Always inline: through [`fma`], LLVM inlined some of `mul_add`'s eight calls and not others,
/// and how many moved with unrelated code. G47's x86-64 count gate caught `mul_add` going from
/// five inlined calls to four, 8% more instructions, with neither function changed; forcing
/// [`fma`] itself inline instead cost the double-double two-product spills, 6.6% more cycles.
///
/// # Errors
///
/// [`NumError::NotFinite`] when the result overflows.
#[allow(
    clippy::inline_always,
    reason = "measured: LLVM's choice moved with unrelated code, on x86-64"
)]
#[inline(always)]
pub(crate) fn fma_of_finite(a: f64, b: f64, c: f64) -> Result<f64, NumError> {
    Real::new(libm::fma(a, b, c)).map(Real::get)
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

    /// Exact (0 or ±1) at every multiple of a quarter turn, a zero always +0, at small and huge
    /// arguments alike: 2^53 is even, 2^52 + 1 is odd, and `f64::MAX` is a multiple of 2^971.
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
            (-f64::MAX, 0.0, 1.0),
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
        // Correctly rounded: within half an ulp of the exact ½ and √3/2.
        assert!((ok(sinpi(x)) - 0.5).abs() <= f64::EPSILON / 2.0);
        assert!((ok(cospi(x)) - ok(sqrt(3.0)) / 2.0).abs() <= f64::EPSILON);
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
    /// units in the last place of the series oracle (itself within about half a unit), both where
    /// the result is tiny (next to a zero) and where it is next to ±1. Reducing after multiplying
    /// by π, as `libm`'s radian functions would, misses by millions of units there (a relative
    /// error of 4.6 · 10^−5 at 1 − 2^−40).
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

    /// The one-argument functions, proven and unproven.
    const UNARY: [fn(f64) -> Result<Real, NumError>; 22] = [
        sqrt,
        unproven::sin,
        unproven::cos,
        unproven::tan,
        unproven::asin,
        unproven::acos,
        unproven::atan,
        exp,
        ln,
        unproven::log2,
        unproven::log10,
        unproven::cbrt,
        abs,
        floor,
        ceil,
        round_ties_even,
        sinpi,
        cospi,
        unproven::exp,
        unproven::ln,
        unproven::sinpi,
        unproven::cospi,
    ];

    /// A two-argument maths function.
    type Binary = fn(f64, f64) -> Result<Real, NumError>;

    /// The two-argument functions, proven and unproven.
    const BINARY: [Binary; 4] = [atan2, unproven::atan2, unproven::pow, unproven::hypot];

    /// Every function refuses NaN and both infinities in every argument, and never returns them.
    #[test]
    fn non_finite_arguments_are_refused() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for f in UNARY {
                assert_eq!(f(bad), Err(NumError::NotFinite), "{bad}");
            }
            for (x, y) in [(bad, 1.0), (1.0, bad)] {
                for f in BINARY {
                    assert_eq!(f(x, y), Err(NumError::NotFinite));
                }
                assert_eq!(fma(x, y, 1.0), Err(NumError::NotFinite));
                assert_eq!(fma(1.0, x, y), Err(NumError::NotFinite));
            }
            assert_eq!(unproven::powi(bad, 2), Err(NumError::NotFinite));
        }
    }

    /// Where a function has no real value for finite arguments, it reports `OutsideDomain`; where
    /// its value would be infinite, `NotFinite`.
    #[test]
    fn domains_poles_and_overflow_are_reported() {
        let outside = Err(NumError::OutsideDomain);
        let infinite = Err(NumError::NotFinite);
        let tiny = f64::from_bits(1);
        assert_eq!(sqrt(-1.0), outside);
        assert_eq!(sqrt(-tiny), outside);
        assert_eq!(ok(sqrt(-0.0)).to_bits(), (-0.0f64).to_bits());
        for x in [1.0 + f64::EPSILON, -2.0, f64::MAX, f64::MIN] {
            assert_eq!(unproven::asin(x), outside, "asin({x})");
            assert_eq!(unproven::acos(x), outside, "acos({x})");
        }
        assert_eq!(ok(unproven::asin(-1.0)), -core::f64::consts::FRAC_PI_2);
        assert_eq!(ok(unproven::acos(-1.0)), core::f64::consts::PI);
        for f in [ln, unproven::ln, unproven::log2, unproven::log10] {
            assert_eq!(f(-1.0), outside);
            assert_eq!(f(-tiny), outside);
            assert_eq!(f(f64::MIN), outside);
            assert_eq!(f(0.0), infinite);
            assert_eq!(f(-0.0), infinite);
            assert_eq!(ok(f(1.0)).to_bits(), 0.0f64.to_bits());
        }
        assert_eq!(unproven::pow(-8.0, 1.0 / 3.0), outside);
        assert_eq!(unproven::pow(-tiny, 0.5), outside);
        assert_eq!(ok(unproven::pow(-8.0, 3.0)), -512.0);
        assert_eq!(ok(unproven::pow(-2.0, -2.0)), 0.25);
        assert_eq!(unproven::pow(0.0, -1.0), infinite);
        assert_eq!(unproven::pow(-0.0, -1.0), infinite);
        assert_eq!(unproven::pow(10.0, 400.0), infinite);
        for f in [exp, unproven::exp] {
            assert_eq!(f(1000.0), infinite);
            assert_eq!(f(709.8), infinite);
            assert_eq!(ok(f(-1000.0)).to_bits(), 0.0f64.to_bits());
            assert_eq!(ok(f(0.0)), 1.0);
        }
        assert_eq!(unproven::hypot(f64::MAX, f64::MAX), infinite);
        assert_eq!(fma(f64::MAX, 2.0, 0.0), infinite);
        assert_eq!(unproven::powi(2.0, 1024), infinite);
        assert_eq!(unproven::powi(0.0, -1), infinite);
    }

    /// `powi` is `pow` with an exact exponent, and `fma` rounds once.
    #[test]
    fn powi_and_fma() {
        assert_eq!(ok(unproven::powi(-3.0, 3)), -27.0);
        assert_eq!(ok(unproven::powi(-0.5, 0)), 1.0);
        for n in [-30, -7, -1, 0, 1, 2, 5, 31] {
            assert_eq!(
                unproven::powi(1.1, n),
                unproven::pow(1.1, f64::from(n)),
                "{n}"
            );
        }
        // (1 + 2^−52)(1 − 2^−52) − 1 = −2^−104 exactly, which only a single rounding keeps.
        let e = f64::EPSILON;
        assert_eq!(ok(fma(1.0 + e, 1.0 - e, -1.0)), -(e * e));
    }

    /// The signs of `atan2` at zero `y`, as documented, for the proven function and its unproven
    /// twin.
    #[test]
    fn atan2_keeps_the_sign_of_a_zero_y() {
        let pi = core::f64::consts::PI;
        for f in [atan2, unproven::atan2] {
            assert_eq!(ok(f(0.0, 1.0)).to_bits(), 0.0f64.to_bits());
            assert_eq!(ok(f(-0.0, 1.0)).to_bits(), (-0.0f64).to_bits());
            assert_eq!(ok(f(-0.0, 0.0)).to_bits(), (-0.0f64).to_bits());
            assert_eq!(ok(f(0.0, -1.0)), pi);
            assert_eq!(ok(f(-0.0, -1.0)), -pi);
            assert_eq!(ok(f(-0.0, -0.0)), -pi);
            assert_eq!(ok(f(0.0, -0.0)), pi);
        }
    }

    /// The public functions return the ports' results unchanged, except that a zero `sinpi` or
    /// `cospi` is +0: the ports keep IEEE 754-2019 §9.2.1's signs, `sinpi(−n) = −0`.
    #[test]
    fn the_public_functions_are_the_ports() {
        assert_eq!(sincospi::sinpi(-1.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(sincospi::sinpi(-0.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(ok(sinpi(-1.0)).to_bits(), 0.0f64.to_bits());
        assert_eq!(ok(sinpi(-0.0)).to_bits(), 0.0f64.to_bits());
        for x in [
            0.1,
            -0.3,
            2.5e-300,
            0.75,
            1e10 + 0.125,
            1_099_511_627_776.375,
        ] {
            assert_eq!(ok(sinpi(x)), sincospi::sinpi(x), "{x}");
            assert_eq!(ok(cospi(x)), sincospi::cospi(x), "{x}");
            assert_eq!(ok(atan2(x, 0.7)), atan2::atan2(x, 0.7), "{x}");
            if x.abs() < 700.0 {
                assert_eq!(ok(exp(x)), exp::exp(x), "{x}");
            }
            if x > 0.0 {
                assert_eq!(ok(ln(x)), log::ln(x), "{x}");
            }
        }
        // The ports' canonical NaN, which the public functions never let out.
        for v in [
            sincospi::sinpi(f64::INFINITY),
            sincospi::cospi(f64::NAN),
            log::ln(-1.0),
        ] {
            assert_eq!(v.to_bits(), NAN.to_bits());
        }
    }

    /// NaNs of both signs, quiet and signalling, with and without a payload.
    const NANS: [u64; 6] = [
        0x7ff8_0000_0000_0000,
        0xfff8_0000_0000_0000,
        0x7ff8_0000_0000_0001,
        0xfff4_0000_dead_beef,
        0x7ff0_0000_0000_0001,
        0xfff0_0000_0000_0001,
    ];

    /// The ports compute on raw doubles, and every NaN they make is the canonical one, whatever
    /// the sign or payload of a NaN argument (chapter 21): for a NaN argument, for `sinpi` and
    /// `cospi` of ±∞, and for the logarithm of a negative number. The public functions never pass
    /// them these arguments; the rule holds inside the module all the same.
    #[test]
    fn the_ports_make_only_the_canonical_nan() {
        let mut made: Vec<(String, f64)> = Vec::new();
        for bits in NANS {
            let n = f64::from_bits(bits);
            made.push((format!("sinpi({bits:#x})"), sincospi::sinpi(n)));
            made.push((format!("cospi({bits:#x})"), sincospi::cospi(n)));
            made.push((format!("exp({bits:#x})"), exp::exp(n)));
            made.push((format!("ln({bits:#x})"), log::ln(n)));
            for w in [1.0, -1.0, 0.0, f64::INFINITY, n] {
                made.push((format!("atan2({bits:#x}, {w})"), atan2::atan2(n, w)));
                made.push((format!("atan2({w}, {bits:#x})"), atan2::atan2(w, n)));
            }
        }
        for x in [f64::INFINITY, f64::NEG_INFINITY] {
            made.push((format!("sinpi({x})"), sincospi::sinpi(x)));
            made.push((format!("cospi({x})"), sincospi::cospi(x)));
        }
        for x in [-1.0, f64::NEG_INFINITY, -f64::from_bits(1), -f64::MAX] {
            made.push((format!("ln({x:e})"), log::ln(x)));
        }
        for (what, v) in made {
            assert_eq!(v.to_bits(), NAN.to_bits(), "{what} = {:#018x}", v.to_bits());
        }
    }

    /// The `sinpi` and `cospi` ports keep IEEE 754-2019 §9.2.1's values and signs at every
    /// multiple of ½ (`sinpi(n)` has the sign of `n`, `cospi(n + ½)` is +0), small and huge alike,
    /// on both sides of the reduction's 2^41 threshold; the public functions then make every zero
    /// +0 (`the_public_functions_are_the_ports`).
    #[test]
    fn the_ports_keep_ieee_values_and_signs_at_multiples_of_a_half() {
        let p2 = |k: i32| libm::scalbn(1.0, k);
        let (two40, two41, two51) = (p2(40), p2(41), p2(51));
        let sin_cases = [
            (0.0, 0.0),
            (-0.0, -0.0),
            (1.0, 0.0),
            (-1.0, -0.0),
            (2.0, 0.0),
            (-2.0, -0.0),
            (two41, 0.0),
            (-two41, -0.0),
            (two41 + 1.0, 0.0),
            (-(two41 + 1.0), -0.0),
            (p2(52) + 1.0, 0.0),
            (-(p2(52) + 1.0), -0.0),
            (f64::MAX, 0.0),
            (-f64::MAX, -0.0),
            (0.5, 1.0),
            (-0.5, -1.0),
            (1.5, -1.0),
            (-1.5, 1.0),
            (2.5, 1.0),
            (-2.5, -1.0),
            (two41 + 0.5, 1.0),
            (two41 + 1.5, -1.0),
            (-(two41 + 0.5), -1.0),
            (-(two41 + 1.5), 1.0),
            (two51 + 0.5, 1.0),
            (two51 + 1.5, -1.0),
            (-(two51 + 0.5), -1.0),
            (two40 + 0.5, 1.0),
            (-(two40 + 1.5), 1.0),
        ];
        for (x, want) in sin_cases {
            let got = sincospi::sinpi(x);
            assert_eq!(got.to_bits(), f64::to_bits(want), "sinpi({x:e}) = {got:e}");
        }
        let cos_cases = [
            (0.0, 1.0),
            (-0.0, 1.0),
            (1.0, -1.0),
            (-1.0, -1.0),
            (2.0, 1.0),
            (3.0, -1.0),
            (0.5, 0.0),
            (-0.5, 0.0),
            (1.5, 0.0),
            (-1.5, 0.0),
            (two51 + 0.5, 0.0),
            (-(two51 + 0.5), 0.0),
            (two41 + 0.5, 0.0),
            (-(two41 + 0.5), 0.0),
            (two41 + 1.0, -1.0),
            (-(two41 + 1.0), -1.0),
            (two41 + 2.0, 1.0),
            (p2(52) + 1.0, -1.0),
            (p2(53), 1.0),
            (f64::MAX, 1.0),
            (-f64::MAX, 1.0),
            (two40 + 1.0, -1.0),
        ];
        for (x, want) in cos_cases {
            let got = sincospi::cospi(x);
            assert_eq!(got.to_bits(), f64::to_bits(want), "cospi({x:e}) = {got:e}");
        }
    }

    /// The `exp`, `ln` and `atan2` ports keep C99 Annex F's special values (F.10.3.1, F.10.3.7,
    /// F.10.1.4), as their modules document. The public functions refuse these arguments and
    /// results (`NotFinite`); the ports' values are what the C sources return.
    #[test]
    fn the_ports_keep_c99_special_values() {
        let pi = core::f64::consts::PI;
        let half_pi = core::f64::consts::FRAC_PI_2;
        let quarter_pi = core::f64::consts::FRAC_PI_4;
        // 3π/4 rounded to nearest, 0x1.2d97c7f3321d2p+1.
        let three_quarter_pi = f64::from_bits(0x4002_d97c_7f33_21d2);
        let inf = f64::INFINITY;
        for (x, want) in [
            (0.0, 1.0),
            (-0.0, 1.0),
            (inf, inf),
            (-inf, 0.0),
            (f64::MAX, inf),
            (-f64::MAX, 0.0),
            (1000.0, inf),
            (-1000.0, 0.0),
        ] {
            let got = exp::exp(x);
            assert_eq!(got.to_bits(), f64::to_bits(want), "exp({x:e}) = {got:e}");
        }
        for (x, want) in [(1.0, 0.0), (0.0, -inf), (-0.0, -inf), (inf, inf)] {
            let got = log::ln(x);
            assert_eq!(got.to_bits(), f64::to_bits(want), "ln({x:e}) = {got:e}");
        }
        let y_fin = 3.5;
        for (y, x, want) in [
            (0.0, -0.0, pi),
            (-0.0, -0.0, -pi),
            (0.0, 0.0, 0.0),
            (-0.0, 0.0, -0.0),
            (0.0, -2.0, pi),
            (-0.0, -2.0, -pi),
            (0.0, 2.0, 0.0),
            (-0.0, 2.0, -0.0),
            (-y_fin, 0.0, -half_pi),
            (-y_fin, -0.0, -half_pi),
            (y_fin, 0.0, half_pi),
            (y_fin, -0.0, half_pi),
            (y_fin, -inf, pi),
            (-y_fin, -inf, -pi),
            (y_fin, inf, 0.0),
            (-y_fin, inf, -0.0),
            (inf, 2.0, half_pi),
            (-inf, 2.0, -half_pi),
            (inf, -2.0, half_pi),
            (-inf, -2.0, -half_pi),
            (inf, -inf, three_quarter_pi),
            (-inf, -inf, -three_quarter_pi),
            (inf, inf, quarter_pi),
            (-inf, inf, -quarter_pi),
            (0.0, inf, 0.0),
            (-0.0, -inf, -pi),
            (inf, 0.0, half_pi),
            (-inf, -0.0, -half_pi),
            (f64::from_bits(1), inf, 0.0),
            (f64::MAX, -inf, pi),
        ] {
            let got = atan2::atan2(y, x);
            assert_eq!(
                got.to_bits(),
                f64::to_bits(want),
                "atan2({y:e}, {x:e}) = {got:e}"
            );
        }
    }
}

/// Bounded proofs (chapter 28.7; piece G41): the exact functions, for every double.
#[cfg(kani)]
// cargo-mutants reads this attribute from the source and skips the module (its harnesses are
// proven by Kani, and no test build compiles them); no build evaluates it, since the module
// exists only under Kani, where `test` is off.
#[cfg_attr(test, mutants::skip)]
mod proofs {
    use super::*;

    /// Whether a finite double is a whole number, from its bits: zero, or a value whose fraction
    /// bits below its units bit are all zero.
    fn is_whole(r: f64) -> bool {
        if r == 0.0 {
            return true;
        }
        let bits = r.to_bits();
        let field = i64::try_from((bits >> 52) & 0x7ff).unwrap();
        let e = field - 1023;
        if e < 0 {
            false
        } else if e >= 52 {
            true
        } else {
            bits & ((1u64 << (52 - e)) - 1) == 0
        }
    }

    /// The result for a double with these bits: `Some(value)` when the function answered,
    /// `None` when it reported `NotFinite`, which it must do exactly for non-finite arguments.
    fn answer(x: f64, r: Result<Real, NumError>) -> Option<f64> {
        match r {
            Ok(v) => {
                assert!(x.is_finite());
                Some(v.get())
            }
            Err(e) => {
                assert!(e == NumError::NotFinite && !x.is_finite());
                None
            }
        }
    }

    /// `abs` is the magnitude, exactly.
    #[kani::proof]
    fn abs_is_exact() {
        let x = f64::from_bits(kani::any());
        if let Some(r) = answer(x, abs(x)) {
            assert!(r >= 0.0 && (r == x || r == -x));
        }
    }

    /// Every double of magnitude at least 2^52 is a whole number.
    const WHOLE_FROM: f64 = 4_503_599_627_370_496.0;

    /// `floor` is the largest whole number not above x: x itself from 2^52 up, else r ≤ x < r + 1
    /// (`r + 1` is exact below 2^52; `x − r` is not, as for x = −2^−1074).
    #[kani::proof]
    fn floor_is_exact() {
        let x = f64::from_bits(kani::any());
        if let Some(r) = answer(x, floor(x)) {
            assert!(is_whole(r) && r <= x);
            assert!(if x >= WHOLE_FROM || x <= -WHOLE_FROM {
                r == x
            } else {
                x < r + 1.0
            });
        }
    }

    /// `ceil` is the smallest whole number not below x, as [`floor_is_exact`] states it.
    #[kani::proof]
    fn ceil_is_exact() {
        let x = f64::from_bits(kani::any());
        if let Some(r) = answer(x, ceil(x)) {
            assert!(is_whole(r) && r >= x);
            assert!(if x >= WHOLE_FROM || x <= -WHOLE_FROM {
                r == x
            } else {
                r - 1.0 < x
            });
        }
    }

    /// `round_ties_even` is the nearest whole number, a tie going to the even one.
    #[kani::proof]
    fn round_ties_even_is_exact() {
        let x = f64::from_bits(kani::any());
        if let Some(r) = answer(x, round_ties_even(x)) {
            let d = if x > r { x - r } else { r - x };
            assert!(is_whole(r) && d <= 0.5);
            if d == 0.5 {
                assert!(is_whole(r * 0.5));
            }
        }
    }
}
