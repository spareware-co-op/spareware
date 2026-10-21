//! Tests of `geom_num::eft` and `geom_num::interval` against exact rational arithmetic (`num-rational`).
//!
//! What this file proves:
//!
//! - Error-free transformations. `two_sum`, `fast_two_sum` and `two_prod` return a pair whose exact
//!   rational sum equals the exact sum or product of the inputs, whose first part is the IEEE result,
//!   and which does not overlap (`s + e` rounds back to `s`), for every input inside each function's
//!   documented precondition: random doubles of every exponent, overlapping and cancelling pairs, ties,
//!   subnormals, powers of two, the edges of the precondition, the overflow edge of `two_sum` (including
//!   the case where Knuth's textbook form overflows and the fallback must act) and grid coordinates at
//!   ±(2^40 − 1). `split` gives two 26-bit halves that add up to the input exactly. A product just
//!   outside `two_prod`'s lower exponent limit has an error no double can hold, so the limit is sharp.
//! - Intervals. For every operation (`add`, `sub`, `mul`, `div`, `sqrt`, `neg`) and random operands
//!   of every magnitude, the result contains the exact rational result at every corner and at an
//!   exact interior point; each bound is less than two steps outside the exact bound (tight);
//!   `NotFinite` is returned exactly when a bound cannot be widened to a finite double; the other
//!   errors exactly when the operation is undefined somewhere in the operands. Laws: commutativity,
//!   `a − b = a + (−b)`, negation is an exact involution, inclusion is monotone. `certain_sign` is
//!   right whenever it answers and answers whenever the interval alone decides the sign, including on
//!   orientation determinants of grid points at ±(2^40 − 1).
//!
//! How the file is built: `oracle` reads doubles as exact rationals from their bits and steps between
//! neighbouring doubles by their bits, independently of the code under test (and is checked on its
//! own first). `strategies` makes doubles weighted towards where bugs live. `checks` holds one
//! function per property, shared by the property tests and the exhaustive edge sweeps.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings.
// A broken oracle or strategy must stop the test loudly, so the kernel's panic lints are relaxed
// for this test file only (clippy.toml: "Tests may use them").
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use proptest::test_runner::{Config, TestCaseError};

/// The outcome of one check: `Err` carries the first disagreement with the oracle.
type TestResult = Result<(), TestCaseError>;

/// The configuration a property runs with: `cases` random cases, each compared with the oracle.
fn config(cases: u32) -> Config {
    Config { cases, ..Config::default() }
}

/// Cases per error-free transformation property (each case is a handful of rational operations).
const EFT_CASES: u32 = 4096;

/// Cases per interval property (each case is a few dozen rational operations).
const INTERVAL_CASES: u32 = 1024;

/// Turns a check's failure into a test failure, for checks run outside `proptest!`.
fn run(result: TestResult) {
    if let Err(e) = result {
        panic!("{e}");
    }
}

/// The exact model: doubles read as rationals from their bits, neighbouring doubles found from their
/// bits, signs of rationals. None of it calls the code under test.
mod oracle {
    use geom_num::Sign;
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use num_traits::{One, Signed, Zero};

    /// The bits of the fraction field of a double.
    pub const FRACTION_MASK: u64 = (1 << 52) - 1;

    /// 2^k exactly, for any integer k.
    pub fn pow2(k: i32) -> BigRational {
        let p = BigInt::one() << k.unsigned_abs();
        if k >= 0 { BigRational::from_integer(p) } else { BigRational::new(BigInt::one(), p) }
    }

    /// The exact value of a finite double, read from its sign, exponent and fraction bits.
    pub fn exact(x: f64) -> BigRational {
        assert!(x.is_finite(), "exact() of a non-finite double {x}");
        let bits = x.to_bits();
        let biased = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & FRACTION_MASK;
        let (significand, exponent) = if biased == 0 {
            (fraction, -1074)
        } else {
            (fraction | (1 << 52), biased - 1075)
        };
        let magnitude = BigRational::from_integer(BigInt::from(significand)) * pow2(exponent);
        if bits >> 63 == 1 { -magnitude } else { magnitude }
    }

    /// A normal double from its sign, unbiased exponent (−1022 ..= 1023) and 52 fraction bits.
    pub fn make(negative: bool, exponent: i32, fraction: u64) -> f64 {
        assert!((-1022..=1023).contains(&exponent), "exponent {exponent} is not normal");
        let biased = u64::try_from(exponent + 1023).unwrap();
        f64::from_bits((u64::from(negative) << 63) | (biased << 52) | (fraction & FRACTION_MASK))
    }

    /// 2^k as a double, for normal k.
    pub fn pow2_f64(k: i32) -> f64 {
        make(false, k, 0)
    }

    /// The binary exponent e of a normal double: 2^e ≤ |x| < 2^(e + 1).
    pub fn exponent(x: f64) -> i32 {
        assert!(x.is_normal(), "exponent() of {x:e}, which is not normal");
        i32::try_from((x.to_bits() >> 52) & 0x7ff).unwrap() - 1023
    }

    /// The number of significant bits of a finite double: from its highest to its lowest set
    /// significand bit (0 for zero).
    pub fn significant_bits(x: f64) -> u32 {
        let bits = x.to_bits();
        let fraction = bits & FRACTION_MASK;
        let significand = if (bits >> 52) & 0x7ff == 0 { fraction } else { fraction | (1 << 52) };
        if significand == 0 { 0 } else { 64 - significand.leading_zeros() - significand.trailing_zeros() }
    }

    /// The next double above a finite double (+∞ above the largest).
    pub fn step_up(x: f64) -> f64 {
        assert!(x.is_finite());
        if x == 0.0 {
            f64::from_bits(1)
        } else if x > 0.0 {
            f64::from_bits(x.to_bits() + 1)
        } else {
            f64::from_bits(x.to_bits() - 1)
        }
    }

    /// The next double below a finite double (−∞ below the most negative).
    pub fn step_down(x: f64) -> f64 {
        -step_up(-x)
    }

    /// The sign of a rational.
    pub fn sign(r: &BigRational) -> Sign {
        if r.is_positive() {
            Sign::Positive
        } else if r.is_negative() {
            Sign::Negative
        } else {
            Sign::Zero
        }
    }

    /// Whether a real rounds to nearest to a double of magnitude at least the largest finite double
    /// Ω = (2^53 − 1) · 2^971, or overflows. The double below Ω is Ω − 2^971, with an even
    /// significand, so the midpoint Ω − 2^970 itself rounds down: the test is |x| > Ω − 2^970.
    pub fn rounds_to_max_or_beyond(x: &BigRational) -> bool {
        static THRESHOLD: std::sync::OnceLock<BigRational> = std::sync::OnceLock::new();
        x.abs() > *THRESHOLD.get_or_init(|| exact(f64::MAX) - pow2(970))
    }

    /// The exact point a fraction `t / 2^16` of the way from `lo` to `hi`.
    pub fn interior(lo: &BigRational, hi: &BigRational, t: u32) -> BigRational {
        lo + (hi - lo) * BigRational::new(BigInt::from(t), BigInt::from(1u32 << 16))
    }

    /// The smallest and largest of some rationals.
    pub fn min_max(values: &[BigRational]) -> (BigRational, BigRational) {
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

    /// Zero, as a rational.
    pub fn zero() -> BigRational {
        BigRational::zero()
    }
}

/// Doubles weighted towards where bugs live.
mod strategies {
    use super::oracle::{self, FRACTION_MASK};
    use geom_num::grid::COORD_MAX;
    use geom_num::interval::Interval;
    use proptest::prelude::*;

