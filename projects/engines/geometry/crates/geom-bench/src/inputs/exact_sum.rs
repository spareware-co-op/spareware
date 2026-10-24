//! Inputs for the exact sum of doubles, `geom_num::exact_sum::ExactSum`: adding terms one at a time
//! ([`add`]), from a slice ([`add_slice`]) and from an iterator ([`add_iter`]), merging two sums
//! ([`merge`]) and rounding the exact sum once ([`round`], [`round_ieee`]).
//!
//! An accumulator's cost is per term, so the inputs of the three adding operations are groups of
//! terms: [`TERMS`] for [`add`], and slices for the other two. Classes:
//!
//! - `typical`: doubles with exponents from −30 to 30 and both signs, as frame and survey data
//!   give; slices of [`SHORT`] terms, below `geom_num::exact_sum::LARGE_SLICE`, which go through
//!   the small superaccumulator one term at a time.
//! - `extreme`: terms from the whole range in rotation (subnormals, ±0, terms near the largest
//!   double, and pairs of huge terms that cancel), with NaNs of different payloads and infinities
//!   of both signs among them; slices of [`LONG`] terms, which go through the large
//!   superaccumulator, and for [`add_iter`] an iterator that does not know its length (it adds
//!   the first `LARGE_SLICE` terms one by one, then the rest through the large one); for the
//!   rounding, sums that are exactly zero, ties between two doubles, sums in the subnormal range,
//!   sums that overflow although every term is finite, and sums with special values (all
//!   refused by [`round`]); for [`merge`], sums with special values on either side and sums
//!   whose terms cancel across the two.
//!
//! Guarantees: every batch is full and the same on every target, and takes the paths listed
//! (`tests/inputs.rs` checks each claim); every result is checked against the exact sum by
//! `tests/attack_g06.rs`.
//!
//! Method: boundary-value analysis (<https://en.wikipedia.org/wiki/Boundary-value_analysis>) on the
//! paths `geom_num::exact_sum` documents: the small and large superaccumulators of Neal (2015,
//! <https://arxiv.org/abs/1505.05571>), the switch between them, the special values and the
//! rounding to nearest with ties to even.
//!
//! Cost: a few draws of [`SplitMix64`] per term, once per batch, outside every measured run.

use geom_num::exact_sum::{ExactSum, LARGE_SLICE};

use super::eft::{double, pow2, subnormal};
use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// Terms per input of [`add`].
pub const TERMS: usize = 16;

/// Terms in a typical slice: below `LARGE_SLICE`, so every term goes through `ExactSum::add`.
pub const SHORT: usize = 64;

/// Terms in an extreme slice: `LARGE_SLICE` exactly, the fewest that take the large
/// superaccumulator.
pub const LONG: usize = LARGE_SLICE;

/// A typical term.
fn typical(g: &mut SplitMix64) -> f64 {
    double(g, -30, 30)
}

/// A NaN with a pseudo-random nonzero payload and sign.
fn nan(g: &mut SplitMix64) -> f64 {
    let payload = (g.next_u64() >> 12).max(1);
    f64::from_bits((g.next_u64() & (1 << 63)) | 0x7ff0_0000_0000_0000 | payload)
}

/// Term `i` of an extreme group: the whole range in rotation, with a special value now and then
/// when `special` is set.
fn extreme(g: &mut SplitMix64, i: usize, special: bool) -> f64 {
    match i % 16 {
        0 | 1 => subnormal(g),
        2 => 0.0,
        3 => -0.0,
        4 => double(g, 1015, 1023),
        5 => -double(g, 1015, 1023),
        6 => double(g, -1022, -1000),
        7 if special => nan(g),
        8 if special => f64::INFINITY,
        9 if special => f64::NEG_INFINITY,
        _ => double(g, -1022, 1023),
    }
}

/// A group of terms: typical, or the extreme rotation (with special values every few groups).
fn terms(g: &mut SplitMix64, class: Class, group: usize, n: usize) -> Vec<f64> {
    let special = group % 4 == 3;
    (0..n)
        .map(|i| match class {
            Class::Extreme => extreme(g, i + group, special),
            _ => typical(g),
        })
        .collect()
}

/// Groups of [`TERMS`] terms for `ExactSum::add`.
#[must_use]
pub fn add(class: Class) -> Vec<[f64; TERMS]> {
    if !matches!(class, Class::Typical | Class::Extreme) {
        return Vec::new();
    }
    let mut g = rng("exact_sum::add", class);
    batch_of(|i| terms(&mut g, class, i, TERMS).try_into().ok())
}

