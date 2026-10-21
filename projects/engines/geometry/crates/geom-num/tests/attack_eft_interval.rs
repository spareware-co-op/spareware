//! Adversarial tests of `geom_num::eft` and `geom_num::interval`: they try to break the
//! error-free transformations and the outward-rounded intervals, not to confirm them.
//!
//! Oracles (none shared with tests/`eft_interval`.rs, none calling the code under test):
//! - `dy`: a finite double as an exact dyadic number m · 2^e on `num-bigint`, for the error-free
//!   transformations. Checked against `num-rational`'s own `BigRational::from_float`.
//! - `rn`: round to nearest, ties to even, of any rational to a double, written from the definition
//!   (gradual underflow, overflow to infinity from Ω + half a unit), and of the square root of a
//!   double by integer square root. Checked against the hardware first.
//!
//! With `rn`, each interval operation has a bit-exact specification taken from its own
//! documentation: round each exact bound to nearest, step it outward once, store zero as +0, and
//! return `NotFinite` exactly when a stepped bound is infinite. The tests compare bits, which is
//! stronger than containment; containment and sign soundness are still checked on their own.
//!
//! What is attacked:
//! - `two_sum` at the overflow frontier, exhaustively: tops within 48 units of Ω and around 2^1023,
//!   small values around half a unit of Ω, every sign and order, where Knuth's form overflows and
//!   the guard must act.
//! - Sums landing in the subnormal range (the error must be exactly zero).
//! - Expansions built from `two_sum` and `two_prod` (Shewchuk's grow-expansion): many doubles of
//!   wildly different exponents summed exactly; an orientation determinant at the grid extremes.
//! - `two_prod` densely at both edges of its direct (unscaled) range, every exponent, with
//!   significand patterns that make both splits round up a binade.
//! - Every former precondition from outside (the functions now have none): a pair must be exact or
//!   reported (a non-finite output), never a finite wrong pair. Stronger, against the oracle: each
//!   function is exact whenever an exact pair exists at all, and a reported pair carries an
//!   infinity with the sign of the exact error. The crate's "never NaN" invariant (lib.rs) is
//!   checked for every function on finite inputs; on infinite inputs, NaN only where IEEE 754 makes
//!   the sum or product itself NaN.
//! - The claims about `split` that the two-product proof leans on: halves are whole multiples of
//!   `u_a`, the high half is at most 2^(`e_a` + 1) and a single bit when it is that large.
//! - Intervals: every operation bit for bit against the documented method, in regimes built to
//!   break it: sums near Ω, products near 2^1024 and near 2^−1074, quotients by small and subnormal
//!   divisors, operands touching zero, ±0 bounds, the whole line [−Ω, Ω], and the tie at Ω − 2^970.
//! - `sqrt` bit for bit against an integer-square-root oracle, at zero and subnormal bounds, and
//!   the correct rounding of `math::sqrt` it relies on. `square` bit for bit against its
//!   documented method, and the root of the non-negative part by its method (the crate's
//!   `sqrt_nonneg` is private since the third review round; its unit tests hold it to that method),
//!   with containment and monotone inclusion.
//! - Chains of operations against exact rational evaluation; `certain_sign` on expressions that
//!   cancel exactly and on sub-ulp quantities (the error of a rounded sum or product).
//! - The distance between coincident and nearly coincident points: its squared interval reaches
//!   below zero only through widening, and must still be certified.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
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
    clippy::many_single_char_names,
    reason = "points and values are named a, b, c and so on, as in the formulas they check"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use proptest::test_runner::{Config, TestCaseError};

/// The outcome of one check: `Err` carries the first disagreement with an oracle.
type TestResult = Result<(), TestCaseError>;

/// A property's configuration: `cases` random cases.
fn config(cases: u32) -> Config {
    Config {
        cases,
        ..Config::default()
    }
}

/// Runs a check outside `proptest!`, failing the test loudly.
fn run(result: TestResult) {
    if let Err(e) = result {
        panic!("{e}");
    }
}

/// The largest finite double, Ω = (2 − 2^−52) · 2^1023.
const OMEGA: f64 = f64::MAX;

/// A fixed-seed generator for the deterministic sweeps: the same cases on every run and target.
struct Lcg(u64);

impl Lcg {
    /// The next 64 pseudo-random bits.
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let x = self.0;
        (x ^ (x >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd) ^ (x >> 29)
    }

    /// A whole number in `lo ..= hi`.
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        let span = u64::try_from(hi - lo + 1).unwrap();
        lo + i32::try_from(self.next() % span).unwrap()
    }

    /// A random sign.
    fn sign(&mut self) -> bool {
        self.next() & 1 == 1
    }

    /// 52 fraction bits: random, or a pattern that makes ties, carries and splits that round up.
    fn fraction(&mut self) -> u64 {
        let k = self.next() % 64;
        match self.next() % 8 {
            0 => 0,
            1 => fp::FRAC,
            2 => 1 << (k % 52),
            3 => fp::FRAC >> (k % 52),
            4 => (fp::FRAC << (k % 52)) & fp::FRAC,
            _ => self.next() & fp::FRAC,
        }
    }
}

/// Bits of doubles, independent of `f64::next_up` and friends.
mod fp {
    /// The 52 fraction bits.
    pub const FRAC: u64 = (1 << 52) - 1;

    /// A normal double from its sign, unbiased exponent (−1022 ..= 1023) and fraction bits.
    pub fn make(negative: bool, exponent: i32, fraction: u64) -> f64 {
        assert!(
            (-1022..=1023).contains(&exponent),
            "exponent {exponent} is not normal"
        );
        let biased = u64::try_from(exponent + 1023).unwrap();
        f64::from_bits((u64::from(negative) << 63) | (biased << 52) | (fraction & FRAC))
    }

    /// 2^k, for every k a double holds (−1074 ..= 1023).
    pub fn pow2(k: i32) -> f64 {
        assert!((-1074..=1023).contains(&k), "2^{k} is not a double");
        if k >= -1022 {
            make(false, k, 0)
        } else {
            f64::from_bits(1u64 << (k + 1074))
        }
    }

    /// The next double up; +∞ stays, −∞ goes to −Ω.
    pub fn up(x: f64) -> f64 {
        assert!(!x.is_nan(), "up(NaN)");
        if x == f64::INFINITY {
            x
        } else if x == 0.0 {
            f64::from_bits(1)
        } else if x > 0.0 {
            f64::from_bits(x.to_bits() + 1)
        } else {
            f64::from_bits(x.to_bits() - 1)
        }
    }

    /// The next double down; −∞ stays, +∞ goes to Ω.
    pub fn down(x: f64) -> f64 {
        -up(-x)
    }

    /// −0 as +0; every other value unchanged.
    pub fn canon(x: f64) -> f64 {
        if x == 0.0 { 0.0 } else { x }
    }

    /// The format exponent: ⌊log2 |x|⌋ for a normal x, −1022 for a subnormal x or zero.
    pub fn format_exponent(x: f64) -> i32 {
        let biased = i32::try_from((x.to_bits() >> 52) & 0x7ff).unwrap();
        if biased == 0 { -1022 } else { biased - 1023 }
    }

    /// The unbiased exponent e of a normal double: 2^e ≤ |x| < 2^(e + 1).
    pub fn exponent(x: f64) -> i32 {
        assert!(
            x.is_normal(),
            "exponent({x:e}) of a double that is not normal"
        );
        format_exponent(x)
    }
}

/// Doubles as exact dyadic numbers m · 2^e, m odd (or zero), on `num-bigint`.
mod dy {
    use geom_num::Sign;
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use num_traits::{One, Signed, Zero};

    /// An exact dyadic number m · 2^e, normalised so equal values compare equal.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Dy {
        m: BigInt,
        e: i64,
    }

    impl Dy {
        /// m · 2^e with the trailing zero bits of m moved into e.
        fn norm(m: BigInt, e: i64) -> Dy {
            match m.trailing_zeros() {
                None => Dy { m, e: 0 },
                Some(tz) => Dy {
                    m: m >> tz,
                    e: e + i64::try_from(tz).unwrap(),
                },
            }
        }

        /// The exact value of a finite double, from its bits.
        pub fn of(x: f64) -> Dy {
            assert!(x.is_finite(), "Dy::of({x}) of a non-finite double");
            let bits = x.to_bits();
            let biased = i64::try_from((bits >> 52) & 0x7ff).unwrap();
            let fraction = bits & ((1u64 << 52) - 1);
            let (significand, e) = if biased == 0 {
                (fraction, -1074)
            } else {
                (fraction | (1 << 52), biased - 1075)
            };
            let m = BigInt::from(significand);
            Dy::norm(if bits >> 63 == 1 { -m } else { m }, e)
        }

        /// The exact sum.
        pub fn add(&self, o: &Dy) -> Dy {
            let e = self.e.min(o.e);
            let a = &self.m << u64::try_from(self.e - e).unwrap();
            let b = &o.m << u64::try_from(o.e - e).unwrap();
            Dy::norm(a + b, e)
        }

        /// The exact product.
        pub fn mul(&self, o: &Dy) -> Dy {
            Dy::norm(&self.m * &o.m, self.e + o.e)
        }

        /// The sign.
        pub fn sign(&self) -> Sign {
            if self.m.is_positive() {
                Sign::Positive
            } else if self.m.is_negative() {
                Sign::Negative
            } else {
                Sign::Zero
            }
        }

        /// The position of the lowest set bit (the value is a whole multiple of 2^that), if
        /// nonzero.
        pub fn lowest_bit(&self) -> Option<i64> {
            if self.m.is_zero() { None } else { Some(self.e) }
        }

        /// The position of the highest set bit, if nonzero: 2^that ≤ |value| < 2^(that + 1).
        pub fn highest_bit(&self) -> Option<i64> {
            if self.m.is_zero() {
                None
            } else {
                Some(self.e + i64::try_from(self.m.bits()).unwrap() - 1)
            }
        }

        /// The number of significant bits.
        pub fn significant_bits(&self) -> u64 {
            self.m.bits()
        }

        /// The same value as a rational.
        pub fn rational(&self) -> BigRational {
            let p = BigInt::one() << self.e.unsigned_abs();
            if self.e >= 0 {
                BigRational::from_integer(&self.m * p)
            } else {
                BigRational::new(self.m.clone(), p)
            }
        }
    }
}

/// Round to nearest, ties to even, from the definition, on exact rationals.
mod rn {
    use super::fp;
    use geom_num::Sign;
    use num_bigint::BigInt;
    use num_integer::Integer;
    use num_rational::BigRational;
    use num_traits::{One, Signed, ToPrimitive, Zero};
    use std::cmp::Ordering;

    /// The exact value of a finite double, by `num-rational`'s own conversion.
    pub fn exact(x: f64) -> BigRational {
        BigRational::from_float(x).unwrap_or_else(|| panic!("exact({x}) of a non-finite double"))
    }

    /// 2^k exactly.
    pub fn pow2(k: i64) -> BigRational {
        let p = BigInt::one() << k.unsigned_abs();
        if k >= 0 {
            BigRational::from_integer(p)
        } else {
            BigRational::new_raw(BigInt::one(), p)
        }
    }

    /// The sign of a rational.
    pub fn sign(q: &BigRational) -> Sign {
        if q.is_positive() {
            Sign::Positive
        } else if q.is_negative() {
            Sign::Negative
        } else {
            Sign::Zero
        }
    }

    /// The double nearest to `q`, ties to even; ±∞ when |q| ≥ 2^1024 − 2^970 (the midpoint between
    /// Ω and 2^1024, which ties to the even 2^1024); a zero result is +0.
    pub fn round(q: &BigRational) -> f64 {
        if q.is_zero() {
            return 0.0;
        }
        let negative = q.is_negative();
        let a = q.abs();
        // 2^(e − 1) < a < 2^(e + 1) from the bit lengths; then fix e so that 2^e ≤ a < 2^(e + 1).
        let mut e =
            i64::try_from(a.numer().bits()).unwrap() - i64::try_from(a.denom().bits()).unwrap();
        if a < pow2(e) {
            e -= 1;
        }
        // The value of the last significand bit at this binade, never finer than 2^−1074.
        let quantum = (e - 52).max(-1074);
        let scaled = &a * pow2(-quantum);
        let k = scaled.floor().to_integer();
        let twice_rest = (scaled - BigRational::from_integer(k.clone()))
            * BigRational::from_integer(BigInt::from(2));
        let k = match twice_rest.cmp(&BigRational::one()) {
            Ordering::Greater => k + 1u32,
            Ordering::Equal if k.is_odd() => k + 1u32,
            _ => k,
        };
        let magnitude = if k.is_zero() {
            0.0
        } else if i64::try_from(k.bits()).unwrap() - 1 + quantum >= 1024 {
            f64::INFINITY
        } else {
            // k ≤ 2^53, so the conversion is exact, and k · 2^quantum is a double, so the product is.
            k.to_u64().unwrap() as f64 * fp::pow2(i32::try_from(quantum).unwrap())
        };
        if negative { -magnitude } else { magnitude }
    }

    /// The double nearest to √x for a finite x ≥ 0, by integer square root: x = n / 2^j with j
    /// even, √x = √(n · 2^128) / 2^(j/2 + 64), and the integer root s ≥ 2^64 has at least 11 bits
    /// below the last kept bit, so s + ½ stands in for every irrational root between s and s + 1
    /// (no tie or rounding boundary lies strictly between two whole numbers at that scale).
    pub fn round_sqrt(x: f64) -> f64 {
        assert!(x.is_finite() && x >= 0.0, "round_sqrt({x})");
        if x == 0.0 {
            return 0.0;
        }
        let q = exact(x);
        let mut n = q.numer().clone();
        let d = q.denom().clone();
        let mut j = d.bits() - 1;
        assert_eq!(
            d,
            BigInt::one() << j,
            "the denominator of a double is a power of two"
        );
        if j % 2 == 1 {
            n <<= 1u32;
            j += 1;
        }
        let big = n << 128u32;
        let s = big.sqrt();
        let half = i64::try_from(j / 2).unwrap();
        let root = if &s * &s == big {
            BigRational::from_integer(s) * pow2(-(half + 64))
        } else {
            BigRational::from_integer(s * 2u32 + 1u32) * pow2(-(half + 65))
        };
        round(&root)
    }
}

