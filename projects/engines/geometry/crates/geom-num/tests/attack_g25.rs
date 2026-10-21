//! Adversarial tests of piece G25: `geom_num::dd` and `geom_num::exact_sum`. Their only goal is
//! to break the two modules' documented contracts.
//!
//! Oracle: `num-bigint` and `num-rational`, a second library besides the `dashu` oracle of
//! `tests/dd.rs` and `tests/exact_sum.rs`, so a defect in one oracle cannot hide a defect in the
//! code. Nothing here calls the code under test to compute an expected value.
//!
//! What is attacked:
//!
//! - `DD`: operands at binade boundaries (a power-of-two leading part with a negative trailing
//!   part, where the unit below is half the unit above), trailing parts exactly at the half unit on
//!   both sides of a power of two, trailing parts exactly at and around the drop floor 2^−400 after
//!   scaling, the largest and smallest values a `DD` holds, results that land exactly on 2^−1022,
//!   2^−1074 and the overflow threshold, products of plain doubles (which must equal
//!   `exact_product` whenever that exists), the order against −0, and NaN anywhere.
//! - `ExactSum`: terms at every exponent field, so every split position k = E mod 32 in the
//!   chunks; results whose highest chunk is 0, 1 or 2 (the subnormal and first normal windows of
//!   the rounding); slices just below, at and just above `LARGE_SLICE`; merges with carries
//!   pending; the documented "lost" report, reached by doubling a sum past 2^1100; signalling-NaN
//!   payloads; and the sign of zero.

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
    clippy::many_single_char_names,
    clippy::similar_names
)]

use geom_num::dd::{
    ADD_BOUND, ADD_F64_BOUND, DIV_BOUND, DIV_F64_BOUND, MUL_BOUND, MUL_F64_BOUND, SQRT_BOUND,
};
use geom_num::exact_sum::LARGE_SLICE;
use geom_num::{DD, ExactSum, NumError};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// ---------------------------------------------------------------------------------------------
// Oracle.

/// A finite double exactly.
fn q(x: f64) -> BigRational {
    BigRational::from_float(x).unwrap()
}

/// 2^k exactly.
fn qp2(k: i64) -> BigRational {
    if k >= 0 {
        BigRational::from_integer(BigInt::one() << k as usize)
    } else {
        BigRational::new(BigInt::one(), BigInt::one() << (-k) as usize)
    }
}

/// The exact value of a `DD`.
fn qv(d: DD) -> BigRational {
    q(d.hi()) + q(d.lo())
}

/// A double from its sign, leading-bit exponent (−1074 to 1023) and fraction bits.
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

/// 2^k rounded to a double: 0 below 2^−1074 (2^−1075 is a tie, to the even 0).
fn p2(k: i64) -> f64 {
    assert!(k <= 1023, "2^{k} overflows");
    if k < -1074 {
        0.0
    } else {
        with_exponent(false, k, 0)
    }
}

/// The documented contract of a `DD` result: the invariant, and `|z − r| ≤ B·|r| + A`, or
/// `NotFinite` only above (1 − 2^−52)·2^1024.
fn holds(what: &str, got: Result<DD, NumError>, exact: &BigRational, b: f64, a: f64) {
    match got {
        Ok(z) => {
            let (h, l) = (z.hi(), z.lo());
            assert!(h.is_finite() && l.is_finite(), "{what}: not finite");
            assert_eq!(
                (h + l).to_bits(),
                h.to_bits(),
                "{what}: not normalized ({h:e}, {l:e})"
            );
            assert!(
                h.to_bits() != 1 << 63 && l.to_bits() != 1 << 63,
                "{what}: a −0 part"
            );
            let err = (qv(z) - exact).abs();
            assert!(
                err <= q(b) * exact.abs() + q(a),
                "{what}: ({h:e}, {l:e}) against {exact}"
            );
        }
        Err(NumError::NotFinite) => {
            let floor = (BigRational::one() - qp2(-52)) * qp2(1024);
            assert!(
                exact.abs() > floor,
                "{what}: overflow reported below the threshold"
            );
        }
        Err(e) => panic!("{what}: {e:?}"),
    }
}

/// Every operation of `DD` on `(x, y)` against its documented bound.
fn attack_pair(x: DD, y: DD) {
    let (vx, vy) = (qv(x), qv(y));
    let tiny = p2(-1074);
    let w = |op: &str| {
        format!(
            "{op}(({:e},{:e}),({:e},{:e}))",
            x.hi(),
            x.lo(),
            y.hi(),
            y.lo()
        )
    };
    holds(&w("add"), x.add(y), &(&vx + &vy), ADD_BOUND, 0.0);
    holds(&w("sub"), x.sub(y), &(&vx - &vy), ADD_BOUND, 0.0);
    holds(
        &w("add_f64"),
        x.add_f64(y.hi()),
        &(&vx + q(y.hi())),
        ADD_F64_BOUND,
        0.0,
    );
    holds(&w("mul"), x.mul(y), &(&vx * &vy), MUL_BOUND, tiny);
    holds(
        &w("mul_f64"),
        x.mul_f64(y.hi()),
        &(&vx * q(y.hi())),
        MUL_F64_BOUND,
        tiny,
    );
    if !vy.is_zero() {
        holds(&w("div"), x.div(y), &(&vx / &vy), DIV_BOUND, tiny);
        holds(
            &w("div_f64"),
            x.div_f64(y.hi()),
            &(&vx / q(y.hi())),
            DIV_F64_BOUND,
            tiny,
        );
    }
    if !vx.is_negative() {
        let z = x.sqrt().unwrap();
        let z2 = qv(z) * qv(z);
        let b = q(SQRT_BOUND);
        let one = BigRational::one();
        assert!(z2 >= (&one - &b) * (&one - &b) * &vx, "{}", w("sqrt"));
        assert!(z2 <= (&one + &b) * (&one + &b) * &vx, "{}", w("sqrt"));
    }
}

// ---------------------------------------------------------------------------------------------
// DD.

#[test]
fn dd_binade_boundaries_and_half_units() {
    // x = 2^k + t with t negative: the leading part is the power of two, and below it the unit is
    // half the unit above. t at the half unit below (a tie that stays at 2^k, even), just inside
    // it, the half unit above, and zero.
    let mut values = Vec::new();
    for k in [-1074, -1022, -1021, -969, 0, 53, 970, 1023] {
        let h = p2(k);
        for t in [
            0.0,
            -p2(k - 54),
            -p2(k - 55),
            -p2(k - 54) * (1.0 - p2(-52)),
            p2(k - 53),
            p2(k - 53) * (1.0 - p2(-52)),
        ] {
            if t != 0.0 && t.abs() < p2(-1074) {
                continue;
            }
            if let Ok(d) = DD::from_sum(h, t) {
                values.push(d);
                values.push(d.negate());
            }
        }
    }
    for (i, &x) in values.iter().enumerate() {
        for &y in values.iter().skip(i % 7).step_by(7) {
            attack_pair(x, y);
        }
    }
}

#[test]
fn dd_trailing_parts_at_the_drop_floor() {
    // After scaling the leading part to [1, 2), a trailing part below 2^−400 is dropped. Put the
    // trailing part at 2^−400 exactly, one unit either side, and far below (down to 2^−1074 next to
    // a leading part of 2^1023, the widest ratio a DD holds).
    let mut xs = Vec::new();
    for e in [-674, 0, 622, 1023] {
        let h = with_exponent(false, e, 0x000f_0f0f_0f0f_0f0f);
        for k in [-399, -400, -401] {
            let floor = p2(e + k);
            for t in [
                floor,
                floor * (1.0 + p2(-52)),
                floor * (1.0 - p2(-53)),
                -floor,
            ] {
                xs.push(DD::from_sum(h, t).unwrap());
            }
        }
        if e - 1100 < -1074 {
            xs.push(DD::from_sum(h, p2(-1074)).unwrap());
        }
    }
    let ys = [
        DD::from_f64(3.0).unwrap(),
        DD::from_sum(1.0, p2(-60)).unwrap(),
        DD::from_sum(p2(-500), -p2(-900)).unwrap(),
        DD::from_sum(p2(400), p2(-1)).unwrap(),
    ];
    for &x in &xs {
        for &y in &ys {
            attack_pair(x, y);
            attack_pair(y, x);
        }
    }
}

