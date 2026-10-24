//! Inputs for the error-free transformations: [`two_sum`], [`fast_two_sum`], [`split`] and
//! [`two_prod`].
//!
//! `geom_num::eft` takes any input: each function is exact whenever an exact result exists, and
//! otherwise reports that none does with an infinite second part. Each function has several paths,
//! and its batches take every one of them (the tests work out the path of every input from the
//! conditions `geom_num::eft` documents, and check that each is taken). Classes:
//!
//! - `typical`: doubles with exponents from −30 to 30, as survey and frame conversions produce, in
//!   either order: the direct path of every function.
//! - `extreme`: in rotation, the edges of the direct paths and every other path:
//!   - `two_sum`: sums near overflow, subnormal sums, huge plus tiny, the documented input whose
//!     intermediate overflows and takes the fallback, and a sum that overflows, `(s, −s)`.
//!   - `fast_two_sum`: sums near overflow, subnormal sums, huge plus tiny and a zero, each with the
//!     operand of smaller magnitude first half the time, so the function swaps them; and a sum
//!     that overflows.
//!   - `split`: values just below 2^996 (the largest the direct path takes), the smallest normal
//!     exponent, subnormals, ±0, values from 2^996 up (split after an exact scaling by 2^−64), and
//!     ±Ω, whose high half would be ±2^1024 (reported as a pair of infinities).
//!   - `two_prod`: products near overflow and huge times tiny, on the direct path; products on
//!     either side of 2^−968 and of 2^1022, the ends of the direct path, and a factor of 2^996 or
//!     more, on the scaled path where the direct one stops, with exact errors; a zero factor;
//!     products that overflow, `(p, −p)`; and products of two tiny factors, whose errors lie below
//!     2^−1074 and are reported.
//!
//! Guarantees: every batch is full and the same on every target, and takes the paths listed
//! (`tests/inputs.rs` checks it); every result is checked against exact rationals by
//! `tests/attack_g06.rs`.
//!
//! Method: boundary-value analysis (<https://en.wikipedia.org/wiki/Boundary-value_analysis>) on
//! the paths of the error-free transformations of Dekker (1971, <https://doi.org/10.1007/BF01397083>)
//! and Knuth (volume 2, section 4.2.2), as Shewchuk states them (1997,
//! <https://doi.org/10.1007/PL00009321>) and `geom_num::eft` extends them to every double.
//!
//! Cost: a few draws of [`SplitMix64`] per input, once per batch, outside every measured run.

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// A double ±(1 + f)·2^e with e pseudo-random in `lo..=hi` (both within the normal exponents
/// −1022..=1023), f a pseudo-random 52-bit fraction, and a pseudo-random sign.
pub(crate) fn double(g: &mut SplitMix64, lo: i64, hi: i64) -> f64 {
    let e = g.range(lo.max(-1022), hi.min(1023));
    let fraction = g.next_u64() >> 12;
    let sign = g.next_u64() >> 63;
    // e + 1023 is in 1..=2046, the biased exponent of a normal double.
    f64::from_bits((sign << 63) | ((e + 1023).unsigned_abs() << 52) | fraction)
}

/// A positive double (1 + f)·2^e with e in `lo..=hi`.
pub(crate) fn positive(g: &mut SplitMix64, lo: i64, hi: i64) -> f64 {
    let x = double(g, lo, hi);
    if x < 0.0 { -x } else { x }
}

/// A pseudo-random nonzero subnormal double with a pseudo-random sign.
pub(crate) fn subnormal(g: &mut SplitMix64) -> f64 {
    let fraction = (g.next_u64() >> 12).max(1);
    let sign = g.next_u64() >> 63;
    f64::from_bits((sign << 63) | fraction)
}

/// 2^e exactly, for a normal exponent e in −1022..=1023.
pub(crate) fn pow2(e: i64) -> f64 {
    f64::from_bits((e.clamp(-1022, 1023) + 1023).unsigned_abs() << 52)
}

/// The pair ordered so that the first has the larger magnitude. `f64::abs` clears the sign bit,
/// which is exact on every target (IEEE 754's `abs`), so this is no platform maths; it needs no
/// error either, where `geom_num::math::abs` checks for a finite argument that every caller here
/// already gives.
fn larger_first((a, b): (f64, f64)) -> (f64, f64) {
    if a.abs() >= b.abs() { (a, b) } else { (b, a) }
}

/// The pair with the operand of smaller magnitude first when `swap`, larger first otherwise.
fn ordered(pair: (f64, f64), swap: bool) -> (f64, f64) {
    let (large, small) = larger_first(pair);
    if swap { (small, large) } else { (large, small) }
}

/// `x` when `positive`, −x otherwise (negation is exact).
fn signed(x: f64, positive: bool) -> f64 {
    if positive { x } else { -x }
}

