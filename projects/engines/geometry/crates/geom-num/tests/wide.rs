//! Property tests of `geom_num::wide` against the exact oracle `num-bigint`.
//!
//! What this file proves, at every width the kernel uses (`Wide<2>`, `I256`, `I320`, `I384`,
//! `I512`): every public operation (`from_i128`, `from_i64`, `from_le_limbs`, `to_le_limbs`,
//! `checked_add`, `checked_sub`, `checked_neg`, `checked_mul`, `resize` to every width, `to_i128`,
//! `signum`, `is_zero`, `is_negative`, `Ord`, `max_value`, `min_value`, `ZERO`, `one`, `Debug`)
//! agrees with exact big-integer arithmetic, and every checked operation returns `None` exactly when
//! the true result leaves −2^(64L−1) ..= 2^(64L−1) − 1, never otherwise.
//!
//! How the file is built:
//! - `oracle` reads limbs as a `BigInt` and writes a `BigInt` as limbs, independently of the code
//!   under test. It is checked on its own first, against `i128` and hand-written limbs.
//! - `strategies` makes values for one width, weighted towards where bugs live: random limbs, limb
//!   patterns (0, 1, all ones, the sign bit), values near zero, near the limits, of every bit length
//!   and next to powers of two; and pairs whose sum, difference or product straddles a limit.
//! - `checks` holds one generic function per operation. Each compares with the oracle and checks the
//!   laws the operation must keep. Property tests and exhaustive edge tests share them.
//! - `width_suite!` instantiates every property test and every exhaustive edge test for one width.
//! - `kernel_use` feeds grid coordinates at ±(2^40 − 1), the kernel's extreme inputs, through
//!   products and determinant expansions, every step against the oracle.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings.
// A broken oracle or strategy must stop the test loudly, so the kernel's panic lints are relaxed
// for this test file only (clippy.toml: "Tests may use them").
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use proptest::test_runner::{Config, TestCaseError};

/// The outcome of one check: `Err` carries the first disagreement with the oracle.
type TestResult = Result<(), TestCaseError>;

/// Cases per property. Each case is compared with the exact oracle; the exhaustive edge tests
/// cover the fixed corners, so the random cases search the space between them.
const CASES: u32 = 1024;

/// The configuration every property in this file runs with.
fn config() -> Config {
    Config {
        cases: CASES,
        ..Config::default()
    }
}

/// Turns a check's failure into a test failure, for checks run outside `proptest!`.
fn run(result: TestResult) {
    if let Err(e) = result {
        panic!("{e}");
    }
}

/// The exact model: limbs read as integers, integers written as limbs, the range of each width.
/// None of it calls the arithmetic under test.
mod oracle {
    use geom_num::{Sign, Wide};
    use num_bigint::{BigInt, Sign as BigSign};
    use num_traits::{One, Signed, Zero};

    /// Every width the kernel names, in limbs: `Wide<2>`, `I256`, `I320`, `I384`, `I512`.
    pub const WIDTHS: [usize; 5] = [2, 4, 5, 6, 8];

    /// 2^n.
    pub fn pow2(n: usize) -> BigInt {
        BigInt::one() << n
    }

    /// The sign bit's position in a `Wide<l>`: its range is −2^top ..= 2^top − 1.
    pub fn top(l: usize) -> usize {
        64 * l - 1
    }

    /// The largest value of a `Wide<l>`, 2^(64l−1) − 1.
    pub fn max_of(l: usize) -> BigInt {
        pow2(top(l)) - 1u32
    }

    /// The smallest value of a `Wide<l>`, −2^(64l−1).
    pub fn min_of(l: usize) -> BigInt {
        -pow2(top(l))
    }

    /// Whether `v` is representable in a `Wide<l>`.
    pub fn in_range(v: &BigInt, l: usize) -> bool {
        (min_of(l)..=max_of(l)).contains(v)
    }

    /// `v` moved to the nearest representable value of a `Wide<l>`.
    pub fn clamp(v: BigInt, l: usize) -> BigInt {
        v.clamp(min_of(l), max_of(l))
    }

    /// Limbs, least significant first, read as an unsigned number.
    pub fn unsigned(limbs: &[u64]) -> BigInt {
        limbs
            .iter()
            .rev()
            .fold(BigInt::zero(), |acc, &limb| (acc << 64u32) + limb)
    }

    /// Limbs, least significant first, read as a two's-complement number: the unsigned value, less
    /// 2^(64·len) when the top bit is set.
    pub fn signed(limbs: &[u64]) -> BigInt {
        let u = unsigned(limbs);
        if limbs.last().is_some_and(|&top_limb| top_limb >> 63 == 1) {
            u - pow2(64 * limbs.len())
        } else {
            u
        }
    }

    /// The value a `Wide` holds, read from its raw limbs.
    pub fn value_of<const L: usize>(x: &Wide<L>) -> BigInt {
        signed(&x.to_le_limbs())
    }

    /// The two's-complement limbs of `v` in `L` limbs, or `None` when `v` is out of range.
    pub fn limbs<const L: usize>(v: &BigInt) -> Option<[u64; L]> {
        if !in_range(v, L) {
            return None;
        }
        // In range, so 0 <= u < 2^(64L): at most L base-2^64 digits.
        let u = if v.is_negative() {
            v + pow2(64 * L)
        } else {
            v.clone()
        };
        let (_, digits) = u.to_u64_digits();
        assert!(digits.len() <= L, "{v} needs {} limbs", digits.len());
        let mut out = [0u64; L];
        for (o, d) in out.iter_mut().zip(digits) {
            *o = d;
        }
        Some(out)
    }

    /// The `Wide<L>` holding `v`, or `None` when `v` is out of range: the expected result of every
    /// checked operation whose true result is `v`.
    pub fn wide<const L: usize>(v: &BigInt) -> Option<Wide<L>> {
        limbs::<L>(v).map(Wide::from_le_limbs)
    }

    /// The sign of `v`.
    pub fn sign(v: &BigInt) -> Sign {
        match v.sign() {
            BigSign::Minus => Sign::Negative,
            BigSign::NoSign => Sign::Zero,
            BigSign::Plus => Sign::Positive,
        }
    }
}

