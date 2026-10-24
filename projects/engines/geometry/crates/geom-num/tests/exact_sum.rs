//! `ExactSum` against an exact oracle, and its order-freedom.
//!
//! Oracle: every double is a whole multiple of 2^−1074, so a sum of doubles is an integer N times
//! 2^−1074; N is summed exactly in `dashu` integers and rounded to the nearest double, ties to
//! even, by a rounding written here from the definition (gradual underflow, and overflow from
//! 2^1024 − 2^970 up), which is itself checked against the hardware's correctly rounded sum of two
//! doubles and against `dashu`'s correctly rounded rational conversion. Nothing here calls the
//! code under test to compute an expected value.
//!
//! What is held:
//!
//! - correct rounding, bit for bit, on random sums and on sums built to be hard: huge
//!   cancellation, all subnormals, magnitudes spanning 2^−1074 to 2^1023, exact ties (to even) and
//!   near-ties decided by a far bit, results at the overflow threshold and intermediate overflow;
//! - identical bits under any permutation of the terms, any chunking into slices or accumulators,
//!   and any order or shape of merges (property tests), through both internal paths;
//! - Neal's special values;
//! - the boundaries of the carry allowances (2047 additions) and of the large accumulator's chunk
//!   counts (4096 additions);
//! - streams (`add_iter`, `Extend`, `FromIterator`) of known and unknown length on both sides of
//!   their switch to the large accumulator, and that a stream stops at its first `None`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names
)]

use dashu_int::ops::BitTest;
use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;
use geom_num::ExactSum;
use geom_num::NumError;
use geom_num::exact_sum::LARGE_SLICE;
use proptest::prelude::*;

// ---------------------------------------------------------------------------------------------
// The oracle.

/// A finite double as an integer multiple of 2^−1074.
fn units(x: f64) -> IBig {
    assert!(x.is_finite());
    let bits = x.to_bits();
    let field = (bits >> 52) & 0x7ff;
    let fraction = bits & ((1 << 52) - 1);
    let magnitude = if field == 0 {
        UBig::from(fraction)
    } else {
        UBig::from(fraction | (1 << 52)) << (field as usize - 1)
    };
    let n = IBig::from(magnitude);
    if bits >> 63 == 1 { -n } else { n }
}

/// The exact sum of finite doubles, in units of 2^−1074.
fn exact_units(terms: &[f64]) -> IBig {
    let mut n = IBig::ZERO;
    for &x in terms {
        n += units(x);
    }
    n
}

/// N·2^−1074 rounded to the nearest double, ties to even; +0 for zero, ±∞ from 2^1024 − 2^970 up.
fn round_units(n: &IBig) -> f64 {
    if *n == IBig::ZERO {
        return 0.0;
    }
    let negative = *n < IBig::ZERO;
    let m: UBig = if negative {
        UBig::try_from(-n.clone()).unwrap()
    } else {
        UBig::try_from(n.clone()).unwrap()
    };
    let length = m.bit_len(); // the leading bit is at 2^(length − 1 − 1074)
    let e = length as i64 - 1 - 1074;
    let quantum = (e - 52).max(-1074);
    let shift = (quantum + 1074) as usize;
    let mut q = &m >> shift;
    if shift > 0 {
        let rest = &m - (&q << shift);
        let half = UBig::ONE << (shift - 1);
        let odd = q.bit(0);
        if rest > half || (rest == half && odd) {
            q += UBig::ONE;
        }
    }
    // q·2^quantum, with q at most 2^53.
    let q = u64::try_from(q).unwrap();
    let (q, quantum) = if q == 1 << 53 {
        (1 << 52, quantum + 1)
    } else {
        (q, quantum)
    };
    let bits = if q < (1 << 52) {
        assert_eq!(quantum, -1074);
        q
    } else if quantum + 52 > 1023 {
        0x7ff_u64 << 52
    } else {
        (((quantum + 52 + 1023) as u64) << 52) | (q - (1 << 52))
    };
    let x = f64::from_bits(bits);
    if negative { -x } else { x }
}

/// The correctly rounded exact sum.
fn oracle(terms: &[f64]) -> f64 {
    round_units(&exact_units(terms))
}

/// The sum by `ExactSum`, one term at a time.
fn by_add(terms: &[f64]) -> f64 {
    let mut s = ExactSum::new();
    for &x in terms {
        s.add(x);
    }
    s.round_ieee()
}