    /// 52 fraction bits: random, or a pattern (zero, all ones, one bit, runs of ones at either end)
    /// that makes ties, carries and splits that round up.
    pub fn fraction() -> BoxedStrategy<u64> {
        prop_oneof![
            4 => any::<u64>().prop_map(|f| f & FRACTION_MASK),
            1 => Just(0u64),
            1 => Just(FRACTION_MASK),
            1 => (0u32..52).prop_map(|k| 1u64 << k),
            1 => (0u32..52).prop_map(|k| FRACTION_MASK >> k),
            1 => (0u32..52).prop_map(|k| (FRACTION_MASK << k) & FRACTION_MASK),
        ]
        .boxed()
    }

    /// A normal double with its exponent in `range`.
    pub fn normal(range: std::ops::RangeInclusive<i32>) -> BoxedStrategy<f64> {
        (any::<bool>(), range, fraction()).prop_map(|(s, e, f)| oracle::make(s, e, f)).boxed()
    }

    /// A nonzero subnormal double.
    pub fn subnormal() -> BoxedStrategy<f64> {
        (any::<bool>(), 1u64..(1 << 52))
            .prop_map(|(s, f)| f64::from_bits((u64::from(s) << 63) | f))
            .boxed()
    }

    /// Any finite double, uniform over bit patterns (so uniform over exponents).
    pub fn any_finite() -> BoxedStrategy<f64> {
        any::<u64>().prop_map(f64::from_bits).prop_filter("finite", |x| x.is_finite()).boxed()
    }

    /// A grid coordinate in steps: anywhere in ±(2^40 − 1), near the extremes, or near zero.
    pub fn coordinate() -> BoxedStrategy<i64> {
        prop_oneof![
            2 => -COORD_MAX..=COORD_MAX,
            1 => (0i64..16).prop_map(|k| COORD_MAX - k),
            1 => (0i64..16).prop_map(|k| k - COORD_MAX),
            1 => -16i64..=16,
        ]
        .boxed()
    }

    /// Fixed doubles at the edges of the format.
    pub fn edge_values() -> Vec<f64> {
        let mut v = vec![
            0.0,
            f64::from_bits(1),                       // 2^−1074, the smallest subnormal
            f64::from_bits(FRACTION_MASK),           // the largest subnormal
            f64::MIN_POSITIVE,                       // 2^−1022
            f64::EPSILON,                            // 2^−52
            0.5,
            1.0,
            1.0 + f64::EPSILON,
            2.0 - f64::EPSILON,
            3.0,
            ((1u64 << 40) - 1) as f64,               // COORD_MAX
            ((1u64 << 41) - 2) as f64,               // the largest coordinate difference
            ((1u64 << 53) - 1) as f64,
            9_007_199_254_740_992.0,                 // 2^53
            oracle::pow2_f64(995) * (2.0 - f64::EPSILON), // the largest double below 2^996
            oracle::pow2_f64(996),
            3.0 * oracle::pow2_f64(970),
            oracle::pow2_f64(1023),
            f64::MAX - oracle::pow2_f64(971),
            f64::MAX,
        ];
        let negated: Vec<f64> = v.iter().map(|x| -x).collect();
        v.extend(negated);
        v
    }

    /// A double for interval bounds: every magnitude, moderate values, grid coordinates, tiny and
    /// huge values and the format's edges.
    pub fn double() -> BoxedStrategy<f64> {
        prop_oneof![
            2 => any_finite(),
            3 => -1.0e6f64..1.0e6,
            2 => coordinate().prop_map(|c| c as f64),
            1 => subnormal(),
            1 => normal(-1022..=-1000),
            1 => normal(1000..=1023),
            1 => prop::sample::select(edge_values()),
        ]
        .boxed()
    }

    /// The interval between two doubles, in either order.
    fn hull(x: f64, y: f64) -> Interval {
        if x <= y { Interval::new(x, y).unwrap() } else { Interval::new(y, x).unwrap() }
    }

    /// An interval: between two random doubles, a point, or a few steps wide.
    pub fn interval() -> BoxedStrategy<Interval> {
        prop_oneof![
            4 => (double(), double()).prop_map(|(x, y)| hull(x, y)),
            1 => double().prop_map(|x| Interval::point(x).unwrap()),
            1 => (double(), 0usize..8).prop_map(|(x, k)| {
                let mut y = x;
                for _ in 0..k {
                    let next = oracle::step_up(y);
                    if next.is_finite() {
                        y = next;
                    }
                }
                hull(x, y)
            }),
        ]
        .boxed()
    }

    /// A divisor: any interval, or one with both bounds of one sign (so zero is outside).
    pub fn divisor() -> BoxedStrategy<Interval> {
        prop_oneof![
            1 => interval(),
            2 => (double(), double(), any::<bool>()).prop_filter_map("zero bound", |(x, y, negative)| {
                let (x, y) = (x.abs(), y.abs());
                if x == 0.0 || y == 0.0 {
                    return None;
                }
                Some(if negative { hull(-x, -y) } else { hull(x, y) })
            }),
        ]
        .boxed()
    }

    /// An interval of non-negative doubles (sometimes reaching zero), or any interval.
    pub fn radicand() -> BoxedStrategy<Interval> {
        prop_oneof![
            3 => (double(), double()).prop_map(|(x, y)| hull(x.abs(), y.abs())),
            1 => double().prop_map(|x| hull(0.0, x.abs())),
            1 => interval(),
        ]
        .boxed()
    }

    /// A pair for `two_sum`: random, overlapping (exponents up to 60 apart, so the sum rounds),
    /// cancelling, near overflow, subnormal or grid coordinates.
    pub fn sum_pair() -> BoxedStrategy<(f64, f64)> {
        let overlapping = (any::<bool>(), -1022i32..=1023, fraction(), any::<bool>(), -60i32..=60, fraction())
            .prop_map(|(sa, ea, fa, sb, d, fb)| {
                (oracle::make(sa, ea, fa), oracle::make(sb, (ea + d).clamp(-1022, 1023), fb))
            });
        let cancelling = (prop_oneof![normal(-1022..=1023), subnormal()], -16i64..=16).prop_map(|(a, k)| {
            let magnitude = (a.to_bits() & !(1 << 63)).saturating_add_signed(k).min(f64::MAX.to_bits());
            let m = f64::from_bits(magnitude);
            (a, if a < 0.0 { m } else { -m })
        });
        let near_overflow = (
            normal(1013..=1023),
            prop_oneof![
                normal(1013..=1023),
                normal(960..=980),
                (1u64..256, any::<bool>()).prop_map(|(k, s)| {
                    let v = k as f64 * oracle::pow2_f64(970);
                    if s { -v } else { v }
                }),
            ],
        );
        let tiny = prop_oneof![subnormal(), normal(-1022..=-1000)].boxed();
        prop_oneof![
            2 => (any_finite(), any_finite()),
            3 => overlapping,
            2 => cancelling,
            1 => near_overflow,
            1 => (tiny.clone(), tiny),
            1 => (coordinate(), coordinate()).prop_map(|(x, y)| (x as f64, y as f64)),
            1 => (double(), double()),
        ]
        .boxed()
    }