/// Doubles and intervals weighted towards where bugs live.
mod strat {
    use super::OMEGA;
    use super::fp::{self, FRAC};
    use geom_num::interval::Interval;
    use proptest::prelude::*;

    /// 52 fraction bits: random, or a pattern (zero, all ones, one bit, runs, alternating bits).
    pub fn fraction() -> BoxedStrategy<u64> {
        prop_oneof![
            4 => any::<u64>().prop_map(|f| f & FRAC),
            1 => Just(0u64),
            1 => Just(FRAC),
            1 => (0u32..52).prop_map(|k| 1u64 << k),
            1 => (0u32..52).prop_map(|k| FRAC >> k),
            1 => (0u32..52).prop_map(|k| (FRAC << k) & FRAC),
            1 => Just(0x5_5555_5555_5555u64),
            1 => Just(0xA_AAAA_AAAA_AAAAu64),
        ]
        .boxed()
    }

    /// A normal double with its exponent in `range`, either sign.
    pub fn normal(range: std::ops::RangeInclusive<i32>) -> BoxedStrategy<f64> {
        (any::<bool>(), range, fraction())
            .prop_map(|(s, e, f)| fp::make(s, e, f))
            .boxed()
    }

    /// A nonzero subnormal double, either sign.
    pub fn subnormal() -> BoxedStrategy<f64> {
        (any::<bool>(), 1u64..(1 << 52))
            .prop_map(|(s, f)| f64::from_bits((u64::from(s) << 63) | f))
            .boxed()
    }

    /// Any finite double, uniform over bit patterns.
    pub fn any_finite() -> BoxedStrategy<f64> {
        any::<u64>()
            .prop_map(f64::from_bits)
            .prop_filter("finite", |x| x.is_finite())
            .boxed()
    }

    /// Fixed doubles at the edges of the format and of the kernel's ranges, both signs.
    pub fn edges() -> Vec<f64> {
        let mut v = vec![
            0.0,
            f64::from_bits(1),
            f64::from_bits(3),
            f64::from_bits(FRAC),
            f64::MIN_POSITIVE,
            f64::from_bits(f64::MIN_POSITIVE.to_bits() + 1),
            fp::pow2(-537),
            fp::pow2(-536),
            0.5,
            1.0,
            1.0 + f64::EPSILON,
            2.0 - f64::EPSILON,
            3.0,
            ((1u64 << 40) - 1) as f64,
            9_007_199_254_740_992.0,
            fp::pow2(511),
            fp::pow2(512),
            fp::down(fp::pow2(512)),
            fp::pow2(970),
            3.0 * fp::pow2(970),
            fp::down(fp::pow2(996)),
            fp::pow2(996),
            fp::pow2(1023),
            fp::down(OMEGA),
            OMEGA,
        ];
        let negated: Vec<f64> = v.iter().map(|x| -x).collect();
        v.extend(negated);
        v
    }

    /// A double of any kind: random bits, every exponent, subnormal, near overflow, edges,
    /// integers.
    pub fn wild() -> BoxedStrategy<f64> {
        prop_oneof![
            2 => any_finite(),
            2 => normal(-1022..=1023),
            1 => subnormal(),
            1 => normal(1015..=1023),
            1 => normal(-1022..=-1010),
            2 => prop::sample::select(edges()),
            1 => (-64i32..=64).prop_map(f64::from),
        ]
        .boxed()
    }

    /// A double of moderate size, for chains of operations that should mostly stay finite.
    pub fn moderate() -> BoxedStrategy<f64> {
        prop_oneof![
            3 => -1.0e6f64..1.0e6,
            2 => (-64i32..=64).prop_map(f64::from),
            2 => normal(-40..=40),
            1 => (-((1i64 << 40) - 1)..=((1i64 << 40) - 1)).prop_map(|c| c as f64),
            1 => wild(),
        ]
        .boxed()
    }

    /// The interval between two doubles, in either order.
    pub fn hull(x: f64, y: f64) -> Interval {
        if x <= y {
            Interval::new(x, y).unwrap()
        } else {
            Interval::new(y, x).unwrap()
        }
    }

    /// An interval whose bounds come from `values`: two of them, a point, or a few steps wide.
    pub fn interval_of(values: BoxedStrategy<f64>) -> BoxedStrategy<Interval> {
        prop_oneof![
            3 => (values.clone(), values.clone()).prop_map(|(x, y)| hull(x, y)),
            1 => values.clone().prop_map(|x| Interval::point(x).unwrap()),
            1 => (values, 0usize..6).prop_map(|(x, k)| {
                let mut y = x;
                for _ in 0..k {
                    if fp::up(y).is_finite() {
                        y = fp::up(y);
                    }
                }
                hull(x, y)
            }),
        ]
        .boxed()
    }

    /// An interval with both bounds of one sign and nonzero, from `values`.
    pub fn one_signed(values: BoxedStrategy<f64>) -> BoxedStrategy<Interval> {
        (values.clone(), values, any::<bool>())
            .prop_filter_map("a zero bound", |(x, y, negative)| {
                let (x, y) = (x.abs(), y.abs());
                if x == 0.0 || y == 0.0 {
                    return None;
                }
                Some(if negative { hull(-x, -y) } else { hull(x, y) })
            })
            .boxed()
    }

    /// An interval with a zero bound (either zero, either side) or the point zero.
    pub fn touching_zero() -> BoxedStrategy<Interval> {
        prop_oneof![
            2 => wild().prop_map(|x| hull(0.0, x)),
            2 => wild().prop_map(|x| hull(-0.0, x)),
            1 => Just(Interval::point(0.0).unwrap()),
            1 => Just(Interval::new(-0.0, -0.0).unwrap()),
            1 => subnormal().prop_map(|x| hull(0.0, x)),
        ]
        .boxed()
    }

    /// An interval of any kind.
    pub fn interval() -> BoxedStrategy<Interval> {
        prop_oneof![
            4 => interval_of(wild()),
            1 => touching_zero(),
            1 => wild().prop_map(|x| hull(-OMEGA, x)),
            1 => wild().prop_map(|x| hull(x, OMEGA)),
            1 => Just(Interval::new(-OMEGA, OMEGA).unwrap()),
        ]
        .boxed()
    }

    /// A pair of operands from one of the regimes where interval arithmetic breaks.
    pub fn pair() -> BoxedStrategy<(Interval, Interval)> {
        let tiny = prop_oneof![subnormal(), normal(-1022..=-1000)].boxed();
        prop_oneof![
            // Anything against anything.
            3 => (interval(), interval()),
            // Sums near ±Ω.
            1 => (interval_of(normal(1018..=1023)), interval_of(normal(1018..=1023))),
            // Products near 2^1024.
            1 => (interval_of(normal(505..=518)), interval_of(normal(505..=518))),
            // Products near and below 2^−1074.
            1 => (interval_of(normal(-545..=-525)), interval_of(normal(-545..=-525))),
            1 => (interval_of(subnormal()), interval_of(moderate())),
            // Quotients near 2^1024: a huge dividend over a small divisor of one sign.
            1 => (interval_of(normal(995..=1023)), one_signed(normal(-40..=1))),
            // Quotients that underflow: a tiny dividend over a large divisor.
            1 => (interval_of(tiny.clone()), one_signed(normal(0..=1023))),
            // Subnormal divisors.
            1 => (interval(), one_signed(subnormal())),
            1 => (interval_of(tiny), one_signed(subnormal())),
            // Operands touching zero.
            1 => (touching_zero(), touching_zero()),
            1 => (touching_zero(), interval()),
            1 => (interval(), touching_zero()),
        ]
        .boxed()
    }
}

/// Error-free transformations.
mod eft_attacks {
    use super::dy::Dy;
    use super::{Lcg, OMEGA, TestResult, config, fp, rn, run, strat};
    use geom_num::Sign;
    use geom_num::eft::{self, SPLITTER};
    use proptest::prelude::*;