/// Value generators for one width. Every value they produce is in range for that width.
mod strategies {
    use std::collections::BTreeSet;

    use num_bigint::BigInt;
    use num_traits::{Signed, Zero};
    use proptest::collection::vec;
    use proptest::prelude::*;

    use crate::oracle::{WIDTHS, clamp, in_range, max_of, min_of, pow2, signed, top, unsigned};

    /// The grid's extreme coordinate, 2^40 − 1 grid steps.
    pub const C: i64 = (1 << 40) - 1;

    /// Uniformly random limbs: values spread over the whole range, half of them negative.
    fn random_limbs(l: usize) -> BoxedStrategy<BigInt> {
        vec(any::<u64>(), l).prop_map(|v| signed(&v)).boxed()
    }

    /// Limbs drawn from the patterns carry and borrow chains trip on: 0, 1, all ones, all ones but
    /// the lowest bit, the sign bit alone, all but the sign bit, or random.
    fn patterned_limbs(l: usize) -> BoxedStrategy<BigInt> {
        let limb = prop_oneof![
            Just(0u64),
            Just(1u64),
            Just(u64::MAX),
            Just(u64::MAX - 1),
            Just(1u64 << 63),
            Just(u64::MAX >> 1),
            any::<u64>(),
        ];
        vec(limb, l).prop_map(|v| signed(&v)).boxed()
    }

    /// Values within 300 of zero.
    fn near_zero() -> BoxedStrategy<BigInt> {
        (-300i64..=300).prop_map(BigInt::from).boxed()
    }

    /// Values within 300 of the largest or the smallest value.
    fn near_limits(l: usize) -> BoxedStrategy<BigInt> {
        (0u32..300, any::<bool>())
            .prop_map(move |(k, high)| if high { max_of(l) - k } else { min_of(l) + k })
            .boxed()
    }

    /// Values of every bit length 1 ..= 64l − 1, uniformly by length, with either sign.
    fn every_length(l: usize) -> BoxedStrategy<BigInt> {
        (1..=top(l), vec(any::<u64>(), l), any::<bool>())
            .prop_map(|(n, r, negative)| {
                // 2^(n−1) <= m < 2^n <= 2^top: representable with either sign.
                let low = pow2(n - 1);
                let m = &low + unsigned(&r) % &low;
                if negative { -m } else { m }
            })
            .boxed()
    }

    /// Values of at most `bits` bits, with either sign.
    fn at_most_bits(l: usize, bits: usize) -> BoxedStrategy<BigInt> {
        (0..=bits, vec(any::<u64>(), l), any::<bool>())
            .prop_map(|(n, r, negative)| {
                let m = unsigned(&r) % pow2(n);
                if negative { -m } else { m }
            })
            .boxed()
    }

    /// ±2^n + d for d in −2 ..= 2, clamped into range: every carry boundary.
    fn near_powers(l: usize) -> BoxedStrategy<BigInt> {
        (0..=top(l), -2i32..=2, any::<bool>())
            .prop_map(move |(n, d, negative)| {
                let m = pow2(n) + d;
                clamp(if negative { -m } else { m }, l)
            })
            .boxed()
    }

    /// Any value of a `Wide<l>`, weighted towards the places bugs live.
    pub fn value(l: usize) -> BoxedStrategy<BigInt> {
        prop_oneof![
            3 => random_limbs(l),
            2 => patterned_limbs(l),
            1 => near_zero(),
            2 => near_limits(l),
            4 => every_length(l),
            2 => near_powers(l),
        ]
        .boxed()
    }

    /// Operands for the ring laws: any value, or one small enough that products of three fit.
    pub fn law_operand(l: usize) -> BoxedStrategy<BigInt> {
        prop_oneof![value(l), at_most_bits(l, top(l) / 3)].boxed()
    }

    /// Pairs whose sum is within 3 of the largest or smallest value (unless clamping moved b).
    pub fn sum_at_limit(l: usize) -> BoxedStrategy<(BigInt, BigInt)> {
        (value(l), -3i32..=3, any::<bool>())
            .prop_map(move |(a, d, high)| {
                let limit = if high { max_of(l) } else { min_of(l) };
                let b = clamp(limit - &a + d, l);
                (a, b)
            })
            .boxed()
    }

    /// Pairs whose difference is within 3 of the largest or smallest value (unless clamping moved b).
    pub fn difference_at_limit(l: usize) -> BoxedStrategy<(BigInt, BigInt)> {
        (value(l), -3i32..=3, any::<bool>())
            .prop_map(move |(a, d, high)| {
                let limit = if high { max_of(l) } else { min_of(l) };
                let b = clamp(&a - limit - d, l);
                (a, b)
            })
            .boxed()
    }

    /// Pairs whose product magnitude straddles 2^(64l−1): with |a| of any size and
    /// T = 2^(64l−1) + t, |b| = ⌊T / |a|⌋ + d, so |a·b| lands within a few |a| of the threshold on
    /// either side. Signs are independent, so the asymmetric limit is met both ways.
    pub fn product_at_limit(l: usize) -> BoxedStrategy<(BigInt, BigInt)> {
        let nonzero = value(l).prop_filter("a non-zero factor", |v| !v.is_zero());
        (nonzero, -1i32..=1, -2i32..=2, any::<bool>(), any::<bool>())
            .prop_map(move |(a, t, d, negative_a, negative_b)| {
                let threshold = pow2(top(l)) + t;
                let ma = a.abs();
                let mb = (threshold / &ma + d).max(BigInt::zero());
                let a = clamp(if negative_a { -ma } else { ma }, l);
                let b = clamp(if negative_b { -mb } else { mb }, l);
                (a, b)
            })
            .boxed()
    }

    /// Pairs at most 2 apart: equal and adjacent values, where comparison must look at every limb.
    pub fn close_pair(l: usize) -> BoxedStrategy<(BigInt, BigInt)> {
        (value(l), -2i32..=2)
            .prop_map(move |(a, d)| {
                let b = clamp(&a + d, l);
                (a, b)
            })
            .boxed()
    }