/// The sum by `ExactSum::add_slice`, which takes the large accumulator from `LARGE_SLICE` terms.
fn by_slice(terms: &[f64]) -> f64 {
    let mut s = ExactSum::new();
    s.add_slice(terms);
    s.round_ieee()
}

/// The sum by `ExactSum::add_iter` through `FromIterator`, on an iterator that knows its length,
/// so from `LARGE_SLICE` terms it goes to the large accumulator at once.
fn by_iter(terms: &[f64]) -> f64 {
    let s: ExactSum = terms.iter().copied().collect();
    s.round_ieee()
}

/// The sum by `ExactSum::add_iter` through `Extend`, on a stream of unknown length (lower size
/// bound 0), so the first `LARGE_SLICE` terms go one by one and the rest to the large accumulator.
fn by_stream(terms: &[f64]) -> f64 {
    let mut s = ExactSum::new();
    s.extend(terms.iter().copied().filter(|_| true));
    s.round_ieee()
}

/// Both paths against the oracle, bit for bit, and `round` agreeing with `round_ieee`.
fn check(what: &str, terms: &[f64]) {
    let expected = oracle(terms);
    for (path, got) in [
        ("add", by_add(terms)),
        ("add_slice", by_slice(terms)),
        ("add_iter, known length", by_iter(terms)),
        ("add_iter, unknown length", by_stream(terms)),
    ] {
        assert_eq!(
            got.to_bits(),
            expected.to_bits(),
            "{what} ({} terms) by {path}: {got:e}, expected {expected:e}",
            terms.len()
        );
    }
    let mut s = ExactSum::new();
    s.add_slice(terms);
    match s.round() {
        Ok(r) => assert_eq!(r.get().to_bits(), expected.to_bits()),
        Err(e) => {
            assert_eq!(e, NumError::NotFinite);
            assert!(expected.is_infinite(), "{what}: refused a finite sum");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Generators.

/// `SplitMix64`.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }

    /// A uniformly random permutation of `v` (Fisher–Yates).
    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            v.swap(i, j);
        }
    }
}

/// A double from its sign, leading-bit exponent e (−1074 to 1023) and fraction bits.
fn with_exponent(negative: bool, e: i64, fraction: u64) -> f64 {
    assert!(
        (-1074..=1023).contains(&e),
        "no double has its leading bit at 2^{e}"
    );
    let bits = if e >= -1022 {
        (((e + 1023) as u64) << 52) | (fraction & ((1 << 52) - 1))
    } else {
        let lead = e + 1074;
        (1_u64 << lead) | (fraction & ((1_u64 << lead) - 1))
    };
    let x = f64::from_bits(bits);
    if negative { -x } else { x }
}

/// A random double with leading-bit exponent in [lo, hi], either sign.
fn double_in(rng: &mut Rng, lo: i64, hi: i64) -> f64 {
    let e = rng.range(lo, hi);
    with_exponent(rng.next() & 1 == 1, e, rng.next())
}

/// 2^k as a double, for −1074 ≤ k ≤ 1023.
fn p2(k: i64) -> f64 {
    with_exponent(false, k, 0)
}

// ---------------------------------------------------------------------------------------------
// The oracle itself.

#[test]
fn oracle_rounds_like_the_hardware_and_like_dashu() {
    let mut rng = Rng(1);
    for _ in 0..20_000 {
        let (a, b) = (
            double_in(&mut rng, -1074, 1023),
            double_in(&mut rng, -1074, 1023),
        );
        // The hardware sum of two doubles is their exact sum correctly rounded (overflow too).
        assert_eq!(
            oracle(&[a, b]).to_bits(),
            (a + b + 0.0).to_bits(),
            "{a:e} + {b:e}"
        );
        let (c, d) = (
            double_in(&mut rng, -1074, -1000),
            double_in(&mut rng, -1074, -1000),
        );
        assert_eq!(
            oracle(&[c, d]).to_bits(),
            (c + d + 0.0).to_bits(),
            "{c:e} + {d:e}"
        );
    }
    for _ in 0..2000 {
        let terms: Vec<f64> = (0..5).map(|_| double_in(&mut rng, -1074, 1000)).collect();
        let n = exact_units(&terms);
        let exact = RBig::from_parts(n.clone(), UBig::ONE << 1074);
        assert_eq!(
            round_units(&n).to_bits(),
            (exact.to_f64().value() + 0.0).to_bits()
        );
    }
    assert_eq!(oracle(&[f64::MAX, p2(970)]), f64::INFINITY);
    assert_eq!(oracle(&[f64::MAX, p2(969)]), f64::MAX);
    assert_eq!(oracle(&[-f64::MAX, -p2(970)]), f64::NEG_INFINITY);
}