    /// What an error-free transformation did with one pair.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Verdict {
        /// Both outputs finite and their exact sum is the exact result.
        Exact,
        /// An output is not finite: the caller can see that something went wrong.
        Reported,
        /// Both outputs finite but their exact sum is not the exact result: nobody can tell.
        SilentlyWrong,
    }

    /// The verdict on `(hi, lo)` as the sum (`product == false`) or product of `a` and `b`.
    fn verdict(a: f64, b: f64, hi: f64, lo: f64, product: bool) -> Verdict {
        if !(hi.is_finite() && lo.is_finite()) {
            return Verdict::Reported;
        }
        if !(a.is_finite() && b.is_finite()) {
            return Verdict::SilentlyWrong;
        }
        let want = if product {
            Dy::of(a).mul(&Dy::of(b))
        } else {
            Dy::of(a).add(&Dy::of(b))
        };
        if Dy::of(hi).add(&Dy::of(lo)) == want {
            Verdict::Exact
        } else {
            Verdict::SilentlyWrong
        }
    }

    /// Knuth's six operations exactly as published, with no overflow guard.
    fn knuth_unguarded(a: f64, b: f64) -> (f64, f64) {
        let s = a + b;
        let bv = s - a;
        let av = s - bv;
        (s, (a - av) + (b - bv))
    }

    /// A two-sum (`two_sum` or `fast_two_sum`, either order) on finite `a` and `b`:
    /// `s` is the rounded sum by the oracle and by the hardware; when finite, the pair is exact and
    /// does not overlap; when the sum overflows, `e` is the opposite infinity (never NaN).
    fn check_sum(name: &str, f: fn(f64, f64) -> (f64, f64), a: f64, b: f64) -> TestResult {
        let (s, e) = f(a, b);
        let exact = Dy::of(a).add(&Dy::of(b));
        let rounded = rn::round(&exact.rational());
        prop_assert!(
            s == rounded,
            "{}({:e}, {:e}): s = {:e}, but the sum rounds to {:e}",
            name,
            a,
            b,
            s,
            rounded
        );
        prop_assert_eq!(
            s.to_bits(),
            (a + b).to_bits(),
            "{}({:e}, {:e}): s is not the IEEE sum",
            name,
            a,
            b
        );
        if rounded.is_finite() {
            prop_assert!(
                e.is_finite(),
                "{}({:e}, {:e}): error {:e} is not finite",
                name,
                a,
                b,
                e
            );
            prop_assert_eq!(
                Dy::of(s).add(&Dy::of(e)),
                exact,
                "{}({:e}, {:e}) = ({:e}, {:e}) is not exact",
                name,
                a,
                b,
                s,
                e
            );
            prop_assert!(
                s + e == s,
                "{}({:e}, {:e}) = ({:e}, {:e}) overlaps",
                name,
                a,
                b,
                s,
                e
            );
        } else {
            prop_assert!(
                e.is_infinite() && e.is_sign_positive() != s.is_sign_positive(),
                "{}({:e}, {:e}) overflowed to {:e} and must carry the opposite infinity, got {:e}",
                name,
                a,
                b,
                s,
                e
            );
        }
        Ok(())
    }

    /// `two_sum` on any finite pair.
    fn check_two_sum(a: f64, b: f64) -> TestResult {
        check_sum("two_sum", eft::two_sum, a, b)
    }

    /// Whether a pair is inside the range `two_prod`'s former precondition named (both factors
    /// below 2^996, and zero or both normal with −970 ≤ `e_a` + `e_b` ≤ 1021), where no exact pair
    /// can fail.
    fn in_prod_domain(a: f64, b: f64) -> bool {
        let limit = fp::pow2(996);
        if !(a.abs() < limit && b.abs() < limit) {
            return false;
        }
        if a == 0.0 || b == 0.0 {
            return true;
        }
        a.is_normal()
            && b.is_normal()
            && (-970..=1021).contains(&(fp::exponent(a) + fp::exponent(b)))
    }

    /// `two_prod` on any finite pair, against its whole contract. An exact pair exists exactly when
    /// the rounded product is finite and the exact product is a whole multiple of 2^−1074 (the
    /// oracle's own reading: `p` and `e` are doubles, so both are such multiples; conversely such
    /// an error is at most half a unit of `p` and a multiple of the product's lowest bit, so it has
    /// at most 53 bits). Then the pair must be exact, IEEE and non-overlapping. Otherwise `e` must
    /// be an infinity with the sign of the exact error `a · b − p`, and for an overflowed `p` the
    /// opposite infinity. The laws (symmetry, oddness) hold either way.
    fn check_two_prod_total(a: f64, b: f64) -> TestResult {
        let (p, e) = eft::two_prod(a, b);
        prop_assert_eq!(
            p.to_bits(),
            (a * b).to_bits(),
            "two_prod({:e}, {:e}): p is not the IEEE product",
            a,
            b
        );
        prop_assert!(
            !e.is_nan(),
            "two_prod({:e}, {:e}) = ({:e}, NaN) from finite inputs",
            a,
            b,
            p
        );
        let exact = Dy::of(a).mul(&Dy::of(b));
        if !p.is_finite() {
            prop_assert!(
                e.is_infinite() && e.is_sign_positive() != p.is_sign_positive(),
                "two_prod({:e}, {:e}) overflowed to {:e} and must carry the opposite infinity, got {:e}",
                a,
                b,
                p,
                e
            );
        } else if exact.lowest_bit().is_none_or(|bit| bit >= -1074) {
            prop_assert!(
                e.is_finite(),
                "two_prod({:e}, {:e}) = ({:e}, {:e}): an exact pair exists but was not given",
                a,
                b,
                p,
                e
            );
            prop_assert_eq!(
                Dy::of(p).add(&Dy::of(e)),
                exact,
                "two_prod({:e}, {:e}) = ({:e}, {:e}) is not exact",
                a,
                b,
                p,
                e
            );
            prop_assert!(
                p + e == p,
                "two_prod({:e}, {:e}) = ({:e}, {:e}) overlaps",
                a,
                b,
                p,
                e
            );
        } else {
            let error = exact.add(&Dy::of(-p));
            let got = if e == f64::INFINITY {
                Sign::Positive
            } else if e == f64::NEG_INFINITY {
                Sign::Negative
            } else {
                Sign::Zero
            };
            prop_assert!(
                got != Sign::Zero && got == error.sign(),
                "two_prod({:e}, {:e}) = ({:e}, {:e}): no exact pair exists, so the error must be an infinity with the sign of {:?}",
                a,
                b,
                p,
                e,
                error.sign()
            );
        }
        let (p2, e2) = eft::two_prod(b, a);
        prop_assert!(
            p2.to_bits() == p.to_bits() && e2 == e,
            "two_prod is not symmetric at ({:e}, {:e})",
            a,
            b
        );
        let (p3, e3) = eft::two_prod(-a, b);
        prop_assert!(
            p3 == -p && e3 == -e,
            "two_prod is not odd at ({:e}, {:e})",
            a,
            b
        );
        Ok(())
    }

    /// `split` on any finite double, against its whole contract: exact, two halves of at most 26
    /// bits, |hi| ≥ |lo|, both whole multiples of `u_a`, |hi| ≤ 2^(`e_a` + 1); or, only when the
    /// high half would be 2^1024 (which needs |a| > 2^1024 − 2^999, since |lo| ≤ 2^(`e_a` − 26)),
    /// the pair (±∞, ∓∞) with the sign of `a`.
    fn check_split_total(a: f64) -> TestResult {
        let (hi, lo) = eft::split(a);
        if !hi.is_finite() {
            prop_assert!(
                hi.is_infinite() && lo == -hi && (hi > 0.0) == (a > 0.0),
                "split({:e}) = ({:e}, {:e}): a reported split must be an infinity with the sign of a and the opposite one",
                a,
                hi,
                lo
            );
            prop_assert!(
                a.abs() > fp::pow2(1023) * (2.0 - fp::pow2(-24)),
                "split({:e}) reported although its high half fits",
                a
            );
            return Ok(());
        }
        let (dh, dl) = (Dy::of(hi), Dy::of(lo));
        prop_assert_eq!(
            dh.add(&dl),
            Dy::of(a),
            "split({:e}) = ({:e}, {:e}) does not add up",
            a,
            hi,
            lo
        );
        prop_assert!(
            dh.significant_bits() <= 26 && dl.significant_bits() <= 26,
            "split({:e}) = ({:e}, {:e}): a half has more than 26 bits",
            a,
            hi,
            lo
        );
        prop_assert!(
            hi.abs() >= lo.abs(),
            "split({:e}) = ({:e}, {:e}): |hi| < |lo|",
            a,
            hi,
            lo
        );
        if let Some(ea) = Dy::of(a).highest_bit() {
            for half in [&dh, &dl] {
                prop_assert!(
                    half.lowest_bit().is_none_or(|b| b >= ea - 52),
                    "split({:e}) = ({:e}, {:e}): a half is not a multiple of u_a",
                    a,
                    hi,
                    lo
                );
            }
            prop_assert!(
                dh.highest_bit().is_some_and(|t| t <= ea + 1),
                "split({:e}): |hi| = {:e} exceeds 2^(e_a + 1)",
                a,
                hi
            );
        }
        Ok(())
    }

    /// `two_prod` on a pair the caller guarantees exact for: `p` is the rounded product by the
    /// oracle and by the hardware, the pair is exact, does not overlap, and the laws (symmetry,
    /// oddness) hold.
    fn check_two_prod(a: f64, b: f64) -> TestResult {
        let (p, e) = eft::two_prod(a, b);
        let exact = Dy::of(a).mul(&Dy::of(b));
        prop_assert_eq!(
            p.to_bits(),
            (a * b).to_bits(),
            "two_prod({:e}, {:e}): p is not the IEEE product",
            a,
            b
        );
        prop_assert!(
            p == rn::round(&exact.rational()),
            "two_prod({:e}, {:e}): p = {:e} is not the rounded product",
            a,
            b,
            p
        );
        prop_assert!(
            e.is_finite(),
            "two_prod({:e}, {:e}): error {:e} is not finite",
            a,
            b,
            e
        );
        prop_assert_eq!(
            Dy::of(p).add(&Dy::of(e)),
            exact,
            "two_prod({:e}, {:e}) = ({:e}, {:e}) is not exact",
            a,
            b,
            p,
            e
        );
        prop_assert!(
            p + e == p,
            "two_prod({:e}, {:e}) = ({:e}, {:e}) overlaps",
            a,
            b,
            p,
            e
        );
        let (p2, e2) = eft::two_prod(b, a);
        prop_assert!(
            p2.to_bits() == p.to_bits() && e2 == e,
            "two_prod is not symmetric at ({:e}, {:e})",
            a,
            b
        );
        let (p3, e3) = eft::two_prod(-a, b);
        prop_assert!(
            p3 == -p && e3 == -e,
            "two_prod is not odd at ({:e}, {:e})",
            a,
            b
        );
        Ok(())
    }

    /// Shewchuk's grow-expansion with zero elimination: adds `b` to a nonoverlapping expansion
    /// (components in increasing magnitude) with `two_sum`.
    fn grow(expansion: &[f64], b: f64) -> Vec<f64> {
        let mut q = b;
        let mut h = Vec::with_capacity(expansion.len() + 1);
        for &x in expansion {
            let (s, err) = eft::two_sum(q, x);
            if err != 0.0 {
                h.push(err);
            }
            q = s;
        }
        if q != 0.0 {
            h.push(q);
        }
        h
    }

    /// The exact sum of an expansion's components.
    fn expansion_value(h: &[f64]) -> Dy {
        h.iter().fold(Dy::of(0.0), |acc, &x| acc.add(&Dy::of(x)))
    }

    /// Components nonzero, nonoverlapping (the highest bit of each below the lowest bit of the
    /// next), so the sign of the expansion is the sign of its last component.
    fn nonoverlapping(h: &[f64]) -> bool {
        h.windows(2).all(|w| {
            let (lo, hi) = (Dy::of(w[0]), Dy::of(w[1]));
            match (lo.highest_bit(), hi.lowest_bit()) {
                (Some(top), Some(bottom)) => top < bottom,
                _ => false,
            }
        })
    }

    proptest! {
        #![proptest_config(config(8192))]

        /// `two_sum` on any pair of finite bit patterns.
        #[test]
        fn two_sum_any_finite_pair(a in strat::any_finite(), b in strat::any_finite()) {
            check_two_sum(a, b)?;
        }

        /// `two_sum` near overflow: a top-binade value against anything from 2^900 up, both signs.
        #[test]
        fn two_sum_near_overflow(a in strat::normal(1000..=1023), b in strat::normal(900..=1023)) {
            check_two_sum(a, b)?;
            check_two_sum(b, a)?;
            check_two_sum(-a, b)?;
            check_two_sum(b, -a)?;
        }

        /// A sum whose rounded value is subnormal was exact, so its error must be exactly zero.
        #[test]
        fn sums_in_the_subnormal_range_have_zero_error(
            a in prop_oneof![strat::subnormal(), strat::normal(-1022..=-1015)],
            b in prop_oneof![strat::subnormal(), strat::normal(-1022..=-1015)],
            k in -64i64..=64,
        ) {
            // b is either free or a near-cancellation of a, a few units away.
            let near = f64::from_bits((a.to_bits() & !(1u64 << 63)).saturating_add_signed(k));
            for (x, y) in [(a, b), (a, -near), (-near, a)] {
                check_two_sum(x, y)?;
                let (s, e) = eft::two_sum(x, y);
                if s.abs() < f64::MIN_POSITIVE {
                    prop_assert!(e == 0.0, "two_sum({:e}, {:e}) = ({:e}, {:e}): a subnormal sum is exact", x, y, s, e);
                }
            }
        }

        /// `fast_two_sum` is documented for |a| ≥ |b|; Dekker's condition is only that the exponent
        /// of `a` is at least that of `b`. Shows whether the documented precondition is the true
        /// one.
        #[test]
        fn fast_two_sum_is_exact_when_exponents_are_ordered(
            e in -1022i32..=1023,
            d in 0i32..=60,
            fa in strat::fraction(),
            fb in strat::fraction(),
            sa in any::<bool>(),
            sb in any::<bool>(),
        ) {
            let a = fp::make(sa, e, fa);
            let b = fp::make(sb, (e - d).max(-1022), fb);
            prop_assume!(fp::format_exponent(a) >= fp::format_exponent(b));
            check_sum("fast_two_sum", eft::fast_two_sum, a, b)?;
        }

        /// The documented promise of `fast_two_sum`: finite inputs never give NaN, even outside the
        /// precondition.
        #[test]
        fn fast_two_sum_never_gives_nan_on_finite_inputs(a in strat::wild(), b in strat::wild()) {
            let (s, e) = eft::fast_two_sum(a, b);
            prop_assert!(!s.is_nan() && !e.is_nan(), "fast_two_sum({:e}, {:e}) = ({:e}, {:e})", a, b, s, e);
            let (s, e) = eft::two_sum(a, b);
            prop_assert!(!s.is_nan() && !e.is_nan(), "two_sum({:e}, {:e}) = ({:e}, {:e})", a, b, s, e);
        }

        /// `two_prod` inside its precondition, with the exponent sum drawn uniformly over the whole
        /// allowed range and the factors' exponents split every possible way.
        #[test]
        fn two_prod_inside_its_precondition(
            sum in -970i32..=1021,
            t in 0u32..=1_000_000,
            fa in strat::fraction(),
            fb in strat::fraction(),
            sa in any::<bool>(),
            sb in any::<bool>(),
        ) {
            let lo = (sum - 995).max(-1022);
            let hi = (sum + 1022).min(995);
            prop_assume!(lo <= hi);
            let ea = lo + i32::try_from(u64::from(t) * u64::try_from(hi - lo).unwrap() / 1_000_000).unwrap();
            let (a, b) = (fp::make(sa, ea, fa), fp::make(sb, sum - ea, fb));
            prop_assert!(in_prod_domain(a, b));
            check_two_prod(a, b)?;
        }

        /// The doc's sufficient condition for `two_prod`, "2^−968 ≤ |a · b| < 2^1022" with both
        /// factors below 2^996, read loosely: also for subnormal factors, which the precondition
        /// excludes. Exact either way, so the loose reading is safe.
        #[test]
        fn two_prod_is_exact_whenever_the_documented_product_range_holds(
            a in prop_oneof![strat::subnormal(), strat::normal(-1022..=995)],
            t in 0u32..=1_000_000,
            fb in strat::fraction(),
            sb in any::<bool>(),
        ) {
            let ea = if a.is_normal() { fp::exponent(a) } else { Dy::of(a).highest_bit().map(|h| i32::try_from(h).unwrap()).unwrap() };
            // Every exponent of b that can put the product in range, with b normal below 2^996.
            let lo = (-969 - ea).max(-1022);
            let hi = (1021 - ea).min(995);
            prop_assume!(lo <= hi);
            let eb = lo + i32::try_from(u64::from(t) * u64::try_from(hi - lo).unwrap() / 1_000_000).unwrap();
            let b = fp::make(sb, eb, fb);
            let top = Dy::of(a).mul(&Dy::of(b)).highest_bit().unwrap();
            // 2^−968 ≤ |a · b| < 2^1022 exactly.
            prop_assume!((-968..1022).contains(&top));
            check_two_prod(a, b)?;
        }

        /// Shewchuk's grow-expansion over many doubles of wildly different exponents: the expansion
        /// holds the exact sum, is nonoverlapping, and its last component carries the exact sign.
        #[test]
        fn expansions_of_many_doubles_are_exact(
            xs in proptest::collection::vec(
                // Magnitudes at most 2^1001, so 40 of them never overflow.
                prop_oneof![strat::normal(-1022..=1000), strat::subnormal(), -1.0e6f64..1.0e6, Just(0.0)],
                1..40,
            ),
        ) {
            let mut h: Vec<f64> = Vec::new();
            let mut exact = Dy::of(0.0);
            for &x in &xs {
                h = grow(&h, x);
                exact = exact.add(&Dy::of(x));
                prop_assert_eq!(expansion_value(&h), exact.clone(), "expansion {:?} lost value", h);
                prop_assert!(nonoverlapping(&h), "expansion {:?} overlaps", h);
            }
            let sign = h.last().map_or(Sign::Zero, |&top| Dy::of(top).sign());
            prop_assert_eq!(sign, exact.sign(), "expansion {:?}", h);
        }

        /// The orientation determinant (q − p) × (r − p) of grid points, carried exactly as an
        /// expansion of two products and their errors, has the exact sign of the i128 determinant.
        #[test]
        fn orient2d_by_expansion_is_exact_at_the_grid_extremes(
            c in proptest::collection::vec(
                prop_oneof![
                    -((1i64 << 40) - 1)..=((1i64 << 40) - 1),
                    (0i64..64).prop_map(|k| ((1i64 << 40) - 1) - k),
                    (0i64..64).prop_map(|k| k - ((1i64 << 40) - 1)),
                ],
                6,
            ),
            collinear in any::<bool>(),
            k in -3i64..=3,
            nudge in -1i64..=1,
        ) {
            let (px, py, qx, qy) = (c[0], c[1], c[2], c[3]);
            let (rx, ry) = if collinear {
                // r on the line through p and q (clamped), or one step off it.
                let rx = (px + k * (qx - px) / 4 + nudge).clamp(-((1i64 << 40) - 1), (1i64 << 40) - 1);
                let ry = (py + k * (qy - py) / 4).clamp(-((1i64 << 40) - 1), (1i64 << 40) - 1);
                (rx, ry)
            } else {
                (c[4], c[5])
            };
            let d = |a: i64, b: i64| {
                let (s, e) = eft::two_sum(a as f64, -(b as f64));
                assert_eq!(e, 0.0, "a grid difference below 2^41 is exact in a double");
                s
            };
            let (ux, uy, vx, vy) = (d(qx, px), d(qy, py), d(rx, px), d(ry, py));
            let (p1, e1) = eft::two_prod(ux, vy);
            let (p2, e2) = eft::two_prod(uy, vx);
            let mut h = Vec::new();
            for x in [e1, p1, -e2, -p2] {
                h = grow(&h, x);
            }
            let di = |a: i64, b: i64| i128::from(a) - i128::from(b);
            let det = di(qx, px) * di(ry, py) - di(qy, py) * di(rx, px);
            let sign = h.last().map_or(Sign::Zero, |&top| Dy::of(top).sign());
            prop_assert_eq!(sign, Sign::of_i128(det), "expansion {:?} for determinant {}", h, det);
        }
    }

    proptest! {
        #![proptest_config(config(8192))]

        /// `fast_two_sum` on any finite pair, in both orders: no ordering is asked of the caller.
        #[test]
        fn fast_two_sum_any_finite_pair(a in strat::wild(), b in strat::wild()) {
            check_sum("fast_two_sum", eft::fast_two_sum, a, b)?;
            check_sum("fast_two_sum", eft::fast_two_sum, b, a)?;
            let (s, e) = eft::fast_two_sum(a, b);
            let (s2, e2) = eft::two_sum(a, b);
            prop_assert!(s.to_bits() == s2.to_bits() && e == e2, "fast_two_sum and two_sum differ at ({:e}, {:e})", a, b);
        }

        /// `two_prod` on any finite pair, every regime: exact whenever an exact pair exists,
        /// otherwise an infinite error with the right sign.
        #[test]
        fn two_prod_whole_contract_any_pair(
            a in prop_oneof![strat::wild(), strat::subnormal(), strat::normal(990..=1023)],
            b in prop_oneof![strat::wild(), strat::subnormal(), strat::normal(-1022..=-990)],
        ) {
            check_two_prod_total(a, b)?;
        }

        /// `two_prod` where the product is near and below the subnormal range, from both normal and
        /// subnormal factors: the regime where exact pairs stop existing.
        #[test]
        fn two_prod_whole_contract_near_underflow(
            sum in -1100i32..=-940,
            t in 0u32..=1_000_000,
            fa in strat::fraction(),
            fb in strat::fraction(),
            sa in any::<bool>(),
            sb in any::<bool>(),
            sub in 0u32..4,
        ) {
            let lo = (sum - 1023).max(-1022);
            let hi = (sum + 1022).min(1023);
            prop_assume!(lo <= hi);
            let ea = lo + i32::try_from(u64::from(t) * u64::try_from(hi - lo).unwrap() / 1_000_000).unwrap();
            let a = fp::make(sa, ea, fa);
            // Sometimes a subnormal second factor, whatever the sum.
            let b = if sub == 0 {
                f64::from_bits((u64::from(sb) << 63) | (fb >> (t % 52)).max(1))
            } else {
                fp::make(sb, sum - ea, fb)
            };
            check_two_prod_total(a, b)?;
        }

        /// `two_prod` with a factor at or above 2^996 and products anywhere from overflow down to
        /// the subnormal range.
        #[test]
        fn two_prod_whole_contract_large_factor(
            a in strat::normal(996..=1023),
            b in prop_oneof![strat::normal(-1022..=40), strat::subnormal(), Just(0.0)],
        ) {
            check_two_prod_total(a, b)?;
        }

        /// `split` on any finite double.
        #[test]
        fn split_whole_contract_any_double(
            a in prop_oneof![strat::wild(), strat::subnormal(), strat::normal(990..=1023)],
        ) {
            check_split_total(a)?;
        }
    }

    /// Every pair at the overflow frontier of `two_sum`: tops within 48 units of Ω and 16 either
    /// side of 2^1023, small values from 2^966 to 2^982 and either side of 2^970 (half a unit of
    /// Ω), every sign and order. The guard must make each finite sum exact and each overflowed sum
    /// carry the opposite infinity.
    #[test]
    fn two_sum_overflow_frontier_exhaustive() {
        let mut tops = Vec::new();
        for k in 0..48u64 {
            tops.push(f64::from_bits(OMEGA.to_bits() - k));
        }
        for k in 0..16u64 {
            tops.push(f64::from_bits(fp::pow2(1023).to_bits() + k));
            tops.push(f64::from_bits(fp::pow2(1023).to_bits() - k - 1));
        }
        let mut smalls = Vec::new();
        for s in 966..=976 {
            for j in 1..=64u32 {
                smalls.push(f64::from(j) * fp::pow2(s));
            }
        }
        for k in 1..=8u64 {
            smalls.push(f64::from_bits(fp::pow2(970).to_bits() + k));
            smalls.push(f64::from_bits(fp::pow2(970).to_bits() - k));
        }
        let (mut fallbacks, mut overflows) = (0u32, 0u32);
        for &t in &tops {
            for &x in &smalls {
                for (a, b) in [
                    (t, x),
                    (x, t),
                    (t, -x),
                    (-x, t),
                    (-t, x),
                    (x, -t),
                    (-t, -x),
                    (-x, -t),
                ] {
                    if (a + b).is_finite() {
                        if !knuth_unguarded(a, b).1.is_finite() {
                            fallbacks += 1;
                            // The proof's key step: an intermediate overflows only when |b| > |a|,
                            // which is what makes the fallback fast_two_sum(b, a) exact.
                            assert!(
                                b.abs() > a.abs(),
                                "Knuth's form overflowed with |a| ≥ |b|: ({a:e}, {b:e})"
                            );
                        }
                    } else {
                        overflows += 1;
                    }
                    run(check_two_sum(a, b));
                }
            }
        }
        assert!(
            fallbacks >= 50,
            "the sweep reached the overflow guard only {fallbacks} times"
        );
        assert!(
            overflows > 100,
            "the sweep overflowed only {overflows} times"
        );
    }

    /// `two_prod` at both limits of its exponent condition, for every exponent of `a`, with
    /// significand patterns that make both splits round up a binade (all ones), carry, or not.
    #[test]
    fn two_prod_dense_at_both_exponent_limits() {
        let patterns = [
            0u64,
            fp::FRAC,
            1,
            0x5_5555_5555_5555,
            0xA_AAAA_AAAA_AAAA,
            1 << 51,
            fp::FRAC >> 1,
            fp::FRAC ^ (1 << 26),
        ];
        let mut rng = Lcg(0x0171);
        let mut checked = 0u32;
        for ea in -1022..=995 {
            for eb in [-970 - ea, -969 - ea, 1020 - ea, 1021 - ea] {
                if !(-1022..=995).contains(&eb) {
                    continue;
                }
                for (i, &fa) in patterns.iter().enumerate() {
                    for (j, &fb) in patterns.iter().enumerate() {
                        let (a, b) = (fp::make(i % 2 == 1, ea, fa), fp::make(j % 3 == 1, eb, fb));
                        assert!(in_prod_domain(a, b));
                        run(check_two_prod(a, b));
                        checked += 1;
                    }
                }
                let (a, b) = (
                    fp::make(rng.sign(), ea, rng.fraction()),
                    fp::make(rng.sign(), eb, rng.fraction()),
                );
                run(check_two_prod(a, b));
            }
        }
        assert!(checked > 100_000, "only {checked} pairs checked");
    }

    /// The claims about `split` that the two-product proof leans on, for every exponent in its
    /// domain and many significands: the halves add up, have at most 26 bits each, |hi| ≥ |lo|,
    /// both are whole multiples of `u_a` = 2^(`e_a` − 52), |hi| ≤ 2^(`e_a` + 1), and hi is a single
    /// bit when it reaches 2^(`e_a` + 1).
    #[test]
    fn split_claims_the_product_proof_relies_on() {
        let mut rng = Lcg(0x5917);
        for ea in -1022..=995 {
            for k in 0..24 {
                let fraction = if k < 6 {
                    [
                        0,
                        fp::FRAC,
                        1,
                        fp::FRAC >> 26,
                        (fp::FRAC >> 25) << 25,
                        1 << 25,
                    ][k]
                } else {
                    rng.fraction()
                };
                let a = fp::make(rng.sign(), ea, fraction);
                let (hi, lo) = eft::split(a);
                let (dh, dl) = (Dy::of(hi), Dy::of(lo));
                assert_eq!(
                    dh.add(&dl),
                    Dy::of(a),
                    "split({a:e}) = ({hi:e}, {lo:e}) does not add up"
                );
                assert!(
                    dh.significant_bits() <= 26 && dl.significant_bits() <= 26,
                    "split({a:e}) = ({hi:e}, {lo:e}): a half has more than 26 bits"
                );
                assert!(
                    hi.abs() >= lo.abs(),
                    "split({a:e}) = ({hi:e}, {lo:e}): |hi| < |lo|"
                );
                let u = i64::from(ea) - 52;
                for half in [&dh, &dl] {
                    assert!(
                        half.lowest_bit().is_none_or(|b| b >= u),
                        "split({a:e}) = ({hi:e}, {lo:e}): a half is not a multiple of u_a"
                    );
                }
                let top = dh.highest_bit().unwrap();
                assert!(
                    top <= i64::from(ea) + 1,
                    "split({a:e}): |hi| = {hi:e} exceeds 2^(e_a + 1)"
                );
                if top == i64::from(ea) + 1 {
                    assert_eq!(
                        dh.significant_bits(),
                        1,
                        "split({a:e}): hi = {hi:e} reached 2^(e_a + 1) but is not a single bit"
                    );
                }
            }
        }
    }

    /// `split` where its former precondition excluded it: subnormal inputs and inputs from 2^996 up
    /// to Ω. Each must still split exactly into two 26-bit halves, or be reported by a non-finite
    /// half; never a finite wrong split. And the whole contract: reported only when the high half
    /// would be 2^1024.
    #[test]
    fn split_outside_its_precondition_is_exact_or_reported() {
        let mut rng = Lcg(0xd15c);
        let mut inputs: Vec<f64> = (0..52).map(|k| f64::from_bits(1u64 << k)).collect();
        inputs.extend((0..52).map(|k| f64::from_bits(fp::FRAC >> k)));
        inputs.extend((0..4000).map(|_| f64::from_bits(rng.next() & fp::FRAC)));
        for e in 996..=1023 {
            for _ in 0..64 {
                inputs.push(fp::make(false, e, rng.fraction()));
            }
            inputs.push(fp::make(false, e, 0));
            inputs.push(fp::make(false, e, fp::FRAC));
        }
        let mut exact_large = 0;
        for &x in &inputs {
            for a in [x, -x] {
                run(check_split_total(a));
                let (hi, lo) = eft::split(a);
                if !(hi.is_finite() && lo.is_finite()) {
                    continue;
                }
                let (dh, dl) = (Dy::of(hi), Dy::of(lo));
                assert_eq!(
                    dh.add(&dl),
                    Dy::of(a),
                    "split({a:e}) = ({hi:e}, {lo:e}) is finite and wrong"
                );
                assert!(
                    dh.significant_bits() <= 26 && dl.significant_bits() <= 26,
                    "split({a:e}) = ({hi:e}, {lo:e}): a half has more than 26 bits"
                );
                if a.abs() >= fp::pow2(996) {
                    exact_large += 1;
                }
            }
        }
        assert!(exact_large > 0, "no input at or above 2^996 split finitely");
    }

    /// `fast_two_sum` with its operands out of order (|a| < |b|), which its former precondition
    /// excluded without checking. Under the owner's bar such a pair must be reported or harmless,
    /// never a finite wrong pair; the function now orders its operands, so each must be exact.
    #[test]
    fn fast_two_sum_out_of_order_is_reported_or_exact() {
        let mut rng = Lcg(0xfa57);
        let mut wrong = Vec::new();
        let mut total = 0u32;
        let mut cases = vec![
            (1.0, fp::pow2(60)),
            (f64::from_bits(1), 1.0),
            (-1.0, 3.0 * fp::pow2(53)),
            (1.0e-300, 1.0e300),
        ];
        for _ in 0..20_000 {
            let eb = rng.range(-1022, 1023);
            let ea = rng.range(-1022, eb);
            let (a, b) = (
                fp::make(rng.sign(), ea, rng.fraction()),
                fp::make(rng.sign(), eb, rng.fraction()),
            );
            if a.abs() < b.abs() {
                cases.push((a, b));
            }
        }
        for (a, b) in cases {
            run(check_sum("fast_two_sum", eft::fast_two_sum, a, b));
            let (s, e) = eft::fast_two_sum(a, b);
            total += 1;
            if verdict(a, b, s, e, false) == Verdict::SilentlyWrong {
                wrong.push((a, b, s, e));
            }
        }
        assert!(
            wrong.is_empty(),
            "fast_two_sum gave a finite wrong pair on {} of {} out-of-order inputs, e.g. {:?} (as (a, b, s, e)); \
             fast_two_sum(1, 2^60) = {:?} although 1 + 2^60 = 2^60 + 1",
            wrong.len(),
            total,
            &wrong[..wrong.len().min(3)],
            eft::fast_two_sum(1.0, fp::pow2(60)),
        );
    }

    /// Products below the former exponent limit of `two_prod` (`e_a` + `e_b` < −970, both factors
    /// normal), which its old precondition excluded without checking: reported or exact, never a
    /// finite wrong pair, and exact whenever an exact pair exists (the whole contract).
    #[test]
    fn two_prod_below_its_exponent_limit_is_reported_or_exact() {
        let mut rng = Lcg(0x0e11);
        let mut wrong = Vec::new();
        let mut total = 0u32;
        for sum in -2044..=-971 {
            for _ in 0..8 {
                let lo = (sum - 995).max(-1022);
                let hi = (sum + 1022).min(995);
                if lo > hi {
                    continue;
                }
                let ea = rng.range(lo, hi);
                let (a, b) = (
                    fp::make(rng.sign(), ea, rng.fraction()),
                    fp::make(rng.sign(), sum - ea, rng.fraction()),
                );
                run(check_two_prod_total(a, b));
                let (p, e) = eft::two_prod(a, b);
                total += 1;
                if verdict(a, b, p, e, true) == Verdict::SilentlyWrong {
                    wrong.push((a, b, p, e));
                }
            }
        }
        let headline = eft::two_prod(fp::pow2(-600), fp::pow2(-600));
        assert!(
            wrong.is_empty(),
            "two_prod gave a finite wrong pair on {} of {} products below 2^−970, e.g. {:?} (as (a, b, p, e)); \
             two_prod(2^−600, 2^−600) = {:?}: a positive product reported as exactly zero",
            wrong.len(),
            total,
            &wrong[..wrong.len().min(3)],
            headline,
        );
    }

    /// Products with a subnormal factor (excluded by the former precondition): reported or exact,
    /// never a finite wrong pair, and exact whenever an exact pair exists.
    #[test]
    fn two_prod_with_a_subnormal_factor_is_reported_or_exact() {
        let mut rng = Lcg(0x5b);
        let mut wrong = Vec::new();
        let mut total = 0u32;
        for _ in 0..20_000 {
            let a = f64::from_bits(
                (u64::from(rng.sign()) << 63)
                    | ((rng.next() & fp::FRAC) >> (rng.next() % 52)).max(1),
            );
            let b = fp::make(rng.sign(), rng.range(-1022, 995), rng.fraction());
            run(check_two_prod_total(a, b));
            let (p, e) = eft::two_prod(a, b);
            total += 1;
            if verdict(a, b, p, e, true) == Verdict::SilentlyWrong {
                wrong.push((a, b, p, e));
            }
        }
        assert!(
            wrong.is_empty(),
            "two_prod gave a finite wrong pair on {} of {} products with a subnormal factor, e.g. {:?} (as (a, b, p, e))",
            wrong.len(),
            total,
            &wrong[..wrong.len().min(3)],
        );
    }

    /// A factor at or above 2^996 (outside part 1 of the former precondition) with any partner
    /// whose product is in range, and a zero against such a factor: reported or exact, and exact
    /// whenever an exact pair exists.
    #[test]
    fn two_prod_with_a_factor_at_or_above_2_996_is_reported_or_exact() {
        let mut rng = Lcg(0x3e4);
        for _ in 0..20_000 {
            let ea = rng.range(996, 1023);
            let a = fp::make(rng.sign(), ea, rng.fraction());
            let b = if rng.next() % 16 == 0 {
                0.0
            } else {
                fp::make(rng.sign(), rng.range(-1022, 1021 - ea), rng.fraction())
            };
            run(check_two_prod_total(a, b));
            let (p, e) = eft::two_prod(a, b);
            assert_ne!(
                verdict(a, b, p, e, true),
                Verdict::SilentlyWrong,
                "two_prod({a:e}, {b:e}) = ({p:e}, {e:e}) is finite and wrong"
            );
            let (p, e) = eft::two_prod(b, a);
            assert_ne!(
                verdict(b, a, p, e, true),
                Verdict::SilentlyWrong,
                "two_prod({b:e}, {a:e}) = ({p:e}, {e:e}) is finite and wrong"
            );
        }
    }

    /// Products above the former exponent limit of `two_prod` (`e_a` + `e_b` ≥ 1022, both factors
    /// below 2^996): reported or exact, never a finite wrong pair, and exact whenever an exact pair
    /// exists.
    #[test]
    fn two_prod_above_its_exponent_limit_is_reported_or_exact() {
        let mut rng = Lcg(0xab0e);
        for sum in 1022..=1990 {
            for _ in 0..16 {
                let ea = rng.range(sum - 995, 995);
                let (a, b) = (
                    fp::make(rng.sign(), ea, rng.fraction()),
                    fp::make(rng.sign(), sum - ea, rng.fraction()),
                );
                run(check_two_prod_total(a, b));
                let (p, e) = eft::two_prod(a, b);
                assert_ne!(
                    verdict(a, b, p, e, true),
                    Verdict::SilentlyWrong,
                    "two_prod({a:e}, {b:e}) = ({p:e}, {e:e}) is finite and wrong"
                );
            }
        }
        // All-ones significands at e_a + e_b = 1022: both splits round up and a_hi · b_hi = 2^1024.
        let x = fp::down(fp::pow2(512));
        run(check_two_prod_total(x, x));
        let (p, e) = eft::two_prod(x, x);
        assert_ne!(
            verdict(x, x, p, e, true),
            Verdict::SilentlyWrong,
            "two_prod({x:e}, {x:e}) = ({p:e}, {e:e})"
        );
    }

    /// A non-finite input is always reported: no function returns two finite outputs for it.
    #[test]
    fn non_finite_inputs_are_always_reported() {
        let bad = [f64::INFINITY, f64::NEG_INFINITY, f64::NAN];
        let good = [
            0.0,
            -0.0,
            1.0,
            -3.5,
            f64::from_bits(1),
            OMEGA,
            -OMEGA,
            fp::pow2(-600),
        ];
        for &x in &bad {
            let (h, l) = eft::split(x);
            assert!(!(h.is_finite() && l.is_finite()), "split({x}) = ({h}, {l})");
            for &y in good.iter().chain(bad.iter()) {
                for (a, b) in [(x, y), (y, x)] {
                    for (name, f) in [
                        ("two_sum", eft::two_sum as fn(f64, f64) -> (f64, f64)),
                        ("fast_two_sum", eft::fast_two_sum),
                        ("two_prod", eft::two_prod),
                    ] {
                        let (h, l) = f(a, b);
                        assert!(
                            !(h.is_finite() && l.is_finite()),
                            "{name}({a}, {b}) = ({h}, {l}): a non-finite input gave a finite pair"
                        );
                    }
                }
            }
        }
    }

    /// lib.rs: "No floating-point value produced here is ever NaN." Checked for every function of
    /// the module on finite inputs, inside and outside their former preconditions.
    #[test]
    fn finite_inputs_never_produce_nan() {
        let mut nans = Vec::new();
        let big = fp::pow2(1000);
        for a in [big, OMEGA, fp::down(fp::pow2(997)), -OMEGA] {
            let (h, l) = eft::split(a);
            if h.is_nan() || l.is_nan() {
                nans.push(format!("split({a:e}) = ({h:e}, {l:e})"));
            }
        }
        for (a, b) in [
            (big, fp::pow2(-100)),
            (0.0, OMEGA),
            (OMEGA, OMEGA),
            (OMEGA, 1.0),
            (fp::pow2(-1000), big),
        ] {
            let (p, e) = eft::two_prod(a, b);
            if p.is_nan() || e.is_nan() {
                nans.push(format!("two_prod({a:e}, {b:e}) = ({p:e}, {e:e})"));
            }
        }
        for (a, b) in [
            (OMEGA, OMEGA),
            (-OMEGA, -OMEGA),
            (OMEGA, fp::pow2(970)),
            (-3.0 * fp::pow2(970), OMEGA),
        ] {
            for (name, f) in [
                ("two_sum", eft::two_sum as fn(f64, f64) -> (f64, f64)),
                ("fast_two_sum", eft::fast_two_sum),
            ] {
                let (s, e) = f(a, b);
                if s.is_nan() || e.is_nan() {
                    nans.push(format!("{name}({a:e}, {b:e}) = ({s:e}, {e:e})"));
                }
            }
        }
        assert!(
            nans.is_empty(),
            "finite inputs produced NaN, against lib.rs's crate invariant: {nans:#?}"
        );
    }

    /// On infinite inputs (NaN aside) the second part is NaN only where IEEE 754 makes the first
    /// part NaN (∞ − ∞, 0 · ∞); otherwise it is an infinity, opposite to the first part.
    #[test]
    fn infinite_inputs_give_nan_only_where_ieee_does() {
        let inf = [f64::INFINITY, f64::NEG_INFINITY];
        let others = [
            0.0,
            -0.0,
            1.0,
            -3.5,
            f64::from_bits(1),
            OMEGA,
            -OMEGA,
            fp::pow2(-600),
            f64::INFINITY,
            f64::NEG_INFINITY,
        ];
        for &x in &inf {
            let (h, l) = eft::split(x);
            assert!(
                h.to_bits() == x.to_bits() && l.to_bits() == (-x).to_bits(),
                "split({x}) = ({h}, {l})"
            );
            for &y in &others {
                for (a, b) in [(x, y), (y, x)] {
                    for (name, f) in [
                        ("two_sum", eft::two_sum as fn(f64, f64) -> (f64, f64)),
                        ("fast_two_sum", eft::fast_two_sum),
                        ("two_prod", eft::two_prod),
                    ] {
                        let (h, l) = f(a, b);
                        if h.is_nan() {
                            assert!(l.is_nan(), "{name}({a}, {b}) = ({h}, {l})");
                        } else {
                            assert!(
                                h.is_infinite() && l == -h,
                                "{name}({a}, {b}) = ({h}, {l}): expected an infinity and its opposite"
                            );
                        }
                    }
                }
            }
        }
    }

    /// A reported pair keeps the sign of the exact error, so a product that underflows to zero
    /// still says which side of zero it is on; an overflowed product carries the opposite infinity.
    #[test]
    fn reported_pairs_carry_the_sign_of_the_error() {
        let t = fp::pow2(-600);
        let pos = eft::two_prod(t, t);
        assert!(
            pos.0.to_bits() == 0 && pos.1 == f64::INFINITY,
            "two_prod(2^−600, 2^−600) = {pos:?}"
        );
        let neg = eft::two_prod(-t, t);
        assert!(
            neg.0.to_bits() == (-0.0f64).to_bits() && neg.1 == f64::NEG_INFINITY,
            "two_prod(−2^−600, 2^−600) = {neg:?}"
        );
        // The adversary's examples, all reported now with the right sign.
        for (a, b) in [
            (3.7555e-308, -2.2598e-308),
            (-4.0256e-320, 1.2385e-90),
            (fp::pow2(-1074), 0.75),
        ] {
            run(check_two_prod_total(a, b));
            assert!(
                eft::two_prod(a, b).1.is_infinite(),
                "two_prod({a:e}, {b:e}) has no exact pair"
            );
        }
        // Exact pairs that the former precondition excluded: a factor at 2^1000, a zero against Ω,
        // a subnormal factor with a product in range, and a product at the top of the range.
        assert_eq!(
            eft::two_prod(fp::pow2(1000), fp::pow2(-100)),
            (fp::pow2(900), 0.0)
        );
        assert_eq!(eft::two_prod(0.0, OMEGA), (0.0, 0.0));
        assert_eq!(
            eft::two_prod(f64::from_bits(3), fp::pow2(1000)),
            (3.0 * fp::pow2(-74), 0.0)
        );
        assert_eq!(eft::two_prod(OMEGA, 1.0), (OMEGA, 0.0));
        assert_eq!(
            eft::two_prod(OMEGA, OMEGA),
            (f64::INFINITY, f64::NEG_INFINITY)
        );
        // The out-of-order sums from the report are exact now.
        assert_eq!(eft::fast_two_sum(1.0, fp::pow2(60)), (fp::pow2(60), 1.0));
        assert_eq!(
            eft::fast_two_sum(f64::from_bits(1), 1.0),
            (1.0, f64::from_bits(1))
        );
    }

    /// `two_prod` across the underflow frontier, deterministically: every exponent sum from −1080
    /// to −955 with significand patterns, both factors normal, and a subnormal factor of every
    /// length. Both outcomes must occur (exact pairs and reported ones), and each must meet the
    /// whole contract.
    #[test]
    fn two_prod_dense_across_the_underflow_frontier() {
        let patterns = [
            0u64,
            fp::FRAC,
            1,
            0x5_5555_5555_5555,
            1 << 51,
            fp::FRAC ^ (1 << 26),
            0xF_FFFF_F000_0000,
            0x8_0000_0000_0001,
        ];
        let mut rng = Lcg(0x1074);
        let (mut exact, mut reported) = (0u32, 0u32);
        for sum in -1080..=-955 {
            let (lo, hi) = ((sum - 1023).max(-1022), (sum + 1022).min(1023));
            for ea in [hi, sum / 2, lo, rng.range(lo, hi)] {
                for &fa in &patterns {
                    for &fb in &patterns {
                        let (a, b) = (fp::make(false, ea, fa), fp::make(true, sum - ea, fb));
                        run(check_two_prod_total(a, b));
                        if eft::two_prod(a, b).1.is_finite() {
                            exact += 1;
                        } else {
                            reported += 1;
                        }
                    }
                }
            }
            // A subnormal factor with 1 to 52 significant bits against a normal partner.
            for bits in 1..=52i32 {
                let a = f64::from_bits(fp::FRAC >> (52 - bits));
                let eb = (sum + 1075 - bits).clamp(-1022, 1023);
                let b = fp::make(rng.sign(), eb, rng.fraction());
                run(check_two_prod_total(a, b));
            }
        }
        assert!(
            exact > 1000 && reported > 1000,
            "the sweep met only {exact} exact and {reported} reported pairs"
        );
    }

    /// `split` at the top of the range: every exponent from 990 up with significand patterns, the
    /// doubles just below Ω and just below the threshold where the high half reaches 2^1024, and Ω,
    /// whose high half is 2^1024 and must be reported.
    #[test]
    fn split_at_the_top_of_the_range() {
        let mut rng = Lcg(0x5e1);
        for e in 990..=1023 {
            for f in [
                0u64,
                fp::FRAC,
                1,
                fp::FRAC >> 26,
                (fp::FRAC >> 25) << 25,
                1 << 25,
                fp::FRAC ^ (1 << 25),
            ] {
                run(check_split_total(fp::make(false, e, f)));
                run(check_split_total(fp::make(true, e, f)));
            }
            for _ in 0..64 {
                run(check_split_total(fp::make(rng.sign(), e, rng.fraction())));
            }
        }
        let threshold = fp::make(false, 1023, (fp::FRAC >> 26) << 26);
        for k in 0..4096u64 {
            run(check_split_total(f64::from_bits(OMEGA.to_bits() - k)));
            run(check_split_total(f64::from_bits(
                threshold.to_bits() + 2048 - k,
            )));
        }
        assert_eq!(eft::split(OMEGA), (f64::INFINITY, f64::NEG_INFINITY));
        assert_eq!(eft::split(-OMEGA), (f64::NEG_INFINITY, f64::INFINITY));
        // 2^1024 − 2^999 splits exactly: it has 25 significant bits, so its high half is itself.
        let below = fp::pow2(1023) * (2.0 - fp::pow2(-24));
        assert_eq!(eft::split(below), (below, 0.0));
    }

    /// The splitting constant is 2^27 + 1, exactly.
    #[test]
    fn splitter_is_exact() {
        assert_eq!(Dy::of(SPLITTER), Dy::of(fp::pow2(27)).add(&Dy::of(1.0)));
    }

    /// 2^970, half a unit of Ω, built in a constant context.
    const POW970: f64 = f64::from_bits(1993u64 << 52);

    /// Inputs evaluated at compile time: an ordinary pair, the overflow-guard case, a tie, a sum
    /// into the subnormals, an overflowing sum, a grid-scale product, and a split out of range.
    const CT_INPUTS: [(f64, f64); 7] = [
        (0.1, 0.2),
        (-3.0 * POW970, f64::MAX),
        (1.0, f64::EPSILON / 2.0),
        (f64::MIN_POSITIVE, -f64::MIN_POSITIVE * (1.0 - f64::EPSILON)),
        (f64::MAX, POW970),
        (2_199_023_255_550.0, -2_199_023_255_549.0),
        (f64::MAX, 1.0),
    ];

    /// `two_sum`, `fast_two_sum`, `two_prod` and `split` of each input, at compile time.
    const fn compile_time_results() -> [[(f64, f64); 4]; 7] {
        let mut out = [[(0.0, 0.0); 4]; 7];
        let mut i = 0;
        while i < CT_INPUTS.len() {
            let (a, b) = CT_INPUTS[i];
            out[i] = [
                eft::two_sum(a, b),
                eft::fast_two_sum(a, b),
                eft::two_prod(a, b),
                eft::split(a),
            ];
            i += 1;
        }
        out
    }

    /// The compile-time results.
    const COMPILE_TIME: [[(f64, f64); 4]; 7] = compile_time_results();

    /// Determinism: the functions are `const fn`, so the compiler's own (software) floating point
    /// evaluates them too. Compile time and run time must agree bit for bit (any NaN with any NaN,
    /// whose payload Rust leaves unspecified).
    #[test]
    fn compile_time_and_run_time_agree() {
        let same = |x: f64, y: f64| x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan());
        for (i, &(a, b)) in CT_INPUTS.iter().enumerate() {
            let (a, b) = (std::hint::black_box(a), std::hint::black_box(b));
            let runtime = [
                eft::two_sum(a, b),
                eft::fast_two_sum(a, b),
                eft::two_prod(a, b),
                eft::split(a),
            ];
            for (name, (ct, rt)) in ["two_sum", "fast_two_sum", "two_prod", "split"]
                .iter()
                .zip(COMPILE_TIME[i].iter().zip(runtime.iter()))
            {
                assert!(
                    same(ct.0, rt.0) && same(ct.1, rt.1),
                    "{name}({a:e}, {b:e}): compile time {ct:?}, run time {rt:?}"
                );
            }
        }
    }
}