    /// A pair inside `two_prod`'s precondition: both normal, exponents at most 995, and
    /// −970 ≤ e_a + e_b ≤ 1021, with the exponent sum often at one of its limits.
    pub fn prod_pair() -> BoxedStrategy<(f64, f64)> {
        (-1022i32..=995)
            .prop_flat_map(|ea| {
                let lo = (-970 - ea).max(-1022);
                let hi = (1021 - ea).min(995);
                (
                    Just(ea),
                    prop_oneof![2 => lo..=hi, 1 => Just(lo), 1 => Just(hi)],
                    fraction(),
                    fraction(),
                    any::<bool>(),
                    any::<bool>(),
                )
            })
            .prop_map(|(ea, eb, fa, fb, sa, sb)| (oracle::make(sa, ea, fa), oracle::make(sb, eb, fb)))
            .boxed()
    }

    /// A double inside `split`'s precondition: zero, or normal with magnitude below 2^996.
    pub fn splittable() -> BoxedStrategy<f64> {
        prop_oneof![
            8 => normal(-1022..=995),
            1 => Just(0.0),
            1 => coordinate().prop_map(|c| c as f64),
        ]
        .boxed()
    }
}

/// One function per property, shared by property tests and edge sweeps.
mod checks {
    use super::TestResult;
    use super::oracle::{
        exact, exponent, interior, min_max, pow2_f64, rounds_to_max_or_beyond, sign, significant_bits,
        step_down, step_up, zero,
    };
    use geom_num::NumError;
    use geom_num::eft;
    use geom_num::interval::Interval;
    use num_rational::BigRational;
    use proptest::prelude::*;
    use proptest::test_runner::TestCaseError;

    /// Whether a pair is inside `two_prod`'s documented precondition.
    pub fn in_prod_domain(a: f64, b: f64) -> bool {
        let limit = pow2_f64(996);
        if !(a.abs() < limit && b.abs() < limit) {
            return false;
        }
        if a == 0.0 || b == 0.0 {
            return true;
        }
        a.is_normal() && b.is_normal() && (-970..=1021).contains(&(exponent(a) + exponent(b)))
    }

    /// Whether a double is inside `split`'s documented precondition.
    pub fn in_split_domain(a: f64) -> bool {
        a == 0.0 || (a.is_normal() && a.abs() < pow2_f64(996))
    }