    /// Pairs whose limbs differ in exactly one position (or none): comparison must find that limb.
    pub fn one_limb_apart(l: usize) -> BoxedStrategy<(BigInt, BigInt)> {
        (vec(any::<u64>(), l), 0..l, any::<u64>())
            .prop_map(|(v, i, x)| {
                let mut u = v.clone();
                u[i] = x;
                (signed(&v), signed(&u))
            })
            .boxed()
    }

    /// Any value, any `i128`, or a value within 3 of an `i128` limit.
    pub fn near_i128_limits(l: usize) -> BoxedStrategy<BigInt> {
        let edge = (-3i32..=3, any::<bool>()).prop_map(move |(d, high)| {
            let limit = if high {
                BigInt::from(i128::MAX)
            } else {
                BigInt::from(i128::MIN)
            };
            clamp(limit + d, l)
        });
        let any_i128 = any::<i128>().prop_map(move |v| clamp(BigInt::from(v), l));
        prop_oneof![value(l), any_i128, edge].boxed()
    }

    /// Any value, or one within 3 of the limits of some width (clamped into this one): resizing to
    /// that width must succeed just inside its limits and fail just outside.
    pub fn near_every_width_limit(l: usize) -> BoxedStrategy<BigInt> {
        let edge = (
            prop::sample::select(WIDTHS.to_vec()),
            -3i32..=3,
            any::<bool>(),
        )
            .prop_map(move |(m, d, negative)| {
                let p = pow2(top(m));
                clamp(if negative { -p + d } else { p + d }, l)
            });
        prop_oneof![value(l), edge].boxed()
    }

    /// A grid coordinate, weighted towards the extremes ±(2^40 − 1).
    pub fn coordinate() -> BoxedStrategy<i64> {
        prop_oneof![
            2 => Just(C),
            2 => Just(-C),
            1 => Just(0i64),
            1 => Just(1i64),
            1 => Just(-1i64),
            3 => -C..=C,
        ]
        .boxed()
    }

    /// The fixed corners of a `Wide<l>`, for exhaustive pairwise tests: 0, ±1, ±2, ±3, and
    /// ±(2^n − 1), ±2^n, ±(2^n + 1) for n at limb boundaries, at `i64` and `i128` limits, at the
    /// square root of the range (products of two land on the limit), and at the limits themselves.
    pub fn edge_values(l: usize) -> Vec<BigInt> {
        let p = top(l);
        let mut exponents = vec![
            0,
            1,
            31,
            32,
            62,
            63,
            64,
            126,
            127,
            128,
            p - 1,
            p,
            p / 2,
            p / 2 + 1,
        ];
        for k in 1..l {
            exponents.extend([64 * k - 1, 64 * k, 64 * k + 1]);
        }
        let mut set = BTreeSet::new();
        let mut both_signs = |m: BigInt| {
            for v in [m.clone(), -m] {
                if in_range(&v, l) {
                    set.insert(v);
                }
            }
        };
        for n in exponents {
            for d in -1i32..=1 {
                both_signs(pow2(n) + d);
            }
        }
        let root = pow2(p).sqrt();
        for d in -1i32..=2 {
            both_signs(&root + d);
        }
        set.into_iter().collect()
    }
}

/// One generic check per operation: the result must equal the oracle's, and the laws must hold.
mod checks {
    use std::cmp::Ordering;

    use geom_num::{Sign, Wide};
    use num_bigint::BigInt;
    use num_integer::Integer;
    use num_traits::{One, Signed, ToPrimitive, Zero};
    use proptest::{prop_assert, prop_assert_eq};

    use crate::TestResult;
    use crate::oracle::{self, in_range, max_of, min_of, pow2, top, value_of, wide};

    /// The `Wide<L>` holding `v`, which the caller guarantees is in range.
    pub fn w<const L: usize>(v: &BigInt) -> Wide<L> {
        wide::<L>(v).unwrap_or_else(|| panic!("{v} does not fit Wide<{L}>"))
    }

    /// The sign an ordering against zero implies.
    fn ordering_sign(o: Ordering) -> Sign {
        match o {
            Ordering::Less => Sign::Negative,
            Ordering::Equal => Sign::Zero,
            Ordering::Greater => Sign::Positive,
        }
    }

    /// `from_i128`: exact value, exact limbs, and `to_i128` gives it back.
    pub fn from_i128<const L: usize>(v: i128) -> TestResult {
        let x = Wide::<L>::from_i128(v);
        let exact = BigInt::from(v);
        prop_assert_eq!(value_of(&x), exact.clone(), "Wide<{}>::from_i128({})", L, v);
        prop_assert_eq!(Some(x.to_le_limbs()), oracle::limbs::<L>(&exact));
        prop_assert_eq!(x.to_i128(), Some(v));
        prop_assert_eq!(x.signum(), oracle::sign(&exact));
        Ok(())
    }

    /// `from_i64`: exact value, and the same as going through `i128`.
    pub fn from_i64<const L: usize>(v: i64) -> TestResult {
        let x = Wide::<L>::from_i64(v);
        prop_assert_eq!(
            value_of(&x),
            BigInt::from(v),
            "Wide<{}>::from_i64({})",
            L,
            v
        );
        prop_assert_eq!(x, Wide::<L>::from_i128(i128::from(v)));
        prop_assert_eq!(x.to_i128(), Some(i128::from(v)));
        Ok(())
    }

    /// `from_le_limbs` and `to_le_limbs` are inverse; the limbs mean what the oracle says; equality
    /// is equality of value; `Debug` prints the two's-complement bits, most significant first.
    pub fn limbs<const L: usize>(raw: [u64; L]) -> TestResult {
        let x = Wide::<L>::from_le_limbs(raw);
        prop_assert_eq!(x.to_le_limbs(), raw);
        let v = oracle::signed(&raw);
        prop_assert_eq!(wide::<L>(&v), Some(x), "oracle limbs of {} disagree", v);
        let expected = format!(
            "Wide<{L}>(0x{:0width$x})",
            oracle::unsigned(&raw),
            width = 16 * L
        );
        prop_assert_eq!(format!("{x:?}"), expected);
        Ok(())
    }