// ---------------------------------------------------------------------------------------------
// Correct rounding.

#[test]
fn random_sums_are_correctly_rounded() {
    let mut rng = Rng(2);
    for round in 0..300 {
        let n = [0, 1, 2, 3, 10, 100, 511, 512, 513, 2000, 5000][round % 11];
        let (lo, hi) = [
            (-1074, 1023),
            (-60, 60),
            (-1074, -1000),
            (900, 1020),
            (-5, 5),
        ][round % 5];
        let terms: Vec<f64> = (0..n).map(|_| double_in(&mut rng, lo, hi)).collect();
        check("random", &terms);
    }
}

#[test]
fn huge_cancellation() {
    let mut rng = Rng(3);
    for _ in 0..100 {
        // Pairs x, −x over the whole range, plus a few small residues, shuffled: the exact sum is
        // the residues alone, far below the terms.
        let mut terms = Vec::new();
        for _ in 0..rng.range(1, 1500) {
            let x = double_in(&mut rng, -1074, 1023);
            terms.push(x);
            terms.push(-x);
        }
        for _ in 0..rng.range(0, 3) {
            terms.push(double_in(&mut rng, -1074, -900));
        }
        rng.shuffle(&mut terms);
        check("cancellation", &terms);
    }
    // Ω + Ω − Ω: an intermediate far beyond Ω, an exact sum of Ω.
    check("intermediate overflow", &[f64::MAX, f64::MAX, -f64::MAX]);
    check(
        "intermediate overflow",
        &[f64::MAX; 1000]
            .iter()
            .chain(&[-f64::MAX; 999])
            .copied()
            .collect::<Vec<_>>(),
    );
    // 1 + 2^100 − 2^100 = 1, which a plain sum loses.
    check("absorbed", &[1.0, p2(100), -p2(100)]);
}

#[test]
fn all_subnormals_and_mixed_magnitudes() {
    let mut rng = Rng(4);
    for _ in 0..100 {
        let n = rng.range(1, 3000) as usize;
        let subnormals: Vec<f64> = (0..n).map(|_| double_in(&mut rng, -1074, -1023)).collect();
        check("subnormals", &subnormals);
        // One term at every binade from 2^−1074 to 2^1023, both signs, in random order.
        let mut mixed: Vec<f64> = (-1074..=1023)
            .map(|e| with_exponent(rng.next() & 1 == 1, e, rng.next()))
            .collect();
        rng.shuffle(&mut mixed);
        mixed.truncate(rng.range(1, mixed.len() as i64) as usize);
        check("mixed magnitudes", &mixed);
    }
    // Subnormals that sum into the normal range and back.
    let tiny = f64::from_bits((1 << 52) - 1);
    check("subnormals to normal", &[tiny, tiny, f64::from_bits(2)]);
    check(
        "subnormals to normal and back",
        &[tiny, tiny, -tiny, f64::from_bits(1)],
    );
}