#[test]
fn dd_results_on_the_thresholds() {
    // Products and quotients of plain doubles landing exactly on 2^−1074, 2^−1022 and Ω, and
    // one step either side.
    let pairs = [
        (p2(-537), p2(-537)),
        (p2(-500), p2(-574)),
        (p2(-511), p2(-511)),
        (p2(-1000), p2(-22)),
        (f64::MAX, 1.0),
        (f64::MAX / 3.0, 3.0),
        (p2(1023), 2.0 - p2(-52)),
        (p2(512), p2(511) * (2.0 - p2(-52))),
        (3.0 * p2(-540), p2(-535) / 3.0),
    ];
    for (a, b) in pairs {
        for (s, t) in [(a, b), (-a, b), (a, -b)] {
            let (x, y) = (DD::from_f64(s).unwrap(), DD::from_f64(t).unwrap());
            attack_pair(x, y);
            // A product of plain doubles is exact whenever a DD holds it.
            if let Some(p) = DD::exact_product(s, t) {
                assert_eq!(qv(p), q(s) * q(t));
                assert_eq!(x.mul(y), Ok(p), "{s:e} · {t:e}");
                assert_eq!(x.mul_f64(t), Ok(p), "{s:e} · {t:e}");
            }
        }
    }
    // The largest and smallest values a DD holds.
    let top = DD::from_sum(f64::MAX, p2(969) * (1.0 - p2(-53))).unwrap();
    let bottom = DD::from_f64(p2(-1074)).unwrap();
    for &x in &[top, bottom, top.negate(), bottom.negate()] {
        for &y in &[top, bottom, DD::ONE, DD::from_f64(0.5).unwrap()] {
            attack_pair(x, y);
        }
    }
    assert_eq!(top.add(bottom).map(DD::hi), Ok(f64::MAX));
}

#[test]
fn dd_products_of_doubles_are_exact_when_held() {
    let mut s = 0x1234_5678_9abc_def0_u64;
    let mut next = || {
        s = s
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        s
    };
    for _ in 0..5000 {
        let ea = (next() % 2098) as i64 - 1074;
        let eb = ((next() % 400) as i64 - 200 - ea).clamp(-1074, 1023);
        let a = with_exponent(next() & 1 == 1, ea, next());
        let b = with_exponent(next() & 1 == 1, eb, next());
        let (x, y) = (DD::from_f64(a).unwrap(), DD::from_f64(b).unwrap());
        match DD::exact_product(a, b) {
            Some(p) => {
                assert_eq!(qv(p), q(a) * q(b));
                assert_eq!(x.mul(y), Ok(p), "{a:e} · {b:e}");
                assert_eq!(x.mul_f64(b), Ok(p), "{a:e} · {b:e}");
            }
            None => attack_pair(x, y),
        }
    }
}

#[test]
fn dd_order_eq_and_hash_agree() {
    let values = [
        DD::ZERO,
        DD::from_f64(-0.0).unwrap(),
        DD::from_sum(-0.0, -0.0).unwrap(),
        DD::from_sum(1.0, -p2(-54)).unwrap(),
        DD::from_sum(1.0, p2(-60)).unwrap(),
        DD::from_sum(1.0, -p2(-60)).unwrap(),
        DD::ONE,
        DD::from_sum(p2(-1074), 0.0).unwrap(),
        DD::from_sum(-p2(-1074), 0.0).unwrap(),
        DD::from_f64(f64::MAX).unwrap(),
        DD::from_f64(-f64::MAX).unwrap(),
    ];
    let hash = |d: &DD| {
        let mut h = DefaultHasher::new();
        d.hash(&mut h);
        h.finish()
    };
    for a in &values {
        for b in &values {
            let expected = qv(*a).cmp(&qv(*b));
            assert_eq!(a.cmp(b), expected, "{a:?} vs {b:?}");
            assert_eq!(a == b, expected == Ordering::Equal, "{a:?} vs {b:?}");
            if a == b {
                assert_eq!(hash(a), hash(b));
            }
        }
    }
    // −0 never survives into a DD.
    for d in &values {
        assert!(d.hi().to_bits() != 1 << 63 && d.lo().to_bits() != 1 << 63);
    }
}

// ---------------------------------------------------------------------------------------------
// ExactSum.

/// A finite double as an integer multiple of 2^−1074.
fn units(x: f64) -> BigInt {
    let bits = x.to_bits();
    let field = (bits >> 52) & 0x7ff;
    let fraction = bits & ((1 << 52) - 1);
    let m = if field == 0 {
        BigInt::from(fraction)
    } else {
        BigInt::from(fraction | (1 << 52)) << (field as usize - 1)
    };
    if bits >> 63 == 1 { -m } else { m }
}