    /// `checked_add` against the oracle, plus: it commutes, and subtracting b undoes it.
    pub fn add<const L: usize>(a: &BigInt, b: &BigInt) -> TestResult {
        let (x, y) = (w::<L>(a), w::<L>(b));
        let got = x.checked_add(&y);
        prop_assert_eq!(got, wide::<L>(&(a + b)), "Wide<{}>: {} + {}", L, a, b);
        prop_assert_eq!(y.checked_add(&x), got, "addition commutes");
        if let Some(s) = got {
            prop_assert_eq!(s.checked_sub(&y), Some(x), "(a + b) − b = a");
        }
        Ok(())
    }

    /// `checked_sub` against the oracle, plus: adding b undoes it, and a − b = a + (−b).
    pub fn sub<const L: usize>(a: &BigInt, b: &BigInt) -> TestResult {
        let (x, y) = (w::<L>(a), w::<L>(b));
        let got = x.checked_sub(&y);
        prop_assert_eq!(got, wide::<L>(&(a - b)), "Wide<{}>: {} − {}", L, a, b);
        if let Some(d) = got {
            prop_assert_eq!(d.checked_add(&y), Some(x), "(a − b) + b = a");
        }
        if let Some(negated) = y.checked_neg() {
            prop_assert_eq!(x.checked_add(&negated), got, "a − b = a + (−b)");
        }
        Ok(())
    }

    /// `checked_neg` against the oracle, plus: −a = 0 − a, −(−a) = a, a + (−a) = 0.
    pub fn neg<const L: usize>(a: &BigInt) -> TestResult {
        let x = w::<L>(a);
        let got = x.checked_neg();
        prop_assert_eq!(got, wide::<L>(&-a), "Wide<{}>: −({})", L, a);
        prop_assert_eq!(Wide::<L>::ZERO.checked_sub(&x), got, "−a = 0 − a");
        if let Some(n) = got {
            prop_assert_eq!(n.checked_neg(), Some(x), "−(−a) = a");
            prop_assert_eq!(x.checked_add(&n), Some(Wide::<L>::ZERO), "a + (−a) = 0");
        }
        Ok(())
    }

    /// `checked_mul` against the oracle, plus: it commutes and keeps the sign rule.
    pub fn mul<const L: usize>(a: &BigInt, b: &BigInt) -> TestResult {
        let (x, y) = (w::<L>(a), w::<L>(b));
        let got = x.checked_mul(&y);
        prop_assert_eq!(got, wide::<L>(&(a * b)), "Wide<{}>: {} × {}", L, a, b);
        prop_assert_eq!(y.checked_mul(&x), got, "multiplication commutes");
        if let Some(p) = got {
            prop_assert_eq!(p.signum(), x.signum().mul(y.signum()), "sign of a product");
        }
        Ok(())
    }

    /// The units of multiplication: a × 1 = a, a × 0 = 0, a × (−1) = −a.
    pub fn mul_units<const L: usize>(a: &BigInt) -> TestResult {
        let x = w::<L>(a);
        let one = Wide::<L>::one();
        let zero = Wide::<L>::ZERO;
        prop_assert_eq!(x.checked_mul(&one), Some(x), "a × 1 = a");
        prop_assert_eq!(one.checked_mul(&x), Some(x), "1 × a = a");
        prop_assert_eq!(x.checked_mul(&zero), Some(zero), "a × 0 = 0");
        prop_assert_eq!(
            x.checked_mul(&Wide::<L>::from_i64(-1)),
            x.checked_neg(),
            "a × (−1) = −a"
        );
        Ok(())
    }

    /// Associativity of + and ×, and distributivity: wherever both sides are defined they agree,
    /// and each side is defined exactly when every intermediate it computes is in range.
    pub fn ring_laws<const L: usize>(a: &BigInt, b: &BigInt, c: &BigInt) -> TestResult {
        let (x, y, z) = (w::<L>(a), w::<L>(b), w::<L>(c));
        let fits = |v: &BigInt| in_range(v, L);

        let total = a + b + c;
        let lhs = x.checked_add(&y).and_then(|s| s.checked_add(&z));
        let rhs = y.checked_add(&z).and_then(|s| x.checked_add(&s));
        prop_assert_eq!(
            lhs.is_some(),
            fits(&(a + b)) && fits(&total),
            "(a + b) + c defined"
        );
        prop_assert_eq!(
            rhs.is_some(),
            fits(&(b + c)) && fits(&total),
            "a + (b + c) defined"
        );
        if let (Some(l), Some(r)) = (lhs, rhs) {
            prop_assert_eq!(l, r, "+ is associative");
            prop_assert_eq!(value_of(&l), total);
        }

        let total = a * b * c;
        let lhs = x.checked_mul(&y).and_then(|p| p.checked_mul(&z));
        let rhs = y.checked_mul(&z).and_then(|p| x.checked_mul(&p));
        prop_assert_eq!(
            lhs.is_some(),
            fits(&(a * b)) && fits(&total),
            "(a × b) × c defined"
        );
        prop_assert_eq!(
            rhs.is_some(),
            fits(&(b * c)) && fits(&total),
            "a × (b × c) defined"
        );
        if let (Some(l), Some(r)) = (lhs, rhs) {
            prop_assert_eq!(l, r, "× is associative");
            prop_assert_eq!(value_of(&l), total);
        }

        let total = a * (b + c);
        let lhs = y.checked_add(&z).and_then(|s| x.checked_mul(&s));
        let rhs = x
            .checked_mul(&y)
            .zip(x.checked_mul(&z))
            .and_then(|(p, q)| p.checked_add(&q));
        prop_assert_eq!(
            lhs.is_some(),
            fits(&(b + c)) && fits(&total),
            "a × (b + c) defined"
        );
        prop_assert_eq!(
            rhs.is_some(),
            fits(&(a * b)) && fits(&(a * c)) && fits(&total),
            "a × b + a × c defined"
        );
        if let (Some(l), Some(r)) = (lhs, rhs) {
            prop_assert_eq!(l, r, "× distributes over +");
            prop_assert_eq!(value_of(&l), total);
        }
        Ok(())
    }

    /// `signum`, `is_zero` and `is_negative` against the oracle, and against comparison with zero.
    pub fn signs<const L: usize>(a: &BigInt) -> TestResult {
        let x = w::<L>(a);
        prop_assert_eq!(x.signum(), oracle::sign(a), "Wide<{}>: sign of {}", L, a);
        prop_assert_eq!(x.is_zero(), a.is_zero());
        prop_assert_eq!(x.is_negative(), a.is_negative());
        prop_assert_eq!(x.signum(), ordering_sign(x.cmp(&Wide::<L>::ZERO)));
        Ok(())
    }

