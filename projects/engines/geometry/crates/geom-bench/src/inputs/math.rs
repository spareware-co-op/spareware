//! Inputs for the deterministic maths functions of `geom_num::math`, one builder per function.
//!
//! Each function draws finite arguments from the domain it is defined on, so no input is refused
//! as `NotFinite` for its argument or as `OutsideDomain`: every result is a `Real`, or
//! `NotFinite` for a result that overflows. Classes:
//!
//! - `typical`: arguments of ordinary size for the function (angles below 2^10 radians, logarithms
//!   of values from 2^−30 to just below 2^31, and so on; for `sinpi` and `cospi` a quarter are
//!   exact multiples of a quarter turn, which take their exact branch; for `powi`, bases of either
//!   sign to whole powers from −20 to 20; for `fma`, three doubles of ordinary size).
//! - `extreme`: in rotation, the function's hard edges: huge arguments (sines of 2^1000 need the
//!   longest argument reduction), tiny and subnormal arguments, results near overflow and
//!   underflow, and past overflow where the function has one (`exp` just past 709.78, `powi` and
//!   `fma` of large values: `NotFinite`), arguments next to the domain's ends (asin of 1 − 2^−52),
//!   and for `fma` products that the addend cancels, which only the single rounding keeps.
//!
//! Method: the hard cases of elementary-function implementations, each a separate path through
//! libm (which `geom_num::math` wraps): arguments that need the longest trigonometric argument
//! reduction (Payne and Hanek 1983, <https://doi.org/10.1145/1057600.1057602>), results near
//! overflow and underflow, subnormals, and arguments next to the ends of each domain.
//!
//! Guarantees: every batch is full and the same on every target; every input is finite and no
//! result is NaN, and the extreme sines need the longest argument reduction
//! (`tests/inputs.rs`).
//!
//! Cost: a few draws of [`crate::rng::SplitMix64`] per input, once per batch, outside every
//! measured run.

use super::eft::{double, positive, pow2, subnormal};
use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// Where a one-argument function's inputs come from.
#[derive(Clone, Copy)]
enum Domain {
    /// Positive values: square root and logarithms.
    Positive,
    /// Any angle in radians: sine, cosine, tangent.
    Angle,
    /// [−1, 1]: arcsine and arccosine.
    Unit,
    /// Any finite value: arctangent and cube root.
    Any,
    /// Exponent arguments: |x| from 2^−10 to below 2^9 when typical; near overflow and underflow
    /// when extreme.
    Exp,
    /// Values to round: floor, ceiling, absolute value, rounding to even.
    Rounding,
    /// Angles in half-turns: sinpi and cospi.
    HalfTurns,
}

/// Where a two-argument function's inputs come from.
#[derive(Clone, Copy)]
enum Domain2 {
    /// atan2(y, x).
    Atan2,
    /// pow(x, y) with x positive.
    Pow,
    /// hypot(x, y).
    Hypot,
}

/// A positive subnormal.
fn tiny(g: &mut SplitMix64) -> f64 {
    let x = subnormal(g);
    if x < 0.0 { -x } else { x }
}

/// One argument from `domain` for class `class`; `n` is the position in the batch, which picks
/// the extreme case in rotation.
fn argument(g: &mut SplitMix64, domain: Domain, class: Class, n: usize) -> f64 {
    let extreme = class == Class::Extreme;
    let k = n % 4;
    match (domain, extreme) {
        (Domain::Positive, false) => positive(g, -30, 30),
        (Domain::Positive, true) => match k {
            0 => tiny(g),
            1 => positive(g, 1000, 1023),
            2 => positive(g, -1022, -1022),
            // 1 + 2^−e: logarithms of values next to 1 lose the most to cancellation.
            _ => 1.0 + pow2(-g.range(1, 52)),
        },
        (Domain::Angle, false) => double(g, -10, 9),
        (Domain::Angle, true) => match k {
            // The longest argument reduction: the most bits of 2/π are needed.
            0 => double(g, 1000, 1023),
            1 => double(g, -1022, -27),
            2 => subnormal(g),
            // Large and medium arguments, from 2^20 up.
            _ => double(g, 20, 999),
        },
        (Domain::Unit, false) => double(g, -20, -1),
        (Domain::Unit, true) => {
            let s = if g.sign() > 0 { 1.0 } else { -1.0 };
            match k {
                0 => s * (1.0 - pow2(-g.range(1, 53))),
                1 => double(g, -1022, -27),
                2 => subnormal(g),
                _ => s,
            }
        }
        (Domain::Any, false) => double(g, -30, 30),
        (Domain::Any, true) => match k {
            0 => double(g, 1000, 1023),
            1 => double(g, -1022, -1000),
            2 => subnormal(g),
            _ => 0.0,
        },
        (Domain::Exp, false) => double(g, -10, 8),
        (Domain::Exp, true) => match k {
            // Near and just past the overflow threshold, about 709.78.
            0 => 709.0 + positive(g, -10, -1),
            // Results in the subnormal range and below it.
            1 => -708.0 - positive(g, -2, 5),
            2 => double(g, -1022, -30),
            _ => subnormal(g),
        },
        (Domain::Rounding, false) => double(g, -10, 30),
        (Domain::Rounding, true) => match k {
            // Already whole: at and above 2^52 every double is an integer.
            0 => double(g, 52, 1023),
            1 => double(g, -1022, -1),
            2 => subnormal(g),
            // Exact halves: the ties.
            _ => f64::from(i32::try_from(g.range(-1_000_000, 1_000_000)).unwrap_or(0)) + 0.5,
        },
        (Domain::HalfTurns, false) => {
            if k == 0 {
                // A whole number of quarter turns: the exact branch.
                f64::from(i32::try_from(g.range(-1000, 1000)).unwrap_or(0)) * 0.5
            } else {
                double(g, -10, 9)
            }
        }
        (Domain::HalfTurns, true) => match k {
            // At and above 2^53 every double is even: a whole number of turns.
            0 => double(g, 53, 1023),
            1 => double(g, 20, 52),
            2 => double(g, -1022, -27),
            _ => subnormal(g),
        },
    }
}