#[test]
fn ties_to_even_and_far_sticky_bits() {
    let u = p2(-53);
    let cases: Vec<(&str, Vec<f64>)> = vec![
        ("1 + u: a tie, to the even 1", vec![1.0, u]),
        ("1 + 3u: a tie, to the even 1 + 4u", vec![1.0, 3.0 * u]),
        (
            "1 + u + 2^−1074: above the tie",
            vec![1.0, u, f64::from_bits(1)],
        ),
        (
            "1 + u − 2^−1074: below the tie",
            vec![1.0, u, -f64::from_bits(1)],
        ),
        ("−1 − u: a tie, to the even −1", vec![-1.0, -u]),
        ("−1 − u − 2^−1074", vec![-1.0, -u, -f64::from_bits(1)]),
        ("−Ω − 1 (Neal's 2024 fix): −Ω", vec![-f64::MAX, -1.0]),
        ("−Ω − 2^970: −∞", vec![-f64::MAX, -p2(970)]),
        (
            "−Ω − 2^970 + 2^−1074: −Ω",
            vec![-f64::MAX, -p2(970), f64::from_bits(1)],
        ),
        (
            "Ω + 2^970 − 2^−1074: Ω",
            vec![f64::MAX, p2(970), -f64::from_bits(1)],
        ),
        (
            "2^−1075 (half the smallest subnormal), as two quarters: 0",
            vec![f64::from_bits(1), -f64::from_bits(1)],
        ),
        (
            "−(2 − 2^−52)·2^k − its half unit: an all-ones negative significand",
            vec![-(2.0 - p2(-52)) * p2(40), -p2(-12)],
        ),
        (
            "all-ones negative significand, just above the tie",
            vec![-(2.0 - p2(-52)) * p2(40), -p2(-12), -p2(-200)],
        ),
        (
            "all-ones negative significand, just below the tie",
            vec![-(2.0 - p2(-52)) * p2(40), -p2(-12), p2(-200)],
        ),
        ("only −0: +0", vec![-0.0, -0.0]),
        ("nothing: +0", vec![]),
    ];
    for (what, terms) in &cases {
        check(what, terms);
    }
    // Ties at every binade: 2^k + 2^(k − 53), with an odd and an even significand.
    for k in -1021..=1023 {
        let even = p2(k);
        let odd = p2(k) * (1.0 + p2(-52));
        let half = p2(k - 53);
        check("tie even", &[even, half]);
        check("tie odd", &[odd, half]);
        check("tie odd negative", &[-odd, -half]);
        check("near tie", &[even, half, p2((k - 120).max(-1074))]);
    }
}

// ---------------------------------------------------------------------------------------------
// Order-freedom.

/// The same terms summed every way: one by one, as one slice, in random chunks (some long enough
/// for the large accumulator), and as accumulators merged in a random tree. All must have the
/// oracle's bits.
fn every_way(rng: &mut Rng, terms: &[f64]) {
    let expected = oracle(terms).to_bits();
    let mut shuffled = terms.to_vec();
    rng.shuffle(&mut shuffled);
    assert_eq!(
        by_add(&shuffled).to_bits(),
        expected,
        "permuted, one by one"
    );
    assert_eq!(
        by_slice(&shuffled).to_bits(),
        expected,
        "permuted, one slice"
    );
    assert_eq!(
        by_stream(&shuffled).to_bits(),
        expected,
        "permuted, one stream"
    );
    // Random chunks into separate accumulators.
    let mut parts = Vec::new();
    let mut rest: &[f64] = &shuffled;
    while !rest.is_empty() {
        let take = (1 + rng.below(2 * LARGE_SLICE as u64) as usize).min(rest.len());
        let (chunk, tail) = rest.split_at(take);
        let mut s = ExactSum::new();
        match rng.below(3) {
            0 => s.add_slice(chunk),
            1 => s.add_iter(chunk.iter().copied().filter(|_| true)),
            _ => {
                for &x in chunk {
                    s.add(x);
                }
            }
        }
        parts.push(s);
        rest = tail;
    }
    // Merge in a random tree: repeatedly merge two random accumulators.
    while parts.len() > 1 {
        let i = rng.below(parts.len() as u64) as usize;
        let a = parts.swap_remove(i);
        let j = rng.below(parts.len() as u64) as usize;
        parts[j].merge(&a);
    }
    let merged = parts.pop().unwrap_or_default();
    assert_eq!(
        merged.round_ieee().to_bits(),
        expected,
        "chunked and merged"
    );
}