    /// The pair `(s, e)` from a two-sum of `a` and `b` is exact, IEEE and non-overlapping when the
    /// rounded sum is finite, and is that infinity with the opposite infinity (never NaN) when not.
    fn sum_pair_is_exact(a: f64, b: f64, s: f64, e: f64, name: &str) -> TestResult {
        let rounded = a + b;
        prop_assert_eq!(s.to_bits(), rounded.to_bits(), "{}({:e}, {:e}): s is not the IEEE sum", name, a, b);
        if rounded.is_finite() {
            prop_assert!(e.is_finite(), "{}({:e}, {:e}): error {:e} is not finite", name, a, b, e);
            prop_assert_eq!(exact(s) + exact(e), exact(a) + exact(b), "{}({:e}, {:e}) is not exact", name, a, b);
            prop_assert_eq!(s + e, s, "{}({:e}, {:e}): the pair overlaps", name, a, b);
        } else {
            prop_assert!(
                e.is_infinite() && (e > 0.0) != (s > 0.0),
                "{}({:e}, {:e}): overflowed sum {:e} must carry the opposite infinity, got {:e}",
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
    pub fn two_sum(a: f64, b: f64) -> TestResult {
        let (s, e) = eft::two_sum(a, b);
        sum_pair_is_exact(a, b, s, e, "two_sum")
    }

    /// `fast_two_sum` on a pair with `|a| ≥ |b|` or `a == 0`.
    pub fn fast_two_sum(a: f64, b: f64) -> TestResult {
        prop_assert!(a.abs() >= b.abs() || a == 0.0, "precondition of fast_two_sum broken by the test");
        let (s, e) = eft::fast_two_sum(a, b);
        sum_pair_is_exact(a, b, s, e, "fast_two_sum")
    }

    /// Laws of `two_sum`: symmetric in its operands, odd under negating both, and equal to
    /// `fast_two_sum` with the larger operand first.
    pub fn two_sum_laws(a: f64, b: f64) -> TestResult {
        let (s, e) = eft::two_sum(a, b);
        if !s.is_finite() {
            return Ok(());
        }
        let (s2, e2) = eft::two_sum(b, a);
        prop_assert_eq!(s.to_bits(), s2.to_bits());
        prop_assert_eq!(e, e2);
        let (s3, e3) = eft::two_sum(-a, -b);
        prop_assert_eq!(s3, -s);
        prop_assert_eq!(e3, -e);
        let (big, small) = if a.abs() >= b.abs() { (a, b) } else { (b, a) };
        let (s4, e4) = eft::fast_two_sum(big, small);
        prop_assert_eq!(s4.to_bits(), s.to_bits());
        prop_assert_eq!(e4, e);
        Ok(())
    }

    /// `split` gives two halves of at most 26 significant bits, the high one at least as large,
    /// adding up to the input exactly.
    pub fn split(a: f64) -> TestResult {
        prop_assert!(in_split_domain(a), "precondition of split broken by the test");
        let (hi, lo) = eft::split(a);
        prop_assert!(hi.is_finite() && lo.is_finite(), "split({:e}) = ({:e}, {:e})", a, hi, lo);
        prop_assert_eq!(exact(hi) + exact(lo), exact(a), "split({:e}) does not add up", a);
        prop_assert!(significant_bits(hi) <= 26, "split({:e}): hi {:e} has {} bits", a, hi, significant_bits(hi));
        prop_assert!(significant_bits(lo) <= 26, "split({:e}): lo {:e} has {} bits", a, lo, significant_bits(lo));
        prop_assert!(hi.abs() >= lo.abs(), "split({:e}): |hi| < |lo|", a);
        Ok(())
    }

    /// `two_prod` on a pair inside its precondition: exact, IEEE, non-overlapping; and its laws
    /// (symmetric, odd in each factor).
    pub fn two_prod(a: f64, b: f64) -> TestResult {
        prop_assert!(in_prod_domain(a, b), "precondition of two_prod broken by the test");
        let (p, e) = eft::two_prod(a, b);
        prop_assert_eq!(p.to_bits(), (a * b).to_bits(), "two_prod({:e}, {:e}): p is not the IEEE product", a, b);
        prop_assert!(e.is_finite(), "two_prod({:e}, {:e}): error {:e} is not finite", a, b, e);
        prop_assert_eq!(exact(p) + exact(e), exact(a) * exact(b), "two_prod({:e}, {:e}) is not exact", a, b);
        prop_assert_eq!(p + e, p, "two_prod({:e}, {:e}): the pair overlaps", a, b);
        let (p2, e2) = eft::two_prod(b, a);
        prop_assert_eq!(p2.to_bits(), p.to_bits());
        prop_assert_eq!(e2, e);
        let (p3, e3) = eft::two_prod(-a, b);
        prop_assert_eq!(p3, -p);
        prop_assert_eq!(e3, -e);
        Ok(())
    }

    /// The invariant of every interval: finite, ordered, zero bounds stored as +0.
    pub fn invariant(r: Interval) -> TestResult {
        prop_assert!(r.lo().is_finite() && r.hi().is_finite(), "{:?} is not finite", r);
        prop_assert!(r.lo() <= r.hi(), "{:?} is empty", r);
        prop_assert_ne!(r.lo().to_bits(), (-0.0f64).to_bits(), "{:?} has a −0 bound", r);
        prop_assert_ne!(r.hi().to_bits(), (-0.0f64).to_bits(), "{:?} has a −0 bound", r);
        Ok(())
    }

    /// The four interval operations with two operands.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Op {
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
        pub const ALL: [Op; 4] = [Op::Add, Op::Sub, Op::Mul, Op::Div];

        /// The operation on intervals, by the code under test.
        pub fn interval(self, a: Interval, b: Interval) -> Result<Interval, NumError> {
            match self {
                Op::Add => a.add(b),
                Op::Sub => a.sub(b),
                Op::Mul => a.mul(b),
                Op::Div => a.div(b),
            }
        }

        /// The operation on exact rationals.
        pub fn exact(self, x: &BigRational, y: &BigRational) -> BigRational {
            match self {
                Op::Add => x + y,
                Op::Sub => x - y,
                Op::Mul => x * y,
                Op::Div => x / y,
            }
        }
    }

    /// An operation on two intervals: errors exactly when it should; otherwise the result contains the
    /// exact result at all four corners and at an exact interior point, and each bound is less than
    /// two steps outside the exact range.
    pub fn binary(op: Op, a: Interval, b: Interval, t: (u32, u32)) -> TestResult {
        let result = op.interval(a, b);
        if op == Op::Div && b.lo() <= 0.0 && b.hi() >= 0.0 {
            prop_assert_eq!(result, Err(NumError::DivisorContainsZero), "{:?} / {:?}", a, b);
            return Ok(());
        }
        let (alo, ahi, blo, bhi) = (exact(a.lo()), exact(a.hi()), exact(b.lo()), exact(b.hi()));
        let corners = [op.exact(&alo, &blo), op.exact(&alo, &bhi), op.exact(&ahi, &blo), op.exact(&ahi, &bhi)];
        let (m, big_m) = min_max(&corners);
        let unwidenable = rounds_to_max_or_beyond(&m) || rounds_to_max_or_beyond(&big_m);
        let r = match result {
            Ok(r) => r,
            Err(e) => {
                prop_assert_eq!(e, NumError::NotFinite, "{:?} {:?} {:?}", op, a, b);
                prop_assert!(unwidenable, "{:?} {:?} {:?}: NotFinite although the bounds can be widened", op, a, b);
                return Ok(());
            }
        };
        prop_assert!(!unwidenable, "{:?} {:?} {:?} = {:?}: a bound at ±Ω cannot have been widened", op, a, b, r);
        invariant(r)?;
        let (rlo, rhi) = (exact(r.lo()), exact(r.hi()));
        let inside = op.exact(&interior(&alo, &ahi, t.0), &interior(&blo, &bhi, t.1));
        for v in corners.iter().chain(std::iter::once(&inside)) {
            prop_assert!(rlo <= *v && *v <= rhi, "{:?} {:?} {:?} = {:?} misses the exact value {}", op, a, b, r, v);
        }
        let up2 = step_up(step_up(r.lo()));
        prop_assert!(up2.is_infinite() || exact(up2) > m, "{:?} {:?} {:?} = {:?}: lower bound too loose", op, a, b, r);
        let down2 = step_down(step_down(r.hi()));
        prop_assert!(down2.is_infinite() || exact(down2) < big_m, "{:?} {:?} {:?} = {:?}: upper bound too loose", op, a, b, r);
        Ok(())
    }

    /// `sqrt`: `NegativeSqrt` exactly when the lower bound is negative; otherwise the result is
    /// non-negative, contains the exact roots of both bounds and of an interior point (checked by
    /// squaring), keeps zero exact, and each bound is less than two steps outside the exact root.
    pub fn sqrt(a: Interval, t: u32) -> TestResult {
        let result = a.sqrt();
        if a.lo() < 0.0 {
            prop_assert_eq!(result, Err(NumError::NegativeSqrt), "sqrt {:?}", a);
            return Ok(());
        }
        let r = result.map_err(|e| TestCaseError::fail(format!("sqrt {a:?} failed: {e}")))?;
        invariant(r)?;
        prop_assert!(r.lo() >= 0.0, "sqrt {:?} = {:?} has a negative bound", a, r);
        let (alo, ahi, rlo, rhi) = (exact(a.lo()), exact(a.hi()), exact(r.lo()), exact(r.hi()));
        let (rlo2, rhi2) = (&rlo * &rlo, &rhi * &rhi);
        let inside = interior(&alo, &ahi, t);
        for v in [&alo, &ahi, &inside] {
            prop_assert!(rlo2 <= *v && *v <= rhi2, "sqrt {:?} = {:?} misses the root of {}", a, r, v);
        }
        if a.lo() == 0.0 {
            prop_assert_eq!(r.lo().to_bits(), 0u64, "sqrt {:?}: the root of zero is exact", a);
        } else {
            let up2 = exact(step_up(step_up(r.lo())));
            prop_assert!(&up2 * &up2 > alo, "sqrt {:?} = {:?}: lower bound too loose", a, r);
        }
        if a.hi() == 0.0 {
            prop_assert_eq!(r.hi().to_bits(), 0u64, "sqrt {:?}: the root of zero is exact", a);
        } else {
            let down2 = step_down(step_down(r.hi()));
            prop_assert!(
                down2 < 0.0 || exact(down2) * exact(down2) < ahi,
                "sqrt {:?} = {:?}: upper bound too loose",
                a,
                r
            );
        }
        Ok(())
    }

    /// `neg` is exact (`[−hi, −lo]`, no widening), keeps the invariant and is an involution.
    pub fn neg(a: Interval) -> TestResult {
        let r = a.neg();
        invariant(r)?;
        prop_assert_eq!(exact(r.lo()), -exact(a.hi()));
        prop_assert_eq!(exact(r.hi()), -exact(a.lo()));
        let back = r.neg();
        prop_assert_eq!((back.lo().to_bits(), back.hi().to_bits()), (a.lo().to_bits(), a.hi().to_bits()));
        Ok(())
    }

    /// `certain_sign` is right whenever it answers (every real in the interval has that sign), and
    /// answers whenever the interval alone decides the sign (`None` only when the interval holds zero
    /// and a nonzero real).
    pub fn certain_sign(r: Interval) -> TestResult {
        let (lo, hi) = (exact(r.lo()), exact(r.hi()));
        match r.certain_sign() {
            Some(s) => {
                prop_assert_eq!(sign(&lo), s, "{:?}", r);
                prop_assert_eq!(sign(&hi), s, "{:?}", r);
            }
            None => {
                prop_assert!(lo <= zero() && hi >= zero(), "{:?}: undecided but zero is outside", r);
                prop_assert!(!(lo == zero() && hi == zero()), "{:?}: undecided but exactly zero", r);
            }
        }
        Ok(())
    }

    /// A computed interval holds the exact value, and its certain sign, if any, is the exact sign.
    pub fn sign_is_sound(r: Interval, exact_value: &BigRational) -> TestResult {
        prop_assert!(
            exact(r.lo()) <= *exact_value && *exact_value <= exact(r.hi()),
            "{:?} misses the exact value {}",
            r,
            exact_value
        );
        if let Some(s) = r.certain_sign() {
            prop_assert_eq!(s, sign(exact_value), "{:?}: certain sign is wrong for {}", r, exact_value);
        }
        certain_sign(r)
    }

    /// The bits of a result, so laws compare results exactly.
    pub fn bits(r: Result<Interval, NumError>) -> Result<(u64, u64), NumError> {
        r.map(|i| (i.lo().to_bits(), i.hi().to_bits()))
    }

    /// Laws: `+` and `×` commute bit for bit; `a − b` is `a + (−b)` bit for bit.
    pub fn laws(a: Interval, b: Interval) -> TestResult {
        prop_assert_eq!(bits(a.add(b)), bits(b.add(a)), "{:?} + {:?}", a, b);
        prop_assert_eq!(bits(a.mul(b)), bits(b.mul(a)), "{:?} × {:?}", a, b);
        prop_assert_eq!(bits(a.sub(b)), bits(a.add(b.neg())), "{:?} − {:?}", a, b);
        Ok(())
    }

    /// Whether `x ⊆ y`.
    pub fn subset(x: Interval, y: Interval) -> bool {
        y.lo() <= x.lo() && x.hi() <= y.hi()
    }

    /// Inclusion is monotone: if `a ⊆ a2` and `b ⊆ b2` and the operation succeeds on the larger
    /// operands, it succeeds on the smaller ones with a result inside the larger result.
    pub fn monotone(op: Op, a: Interval, a2: Interval, b: Interval, b2: Interval) -> TestResult {
        prop_assert!(subset(a, a2) && subset(b, b2));
        if let Ok(big) = op.interval(a2, b2) {
            let small = op.interval(a, b);
            prop_assert!(
                matches!(small, Ok(s) if subset(s, big)),
                "{:?}: {:?} {:?} gave {:?}, not inside {:?}",
                op,
                a,
                b,
                small,
                big
            );
        }
        if let Ok(big) = a2.sqrt() {
            let small = a.sqrt();
            prop_assert!(matches!(small, Ok(s) if subset(s, big)), "sqrt {:?} gave {:?}, not inside {:?}", a, small, big);
        }
        Ok(())
    }
}

/// The oracle checked on its own, against hand-computed values and `num-rational`'s own conversion.
mod oracle_self_test {
    use super::oracle::{exact, pow2, significant_bits, step_down, step_up};
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use proptest::prelude::*;

    #[test]
    fn exact_matches_hand_values() {
        assert_eq!(exact(1.0), BigRational::from_integer(BigInt::from(1)));
        assert_eq!(exact(-0.75), BigRational::new(BigInt::from(-3), BigInt::from(4)));
        assert_eq!(exact(0.1), BigRational::new(BigInt::from(3_602_879_701_896_397u64), BigInt::from(1u64 << 55)));
        assert_eq!(exact(f64::from_bits(1)), pow2(-1074));
        assert_eq!(exact(f64::MIN_POSITIVE), pow2(-1022));
        assert_eq!(exact(f64::MAX), (pow2(53) - pow2(0)) * pow2(971));
        assert_eq!(exact(-0.0), exact(0.0));
        assert_eq!(significant_bits(1.0), 1);
        assert_eq!(significant_bits(3.0), 2);
        assert_eq!(significant_bits(f64::MAX), 53);
        assert_eq!(significant_bits(f64::from_bits(1)), 1);
        assert_eq!(significant_bits(0.0), 0);
        assert_eq!(step_up(1.0), 1.0 + f64::EPSILON);
        assert_eq!(step_down(1.0), 1.0 - f64::EPSILON / 2.0);
        assert_eq!(step_up(0.0), f64::from_bits(1));
        assert_eq!(step_up(-f64::from_bits(1)).to_bits(), (-0.0f64).to_bits());
        assert_eq!(step_down(0.0), -f64::from_bits(1));
        assert!(step_up(f64::MAX).is_infinite());
    }

    proptest! {
        #![proptest_config(super::config(super::EFT_CASES))]

        #[test]
        fn exact_matches_num_rational(x in super::strategies::any_finite()) {
            prop_assert_eq!(Some(exact(x)), BigRational::from_float(x));
        }

        #[test]
        fn steps_match_std(x in super::strategies::any_finite()) {
            prop_assert_eq!(step_up(x), x.next_up());
            prop_assert_eq!(step_down(x), x.next_down());
        }
    }
}

/// Error-free transformations.
mod eft_tests {
    use super::checks;
    use super::oracle::{exact, make, pow2, pow2_f64};
    use super::strategies::{self, edge_values};
    use super::{EFT_CASES, config, run};
    use geom_num::eft::{SPLITTER, fast_two_sum, two_prod, two_sum};
    use geom_num::grid::COORD_MAX;
    use num_rational::BigRational;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(config(EFT_CASES))]

        #[test]
        fn two_sum_is_exact((a, b) in strategies::sum_pair()) {
            checks::two_sum(a, b)?;
            checks::two_sum_laws(a, b)?;
        }

        #[test]
        fn fast_two_sum_is_exact_when_ordered((a, b) in strategies::sum_pair()) {
            let (big, small) = if a.abs() >= b.abs() { (a, b) } else { (b, a) };
            checks::fast_two_sum(big, small)?;
            checks::fast_two_sum(0.0, a)?;
        }

        #[test]
        fn split_is_exact(a in strategies::splittable()) {
            checks::split(a)?;
        }

        #[test]
        fn two_prod_is_exact((a, b) in strategies::prod_pair()) {
            checks::two_prod(a, b)?;
        }

        #[test]
        fn two_prod_of_grid_differences_is_exact(
            d in proptest::collection::vec(strategies::coordinate(), 4),
        ) {
            // Coordinate differences reach ±(2^41 − 2); their products need up to 82 bits.
            let x = (d[0] - d[1]) as f64;
            let y = (d[2] - d[3]) as f64;
            checks::two_prod(x, y)?;
        }

        #[test]
        fn two_prod_with_a_zero_factor_is_zero(b in strategies::double()) {
            prop_assume!(checks::in_prod_domain(0.0, b));
            let (p, e) = two_prod(0.0, b);
            prop_assert!(p == 0.0 && e == 0.0, "two_prod(0, {:e}) = ({:e}, {:e})", b, p, e);
            let (p, e) = two_prod(b, -0.0);
            prop_assert!(p == 0.0 && e == 0.0, "two_prod({:e}, -0) = ({:e}, {:e})", b, p, e);
        }
    }