/// Two arguments from `domain` for class `class`.
fn arguments(g: &mut SplitMix64, domain: Domain2, class: Class, n: usize) -> (f64, f64) {
    let extreme = class == Class::Extreme;
    let k = n % 4;
    match (domain, extreme) {
        (Domain2::Atan2 | Domain2::Hypot, false) => (double(g, -20, 20), double(g, -20, 20)),
        (Domain2::Atan2 | Domain2::Hypot, true) => match k {
            0 => (double(g, 1000, 1023), double(g, 1000, 1023)),
            1 => (double(g, -1022, -1000), double(g, -1022, -1000)),
            2 => (double(g, 1000, 1023), double(g, -1022, -1000)),
            _ => (subnormal(g), subnormal(g)),
        },
        (Domain2::Pow, false) => (positive(g, -3, 3), double(g, -4, 5)),
        (Domain2::Pow, true) => match k {
            // A base next to 1 raised to a huge power.
            0 => (1.0 + pow2(-g.range(30, 52)), double(g, 30, 50)),
            // A huge base to a fractional power.
            1 => (positive(g, 1000, 1023), double(g, -2, -1)),
            // A subnormal base.
            2 => (tiny(g), double(g, -2, 0)),
            // Two to a whole power near the ends of the exponent range.
            _ => (
                2.0,
                f64::from(i32::try_from(g.range(-1074, 1023)).unwrap_or(0)),
            ),
        },
    }
}

/// (base, whole exponent) pairs for `powi` of class `class`; `n` picks the extreme case in
/// rotation.
fn powi_arguments(g: &mut SplitMix64, class: Class, n: usize) -> Option<(f64, i32)> {
    // A whole exponent in lo..=hi. Every range below lies inside `i32`, so the conversion never
    // fails; were it to, the candidate is drawn again rather than replaced by another exponent.
    let whole = |g: &mut SplitMix64, lo: i64, hi: i64| i32::try_from(g.range(lo, hi)).ok();
    Some(match (class == Class::Extreme, n % 5) {
        (false, _) => (double(g, -3, 3), whole(g, -20, 20)?),
        // A base next to 1 raised to a huge power, either way: from about 1 to past overflow.
        (true, 0) => (1.0 + pow2(-g.range(20, 52)), whole(g, -(1 << 30), 1 << 30)?),
        // Two to a whole power near the ends of the exponent range: subnormal results, and past
        // 1023 an overflow (NotFinite).
        (true, 1) => (2.0, whole(g, -1074, 1030)?),
        // A subnormal base to small powers: underflow to zero, and for negative powers huge
        // results or an overflow.
        (true, 2) => (subnormal(g), whole(g, -2, 3)?),
        // The ends of the exponent's type.
        (true, 3) => (
            double(g, -1, 0),
            if g.sign() > 0 { i32::MAX } else { i32::MIN },
        ),
        // A huge base to a small power: near and past overflow.
        (true, _) => (double(g, 300, 1023), whole(g, 1, 4)?),
    })
}

/// (a, b, c) triples for `fma` of class `class`; `n` picks the extreme case in rotation.
fn fma_arguments(g: &mut SplitMix64, class: Class, n: usize) -> (f64, f64, f64) {
    match (class == Class::Extreme, n % 5) {
        (false, _) => (double(g, -30, 30), double(g, -30, 30), double(g, -30, 30)),
        // c = −(a · b) rounded: the result is the exact rounding error of the product, which a
        // separate multiplication and addition would lose.
        (true, 0) => {
            let (a, b) = (double(g, -500, 500), double(g, -500, 500));
            (a, b, -(a * b))
        }
        // A product past the largest double: NotFinite.
        (true, 1) => (
            double(g, 600, 1023),
            double(g, 600, 1023),
            double(g, -30, 30),
        ),
        // A product near the largest double, with an addend that keeps it finite or not.
        (true, 2) => (
            positive(g, 511, 511),
            positive(g, 511, 511),
            double(g, 1000, 1023),
        ),
        // Subnormal products and addends.
        (true, 3) => (double(g, -540, -500), double(g, -540, -500), subnormal(g)),
        // Huge times tiny, plus an ordinary value.
        (true, _) => (
            double(g, 990, 1023),
            double(g, -1022, -990),
            double(g, -30, 30),
        ),
    }
}