#[test]
fn identical_bits_under_permutation_and_chunking() {
    let mut rng = Rng(5);
    for round in 0..60 {
        let n = [5, 100, 600, 3000][round % 4];
        let (lo, hi) = [(-1074, 1023), (-30, 30), (-1074, -1020), (1000, 1023)][(round / 4) % 4];
        let terms: Vec<f64> = (0..n).map(|_| double_in(&mut rng, lo, hi)).collect();
        every_way(&mut rng, &terms);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Any finite terms, any permutation, any chunking and merge order: the oracle's bits.
    #[test]
    fn property_order_free(
        raw in proptest::collection::vec(any::<u64>(), 0..1500),
        seed in any::<u64>(),
    ) {
        let terms: Vec<f64> = raw
            .iter()
            .map(|&b| f64::from_bits(b))
            .filter(|x| x.is_finite())
            .collect();
        every_way(&mut Rng(seed), &terms);
    }

    /// Merging is integer addition: the sum of two accumulators is the sum of all their terms.
    #[test]
    fn property_merge_is_addition(
        a in proptest::collection::vec(-1.0e300_f64..1.0e300, 0..800),
        b in proptest::collection::vec(-1.0e-300_f64..1.0e-300, 0..800),
    ) {
        let (mut sa, mut sb) = (ExactSum::new(), ExactSum::new());
        sa.add_slice(&a);
        sb.add_slice(&b);
        sa.merge(&sb);
        let all: Vec<f64> = a.iter().chain(&b).copied().collect();
        prop_assert_eq!(sa.round_ieee().to_bits(), oracle(&all).to_bits());
    }
}

#[test]
fn merge_with_itself_and_empty() {
    let mut rng = Rng(6);
    let terms: Vec<f64> = (0..700).map(|_| double_in(&mut rng, -100, 100)).collect();
    let mut s = ExactSum::new();
    s.add_slice(&terms);
    let copy = s.clone();
    s.merge(&copy);
    let doubled: Vec<f64> = terms.iter().chain(&terms).copied().collect();
    assert_eq!(s.round_ieee().to_bits(), oracle(&doubled).to_bits());
    let before = s.round_ieee().to_bits();
    s.merge(&ExactSum::new());
    assert_eq!(s.round_ieee().to_bits(), before);
    let mut empty = ExactSum::default();
    empty.merge(&ExactSum::new());
    assert_eq!(empty.round_ieee().to_bits(), 0);
    // Doubling a sum 60 times stays exact (the value grows to 2^60 times the sum).
    let mut s = ExactSum::new();
    s.add_slice(&[1.5, p2(-1074), -p2(900)]);
    for _ in 0..60 {
        let copy = s.clone();
        s.merge(&copy);
    }
    let expected = round_units(&(exact_units(&[1.5, p2(-1074), -p2(900)]) << 60));
    assert_eq!(s.round_ieee().to_bits(), expected.to_bits());
}

// ---------------------------------------------------------------------------------------------
// Special values, as Neal specifies.

#[test]
fn special_values() {
    let nan_with = |payload: u64, negative: bool| {
        f64::from_bits((u64::from(negative) << 63) | (0x7ff << 52) | payload)
    };
    let sum = |terms: &[f64]| {
        let (mut a, mut b) = (ExactSum::new(), ExactSum::new());
        for &x in terms {
            a.add(x);
        }
        let long: Vec<f64> = terms
            .iter()
            .copied()
            .cycle()
            .take(terms.len() * 600)
            .collect();
        b.add_slice(&long);
        let c: ExactSum = long.iter().copied().filter(|_| true).collect();
        assert_eq!(c.round_ieee().to_bits(), b.round_ieee().to_bits(), "stream");
        (a.round_ieee(), b.round_ieee(), a.round())
    };
    // The NaN with the largest payload wins, sign cleared, in any order, over infinities.
    for terms in [
        vec![1.0, nan_with(5, true), nan_with(9, false), f64::INFINITY],
        vec![
            nan_with(9, true),
            1.0,
            f64::NEG_INFINITY,
            nan_with(5, false),
        ],
    ] {
        let (a, b, r) = sum(&terms);
        assert_eq!(a.to_bits(), nan_with(9, false).to_bits());
        assert_eq!(b.to_bits(), nan_with(9, false).to_bits());
        assert_eq!(r, Err(NumError::NotFinite));
    }
    // Both infinities: the quiet NaN 0x7ff8…, on every target.
    let (a, b, r) = sum(&[f64::INFINITY, 2.0, f64::NEG_INFINITY]);
    assert_eq!(a.to_bits(), 0x7ff8_0000_0000_0000);
    assert_eq!(b.to_bits(), 0x7ff8_0000_0000_0000);
    assert_eq!(r, Err(NumError::NotFinite));
    // One infinity: it, even against a finite sum that overflows the other way.
    let (a, _, r) = sum(&[f64::INFINITY, -f64::MAX, -f64::MAX]);
    assert_eq!(a, f64::INFINITY);
    assert_eq!(r, Err(NumError::NotFinite));
    let (a, b, _) = sum(&[f64::NEG_INFINITY, 1.0]);
    assert_eq!((a, b), (f64::NEG_INFINITY, f64::NEG_INFINITY));
    // A finite overflow: ±∞ from round_ieee, NotFinite from round.
    let (a, _, r) = sum(&[f64::MAX, f64::MAX]);
    assert_eq!(a, f64::INFINITY);
    assert_eq!(r, Err(NumError::NotFinite));
    // Merging carries the special values.
    let mut x = ExactSum::new();
    x.add(1.0);
    let mut y = ExactSum::new();
    y.add(nan_with(3, true));
    x.merge(&y);
    assert_eq!(x.round_ieee().to_bits(), nan_with(3, false).to_bits());
}

// ---------------------------------------------------------------------------------------------
// Allowance boundaries.

#[test]
fn carry_and_chunk_count_boundaries() {
    // The small accumulator propagates every 2047 additions; the large one moves a chunk after
    // 4096 additions to it. Repeat one value (one chunk) across those counts, with and without a
    // second value in another chunk, and with alternating signs.
    let mut rng = Rng(7);
    for count in [
        2046, 2047, 2048, 4094, 4095, 4096, 4097, 8191, 8192, 8193, 12_289,
    ] {
        for x in [
            1.0,
            (2.0 - p2(-52)) * p2(1000),
            p2(-1074),
            f64::from_bits((1 << 52) - 1),
            -(1.0 + p2(-52)),
        ] {
            let mut terms = vec![x; count];
            check("one value repeated", &terms);
            terms.push(double_in(&mut rng, -1074, 1023));
            check("one value repeated and another", &terms);
            let alternating: Vec<f64> = (0..count)
                .map(|i| if i % 3 == 0 { -x } else { x })
                .collect();
            check("alternating", &alternating);
        }
    }
    // Every sign-and-exponent pattern filled past 4096 additions, in one slice.
    let mut terms = Vec::new();
    for e in (-1074..=1023).step_by(37) {
        for _ in 0..4100 {
            terms.push(with_exponent(rng.next() & 1 == 1, e, rng.next()));
        }
    }
    rng.shuffle(&mut terms);
    check("every pattern past its count", &terms);
}

// ---------------------------------------------------------------------------------------------
// Streams.

/// Yields `first`, then `None` once, then `second`: not fused, to check that `add_iter` stops at
/// the first `None` as a `for` loop does.
struct Gap {
    first: Vec<f64>,
    second: Vec<f64>,
    gapped: bool,
}

impl Iterator for Gap {
    type Item = f64;
    fn next(&mut self) -> Option<f64> {
        if let Some(x) = self.first.pop() {
            return Some(x);
        }
        if !self.gapped {
            self.gapped = true;
            return None;
        }
        self.second.pop()
    }
}

#[test]
fn streams_around_the_switch_to_the_large_accumulator() {
    let mut rng = Rng(8);
    let s = LARGE_SLICE;
    for n in [0, 1, s - 1, s, s + 1, 2 * s, 4096 + s, 20_000] {
        let terms: Vec<f64> = (0..n).map(|_| double_in(&mut rng, -1074, 1023)).collect();
        check(&format!("{n} terms"), &terms);
        // Appending to a sum that already holds terms, and a lower size bound that undercounts.
        let mut sum = ExactSum::new();
        sum.add(1.5);
        sum.extend(terms.iter().copied().filter(|_| true));
        sum.add_iter(terms.iter().copied().take(n / 2));
        let mut all = vec![1.5];
        all.extend_from_slice(&terms);
        all.extend_from_slice(&terms[..n / 2]);
        assert_eq!(sum.round_ieee().to_bits(), oracle(&all).to_bits(), "{n}");
    }
    // A stream stops at its first `None`, before or after the switch.
    for n in [3, s + 7] {
        let first: Vec<f64> = (0..n).map(|_| double_in(&mut rng, -60, 60)).collect();
        let second: Vec<f64> = (0..50).map(|_| double_in(&mut rng, -60, 60)).collect();
        let mut sum = ExactSum::new();
        sum.add_iter(Gap {
            first: first.clone(),
            second,
            gapped: false,
        });
        assert_eq!(sum.round_ieee().to_bits(), oracle(&first).to_bits(), "{n}");
    }
}