    #[test]
    fn splitter_is_two_to_the_27_plus_one() {
        assert_eq!(exact(SPLITTER), pow2(27) + pow2(0));
    }

    #[test]
    fn edge_pairs_are_exact() {
        let edges = edge_values();
        for &a in &edges {
            for &b in &edges {
                run(checks::two_sum(a, b));
                run(checks::two_sum_laws(a, b));
                let (big, small) = if a.abs() >= b.abs() { (a, b) } else { (b, a) };
                run(checks::fast_two_sum(big, small));
                if checks::in_prod_domain(a, b) {
                    run(checks::two_prod(a, b));
                }
            }
            if checks::in_split_domain(a) {
                run(checks::split(a));
            }
        }
    }

    #[test]
    fn powers_of_two_are_exact() {
        // 2^i against 2^(i − d) across the whole exponent range, both signs: equal (a carry or an exact
        // zero), adjacent, 53 apart (a tie, rounded to even) and 54 apart (below half a unit).
        for i in (-1074..=1023).step_by(5) {
            for d in [0, 1, 53, 54] {
                if i - d < -1074 {
                    continue;
                }
                let (a, b) = (exact_pow2_f64(i), exact_pow2_f64(i - d));
                for (x, y) in [(a, b), (a, -b), (b, -a)] {
                    run(checks::two_sum(x, y));
                    if checks::in_prod_domain(x, y) {
                        run(checks::two_prod(x, y));
                    }
                }
            }
        }
    }