/// Slices for `ExactSum::add_slice`: [`SHORT`] typical terms, or [`LONG`] extreme ones.
#[must_use]
pub fn add_slice(class: Class) -> Vec<Vec<f64>> {
    let mut g = rng("exact_sum::add_slice", class);
    match class {
        Class::Typical => batch_of(|i| Some(terms(&mut g, class, i, SHORT))),
        Class::Extreme => batch_of(|i| Some(terms(&mut g, class, i, LONG))),
        _ => Vec::new(),
    }
}

/// (terms, whether the iterator knows its length) for `ExactSum::add_iter`: [`SHORT`] typical
/// terms from a slice's iterator; or [`LONG`] + [`SHORT`] extreme terms, from a slice's iterator
/// (whose length is known, so the large superaccumulator takes every term) and, every other
/// input, from one that does not know it.
#[must_use]
pub fn add_iter(class: Class) -> Vec<(Vec<f64>, bool)> {
    let mut g = rng("exact_sum::add_iter", class);
    match class {
        Class::Typical => batch_of(|i| Some((terms(&mut g, class, i, SHORT), true))),
        Class::Extreme => batch_of(|i| Some((terms(&mut g, class, i, LONG + SHORT), i % 2 == 0))),
        _ => Vec::new(),
    }
}

/// The exact sum of `terms`, added one at a time.
fn sum_of(terms: &[f64]) -> ExactSum {
    let mut sum = ExactSum::new();
    for &x in terms {
        sum.add(x);
    }
    sum
}

/// The terms of the sums for the two roundings: [`TERMS`] typical terms; or, in rotation, an exact
/// zero, two ties between doubles (one rounding down to the even neighbour, one up), a subnormal
/// sum, a sum of finite terms that overflows, and the extreme rotation with special values.
fn sum_terms(op: &str, class: Class) -> Vec<Vec<f64>> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|i| Some(terms(&mut g, class, i, TERMS))),
        Class::Extreme => batch_of(|i| {
            let g = &mut g;
            let x = typical(g);
            let big = double(g, 900, 1000);
            Some(match i % 6 {
                // Cancels exactly, terms of both signs and zeros of both signs: +0.
                0 => vec![x, big, -0.0, -x, -big, 0.0],
                // 1 + 2^−53, halfway between 1 and its successor: rounds to the even, 1; and
                // (1 + 2^−52) + 2^−53, halfway the other way: rounds up to 1 + 2^−51.
                1 => vec![1.0, pow2(-53)],
                2 => vec![1.0 + pow2(-52), pow2(-53), big, -big],
                3 => vec![subnormal(g), subnormal(g), -subnormal(g)],
                // Finite terms whose sum reaches 2^1024: an overflow.
                4 => vec![double(g, 1023, 1023).abs(), double(g, 1023, 1023).abs()],
                _ => terms(g, class, i, TERMS),
            })
        }),
        _ => Vec::new(),
    }
}

/// The terms of the sums [`round`] rounds.
#[must_use]
pub fn round_terms(class: Class) -> Vec<Vec<f64>> {
    sum_terms("exact_sum::round", class)
}

/// The terms of the sums [`round_ieee`] rounds.
#[must_use]
pub fn round_ieee_terms(class: Class) -> Vec<Vec<f64>> {
    sum_terms("exact_sum::round_ieee", class)
}

/// Sums for `ExactSum::round`: the sums of [`round_terms`].
#[must_use]
pub fn round(class: Class) -> Vec<ExactSum> {
    round_terms(class).iter().map(|t| sum_of(t)).collect()
}

/// Sums for `ExactSum::round_ieee`: the sums of [`round_ieee_terms`].
#[must_use]
pub fn round_ieee(class: Class) -> Vec<ExactSum> {
    round_ieee_terms(class).iter().map(|t| sum_of(t)).collect()
}

/// The terms of the pairs of sums [`merge`] merges: typical terms; or extreme terms, with special
/// values on either side, and every third pair's second sum the exact negation of its first.
#[must_use]
pub fn merge_terms(class: Class) -> Vec<(Vec<f64>, Vec<f64>)> {
    let mut g = rng("exact_sum::merge", class);
    match class {
        Class::Typical => batch_of(|i| {
            let first = terms(&mut g, class, i, TERMS);
            Some((first, terms(&mut g, class, i + 1, TERMS)))
        }),
        Class::Extreme => batch_of(|i| {
            let first = terms(&mut g, class, i, TERMS);
            let second: Vec<f64> = if i % 3 == 0 {
                first.iter().map(|x| -x).collect()
            } else {
                terms(&mut g, class, i + 1, TERMS)
            };
            Some((first, second))
        }),
        _ => Vec::new(),
    }
}

/// Pairs of sums for `ExactSum::merge`: the sums of [`merge_terms`].
#[must_use]
pub fn merge(class: Class) -> Vec<(ExactSum, ExactSum)> {
    merge_terms(class)
        .iter()
        .map(|(a, b)| (sum_of(a), sum_of(b)))
        .collect()
}