/// A batch of one-argument inputs.
fn unary(op: &str, domain: Domain, class: Class) -> Vec<f64> {
    let mut g = rng(op, class);
    match class {
        Class::Typical | Class::Extreme => batch_of(|n| Some(argument(&mut g, domain, class, n))),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// A batch of two-argument inputs.
fn binary(op: &str, domain: Domain2, class: Class) -> Vec<(f64, f64)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical | Class::Extreme => batch_of(|n| Some(arguments(&mut g, domain, class, n))),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Arguments for `math::sqrt`.
#[must_use]
pub fn sqrt(class: Class) -> Vec<f64> {
    unary("math::sqrt", Domain::Positive, class)
}

/// Arguments for `math::sin`.
#[must_use]
pub fn sin(class: Class) -> Vec<f64> {
    unary("math::sin", Domain::Angle, class)
}

/// Arguments for `math::cos`.
#[must_use]
pub fn cos(class: Class) -> Vec<f64> {
    unary("math::cos", Domain::Angle, class)
}

/// Arguments for `math::tan`.
#[must_use]
pub fn tan(class: Class) -> Vec<f64> {
    unary("math::tan", Domain::Angle, class)
}

/// Arguments for `math::asin`.
#[must_use]
pub fn asin(class: Class) -> Vec<f64> {
    unary("math::asin", Domain::Unit, class)
}

/// Arguments for `math::acos`.
#[must_use]
pub fn acos(class: Class) -> Vec<f64> {
    unary("math::acos", Domain::Unit, class)
}

/// Arguments for `math::atan`.
#[must_use]
pub fn atan(class: Class) -> Vec<f64> {
    unary("math::atan", Domain::Any, class)
}

/// (y, x) pairs for `math::atan2`.
#[must_use]
pub fn atan2(class: Class) -> Vec<(f64, f64)> {
    binary("math::atan2", Domain2::Atan2, class)
}

/// Arguments for `math::exp`.
#[must_use]
pub fn exp(class: Class) -> Vec<f64> {
    unary("math::exp", Domain::Exp, class)
}

/// Arguments for `math::ln`.
#[must_use]
pub fn ln(class: Class) -> Vec<f64> {
    unary("math::ln", Domain::Positive, class)
}

/// Arguments for `math::log2`.
#[must_use]
pub fn log2(class: Class) -> Vec<f64> {
    unary("math::log2", Domain::Positive, class)
}

/// Arguments for `math::log10`.
#[must_use]
pub fn log10(class: Class) -> Vec<f64> {
    unary("math::log10", Domain::Positive, class)
}

/// (base, exponent) pairs for `math::pow`.
#[must_use]
pub fn pow(class: Class) -> Vec<(f64, f64)> {
    binary("math::pow", Domain2::Pow, class)
}

/// (x, y) pairs for `math::hypot`.
#[must_use]
pub fn hypot(class: Class) -> Vec<(f64, f64)> {
    binary("math::hypot", Domain2::Hypot, class)
}

/// Arguments for `math::cbrt`.
#[must_use]
pub fn cbrt(class: Class) -> Vec<f64> {
    unary("math::cbrt", Domain::Any, class)
}

/// Arguments for `math::abs`.
#[must_use]
pub fn abs(class: Class) -> Vec<f64> {
    unary("math::abs", Domain::Rounding, class)
}

/// Arguments for `math::floor`.
#[must_use]
pub fn floor(class: Class) -> Vec<f64> {
    unary("math::floor", Domain::Rounding, class)
}

/// Arguments for `math::ceil`.
#[must_use]
pub fn ceil(class: Class) -> Vec<f64> {
    unary("math::ceil", Domain::Rounding, class)
}

/// Arguments for `math::round_ties_even`.
#[must_use]
pub fn round_ties_even(class: Class) -> Vec<f64> {
    unary("math::round_ties_even", Domain::Rounding, class)
}

/// Arguments, in half-turns, for `math::sinpi`.
#[must_use]
pub fn sinpi(class: Class) -> Vec<f64> {
    unary("math::sinpi", Domain::HalfTurns, class)
}

/// Arguments, in half-turns, for `math::cospi`.
#[must_use]
pub fn cospi(class: Class) -> Vec<f64> {
    unary("math::cospi", Domain::HalfTurns, class)
}

/// (base, whole exponent) pairs for `math::powi`.
#[must_use]
pub fn powi(class: Class) -> Vec<(f64, i32)> {
    let mut g = rng("math::powi", class);
    match class {
        Class::Typical | Class::Extreme => batch_of(|n| powi_arguments(&mut g, class, n)),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// (a, b, c) triples for `math::fma`, a · b + c.
#[must_use]
pub fn fma(class: Class) -> Vec<(f64, f64, f64)> {
    let mut g = rng("math::fma", class);
    match class {
        Class::Typical | Class::Extreme => batch_of(|n| Some(fma_arguments(&mut g, class, n))),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}
