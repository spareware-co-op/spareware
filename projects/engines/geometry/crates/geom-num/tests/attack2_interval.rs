//! Second-round adversarial tests of `geom_num::interval`, after the change that added `square`
//! and `sqrt_nonneg` (git diff 3926ab2 -- src/interval.rs). They try to break the module, not to
//! confirm it. `tests/attack_eft_interval.rs` was the first round; nothing here is shared with it.
//!
//! Oracles, written here and sharing no code with the module:
//! - `dy`: exact dyadic numbers m · 2^e on `num-bigint`, for sums, differences, products and
//!   squares of doubles, and their rounding to doubles from the bit fields (the significand at the
//!   binade's quantum; the bits shifted out decide). The square root by bisection over bit patterns
//!   (the largest double whose exact square is at most x), then a midpoint test.
//! - `q`: the same rounding on `num-rational`, for quotients, by a different construction (the
//!   binade from bit lengths, the significand by a rational floor). Both are checked against the
//!   hardware, against each other, and against a bisection over every double.
//!
//! Each operation is held bit for bit to the method its documentation states, and, independently
//! of the bits, to inclusion, strict widening (every rounded bound is stepped, even an exact one),
//! tightness, signed zeros and the soundness of `certain_sign`. What is attacked:
//! - Every binade, exhaustively: sums, products, quotients, squares and roots whose exact value is
//!   at, just below or just above 2^k for every k from −1074 to 1023, with residues of half a unit
//!   (the ties), a quarter, three quarters and far below.
//! - The overflow threshold: sums at Ω ± 2^970 (the tie), products and quotients exactly Ω and
//!   Ω − 2^971 (by factoring 2^53 − 1 and 2^52 − 1), squares at 2^512, a lower bound that rounds
//!   to −Ω while the upper bound is finite. `NotFinite` exactly when a stepped bound is infinite.
//! - Signed zeros in every position of every operation; `certain_sign` on every interval over a
//!   grid of bounds; `Zero` only from the operations that do not widen a zero bound.
//! - Division by intervals touching zero through ±0, ±2^−1074 and wide bounds, and which error
//!   comes first.
//! - The square root near zero: subnormal operands, exact roots, and hard cases whose root lies
//!   within a tiny fraction of a unit of a midpoint (the correct rounding the module relies on).
//! - The design gap the first round found: the distance between coincident points, now in 3D at
//!   every scale, through `square` and `mul`; the root of the non-negative part on every kind of
//!   interval, including the roots of interior values. That operation, `sqrt_nonneg`, was public
//!   then and is private to the crate since the third review round (only `norm` takes it); these
//!   tests now apply its documented method through the public `sqrt`, and the crate's unit tests
//!   hold the private function to that method bit for bit.
//! - Laws that tie operations together, bit for bit: commutativity, `a − b = a + (−b)`,
//!   `a − b = −(b − a)`, sign symmetry of products, quotients and squares; inclusion monotonicity
//!   (a narrower operand never gives a wider result or a new error).
//! - Agreement with `eft::two_sum` and `eft::two_prod`: the interval of a sum or product of two
//!   doubles is the rounded value stepped once each way, and the sign of the exact error picks the
//!   half that holds the exact value.
//! - Expression trees with leaves a few steps wide: at every node the interval holds the exact
//!   value for a point chosen inside each leaf.
//! - The documentation's tightness claim: each bound the first or second double beyond its exact
//!   bound, and the maxima it states in steps, with its two examples. The first form of this
//!   section showed that the earlier wording, "less than two floating-point steps outside", was
//!   false in either reading of the step; the module's documentation was restated to what holds.
//! - `norm`, added after that report so that a length proves its own radicand non-negative
//!   instead of taking the caller's word, as `sqrt_nonneg` must: bit for bit its method, each
//!   step against its oracle, the whole exact range of lengths inside, coincident points and
//!   points one step apart, and overflow reported.

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

use dy::Dy;
use geom_num::interval::Interval;
use geom_num::{NumError, Sign};
use num_rational::BigRational;
use num_traits::Zero;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError};
use std::fmt;

/// The outcome of one check: `Err` carries the first disagreement with an oracle.
type TestResult = Result<(), TestCaseError>;

/// The largest finite double, Ω = (2 − 2^−52) · 2^1023.
const OMEGA: f64 = f64::MAX;

/// The smallest positive double, 2^−1074.
const TINY: f64 = f64::from_bits(1);

/// A property's configuration: `cases` random cases, and no regression files written next to the
/// sources (this file is the only one these tests may touch).
fn config(cases: u32) -> Config {
    Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    }
}

/// Runs a check outside `proptest!`, failing the test loudly.
fn run(result: TestResult) {
    if let Err(e) = result {
        panic!("{e}");
    }
}

/// The point interval of a finite double.
fn pt(x: f64) -> Interval {
    Interval::point(x).unwrap()
}