    /// 2^k as a double for every k a double can hold (subnormal or normal).
    fn exact_pow2_f64(k: i32) -> f64 {
        if k >= -1022 { pow2_f64(k) } else { f64::from_bits(1u64 << (k + 1074)) }
    }

    #[test]
    fn ties_round_to_even_and_stay_exact() {
        let cases = [
            (1.0, f64::EPSILON / 2.0),                       // 1 + 2^−53: tie, rounds down to 1
            (1.0 + f64::EPSILON, f64::EPSILON / 2.0),         // tie, rounds up to even
            (f64::MAX, pow2_f64(970) * (1.0 - f64::EPSILON / 2.0)), // just below the overflow tie
            (-(f64::MAX), -pow2_f64(969)),
            (pow2_f64(1023), -pow2_f64(969)),                 // binade boundary below 2^1023
            (f64::MIN_POSITIVE, -f64::from_bits(1)),          // into the subnormals: exact
        ];
        for (a, b) in cases {
            run(checks::two_sum(a, b));
            run(checks::two_sum(b, a));
        }
        let (s, e) = two_sum(1.0, f64::EPSILON / 2.0);
        assert_eq!((s, e), (1.0, f64::EPSILON / 2.0));
    }

    /// Knuth's six operations exactly as published, with no overflow guard.
    fn knuth_two_sum_unguarded(a: f64, b: f64) -> (f64, f64) {
        let s = a + b;
        let b_virtual = s - a;
        let a_virtual = s - b_virtual;
        (s, (a - a_virtual) + (b - b_virtual))
    }

    #[test]
    fn two_sum_survives_the_intermediate_overflow_of_knuths_form() {
        // a = −3 · 2^970, b = Ω: s = Ω − 2^971 is finite, but s − a = Ω + 2^970 rounds to infinity.
        let a = -3.0 * pow2_f64(970);
        let b = f64::MAX;
        let (s, e) = knuth_two_sum_unguarded(a, b);
        assert!(s.is_finite() && !e.is_finite(), "the textbook form was expected to overflow here");
        run(checks::two_sum(a, b));
        assert_eq!(two_sum(a, b), (f64::MAX - pow2_f64(971), -pow2_f64(970)));

        // Every pair of this shape near the top of the range, both signs, both orders: exact, and the
        // guard is what makes some of them exact.
        let tops = [f64::MAX, f64::MAX - pow2_f64(971), f64::MAX - 2.0 * pow2_f64(971), pow2_f64(1023)];
        let mut fallbacks = 0;
        for &top in &tops {
            for k in 1..=128u32 {
                for scale in [pow2_f64(969), pow2_f64(970), pow2_f64(971)] {
                    let small = f64::from(k) * scale;
                    for (x, y) in [(-small, top), (small, -top), (top, -small), (-top, small)] {
                        if !knuth_two_sum_unguarded(x, y).1.is_finite() && (x + y).is_finite() {
                            fallbacks += 1;
                        }
                        run(checks::two_sum(x, y));
                    }
                }
            }
        }
        assert!(fallbacks > 0, "the sweep never reached the overflow guard");
    }

    #[test]
    fn overflowing_sums_never_give_nan() {
        for (a, b) in [(f64::MAX, f64::MAX), (f64::MAX, pow2_f64(970)), (-f64::MAX, -f64::MAX), (pow2_f64(1023), pow2_f64(1023))] {
            run(checks::two_sum(a, b));
            run(checks::two_sum(b, a));
            let (big, small) = if a.abs() >= b.abs() { (a, b) } else { (b, a) };
            run(checks::fast_two_sum(big, small));
        }
    }

    #[test]
    fn cancellation_is_exact() {
        // Nearly equal opposite values: the sum is exact (Sterbenz) and the error zero.
        for k in 0..64u32 {
            let a = 1.0 + f64::from(k) * f64::EPSILON;
            for b in [-1.0, -(1.0 + f64::EPSILON), -(1.0 - f64::EPSILON / 2.0)] {
                run(checks::two_sum(a, b));
                assert_eq!(two_sum(a, b).1, 0.0);
            }
        }
        // A large value, a small one, and the large one back: the small one survives in the errors.
        let (s, e) = two_sum(1.0e16, 1.0);
        let (s2, e2) = two_sum(s, -1.0e16);
        assert_eq!(exact(s2) + exact(e2) + exact(e), exact(1.0));
    }

    #[test]
    fn split_at_its_edges() {
        let below_996 = pow2_f64(995) * (2.0 - f64::EPSILON);
        for a in [0.0, f64::MIN_POSITIVE, 1.0, 2.0 - f64::EPSILON, below_996, ((1u64 << 53) - 1) as f64] {
            run(checks::split(a));
            run(checks::split(-a));
        }
        // Every exponent with an all-ones significand, where the high half rounds up a binade.
        for e in -1022..=995 {
            run(checks::split(make(false, e, (1 << 52) - 1)));
            run(checks::split(make(true, e, (1 << 52) - 1)));
        }
    }

    #[test]
    fn two_prod_at_the_edges_of_its_precondition() {
        let ones = (1u64 << 52) - 1;
        for ea in (-1022..=995).step_by(7) {
            for (eb, label) in [(-970 - ea, "lower"), (1021 - ea, "upper")] {
                if !(-1022..=995).contains(&eb) {
                    continue;
                }
                for (fa, fb) in [(0, 0), (ones, ones), (ones, 1), (1, ones), (0x000f_ffff_ffff_0001, 0x0008_0000_0000_0001)] {
                    let (a, b) = (make(false, ea, fa), make(true, eb, fb));
                    assert!(checks::in_prod_domain(a, b), "{label} edge pair outside the domain");
                    run(checks::two_prod(a, b));
                }
            }
        }
    }

    #[test]
    fn two_prod_lower_limit_is_sharp() {
        // With e_a + e_b = −971 the exact error of a product can need a bit worth 2^−1075, which no
        // double holds: no two-product can be exact there, so −970 is the right limit.
        let ones = (1u64 << 52) - 1;
        let (a, b) = (make(false, -485, ones), make(false, -486, ones));
        assert!(!checks::in_prod_domain(a, b));
        let error = exact(a) * exact(b) - exact(a * b);
        let scaled = error * pow2(1074);
        assert!(!scaled.is_integer(), "expected an error finer than 2^−1074");
        // One exponent higher, the same significands are inside the domain and exact.
        let a2 = make(false, -484, ones);
        run(checks::two_prod(a2, b));
    }

    #[test]
    fn extreme_grid_coordinates() {
        let m = COORD_MAX as f64; // 2^40 − 1, exact in a double
        let d = 2.0 * m; // the largest coordinate difference, 2^41 − 2
        for (a, b) in [(m, m), (m, -m), (d, d), (-d, d), (d, m)] {
            run(checks::two_prod(a, b));
            run(checks::two_sum(a, b));
        }
        // A near-degenerate orientation determinant at the extremes, d · (d − 1) − 1 · d, carried
        // exactly by two products: p₁ + e₁ − (p₂ + e₂). The first product needs about 82 bits.
        let (p1, e1) = two_prod(d, d - 1.0);
        let (p2, e2) = two_prod(1.0, d);
        assert_ne!(e1, 0.0, "a product of 41-bit numbers must carry a nonzero error");
        let di = 2 * i128::from(COORD_MAX);
        let expected = BigRational::from_integer((di * (di - 1) - di).into());
        assert_eq!(exact(p1) + exact(e1) - exact(p2) - exact(e2), expected);
        assert_eq!(fast_two_sum(d, m), two_sum(d, m));
    }
}

/// Intervals.
mod interval_tests {
    use super::checks::{self, Op};
    use super::oracle::{exact, step_down, step_up};
    use super::strategies::{self, edge_values};
    use super::{INTERVAL_CASES, config, run};
    use geom_num::grid::COORD_MAX;
    use geom_num::interval::Interval;
    use geom_num::{NumError, Sign};
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(config(INTERVAL_CASES))]