    /// `Ord`, `PartialOrd` and `Eq` against the oracle, plus antisymmetry, reflexivity, `max` and
    /// `min`, and agreement with the sign of the difference wherever it is representable.
    pub fn order<const L: usize>(a: &BigInt, b: &BigInt) -> TestResult {
        let (x, y) = (w::<L>(a), w::<L>(b));
        let expected = a.cmp(b);
        prop_assert_eq!(x.cmp(&y), expected, "Wide<{}>: {} vs {}", L, a, b);
        prop_assert_eq!(x.partial_cmp(&y), Some(expected));
        prop_assert_eq!(y.cmp(&x), expected.reverse(), "antisymmetry");
        prop_assert_eq!(x == y, a == b);
        prop_assert_eq!(x.cmp(&x), Ordering::Equal, "reflexivity");
        prop_assert_eq!(x.max(y), w::<L>(a.max(b)));
        prop_assert_eq!(x.min(y), w::<L>(a.min(b)));
        if let Some(d) = x.checked_sub(&y) {
            prop_assert_eq!(d.signum(), ordering_sign(expected), "sign of a − b");
        }
        Ok(())
    }

    /// Sorting by `Ord` sorts by value: transitivity over three values.
    pub fn order_transitive<const L: usize>(a: &BigInt, b: &BigInt, c: &BigInt) -> TestResult {
        let mut sorted = [w::<L>(a), w::<L>(b), w::<L>(c)];
        sorted.sort();
        let mut expected = [a.clone(), b.clone(), c.clone()];
        expected.sort();
        prop_assert_eq!(sorted.map(|x| value_of(&x)), expected);
        Ok(())
    }

    /// `to_i128` against the oracle: `Some` exactly when the value fits an `i128`.
    pub fn to_i128<const L: usize>(a: &BigInt) -> TestResult {
        let x = w::<L>(a);
        let got = x.to_i128();
        prop_assert_eq!(got, a.to_i128(), "Wide<{}>: {} as i128", L, a);
        if let Some(v) = got {
            prop_assert_eq!(Wide::<L>::from_i128(v), x, "from_i128 undoes to_i128");
        }
        Ok(())
    }

    /// `resize::<M>` from `Wide<L>` against the oracle: `Some` exactly when the value fits `Wide<M>`,
    /// and then resizing back gives the original.
    pub fn resize_to<const L: usize, const M: usize>(a: &BigInt) -> TestResult {
        let x = w::<L>(a);
        let got = x.resize::<M>();
        prop_assert_eq!(got, wide::<M>(a), "Wide<{}> to Wide<{}>: {}", L, M, a);
        if let Some(r) = got {
            prop_assert_eq!(r.resize::<L>(), Some(x), "resizing back restores the value");
        }
        Ok(())
    }

    /// `resize` from `Wide<L>` to every width the kernel uses, narrower, equal and wider.
    pub fn resize_all<const L: usize>(a: &BigInt) -> TestResult {
        resize_to::<L, 2>(a)?;
        resize_to::<L, 4>(a)?;
        resize_to::<L, 5>(a)?;
        resize_to::<L, 6>(a)?;
        resize_to::<L, 8>(a)?;
        Ok(())
    }

    /// The constants: `ZERO`, `one`, `max_value`, `min_value`, and the overflow at each limit.
    pub fn constants<const L: usize>() -> TestResult {
        let (zero, one) = (Wide::<L>::ZERO, Wide::<L>::one());
        let (max, min) = (Wide::<L>::max_value(), Wide::<L>::min_value());
        prop_assert_eq!(value_of(&zero), BigInt::zero());
        prop_assert_eq!(value_of(&one), BigInt::one());
        prop_assert_eq!(value_of(&max), max_of(L));
        prop_assert_eq!(value_of(&min), min_of(L));
        prop_assert!(zero.is_zero() && !one.is_zero() && !max.is_zero() && !min.is_zero());
        prop_assert_eq!(max.signum(), Sign::Positive);
        prop_assert_eq!(min.signum(), Sign::Negative);
        prop_assert!(min < zero && zero < one && one <= max);
        prop_assert_eq!(max.checked_add(&one), None, "max + 1 overflows");
        prop_assert_eq!(min.checked_sub(&one), None, "min − 1 overflows");
        prop_assert_eq!(min.checked_neg(), None, "−min overflows");
        prop_assert_eq!(max.checked_neg(), wide::<L>(&-max_of(L)), "−max = min + 1");
        prop_assert_eq!(
            min.checked_mul(&Wide::<L>::from_i64(-1)),
            None,
            "min × (−1) overflows"
        );
        prop_assert_eq!(
            min.checked_add(&max),
            Some(Wide::<L>::from_i64(-1)),
            "min + max = −1"
        );
        Ok(())
    }