/// Intervals.
mod interval_attacks {
    use super::fp::{self, canon, down, up};
    use super::rn::{self, exact, sign};
    use super::{OMEGA, TestResult, config, run, strat};
    use geom_num::interval::Interval;
    use geom_num::{NumError, Sign, eft, math};
    use num_rational::BigRational;
    use num_traits::Zero;
    use proptest::prelude::*;

    /// The four interval operations with two operands.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Op {
        /// `add`.
        Add,
        /// `sub`.
        Sub,
        /// `mul`.
        Mul,
        /// `div`.
        Div,
    }

    impl Op {
        /// Every operation.
        const ALL: [Op; 4] = [Op::Add, Op::Sub, Op::Mul, Op::Div];

        /// The operation by the code under test.
        fn apply(self, a: Interval, b: Interval) -> Result<Interval, NumError> {
            match self {
                Op::Add => a.add(b),
                Op::Sub => a.sub(b),
                Op::Mul => a.mul(b),
                Op::Div => a.div(b),
            }
        }

        /// The operation on exact rationals.
        fn exact(self, x: &BigRational, y: &BigRational) -> BigRational {
            match self {
                Op::Add => x + y,
                Op::Sub => x - y,
                Op::Mul => x * y,
                Op::Div => x / y,
            }
        }
    }

    /// The bits of a result, so results compare exactly.
    fn bits(r: Result<Interval, NumError>) -> Result<(u64, u64), NumError> {
        r.map(|i| (i.lo().to_bits(), i.hi().to_bits()))
    }

    /// The documented method for exact bounds `m ≤ big_m`: round each to nearest, step outward
    /// once, store zero as +0; `NotFinite` when a stepped bound is infinite.
    fn spec(m: &BigRational, big_m: &BigRational) -> Result<(u64, u64), NumError> {
        let lo = down(rn::round(m));
        let hi = up(rn::round(big_m));
        if lo.is_finite() && hi.is_finite() {
            Ok((canon(lo).to_bits(), canon(hi).to_bits()))
        } else {
            Err(NumError::NotFinite)
        }
    }

    /// The smallest and largest of some rationals.
    fn min_max(values: &[BigRational]) -> (BigRational, BigRational) {
        let mut lo = values[0].clone();
        let mut hi = values[0].clone();
        for v in &values[1..] {
            if *v < lo {
                lo = v.clone();
            }
            if *v > hi {
                hi = v.clone();
            }
        }
        (lo, hi)
    }

    /// Whether an interval holds an exact value.
    fn holds(r: Interval, v: &BigRational) -> bool {
        exact(r.lo()) <= *v && *v <= exact(r.hi())
    }

    /// The certain sign, if any, is the sign of every exact value given.
    fn sign_is_sound(r: Interval, values: &[&BigRational]) -> TestResult {
        if let Some(s) = r.certain_sign() {
            for v in values {
                prop_assert_eq!(s, sign(v), "{:?} claims {:?} but holds {}", r, s, v);
            }
        }
        Ok(())
    }

    /// One binary operation: bit for bit the documented method (or the right error), and
    /// independently, a finite ordered result that holds the exact range with a sound sign.
    fn check_binary(op: Op, a: Interval, b: Interval) -> TestResult {
        let got = op.apply(a, b);
        if op == Op::Div && b.lo() <= 0.0 && b.hi() >= 0.0 {
            prop_assert_eq!(got, Err(NumError::DivisorContainsZero), "{:?} / {:?}", a, b);
            return Ok(());
        }
        let (al, ah, bl, bh) = (exact(a.lo()), exact(a.hi()), exact(b.lo()), exact(b.hi()));
        let corners = [
            op.exact(&al, &bl),
            op.exact(&al, &bh),
            op.exact(&ah, &bl),
            op.exact(&ah, &bh),
        ];
        let (m, big_m) = min_max(&corners);
        prop_assert_eq!(
            bits(got),
            spec(&m, &big_m),
            "{:?} {:?} {:?} = {:?}, not the documented method",
            op,
            a,
            b,
            got
        );
        if let Ok(r) = got {
            prop_assert!(
                r.lo().is_finite() && r.hi().is_finite() && r.lo() <= r.hi(),
                "{:?} broken",
                r
            );
            prop_assert!(
                holds(r, &m) && holds(r, &big_m),
                "{:?} {:?} {:?} = {:?} misses [{}, {}]",
                op,
                a,
                b,
                r,
                m,
                big_m
            );
            sign_is_sound(r, &[&m, &big_m])?;
        }
        Ok(())
    }

    /// `sqrt`: bit for bit the documented method against the integer-square-root oracle, and
    /// independently a non-negative result whose squared bounds enclose the operand's bounds.
    fn check_sqrt(a: Interval) -> TestResult {
        let got = a.sqrt();
        if a.lo() < 0.0 {
            prop_assert_eq!(got, Err(NumError::NegativeSqrt), "sqrt {:?}", a);
            return Ok(());
        }
        let lo = if a.lo() == 0.0 {
            0.0
        } else {
            canon(down(rn::round_sqrt(a.lo())))
        };
        let hi = if a.hi() == 0.0 {
            0.0
        } else {
            canon(up(rn::round_sqrt(a.hi())))
        };
        prop_assert_eq!(
            bits(got),
            Ok((lo.to_bits(), hi.to_bits())),
            "sqrt {:?} = {:?}, not the documented method",
            a,
            got
        );
        let r = got.unwrap();
        prop_assert!(
            r.lo() >= 0.0 && r.lo() <= r.hi() && r.hi().is_finite(),
            "sqrt {:?} = {:?}",
            a,
            r
        );
        let (rl, rh) = (exact(r.lo()), exact(r.hi()));
        prop_assert!(
            &rl * &rl <= exact(a.lo()) && exact(a.hi()) <= &rh * &rh,
            "sqrt {:?} = {:?} misses a root",
            a,
            r
        );
        Ok(())
    }

    /// `neg`: exactly `[−hi, −lo]` with zero as +0.
    fn check_neg(a: Interval) -> TestResult {
        let r = a.neg();
        prop_assert_eq!(
            (r.lo().to_bits(), r.hi().to_bits()),
            (canon(-a.hi()).to_bits(), canon(-a.lo()).to_bits()),
            "neg {:?}",
            a
        );
        Ok(())
    }

    /// `square`: bit for bit its documented method (lower bound exactly 0 when the interval holds
    /// zero, else the nearer bound's square rounded, stepped down and kept at or above 0; upper
    /// bound the farther bound's square rounded and stepped up; `NotFinite` when that is infinite),
    /// and independently a non-negative result holding both extreme squares, with a sound sign,
    /// inside `mul(a, a)` whenever that succeeds.
    fn check_square(a: Interval) -> TestResult {
        let got = a.square();
        let (l, h) = (exact(a.lo()), exact(a.hi()));
        let zero = BigRational::zero();
        let near = if l >= zero {
            l.clone()
        } else if h <= zero {
            -h.clone()
        } else {
            zero.clone()
        };
        let far = if -l.clone() > h { -l } else { h };
        let (near2, far2) = (&near * &near, &far * &far);
        let top = up(rn::round(&far2));
        let want = if top.is_finite() {
            let stepped = if near.is_zero() {
                0.0
            } else {
                canon(down(rn::round(&near2)))
            };
            let bottom: f64 = if stepped < 0.0 { 0.0 } else { stepped };
            Ok((bottom.to_bits(), canon(top).to_bits()))
        } else {
            Err(NumError::NotFinite)
        };
        prop_assert_eq!(
            bits(got),
            want,
            "square {:?} = {:?}, not the documented method",
            a,
            got
        );
        if let Ok(r) = got {
            prop_assert!(
                r.lo() >= 0.0 && r.lo() <= r.hi() && r.hi().is_finite(),
                "square {:?} = {:?}",
                a,
                r
            );
            prop_assert!(
                holds(r, &near2) && holds(r, &far2),
                "square {:?} = {:?} misses [{}, {}]",
                a,
                r,
                near2,
                far2
            );
            sign_is_sound(r, &[&near2, &far2])?;
            if let Ok(m) = a.mul(a) {
                prop_assert!(
                    m.lo() <= r.lo() && r.hi() <= m.hi(),
                    "square {:?} = {:?} is not inside mul {:?}",
                    a,
                    r,
                    m
                );
            }
        }
        Ok(())
    }

    /// The root of the non-negative part of `a`, by its documented method: `NegativeSqrt` when
    /// every value is negative, otherwise `sqrt` of [max(lo, 0), hi].
    ///
    /// The crate's own operation is private since the third review round: on a caller's word that
    /// the value is not negative it would certify a negative value as an exact zero, so only
    /// `Interval::norm`, which proves its radicand non-negative, takes it. The crate's unit tests
    /// hold it to this method bit for bit; here the method stands in for it, through the public
    /// `sqrt`.
    fn sqrt_nonneg(a: Interval) -> Result<Interval, NumError> {
        if a.hi() < 0.0 {
            return Err(NumError::NegativeSqrt);
        }
        Interval::new(if a.lo() < 0.0 { 0.0 } else { a.lo() }, a.hi())?.sqrt()
    }

    /// `sqrt_nonneg`: `NegativeSqrt` exactly when every value is negative; otherwise bit for bit
    /// `sqrt` of the non-negative part, which `check_sqrt` holds to the integer-square-root oracle.
    fn check_sqrt_nonneg(a: Interval) -> TestResult {
        let got = sqrt_nonneg(a);
        if a.hi() < 0.0 {
            prop_assert_eq!(got, Err(NumError::NegativeSqrt), "sqrt_nonneg {:?}", a);
            return Ok(());
        }
        let part = Interval::new(if a.lo() < 0.0 { 0.0 } else { a.lo() }, a.hi()).unwrap();
        prop_assert_eq!(
            bits(got),
            bits(part.sqrt()),
            "sqrt_nonneg {:?} is not sqrt of {:?}",
            a,
            part
        );
        check_sqrt(part)
    }

    /// A point interval.
    fn pt(x: f64) -> Interval {
        Interval::point(x).unwrap()
    }

    /// `x` moved `n` steps along the doubles (up for n > 0), staying finite.
    fn steps(x: f64, n: i64) -> f64 {
        let mut y = x;
        for _ in 0..n.unsigned_abs() {
            let next = if n > 0 { up(y) } else { down(y) };
            if next.is_finite() {
                y = next;
            }
        }
        y
    }

    proptest! {
        #![proptest_config(config(4096))]

        /// The oracle against the hardware: `rn::round` of an exact sum, difference, product or
        /// quotient of two doubles is what IEEE 754 arithmetic gives.
        #[test]
        fn oracle_round_matches_the_hardware(a in strat::wild(), b in strat::wild()) {
            let (x, y) = (exact(a), exact(b));
            prop_assert!(rn::round(&(&x + &y)) == a + b, "{:e} + {:e}", a, b);
            prop_assert!(rn::round(&(&x - &y)) == a - b, "{:e} − {:e}", a, b);
            prop_assert!(rn::round(&(&x * &y)) == a * b, "{:e} × {:e}", a, b);
            if b != 0.0 {
                prop_assert!(rn::round(&(&x / &y)) == a / b, "{:e} / {:e}", a, b);
            }
        }

        /// The two exact readings of a double agree: bits read by hand and `num-rational`'s own.
        #[test]
        fn oracle_dyadic_matches_num_rational(x in strat::wild()) {
            prop_assert_eq!(super::dy::Dy::of(x).rational(), exact(x));
        }

        /// `math::sqrt`, which the interval square root relies on, is correctly rounded for every
        /// kind of positive double (the oracle is an integer square root).
        #[test]
        fn math_sqrt_is_correctly_rounded(x in prop_oneof![strat::any_finite(), strat::subnormal(), strat::wild()]) {
            let x = x.abs();
            prop_assert_eq!(math::sqrt(x).unwrap().get().to_bits(), rn::round_sqrt(x).to_bits(), "sqrt({:e})", x);
        }

        /// Every binary operation, bit for bit, in every regime of `strat::pair`.
        #[test]
        fn binary_operations_follow_the_documented_method_bit_for_bit((a, b) in strat::pair()) {
            for op in Op::ALL {
                check_binary(op, a, b)?;
                check_binary(op, b, a)?;
            }
            check_neg(a)?;
        }

        /// `sqrt` bit for bit on non-negative intervals of every kind, and the error below zero.
        #[test]
        fn sqrt_follows_the_documented_method_bit_for_bit(
            x in prop_oneof![strat::wild(), strat::subnormal(), Just(0.0), Just(-0.0)],
            y in prop_oneof![strat::wild(), strat::subnormal(), Just(0.0)],
        ) {
            check_sqrt(strat::hull(x.abs(), y.abs()))?;
            check_sqrt(strat::hull(x, y))?;
            check_sqrt(pt(x.abs()))?;
        }
    }

    proptest! {
        #![proptest_config(config(1024))]

        /// Chains of operations on point intervals: after every step the interval holds the exact
        /// value of the expression, and its certain sign, if any, is the exact sign. A final square
        /// root, when defined, holds the exact root.
        #[test]
        fn chained_operations_hold_the_exact_value(
            leaves in proptest::collection::vec(strat::moderate(), 4),
            steps in proptest::collection::vec((0usize..5, 0usize..64, 0usize..64), 1..9),
        ) {
            let mut values: Vec<(Interval, BigRational)> = leaves.iter().map(|&x| (pt(x), exact(x))).collect();
            for (op, i, j) in steps {
                let (a, va) = values[i % values.len()].clone();
                let (b, vb) = values[j % values.len()].clone();
                let next = if op < 4 {
                    let op = Op::ALL[op];
                    check_binary(op, a, b)?;
                    op.apply(a, b).ok().map(|r| (r, op.exact(&va, &vb)))
                } else {
                    Some((a.neg(), -va))
                };
                if let Some((r, v)) = next {
                    prop_assert!(holds(r, &v), "{:?} misses the exact value {}", r, v);
                    sign_is_sound(r, &[&v])?;
                    values.push((r, v));
                }
            }
            let (last, v) = values.last().unwrap().clone();
            if let Ok(root) = last.sqrt() {
                let (rl, rh) = (exact(root.lo()), exact(root.hi()));
                prop_assert!(&rl * &rl <= v && v <= &rh * &rh, "sqrt {:?} = {:?} misses the root of {}", last, root, v);
            }
        }

        /// Expressions whose exact value is zero, and sub-ulp quantities (the exact error of a
        /// rounded sum or product): the interval holds the exact value and never claims a wrong
        /// sign.
        #[test]
        fn certain_sign_is_never_wrong_on_cancellation(
            a in prop_oneof![strat::moderate(), strat::wild()],
            b in prop_oneof![strat::moderate(), strat::wild()],
            c in prop_oneof![strat::moderate(), strat::wild()],
        ) {
            let zero = BigRational::zero();
            let (pa, pb, pc) = (pt(a), pt(b), pt(c));
            let mut cases: Vec<(Result<Interval, NumError>, BigRational)> = vec![
                (pa.mul(pb).and_then(|ab| ab.mul(pc)).and_then(|l| pb.mul(pc).and_then(|bc| pa.mul(bc)).and_then(|r| l.sub(r))), zero.clone()),
                (pa.add(pb).and_then(|s| s.sub(pa)).and_then(|s| s.sub(pb)), zero.clone()),
                (pa.mul(pb).and_then(|x| pb.mul(pa).and_then(|y| x.sub(y))), zero.clone()),
                (pa.add(pb).and_then(|s| pa.sub(pb).and_then(|d| s.mul(d))).and_then(|l| pa.mul(pa).and_then(|aa| pb.mul(pb).and_then(|bb| aa.sub(bb))).and_then(|r| l.sub(r))), zero.clone()),
            ];
            if b != 0.0 {
                cases.push((pa.div(pb).and_then(|q| q.mul(pb)).and_then(|x| x.sub(pa)), zero.clone()));
            }
            // The exact error of the rounded product and sum, a quantity below half a unit.
            if (a * b).is_finite() {
                cases.push((pa.mul(pb).and_then(|x| x.sub(pt(a * b))), exact(a) * exact(b) - exact(a * b)));
            }
            if (a + b).is_finite() {
                cases.push((pa.add(pb).and_then(|x| x.sub(pt(a + b))), exact(a) + exact(b) - exact(a + b)));
            }
            for (r, v) in cases {
                if let Ok(r) = r {
                    prop_assert!(holds(r, &v), "{:?} misses the exact value {} (a = {:e}, b = {:e}, c = {:e})", r, v, a, b, c);
                    sign_is_sound(r, &[&v])?;
                }
            }
        }

        /// The interval of a product minus its rounded value holds `two_prod`'s exact error, and
        /// its certain sign, if any, is the sign of that error.
        #[test]
        fn interval_agrees_with_two_prod_on_the_sign_of_the_rounding_error(
            a in strat::normal(-400..=400),
            b in strat::normal(-400..=400),
        ) {
            let (p, e) = eft::two_prod(a, b);
            let r = pt(a).mul(pt(b)).unwrap().sub(pt(p)).unwrap();
            prop_assert!(holds(r, &exact(e)), "{:?} misses two_prod's error {:e}", r, e);
            sign_is_sound(r, &[&exact(e)])?;
        }
    }

    /// Every interval between two of these bounds against every other, through every operation, bit
    /// for bit: ±0, ±2^−1074, ±2^−1022, ±1, 2^512, ±2^1023, Ω − 2^971 and ±Ω.
    #[test]
    fn edge_grid_every_operation_bit_for_bit() {
        let bounds = [
            -OMEGA,
            -fp::pow2(1023),
            -1.0,
            -f64::MIN_POSITIVE,
            -f64::from_bits(1),
            -0.0,
            0.0,
            f64::from_bits(1),
            f64::MIN_POSITIVE,
            1.0,
            fp::pow2(512),
            fp::down(OMEGA),
            OMEGA,
        ];
        let mut intervals = Vec::new();
        for (i, &lo) in bounds.iter().enumerate() {
            for &hi in &bounds[i..] {
                intervals.push(Interval::new(lo, hi).unwrap());
            }
        }
        for &a in &intervals {
            run(check_neg(a));
            run(check_sqrt(a));
            run(check_square(a));
            run(check_sqrt_nonneg(a));
            for &b in &intervals {
                for op in Op::ALL {
                    run(check_binary(op, a, b));
                }
            }
        }
    }

    /// At every binade boundary 2^k (the step size halves just below it), sums, differences,
    /// products and quotients landing exactly on, just above, just below and on the tie below the
    /// boundary, and square roots of the boundary and its neighbours, bit for bit.
    #[test]
    fn binade_boundaries_bit_for_bit() {
        for k in (-1074..=1023).step_by(7) {
            let x = fp::pow2(k);
            for d in [k - 52, k - 53, k - 54, k - 106] {
                if d < -1074 {
                    continue;
                }
                let y = fp::pow2(d);
                for (a, b) in [(x, y), (x, -y), (-x, y), (x, 3.0 * y), (x, -3.0 * y)] {
                    if !b.is_finite() {
                        continue;
                    }
                    for op in [Op::Add, Op::Sub] {
                        run(check_binary(op, pt(a), pt(b)));
                        run(check_binary(op, hull(a, up(a)), hull(b, down(b))));
                    }
                }
            }
            for f in [
                1.0 + f64::EPSILON,
                1.0 - f64::EPSILON / 2.0,
                2.0 - f64::EPSILON,
                0.5 + f64::EPSILON / 4.0,
                3.0,
            ] {
                for op in [Op::Mul, Op::Div] {
                    run(check_binary(op, pt(x), pt(f)));
                    run(check_binary(op, pt(-x), pt(f)));
                    run(check_binary(op, hull(down(x), up(x)), pt(f)));
                }
            }
            for r in [x, up(x), down(x)] {
                run(check_sqrt(pt(r)));
            }
        }
    }

    /// The interval between two doubles in either order.
    fn hull(x: f64, y: f64) -> Interval {
        strat::hull(x, y)
    }

    /// Operands evaluated at compile time: ordinary, the overflow threshold, a subnormal divisor,
    /// touching zero and the whole line.
    const CT_OPERANDS: [((f64, f64), (f64, f64)); 6] = [
        ((0.1, 0.3), (-0.7, 0.2)),
        ((1.0, 1.0), (3.0, 3.0)),
        ((f64::MAX, f64::MAX), (0.0, 0.0)),
        ((1.0, 2.0), (5e-324, 5e-324)),
        ((-0.0, 1.0), (-1.0, -0.0)),
        ((-f64::MAX, f64::MAX), (0.5, 0.5)),
    ];

    /// One operation on two intervals made from bound pairs, in a constant context.
    const fn ct_op(x: (f64, f64), y: (f64, f64), op: usize) -> Result<Interval, NumError> {
        let a = match Interval::new(x.0, x.1) {
            Ok(i) => i,
            Err(e) => return Err(e),
        };
        let b = match Interval::new(y.0, y.1) {
            Ok(i) => i,
            Err(e) => return Err(e),
        };
        match op {
            0 => a.add(b),
            1 => a.sub(b),
            2 => a.mul(b),
            _ => a.div(b),
        }
    }

    /// Every operation on every operand pair, at compile time.
    const fn compile_time_results() -> [[Result<Interval, NumError>; 4]; 6] {
        let mut out = [[Err(NumError::NotFinite); 4]; 6];
        let mut i = 0;
        while i < CT_OPERANDS.len() {
            let mut op = 0;
            while op < 4 {
                out[i][op] = ct_op(CT_OPERANDS[i].0, CT_OPERANDS[i].1, op);
                op += 1;
            }
            i += 1;
        }
        out
    }

    /// The compile-time results.
    const COMPILE_TIME: [[Result<Interval, NumError>; 4]; 6] = compile_time_results();

    /// Determinism: compile-time (the compiler's software floating point) and run-time results
    /// agree bit for bit, and both follow the documented method.
    #[test]
    fn compile_time_and_run_time_agree() {
        for (i, &(x, y)) in CT_OPERANDS.iter().enumerate() {
            let a = Interval::new(std::hint::black_box(x.0), std::hint::black_box(x.1)).unwrap();
            let b = Interval::new(std::hint::black_box(y.0), std::hint::black_box(y.1)).unwrap();
            for (k, op) in Op::ALL.iter().enumerate() {
                assert_eq!(
                    bits(COMPILE_TIME[i][k]),
                    bits(op.apply(a, b)),
                    "{op:?} {a:?} {b:?}"
                );
                run(check_binary(*op, a, b));
            }
        }
    }

    /// The overflow threshold, exactly. The double below Ω is Ω − 2^971, with an even significand,
    /// so the exact value Ω − 2^970 (the midpoint) rounds down to it and the upper bound can still
    /// widen to Ω; anything above the midpoint rounds to Ω and must be `NotFinite`.
    #[test]
    fn ties_at_the_overflow_threshold() {
        let below = fp::down(OMEGA);
        let half = fp::pow2(970);
        let r = pt(below).add(pt(half)).unwrap();
        assert_eq!((r.lo(), r.hi()), (down(below), OMEGA), "Ω − 2^971 + 2^970");
        let r = pt(OMEGA).sub(pt(half)).unwrap();
        assert_eq!((r.lo(), r.hi()), (down(below), OMEGA), "Ω − 2^970");
        assert_eq!(
            pt(below).add(pt(up(half))),
            Err(NumError::NotFinite),
            "just above the midpoint"
        );
        assert_eq!(
            pt(OMEGA).sub(pt(down(half))),
            Err(NumError::NotFinite),
            "just above the midpoint"
        );
        // Mirrored below −Ω + 2^971.
        let r = pt(-below).sub(pt(half)).unwrap();
        assert_eq!((r.lo(), r.hi()), (-OMEGA, up(-below)));
        assert_eq!(pt(-below).sub(pt(up(half))), Err(NumError::NotFinite));
        // Products and quotients that land exactly on Ω, just below, and just above.
        assert_eq!(pt(OMEGA / 2.0).mul(pt(2.0)), Err(NumError::NotFinite));
        assert_eq!(pt(below / 2.0).mul(pt(2.0)).map(Interval::hi), Ok(OMEGA));
        assert_eq!(pt(OMEGA).div(pt(1.0)), Err(NumError::NotFinite));
        assert_eq!(pt(below).div(pt(1.0)).map(Interval::hi), Ok(OMEGA));
        assert_eq!(pt(below).div(pt(down(1.0))), Err(NumError::NotFinite));
        // Every result here also follows the documented method bit for bit.
        for (x, y) in [
            (below, half),
            (OMEGA, -half),
            (below, up(half)),
            (-below, -half),
            (OMEGA / 2.0, 2.0),
            (below, down(1.0)),
        ] {
            for op in Op::ALL {
                run(check_binary(op, pt(x), pt(y)));
            }
        }
    }

    /// Division at zero boundaries: zero and ±0 dividends, intervals touching zero, subnormal
    /// divisors on either side of zero, the whole line as dividend, and divisors with a ±0 bound.
    #[test]
    fn division_at_zero_boundaries() {
        let tiny = f64::from_bits(1);
        let dividends = [
            pt(0.0),
            Interval::new(-0.0, -0.0).unwrap(),
            Interval::new(0.0, tiny).unwrap(),
            Interval::new(-tiny, 0.0).unwrap(),
            Interval::new(-tiny, tiny).unwrap(),
            Interval::new(-1.0, 1.0).unwrap(),
            Interval::new(-OMEGA, OMEGA).unwrap(),
            pt(fp::pow2(-1060)),
            pt(OMEGA),
        ];
        let divisors = [
            pt(tiny),
            pt(-tiny),
            Interval::new(tiny, 1.0).unwrap(),
            Interval::new(-OMEGA, -tiny).unwrap(),
            Interval::new(tiny, OMEGA).unwrap(),
            pt(OMEGA),
            pt(-OMEGA),
            Interval::new(-0.0, 1.0).unwrap(),
            Interval::new(-1.0, -0.0).unwrap(),
            Interval::new(-0.0, -0.0).unwrap(),
            Interval::new(-tiny, tiny).unwrap(),
        ];
        for &a in &dividends {
            for &b in &divisors {
                run(check_binary(Op::Div, a, b));
            }
        }
        // A divisor that touches zero only through −0 still contains zero.
        assert_eq!(
            pt(1.0).div(Interval::new(-1.0, -0.0).unwrap()),
            Err(NumError::DivisorContainsZero)
        );
        // 2^−1060 / 2^−1074 = 2^14 exactly: a subnormal divisor that does not overflow.
        let q = pt(fp::pow2(-1060)).div(pt(tiny)).unwrap();
        assert!(q.contains(16_384.0) && q.certain_sign() == Some(Sign::Positive));
    }

    /// The square root at zero and subnormal bounds: a zero bound stays +0, a subnormal bound has a
    /// positive root, and the error is exactly for a negative lower bound.
    #[test]
    fn sqrt_at_zero_boundaries() {
        let tiny = f64::from_bits(1);
        let cases = [
            Interval::point(0.0).unwrap(),
            Interval::new(-0.0, -0.0).unwrap(),
            Interval::new(-0.0, tiny).unwrap(),
            Interval::new(tiny, tiny).unwrap(),
            Interval::new(0.0, f64::from_bits(fp::FRAC)).unwrap(),
            Interval::new(f64::from_bits(4), f64::MIN_POSITIVE).unwrap(),
            Interval::new(0.0, OMEGA).unwrap(),
            Interval::new(OMEGA, OMEGA).unwrap(),
            Interval::new(-tiny, 0.0).unwrap(),
            Interval::new(-tiny, 4.0).unwrap(),
        ];
        for a in cases {
            run(check_sqrt(a));
        }
        let z = Interval::new(-0.0, -0.0).unwrap().sqrt().unwrap();
        assert_eq!(
            (z.lo().to_bits(), z.hi().to_bits(), z.certain_sign()),
            (0, 0, Some(Sign::Zero))
        );
        // √(2^−1072) = 2^−536 exactly; the widened lower bound stays positive.
        let r = pt(f64::from_bits(4)).sqrt().unwrap();
        assert!(
            r.lo() > 0.0 && r.contains(fp::pow2(-536)) && r.certain_sign() == Some(Sign::Positive)
        );
    }

    /// Signed zeros in every position give the same bits as +0, and every zero bound is +0.
    #[test]
    fn signed_zeros_never_change_a_result() {
        let others = [
            1.0,
            -1.0,
            f64::from_bits(1),
            -f64::from_bits(1),
            OMEGA,
            -OMEGA,
            0.0,
        ];
        for &x in &others {
            let plus = strat::hull(0.0, x);
            let minus = strat::hull(-0.0, x);
            assert_eq!(
                (plus.lo().to_bits(), plus.hi().to_bits()),
                (minus.lo().to_bits(), minus.hi().to_bits())
            );
            for &y in &others {
                let b = strat::hull(y, y);
                for op in Op::ALL {
                    assert_eq!(
                        bits(op.apply(plus, b)),
                        bits(op.apply(minus, b)),
                        "{op:?} [±0, {x:e}] with {y:e}"
                    );
                    assert_eq!(
                        bits(op.apply(b, plus)),
                        bits(op.apply(b, minus)),
                        "{op:?} {y:e} with [±0, {x:e}]"
                    );
                    if let Ok(r) = op.apply(plus, b) {
                        assert!(
                            r.lo().to_bits() != (-0.0f64).to_bits()
                                && r.hi().to_bits() != (-0.0f64).to_bits(),
                            "{r:?}"
                        );
                    }
                }
            }
            let n = minus.neg();
            assert!(
                n.lo().to_bits() != (-0.0f64).to_bits() && n.hi().to_bits() != (-0.0f64).to_bits(),
                "{n:?}"
            );
        }
    }

    /// `contains` and `certain_sign` at the edges: ±0, NaN, infinities, the smallest subnormals.
    #[test]
    fn contains_and_certain_sign_at_the_edges() {
        let tiny = f64::from_bits(1);
        let r = Interval::new(0.0, 1.0).unwrap();
        assert!(r.contains(-0.0) && r.contains(0.0) && !r.contains(-tiny) && !r.contains(f64::NAN));
        let r = Interval::new(-1.0, -0.0).unwrap();
        assert!(r.contains(0.0) && r.contains(-0.0) && !r.contains(tiny));
        let all = Interval::new(-OMEGA, OMEGA).unwrap();
        assert!(
            !all.contains(f64::INFINITY) && !all.contains(f64::NEG_INFINITY) && all.contains(OMEGA)
        );
        let bounds = [-OMEGA, -1.0, -tiny, -0.0, 0.0, tiny, 1.0, OMEGA];
        for (i, &lo) in bounds.iter().enumerate() {
            for &hi in &bounds[i..] {
                let r = Interval::new(lo, hi).unwrap();
                let (l, h) = (exact(lo), exact(hi));
                match r.certain_sign() {
                    Some(s) => assert!(sign(&l) == s && sign(&h) == s, "{r:?} claims {s:?}"),
                    // Undecided only when the interval holds zero and a nonzero real.
                    None => assert!(
                        sign(&l) != Sign::Positive
                            && sign(&h) != Sign::Negative
                            && !(lo == 0.0 && hi == 0.0),
                        "{r:?} undecided"
                    ),
                }
            }
        }
    }

    /// A curved computation's most basic certificate: the distance between two coincident points,
    /// √(dx² + dy²) with dx = dy = x − x. The exact value is 0, but every bound is widened, so
    /// dx = [−2^−1074, 2^−1074] and dx² + dy² has a negative lower bound, with `mul` and with
    /// `square` alike (the sum widens 0 + 0 too). The strict `sqrt` must refuse that interval: its
    /// documentation and tests/`eft_interval`.rs (`square_root_cases`, `checks::sqrt`) require
    /// `NegativeSqrt` whenever the lower bound is negative, and the always-widen method is pinned
    /// there too (`overflow_gives_not_finite`, `zero_bounds_are_stored_as_positive_zero`, and the
    /// collinear cases of `orient2d_at_extreme_coordinates`, which must stay undecided). The
    /// certificate comes from `sqrt_nonneg`, the root of the non-negative part, which a squared
    /// distance is by construction: it must hold 0, start at exactly 0 and stay tight.
    #[test]
    fn distance_between_coincident_points_is_certifiable() {
        let tiny = f64::from_bits(1);
        for x in [3.0, 0.0, -1.5e300, tiny, ((1u64 << 40) - 1) as f64, -OMEGA] {
            let p = pt(x);
            let dx = p.sub(p).unwrap();
            assert_eq!((dx.lo(), dx.hi()), (-tiny, tiny), "{x:e} − {x:e}");
            let by_mul = dx.mul(dx).unwrap().add(dx.mul(dx).unwrap()).unwrap();
            let by_square = dx.square().unwrap().add(dx.square().unwrap()).unwrap();
            for d2 in [by_mul, by_square] {
                assert!(d2.contains(0.0) && d2.lo() < 0.0, "{d2:?}");
                assert_eq!(
                    d2.sqrt(),
                    Err(NumError::NegativeSqrt),
                    "the strict root of {d2:?}"
                );
                run(check_sqrt_nonneg(d2));
                let r = sqrt_nonneg(d2);
                assert!(
                    matches!(r, Ok(i) if i.lo().to_bits() == 0 && i.contains(0.0) && i.hi() < fp::pow2(-530)),
                    "√(dx² + dy²) for coincident points: dx = {dx:?}, dx² + dy² = {d2:?}, sqrt_nonneg gave {r:?}"
                );
            }
        }
    }

    /// `square` and `sqrt_nonneg` at their edges: zero and ±0 bounds, intervals straddling zero,
    /// squares that underflow (the lower bound must stay at 0, not step to −2^−1074), squares at
    /// the overflow threshold, wholly negative intervals.
    #[test]
    fn square_and_sqrt_nonneg_at_the_edges() {
        let tiny = f64::from_bits(1);
        let cases = [
            pt(0.0),
            Interval::new(-0.0, -0.0).unwrap(),
            Interval::new(-tiny, tiny).unwrap(),
            Interval::new(-1.0, 2.0).unwrap(),
            Interval::new(-3.0, -2.0).unwrap(),
            Interval::new(fp::pow2(-600), 1.0).unwrap(),
            Interval::new(-1.0, -fp::pow2(-600)).unwrap(),
            Interval::new(tiny, fp::pow2(-537)).unwrap(),
            pt(fp::down(fp::pow2(512))),
            pt(fp::pow2(512)),
            Interval::new(-OMEGA, OMEGA).unwrap(),
            Interval::new(-OMEGA, -1.0).unwrap(),
        ];
        for a in cases {
            run(check_square(a));
            run(check_sqrt_nonneg(a));
            run(check_sqrt_nonneg(a.neg()));
        }
        // [−1, 2]: the squares are [0, 4]; `mul` cannot know the factors are equal.
        let r = Interval::new(-1.0, 2.0).unwrap().square().unwrap();
        assert_eq!((r.lo().to_bits(), r.hi()), (0, up(4.0)));
        // A positive square that underflows keeps a lower bound of exactly 0.
        let r = pt(fp::pow2(-600)).square().unwrap();
        assert_eq!((r.lo().to_bits(), r.hi()), (0, tiny));
        // The largest square that can still widen, and the first that cannot.
        assert!(pt(fp::down(fp::pow2(512))).square().is_ok());
        assert_eq!(pt(fp::pow2(512)).square(), Err(NumError::NotFinite));
        // Wholly negative: no non-negative part to take the root of.
        assert_eq!(
            sqrt_nonneg(Interval::new(-2.0, -1.0).unwrap()),
            Err(NumError::NegativeSqrt)
        );
        assert_eq!(
            sqrt_nonneg(Interval::new(-tiny, 0.0).unwrap()).map(bits_of),
            Ok((0, 0))
        );
    }

    /// The bits of an interval's bounds.
    fn bits_of(r: Interval) -> (u64, u64) {
        (r.lo().to_bits(), r.hi().to_bits())
    }

    proptest! {
        #![proptest_config(config(4096))]

        /// `square` bit for bit, in every regime, and monotone: a wider operand never gives a
        /// narrower square.
        #[test]
        fn square_follows_the_documented_method(a in strat::interval(), c in strat::wild()) {
            check_square(a)?;
            let wider = strat::hull(if c < a.lo() { c } else { a.lo() }, if c > a.hi() { c } else { a.hi() });
            check_square(wider)?;
            if let Ok(big) = wider.square() {
                let small = a.square();
                prop_assert!(
                    matches!(small, Ok(s) if big.lo() <= s.lo() && s.hi() <= big.hi()),
                    "square {:?} = {:?} is not inside square {:?} = {:?}", a, small, wider, big
                );
            }
        }

        /// `sqrt_nonneg` bit for bit on intervals of every kind, both signs.
        #[test]
        fn sqrt_nonneg_follows_the_documented_method(a in strat::interval()) {
            check_sqrt_nonneg(a)?;
            check_sqrt_nonneg(a.neg())?;
        }
    }

    proptest! {
        #![proptest_config(config(1024))]

        /// Points known only to boxes a few steps wide (the output of a curved construction), the
        /// two boxes overlapping, touching or a few steps apart. The distance √(dx² + dy²), with
        /// `square` and with `mul`, through `sqrt_nonneg`, is always certified: it never refuses,
        /// holds the exact distance between every pair of corner points (checked by squaring, in
        /// rationals), starts at exactly 0 when both coordinate differences can be zero, and the
        /// `square` form is inside the `mul` form.
        #[test]
        fn distance_between_nearly_coincident_points_is_certified(
            x in strat::moderate(),
            y in strat::moderate(),
            widths in proptest::collection::vec(0i64..4, 4),
            shift in proptest::collection::vec(-4i64..=4, 2),
        ) {
            let px = strat::hull(x, steps(x, widths[0]));
            let py = strat::hull(y, steps(y, widths[1]));
            let qx = strat::hull(steps(x, shift[0]), steps(x, shift[0] + widths[2]));
            let qy = strat::hull(steps(y, shift[1]), steps(y, shift[1] + widths[3]));
            let (Ok(dx), Ok(dy)) = (qx.sub(px), qy.sub(py)) else { return Ok(()) };
            let by_square = dx.square().and_then(|a| dy.square().and_then(|b| a.add(b)));
            let by_mul = dx.mul(dx).and_then(|a| dy.mul(dy).and_then(|b| a.add(b)));
            let (Ok(s2), Ok(m2)) = (by_square, by_mul) else { return Ok(()) };
            check_sqrt_nonneg(s2)?;
            check_sqrt_nonneg(m2)?;
            let (rs, rm) = (sqrt_nonneg(s2).unwrap(), sqrt_nonneg(m2).unwrap());
            prop_assert!(rm.lo() <= rs.lo() && rs.hi() <= rm.hi(), "square form {:?} not inside mul form {:?}", rs, rm);
            for (pxv, qxv) in [(px.lo(), qx.lo()), (px.lo(), qx.hi()), (px.hi(), qx.lo()), (px.hi(), qx.hi())] {
                for (pyv, qyv) in [(py.lo(), qy.lo()), (py.lo(), qy.hi()), (py.hi(), qy.lo()), (py.hi(), qy.hi())] {
                    let (ddx, ddy) = (exact(qxv) - exact(pxv), exact(qyv) - exact(pyv));
                    let d2 = &ddx * &ddx + &ddy * &ddy;
                    let (lo, hi) = (exact(rs.lo()), exact(rs.hi()));
                    prop_assert!(&lo * &lo <= d2 && d2 <= &hi * &hi, "distance {:?} misses √{} ({:?} to {:?})", rs, d2, (pxv, pyv), (qxv, qyv));
                }
            }
            let zero = BigRational::zero();
            let can_touch = |p: Interval, q: Interval| exact(q.lo()) - exact(p.hi()) <= zero && exact(q.hi()) - exact(p.lo()) >= zero;
            if can_touch(px, qx) && can_touch(py, qy) {
                prop_assert_eq!(rs.lo().to_bits(), 0, "the boxes can touch, so the distance {:?} must start at 0", rs);
            }
        }
    }

    /// Operands of `square`, evaluated at compile time like the other operations.
    const CT_SQUARES: [(f64, f64); 5] = [
        (-1.0, 2.0),
        (0.1, 0.3),
        (-3.0, -2.0),
        (1.0e-300, 1.0e-200),
        (-1.0e200, 1.0e154),
    ];

    /// `square` of each operand, at compile time.
    const fn compile_time_squares() -> [Result<Interval, NumError>; 5] {
        let mut out = [Err(NumError::NotFinite); 5];
        let mut i = 0;
        while i < CT_SQUARES.len() {
            out[i] = match Interval::new(CT_SQUARES[i].0, CT_SQUARES[i].1) {
                Ok(a) => a.square(),
                Err(e) => Err(e),
            };
            i += 1;
        }
        out
    }

    /// The compile-time squares.
    const COMPILE_TIME_SQUARES: [Result<Interval, NumError>; 5] = compile_time_squares();

    /// Determinism of `square`: compile time and run time agree bit for bit, and follow the method.
    #[test]
    fn square_compile_time_and_run_time_agree() {
        for (i, &(lo, hi)) in CT_SQUARES.iter().enumerate() {
            let a = Interval::new(std::hint::black_box(lo), std::hint::black_box(hi)).unwrap();
            assert_eq!(
                bits(COMPILE_TIME_SQUARES[i]),
                bits(a.square()),
                "square {a:?}"
            );
            run(check_square(a));
        }
    }
}