/// N·2^−1074 rounded to the nearest double, ties to even, by a binary search on the doubles (a
/// different method from `tests/exact_sum.rs`), all in units of 2^−1074: the result is the double
/// nearest the value, the even one of two equally near, or ±∞ from 2^1024 − 2^970 up.
fn round_oracle(n: &BigInt) -> f64 {
    if n.is_zero() {
        return 0.0;
    }
    let negative = n.is_negative();
    let m = n.abs();
    // 2^1024 − 2^970 in units of 2^−1074.
    let limit = (BigInt::one() << 2098_usize) - (BigInt::one() << 2044_usize);
    if m >= limit {
        return if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    // The largest double at or below m, by bisection on the bit patterns (monotone in value).
    let (mut lo, mut hi) = (0_u64, 0x7ff0_0000_0000_0000_u64);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if units(f64::from_bits(mid)) <= m {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let below = f64::from_bits(lo);
    let above = f64::from_bits(lo + 1);
    let r = if units(below) == m || above.is_infinite() {
        below
    } else {
        let (d_below, d_above) = (&m - units(below), units(above) - &m);
        match d_below.cmp(&d_above) {
            Ordering::Less => below,
            Ordering::Greater => above,
            Ordering::Equal => {
                if lo & 1 == 0 {
                    below
                } else {
                    above
                }
            }
        }
    };
    if negative { -r } else { r }
}

/// The oracle's sum of finite terms.
fn oracle(terms: &[f64]) -> f64 {
    let mut n = BigInt::zero();
    for &x in terms {
        n += units(x);
    }
    round_oracle(&n)
}

/// `ExactSum` both ways against the oracle.
fn sums_to(what: &str, terms: &[f64]) {
    let expected = oracle(terms).to_bits();
    let mut a = ExactSum::new();
    for &x in terms {
        a.add(x);
    }
    let mut b = ExactSum::new();
    b.add_slice(terms);
    assert_eq!(a.round_ieee().to_bits(), expected, "{what}: one by one");
    assert_eq!(b.round_ieee().to_bits(), expected, "{what}: by slice");
}

#[test]
fn exact_sum_every_exponent_field() {
    // Every exponent field 0..2046, so every split position k = E mod 32 and every chunk j, with
    // significands of all ones and of a single bit, alone and against a neighbour.
    for e in -1074..=1023_i64 {
        let ones = with_exponent(false, e, u64::MAX);
        let one = p2(e);
        sums_to("one term", &[ones]);
        sums_to("two terms", &[ones, one]);
        sums_to("cancelling", &[ones, -one, p2(-1074)]);
        sums_to("negative", &[-ones, -one]);
    }
}

#[test]
fn exact_sum_rounding_windows() {
    // Results whose highest nonzero chunk is 0, 1 or 2 (values near 2^−1075, 2^−1043 and 2^−1011),
    // so the rounding's window holds one, two or three chunks.
    for e in [
        -1074, -1060, -1044, -1043, -1042, -1023, -1022, -1012, -1011, -1010, -980,
    ] {
        for f in [0_u64, 1, 0xf_ffff_ffff_ffff, 0x8_0000_0000_0001] {
            let x = with_exponent(false, e, f);
            sums_to("window", &[x]);
            sums_to("window", &[x, p2(-1074)]);
            sums_to("window", &[x, -p2(-1074)]);
            sums_to("window", &[-x, p2(e.max(-1074)), p2(-1074)]);
        }
    }
    // A 54-bit exact sum just above, at and below each tie, at every height of the window.
    for e in (-1020..=1020).step_by(17) {
        let base = p2(e);
        let half = p2(e - 53);
        for sticky in [0.0, p2((e - 200).max(-1074)), -p2((e - 200).max(-1074))] {
            sums_to("tie", &[base, half, sticky]);
            sums_to("tie odd", &[base * (1.0 + p2(-52)), half, sticky]);
            sums_to("tie negative", &[-base, -half, -sticky]);
        }
    }
}

#[test]
fn exact_sum_slices_around_the_large_threshold_and_merges_with_pending_carries() {
    let mut s = 0x0bad_5eed_u64;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    for n in [
        LARGE_SLICE - 1,
        LARGE_SLICE,
        LARGE_SLICE + 1,
        2 * LARGE_SLICE + 7,
    ] {
        let terms: Vec<f64> = (0..n)
            .map(|_| with_exponent(next() & 1 == 1, (next() % 2098) as i64 - 1074, next()))
            .collect();
        sums_to("around LARGE_SLICE", &terms);
    }
    // Merge accumulators whose allowances are nearly used up: 2046 additions each.
    let terms: Vec<f64> = (0_i64..2046 * 3)
        .map(|i| with_exponent(i % 2 == 0, i % 300 - 150, next()))
        .collect();
    let mut parts: Vec<ExactSum> = terms
        .chunks(2046)
        .map(|c| {
            let mut s = ExactSum::new();
            for &x in c {
                s.add(x);
            }
            s
        })
        .collect();
    let mut total = parts.remove(0);
    for p in &parts {
        total.merge(p);
        // And keep adding after the merge, up to its allowance and past it.
        for &x in &terms[..2047] {
            total.add(x);
        }
    }
    let mut all = terms.clone();
    for _ in 0..parts.len() {
        all.extend_from_slice(&terms[..2047]);
    }
    assert_eq!(total.round_ieee().to_bits(), oracle(&all).to_bits());
}

#[test]
fn exact_sum_lost_is_reported_not_wrong() {
    // The accumulator holds magnitudes below 2^1100. Doubling Ω by self-merges passes that after
    // about 77 doublings; until then the sum is +∞ (an overflow of the double), after it the
    // documented report, a NaN with every payload bit set; never a finite wrong answer.
    let mut s = ExactSum::new();
    s.add(f64::MAX);
    let mut lost_at = None;
    for k in 1..=90 {
        let copy = s.clone();
        s.merge(&copy);
        let r = s.round_ieee();
        if r.is_nan() {
            assert_eq!(r.to_bits(), 0x7fff_ffff_ffff_ffff);
            lost_at.get_or_insert(k);
        } else {
            assert!(lost_at.is_none(), "a lost sum came back at doubling {k}");
            assert_eq!(r, f64::INFINITY, "doubling {k}");
        }
        assert_eq!(s.round(), Err(NumError::NotFinite));
    }
    let k = lost_at.expect("never lost");
    assert!((70..=80).contains(&k), "lost at doubling {k}");
    // Once lost, it stays lost (the value is gone), even after cancelling terms.
    s.add(-f64::MAX);
    assert!(s.round_ieee().is_nan());
}

#[test]
fn exact_sum_nan_payloads_and_zero_signs() {
    // A signalling NaN's payload (top fraction bit clear) is kept as it is, its sign cleared.
    let snan = f64::from_bits(0xfff0_0000_0000_0001);
    let mut s = ExactSum::new();
    s.add(snan);
    s.add(1.0);
    assert_eq!(s.round_ieee().to_bits(), 0x7ff0_0000_0000_0001);
    // The largest payload, whatever the order and the path.
    let nans = [
        0x7ff0_0000_0000_0003_u64,
        0xfff8_0000_0000_0000,
        0x7ff4_0000_0000_0000,
    ];
    let terms: Vec<f64> = nans
        .iter()
        .map(|&b| f64::from_bits(b))
        .cycle()
        .take(3 * LARGE_SLICE)
        .collect();
    let mut a = ExactSum::new();
    a.add_slice(&terms);
    assert_eq!(a.round_ieee().to_bits(), 0x7ff8_0000_0000_0000);
    // Zeros: +0, from −0 terms, from an exact cancellation, and from nothing.
    for terms in [
        vec![-0.0; 3],
        vec![-0.0; 3 * LARGE_SLICE],
        vec![-1.5, 1.5],
        vec![],
    ] {
        let mut s = ExactSum::new();
        s.add_slice(&terms);
        assert_eq!(s.round_ieee().to_bits(), 0, "{terms:?}");
        assert_eq!(s.round().map(|r| r.get().to_bits()), Ok(0));
    }
}

// =============================================================================================
// Second adversary pass. Same oracle (num-bigint, num-rational); nothing below computes an
// expected value with the code under test, except where a test compares the code with itself
// (symmetry, scaling, order), which the contract makes exact.

use geom_num::math;
use num_bigint::BigUint;
use num_traits::ToPrimitive;

/// `SplitMix64`.
struct Mix(u64);

impl Mix {
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
    fn coin(&mut self) -> bool {
        self.next() & 1 == 1
    }
    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            v.swap(i, j);
        }
    }
}

/// The exponent of the leading bit of a finite nonzero double.
fn lead(x: f64) -> i64 {
    let bits = x.to_bits() & !(1 << 63);
    let field = (bits >> 52) as i64;
    if field == 0 {
        63 - i64::from(bits.leading_zeros()) - 1074
    } else {
        field - 1023
    }
}

/// x·2^k exactly, in two steps (each intermediate lies between x and the result), or `None` if
/// x or the result is not a normal double (zero aside).
fn scale_exact(x: f64, k: i64) -> Option<f64> {
    if x == 0.0 {
        return Some(0.0);
    }
    let e = lead(x) + k;
    if !(-1022..=1023).contains(&e) || !(-1022..=1023).contains(&lead(x)) {
        return None;
    }
    let half = k / 2;
    Some(x * p2(half) * p2(k - half))
}

/// A `DD` scaled by 2^k exactly, or `None` when a part would leave the normal range.
fn dd_scaled(d: DD, k: i64) -> Option<DD> {
    let h = scale_exact(d.hi(), k)?;
    let l = scale_exact(d.lo(), k)?;
    let s = DD::from_sum(h, l).ok()?;
    assert_eq!((s.hi(), s.lo()), (h, l));
    Some(s)
}

/// A random `DD` with its leading bit at 2^e: a random leading part and a trailing part from half a
/// unit of it down to 2^−1074, or zero.
fn adv_dd(rng: &mut Mix, e: i64) -> DD {
    let h = with_exponent(rng.coin(), e, rng.next());
    let top = e - 54;
    let lo = if top < -1074 || rng.below(8) == 0 {
        0.0
    } else {
        let le = (top - rng.below(80) as i64).max(-1074);
        with_exponent(rng.coin(), le, rng.next())
    };
    DD::from_sum(h, lo).unwrap()
}

/// n·2^−s rounded to the nearest double, ties to even, with the sign of n (so a nonzero n too
/// small for the subnormals gives a zero of its sign); ±∞ on overflow; +0 for n = 0.
fn round_int(n: &BigInt, s: i64) -> f64 {
    if n.is_zero() {
        return 0.0;
    }
    let negative = n.is_negative();
    let m: BigUint = n.magnitude().clone();
    let e = m.bits() as i64 - 1 - s;
    let quantum = (e - 52).max(-1074);
    let shift = quantum + s;
    let mut kept;
    let up;
    if shift <= 0 {
        kept = &m << ((-shift) as usize);
        up = false;
    } else {
        kept = &m >> (shift as usize);
        let rest = &m - (&kept << (shift as usize));
        let half = BigUint::one() << ((shift - 1) as usize);
        up = match rest.cmp(&half) {
            Ordering::Greater => true,
            Ordering::Equal => kept.bit(0),
            Ordering::Less => false,
        };
    }
    if up {
        kept += 1_u32;
    }
    let kept = kept.to_u64().unwrap();
    let bits = (((quantum + 1074) as u64) << 52) + kept;
    let r = if bits >> 52 >= 0x7ff {
        f64::INFINITY
    } else {
        f64::from_bits(bits)
    };
    if negative { -r } else { r }
}

/// A finite double as a multiple of 2^−s (s ≥ 1074).
fn at_scale(x: f64, s: i64) -> BigInt {
    units(x) << ((s - 1074) as usize)
}

// ---------------------------------------------------------------------------------------------
// DD: rounding to integers at the ends of i128.