        #[test]
        fn add_contains_the_exact_sum(a in strategies::interval(), b in strategies::interval(), t in (0u32..=65536, 0u32..=65536)) {
            checks::binary(Op::Add, a, b, t)?;
        }

        #[test]
        fn sub_contains_the_exact_difference(a in strategies::interval(), b in strategies::interval(), t in (0u32..=65536, 0u32..=65536)) {
            checks::binary(Op::Sub, a, b, t)?;
        }

        #[test]
        fn mul_contains_the_exact_product(a in strategies::interval(), b in strategies::interval(), t in (0u32..=65536, 0u32..=65536)) {
            checks::binary(Op::Mul, a, b, t)?;
        }

        #[test]
        fn div_contains_the_exact_quotient(a in strategies::interval(), b in strategies::divisor(), t in (0u32..=65536, 0u32..=65536)) {
            checks::binary(Op::Div, a, b, t)?;
        }

        #[test]
        fn sqrt_contains_the_exact_root(a in strategies::radicand(), t in 0u32..=65536) {
            checks::sqrt(a, t)?;
        }

        #[test]
        fn neg_is_exact(a in strategies::interval()) {
            checks::neg(a)?;
        }

        #[test]
        fn certain_sign_is_right_and_complete(a in strategies::interval(), b in strategies::interval()) {
            checks::certain_sign(a)?;
            for op in Op::ALL {
                if let Ok(r) = op.interval(a, b) {
                    checks::certain_sign(r)?;
                }
            }
        }

        #[test]
        fn laws_hold(a in strategies::interval(), b in strategies::interval()) {
            checks::laws(a, b)?;
        }

        #[test]
        fn inclusion_is_monotone(
            a in strategies::interval(),
            b in strategies::divisor(),
            ca in strategies::double(),
            cb in strategies::double(),
            op in prop::sample::select(Op::ALL.to_vec()),
        ) {
            let widen = |x: Interval, c: f64| Interval::new(x.lo().min(c), x.hi().max(c)).unwrap();
            checks::monotone(op, a, widen(a, ca), b, widen(b, cb))?;
        }

        #[test]
        fn determinant_sign_is_sound(v in proptest::collection::vec(strategies::double(), 4)) {
            // a·d − b·c from point intervals: the interval holds the exact value and never claims a
            // wrong sign.
            let p = |x: f64| Interval::point(x).unwrap();
            let exact_value = exact(v[0]) * exact(v[3]) - exact(v[1]) * exact(v[2]);
            if let Ok(r) = p(v[0]).mul(p(v[3])).and_then(|ad| p(v[1]).mul(p(v[2])).and_then(|bc| ad.sub(bc))) {
                checks::sign_is_sound(r, &exact_value)?;
            }
        }

        #[test]
        fn orient2d_sign_is_sound_on_grid_points(c in proptest::collection::vec(strategies::coordinate(), 6)) {
            let (p, q, r) = ((c[0], c[1]), (c[2], c[3]), (c[4], c[5]));
            let det = orient2d(p, q, r).unwrap();
            checks::sign_is_sound(det, &orient2d_exact(p, q, r))?;
        }

