//! Independent audit of `geom_num::exact_sum::ExactSum` (its merges and its large accumulator) and
//! of `geom_num::dd::DD::round_shifted`.
//!
//! `ExactSum`: sums built the ways a kernel reduction builds them, several accumulators filled by
//! `add`, `add_slice` and `add_iter` (slices short, just under and over the large accumulator's
//! 512-term threshold, around the 4096 additions one large chunk takes, and tens of thousands of
//! terms in a few binades so that chunks fill and transfer many times), merged in a random shape,
//! with more terms added after each merge. The oracle is a big integer: every double is a whole
//! number of 2^−1074, so the exact sum is a sum of integers, rounded once to the nearest double,
//! ties to even, by integer division. Nothing in it is shared with the accumulator.
//!
//! `round_shifted`: the exact rational `hi + lo − offset` rounded half to even, with offsets at
//! both ends of `i128`, ties, and leading parts up to the documented 2^100.
//!
//! Seeds are fixed; `SPAREKIT_SEED=<hex>` replays another.

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
    clippy::too_many_lines,
    reason = "test code: a broken oracle must stop the test loudly"
)]

use geom_num::{DD, ExactSum};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

struct Rng(u64);

impl Rng {
    fn for_test(name: &str, default: u64) -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
            .unwrap_or(default);
        println!("{name}: seed: {seed:016x}");
        Rng(seed)
    }

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
}