/// The oracle of `round_shifted`: hi + lo − offset rounded to nearest, ties to even, and whether
/// it was an integer; `None` when that integer is not an i128.
fn round_shifted_oracle(d: DD, offset: i128) -> Option<(i128, bool)> {
    let v = qv(d) - BigRational::from_integer(BigInt::from(offset));
    let f = v.floor();
    let frac = &v - &f;
    let half = BigRational::new(BigInt::one(), BigInt::from(2));
    let fl = f.to_integer();
    let n = match frac.cmp(&half) {
        Ordering::Less => fl,
        Ordering::Greater => fl + 1,
        Ordering::Equal => {
            if (&fl % 2_i32).is_zero() {
                fl
            } else {
                fl + 1
            }
        }
    };
    n.to_i128().map(|n| (n, frac.is_zero()))
}

#[test]
fn adv_round_shifted_answers_whenever_the_result_fits_an_i128() {
    // The contract: `None` only if |hi| > 2^100 or the result does not fit an i128. Three results
    // that fit, each at an end of i128:
    // −4.5 − (2^127 − 4) = −2^127 − 1/2, a tie, to the even −2^127 = i128::MIN;
    // 3.5 − 2^−60 − (4 − 2^127) = 2^127 − 1/2 − 2^−60, nearest i128::MAX;
    // −3.5 + 2^−60 − (2^127 − 3) = −2^127 − 1/2 + 2^−60, nearest i128::MIN.
    let cases = [
        (DD::from_f64(-4.5).unwrap(), i128::MAX - 3, i128::MIN),
        (
            DD::from_sum(3.5, -p2(-60)).unwrap(),
            i128::MIN + 4,
            i128::MAX,
        ),
        (
            DD::from_sum(-3.5, p2(-60)).unwrap(),
            i128::MAX - 2,
            i128::MIN,
        ),
    ];
    let mut failures = Vec::new();
    for (d, offset, want) in cases {
        assert_eq!(round_shifted_oracle(d, offset), Some((want, false)));
        let got = d.round_shifted(offset).map(|r| (r.value(), r.is_exact()));
        if got != Some((want, false)) {
            failures.push(format!(
                "({:e}, {:e}).round_shifted({offset}) = {got:?}, want Some(({want}, false))",
                d.hi(),
                d.lo()
            ));
        }
    }
    // And systematically: small values with offsets at both ends of i128.
    let mut rng = Mix(0x7273_6869_6674);
    let mut sweep = 0;
    for _ in 0..4000 {
        let e = rng.range(-3, 6);
        let d = adv_dd(&mut rng, e);
        // Half-integers and integers too.
        let d = if rng.below(3) == 0 {
            DD::from_f64((rng.range(-40, 40) as f64) * 0.5).unwrap()
        } else {
            d
        };
        let k = i128::from(rng.range(0, 12));
        let offset = if rng.coin() {
            i128::MIN + k
        } else {
            i128::MAX - k
        };
        let want = round_shifted_oracle(d, offset);
        let got = d.round_shifted(offset).map(|r| (r.value(), r.is_exact()));
        if got != want {
            sweep += 1;
            if failures.len() < 12 {
                failures.push(format!(
                    "({:e}, {:e}).round_shifted({offset}) = {got:?}, want {want:?}",
                    d.hi(),
                    d.lo()
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "round_shifted refuses results that fit an i128 ({sweep} of 4000 in the sweep):\n{}",
        failures.join("\n")
    );
}

// ---------------------------------------------------------------------------------------------
// DD: exact symmetries. Rounding to nearest, ties to even, is symmetric and commutes with scaling
// by powers of two away from underflow, and every multiplicative operation scales its operands to
// the same unit range, so these must hold bit for bit.

#[test]
fn adv_dd_sign_and_power_of_two_scaling_are_exact_symmetries() {
    let mut rng = Mix(0x7363_616c_6573);
    let mut scaled = 0;
    for _ in 0..12_000 {
        let ex = rng.range(-300, 300);
        let ey = rng.range(-300, 300);
        let x = adv_dd(&mut rng, ex);
        let y = adv_dd(&mut rng, ey);
        let fy = y.hi();
        let ops = [
            ("mul", x.mul(y), x.negate().mul(y), x.mul(y.negate())),
            (
                "mul_f64",
                x.mul_f64(fy),
                x.negate().mul_f64(fy),
                x.mul_f64(-fy),
            ),
            ("div", x.div(y), x.negate().div(y), x.div(y.negate())),
            (
                "div_f64",
                x.div_f64(fy),
                x.negate().div_f64(fy),
                x.div_f64(-fy),
            ),
            (
                "add",
                x.add(y),
                x.negate().add(y.negate()),
                y.negate().add(x.negate()),
            ),
            (
                "add_f64",
                x.add_f64(fy),
                x.negate().add_f64(-fy),
                x.negate().sub_f64(fy),
            ),
        ];
        for (what, base, a, b) in ops {
            let base = base.unwrap();
            assert_eq!(a.unwrap(), base.negate(), "{what}: sign of x, {x:?} {y:?}");
            assert_eq!(b.unwrap(), base.negate(), "{what}: sign of y, {x:?} {y:?}");
        }
        assert_eq!(x.add(y).unwrap(), y.add(x).unwrap(), "add commutes");
        // Scaling: x·2^a and y·2^b.
        let (a, b) = (rng.range(-700, 700), rng.range(-700, 700));
        let (Some(xs), Some(ys)) = (dd_scaled(x, a), dd_scaled(y, b)) else {
            continue;
        };
        let pairs = [
            ("mul", x.mul(y), xs.mul(ys), a + b),
            ("div", x.div(y), xs.div(ys), a - b),
            ("mul_f64", x.mul_f64(fy), xs.mul_f64(ys.hi()), a + b),
            ("div_f64", x.div_f64(fy), xs.div_f64(ys.hi()), a - b),
        ];
        for (what, base, got, k) in pairs {
            if let Some(want) = dd_scaled(base.unwrap(), k) {
                scaled += 1;
                assert_eq!(
                    got.unwrap(),
                    want,
                    "{what} scaled by 2^{a}, 2^{b}: {x:?} {y:?}"
                );
            }
        }
        if let Some(ya) = dd_scaled(y, a) {
            if let Some(want) = dd_scaled(x.add(y).unwrap(), a) {
                assert_eq!(xs.add(ya).unwrap(), want, "add scaled by 2^{a}");
            }
        }
        let xa = x.abs();
        if let Some(x2) = dd_scaled(xa, 2 * (a / 2)) {
            if let Some(want) = dd_scaled(xa.sqrt().unwrap(), a / 2) {
                assert_eq!(x2.sqrt().unwrap(), want, "sqrt scaled by 2^{}", 2 * (a / 2));
            }
        }
    }
    assert!(scaled > 20_000, "too few scaled comparisons: {scaled}");
}

// ---------------------------------------------------------------------------------------------
// DD: the published inputs that come closest to each bound, moved to the ends of the exponent
// range, where the scaling and the scaled-back rounding do the work.

#[test]
fn adv_dd_published_worst_cases_at_the_ends_of_the_range() {
    let r = |n: i64, k: i64| BigRational::from_integer(BigInt::from(n)) * qp2(k);
    let pair = |h: BigRational, l: BigRational| {
        let (h, l) = (h.to_f64().unwrap(), l.to_f64().unwrap());
        let d = DD::from_sum(h, l).unwrap();
        assert_eq!((d.hi(), d.lo()), (h, l));
        d
    };
    let u2 = qp2(-106);
    let tiny = p2(-1074);
    let mr_mul = (
        pair(
            r(2_251_799_825_991_851, -51),
            r(9_007_199_203_085_987, -106),
        ),
        pair(
            r(4_503_599_627_471_459, -52),
            r(4_503_599_627_284_651, -105),
        ),
    );
    let jmp_div = (
        pair(r(4_528_288_502_329_187, 0), r(1_125_391_118_633_487, -51)),
        pair(r(4_522_593_432_466_394, 0), r(-9_006_008_290_016_505, -54)),
    );
    let jmp_div_f64 = (
        pair(r(4_588_860_379_563_012, 0), r(-4_474_949_195_791_253, -53)),
        DD::from_f64(4_578_284_000_230_917.0).unwrap(),
    );
    let jmp_mul_f64 = (
        pair(r(4_505_619_370_757_448, 0), r(-9_003_265_529_542_491, -54)),
        DD::from_f64(4_511_413_997_183_120.0).unwrap(),
    );
    // (name, operands, operation: 0 mul, 1 mul_f64, 2 div, 3 div_f64, published error in u²)
    let cases = [
        ("DWTimesDW3", mr_mul, 0, 3.997),
        ("DWDivDW3", jmp_div, 2, 5.922),
        ("DWDivFP3", jmp_div_f64, 3, 2.951),
        ("DWTimesFP3", jmp_mul_f64, 1, 1.984),
    ];
    let run = |op: usize, x: DD, y: DD| -> (Result<DD, NumError>, BigRational, f64) {
        let (vx, vy) = (qv(x), qv(y));
        match op {
            0 => (x.mul(y), &vx * &vy, MUL_BOUND),
            1 => (x.mul_f64(y.hi()), &vx * &vy, MUL_F64_BOUND),
            2 => (x.div(y), &vx / &vy, DIV_BOUND),
            _ => (x.div_f64(y.hi()), &vx / &vy, DIV_F64_BOUND),
        }
    };
    let mut compared = 0;
    for (name, (x, y), op, err) in cases {
        let (base, exact, _) = run(op, x, y);
        let base = base.unwrap();
        let rel = ((qv(base) - &exact).abs() / exact.abs() / &u2)
            .to_f64()
            .unwrap();
        assert!(rel > err && rel < err + 0.001, "{name}: {rel}");
        let ex = lead(x.hi());
        let ey = lead(y.hi());
        for a in [
            -1074 - ex,
            -1060 - ex,
            -1022 - ex,
            -969 - ex,
            -600,
            0,
            600,
            1023 - ex,
        ] {
            for b in [-1074 - ey, -1022 - ey, -969 - ey, -500, 0, 500, 1023 - ey] {
                let sc = |v: f64, k: i64| v * p2(k / 2) * p2(k - k / 2);
                let xs = DD::from_sum(sc(x.hi(), a), sc(x.lo(), a));
                let ys = DD::from_sum(sc(y.hi(), b), sc(y.lo(), b));
                let (Ok(xs), Ok(ys)) = (xs, ys) else { continue };
                if (op == 1 || op == 3) && ys.lo() != 0.0 {
                    continue;
                }
                if ys.hi() == 0.0 {
                    continue;
                }
                let (got, exact, bound) = run(op, xs, ys);
                holds(&format!("{name} at 2^{a}, 2^{b}"), got, &exact, bound, tiny);
                // Where nothing was rounded away by scaling, the result is the base one scaled.
                let k = if op < 2 { a + b } else { a - b };
                if qv(xs) == qv(x) * qp2(a) && qv(ys) == qv(y) * qp2(b) {
                    if let Some(want) = dd_scaled(base, k) {
                        compared += 1;
                        assert_eq!(got.unwrap(), want, "{name} at 2^{a}, 2^{b}");
                    }
                }
            }
        }
    }
    assert!(compared > 20, "{compared}");
}

// ---------------------------------------------------------------------------------------------
// DD: overflow. The module promises NotFinite only when |r| > (1 − 2^−52)·2^1024, and every
// other result within its bound; `holds` checks both.

#[test]
fn adv_dd_results_around_the_overflow_threshold() {
    let mut rng = Mix(0x6f76_6572_666c);
    let tiny = p2(-1074);
    let big = |rng: &mut Mix| -> DD {
        let j = rng.below(12) as f64;
        let h = f64::MAX - j * p2(971);
        let lo = if rng.below(3) == 0 {
            0.0
        } else {
            with_exponent(rng.coin(), rng.range(880, 968), rng.next())
        };
        DD::from_sum(h, lo).unwrap()
    };
    let near_one = |rng: &mut Mix| -> DD {
        let h = if rng.coin() {
            1.0 + (rng.below(8) as f64) * p2(-52)
        } else {
            1.0 - (rng.below(8) as f64) * p2(-53)
        };
        let lo = if rng.below(3) == 0 {
            0.0
        } else {
            with_exponent(rng.coin(), rng.range(-130, -56), rng.next())
        };
        DD::from_sum(h, lo).unwrap()
    };
    for _ in 0..1200 {
        let x = big(&mut rng);
        let y = near_one(&mut rng);
        let (vx, vy) = (qv(x), qv(y));
        holds("mul", x.mul(y), &(&vx * &vy), MUL_BOUND, tiny);
        holds("mul swapped", y.mul(x), &(&vx * &vy), MUL_BOUND, tiny);
        holds(
            "mul_f64",
            x.mul_f64(y.hi()),
            &(&vx * q(y.hi())),
            MUL_F64_BOUND,
            tiny,
        );
        holds("div", x.div(y), &(&vx / &vy), DIV_BOUND, tiny);
        holds(
            "div_f64",
            x.div_f64(y.hi()),
            &(&vx / q(y.hi())),
            DIV_F64_BOUND,
            tiny,
        );
        let s = x.sqrt().unwrap();
        holds("sqrt²", s.mul(s), &(qv(s) * qv(s)), MUL_BOUND, tiny);
        // Sums that approach the threshold from below and pass it.
        let t = with_exponent(rng.coin(), rng.range(940, 972), rng.next());
        let td = DD::from_sum(t, 0.0).unwrap();
        holds("add", x.add(td), &(&vx + q(t)), ADD_BOUND, 0.0);
        holds("add_f64", x.add_f64(t), &(&vx + q(t)), ADD_F64_BOUND, 0.0);
        holds("sub", x.negate().sub(td), &(-&vx - q(t)), ADD_BOUND, 0.0);
        let z = big(&mut rng);
        holds("big − big", x.sub(z), &(&vx - qv(z)), ADD_BOUND, 0.0);
    }
    // The largest value a DD holds, Ω + 2^969 − 2^916: every operation that keeps it must answer.
    let top = DD::from_sum(f64::MAX, p2(969) - p2(916)).unwrap();
    assert_eq!(top.mul(DD::ONE), Ok(top));
    assert_eq!(top.mul_f64(1.0), Ok(top));
    assert_eq!(top.div(DD::ONE), Ok(top));
    assert_eq!(top.div_f64(1.0), Ok(top));
    assert_eq!(top.add(DD::ZERO), Ok(top));
    assert_eq!(top.add_f64(0.0), Ok(top));
    let half = top.mul_f64(0.5).unwrap();
    assert_eq!(
        qv(half),
        qv(top) / BigRational::from_integer(BigInt::from(2))
    );
    assert_eq!(half.mul_f64(2.0), Ok(top));
    assert_eq!(half.div_f64(0.5), Ok(top));
    assert_eq!(half.mul(DD::from_f64(2.0).unwrap()), Ok(top));
}

// ---------------------------------------------------------------------------------------------
// DD: division and square root near zero and near overflow, over the whole exponent range.

#[test]
fn adv_dd_division_and_root_at_the_extremes() {
    let mut rng = Mix(0x6469_7673_7172);
    let edges = [
        -1074, -1073, -1060, -1023, -1022, -1021, -1000, -969, -968, -537, -1, 0, 1, 537, 968,
        1000, 1022, 1023,
    ];
    let tiny = p2(-1074);
    let one = BigRational::one();
    let b = q(SQRT_BOUND);
    let root_ok = |x: DD, z: DD| {
        let v = qv(x);
        let z2 = qv(z) * qv(z);
        z2 >= (&one - &b) * (&one - &b) * &v && z2 <= (&one + &b) * (&one + &b) * &v
    };
    for &ex in &edges {
        for &ey in &edges {
            for _ in 0..2 {
                let x = adv_dd(&mut rng, ex);
                let y = adv_dd(&mut rng, ey);
                let (vx, vy) = (qv(x), qv(y));
                holds("div", x.div(y), &(&vx / &vy), DIV_BOUND, tiny);
                holds(
                    "div_f64",
                    x.div_f64(y.hi()),
                    &(&vx / q(y.hi())),
                    DIV_F64_BOUND,
                    tiny,
                );
                holds("mul", x.mul(y), &(&vx * &vy), MUL_BOUND, tiny);
            }
        }
        // The root: relative bound alone, no absolute part (module documentation).
        for _ in 0..16 {
            let x = adv_dd(&mut rng, ex).abs();
            let z = x.sqrt().unwrap();
            assert_eq!(
                (z.hi() + z.lo()).to_bits(),
                z.hi().to_bits(),
                "sqrt normalized"
            );
            assert!(root_ok(x, z), "sqrt({x:?}) = {z:?}");
        }
    }
    // The roots of the smallest subnormals, k·2^−1074.
    for k in 1_u64..=400 {
        let x = DD::from_f64(f64::from_bits(k)).unwrap();
        let z = x.sqrt().unwrap();
        assert!(root_ok(x, z), "sqrt of {k}·2^-1074 = {z:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// The fused multiply-add the kernel's TwoProduct and every DD product rest on: correctly rounded,
// so the same bits on every target (native here: the processor's instruction where `libm` finds
// one; on wasm32: `libm`'s own routine). Hard cases: exact ties, ties broken by a sticky bit far
// below, cancellation, subnormal and overflowing results.

#[test]
fn adv_fma_is_correctly_rounded() {
    let mut rng = Mix(0x666d_615f_6f72);
    let check = |a: f64, b: f64, c: f64| {
        let n = units(a) * units(b) + at_scale(c, 2148);
        let want = round_int(&n, 2148);
        let got = math::fma(a, b, c);
        if want.is_infinite() {
            assert_eq!(got, Err(NumError::NotFinite), "fma({a:e}, {b:e}, {c:e})");
        } else if n.is_zero() {
            // An exact zero of nonzero terms is +0 under round to nearest.
            assert_eq!(got.unwrap().get().to_bits(), 0, "fma({a:e}, {b:e}, {c:e})");
        } else {
            let got = got.unwrap().get();
            assert_eq!(
                got.to_bits(),
                want.to_bits(),
                "fma({a:e}, {b:e}, {c:e}) = {got:e}, want {want:e}"
            );
        }
    };
    // Exact ties of the product alone: 3·(2^52 + odd) needs 54 bits and ends in a 1.
    for i in 0..300_u64 {
        let a = 3.0 * p2(rng.range(-500, 500));
        let b = (p2(52) + (2 * i + 1) as f64) * p2(rng.range(-500, 450));
        for c in [p2(-1074), -p2(-1074), p2(-700), -p2(-700)] {
            check(a, b, c);
            check(-a, b, c);
        }
    }
    // Random products with addends at every alignment, and the error term itself.
    for _ in 0..15_000 {
        let a = with_exponent(rng.coin(), rng.range(-560, 500), rng.next());
        let b = with_exponent(rng.coin(), rng.range(-560, 500), rng.next());
        let p = a * b;
        if p == 0.0 || !p.is_finite() {
            continue;
        }
        let ep = lead(p);
        let d = rng.range(-60, 140);
        let ec = (ep - d).clamp(-1074, 1023);
        let c_rand = with_exponent(rng.coin(), ec, rng.next());
        let half_ulp = p2((ep - 53).max(-1074));
        for c in [
            c_rand,
            -p,
            -p + half_ulp,
            -p - half_ulp,
            half_ulp,
            -half_ulp,
            -p * (1.0 + p2(-52)),
        ] {
            if c.is_finite() && c != 0.0 {
                check(a, b, c);
            }
        }
    }
    // Results in the subnormal range, and around the overflow threshold.
    for _ in 0..3000 {
        let a = with_exponent(rng.coin(), rng.range(-540, -500), rng.next());
        let b = with_exponent(rng.coin(), rng.range(-540, -500), rng.next());
        let c = with_exponent(rng.coin(), rng.range(-1074, -1000), rng.next());
        check(a, b, c);
        let a = with_exponent(false, rng.range(500, 523), rng.next());
        let b = with_exponent(false, 1023 - lead(a), rng.next());
        let c = with_exponent(rng.coin(), rng.range(960, 1023), rng.next());
        check(a, b, c);
    }
}

#[test]
fn adv_two_prod_fma_at_its_switch_points() {
    // `two_prod_fma` takes the fused multiply-add from |p| ≥ 2^−968 and Dekker's product below;
    // its contract is `eft::two_prod`'s bits for every input. Products just either side of 2^−968,
    // of 2^−1022, of 2^−1074 and of the overflow threshold, with subnormal factors among them.
    let mut rng = Mix(0x3270_726f_6466);
    for target in [-969_i64, -968, -967, -1023, -1022, -1074, -1075, 1022, 1023] {
        for _ in 0..1500 {
            let ea = rng.range((target - 1023).max(-1074), (target + 1074).min(1023));
            let eb = (target - ea).clamp(-1074, 1023);
            let a = with_exponent(rng.coin(), ea, rng.next());
            let b = with_exponent(rng.coin(), eb, rng.next());
            let (p, e) = geom_num::dd::two_prod_fma(a, b);
            let (p2_, e2) = geom_num::eft::two_prod(a, b);
            assert_eq!(
                (p.to_bits(), e.to_bits()),
                (p2_.to_bits(), e2.to_bits()),
                "two_prod_fma({a:e}, {b:e})"
            );
            if p.is_finite() && e.is_finite() {
                assert_eq!(q(p) + q(e), q(a) * q(b), "two_prod_fma({a:e}, {b:e}) exact");
                assert_eq!(
                    DD::exact_product(a, b).map(|d| (d.hi(), d.lo())),
                    Some((p, e))
                );
            } else {
                assert_eq!(DD::exact_product(a, b), None, "{a:e} · {b:e}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// ExactSum: ties and overflow at the top of the range, through every path.

/// Sums `terms` every way the contract covers: one by one; by one slice (the large path, padded
/// with cancelling pairs past `LARGE_SLICE`), shuffled; and split into pieces (some added one by
/// one, some by slice) merged in a random order. Every way must give `want`'s bits.
fn every_way(rng: &mut Mix, what: &str, terms: &[f64], want: f64) {
    assert_eq!(
        oracle(terms).to_bits(),
        want.to_bits(),
        "{what}: the oracle disagrees with the stated answer"
    );
    let mut one = ExactSum::new();
    for &x in terms {
        one.add(x);
    }
    assert_eq!(one.round_ieee().to_bits(), want.to_bits(), "{what}: add");
    let mut padded = terms.to_vec();
    while padded.len() < 2 * LARGE_SLICE + 3 {
        let x = with_exponent(rng.coin(), rng.range(-1074, 1023), rng.next());
        padded.push(x);
        padded.push(-x);
    }
    for _ in 0..3 {
        rng.shuffle(&mut padded);
        let mut s = ExactSum::new();
        s.add_slice(&padded);
        assert_eq!(
            s.round_ieee().to_bits(),
            want.to_bits(),
            "{what}: add_slice"
        );
        let mut parts = Vec::new();
        let mut rest = &padded[..];
        while !rest.is_empty() {
            let n = (rng.below(3 * LARGE_SLICE as u64) as usize + 1).min(rest.len());
            let (piece, tail) = rest.split_at(n);
            let mut p = ExactSum::new();
            if rng.coin() {
                p.add_slice(piece);
            } else {
                for &x in piece {
                    p.add(x);
                }
            }
            parts.push(p);
            rest = tail;
        }
        while parts.len() > 1 {
            let i = rng.below(parts.len() as u64) as usize;
            let p = parts.swap_remove(i);
            let j = rng.below(parts.len() as u64) as usize;
            parts[j].merge(&p);
        }
        assert_eq!(
            parts[0].round_ieee().to_bits(),
            want.to_bits(),
            "{what}: merges"
        );
        match parts[0].round() {
            Ok(r) => assert_eq!(r.get().to_bits(), want.to_bits(), "{what}: round"),
            Err(e) => {
                assert!(!want.is_finite(), "{what}: round gave {e:?}");
                assert_eq!(e, NumError::NotFinite);
            }
        }
    }
}

#[test]
fn adv_exact_sum_ties_and_overflow_at_the_top() {
    let mut rng = Mix(0x746f_705f_7469);
    let om = f64::MAX; // Ω = 2^1024 − 2^971, odd
    let inf = f64::INFINITY;
    let cases: Vec<(&str, Vec<f64>, f64)> = vec![
        // Ω + 2^970 = 2^1024 − 2^970: the tie between Ω and 2^1024, to the even 2^1024: +∞.
        ("tie at the top", vec![om, p2(970)], inf),
        (
            "tie at the top in pieces",
            vec![om, p2(969), p2(968), p2(968)],
            inf,
        ),
        ("just below the tie", vec![om, p2(970), -p2(-1074)], om),
        (
            "just below the tie, deep",
            vec![om, p2(969), p2(968), p2(967), p2(967), -p2(-1074)],
            om,
        ),
        ("just above the tie", vec![om, p2(970), p2(-1074)], inf),
        // The exact sum passes 2^1024 and comes back.
        ("out and back", vec![om, om, -om], om),
        (
            "out and back with a tail",
            vec![om, om, om, -om, -om, p2(-1074)],
            om,
        ),
        (
            "far out and back",
            [vec![om; 40], vec![-om; 39]].concat(),
            om,
        ),
        ("2^1024 exactly", vec![p2(1023), p2(1023)], inf),
        // Ω − 2^970 is the tie between Ω − 2^971 (even) and Ω (odd).
        ("even tie below Ω", vec![om, -p2(970)], om - p2(971)),
        (
            "below the tie below Ω",
            vec![om, -p2(970), -p2(-1074)],
            om - p2(971),
        ),
        ("above the tie below Ω", vec![om, -p2(970), p2(-1074)], om),
    ];
    for (what, terms, want) in cases {
        every_way(&mut rng, what, &terms, want);
        let neg: Vec<f64> = terms.iter().map(|&x| -x).collect();
        every_way(&mut rng, &format!("{what}, negated"), &neg, -want);
    }
}

#[test]
fn adv_exact_sum_many_terms_at_the_capacity_edges() {
    // Thousands of Ω: every term in the large accumulator's chunk for exponent field 2046, whose
    // fractions are all ones, the most a chunk can take before it must move; the exact sum goes
    // to about 2^1038 and back.
    let mut rng = Mix(0x6d61_6e79_5f6f);
    let om = f64::MAX;
    for n in [4095_usize, 4096, 4097, 3 * 4096 + 5, 20_000] {
        let mut terms = vec![om; n];
        terms.extend(std::iter::repeat_n(-om, n - 1));
        rng.shuffle(&mut terms);
        every_way(&mut rng, &format!("{n} Ω and {} −Ω", n - 1), &terms, om);
        every_way(&mut rng, &format!("{n} Ω"), &vec![om; n], f64::INFINITY);
    }
    // Largest high part for the small accumulator: exponent field 32j + 31 (k = 31), significand
    // all ones, so each addition moves chunk j + 1 by 2^52 − 1, the most there is. Around the
    // 2047-addition allowance, and after merges.
    for (field, sign) in [(1023_i64, false), (1023, true), (2015, false), (31, true)] {
        let x = with_exponent(sign, field - 1023, u64::MAX);
        for n in [
            2046_usize, 2047, 2048, 2049, 4094, 4095, 4096, 6143, 100_003,
        ] {
            let terms = vec![x; n];
            let want = oracle(&terms);
            let mut s = ExactSum::new();
            for &t in &terms {
                s.add(t);
            }
            assert_eq!(s.round_ieee().to_bits(), want.to_bits(), "{n} × {x:e}");
            // A merge, then the most additions it allows, then one more.
            let mut a = ExactSum::new();
            for &t in &terms[..n / 2] {
                a.add(t);
            }
            let mut b = ExactSum::new();
            b.add_slice(&terms[n / 2..]);
            a.merge(&b);
            for _ in 0..2047 {
                a.add(x);
            }
            let mut all = terms.clone();
            all.extend(std::iter::repeat_n(x, 2047));
            assert_eq!(
                a.round_ieee().to_bits(),
                oracle(&all).to_bits(),
                "{n} × {x:e} merged"
            );
        }
    }
    // The subnormal and lowest normal chunks of the large accumulator, full of all-ones fractions.
    for e in [-1074_i64, -1023, -1022] {
        let x = if e == -1074 {
            f64::from_bits((1 << 52) - 1)
        } else {
            with_exponent(false, e, u64::MAX)
        };
        for n in [4096_usize, 4097, 8193] {
            let mut terms = vec![x; n];
            terms.extend(vec![-x; n / 3]);
            rng.shuffle(&mut terms);
            let want = oracle(&terms);
            every_way(&mut rng, &format!("{n} × {x:e}"), &terms, want);
        }
    }
}

#[test]
fn adv_exact_sum_holds_what_it_documents_up_to_2_pow_1100() {
    // "The accumulator holds magnitudes below 2^1100": Ω doubled 75 times is about 2^1099. Built
    // by self-merges (2^75 terms in all), then cancelled by the same for −Ω: the exact sum is 0,
    // and what is added next must come out exactly.
    let mut pos = ExactSum::new();
    pos.add(f64::MAX);
    let mut neg = ExactSum::new();
    neg.add(-f64::MAX);
    for k in 1..=75 {
        let (p, n) = (pos.clone(), neg.clone());
        pos.merge(&p);
        neg.merge(&n);
        assert_eq!(pos.round_ieee(), f64::INFINITY, "2^{k} Ω");
        assert_eq!(neg.round_ieee(), f64::NEG_INFINITY, "2^{k} −Ω");
    }
    let mut total = pos.clone();
    total.merge(&neg);
    total.add(1.5);
    total.add(p2(-1074));
    assert_eq!(total.round_ieee(), 1.5);
    // 2^75·Ω − (2^75 − 1)·Ω = Ω: the top chunk near its limit, then back to a finite double.
    let mut almost = neg.clone();
    almost.add(f64::MAX);
    let mut t = pos.clone();
    t.merge(&almost);
    t.merge(&ExactSum::new());
    assert_eq!(t.round_ieee(), f64::MAX);
    assert_eq!(t.round().map(geom_num::Real::get), Ok(f64::MAX));
}

#[test]
fn adv_exact_sum_order_free_at_scale() {
    // About 40 000 terms built to cancel: every exponent, pairs that cancel exactly or to their
    // last bit, tiny terms far below, and Ω pairs that take the sum past 2^1024 on the way.
    let mut rng = Mix(0x7363_616c_655f);
    let mut terms = Vec::new();
    for _ in 0..12_000 {
        let e = rng.range(-1074, 1023);
        let x = with_exponent(rng.coin(), e, rng.next());
        terms.push(x);
        let y = if rng.coin() {
            -x
        } else {
            f64::from_bits((-x).to_bits() + 1)
        };
        if y.is_finite() {
            terms.push(y);
        }
        terms.push(with_exponent(
            rng.coin(),
            rng.range(-1074, -900),
            rng.next(),
        ));
        if rng.below(20) == 0 {
            terms.push(f64::MAX);
            terms.push(-f64::MAX);
        }
    }
    let base = terms.clone();
    let want = oracle(&base);
    let mut t = base.clone();
    every_way(&mut rng, "cancelling", &t, want);
    rng.shuffle(&mut t);
    every_way(&mut rng, "cancelling, shuffled", &t, want);
    // A tie, 1 + 2^−53, hidden among exact cancellations at every exponent, then broken by a
    // sticky bit of either sign 1021 binades below.
    let mut pairs = Vec::new();
    for _ in 0..3000 {
        let x = with_exponent(rng.coin(), rng.range(-1074, 1023), rng.next());
        pairs.push(x);
        pairs.push(-x);
    }
    for (label, extra, want) in [
        ("tie", None, 1.0),
        ("sticky up", Some(p2(-1074)), 1.0 + p2(-52)),
        ("sticky down", Some(-p2(-1074)), 1.0),
    ] {
        let mut t = pairs.clone();
        t.push(1.0);
        t.push(p2(-53));
        if let Some(e) = extra {
            t.push(e);
        }
        rng.shuffle(&mut t);
        every_way(&mut rng, label, &t, want);
    }
}

#[test]
fn adv_exact_sum_specials_are_order_free_across_merges() {
    let mut rng = Mix(0x7370_6563_6961);
    let nan_a = f64::from_bits(0xfff0_0000_0000_0005);
    let nan_b = f64::from_bits(0x7ff0_0000_0000_0005);
    let quiet = f64::from_bits(0x7ff8_0000_0000_0000);
    let cases: Vec<(Vec<f64>, u64)> = vec![
        (
            vec![f64::INFINITY, 1.0, f64::NEG_INFINITY],
            0x7ff8_0000_0000_0000,
        ),
        (
            vec![f64::INFINITY, f64::INFINITY, -1.0],
            f64::INFINITY.to_bits(),
        ),
        (
            vec![f64::NEG_INFINITY, f64::MAX, f64::MAX],
            f64::NEG_INFINITY.to_bits(),
        ),
        (
            vec![nan_a, f64::INFINITY, f64::NEG_INFINITY],
            0x7ff0_0000_0000_0005,
        ),
        (vec![nan_a, nan_b], 0x7ff0_0000_0000_0005),
        (vec![quiet, nan_a, -quiet], 0x7ff8_0000_0000_0000),
    ];
    for (terms, want) in cases {
        for _ in 0..12 {
            let mut t = terms.clone();
            for _ in 0..rng.below(1500) {
                t.push(with_exponent(
                    rng.coin(),
                    rng.range(-1074, 1023),
                    rng.next(),
                ));
            }
            rng.shuffle(&mut t);
            let cut = rng.below(t.len() as u64 + 1) as usize;
            let (l, r) = t.split_at(cut);
            let mut a = ExactSum::new();
            a.add_slice(l);
            let mut b = ExactSum::new();
            for &x in r {
                b.add(x);
            }
            let mut ab = a.clone();
            ab.merge(&b);
            let mut ba = b.clone();
            ba.merge(&a);
            assert_eq!(ab.round_ieee().to_bits(), want, "{terms:?}");
            assert_eq!(ba.round_ieee().to_bits(), want, "{terms:?}");
            assert_eq!(ab.round(), Err(NumError::NotFinite));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Determinism: a digest of the bits of many results, fixed here from a native release run. The
// same test must pass in debug, in release, and on wasm32 (`cargo test --target wasm32-wasip1`).

/// FNV-1a over 64-bit words.
struct Digest(u64);

impl Digest {
    fn word(&mut self, w: u64) {
        for b in w.to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn dd(&mut self, r: Result<DD, NumError>) {
        match r {
            Ok(d) => {
                self.word(d.hi().to_bits());
                self.word(d.lo().to_bits());
            }
            Err(e) => self.word(match e {
                NumError::NotFinite => 1,
                NumError::OutsideDomain => 2,
                _ => 3,
            }),
        }
    }
}

/// The golden digest (native x86-64 release run, 2026-10-22). It was `0x58b2_1f7b_3059_4beb` until
/// `DD::mul` put its operands in a fixed order to be commutative bit for bit, which changes the
/// last bit of some products; with that order undone the old value is reproduced exactly.
const GOLDEN: u64 = 0x01fe_5679_df2b_0ede;

#[test]
fn adv_bits_are_the_same_on_every_target_and_profile() {
    let mut rng = Mix(0x676f_6c64_656e);
    let mut h = Digest(0xcbf2_9ce4_8422_2325);
    for i in 0..6000 {
        let (lo, hi) = if i % 3 == 0 { (-1074, 1023) } else { (-80, 80) };
        let ex = rng.range(lo, hi);
        let ey = rng.range(lo, hi);
        let x = adv_dd(&mut rng, ex);
        let y = adv_dd(&mut rng, ey);
        h.dd(x.add(y));
        h.dd(x.sub(y));
        h.dd(x.add_f64(y.hi()));
        h.dd(x.mul(y));
        h.dd(x.mul_f64(y.hi()));
        h.dd(x.div(y));
        h.dd(x.div_f64(y.hi()));
        h.dd(x.abs().sqrt());
        let (p, e) = geom_num::dd::two_prod_fma(x.hi(), y.hi());
        h.word(p.to_bits());
        h.word(e.to_bits());
        let f = math::fma(x.hi(), y.hi(), x.lo()).map_or(1, |r| r.get().to_bits());
        h.word(f);
    }
    for n in [3_usize, 700, 5000] {
        let terms: Vec<f64> = (0..n)
            .map(|_| with_exponent(rng.coin(), rng.range(-1074, 1023), rng.next()))
            .collect();
        let mut s = ExactSum::new();
        s.add_slice(&terms);
        h.word(s.round_ieee().to_bits());
        let mut t = ExactSum::new();
        for &x in &terms {
            t.add(x * 0.75);
        }
        h.word(t.round_ieee().to_bits());
    }
    println!("digest {:#018x}", h.0);
    assert_eq!(h.0, GOLDEN, "bits differ from the native release run");
}

// ---------------------------------------------------------------------------------------------
// The board's benchmark on the inputs that are hardest for the large accumulator: exponents over
// the whole range with both signs (every one of its 4094 finite chunks in use, so its 41 KB of
// state is touched at random), subnormals and a few chunks hit in random order (each load may or
// may not depend on the store before it). Asserts the board's 2× for `add_slice`; also reports,
// without asserting, `add` one term at a time. Ignored by default: run it in release on a quiet
// machine, `cargo test -p geom-num --release --test attack_g25 -- --ignored --nocapture
// --test-threads=1`.

#[test]
#[ignore = "a timing benchmark: run in release with --ignored"]
#[allow(
    clippy::too_many_lines,
    clippy::assertions_on_constants,
    reason = "a benchmark table; the build profile is what is checked"
)]
fn adv_bench_exact_sum_on_its_hardest_inputs() {
    use std::hint::black_box;
    use std::time::{Duration, Instant};
    assert!(!cfg!(debug_assertions), "run in release");
    let mut rng = Mix(0x6265_6e63_6821);
    let n = 1_000_000;
    let cases: Vec<(&str, Vec<f64>)> = vec![
        (
            "every exponent, both signs",
            (0..n)
                .map(|_| with_exponent(rng.coin(), rng.range(-1074, 1023), rng.next()))
                .collect(),
        ),
        (
            "every normal exponent, both signs",
            (0..n)
                .map(|_| with_exponent(rng.coin(), rng.range(-1022, 1023), rng.next()))
                .collect(),
        ),
        (
            "subnormals",
            (0..n)
                .map(|_| {
                    f64::from_bits(rng.next() & ((1 << 52) - 1))
                        * if rng.coin() { 1.0 } else { -1.0 }
                })
                .collect(),
        ),
        (
            "[1, 2), random signs",
            (0..n)
                .map(|_| {
                    (1.0 + (rng.next() >> 12) as f64 * p2(-52))
                        * if rng.coin() { 1.0 } else { -1.0 }
                })
                .collect(),
        ),
        (
            "two binades, random",
            (0..n)
                .map(|_| {
                    (1.0 + (rng.next() >> 12) as f64 * p2(-52)) * if rng.coin() { 1.0 } else { 2.0 }
                })
                .collect(),
        ),
        (
            "four chunks, random",
            (0..n)
                .map(|_| {
                    (1.0 + (rng.next() >> 12) as f64 * p2(-52))
                        * [1.0, -1.0, 2.0, -2.0][rng.below(4) as usize]
                })
                .collect(),
        ),
        (
            "eight binades, random",
            (0..n)
                .map(|_| (1.0 + (rng.next() >> 12) as f64 * p2(-52)) * p2(rng.range(0, 7)))
                .collect(),
        ),
        (
            "two exponents alternating",
            (0..n)
                .map(|i| {
                    if i % 2 == 0 {
                        1.0 + p2(-30) * f64::from(i)
                    } else {
                        -p2(100)
                    }
                })
                .collect(),
        ),
    ];
    let plain = |t: &[f64]| {
        let mut s = 0.0;
        for &x in t {
            s += x;
        }
        s
    };
    let slice = |t: &[f64]| {
        let mut s = ExactSum::new();
        s.add_slice(t);
        s.round_ieee()
    };
    let one = |t: &[f64]| {
        let mut s = ExactSum::new();
        for &x in t {
            s.add(x);
        }
        s.round_ieee()
    };
    let mut worst: f64 = 0.0;
    for (name, terms) in &cases {
        let (mut p, mut e, mut o) = (Duration::MAX, Duration::MAX, Duration::MAX);
        for _ in 0..61 {
            let t = Instant::now();
            black_box(plain(black_box(terms)));
            p = p.min(t.elapsed());
            let t = Instant::now();
            black_box(slice(black_box(terms)));
            e = e.min(t.elapsed());
            let t = Instant::now();
            black_box(one(black_box(terms)));
            o = o.min(t.elapsed());
        }
        let r = e.as_secs_f64() / p.as_secs_f64();
        let r1 = o.as_secs_f64() / p.as_secs_f64();
        println!(
            "{name:36} plain {:.3} ms  add_slice {:.3} ms ({r:.2}x)  add {:.3} ms ({r1:.2}x)",
            p.as_secs_f64() * 1e3,
            e.as_secs_f64() * 1e3,
            o.as_secs_f64() * 1e3
        );
        worst = worst.max(r);
    }
    assert!(worst <= 2.0, "add_slice took {worst:.2} times a plain sum");
}