/// The interval `[lo, hi]` of finite doubles with `lo ≤ hi`.
fn iv(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

/// The bits of a result, so results compare exactly.
fn bits(r: Result<Interval, NumError>) -> Result<(u64, u64), NumError> {
    r.map(|i| (i.lo().to_bits(), i.hi().to_bits()))
}

/// The neighbours of an exact value among the doubles.
#[derive(Clone, Copy, Debug)]
pub struct Bracket {
    /// The largest double at or below the value (−∞ below −Ω).
    floor: f64,
    /// The smallest double at or above the value (+∞ above Ω).
    ceil: f64,
    /// The nearest double, ties to even; ±∞ from |v| ≥ Ω + 2^970, the midpoint between Ω and
    /// 2^1024, which ties to the even 2^1024.
    nearest: f64,
}

/// An exact number an oracle computes with: a dyadic or a rational.
trait Exact: Clone + Ord + fmt::Display {
    /// The exact value of a finite double.
    fn of(x: f64) -> Self;
    /// The neighbours among the doubles.
    fn bracket(&self) -> Bracket;
    /// The sign.
    fn sign(&self) -> Sign;
}

impl Exact for Dy {
    fn of(x: f64) -> Self {
        Dy::of(x)
    }

    fn bracket(&self) -> Bracket {
        Dy::bracket(self)
    }

    fn sign(&self) -> Sign {
        Dy::sign(self)
    }
}

impl Exact for BigRational {
    fn of(x: f64) -> Self {
        q::exact(x)
    }

    fn bracket(&self) -> Bracket {
        q::bracket(self)
    }

    fn sign(&self) -> Sign {
        q::sign(self)
    }
}

/// Bits of doubles, independent of `f64::next_up` and friends.
mod fp {
    use super::TINY;

    /// The next double up; +∞ stays, −∞ goes to −Ω, the next above −2^−1074 is a zero.
    pub fn up(x: f64) -> f64 {
        assert!(!x.is_nan(), "up(NaN)");
        if x == f64::INFINITY {
            x
        } else if x == 0.0 {
            TINY
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

    /// 2^k, for every k a double holds (−1074 ..= 1023).
    pub fn pow2(k: i32) -> f64 {
        assert!((-1074..=1023).contains(&k), "2^{k} is not a double");
        if k >= -1022 {
            f64::from_bits(u64::try_from(k + 1023).unwrap() << 52)
        } else {
            f64::from_bits(1u64 << (k + 1074))
        }
    }

    /// The double with significand `k` at the quantum of the binade whose leading bit is 2^top:
    /// k in [2^52, 2^53) for a normal binade (top ≥ −1022), k < 2^52 below it (quantum 2^−1074).
    pub fn from_significand(k: u64, top: i64) -> f64 {
        if top >= -1022 {
            assert!(
                (1u64 << 52..1u64 << 53).contains(&k),
                "significand {k} at 2^{top}"
            );
            let biased = u64::try_from(top + 1023).unwrap();
            f64::from_bits((biased << 52) | (k - (1u64 << 52)))
        } else {
            assert!(k < 1u64 << 52, "significand {k} below 2^−1022");
            f64::from_bits(k)
        }
    }

    /// −0 as +0; every other value unchanged.
    pub fn canon(x: f64) -> f64 {
        if x == 0.0 { 0.0 } else { x }
    }

    /// Whether `x` is −0.
    pub fn is_minus_zero(x: f64) -> bool {
        x.to_bits() == 1u64 << 63
    }

    /// `x` moved `n` steps along the doubles (up for n > 0), stopping at the last finite double.
    pub fn steps(x: f64, n: i64) -> f64 {
        let mut y = x;
        for _ in 0..n.unsigned_abs() {
            let next = if n > 0 { up(y) } else { down(y) };
            if !next.is_finite() {
                break;
            }
            y = next;
        }
        y
    }
}

/// Exact dyadic numbers m · 2^e on `num-bigint`: sums, differences and products of doubles with no
/// rounding and no reduction, rounded to doubles from the bit fields.
mod dy {
    use super::{Bracket, fp};
    use geom_num::Sign;
    use num_bigint::{BigInt, BigUint};
    use num_rational::BigRational;
    use num_traits::{One, Signed, ToPrimitive, Zero};
    use std::cmp::Ordering;
    use std::fmt;
    use std::ops::{Add, Mul, Neg, Sub};

    /// The exact value m · 2^e.
    #[derive(Clone, Debug)]
    pub struct Dy {
        /// The integer part.
        m: BigInt,
        /// The power of two it is scaled by.
        e: i64,
    }

    impl Dy {
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
            Dy {
                m: if x.is_sign_negative() { -m } else { m },
                e,
            }
        }

        /// The two integer parts on the scale of the smaller exponent, and that exponent.
        fn aligned(&self, o: &Dy) -> (BigInt, BigInt, i64) {
            let e = self.e.min(o.e);
            let a = &self.m << u64::try_from(self.e - e).unwrap();
            let b = &o.m << u64::try_from(o.e - e).unwrap();
            (a, b, e)
        }

        /// Half the value, exactly.
        pub fn half(&self) -> Dy {
            Dy {
                m: self.m.clone(),
                e: self.e - 1,
            }
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

        /// The position of the leading bit: 2^top ≤ |value| < 2^(top + 1), if nonzero.
        pub fn top(&self) -> Option<i64> {
            if self.m.is_zero() {
                None
            } else {
                Some(self.e + i64::try_from(self.m.bits()).unwrap() - 1)
            }
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

        /// The neighbours among the doubles. The significand k is the magnitude at the quantum of
        /// its binade (2^(top − 52), never finer than 2^−1074); the bits shifted out decide the
        /// rounding: below half a unit down, above it up, exactly half to the even significand.
        pub fn bracket(&self) -> Bracket {
            let Some(top) = self.top() else {
                return Bracket {
                    floor: 0.0,
                    ceil: 0.0,
                    nearest: 0.0,
                };
            };
            let a = self.m.magnitude();
            let (below, above, nearest) = if top >= 1024 {
                (f64::MAX, f64::INFINITY, f64::INFINITY)
            } else {
                let quantum = (top - 52).max(-1074);
                let (k, rest) = if quantum <= self.e {
                    (a << u64::try_from(self.e - quantum).unwrap(), None)
                } else {
                    let s = u64::try_from(quantum - self.e).unwrap();
                    let k = a >> s;
                    let rest: BigUint = a - (&k << s);
                    let half = BigUint::one() << (s - 1);
                    (
                        k,
                        if rest.is_zero() {
                            None
                        } else {
                            Some(rest.cmp(&half))
                        },
                    )
                };
                // Below 2^−1074 the significand is 0 and the value lies between 0 and 2^−1074.
                let t = fp::from_significand(k.to_u64().unwrap(), top.max(-1074));
                match rest {
                    None => (t, t, t),
                    Some(order) => {
                        // Above Ω the next "double" is 2^1024, whose significand is even.
                        let c = fp::up(t);
                        let even = t.to_bits() & 1 == 0;
                        let nearest = match order {
                            Ordering::Less => t,
                            Ordering::Greater => c,
                            Ordering::Equal => {
                                if even {
                                    t
                                } else {
                                    c
                                }
                            }
                        };
                        (t, c, nearest)
                    }
                }
            };
            if self.m.is_negative() {
                Bracket {
                    floor: -above,
                    ceil: -below,
                    nearest: -nearest,
                }
            } else {
                Bracket {
                    floor: below,
                    ceil: above,
                    nearest,
                }
            }
        }
    }

    impl Add for &Dy {
        type Output = Dy;

        fn add(self, o: &Dy) -> Dy {
            let (a, b, e) = self.aligned(o);
            Dy { m: a + b, e }
        }
    }

    impl Sub for &Dy {
        type Output = Dy;

        fn sub(self, o: &Dy) -> Dy {
            let (a, b, e) = self.aligned(o);
            Dy { m: a - b, e }
        }
    }

    impl Mul for &Dy {
        type Output = Dy;

        fn mul(self, o: &Dy) -> Dy {
            Dy {
                m: &self.m * &o.m,
                e: self.e + o.e,
            }
        }
    }

    impl Neg for &Dy {
        type Output = Dy;

        fn neg(self) -> Dy {
            Dy {
                m: -&self.m,
                e: self.e,
            }
        }
    }

    impl Ord for Dy {
        fn cmp(&self, o: &Dy) -> Ordering {
            let (a, b, _) = self.aligned(o);
            a.cmp(&b)
        }
    }

    impl PartialOrd for Dy {
        fn partial_cmp(&self, o: &Dy) -> Option<Ordering> {
            Some(self.cmp(o))
        }
    }

    impl PartialEq for Dy {
        fn eq(&self, o: &Dy) -> bool {
            self.cmp(o) == Ordering::Equal
        }
    }

    impl Eq for Dy {}

    impl fmt::Display for Dy {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.rational())
        }
    }

    /// The largest double r ≥ 0 with r² ≤ x, for a finite x ≥ 0, by bisection over bit patterns
    /// (ordered like the values for non-negative doubles). With 2^t ≤ x < 2^(t + 1) and
    /// h = ⌊t/2⌋, √x lies in [2^h, 2^(h + 1)), so the search starts between those two doubles.
    pub fn sqrt_floor(x: f64) -> f64 {
        assert!(x.is_finite() && x >= 0.0, "sqrt_floor({x})");
        let target = Dy::of(x);
        let Some(t) = target.top() else { return 0.0 };
        let h = i32::try_from(t.div_euclid(2)).unwrap();
        let (mut lo, mut hi) = (fp::pow2(h).to_bits(), fp::pow2(h + 1).to_bits());
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            let r = Dy::of(f64::from_bits(mid));
            if &r * &r <= target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        f64::from_bits(lo)
    }

    /// The neighbours of √x among the doubles, for a finite x ≥ 0, and the correctly rounded root.
    pub fn sqrt_bracket(x: f64) -> Bracket {
        let f = sqrt_floor(x);
        let (fv, xv) = (Dy::of(f), Dy::of(x));
        if &fv * &fv == xv {
            return Bracket {
                floor: f,
                ceil: f,
                nearest: f,
            };
        }
        let c = fp::up(f);
        let mid = (&fv + &Dy::of(c)).half();
        let m2 = &mid * &mid;
        // A midpoint has 54 significant bits, its square more than 53, so it is never a double.
        assert!(m2 != xv, "the square of a midpoint is a double: {x:e}");
        Bracket {
            floor: f,
            ceil: c,
            nearest: if xv < m2 { f } else { c },
        }
    }
}

/// Exact values of doubles, and their rounding, on `num-rational`.
mod q {
    use super::{Bracket, fp};
    use geom_num::Sign;
    use num_bigint::BigInt;
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
    pub fn sign(v: &BigRational) -> Sign {
        if v.is_positive() {
            Sign::Positive
        } else if v.is_negative() {
            Sign::Negative
        } else {
            Sign::Zero
        }
    }

    /// ⌊log2 a⌋ for a > 0. With n and d the bit lengths of the reduced numerator and denominator,
    /// 2^(n − d − 1) < a < 2^(n − d + 1), so one comparison fixes it.
    pub fn log2_floor(a: &BigRational) -> i64 {
        assert!(
            a.is_positive(),
            "log2_floor({a}) of a value that is not positive"
        );
        let n = i64::try_from(a.numer().bits()).unwrap();
        let d = i64::try_from(a.denom().bits()).unwrap();
        let e = n - d;
        if *a < pow2(e) { e - 1 } else { e }
    }

    /// The spacing of the doubles at |v| (the value of the last significand bit of the binade
    /// holding |v|): 2^max(⌊log2 |v|⌋ − 52, −1074), and 2^−1074 at zero.
    pub fn ulp(v: &BigRational) -> BigRational {
        if v.is_zero() {
            return pow2(-1074);
        }
        pow2((log2_floor(&v.abs()) - 52).max(-1074))
    }

    /// The largest double at or below `a ≥ 0`, saturating at Ω: the significand is the rational
    /// floor of a over the quantum of its binade.
    fn trunc_mag(a: &BigRational) -> f64 {
        if a.is_zero() {
            return 0.0;
        }
        let e = log2_floor(a);
        if e >= 1024 {
            return f64::MAX;
        }
        let quantum = (e - 52).max(-1074);
        let k = (a * pow2(-quantum)).floor().to_integer().to_u64().unwrap();
        fp::from_significand(k, e.max(-1074))
    }

    /// The neighbours of `v` among the doubles, from the definition of rounding to nearest.
    pub fn bracket(v: &BigRational) -> Bracket {
        let a = v.abs();
        let t = trunc_mag(&a);
        let tv = exact(t);
        let (below, above, nearest) = if tv == a {
            (t, t, t)
        } else {
            let c = fp::up(t);
            // Above Ω the next "double" is 2^1024, whose significand is even.
            let cv = if c.is_finite() { exact(c) } else { pow2(1024) };
            let even = t.to_bits() & 1 == 0;
            let nearest = match (&a - &tv).cmp(&(&cv - &a)) {
                Ordering::Less => t,
                Ordering::Greater => c,
                Ordering::Equal => {
                    if even {
                        t
                    } else {
                        c
                    }
                }
            };
            (t, c, nearest)
        };
        if v.is_negative() {
            Bracket {
                floor: -above,
                ceil: -below,
                nearest: -nearest,
            }
        } else {
            Bracket {
                floor: below,
                ceil: above,
                nearest,
            }
        }
    }

    /// The largest double at or below `v`, by bisection over the ordered doubles: a third oracle,
    /// slow, sharing nothing with the other two but `exact`.
    pub fn floor_by_bisection(v: &BigRational) -> f64 {
        // Every double, −∞ to +∞, as an integer in the same order: its magnitude bits, negated for
        // a negative double (both zeros give 0). i128, so the span of keys cannot overflow.
        fn key(x: f64) -> i128 {
            let magnitude = i128::from(x.to_bits() & !(1u64 << 63));
            if x.is_sign_negative() {
                -magnitude
            } else {
                magnitude
            }
        }
        fn unkey(k: i128) -> f64 {
            let magnitude = f64::from_bits(u64::try_from(k.unsigned_abs()).unwrap());
            if k < 0 { -magnitude } else { magnitude }
        }
        // Invariant: unkey(lo) ≤ v (−∞ counts as below everything), unkey(hi) > v.
        let (mut lo, mut hi) = (key(f64::NEG_INFINITY), key(f64::INFINITY));
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            let m = unkey(mid);
            if m.is_finite() && exact(m) <= *v {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        fp::canon(unkey(lo))
    }
}

/// Doubles and intervals weighted towards where outward rounding breaks.
mod strat {
    use super::{OMEGA, TINY, fp};
    use geom_num::interval::Interval;
    use proptest::prelude::*;
    use std::ops::RangeInclusive;

    /// The 52 fraction bits.
    const FRAC: u64 = (1 << 52) - 1;

    /// A normal double from its sign, unbiased exponent (−1022 ..= 1023) and fraction bits.
    pub fn make(negative: bool, exponent: i32, fraction: u64) -> f64 {
        assert!(
            (-1022..=1023).contains(&exponent),
            "exponent {exponent} is not normal"
        );
        let biased = u64::try_from(exponent + 1023).unwrap();
        f64::from_bits((u64::from(negative) << 63) | (biased << 52) | (fraction & FRAC))
    }

    /// 52 fraction bits: random, or a pattern (zero, all ones, one bit, a run of ones).
    pub fn fraction() -> BoxedStrategy<u64> {
        prop_oneof![
            3 => any::<u64>().prop_map(|f| f & FRAC),
            1 => Just(0u64),
            1 => Just(FRAC),
            1 => (0u32..52).prop_map(|k| 1u64 << k),
            1 => (0u32..52).prop_map(|k| FRAC >> k),
        ]
        .boxed()
    }

    /// A normal double with its exponent in `range`, either sign.
    pub fn normal(range: RangeInclusive<i32>) -> BoxedStrategy<f64> {
        (any::<bool>(), range, fraction())
            .prop_map(|(s, e, f)| make(s, e, f))
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

    /// ±2^k moved up to three steps either way, for every k a double holds: the doubles on both
    /// sides of every binade boundary, the subnormal one at 2^−1022 included.
    pub fn near_pow2() -> BoxedStrategy<f64> {
        (any::<bool>(), -1074i32..=1023, -3i64..=3)
            .prop_map(|(s, k, j)| {
                let x = fp::steps(fp::pow2(k), j);
                if s { -x } else { x }
            })
            .boxed()
    }

    /// Fixed doubles at the edges of the format, both signs.
    pub fn edges() -> Vec<f64> {
        let mut v = vec![
            0.0,
            TINY,
            f64::from_bits(2),
            f64::from_bits(FRAC),
            f64::MIN_POSITIVE,
            fp::pow2(-537),
            fp::steps(fp::pow2(-537), 1),
            0.5,
            1.0,
            fp::steps(1.0, 1),
            fp::steps(1.0, -1),
            3.0,
            fp::pow2(40) - 1.0,
            fp::pow2(511),
            fp::steps(fp::pow2(512), -1),
            fp::pow2(512),
            fp::pow2(970),
            fp::pow2(1023),
            fp::steps(OMEGA, -1),
            OMEGA,
        ];
        let negated: Vec<f64> = v.iter().map(|x| -x).collect();
        v.extend(negated);
        v
    }

    /// A double of any kind.
    pub fn wild() -> BoxedStrategy<f64> {
        prop_oneof![
            2 => any_finite(),
            2 => near_pow2(),
            1 => normal(-1022..=1023),
            1 => subnormal(),
            1 => normal(1015..=1023),
            2 => prop::sample::select(edges()),
            1 => (-64i32..=64).prop_map(f64::from),
        ]
        .boxed()
    }

    /// A double of moderate size, for expression trees that should mostly stay finite.
    pub fn moderate() -> BoxedStrategy<f64> {
        prop_oneof![
            3 => -1.0e6f64..1.0e6,
            2 => (-64i32..=64).prop_map(f64::from),
            2 => normal(-40..=40),
            // A grid coordinate of up to 2^40 − 1 steps, as h · 2^20 + l, exactly.
            1 => (-(1i32 << 20) + 1..(1i32 << 20), 0i32..(1 << 20))
                .prop_map(|(h, l)| f64::from(h) * 1_048_576.0 + f64::from(l)),
            1 => near_pow2(),
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
            2 => values.clone().prop_map(|x| Interval::point(x).unwrap()),
            1 => (values, 1i64..4).prop_map(|(x, k)| hull(x, fp::steps(x, k))),
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

    /// An interval with a zero bound (either zero), or a bound one step from zero.
    pub fn touching_zero() -> BoxedStrategy<Interval> {
        prop_oneof![
            2 => wild().prop_map(|x| hull(0.0, x)),
            2 => wild().prop_map(|x| hull(-0.0, x)),
            1 => wild().prop_map(|x| hull(TINY, x)),
            1 => wild().prop_map(|x| hull(-TINY, x)),
            1 => Just(Interval::new(-0.0, 0.0).unwrap()),
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
        ]
        .boxed()
    }

    /// Two points whose sum lands within a few units of a power of two: ±2^k moved j steps and
    /// ±2^(k − d) moved i steps, d from 1 to 110.
    pub fn binade_sum() -> BoxedStrategy<(Interval, Interval)> {
        let shape = (-1074i32..=1023, 1i32..=110, -2i64..=2, -2i64..=2);
        (shape, any::<bool>(), any::<bool>())
            .prop_map(|((k, d, j, i), s, t)| {
                let a = fp::steps(fp::pow2(k), j);
                let b = fp::steps(fp::pow2((k - d).max(-1074)), i);
                let a = if s { -a } else { a };
                let b = if t { -b } else { b };
                (Interval::point(a).unwrap(), Interval::point(b).unwrap())
            })
            .boxed()
    }

    /// A pair of operands from one of the regimes where outward rounding breaks.
    pub fn pair() -> BoxedStrategy<(Interval, Interval)> {
        prop_oneof![
            3 => (interval(), interval()),
            // Products and quotients of near-powers of two land next to a binade boundary.
            2 => (interval_of(near_pow2()), interval_of(near_pow2())),
            2 => binade_sum(),
            1 => (interval_of(normal(1018..=1023)), interval_of(normal(1018..=1023))),
            1 => (interval_of(normal(505..=518)), interval_of(normal(505..=518))),
            1 => (interval_of(normal(-545..=-525)), interval_of(normal(-545..=-525))),
            1 => (interval(), one_signed(subnormal())),
            1 => (touching_zero(), interval()),
            1 => (interval(), touching_zero()),
        ]
        .boxed()
    }

    /// An interval inside another: the middle two of four sorted doubles inside the outer two, or
    /// an interval inside itself widened by a few steps.
    pub fn nested() -> BoxedStrategy<(Interval, Interval)> {
        let values = prop_oneof![wild(), near_pow2(), moderate()].boxed();
        prop_oneof![
            2 => proptest::collection::vec(values.clone(), 4).prop_map(|mut v| {
                v.sort_by(f64::total_cmp);
                (hull(v[1], v[2]), hull(v[0], v[3]))
            }),
            1 => (interval_of(values), 0i64..3, 0i64..3).prop_map(|(a, i, j)| {
                (a, hull(fp::steps(a.lo(), -i), fp::steps(a.hi(), j)))
            }),
        ]
        .boxed()
    }
}

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

/// Whether an interval holds an exact value.
fn holds<V: Exact>(r: Interval, v: &V) -> bool {
    V::of(r.lo()) <= *v && *v <= V::of(r.hi())
}

/// Whether neither bound is −0.
fn no_minus_zero(r: Interval) -> bool {
    !fp::is_minus_zero(r.lo()) && !fp::is_minus_zero(r.hi())
}

/// The certain sign, if any, is the sign of every exact value given.
fn sign_is_sound<V: Exact>(r: Interval, values: &[&V]) -> TestResult {
    if let Some(s) = r.certain_sign() {
        for v in values {
            prop_assert_eq!(
                s,
                v.sign(),
                "{:?} claims {:?} but holds the exact value {}",
                r,
                s,
                v
            );
        }
    }
    Ok(())
}

/// The smallest and largest of some exact values.
fn min_max<V: Exact>(values: &[V]) -> (V, V) {
    let lo = values.iter().min().unwrap().clone();
    let hi = values.iter().max().unwrap().clone();
    (lo, hi)
}

/// A result against the documented method for an exact range `[m, big_m]`, bit for bit: round each
/// exact bound to nearest, step it outward once, store zero as +0; `NotFinite` exactly when a
/// stepped bound is infinite. Then, independently of the bits: finite and ordered, no −0, both
/// exact bounds strictly inside (every rounded bound is stepped, even an exact one), each bound at
/// most the second double beyond its exact bound, a sound sign, and never `Zero`.
fn check_widened<V: Exact>(
    got: Result<Interval, NumError>,
    m: &V,
    big_m: &V,
    what: &dyn Fn() -> String,
) -> TestResult {
    let (lo_b, hi_b) = (m.bracket(), big_m.bracket());
    let (lo_w, hi_w) = (fp::down(lo_b.nearest), fp::up(hi_b.nearest));
    if !lo_w.is_finite() || !hi_w.is_finite() {
        prop_assert_eq!(
            got,
            Err(NumError::NotFinite),
            "{}: the exact range [{}, {}] rounds to [{:e}, {:e}], so a stepped bound is infinite",
            what(),
            m,
            big_m,
            lo_b.nearest,
            hi_b.nearest
        );
        return Ok(());
    }
    let (lo_w, hi_w) = (fp::canon(lo_w), fp::canon(hi_w));
    prop_assert_eq!(
        bits(got),
        Ok((lo_w.to_bits(), hi_w.to_bits())),
        "{}: the exact range [{}, {}] rounds to [{:e}, {:e}], so the method gives [{:e}, {:e}]",
        what(),
        m,
        big_m,
        lo_b.nearest,
        hi_b.nearest,
        lo_w,
        hi_w
    );
    let r = got.unwrap();
    prop_assert!(
        r.lo().is_finite() && r.hi().is_finite() && r.lo() <= r.hi(),
        "{}",
        what()
    );
    prop_assert!(no_minus_zero(r), "{}: a −0 bound", what());
    prop_assert!(
        V::of(r.lo()) < *m && *big_m < V::of(r.hi()),
        "{}: the exact range [{}, {}] is not strictly inside",
        what(),
        m,
        big_m
    );
    prop_assert!(
        r.lo() >= fp::down(lo_b.floor) && r.hi() <= fp::up(hi_b.ceil),
        "{}: more than two doubles outside the exact range [{}, {}]",
        what(),
        m,
        big_m
    );
    sign_is_sound(r, &[m, big_m])?;
    prop_assert!(
        r.certain_sign() != Some(Sign::Zero),
        "{}: decided Zero",
        what()
    );
    Ok(())
}

/// One binary operation against its documented method: `DivisorContainsZero` exactly when the
/// divisor holds 0, before anything else; otherwise `check_widened` on the exact range, the
/// extremes of the four corner values (dyadic for sums, differences and products, rational for
/// quotients).
fn check_binary(op: Op, a: Interval, b: Interval) -> TestResult {
    let got = op.apply(a, b);
    let what = || format!("{op:?} {a:?} {b:?} = {got:?}");
    if op == Op::Div {
        let (al, ah) = (q::exact(a.lo()), q::exact(a.hi()));
        let (bl, bh) = (q::exact(b.lo()), q::exact(b.hi()));
        let zero = BigRational::zero();
        if bl <= zero && bh >= zero {
            prop_assert_eq!(
                got,
                Err(NumError::DivisorContainsZero),
                "{}: the divisor holds 0",
                what()
            );
            return Ok(());
        }
        let (m, big_m) = min_max(&[&al / &bl, &al / &bh, &ah / &bl, &ah / &bh]);
        return check_widened(got, &m, &big_m, &what);
    }
    let (al, ah, bl, bh) = (
        Dy::of(a.lo()),
        Dy::of(a.hi()),
        Dy::of(b.lo()),
        Dy::of(b.hi()),
    );
    let corners = match op {
        Op::Add => [&al + &bl, &al + &bh, &ah + &bl, &ah + &bh],
        Op::Sub => [&al - &bl, &al - &bh, &ah - &bl, &ah - &bh],
        _ => [&al * &bl, &al * &bh, &ah * &bl, &ah * &bh],
    };
    let (m, big_m) = min_max(&corners);
    check_widened(got, &m, &big_m, &what)
}

/// `square` against its documentation, written from the exact range of x² over the interval (0
/// when it holds zero, else the smaller square; the larger square on top): upper bound rounded and
/// stepped up, `NotFinite` when that is infinite; lower bound exactly 0 when the range starts at 0,
/// else rounded, stepped down and kept at or above 0. Then: no −0, inclusion, a strictly widened
/// upper bound, tightness, a sound sign that is never `Zero`, inside `mul(a, a)`, and refusing only
/// where `mul(a, a)` refuses too.
fn check_square(a: Interval) -> TestResult {
    let got = a.square();
    let what = || format!("square {a:?} = {got:?}");
    let (l, h) = (Dy::of(a.lo()), Dy::of(a.hi()));
    let (l2, h2) = (&l * &l, &h * &h);
    let straddles = l.sign() != Sign::Positive && h.sign() != Sign::Negative;
    let max = if l2 > h2 { l2.clone() } else { h2.clone() };
    let min = if straddles {
        Dy::of(0.0)
    } else if l2 < h2 {
        l2
    } else {
        h2
    };
    let hi_b = max.bracket();
    let hi_w = fp::up(hi_b.nearest);
    if !hi_w.is_finite() {
        prop_assert_eq!(
            got,
            Err(NumError::NotFinite),
            "{}: the top square {}",
            what(),
            max
        );
        prop_assert_eq!(
            a.mul(a),
            Err(NumError::NotFinite),
            "{}: but mul does not refuse",
            what()
        );
        return Ok(());
    }
    let lo_b = min.bracket();
    let lo_w = if straddles {
        0.0
    } else {
        let s = fp::canon(fp::down(lo_b.nearest));
        if s < 0.0 { 0.0 } else { s }
    };
    let hi_w = fp::canon(hi_w);
    prop_assert_eq!(
        bits(got),
        Ok((lo_w.to_bits(), hi_w.to_bits())),
        "{}: the exact squares are [{}, {}], so the method gives [{:e}, {:e}]",
        what(),
        min,
        max,
        lo_w,
        hi_w
    );
    let r = got.unwrap();
    prop_assert!(r.lo() >= 0.0 && no_minus_zero(r), "{}", what());
    prop_assert!(
        holds(r, &min) && holds(r, &max),
        "{}: misses [{}, {}]",
        what(),
        min,
        max
    );
    prop_assert!(
        max < Dy::of(r.hi()),
        "{}: the upper bound is not stepped",
        what()
    );
    prop_assert!(
        r.lo() >= fp::down(lo_b.floor) && r.hi() <= fp::up(hi_b.ceil),
        "{}: more than two doubles outside [{}, {}]",
        what(),
        min,
        max
    );
    sign_is_sound(r, &[&min, &max])?;
    prop_assert!(
        r.certain_sign() != Some(Sign::Zero),
        "{}: decided Zero",
        what()
    );
    if let Ok(m) = a.mul(a) {
        prop_assert!(
            m.lo() <= r.lo() && r.hi() <= m.hi(),
            "{}: not inside mul {:?}",
            what(),
            m
        );
    }
    Ok(())
}

/// `sqrt` against its documentation with the bisection oracle: `NegativeSqrt` exactly when the
/// lower bound is negative; a zero bound stays +0; every other bound is the correctly rounded root
/// stepped outward once. Then: inclusion by squaring, tightness, a positive lower bound for a
/// positive operand, and `Zero` exactly for the operand [0, 0].
fn check_sqrt(a: Interval) -> TestResult {
    let got = a.sqrt();
    let what = || format!("sqrt {a:?} = {got:?}");
    if a.lo() < 0.0 {
        prop_assert_eq!(got, Err(NumError::NegativeSqrt), "{}", what());
        return Ok(());
    }
    let (lo_w, lo_floor) = if a.lo() == 0.0 {
        (0.0, 0.0)
    } else {
        let r = dy::sqrt_bracket(a.lo());
        (fp::canon(fp::down(r.nearest)), r.floor)
    };
    let (hi_w, hi_ceil) = if a.hi() == 0.0 {
        (0.0, 0.0)
    } else {
        let r = dy::sqrt_bracket(a.hi());
        (fp::canon(fp::up(r.nearest)), r.ceil)
    };
    prop_assert_eq!(
        bits(got),
        Ok((lo_w.to_bits(), hi_w.to_bits())),
        "{}: the method gives [{:e}, {:e}]",
        what(),
        lo_w,
        hi_w
    );
    let r = got.unwrap();
    let (rl, rh) = (Dy::of(r.lo()), Dy::of(r.hi()));
    prop_assert!(r.lo() >= 0.0 && no_minus_zero(r), "{}", what());
    prop_assert!(
        &rl * &rl <= Dy::of(a.lo()) && Dy::of(a.hi()) <= &rh * &rh,
        "{}: misses a root",
        what()
    );
    prop_assert!(
        r.lo() >= fp::down(lo_floor) && r.hi() <= fp::up(hi_ceil),
        "{}: more than two doubles outside the exact roots",
        what()
    );
    if a.lo() > 0.0 {
        prop_assert_eq!(r.certain_sign(), Some(Sign::Positive), "{}", what());
    }
    let zero = r.certain_sign() == Some(Sign::Zero);
    prop_assert_eq!(zero, a.hi() == 0.0, "{}: Zero exactly for [0, 0]", what());
    Ok(())
}

/// The root of the non-negative part of `a`, by its documented method: `NegativeSqrt` when every
/// value is negative, otherwise `sqrt` of [max(lo, 0), hi].
///
/// The crate's own operation is private since the third review round: on a caller's word that the
/// value is not negative it would certify a negative value as an exact zero, so only
/// `Interval::norm`, which proves its radicand non-negative, takes it. The crate's unit tests hold
/// it to this method bit for bit; here the method stands in for it, through the public `sqrt`, and
/// `norm` is checked against it.
fn sqrt_nonneg(a: Interval) -> Result<Interval, NumError> {
    if a.hi() < 0.0 {
        return Err(NumError::NegativeSqrt);
    }
    Interval::new(if a.lo() < 0.0 { 0.0 } else { a.lo() }, a.hi())?.sqrt()
}

/// `sqrt_nonneg`: `NegativeSqrt` exactly when every value is negative; otherwise bit for bit the
/// strict root of the non-negative part, which `check_sqrt` holds to the oracle.
fn check_sqrt_nonneg(a: Interval) -> TestResult {
    let got = sqrt_nonneg(a);
    if a.hi() < 0.0 {
        prop_assert_eq!(got, Err(NumError::NegativeSqrt), "sqrt_nonneg {:?}", a);
        return Ok(());
    }
    let part = iv(if a.lo() < 0.0 { 0.0 } else { a.lo() }, a.hi());
    prop_assert_eq!(
        bits(got),
        bits(part.sqrt()),
        "sqrt_nonneg {:?} is not sqrt of {:?}",
        a,
        part
    );
    check_sqrt(part)
}

/// Every operation of the module on one pair of operands.
fn check_all(a: Interval, b: Interval) -> TestResult {
    for op in Op::ALL {
        check_binary(op, a, b)?;
        check_binary(op, b, a)?;
    }
    check_square(a)?;
    check_sqrt(a)?;
    check_sqrt_nonneg(a)?;
    let n = a.neg();
    prop_assert_eq!(
        (n.lo().to_bits(), n.hi().to_bits()),
        (fp::canon(-a.hi()).to_bits(), fp::canon(-a.lo()).to_bits()),
        "neg {:?} = {:?}",
        a,
        n
    );
    Ok(())
}

mod oracle_checks {
    use super::{Dy, config, dy, fp, q, strat};
    use geom_num::math;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(config(4096))]

        /// Both rounding oracles give what the hardware gives for an exact sum, difference and
        /// product of two doubles, agree with each other on all three neighbours, and bracket the
        /// value with nothing between; the rational one does the same for quotients.
        #[test]
        fn brackets_match_the_hardware_and_each_other(a in strat::wild(), b in strat::wild()) {
            let (x, y) = (Dy::of(a), Dy::of(b));
            let dyadic = [(&x + &y, a + b), (&x - &y, a - b), (&x * &y, a * b)];
            for (v, hw) in dyadic {
                let (d, r) = (v.bracket(), q::bracket(&v.rational()));
                // Both zeros are the same rounded value; the oracle keeps the sign of the exact one.
                let (oracle, hardware) = (fp::canon(d.nearest).to_bits(), fp::canon(hw).to_bits());
                prop_assert!(oracle == hardware, "{}: {:?}, hardware {:e}", v, d, hw);
                let same = (d.floor.to_bits(), d.ceil.to_bits(), d.nearest.to_bits())
                    == (r.floor.to_bits(), r.ceil.to_bits(), r.nearest.to_bits());
                prop_assert!(same, "{}: dyadic {:?}, rational {:?}", v, d, r);
            }
            if b != 0.0 {
                let v = q::exact(a) / q::exact(b);
                let r = q::bracket(&v);
                let (oracle, hardware) = (fp::canon(r.nearest).to_bits(), fp::canon(a / b).to_bits());
                prop_assert!(oracle == hardware, "{} / {}: rational {:?}", a, b, r);
                prop_assert!(r.floor <= r.ceil, "{:?}", r);
                if r.floor.is_finite() {
                    prop_assert!(q::exact(r.floor) <= v, "floor {:?} of {}", r, v);
                }
                if r.ceil.is_finite() {
                    prop_assert!(v <= q::exact(r.ceil), "ceil {:?} of {}", r, v);
                }
                if r.floor.to_bits() != r.ceil.to_bits() {
                    let next = fp::canon(fp::up(r.floor)).to_bits();
                    prop_assert!(next == fp::canon(r.ceil).to_bits(), "a double inside {:?}", r);
                }
            }
        }

        /// The bisection root gives the hardware's correctly rounded root (`math::sqrt`).
        #[test]
        fn sqrt_bracket_matches_math_sqrt(
            x in prop_oneof![strat::any_finite(), strat::subnormal(), strat::wild()],
        ) {
            let x = x.abs();
            let r = dy::sqrt_bracket(x);
            prop_assert_eq!(math::sqrt(x).unwrap().get().to_bits(), r.nearest.to_bits(), "sqrt({:e})", x);
        }
    }

    proptest! {
        #![proptest_config(config(256))]

        /// Both floors against an independent bisection over every double.
        #[test]
        fn floors_match_bisection(a in strat::wild(), b in strat::wild()) {
            let (x, y) = (Dy::of(a), Dy::of(b));
            for v in [&x * &y, &x + &y, &x - &y] {
                let f = q::floor_by_bisection(&v.rational()).to_bits();
                prop_assert_eq!(fp::canon(v.bracket().floor).to_bits(), f, "floor of {}", v);
                let r = q::bracket(&v.rational()).floor;
                prop_assert_eq!(fp::canon(r).to_bits(), f, "floor of {}", v);
            }
        }
    }
}

mod every_binade {
    use super::{Op, check_binary, check_sqrt, check_sqrt_nonneg, check_square, fp, iv, pt, run};

    /// The doubles on both sides of 2^k: 2^k − one step, 2^k and 2^k + one step.
    fn around(k: i32) -> [f64; 3] {
        let p = fp::pow2(k);
        [fp::down(p), p, fp::up(p)]
    }

    /// Sums whose exact value is at or next to 2^k, for every k: 2^k and its neighbours plus or
    /// minus 2^(k − d) for d = 1, 53 (half a unit above 2^k, a tie), 54 (half a unit below, a tie),
    /// 55 and 106, and three times those, in both signs. Every rounded bound lands on, just below
    /// or just above a binade boundary, in the subnormal range and at 2^−1022 too.
    #[test]
    fn sums_at_every_binade_boundary() {
        for k in -1074..=1023 {
            for a in around(k) {
                for d in [1, 53, 54, 55, 106] {
                    let e = k - d;
                    if e < -1074 {
                        continue;
                    }
                    let small = fp::pow2(e);
                    for b in [small, -small, 3.0 * small] {
                        run(check_binary(Op::Add, pt(a), pt(b)));
                        run(check_binary(Op::Sub, pt(-a), pt(b)));
                    }
                }
            }
        }
    }

    /// Products and quotients of the doubles around 2^k by the doubles around 1 (and by 3/4 and
    /// 3/2): every result lands within a few units of a binade boundary, with residues far below a
    /// unit (products) or not dyadic at all (quotients), across underflow and overflow.
    #[test]
    fn products_and_quotients_at_every_binade_boundary() {
        let factors = [
            fp::down(1.0),
            fp::up(1.0),
            fp::steps(1.0, -2),
            fp::steps(1.0, 2),
            0.75,
            1.5,
        ];
        for k in -1074..=1023 {
            for a in around(k) {
                for f in factors {
                    run(check_binary(Op::Mul, pt(a), pt(f)));
                    run(check_binary(Op::Div, pt(a), pt(f)));
                    run(check_binary(Op::Div, pt(-a), pt(f)));
                }
            }
        }
    }

    /// `square` of the doubles around ±2^k for every k (underflow, the 2^−537 edge where squares
    /// start to be nonzero, the 2^512 edge where they overflow) and of intervals around zero.
    #[test]
    fn squares_at_every_binade() {
        for k in -1074..=1023 {
            for a in around(k) {
                run(check_square(pt(a)));
                run(check_square(pt(-a)));
                run(check_square(iv(-a, 0.5 * a)));
                run(check_square(iv(fp::down(a), a)));
            }
        }
    }

    /// `sqrt` and `sqrt_nonneg` of the doubles around 2^k for every k, of the interval they span,
    /// and of intervals reaching below zero.
    #[test]
    fn roots_at_every_binade() {
        for k in -1074..=1023 {
            let [below, at, above] = around(k);
            for a in [pt(below), pt(at), pt(above), iv(below, above)] {
                run(check_sqrt(a));
            }
            run(check_sqrt_nonneg(iv(-at, at)));
            run(check_sqrt_nonneg(iv(-at, -0.0)));
        }
    }
}

mod overflow_threshold {
    use super::{OMEGA, Op, check_binary, check_square, fp, iv, pt, run};
    use geom_num::NumError;
    use geom_num::interval::Interval;

    /// Sums at the overflow threshold. The double below Ω is Ω − 2^971 (even significand), so an
    /// exact value up to the midpoint Ω − 2^970 rounds down and can still be stepped to Ω, and
    /// anything above it rounds to Ω and must be `NotFinite`; from Ω + 2^970 (the tie, rounding to
    /// the even 2^1024) the rounded value itself is infinite. Both signs, every operation.
    #[test]
    fn sums_at_the_overflow_threshold() {
        let below = fp::down(OMEGA);
        let half = fp::pow2(970);
        let pairs = [
            (below, half),
            (below, fp::down(half)),
            (below, fp::up(half)),
            (OMEGA, -half),
            (OMEGA, -fp::down(half)),
            (OMEGA, -fp::up(half)),
            (OMEGA, half),
            (OMEGA, fp::down(half)),
            (OMEGA, 0.0),
            (below, 0.0),
            (OMEGA, -OMEGA),
            (fp::pow2(1023), fp::pow2(1023)),
            (fp::pow2(1023), below - fp::pow2(1023)),
        ];
        for (a, b) in pairs {
            for s in [1.0, -1.0] {
                for op in Op::ALL {
                    run(check_binary(op, pt(s * a), pt(s * b)));
                }
            }
        }
        // A valid operand whose rounded lower bound is −Ω: the operation must refuse although the
        // upper bound is finite, and likewise for an upper bound at −Ω or a lower bound at +Ω.
        assert_eq!(
            iv(-OMEGA, 0.0).add(pt(0.0)),
            Err(NumError::NotFinite),
            "[−Ω, 0] + 0"
        );
        assert_eq!(pt(-OMEGA).add(pt(0.0)), Err(NumError::NotFinite), "−Ω + 0");
        assert_eq!(pt(OMEGA).sub(pt(0.0)), Err(NumError::NotFinite), "Ω − 0");
        run(check_binary(Op::Add, iv(-OMEGA, 0.0), pt(0.0)));
        run(check_binary(Op::Mul, iv(-OMEGA, 1.0), pt(1.0)));
    }

    /// Products and quotients whose exact value is exactly Ω or Ω − 2^971, built by factoring:
    /// Ω = (2^53 − 1) · 2^971 with 2^53 − 1 = 6361 · 69431 · 20394401, and Ω − 2^971 =
    /// (2^52 − 1) · 2^972 with 2^52 − 1 = (3 · 5 · 53 · 157) · (1613 · 2731 · 8191).
    #[test]
    fn products_and_quotients_exactly_at_omega() {
        assert_eq!(6361u64 * 69431 * 20_394_401, (1u64 << 53) - 1);
        assert_eq!(
            (3u64 * 5 * 53 * 157) * (1613 * 2731 * 8191),
            (1u64 << 52) - 1
        );
        let a = f64::from(6361u32) * fp::pow2(500);
        let b = f64::from(69431u32) * f64::from(20_394_401u32) * fp::pow2(471);
        assert_eq!(a * b, OMEGA, "the factors of Ω");
        assert_eq!(
            pt(a).mul(pt(b)),
            Err(NumError::NotFinite),
            "a product of exactly Ω"
        );
        let c = f64::from(3u32 * 5 * 53 * 157) * fp::pow2(500);
        let d = f64::from(1613u32 * 2731) * f64::from(8191u32) * fp::pow2(472);
        assert_eq!(c * d, fp::down(OMEGA), "the factors of Ω − 2^971");
        let widened = pt(c).mul(pt(d)).map(Interval::hi);
        assert_eq!(widened, Ok(OMEGA), "Ω − 2^971 widens to Ω");
        for (x, y) in [
            (a, b),
            (c, d),
            (a, fp::up(b)),
            (c, fp::down(d)),
            (c, fp::up(d)),
        ] {
            for s in [1.0, -1.0] {
                run(check_binary(Op::Mul, pt(s * x), pt(y)));
                run(check_binary(Op::Div, pt(s * OMEGA), pt(y)));
                run(check_binary(Op::Div, pt(s * OMEGA), pt(x)));
            }
        }
        // Quotients that land exactly on Ω and on the double below it.
        assert_eq!(pt(OMEGA).div(pt(1.0)), Err(NumError::NotFinite));
        assert_eq!(
            pt(fp::down(OMEGA)).div(pt(1.0)).map(Interval::hi),
            Ok(OMEGA)
        );
        assert_eq!(pt(OMEGA / 4.0).div(pt(0.25)), Err(NumError::NotFinite));
        run(check_binary(Op::Div, pt(OMEGA / 4.0), pt(0.25)));
        run(check_binary(
            Op::Div,
            pt(fp::down(OMEGA) / 4.0),
            iv(0.25, 0.5),
        ));
    }

    /// Squares at the overflow edge: the largest operand whose square still widens to a finite
    /// bound, and its neighbours, for points and for intervals reaching it from either side.
    #[test]
    fn squares_at_the_overflow_threshold() {
        let edge = fp::down(fp::pow2(512));
        for j in -3..=0 {
            let x = fp::steps(edge, j);
            run(check_square(pt(x)));
            run(check_square(iv(-x, x)));
            run(check_square(iv(-x, 1.0)));
            run(check_square(iv(-1.0, x)));
        }
        let top = pt(edge).square().map(Interval::hi);
        assert_eq!(top, Ok(OMEGA), "(2^512 − 2^459)² widens to Ω");
        assert_eq!(pt(fp::pow2(512)).square(), Err(NumError::NotFinite));
        assert_eq!(iv(-fp::pow2(512), 0.0).square(), Err(NumError::NotFinite));
    }
}

mod zeros_and_signs {
    use super::{OMEGA, Op, TINY, bits, check_all, check_binary, fp, iv, pt, q, run};
    use geom_num::interval::Interval;
    use geom_num::{NumError, Sign};

    /// The bounds of the grid: ±Ω, ±1, ±2^−1074 and both zeros.
    const BOUNDS: [f64; 8] = [-OMEGA, -1.0, -TINY, -0.0, 0.0, TINY, 1.0, OMEGA];

    /// Every interval with both bounds in `BOUNDS`.
    fn grid() -> Vec<Interval> {
        let mut v = Vec::new();
        for (i, &lo) in BOUNDS.iter().enumerate() {
            for &hi in &BOUNDS[i..] {
                v.push(iv(lo, hi));
            }
        }
        v
    }

    /// Every operation on every pair from the grid: zeros stored as +0, signed zeros never change
    /// a result, every result follows its documented method.
    #[test]
    fn every_operation_on_the_zero_grid() {
        let grid = grid();
        for &a in &grid {
            for &b in &grid {
                run(check_all(a, b));
            }
        }
    }

    /// `new` treats −0 and +0 as the same real: `[+0, −0]` is not empty, and both orders store +0.
    /// Its errors: `NotFinite` before `EmptyInterval`, NaN and infinities refused.
    #[test]
    fn new_with_signed_zeros_and_non_finite_bounds() {
        for (lo, hi) in [(0.0, -0.0), (-0.0, 0.0), (-0.0, -0.0)] {
            let r = Interval::new(lo, hi);
            assert_eq!(bits(r), Ok((0, 0)), "new({lo:e}, {hi:e}) = {r:?}");
        }
        assert_eq!(bits(Interval::point(-0.0)), Ok((0, 0)));
        assert_eq!(
            Interval::new(f64::INFINITY, f64::NEG_INFINITY),
            Err(NumError::NotFinite)
        );
        assert_eq!(Interval::new(f64::NAN, 0.0), Err(NumError::NotFinite));
        assert_eq!(Interval::new(0.0, f64::NAN), Err(NumError::NotFinite));
        assert_eq!(Interval::new(1.0, f64::INFINITY), Err(NumError::NotFinite));
        assert_eq!(Interval::point(f64::NEG_INFINITY), Err(NumError::NotFinite));
        assert_eq!(Interval::new(TINY, 0.0), Err(NumError::EmptyInterval));
        assert_eq!(Interval::new(OMEGA, -OMEGA), Err(NumError::EmptyInterval));
    }

    /// `certain_sign` on every grid interval, against the exact bounds: `Some(s)` exactly when
    /// every real in it has sign s, `None` exactly when it holds zero and a nonzero real.
    #[test]
    fn certain_sign_is_exact_on_the_grid() {
        for r in grid() {
            let (l, h) = (q::sign(&q::exact(r.lo())), q::sign(&q::exact(r.hi())));
            let want = if l == h { Some(l) } else { None };
            assert_eq!(r.certain_sign(), want, "{r:?}");
        }
    }

    /// `contains` at the edges: both zeros, NaN, the infinities, the smallest subnormals.
    #[test]
    fn contains_at_the_edges() {
        let r = iv(0.0, TINY);
        assert!(
            r.contains(-0.0) && r.contains(0.0) && r.contains(TINY),
            "{r:?}"
        );
        assert!(
            !r.contains(-TINY) && !r.contains(f64::from_bits(2)),
            "{r:?}"
        );
        assert!(!r.contains(f64::NAN), "{r:?}");
        let all = iv(-OMEGA, OMEGA);
        assert!(
            !all.contains(f64::INFINITY) && !all.contains(f64::NEG_INFINITY),
            "{all:?}"
        );
        assert!(all.contains(-OMEGA) && all.contains(OMEGA), "{all:?}");
    }

    /// The documented example: 3 − 3 is [−2^−1074, 2^−1074], undecided, and so is every x − x,
    /// x + (−x), x · 0 and 0 / x. A computed result is never `Zero`: only `point`, `new`, `neg`,
    /// `sqrt` and `sqrt_nonneg` make [0, 0].
    #[test]
    fn computed_zeros_stay_undecided() {
        for x in [
            3.0,
            0.0,
            -0.0,
            TINY,
            -1.5e300,
            OMEGA,
            -OMEGA,
            fp::pow2(-1022),
        ] {
            let d = pt(x).sub(pt(x)).unwrap();
            let got = (d.lo(), d.hi(), d.certain_sign());
            assert_eq!(got, (-TINY, TINY, None), "{x:e} − {x:e} = {d:?}");
            let s = pt(x).add(pt(-x));
            assert_eq!(bits(s), bits(Ok(d)), "{x:e} + (−{x:e}) = {s:?}");
            let p = pt(x).mul(pt(0.0));
            assert_eq!(bits(p), bits(Ok(d)), "{x:e} · 0 = {p:?}");
            if x != 0.0 {
                let q0 = pt(0.0).div(pt(x));
                assert_eq!(bits(q0), bits(Ok(d)), "0 / {x:e} = {q0:?}");
            }
        }
        let z = pt(0.0).square().unwrap();
        let got = (z.lo().to_bits(), z.hi(), z.certain_sign());
        assert_eq!(got, (0, TINY, None), "0² = {z:?}");
        assert_eq!(pt(0.0).sqrt().unwrap().certain_sign(), Some(Sign::Zero));
    }

    /// Division by intervals touching zero, through every kind of bound: the error is
    /// `DivisorContainsZero` exactly when the divisor holds 0, and it comes before `NotFinite`.
    #[test]
    fn division_by_intervals_touching_zero() {
        let divisors = [
            iv(-0.0, -0.0),
            iv(-0.0, TINY),
            iv(-TINY, -0.0),
            iv(-TINY, TINY),
            iv(TINY, TINY),
            iv(-TINY, -TINY),
            iv(TINY, OMEGA),
            iv(-OMEGA, -TINY),
            iv(-OMEGA, OMEGA),
            iv(f64::from_bits(2), 1.0),
            iv(-1.0, -0.0),
            iv(0.0, 1.0),
        ];
        let dividends = [
            pt(0.0),
            pt(TINY),
            iv(-TINY, TINY),
            iv(-1.0, 1.0),
            pt(OMEGA),
            iv(-OMEGA, OMEGA),
            pt(fp::pow2(-1060)),
        ];
        for &a in &dividends {
            for &b in &divisors {
                run(check_binary(Op::Div, a, b));
            }
        }
        // A quotient that would overflow, by a divisor holding zero: the zero is reported first.
        assert_eq!(
            pt(OMEGA).div(iv(-TINY, TINY)),
            Err(NumError::DivisorContainsZero)
        );
        assert_eq!(
            pt(OMEGA).div(iv(0.0, TINY)),
            Err(NumError::DivisorContainsZero)
        );
        // The same quotient by a divisor one step away from zero overflows and is reported.
        assert_eq!(pt(OMEGA).div(pt(TINY)), Err(NumError::NotFinite));
        assert_eq!(pt(1.0).div(pt(-TINY)), Err(NumError::NotFinite));
        // 2^−1074 / 2^−1074 is exactly 1, and the widened quotient is decided positive.
        let r = pt(TINY).div(pt(TINY)).unwrap();
        let want = (fp::down(1.0), fp::up(1.0), Some(Sign::Positive));
        assert_eq!((r.lo(), r.hi(), r.certain_sign()), want);
    }
}

mod roots {
    use super::{Dy, TINY, bits, check_sqrt, check_sqrt_nonneg, config, dy, fp, iv, pt, q, run};
    use super::{TestResult, sqrt_nonneg, strat};
    use geom_num::interval::Interval;
    use geom_num::{NumError, Sign, math};
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use proptest::prelude::*;

    /// A fixed-seed generator (SplitMix64): the same cases on every run and target.
    struct SplitMix(u64);

    impl SplitMix {
        /// The next 64 pseudo-random bits.
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        /// A whole number in `lo ..= hi`.
        fn range(&mut self, lo: i32, hi: i32) -> i32 {
            let span = u64::try_from(i64::from(hi) - i64::from(lo) + 1).unwrap();
            let offset = i64::try_from(self.next() % span).unwrap();
            i32::try_from(i64::from(lo) + offset).unwrap()
        }
    }

    /// Hard cases for the square root: x is the double nearest the square of a midpoint M between
    /// two adjacent doubles, so √x lies within a tiny fraction of a unit of M, where a root that
    /// is not correctly rounded (or not even faithful) goes wrong. Every exponent of the root from
    /// 2^−537 (subnormal x) to 2^511, and x's neighbours. `math::sqrt` must be the correctly
    /// rounded root, and the interval must hold it, stepped once.
    #[test]
    fn sqrt_hard_cases_next_to_midpoints() {
        let mut rng = SplitMix(0x5EED_C0FF_EE00);
        for _ in 0..1500 {
            let exponent = rng.range(-537, 511);
            let y = strat::make(false, exponent, rng.next());
            let mid = (&Dy::of(y) + &Dy::of(fp::up(y))).half();
            let near = (&mid * &mid).bracket().nearest;
            for x in [fp::down(near), near, fp::up(near)] {
                if !(x.is_finite() && x >= 0.0) {
                    continue;
                }
                let want = dy::sqrt_bracket(x).nearest;
                let got = math::sqrt(x).unwrap().get();
                let msg = format!("math::sqrt({x:e}) = {got:e}, correctly rounded {want:e}");
                assert_eq!(got.to_bits(), want.to_bits(), "{msg}");
                run(check_sqrt(pt(x)));
                run(check_sqrt(iv(x, fp::up(x))));
            }
        }
    }

    /// Exact roots: x = y² for a double y with at most 26 significant bits is a double, its root
    /// is exact, and the interval is still stepped: [y − one step, y + one step].
    #[test]
    fn exact_roots_are_still_widened() {
        let mut rng = SplitMix(42);
        for _ in 0..600 {
            let exponent = rng.range(-511, 511);
            let root = strat::make(false, exponent, rng.next() & !((1u64 << 27) - 1));
            let square = root * root;
            let exact = Dy::of(square) == &Dy::of(root) * &Dy::of(root);
            assert!(exact, "{root:e}² is not exact");
            let got = pt(square).sqrt().unwrap();
            assert_eq!(
                (got.lo(), got.hi()),
                (fp::down(root), fp::up(root)),
                "sqrt {square:e} = {got:?}"
            );
            run(check_sqrt(pt(square)));
        }
    }

    /// Subnormal operands: every root is normal (at least 2^−537), the lower bound positive.
    #[test]
    fn sqrt_of_subnormals() {
        for k in 0..52 {
            for x in [
                f64::from_bits(1u64 << k),
                f64::from_bits((1u64 << (k + 1)) - 1),
            ] {
                run(check_sqrt(pt(x)));
                run(check_sqrt(iv(0.0, x)));
                run(check_sqrt(iv(x, f64::MIN_POSITIVE)));
                run(check_sqrt_nonneg(iv(-x, x)));
            }
        }
        let r = pt(TINY).sqrt().unwrap();
        assert!(
            r.lo() > 0.0 && r.contains(fp::pow2(-537)),
            "sqrt 2^−1074 = {r:?}"
        );
    }

    /// The square of an interval, then its root: `sqrt_nonneg(square(a))` holds |x| for both
    /// endpoints of `a`, even when `a` holds 0 and its square underflows; a square is never
    /// negative, so the strict root agrees bit for bit.
    fn check_root_of_square(a: Interval) -> TestResult {
        let Ok(s) = a.square() else { return Ok(()) };
        let r = sqrt_nonneg(s);
        prop_assert!(r.is_ok(), "sqrt_nonneg(square {:?} = {:?}) = {:?}", a, s, r);
        let r = r.unwrap();
        prop_assert!(r.lo() >= 0.0, "{:?}", r);
        for x in [a.lo(), a.hi()] {
            prop_assert!(
                r.contains(x.abs()),
                "√(square {:?}) = {:?} misses |{:e}|",
                a,
                r,
                x
            );
        }
        prop_assert_eq!(bits(s.sqrt()), bits(Ok(r)), "square {:?} = {:?}", a, s);
        Ok(())
    }

    proptest! {
        #![proptest_config(config(2048))]

        /// `sqrt_nonneg` holds the root of every non-negative double inside its operand (interior
        /// values chosen by bit pattern), and never refuses an operand reaching 0 or above.
        #[test]
        fn sqrt_nonneg_holds_the_root_of_every_non_negative_value(
            a in strat::interval(),
            t in 0u64..=64,
        ) {
            check_sqrt_nonneg(a)?;
            let Ok(r) = sqrt_nonneg(a) else {
                prop_assert!(a.hi() < 0.0, "sqrt_nonneg {:?} refused", a);
                return Ok(());
            };
            let lo = if a.lo() < 0.0 { 0.0 } else { a.lo() };
            let (lb, hb) = (lo.to_bits(), a.hi().to_bits());
            let x = f64::from_bits(lb + (hb - lb) / 64 * t);
            prop_assert!(lo <= x && x <= a.hi(), "{:e} outside {:?}", x, a);
            let (rl, rh, xv) = (Dy::of(r.lo()), Dy::of(r.hi()), Dy::of(x));
            let inside = &rl * &rl <= xv && xv <= &rh * &rh;
            prop_assert!(inside, "sqrt_nonneg {:?} = {:?} misses √{:e}", a, r, x);
        }

        /// `sqrt_nonneg(square(a))` holds |x| for both endpoints of every kind of interval.
        #[test]
        fn root_of_a_square_holds_the_magnitude(a in strat::interval()) {
            check_root_of_square(a)?;
        }
    }

    /// The design gap of the first round: the distance between coincident points. In 3D, at every
    /// third scale from 2^−1074 to 2^1023 and at grid coordinates (±(2^40 − 1) steps, and 25 times
    /// that, the same span at the finer step), through `square` and through `mul`: the squared
    /// distance reaches below zero only through widening, the strict root refuses it, and
    /// `sqrt_nonneg` certifies a distance that starts at exactly +0, holds 0, stays undecided and
    /// stays below 2^−535 (√(5 · 2^−1074) stepped once).
    #[test]
    fn coincident_points_in_3d_at_every_scale() {
        let grid = fp::pow2(40) - 1.0;
        let mut coords = vec![0.0, grid, 25.0 * grid];
        for k in (-1074..=1023).step_by(3) {
            coords.push(fp::pow2(k));
            coords.push(fp::steps(fp::pow2(k), -1));
        }
        let negated: Vec<f64> = coords.iter().map(|x| -x).collect();
        coords.extend(negated);
        let count = coords.len();
        for (i, &x) in coords.iter().enumerate() {
            let (y, z) = (coords[(i * 7 + 3) % count], coords[(i * 13 + 5) % count]);
            let diffs: Vec<Interval> = [x, y, z]
                .iter()
                .map(|&c| pt(c).sub(pt(c)).unwrap())
                .collect();
            let by_square = diffs[0]
                .square()
                .and_then(|s| s.add(diffs[1].square()?))
                .and_then(|s| s.add(diffs[2].square()?))
                .unwrap();
            let by_mul = diffs[0]
                .mul(diffs[0])
                .and_then(|s| s.add(diffs[1].mul(diffs[1])?))
                .and_then(|s| s.add(diffs[2].mul(diffs[2])?))
                .unwrap();
            for d2 in [by_square, by_mul] {
                let at = format!("({x:e}, {y:e}, {z:e}): d² = {d2:?}");
                assert!(d2.lo() < 0.0 && d2.contains(0.0), "{at}");
                assert_eq!(
                    d2.sqrt(),
                    Err(NumError::NegativeSqrt),
                    "the strict root, {at}"
                );
                run(check_sqrt_nonneg(d2));
                let root = sqrt_nonneg(d2).unwrap();
                assert!(
                    root.lo().to_bits() == 0
                        && root.contains(0.0)
                        && root.hi() < fp::pow2(-535)
                        && root.certain_sign().is_none(),
                    "distance between coincident points {at}: sqrt_nonneg = {root:?}"
                );
            }
        }
    }

    /// Points one grid step apart along one, two or three axes, anywhere in a frame of ±(2^40 − 1)
    /// steps: the exact distance is 1, √2 or √3 steps, and the certified interval holds it (by
    /// squaring) with a positive lower bound, through `sqrt_nonneg` and the strict root alike.
    #[test]
    fn points_one_step_apart_have_a_positive_certified_distance() {
        let max = fp::pow2(40) - 1.0;
        let mut g = SplitMix(7);
        for i in 0..400 {
            let c: Vec<f64> = (0..3)
                .map(|_| f64::from(g.range(-(1 << 29), 1 << 29)) * 2048.0)
                .map(|v| v.clamp(-max, max - 1.0))
                .collect();
            let axes = 1 + i % 3;
            let mut d2 = pt(0.0);
            for (j, &cj) in c.iter().enumerate() {
                let other = if j < axes { cj + 1.0 } else { cj };
                let dj = pt(other).sub(pt(cj)).unwrap();
                d2 = d2.add(dj.square().unwrap()).unwrap();
            }
            let want = BigRational::from_integer(BigInt::from(axes));
            for r in [sqrt_nonneg(d2).unwrap(), d2.sqrt().unwrap()] {
                let (rl, rh) = (q::exact(r.lo()), q::exact(r.hi()));
                assert!(
                    &rl * &rl <= want && want <= &rh * &rh,
                    "distance √{axes} from {c:?}: d² = {d2:?}, root {r:?}"
                );
                assert_eq!(r.certain_sign(), Some(Sign::Positive), "{c:?}: {r:?}");
            }
        }
    }
}

mod laws {
    use super::{Op, TestResult, bits, check_all, config, sqrt_nonneg, strat};
    use geom_num::NumError;
    use geom_num::interval::Interval;
    use proptest::prelude::*;

    /// An operation with one operand.
    type Unary = fn(Interval) -> Result<Interval, NumError>;

    /// Whether `inner ⊆ outer`.
    fn inside(inner: Interval, outer: Interval) -> bool {
        outer.lo() <= inner.lo() && inner.hi() <= outer.hi()
    }

    /// A narrower operand never gives a wider result, and never an error the wider operand does
    /// not give: each operation is monotone for inclusion, errors included.
    fn check_isotone(a: Interval, a2: Interval, b: Interval, b2: Interval) -> TestResult {
        for op in Op::ALL {
            let (small, big) = (op.apply(a, b), op.apply(a2, b2));
            let what =
                || format!("{op:?}: {a:?}, {b:?} gave {small:?}; {a2:?}, {b2:?} gave {big:?}");
            match (small, big) {
                (Ok(s), Ok(g)) => prop_assert!(inside(s, g), "not inside, {}", what()),
                (Err(_), Ok(_)) => prop_assert!(false, "only the narrower refused, {}", what()),
                (Err(NumError::DivisorContainsZero), big) => prop_assert_eq!(
                    big,
                    Err(NumError::DivisorContainsZero),
                    "the narrower divisor holds 0, {}",
                    what()
                ),
                _ => {}
            }
        }
        let unary: [(&str, Unary); 3] = [
            ("square", Interval::square),
            ("sqrt", Interval::sqrt),
            ("sqrt_nonneg", sqrt_nonneg),
        ];
        for (name, f) in unary {
            let (small, big) = (f(a), f(a2));
            let what = || format!("{name}: {a:?} gave {small:?}; {a2:?} gave {big:?}");
            match (small, big) {
                (Ok(s), Ok(g)) => prop_assert!(inside(s, g), "not inside, {}", what()),
                // Only `sqrt_nonneg` may refuse the narrower operand (wholly negative) alone.
                (Err(_), Ok(_)) if name != "sqrt_nonneg" => {
                    prop_assert!(false, "only the narrower refused, {}", what());
                }
                (Ok(_), Err(_)) if name == "sqrt_nonneg" => {
                    prop_assert!(false, "only the wider refused, {}", what());
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Laws that hold bit for bit, because rounding to nearest is symmetric and IEEE addition and
    /// multiplication commute: equal intervals must have equal bits whatever their history.
    fn check_laws(a: Interval, b: Interval) -> TestResult {
        let neg = |r: Result<Interval, NumError>| r.map(Interval::neg);
        let pairs = [
            ("a + b = b + a", a.add(b), b.add(a)),
            ("a · b = b · a", a.mul(b), b.mul(a)),
            ("a − b = a + (−b)", a.sub(b), a.add(b.neg())),
            ("a − b = −(b − a)", a.sub(b), neg(b.sub(a))),
            ("a · (−b) = −(a · b)", a.mul(b.neg()), neg(a.mul(b))),
            ("(−a) · b = −(a · b)", a.neg().mul(b), neg(a.mul(b))),
            ("(−a) · (−b) = a · b", a.neg().mul(b.neg()), a.mul(b)),
            ("a / (−b) = −(a / b)", a.div(b.neg()), neg(a.div(b))),
            ("(−a) / b = −(a / b)", a.neg().div(b), neg(a.div(b))),
            ("(−a)² = a²", a.neg().square(), a.square()),
            ("−(−a) = a", Ok(a.neg().neg()), Ok(a)),
        ];
        for (law, x, y) in pairs {
            prop_assert_eq!(
                bits(x),
                bits(y),
                "{} fails for a = {:?}, b = {:?}: {:?} against {:?}",
                law,
                a,
                b,
                x,
                y
            );
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(config(4096))]

        /// Every operation on every regime of operands, against the oracle.
        #[test]
        fn every_operation_against_the_oracle((a, b) in strat::pair()) {
            check_all(a, b)?;
        }

        /// Inclusion monotonicity of every operation, errors included.
        #[test]
        fn every_operation_is_monotone_for_inclusion(
            (a, a2) in strat::nested(),
            (b, b2) in strat::nested(),
        ) {
            check_isotone(a, a2, b, b2)?;
        }

        /// Commutativity and sign symmetry, bit for bit.
        #[test]
        fn laws_hold_bit_for_bit((a, b) in strat::pair()) {
            check_laws(a, b)?;
        }
    }
}

mod eft_agreement {
    use super::{OMEGA, TestResult, bits, config, fp, holds, pt, q, run, sign_is_sound, strat};
    use geom_num::interval::Interval;
    use geom_num::{NumError, Sign, eft};
    use num_rational::BigRational;
    use proptest::prelude::*;

    /// The interval of `a ∘ b` for two points against an error-free pair `(r, e)` of the same
    /// operation: if `r` is ±Ω or overflows, `NotFinite`; otherwise exactly `[r − one step, r +
    /// one step]`, and the exact value lies on the side of `r` that the sign of `e` names (or is
    /// `r` when `e` is 0). An infinite `e` (no exact pair exists) still carries the sign of the
    /// exact error, so it must still pick the right half. The interval of the result minus `r`
    /// holds the exact error with a sound sign.
    fn check_pair(
        what: &str,
        got: Result<Interval, NumError>,
        exact: &BigRational,
        (r, e): (f64, f64),
    ) -> TestResult {
        if !r.is_finite() || r.abs() == OMEGA {
            prop_assert_eq!(got, Err(NumError::NotFinite), "{}: rounded {:e}", what, r);
            return Ok(());
        }
        let want = (
            fp::canon(fp::down(r)).to_bits(),
            fp::canon(fp::up(r)).to_bits(),
        );
        prop_assert_eq!(
            bits(got),
            Ok(want),
            "{}: {:?} is not {:e} stepped",
            what,
            got,
            r
        );
        let got = got.unwrap();
        let rv = q::exact(r);
        let error = exact - &rv;
        let side = if e.is_finite() {
            prop_assert_eq!(
                &q::exact(e),
                &error,
                "{}: ({:e}, {:e}) is not exact",
                what,
                r,
                e
            );
            q::sign(&error)
        } else {
            let s = if e > 0.0 {
                Sign::Positive
            } else {
                Sign::Negative
            };
            prop_assert_eq!(
                s,
                q::sign(&error),
                "{}: error {:e}, exact {}",
                what,
                e,
                error
            );
            s
        };
        let ok = match side {
            Sign::Positive => rv < *exact && holds(got, exact),
            Sign::Negative => *exact < rv && holds(got, exact),
            Sign::Zero => *exact == rv,
        };
        prop_assert!(
            ok,
            "{}: error {:e} says {:?} of {:e}; exact {} in {:?}",
            what,
            e,
            side,
            r,
            exact,
            got
        );
        if let Ok(d) = got.sub(pt(r)) {
            prop_assert!(
                holds(d, &error),
                "{}: {:?} − {:e} = {:?} misses {}",
                what,
                got,
                r,
                d,
                error
            );
            sign_is_sound(d, &[&error])?;
        }
        Ok(())
    }

    /// Sum, difference and product of two doubles against `two_sum` and `two_prod`.
    fn check_eft(a: f64, b: f64) -> TestResult {
        let (x, y) = (q::exact(a), q::exact(b));
        let sum = pt(a).add(pt(b));
        check_pair(
            &format!("{a:e} + {b:e}"),
            sum,
            &(&x + &y),
            eft::two_sum(a, b),
        )?;
        let difference = pt(a).sub(pt(b));
        check_pair(
            &format!("{a:e} − {b:e}"),
            difference,
            &(&x - &y),
            eft::two_sum(a, -b),
        )?;
        let product = pt(a).mul(pt(b));
        check_pair(
            &format!("{a:e} · {b:e}"),
            product,
            &(&x * &y),
            eft::two_prod(a, b),
        )
    }

    proptest! {
        #![proptest_config(config(4096))]

        /// Random doubles of every kind.
        #[test]
        fn intervals_agree_with_the_error_free_pairs(a in strat::wild(), b in strat::wild()) {
            check_eft(a, b)?;
        }

        /// Products near and below the underflow threshold, where `two_prod`'s error may be finer
        /// than 2^−1074 and is then reported as a signed infinity.
        #[test]
        fn intervals_agree_with_two_prod_below_the_underflow_threshold(
            a in strat::normal(-600..=-400),
            b in strat::normal(-680..=-420),
        ) {
            check_eft(a, b)?;
        }
    }

    /// The overflow frontier: the 32 largest doubles of each sign against small values around
    /// half a unit of Ω (2^970), where the rounded sum flips between finite, Ω and infinity.
    #[test]
    fn intervals_agree_with_two_sum_at_the_overflow_frontier() {
        let mut smalls = vec![3.0 * fp::pow2(970)];
        for k in [969, 970, 971, 972] {
            for j in -2..=2 {
                smalls.push(fp::steps(fp::pow2(k), j));
            }
        }
        for i in 0..32 {
            let top = fp::steps(OMEGA, -i);
            for &s in &smalls {
                for (a, b) in [
                    (top, s),
                    (top, -s),
                    (-top, s),
                    (-top, -s),
                    (s, top),
                    (-s, -top),
                ] {
                    run(check_eft(a, b));
                }
            }
        }
    }
}

mod expression_trees {
    use super::sqrt_nonneg;
    use super::{Op, check_binary, check_square, config, fp, holds, q, sign_is_sound, strat};
    use geom_num::interval::Interval;
    use num_rational::BigRational;
    use num_traits::{Signed, Zero};
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(config(768))]

        /// Leaves a few steps wide (the output of earlier certified steps), a point chosen inside
        /// each; nodes made by every operation, `square` and `neg`, sharing operands freely. At
        /// every node the interval holds the exact value of the expression at the chosen points
        /// (the inclusion property for any reals in the operands), with a sound sign, and each
        /// node follows its documented method. A final root, strict or of the non-negative part,
        /// holds the exact root, and the two agree whenever the strict one succeeds.
        #[test]
        fn every_node_holds_the_exact_value(
            leaves in proptest::collection::vec((strat::moderate(), 0i64..4, 0i64..4), 3..6),
            nodes in proptest::collection::vec((0usize..6, any::<usize>(), any::<usize>()), 1..10),
        ) {
            let mut values: Vec<(Interval, BigRational)> = Vec::new();
            for &(x, width, pick) in &leaves {
                let r = strat::hull(x, fp::steps(x, width));
                let p = fp::steps(x, pick.min(width));
                prop_assert!(r.contains(p), "{:e} outside {:?}", p, r);
                values.push((r, q::exact(p)));
            }
            for &(op, i, j) in &nodes {
                let (a, va) = values[i % values.len()].clone();
                let (b, vb) = values[j % values.len()].clone();
                let next = match op {
                    0..=3 => {
                        let op = Op::ALL[op];
                        check_binary(op, a, b)?;
                        let r = op.apply(a, b);
                        let zero_divisor = op == Op::Div && vb.is_zero();
                        prop_assert!(!(zero_divisor && r.is_ok()), "{:?} / {:?} = {:?}", a, b, r);
                        r.ok().map(|r| (r, op.exact(&va, &vb)))
                    }
                    4 => {
                        check_square(a)?;
                        a.square().ok().map(|r| (r, &va * &va))
                    }
                    _ => Some((a.neg(), -va)),
                };
                if let Some((r, v)) = next {
                    prop_assert!(holds(r, &v), "{:?} misses the exact value {}", r, v);
                    sign_is_sound(r, &[&v])?;
                    values.push((r, v));
                }
            }
            let (last, v) = values.last().unwrap().clone();
            if last.sqrt().is_ok() {
                prop_assert!(!v.is_negative(), "sqrt {:?} succeeded on the value {}", last, v);
            }
            if !v.is_negative() {
                let root = sqrt_nonneg(last);
                prop_assert!(root.is_ok(), "sqrt_nonneg {:?} refused the value {}", last, v);
                let root = root.unwrap();
                let (rl, rh) = (q::exact(root.lo()), q::exact(root.hi()));
                let inside = &rl * &rl <= v && v <= &rh * &rh;
                prop_assert!(inside, "sqrt_nonneg {:?} = {:?} misses √{}", last, root, v);
                if let Ok(strict) = last.sqrt() {
                    prop_assert_eq!(strict, root, "sqrt and sqrt_nonneg differ on {:?}", last);
                }
            }
        }
    }
}

mod tightness_claim {
    use super::{fp, pt, q};
    use num_bigint::BigInt;
    use num_rational::BigRational;

    /// Sums whose exact value is next to a power of two in a spread of binades, both signs: ±2^k
    /// plus or minus a quarter, a half (the ties), three quarters and one and a half units, and
    /// the module documentation's examples.
    fn cases() -> Vec<(f64, f64)> {
        let mut v = vec![
            (1.0, -fp::pow2(-55)),
            (1.0, fp::pow2(-53)),
            (1.0, -fp::pow2(-54)),
        ];
        for k in (-1000..=1000).step_by(50) {
            let p = fp::pow2(k);
            for d in [53, 54, 55, 56] {
                let s = fp::pow2(k - d);
                v.extend([(p, s), (p, -s), (p, 3.0 * s), (p, -3.0 * s)]);
            }
        }
        let negated: Vec<(f64, f64)> = v.iter().map(|&(a, b)| (-a, -b)).collect();
        v.extend(negated);
        v
    }

    /// One computed bound of a sum, and how far outside the exact sum it is.
    struct Bound {
        /// The first summand.
        a: f64,
        /// The second summand.
        b: f64,
        /// Which bound: "lower" or "upper".
        which: &'static str,
        /// The exact sum.
        exact: BigRational,
        /// The computed bound.
        computed: f64,
        /// The distance from the exact sum out to the computed bound.
        gap: BigRational,
    }

    impl Bound {
        /// The gap in steps of `step`, exactly.
        fn steps(&self, step: &BigRational) -> BigRational {
            &self.gap / step
        }

        /// The step at the exact bound: the spacing of the doubles in its binade.
        fn exact_step(&self) -> BigRational {
            q::ulp(&self.exact)
        }

        /// The step at the computed bound.
        fn computed_step(&self) -> BigRational {
            q::ulp(&q::exact(self.computed))
        }

        /// Where the bound comes from, for a failure message.
        fn describe(&self) -> String {
            format!(
                "{:e} + {:e} = {} exactly; the {} bound {:e}",
                self.a, self.b, self.exact, self.which, self.computed
            )
        }
    }

    /// Both computed bounds of every case.
    fn bounds() -> Vec<Bound> {
        let mut out = Vec::new();
        for (a, b) in cases() {
            let r = pt(a).add(pt(b)).unwrap();
            let x = q::exact(a) + q::exact(b);
            out.push(Bound {
                a,
                b,
                which: "lower",
                computed: r.lo(),
                gap: &x - q::exact(r.lo()),
                exact: x.clone(),
            });
            out.push(Bound {
                a,
                b,
                which: "upper",
                computed: r.hi(),
                gap: q::exact(r.hi()) - &x,
                exact: x,
            });
        }
        out
    }

    /// The bound named `which` of the sum `a + b`, which must be one of the cases.
    fn find(a: f64, b: f64, which: &str) -> Bound {
        bounds()
            .into_iter()
            .find(|x| {
                x.a.to_bits() == a.to_bits() && x.b.to_bits() == b.to_bits() && x.which == which
            })
            .unwrap()
    }

    /// `n / d` exactly.
    fn ratio(n: i64, d: i64) -> BigRational {
        BigRational::new(BigInt::from(n), BigInt::from(d))
    }

    /// The module's statement of tightness: each computed bound is the first or second double
    /// beyond its exact bound. Strictly beyond (every rounded bound is stepped, even an exact
    /// one), and never past the double after the first one beyond.
    #[test]
    fn each_bound_is_the_first_or_second_double_beyond() {
        for b in bounds() {
            let br = q::bracket(&b.exact);
            let first = if b.which == "lower" {
                if q::exact(br.floor) == b.exact {
                    fp::down(br.floor)
                } else {
                    br.floor
                }
            } else if q::exact(br.ceil) == b.exact {
                fp::up(br.ceil)
            } else {
                br.ceil
            };
            let second = if b.which == "lower" {
                fp::down(first)
            } else {
                fp::up(first)
            };
            // Bits, with zero as +0, the module's one zero.
            let is = |x: f64| b.computed.to_bits() == fp::canon(x).to_bits();
            assert!(
                is(first) || is(second),
                "{}: neither the first double beyond, {first:e}, nor the second, {second:e}",
                b.describe()
            );
        }
    }

    /// In steps of the exact bound, at most 2.5, and the documentation's example reaches it:
    /// 1 + (−2^−54) = 1 − 2^−54 is a tie that rounds to the even 1, and its upper bound 1 + 2^−52
    /// is 2.5 steps of 2^−53 (the spacing just below 1) above it. The first form of this test
    /// asserted fewer than 2 such steps, the documentation's earlier wording, which this example
    /// broke; the wording was restated, and this test now holds the restated maximum.
    #[test]
    fn at_most_two_and_a_half_steps_of_the_exact_bound() {
        for b in bounds() {
            let n = b.steps(&b.exact_step());
            assert!(
                n <= ratio(5, 2),
                "{} is {n} steps of {} outside it",
                b.describe(),
                b.exact_step()
            );
        }
        let e = find(1.0, -fp::pow2(-54), "upper");
        assert_eq!(e.computed, fp::up(1.0), "{}", e.describe());
        assert_eq!(e.exact_step(), q::pow2(-53), "{}", e.describe());
        assert_eq!(e.steps(&e.exact_step()), ratio(5, 2), "{}", e.describe());
    }

    /// In steps of the computed bound, at most 2, and the documentation's example reaches it:
    /// 1 + 2^−53 is a tie that rounds to the even 1, and its lower bound 1 − 2^−53 is exactly 2
    /// steps of 2^−53 (the spacing at that bound) below it. The first form of this test asserted
    /// fewer than 2, which this example broke; the documentation now states "at most 2".
    #[test]
    fn at_most_two_steps_of_the_computed_bound() {
        for b in bounds() {
            let n = b.steps(&b.computed_step());
            assert!(
                n <= ratio(2, 1),
                "{} is {n} steps of {} outside it",
                b.describe(),
                b.computed_step()
            );
        }
        let e = find(1.0, fp::pow2(-53), "lower");
        assert_eq!(e.computed, fp::down(1.0), "{}", e.describe());
        assert_eq!(e.computed_step(), q::pow2(-53), "{}", e.describe());
        assert_eq!(e.steps(&e.computed_step()), ratio(2, 1), "{}", e.describe());
    }

    /// In steps of the larger of the two spacings, always fewer than 2.
    #[test]
    fn less_than_two_steps_of_the_larger_spacing() {
        for b in bounds() {
            let step = std::cmp::max(b.exact_step(), b.computed_step());
            let n = b.steps(&step);
            assert!(
                n < ratio(2, 1),
                "{} is {n} steps of {step} outside it",
                b.describe()
            );
        }
    }

    /// When the exact bound and the computed bound have the same step, at most 1.5 of it: half a
    /// step of rounding and one of widening. Checked on enough bounds that it is not vacuous.
    #[test]
    fn at_most_one_and_a_half_steps_when_the_steps_agree() {
        let mut checked = 0;
        for b in bounds() {
            let step = b.exact_step();
            if step == b.computed_step() {
                checked += 1;
                let n = b.steps(&step);
                assert!(
                    n <= ratio(3, 2),
                    "{} is {n} steps of {step} outside it",
                    b.describe()
                );
            }
        }
        assert!(
            checked >= 100,
            "only {checked} bounds share a step with the exact bound"
        );
    }
}

mod norm {
    use super::sqrt_nonneg;
    use super::{Dy, Op, TINY, TestResult, bits, check_binary, check_sqrt_nonneg, check_square};
    use super::{config, fp, no_minus_zero, pt, q, run, sign_is_sound, strat};
    use geom_num::interval::Interval;
    use geom_num::{NumError, Sign};
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use proptest::prelude::*;

    /// `norm` by its documented method, from the module's other operations: [0, 0] for no
    /// components, otherwise the first square, the other squares added in order, and the root of
    /// the non-negative part.
    fn by_method(cs: &[Interval]) -> Result<Interval, NumError> {
        let Some((first, rest)) = cs.split_first() else {
            return Interval::new(0.0, 0.0);
        };
        let mut sum = first.square()?;
        for c in rest {
            sum = sum.add(c.square()?)?;
        }
        sqrt_nonneg(sum)
    }

    /// Each step of `norm` against its own oracle: every square, every partial sum and the root,
    /// as far as the steps stay finite.
    fn check_steps(cs: &[Interval]) -> TestResult {
        let mut sum: Option<Interval> = None;
        for &c in cs {
            check_square(c)?;
            let Ok(c2) = c.square() else { return Ok(()) };
            sum = Some(match sum {
                None => c2,
                Some(s) => {
                    check_binary(Op::Add, s, c2)?;
                    let Ok(next) = s.add(c2) else { return Ok(()) };
                    next
                }
            });
        }
        if let Some(s) = sum {
            check_sqrt_nonneg(s)?;
        }
        Ok(())
    }

    /// `norm` against its documentation: bit for bit its method, each step against its oracle,
    /// never `NegativeSqrt`; and, independently of the bits, the whole exact range of lengths
    /// inside (the least and greatest sums of squares over the box of components, held by squaring
    /// the bounds), a lower bound never negative and never −0, a sound sign, and `Zero` only for
    /// no components.
    fn check_norm(cs: &[Interval]) -> TestResult {
        let got = Interval::norm(cs);
        prop_assert_eq!(bits(got), bits(by_method(cs)), "norm {:?}", cs);
        prop_assert_ne!(got, Err(NumError::NegativeSqrt), "norm {:?}", cs);
        check_steps(cs)?;
        let Ok(r) = got else {
            return Ok(());
        };
        let (mut least, mut most) = (Dy::of(0.0), Dy::of(0.0));
        for c in cs {
            let (l, h) = (Dy::of(c.lo()), Dy::of(c.hi()));
            let (l2, h2) = (&l * &l, &h * &h);
            let (small, large) = if l2 < h2 { (l2, h2) } else { (h2, l2) };
            if !c.contains(0.0) {
                least = &least + &small;
            }
            most = &most + &large;
        }
        let (rl, rh) = (Dy::of(r.lo()), Dy::of(r.hi()));
        prop_assert!(
            &rl * &rl <= least && most <= &rh * &rh,
            "norm {:?} = {:?} misses [√{}, √{}]",
            cs,
            r,
            least,
            most
        );
        prop_assert!(r.lo() >= 0.0 && no_minus_zero(r), "norm {:?} = {:?}", cs, r);
        sign_is_sound(r, &[&least, &most])?;
        prop_assert_eq!(
            r.certain_sign() == Some(Sign::Zero),
            cs.is_empty(),
            "norm {:?} = {:?}: Zero exactly for no components",
            cs,
            r
        );
        Ok(())
    }

    proptest! {
        #![proptest_config(config(1024))]

        /// `norm` against its documentation on up to four components of every kind.
        #[test]
        fn norm_follows_its_documented_method(
            cs in proptest::collection::vec(
                prop_oneof![strat::interval(), strat::interval_of(strat::moderate())],
                0..5,
            ),
        ) {
            check_norm(&cs)?;
        }
    }

    /// No components: exactly [0, 0], the one case whose sign is `Zero`.
    #[test]
    fn no_components_is_exactly_zero() {
        run(check_norm(&[]));
        let r = Interval::norm(&[]).unwrap();
        assert_eq!((r.lo().to_bits(), r.hi().to_bits()), (0, 0), "{r:?}");
        assert_eq!(r.certain_sign(), Some(Sign::Zero), "{r:?}");
    }

    /// The distance between coincident points in one, two and three dimensions, at every third
    /// scale from 2^−1074 to 2^1023 and at grid coordinates (±(2^40 − 1) steps, and 25 times that):
    /// `norm` of the differences never refuses, starts at exactly +0, holds 0, has no certain sign
    /// and stays below 2^−535 (√(5 · 2^−1074) stepped once).
    #[test]
    fn coincident_points_at_every_scale() {
        let grid = fp::pow2(40) - 1.0;
        let mut coords = vec![0.0, grid, 25.0 * grid];
        for k in (-1074..=1023).step_by(3) {
            coords.push(fp::pow2(k));
            coords.push(fp::steps(fp::pow2(k), -1));
        }
        let negated: Vec<f64> = coords.iter().map(|x| -x).collect();
        coords.extend(negated);
        let count = coords.len();
        for (i, &x) in coords.iter().enumerate() {
            let (y, z) = (coords[(i * 7 + 3) % count], coords[(i * 13 + 5) % count]);
            let diffs: Vec<Interval> = [x, y, z]
                .iter()
                .map(|&c| pt(c).sub(pt(c)).unwrap())
                .collect();
            for n in 1..=3 {
                let cs = &diffs[..n];
                run(check_norm(cs));
                let r = Interval::norm(cs).unwrap();
                assert!(
                    r.lo().to_bits() == 0
                        && r.contains(0.0)
                        && r.hi() < fp::pow2(-535)
                        && r.certain_sign().is_none(),
                    "coincident points ({x:e}, {y:e}, {z:e}) in {n}D: norm {cs:?} = {r:?}"
                );
            }
        }
    }

    /// Points one grid step apart along one, two or three axes, at coordinates across a frame of
    /// ±(2^40 − 1) steps and the 25-times finer one: the exact length is 1, √2 or √3 steps, and
    /// `norm` of the differences holds it (by squaring) with a positive lower bound.
    #[test]
    fn points_one_step_apart_have_a_positive_certified_length() {
        let max = fp::pow2(40) - 1.0;
        let coords = [
            -max,
            -fp::pow2(39),
            -1.0,
            0.0,
            1.0,
            fp::pow2(39),
            max - 1.0,
            25.0 * max - 1.0,
        ];
        for axes in 1..=3usize {
            let want = BigRational::from_integer(BigInt::from(axes));
            for &x in &coords {
                for &y in &coords {
                    for &z in &coords {
                        let ds: Vec<Interval> = [x, y, z]
                            .iter()
                            .enumerate()
                            .map(|(j, &c)| {
                                let other = if j < axes { c + 1.0 } else { c };
                                pt(other).sub(pt(c)).unwrap()
                            })
                            .collect();
                        run(check_norm(&ds));
                        let r = Interval::norm(&ds).unwrap();
                        let (rl, rh) = (q::exact(r.lo()), q::exact(r.hi()));
                        assert!(
                            &rl * &rl <= want && want <= &rh * &rh,
                            "length √{axes} from ({x:e}, {y:e}, {z:e}): norm {ds:?} = {r:?}"
                        );
                        assert_eq!(r.certain_sign(), Some(Sign::Positive), "{ds:?}: {r:?}");
                    }
                }
            }
        }
    }

    /// A square or a sum of squares that leaves the finite doubles is reported, not rescued:
    /// 2^512 alone, and two components of 10^154, whose squares (about 10^308) are finite but
    /// whose sum is not. One component of 10^154 is fine.
    #[test]
    fn overflow_is_reported() {
        let big = 1.0e154;
        let cases: [(&[Interval], bool); 4] = [
            (&[pt(fp::pow2(512))], false),
            (&[pt(big), pt(big)], false),
            (&[pt(1.0), pt(-big), pt(big)], false),
            (&[pt(big)], true),
        ];
        for (cs, finite) in cases {
            run(check_norm(cs));
            let r = Interval::norm(cs);
            let expected = if finite {
                r.is_ok()
            } else {
                r == Err(NumError::NotFinite)
            };
            assert!(expected, "norm {cs:?} = {r:?}");
        }
    }

    /// The example in `sqrt_nonneg`'s documentation of why a length goes through `norm`:
    /// 0 − 2^−1074 is [−2^−1073, 0], whose exact value is negative, and `sqrt_nonneg`, which must
    /// take the caller's word, gives [0, 0] with the sign `Zero`.
    #[test]
    fn sqrt_nonneg_takes_the_callers_word() {
        let d = pt(0.0).sub(pt(TINY)).unwrap();
        assert_eq!((d.lo(), d.hi()), (-2.0 * TINY, 0.0), "{d:?}");
        let r = sqrt_nonneg(d).unwrap();
        assert_eq!((r.lo().to_bits(), r.hi().to_bits()), (0, 0), "{r:?}");
        assert_eq!(r.certain_sign(), Some(Sign::Zero), "{r:?}");
    }
}