        #[test]
        fn orient2d_never_decides_collinear_points_wrongly(
            px in -(1i64 << 39)..(1i64 << 39),
            py in -(1i64 << 39)..(1i64 << 39),
            dx in -(1i64 << 20)..(1i64 << 20),
            dy in -(1i64 << 20)..(1i64 << 20),
            k in -(1i64 << 18)..(1i64 << 18),
            nudge in -1i64..=1,
        ) {
            // r on the line through p and q, or one step off it: a near-degenerate case the interval
            // may not decide, but must never decide wrongly.
            let (p, q, r) = ((px, py), (px + dx, py + dy), (px + k * dx + nudge, py + k * dy));
            let det = orient2d(p, q, r).unwrap();
            let exact_value = orient2d_exact(p, q, r);
            checks::sign_is_sound(det, &exact_value)?;
            if nudge == 0 {
                prop_assert_eq!(det.certain_sign(), None, "collinear points decided as {:?}", det.certain_sign());
            }
        }
    }

    /// The orientation determinant (q − p) × (r − p) of grid points, in intervals.
    fn orient2d(p: (i64, i64), q: (i64, i64), r: (i64, i64)) -> Result<Interval, NumError> {
        let c = |v: i64| Interval::point(v as f64).unwrap();
        let qx = c(q.0).sub(c(p.0))?;
        let qy = c(q.1).sub(c(p.1))?;
        let rx = c(r.0).sub(c(p.0))?;
        let ry = c(r.1).sub(c(p.1))?;
        qx.mul(ry)?.sub(qy.mul(rx)?)
    }

    /// The same determinant exactly: differences below 2^41, products below 2^82, the result below
    /// 2^83, so `i128` holds it.
    fn orient2d_exact(p: (i64, i64), q: (i64, i64), r: (i64, i64)) -> BigRational {
        let d = |a: i64, b: i64| i128::from(a) - i128::from(b);
        let v = d(q.0, p.0) * d(r.1, p.1) - d(q.1, p.1) * d(r.0, p.0);
        BigRational::from_integer(BigInt::from(v))
    }

    #[test]
    fn edge_intervals_every_operation() {
        // Every interval between two of these bounds, against every other, through every operation.
        let bounds = [
            -f64::MAX,
            -1.5,
            -f64::from_bits(1),
            0.0,
            f64::from_bits(1),
            1.0,
            COORD_MAX as f64,
            f64::MAX,
        ];
        let mut intervals = Vec::new();
        for (i, &lo) in bounds.iter().enumerate() {
            for &hi in &bounds[i..] {
                intervals.push(Interval::new(lo, hi).unwrap());
            }
        }
        for &a in &intervals {
            run(checks::neg(a));
            run(checks::certain_sign(a));
            run(checks::sqrt(a, 12_345));
            for &b in &intervals {
                for op in Op::ALL {
                    run(checks::binary(op, a, b, (0, 65_536)));
                }
                run(checks::laws(a, b));
            }
        }
    }

    #[test]
    fn edge_points_every_operation() {
        // Points at every edge value of the format, pairwise, through every operation.
        let points: Vec<Interval> = edge_values().into_iter().map(|x| Interval::point(x).unwrap()).collect();
        for &a in &points {
            for &b in &points {
                for op in Op::ALL {
                    run(checks::binary(op, a, b, (32_768, 32_768)));
                }
            }
        }
    }

    #[test]
    fn construction_errors() {
        assert_eq!(Interval::new(1.0, 0.0), Err(NumError::EmptyInterval));
        assert_eq!(Interval::new(f64::from_bits(1), 0.0), Err(NumError::EmptyInterval));
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(Interval::point(bad), Err(NumError::NotFinite));
            assert_eq!(Interval::new(bad, 1.0), Err(NumError::NotFinite));
            assert_eq!(Interval::new(-1.0, bad), Err(NumError::NotFinite));
        }
        // Finiteness is checked before order.
        assert_eq!(Interval::new(f64::NAN, -1.0), Err(NumError::NotFinite));
        assert_eq!(Interval::new(f64::INFINITY, -1.0), Err(NumError::NotFinite));
        // Extreme but finite bounds are fine.
        let all = Interval::new(-f64::MAX, f64::MAX).unwrap();
        assert_eq!((all.lo(), all.hi()), (-f64::MAX, f64::MAX));
    }

    #[test]
    fn zero_bounds_are_stored_as_positive_zero() {
        let z = Interval::point(-0.0).unwrap();
        assert_eq!((z.lo().to_bits(), z.hi().to_bits()), (0, 0));
        let z = Interval::new(0.0, -0.0).unwrap();
        assert_eq!((z.lo().to_bits(), z.hi().to_bits()), (0, 0));
        let n = Interval::point(0.0).unwrap().neg();
        assert_eq!((n.lo().to_bits(), n.hi().to_bits()), (0, 0));
        // next_up(−2^−1074) is −0; the widened bound must be stored as +0.
        let tiny = Interval::point(-f64::from_bits(1)).unwrap();
        let r = tiny.add(Interval::point(0.0).unwrap()).unwrap();
        assert_eq!(r.hi().to_bits(), 0);
        // next_down(2^−1074) is +0 already.
        let r = tiny.neg().add(Interval::point(0.0).unwrap()).unwrap();
        assert_eq!(r.lo().to_bits(), 0);
        run(checks::invariant(r));
    }

    #[test]
    fn division_by_an_interval_containing_zero() {
        let one = Interval::point(1.0).unwrap();
        let huge = Interval::point(f64::MAX).unwrap();
        for (lo, hi) in [(0.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (-1.0, 1.0), (-0.0, 2.0), (-f64::MAX, f64::MAX)] {
            let d = Interval::new(lo, hi).unwrap();
            assert_eq!(one.div(d), Err(NumError::DivisorContainsZero));
            // The divisor is checked before anything can overflow.
            assert_eq!(huge.div(d), Err(NumError::DivisorContainsZero));
        }
        let d = Interval::new(f64::from_bits(1), 1.0).unwrap();
        assert!(one.div(d).is_err_and(|e| e == NumError::NotFinite)); // 1 / 2^−1074 = 2^1074
        let d = Interval::new(0.5, 4.0).unwrap();
        let q = one.div(d).unwrap();
        assert!(q.contains(0.25) && q.contains(2.0) && q.certain_sign() == Some(Sign::Positive));
    }

    #[test]
    fn square_root_cases() {
        for (lo, hi) in [(-1.0, 4.0), (-f64::from_bits(1), 0.0), (-2.0, -1.0)] {
            assert_eq!(Interval::new(lo, hi).unwrap().sqrt(), Err(NumError::NegativeSqrt));
        }
        let z = Interval::point(0.0).unwrap().sqrt().unwrap();
        assert_eq!((z.lo().to_bits(), z.hi().to_bits()), (0, 0));
        assert_eq!(z.certain_sign(), Some(Sign::Zero));
        let four = Interval::point(4.0).unwrap().sqrt().unwrap();
        assert_eq!((four.lo(), four.hi()), (step_down(2.0), step_up(2.0)));
        let max = Interval::new(0.0, f64::MAX).unwrap().sqrt().unwrap();
        assert!(max.lo() == 0.0 && max.hi().is_finite());
        let tiny = Interval::point(f64::from_bits(1)).unwrap().sqrt().unwrap();
        assert!(tiny.lo() > 0.0 && tiny.contains(2.0f64.powi(-537)));
        // The root of a root stays defined: the lower bound never goes below zero.
        let r = Interval::new(0.0, 2.0).unwrap().sqrt().unwrap().sqrt().unwrap();
        let h = exact(r.hi());
        assert!(r.lo() == 0.0 && &h * &h * &h * &h >= BigRational::from_integer(BigInt::from(2)));
    }

    #[test]
    fn overflow_gives_not_finite() {
        let max = Interval::point(f64::MAX).unwrap();
        let zero = Interval::point(0.0).unwrap();
        let big = Interval::point(2.0f64.powi(600)).unwrap();
        assert_eq!(max.add(max), Err(NumError::NotFinite));
        assert_eq!(max.neg().sub(max), Err(NumError::NotFinite));
        assert_eq!(big.mul(big), Err(NumError::NotFinite));
        assert_eq!(big.div(Interval::point(2.0f64.powi(-600)).unwrap()), Err(NumError::NotFinite));
        // The exact sum Ω is finite, but no finite double lies above it to widen to.
        assert_eq!(max.add(zero), Err(NumError::NotFinite));
        // One step below Ω the upper bound can still widen, to Ω itself.
        let below = Interval::point(step_down(f64::MAX)).unwrap();
        assert_eq!(below.add(zero).map(Interval::hi), Ok(f64::MAX));
        // Negation never fails, even at the extremes.
        let all = Interval::new(-f64::MAX, f64::MAX).unwrap();
        assert_eq!(all.neg(), all);
    }

    #[test]
    fn contains_and_certain_sign_basics() {
        let r = Interval::new(-1.0, 2.0).unwrap();
        assert!(r.contains(-1.0) && r.contains(2.0) && r.contains(0.0));
        assert!(!r.contains(step_down(-1.0)) && !r.contains(step_up(2.0)) && !r.contains(f64::NAN));
        assert_eq!(r.certain_sign(), None);
        assert_eq!(Interval::new(f64::from_bits(1), 1.0).unwrap().certain_sign(), Some(Sign::Positive));
        assert_eq!(Interval::new(-1.0, -f64::from_bits(1)).unwrap().certain_sign(), Some(Sign::Negative));
        assert_eq!(Interval::new(0.0, 1.0).unwrap().certain_sign(), None);
        assert_eq!(Interval::new(-1.0, 0.0).unwrap().certain_sign(), None);
        assert_eq!(Interval::point(0.0).unwrap().certain_sign(), Some(Sign::Zero));
    }

    #[test]
    fn orient2d_at_extreme_coordinates() {
        let m = COORD_MAX;
        // Counter-clockwise, clockwise and collinear triangles with every coordinate at ±(2^40 − 1).
        let ccw = ((-m, -m), (m, -m), (-m, m));
        let cw = ((-m, -m), (-m, m), (m, -m));
        let line = ((-m, -m), (m, m), (-m, -m));
        let diagonal = ((-m, -m), (m, m), (0, 0));
        for ((p, q, r), expected) in
            [(ccw, Some(Sign::Positive)), (cw, Some(Sign::Negative)), (line, None), (diagonal, None)]
        {
            let det = orient2d(p, q, r).unwrap();
            run(checks::sign_is_sound(det, &orient2d_exact(p, q, r)));
            assert_eq!(det.certain_sign(), expected, "{p:?} {q:?} {r:?}");
        }
        // Every corner triangle of the coordinate box, every orientation.
        let corners = [(-m, -m), (m, -m), (m, m), (-m, m)];
        for &p in &corners {
            for &q in &corners {
                for &r in &corners {
                    let det = orient2d(p, q, r).unwrap();
                    let exact_value = orient2d_exact(p, q, r);
                    run(checks::sign_is_sound(det, &exact_value));
                    if exact_value != BigRational::from_integer(BigInt::from(0)) {
                        assert!(det.certain_sign().is_some(), "{p:?} {q:?} {r:?} undecided");
                    }
                }
            }
        }
    }
}