/// A finite double as a whole number of 2^−1074.
fn units(x: f64) -> BigInt {
    let bits = x.to_bits();
    let field = (bits >> 52) & 0x7ff;
    assert!(field != 0x7ff, "not finite");
    let fraction = bits & ((1 << 52) - 1);
    let (significand, shift) = if field == 0 {
        (fraction, 0)
    } else {
        (fraction | (1 << 52), field - 1)
    };
    let magnitude = BigInt::from(significand) << shift as usize;
    if bits >> 63 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

/// The double nearest `n · 2^−1074`, ties to even; an infinity from 2^1024 − 2^970 up.
fn nearest(n: &BigInt) -> f64 {
    if n.is_zero() {
        return 0.0;
    }
    let magnitude = n.abs();
    let bits = magnitude.bits();
    let (kept, field) = if bits <= 53 {
        (magnitude, 1u64)
    } else {
        let shift = (bits - 53) as usize;
        let half = BigInt::one() << (shift - 1);
        let (q, r) = magnitude.div_rem(&(BigInt::one() << shift));
        let up = r > half || (r == half && q.is_odd());
        (if up { q + 1 } else { q }, shift as u64 + 1)
    };
    // `kept` is at most 2^53; a subnormal result has field 1 and kept below 2^52.
    let kept = kept.to_u64().unwrap();
    let pattern = if kept < (1 << 52) {
        kept
    } else {
        ((field - 1) << 52) + kept
    };
    let value = if pattern >= 0x7ff << 52 {
        f64::INFINITY
    } else {
        f64::from_bits(pattern)
    };
    if n.is_negative() { -value } else { value }
}

/// A term from one of six families of exponents.
fn term(rng: &mut Rng, family: u64) -> f64 {
    let fraction = rng.next() & ((1 << 52) - 1);
    let sign = rng.next() >> 63;
    let field = match family {
        0 => rng.below(2047),
        1 => 1000 + rng.below(3),
        2 => 0,
        3 => 2046,
        4 => [0, 1, 31, 32, 33, 63, 64, 2046][rng.below(8) as usize],
        _ => 1023 + rng.below(60),
    };
    f64::from_bits((sign << 63) | (field << 52) | fraction)
}

/// The oracle agrees with IEEE addition on pairs, so its rounding is the rounding under test.
#[test]
fn the_integer_oracle_rounds_as_ieee_does() {
    let mut rng = Rng::for_test(
        "the_integer_oracle_rounds_as_ieee_does",
        0x6ad1_7000_5eed_0005,
    );
    for _ in 0..200_000 {
        let family = rng.below(6);
        let (a, b) = (term(&mut rng, family), term(&mut rng, family));
        let want = a + b;
        let got = nearest(&(units(a) + units(b)));
        assert_eq!(got.to_bits(), (want + 0.0).to_bits(), "{a:e} + {b:e}");
    }
}

/// Every way of building a sum gives the exact sum rounded once.
#[test]
fn merged_and_large_sums_are_the_exact_sum_rounded_once() {
    let mut rng = Rng::for_test(
        "merged_and_large_sums_are_the_exact_sum_rounded_once",
        0x6ad1_7000_5eed_0002,
    );
    let rounds = if cfg!(debug_assertions) { 150 } else { 1_500 };
    let (mut terms_added, mut merges, mut overflows) = (0usize, 0usize, 0usize);
    for round in 0..rounds {
        let family = rng.below(6);
        let parts = 1 + rng.below(5) as usize;
        let mut exact = BigInt::zero();
        let mut sums: Vec<ExactSum> = Vec::new();
        for _ in 0..parts {
            let len = match rng.below(6) {
                0 => rng.below(20),
                1 => 505 + rng.below(15),
                2 => 4090 + rng.below(12),
                3 => 9_000 + rng.below(9_000),
                4 => 8_190 + rng.below(6),
                _ => rng.below(3_000),
            } as usize;
            let mut terms: Vec<f64> = (0..len).map(|_| term(&mut rng, family)).collect();
            if rng.below(3) == 0 {
                // Cancel all of it but one term.
                let negated: Vec<f64> = terms.iter().map(|x| -x).collect();
                terms.extend(negated);
                terms.push(term(&mut rng, family));
            }
            for &t in &terms {
                exact += units(t);
            }
            terms_added += terms.len();
            let mut s = ExactSum::new();
            match rng.below(4) {
                0 => {
                    for &t in &terms {
                        s.add(t);
                    }
                }
                1 => s.add_slice(&terms),
                2 => s.add_iter(terms.iter().copied()),
                _ => {
                    let cut = terms.len() / 2;
                    s.add_slice(&terms[..cut]);
                    // An iterator with no size hint: the first 512 terms go one by one.
                    s.add_iter(terms[cut..].iter().copied().filter(|_| true));
                }
            }
            sums.push(s);
        }
        while sums.len() > 1 {
            let taken = sums.swap_remove(rng.below(sums.len() as u64) as usize);
            let j = rng.below(sums.len() as u64) as usize;
            if rng.below(2) == 0 {
                sums[j].merge(&taken);
            } else {
                let mut other = taken.clone();
                other.merge(&sums[j]);
                sums[j] = other;
            }
            merges += 1;
            // More terms after a merge than its allowance before a carry propagation (2046).
            for _ in 0..rng.below(5_000) {
                let t = term(&mut rng, family);
                exact += units(t);
                sums[j].add(t);
                terms_added += 1;
            }
        }
        let mut total = ExactSum::new();
        total.merge(&sums[0]);
        let want = nearest(&exact);
        let got = total.round_ieee();
        overflows += usize::from(want.is_infinite());
        assert_eq!(
            got.to_bits(),
            want.to_bits(),
            "round {round}, family {family}: got {got:e}, want {want:e}"
        );
        assert_eq!(total.round().is_ok(), want.is_finite(), "round {round}");
    }
    println!("{terms_added} terms, {merges} merges, {overflows} sums that overflow");
}

fn rat(x: f64) -> BigRational {
    BigRational::from_float(x).unwrap()
}

/// `round_shifted` is the exact value rounded half to even, or `None` exactly where its
/// documentation says: a leading part above 2^100, or a result outside `i128`.
#[test]
fn round_shifted_is_the_exact_value_rounded_half_to_even() {
    let mut rng = Rng::for_test(
        "round_shifted_is_the_exact_value_rounded_half_to_even",
        0x6ad1_7000_5eed_0003,
    );
    let cases = if cfg!(debug_assertions) {
        20_000
    } else {
        400_000
    };
    let (mut answered, mut ties, mut refused) = (0usize, 0usize, 0usize);
    for _ in 0..cases {
        let e = rng.below(110) as i32 - 5;
        let unit = (rng.next() >> 12) as f64 / 4_503_599_627_370_496.0;
        let sign = if rng.below(2) == 0 { 1.0 } else { -1.0 };
        let hi = (1.0 + unit) * 2f64.powi(e) * sign;
        let lo = match rng.below(5) {
            0 => 0.0,
            1 => 0.5,
            2 => -0.5,
            3 => hi * 2f64.powi(-54),
            _ => {
                hi * 2f64.powi(-54 - rng.below(40) as i32)
                    * ((rng.next() >> 12) as f64 / 4_503_599_627_370_496.0)
            }
        };
        let hi = if rng.below(4) == 0 {
            hi.round() + [0.0, 0.5, 0.25][rng.below(3) as usize]
        } else {
            hi
        };
        let Ok(d) = DD::from_sum(hi, lo) else {
            continue;
        };
        let offset: i128 = match rng.below(6) {
            0 => 0,
            1 => i128::MAX - rng.below(8) as i128,
            2 => i128::MIN + rng.below(8) as i128,
            3 => (hi as i128).wrapping_add(rng.below(5) as i128 - 2),
            _ => (rng.next() as i128) << rng.below(60),
        };
        let exact = rat(d.hi()) + rat(d.lo()) - BigRational::from_integer(BigInt::from(offset));
        let floor = exact.floor().to_integer();
        let rest = &exact - BigRational::from_integer(floor.clone());
        let half = BigRational::new(BigInt::one(), BigInt::from(2));
        let tie = rest == half;
        let rounded = if rest > half || (tie && floor.is_odd()) {
            floor + 1
        } else {
            floor
        };
        let want = if d.hi().abs() > 2f64.powi(100) {
            None
        } else {
            rounded.to_i128().map(|v| (v, exact.is_integer()))
        };
        let got = d.round_shifted(offset).map(|r| (r.value(), r.is_exact()));
        assert_eq!(
            got,
            want,
            "hi {:e}, lo {:e}, offset {offset}",
            d.hi(),
            d.lo()
        );
        answered += usize::from(got.is_some());
        refused += usize::from(got.is_none());
        ties += usize::from(tie);
    }
    println!("{answered} answered ({ties} ties), {refused} refused");
    assert!(ties > 100 && refused > 100);
}