    /// Every split 2^k × 2^(64L−1−k) of the threshold, each factor moved by −1, 0 or +1, with every
    /// sign: the products land on, just below and just above the range limits.
    pub fn threshold_splits<const L: usize>() -> TestResult {
        let p = top(L);
        for k in 0..=p {
            for da in -1i32..=1 {
                for db in -1i32..=1 {
                    let (ma, mb) = (pow2(k) + da, pow2(p - k) + db);
                    for (a, b) in [
                        (ma.clone(), mb.clone()),
                        (-&ma, mb.clone()),
                        (ma.clone(), -&mb),
                        (-&ma, -&mb),
                    ] {
                        if in_range(&a, L) && in_range(&b, L) {
                            mul::<L>(&a, &b)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Exact factorisations q × r = 2^(64L−1) − 1 of the largest value: q from every divisor of
    /// the largest value up to 2^16, and q = 2^d − 1 for each divisor d of 64L − 1 (2^d − 1 divides
    /// 2^(64L−1) − 1). Each must give exactly `max` or `−max`; the neighbours r ± 1 must give the
    /// oracle's answer.
    pub fn factorisations_of_max<const L: usize>() -> TestResult {
        let max = max_of(L);
        let mut divisors: Vec<BigInt> = (1u32..=1 << 16)
            .map(BigInt::from)
            .filter(|q| max.is_multiple_of(q))
            .collect();
        let p = top(L);
        divisors.extend((2..p).filter(|d| p % d == 0).map(|d| pow2(d) - 1u32));
        for q in &divisors {
            prop_assert!(max.is_multiple_of(q), "{} divides {}", q, max);
            let r = &max / q;
            let (x, y) = (w::<L>(q), w::<L>(&r));
            prop_assert_eq!(
                x.checked_mul(&y),
                Some(Wide::<L>::max_value()),
                "{} × {} = max",
                q,
                r
            );
            prop_assert_eq!(
                x.checked_neg().and_then(|n| n.checked_mul(&y)),
                wide::<L>(&-&max),
                "−{} × {} = −max",
                q,
                r
            );
            for b in [&r - 1u32, &r + 1u32, -(&r + 1u32), -(&r + 2u32)] {
                if in_range(&b, L) {
                    mul::<L>(q, &b)?;
                    mul::<L>(&-q, &b)?;
                }
            }
        }
        Ok(())
    }

    /// The fixed corners of the range, every pair, every operation.
    pub fn edges<const L: usize>() -> TestResult {
        let edges = crate::strategies::edge_values(L);
        for a in &edges {
            neg::<L>(a)?;
            signs::<L>(a)?;
            to_i128::<L>(a)?;
            resize_all::<L>(a)?;
            mul_units::<L>(a)?;
            for b in &edges {
                add::<L>(a, b)?;
                sub::<L>(a, b)?;
                mul::<L>(a, b)?;
                order::<L>(a, b)?;
            }
        }
        Ok(())
    }
}

/// The kernel's own use: grid coordinates at ±(2^40 − 1) through products and the cofactor
/// expansions of exact predicates, every intermediate checked against the oracle.
mod kernel_use {
    use geom_num::Wide;
    use num_bigint::BigInt;
    use num_traits::One;
    use proptest::prop_assert_eq;

    use crate::TestResult;
    use crate::oracle::wide;

    /// Bits an orient3d cofactor expansion needs at coordinates within ±(2^40 − 1): differences
    /// < 2^41, products of two < 2^82, 2×2 minors < 2^83, terms < 2^41 · 2^83 = 2^124, and the sum
    /// of three terms < 3 · 2^124 < 2^126. Every width, `Wide<2>` included (2^127), holds it.
    pub const ORIENT3D_BITS: usize = 126;

    /// Bits an in-circle expansion needs at the same coordinates: differences < 2^41, lifted terms
    /// dx² + dy² < 2^83, minors < 2^83, products < 2^166, the sum of three < 3 · 2^166 < 2^168.
    /// `I256` and wider hold it; `Wide<2>` overflows at the extremes and must say so.
    pub const INCIRCLE_BITS: usize = 168;

    /// A `Wide<L>` from the code under test, alongside its exact value. Each operation compares its
    /// result with the oracle (a disagreement panics, which proptest reports as a failure) and gives
    /// `None` exactly when the true result leaves the range.
    #[derive(Clone)]
    pub struct Tracked<const L: usize> {
        w: Wide<L>,
        v: BigInt,
    }

    impl<const L: usize> Tracked<L> {
        /// A coordinate.
        pub fn of(c: i64) -> Self {
            Self {
                w: Wide::from_i64(c),
                v: BigInt::from(c),
            }
        }

        /// The exact value.
        pub fn value(&self) -> &BigInt {
            &self.v
        }

        /// Checks `got` against the exact result `v` of `self op other`.
        fn step(&self, got: Option<Wide<L>>, v: BigInt, op: &str, other: &Self) -> Option<Self> {
            assert_eq!(got, wide::<L>(&v), "Wide<{L}>: {} {op} {}", self.v, other.v);
            got.map(|w| Self { w, v })
        }

        /// `self + other`.
        pub fn add(&self, other: &Self) -> Option<Self> {
            self.step(self.w.checked_add(&other.w), &self.v + &other.v, "+", other)
        }

        /// `self − other`.
        pub fn sub(&self, other: &Self) -> Option<Self> {
            self.step(self.w.checked_sub(&other.w), &self.v - &other.v, "−", other)
        }

        /// `self × other`.
        pub fn mul(&self, other: &Self) -> Option<Self> {
            self.step(self.w.checked_mul(&other.w), &self.v * &other.v, "×", other)
        }
    }

    /// The orient3d determinant of four points by cofactor expansion along the first row of
    /// differences from d, or `None` at the first intermediate that leaves the range.
    pub fn orient3d<const L: usize>(p: &[[i64; 3]; 4]) -> Option<Tracked<L>> {
        let [a, b, c, d] = p;
        let diff = |q: &[i64; 3], k: usize| Tracked::<L>::of(q[k]).sub(&Tracked::of(d[k]));
        let (adx, ady, adz) = (diff(a, 0)?, diff(a, 1)?, diff(a, 2)?);
        let (bdx, bdy, bdz) = (diff(b, 0)?, diff(b, 1)?, diff(b, 2)?);
        let (cdx, cdy, cdz) = (diff(c, 0)?, diff(c, 1)?, diff(c, 2)?);
        let m1 = bdy.mul(&cdz)?.sub(&bdz.mul(&cdy)?)?;
        let m2 = bdx.mul(&cdz)?.sub(&bdz.mul(&cdx)?)?;
        let m3 = bdx.mul(&cdy)?.sub(&bdy.mul(&cdx)?)?;
        adx.mul(&m1)?.sub(&ady.mul(&m2)?)?.add(&adz.mul(&m3)?)
    }

    /// The in-circle determinant of four points (lifted 3×3 form, differences from d), or `None` at
    /// the first intermediate that leaves the range.
    pub fn incircle<const L: usize>(p: &[[i64; 2]; 4]) -> Option<Tracked<L>> {
        let [a, b, c, d] = p;
        let diff = |q: &[i64; 2], k: usize| Tracked::<L>::of(q[k]).sub(&Tracked::of(d[k]));
        let lift = |x: &Tracked<L>, y: &Tracked<L>| x.mul(x)?.add(&y.mul(y)?);
        let (adx, ady) = (diff(a, 0)?, diff(a, 1)?);
        let (bdx, bdy) = (diff(b, 0)?, diff(b, 1)?);
        let (cdx, cdy) = (diff(c, 0)?, diff(c, 1)?);
        let (al, bl, cl) = (lift(&adx, &ady)?, lift(&bdx, &bdy)?, lift(&cdx, &cdy)?);
        let ma = bdx.mul(&cdy)?.sub(&bdy.mul(&cdx)?)?;
        let mb = cdx.mul(&ady)?.sub(&cdy.mul(&adx)?)?;
        let mc = adx.mul(&bdy)?.sub(&ady.mul(&bdx)?)?;
        al.mul(&ma)?.add(&bl.mul(&mb)?)?.add(&cl.mul(&mc)?)
    }

    /// A running product of coordinates: every partial product equals the oracle's, and the first
    /// `None` comes exactly where the partial product first leaves the range.
    pub fn coordinate_product<const L: usize>(factors: &[i64]) -> TestResult {
        let mut acc = Wide::<L>::one();
        let mut exact = BigInt::one();
        for &c in factors {
            let next = &exact * c;
            let got = acc.checked_mul(&Wide::from_i64(c));
            prop_assert_eq!(got, wide::<L>(&next), "Wide<{}>: {} × {}", L, exact, c);
            match got {
                Some(g) => {
                    acc = g;
                    exact = next;
                }
                None => break,
            }
        }
        Ok(())
    }
}

/// Every property and exhaustive edge test, for one width.
macro_rules! width_suite {
    ($module:ident, $limbs:literal) => {
        mod $module {
            use proptest::collection::vec;
            use proptest::prelude::*;

            use crate::kernel_use::{self, INCIRCLE_BITS, ORIENT3D_BITS};
            use crate::strategies::{self as s, coordinate};
            use crate::{checks, config, oracle, run};

            const L: usize = $limbs;

            proptest! {
                #![proptest_config(config())]

                #[test]
                fn from_i128_is_exact(v in any::<i128>()) {
                    checks::from_i128::<L>(v)?;
                }

                #[test]
                fn from_i64_is_exact(v in any::<i64>()) {
                    checks::from_i64::<L>(v)?;
                }

                #[test]
                fn limbs_round_trip(raw in prop::array::uniform::<_, L>(any::<u64>())) {
                    checks::limbs::<L>(raw)?;
                }

                #[test]
                fn add_matches_oracle(a in s::value(L), b in s::value(L)) {
                    checks::add::<L>(&a, &b)?;
                }

                #[test]
                fn add_at_the_limits((a, b) in s::sum_at_limit(L)) {
                    checks::add::<L>(&a, &b)?;
                }

                #[test]
                fn sub_matches_oracle(a in s::value(L), b in s::value(L)) {
                    checks::sub::<L>(&a, &b)?;
                }

                #[test]
                fn sub_at_the_limits((a, b) in s::difference_at_limit(L)) {
                    checks::sub::<L>(&a, &b)?;
                }

                #[test]
                fn neg_matches_oracle(a in s::value(L)) {
                    checks::neg::<L>(&a)?;
                }

                #[test]
                fn mul_matches_oracle(a in s::value(L), b in s::value(L)) {
                    checks::mul::<L>(&a, &b)?;
                }

                #[test]
                fn mul_at_the_overflow_threshold((a, b) in s::product_at_limit(L)) {
                    checks::mul::<L>(&a, &b)?;
                }

                #[test]
                fn mul_units(a in s::value(L)) {
                    checks::mul_units::<L>(&a)?;
                }

                #[test]
                fn ring_laws(a in s::law_operand(L), b in s::law_operand(L), c in s::law_operand(L)) {
                    checks::ring_laws::<L>(&a, &b, &c)?;
                }

                #[test]
                fn signs_match_oracle(a in s::value(L)) {
                    checks::signs::<L>(&a)?;
                }

                #[test]
                fn order_matches_oracle(a in s::value(L), b in s::value(L)) {
                    checks::order::<L>(&a, &b)?;
                }

                #[test]
                fn order_of_close_values((a, b) in s::close_pair(L)) {
                    checks::order::<L>(&a, &b)?;
                }

                #[test]
                fn order_of_values_one_limb_apart((a, b) in s::one_limb_apart(L)) {
                    checks::order::<L>(&a, &b)?;
                }

                #[test]
                fn order_is_transitive(a in s::value(L), b in s::value(L), c in s::value(L)) {
                    checks::order_transitive::<L>(&a, &b, &c)?;
                }

                #[test]
                fn to_i128_matches_oracle(a in s::near_i128_limits(L)) {
                    checks::to_i128::<L>(&a)?;
                }

                #[test]
                fn resize_to_every_width(a in s::near_every_width_limit(L)) {
                    checks::resize_all::<L>(&a)?;
                }

                #[test]
                fn products_of_extreme_coordinates(factors in vec(coordinate(), 1..=16)) {
                    kernel_use::coordinate_product::<L>(&factors)?;
                }

                #[test]
                fn orient3d_at_extreme_coordinates(
                    p in prop::array::uniform4(prop::array::uniform3(coordinate()))
                ) {
                    let det = kernel_use::orient3d::<L>(&p);
                    prop_assert!(oracle::top(L) >= ORIENT3D_BITS);
                    prop_assert!(det.is_some(), "orient3d fits Wide<{}> at {:?}", L, p);
                }

                #[test]
                fn incircle_at_extreme_coordinates(
                    p in prop::array::uniform4(prop::array::uniform2(coordinate()))
                ) {
                    let det = kernel_use::incircle::<L>(&p);
                    if oracle::top(L) >= INCIRCLE_BITS {
                        prop_assert!(det.is_some(), "incircle fits Wide<{}> at {:?}", L, p);
                    }
                }
            }

            #[test]
            fn constants() {
                run(checks::constants::<L>());
            }

            #[test]
            fn edge_values_exhaustively() {
                run(checks::edges::<L>());
            }

            #[test]
            fn products_at_every_split_of_the_threshold() {
                run(checks::threshold_splits::<L>());
            }

            #[test]
            fn exact_factorisations_of_the_largest_value() {
                run(checks::factorisations_of_max::<L>());
            }
        }
    };
}

width_suite!(wide2, 2);
width_suite!(i256, 4);
width_suite!(i320, 5);
width_suite!(i384, 6);
width_suite!(i512, 8);

/// Checks of the oracle itself, so that agreement with it means something.
mod oracle_self_test {
    use num_bigint::BigInt;
    use proptest::collection::vec;
    use proptest::prelude::*;

    use crate::config;
    use crate::oracle::{self, WIDTHS};

    #[test]
    fn reads_hand_written_limbs() {
        assert_eq!(oracle::signed(&[0, 0]), BigInt::from(0));
        assert_eq!(oracle::signed(&[u64::MAX, u64::MAX]), BigInt::from(-1));
        assert_eq!(oracle::signed(&[0, 1 << 63]), BigInt::from(i128::MIN));
        assert_eq!(
            oracle::signed(&[u64::MAX, u64::MAX >> 1]),
            BigInt::from(i128::MAX)
        );
        assert_eq!(oracle::signed(&[0, 0, 1, 0]), oracle::pow2(128));
        assert_eq!(oracle::signed(&[1, 0, 0, 1 << 63]), -oracle::pow2(255) + 1);
        assert_eq!(
            oracle::unsigned(&[u64::MAX, u64::MAX]),
            oracle::pow2(128) - 1
        );
    }

    #[test]
    fn writes_hand_written_limbs() {
        assert_eq!(
            oracle::limbs::<4>(&BigInt::from(-2)),
            Some([u64::MAX - 1, u64::MAX, u64::MAX, u64::MAX])
        );
        assert_eq!(
            oracle::limbs::<2>(&BigInt::from(i128::MIN)),
            Some([0, 1 << 63])
        );
        assert_eq!(oracle::limbs::<2>(&(BigInt::from(i128::MAX) + 1)), None);
        assert_eq!(oracle::limbs::<2>(&(BigInt::from(i128::MIN) - 1)), None);
        assert_eq!(
            oracle::limbs::<5>(&oracle::pow2(256)),
            Some([0, 0, 0, 0, 1])
        );
    }

    #[test]
    fn ranges_match_the_widths() {
        for l in WIDTHS {
            assert_eq!(
                oracle::max_of(l) - oracle::min_of(l) + 1,
                oracle::pow2(64 * l)
            );
            assert!(
                oracle::in_range(&oracle::max_of(l), l)
                    && !oracle::in_range(&(oracle::max_of(l) + 1), l)
            );
            assert!(
                oracle::in_range(&oracle::min_of(l), l)
                    && !oracle::in_range(&(oracle::min_of(l) - 1), l)
            );
        }
        assert_eq!(oracle::max_of(2), BigInt::from(i128::MAX));
        assert_eq!(oracle::min_of(2), BigInt::from(i128::MIN));
    }

    proptest! {
        #![proptest_config(config())]

        #[test]
        fn agrees_with_i128_twos_complement(v in any::<i128>()) {
            // The two's-complement limbs of an i128 are its low and high 64 bits.
            let expected = [v as u64, (v >> 64) as u64];
            prop_assert_eq!(oracle::limbs::<2>(&BigInt::from(v)), Some(expected));
            prop_assert_eq!(oracle::signed(&expected), BigInt::from(v));
        }

        #[test]
        fn reading_then_writing_limbs_round_trips(raw in vec(any::<u64>(), 8)) {
            prop_assert_eq!(oracle::limbs::<8>(&oracle::signed(&raw)).map(Vec::from), Some(raw));
        }
    }
}

/// Exhaustive extreme corners: every point at (±(2^40 − 1), …), in the narrowest width.
mod extreme_corners {
    use crate::kernel_use::{self, INCIRCLE_BITS, ORIENT3D_BITS};
    use crate::oracle;
    use crate::strategies::C;

    /// Coordinate i of a corner pattern: bit i chooses +C or −C.
    fn corner(bits: u32, i: u32) -> i64 {
        if (bits >> i) & 1 == 1 { C } else { -C }
    }

    #[test]
    fn orient3d_fits_wide2_at_every_corner() {
        assert!(oracle::top(2) >= ORIENT3D_BITS);
        for bits in 0u32..1 << 12 {
            let c = |i| corner(bits, i);
            let p = [
                [c(0), c(1), c(2)],
                [c(3), c(4), c(5)],
                [c(6), c(7), c(8)],
                [c(9), c(10), c(11)],
            ];
            assert!(
                kernel_use::orient3d::<2>(&p).is_some(),
                "orient3d overflowed at {p:?}"
            );
        }
    }

    #[test]
    fn incircle_overflow_is_reported_exactly_at_every_corner() {
        assert!(oracle::top(2) < INCIRCLE_BITS && oracle::top(4) >= INCIRCLE_BITS);
        let (mut fitted, mut overflowed) = (0, 0);
        for bits in 0u32..1 << 8 {
            let c = |i| corner(bits, i);
            let p = [[c(0), c(1)], [c(2), c(3)], [c(4), c(5)], [c(6), c(7)]];
            // Every step of the narrow evaluation is checked against the oracle inside `Tracked`.
            match kernel_use::incircle::<2>(&p) {
                Some(_) => fitted += 1,
                None => overflowed += 1,
            }
            let wide = kernel_use::incircle::<4>(&p).expect("incircle fits I256");
            let wider = kernel_use::incircle::<8>(&p).expect("incircle fits I512");
            assert_eq!(wide.value(), wider.value());
        }
        // Both outcomes occur, so both paths of the narrow evaluation were exercised.
        assert!(
            fitted > 0 && overflowed > 0,
            "fitted {fitted}, overflowed {overflowed}"
        );
    }

    #[test]
    fn powers_of_the_extreme_coordinate() {
        for factors in [[C; 16], [-C; 16]] {
            crate::run(kernel_use::coordinate_product::<2>(&factors));
            crate::run(kernel_use::coordinate_product::<4>(&factors));
            crate::run(kernel_use::coordinate_product::<5>(&factors));
            crate::run(kernel_use::coordinate_product::<6>(&factors));
            crate::run(kernel_use::coordinate_product::<8>(&factors));
        }
    }
}