/// Pairs for [`geom_num::eft::two_sum`].
#[must_use]
pub fn two_sum(class: Class) -> Vec<(f64, f64)> {
    let mut g = rng("eft::two_sum", class);
    match class {
        Class::Typical => batch_of(|_| Some((double(&mut g, -30, 30), double(&mut g, -30, 30)))),
        Class::Extreme => batch_of(|n| {
            let up = (n / 5).is_multiple_of(2);
            Some(match n % 5 {
                // Both below 2^1022 in magnitude: the sum is below 2^1023.
                0 => (double(&mut g, 1000, 1021), double(&mut g, 1000, 1021)),
                1 => (subnormal(&mut g), subnormal(&mut g)),
                2 => (double(&mut g, 1000, 1021), double(&mut g, -1022, -1000)),
                // The documented case whose intermediate s − a overflows: a = ∓3·2^970,
                // b = ±(largest double).
                3 => (signed(-3.0 * pow2(970), up), signed(f64::MAX, up)),
                // Both at least 2^1023, with one sign: the sum overflows.
                _ => (
                    signed(positive(&mut g, 1023, 1023), up),
                    signed(positive(&mut g, 1023, 1023), up),
                ),
            })
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Pairs for [`geom_num::eft::fast_two_sum`].
#[must_use]
pub fn fast_two_sum(class: Class) -> Vec<(f64, f64)> {
    let mut g = rng("eft::fast_two_sum", class);
    match class {
        Class::Typical => batch_of(|_| Some((double(&mut g, -30, 30), double(&mut g, -30, 30)))),
        Class::Extreme => batch_of(|n| {
            // The operand of smaller magnitude first in every other round of the rotation.
            let swap = !(n / 5).is_multiple_of(2);
            Some(match n % 5 {
                0 => ordered(
                    (double(&mut g, 1000, 1021), double(&mut g, 1000, 1021)),
                    swap,
                ),
                1 => ordered((subnormal(&mut g), subnormal(&mut g)), swap),
                2 => ordered(
                    (double(&mut g, 1000, 1021), double(&mut g, -1022, -1000)),
                    swap,
                ),
                3 => ordered((double(&mut g, -1022, 1021), 0.0), swap),
                // Both at least 2^1023, with one sign: the sum overflows.
                _ => (
                    signed(positive(&mut g, 1023, 1023), swap),
                    signed(positive(&mut g, 1023, 1023), swap),
                ),
            })
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Values for [`geom_num::eft::split`].
#[must_use]
pub fn split(class: Class) -> Vec<f64> {
    let mut g = rng("eft::split", class);
    match class {
        Class::Typical => batch_of(|_| Some(double(&mut g, -30, 30))),
        Class::Extreme => batch_of(|n| {
            let up = (n / 6).is_multiple_of(2);
            Some(match n % 6 {
                // Just below 2^996: the largest values the direct path takes.
                0 => double(&mut g, 990, 995),
                1 => double(&mut g, -1022, -1022),
                2 => subnormal(&mut g),
                3 => signed(0.0, up),
                // From 2^996 up, below 2^1023: the scaled path, whose halves are finite.
                4 => double(&mut g, 996, 1022),
                // ±Ω: the high half would be ±2^1024, which no double holds.
                _ => signed(f64::MAX, up),
            })
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Pairs for [`geom_num::eft::two_prod`]. The direct path takes |a|, |b| < 2^996 with
/// 2^−968 ≤ |a · b| < 2^1022; every other finite product is scaled first. Below, e_a and e_b are
/// the factors' exponents.
#[must_use]
pub fn two_prod(class: Class) -> Vec<(f64, f64)> {
    let mut g = rng("eft::two_prod", class);
    match class {
        Class::Typical => batch_of(|_| Some((double(&mut g, -30, 30), double(&mut g, -30, 30)))),
        Class::Extreme => batch_of(|n| {
            Some(match n % 8 {
                // e_a + e_b from 1000 to 1020: products near overflow, on the direct path.
                0 => (double(&mut g, 500, 510), double(&mut g, 500, 510)),
                // e_a + e_b of −970 or −969: products either side of 2^−968, the direct path's
                // lower limit. The error is a whole number of 2^−1074, so it is exact either way.
                1 => (double(&mut g, -485, -485), double(&mut g, -485, -484)),
                // Huge times tiny, on the direct path.
                2 => (double(&mut g, 990, 995), double(&mut g, -1000, -980)),
                // A zero factor.
                3 => (0.0, double(&mut g, 990, 995)),
                // e_a + e_b of 1024 or more: the product overflows.
                4 => (double(&mut g, 512, 520), double(&mut g, 512, 520)),
                // A factor of 2^996 or more: the scaled path, with an exact error.
                5 => (double(&mut g, 996, 1020), double(&mut g, -60, -20)),
                // e_a + e_b = 1021: products either side of 2^1022, below 2^1023.
                6 => (double(&mut g, 511, 511), double(&mut g, 510, 510)),
                // e_a + e_b from −1080 to −1000: the error has bits below 2^−1074 and is reported.
                _ => (double(&mut g, -540, -500), double(&mut g, -540, -500)),
            })
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}
