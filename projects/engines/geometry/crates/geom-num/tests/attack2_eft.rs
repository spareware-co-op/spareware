//! Second-round adversarial tests of `geom_num::eft`, written against the rewrite in which
//! `fast_two_sum` orders its own operands, `split` scales by 2^−64 from 2^996 up, and `two_prod`
//! has a direct Dekker path (|a|, |b| < 2^996 and 2^−968 ≤ |p| < 2^1022) and a scaled path for
//! every other finite pair. Their only goal is to break the module.
//!
//! Oracle (nothing here calls the code under test to compute an expected value):
//! - `D`: a finite double as an exact dyadic number m · 2^e on `num-bigint`, checked against
//!   `num-rational`'s `BigRational::from_float`.
//! - Round to nearest, ties to even, written from the definition twice: for binary64 (gradual
//!   underflow, overflow from the midpoint Ω + 2^970 up) and for Shewchuk's unbounded 53-bit
//!   arithmetic, which defines Veltkamp's split of every double, the huge ones included. Both are
//!   checked against the hardware first.
//!
//! The contract each test holds the module to, from its own documentation: for finite inputs the
//! pair is exact whenever an exact pair exists (and then both parts are finite); otherwise the
//! second part is an infinity with the sign of the exact error; never NaN from finite inputs; the
//! rounded part is the IEEE result; an exact pair does not overlap.
//!
//! What is attacked:
//! - `two_prod` on every exponent sum at an edge (total underflow, the 2^−1074 and 2^−1022
//!   frontiers, the direct path's lower limit near 2^−968, zero, its upper limit near 2^1022 and
//!   overflow), every exponent split of each sum, subnormal factors at every leading-bit position,
//!   significands built to make the splits round up a binade.
//! - Factors at and around 2^996 against every exponent of the other factor, and factors whose own
//!   split would overflow.
//! - Products straddling 2^−968, 2^1022, Ω, the overflow midpoint, 2^−1022, 2^−1074 and 2^−1075,
//!   and products that are exactly those values (or their neighbours) under every exponent split.
//! - The scaled path's acceptance test at the exact-pair frontier: factors whose lowest bits sum to
//!   just above, at and just below 2^−1074, so an exact pair exists or just fails to.
//! - The sign of every reported infinity against the exact error.
//! - Each step of the documented bit-budget proofs, numerically, at its extreme: every Dekker step
//!   exact on the 2^(`e_a` + `e_b` − 104) grid and within 2^(`e_a` + `e_b` + 2); the scaled path's
//!   `p_s` exact, G ≥ 2g, |`p_m` − `p_s`| ≥ g > g/2 ≥ |`e_m`|; the split's 26-bit halves, its
//!   exponent claim and its overflow bound; the two-sum overflow example.
//! - `split` at and around 2^996 and over the whole overflow frontier below Ω, against Veltkamp's
//!   method in unbounded arithmetic.
//! - −0 against +0 in every output: zero factors, exact products, `fast_two_sum` against
//!   `two_sum` (documented as equal bit for bit), `split(±0)` and every low half, and
//!   `two_prod(−a, b)` against `two_prod(a, b)`. The module documents that a zero second part is
//!   always +0 ("One zero").

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

use geom_num::eft::{SPLITTER, fast_two_sum, split, two_prod, two_sum};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::fmt;

/// The largest finite double, Ω = (2 − 2^−52) · 2^1023.
const OMEGA: f64 = f64::MAX;

/// The 52 fraction bits of a double.
const FRAC: u64 = (1 << 52) - 1;

/// The outcome of one check: `Err` names the inputs and the expected and actual values.
type Check = Result<(), String>;

/// A double with its bits, for failure messages.
fn show(x: f64) -> String {
    format!("{x:e} [{:#018x}]", x.to_bits())
}

/// An exact dyadic number m · 2^e, normalised (m odd, or m = 0 with e = 0) so that equal values
/// compare equal.
#[derive(Clone, Debug, PartialEq, Eq)]
struct D {
    m: BigInt,
    e: i64,
}

impl fmt::Display for D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} * 2^{} (~{:e})", self.m, self.e, self.rn64())
    }
}

impl D {
    /// m · 2^e with the trailing zero bits of m moved into e.
    fn new(m: BigInt, e: i64) -> D {
        match m.trailing_zeros() {
            None => D {
                m: BigInt::zero(),
                e: 0,
            },
            Some(tz) => D {
                m: m >> tz,
                e: e + i64::try_from(tz).unwrap(),
            },
        }
    }

    /// The exact value of a finite double, from its bits.
    fn of(x: f64) -> D {
        assert!(x.is_finite(), "D::of({x}) of a non-finite double");
        let bits = x.to_bits();
        let biased = i64::try_from((bits >> 52) & 0x7ff).unwrap();
        let fraction = bits & FRAC;
        let (significand, e) = if biased == 0 {
            (fraction, -1074)
        } else {
            (fraction | (1 << 52), biased - 1075)
        };
        let m = BigInt::from(significand);
        D::new(if bits >> 63 == 1 { -m } else { m }, e)
    }

    /// 2^k exactly.
    fn pow2(k: i64) -> D {
        D {
            m: BigInt::one(),
            e: k,
        }
    }

    /// An integer times 2^k exactly.
    fn int(m: i64, k: i64) -> D {
        D::new(BigInt::from(m), k)
    }

    fn is_zero(&self) -> bool {
        self.m.is_zero()
    }

    /// −1, 0 or 1.
    fn signum(&self) -> i32 {
        if self.m.is_positive() {
            1
        } else if self.m.is_negative() {
            -1
        } else {
            0
        }
    }

    fn neg(&self) -> D {
        D {
            m: -self.m.clone(),
            e: self.e,
        }
    }

    fn abs(&self) -> D {
        D {
            m: self.m.abs(),
            e: self.e,
        }
    }

    fn add(&self, o: &D) -> D {
        if self.is_zero() {
            return o.clone();
        }
        if o.is_zero() {
            return self.clone();
        }
        let e = self.e.min(o.e);
        let a = &self.m << u64::try_from(self.e - e).unwrap();
        let b = &o.m << u64::try_from(o.e - e).unwrap();
        D::new(a + b, e)
    }

    fn sub(&self, o: &D) -> D {
        self.add(&o.neg())
    }

    fn mul(&self, o: &D) -> D {
        D::new(&self.m * &o.m, self.e + o.e)
    }

    /// The number of significant bits (0 for zero).
    fn bits(&self) -> i64 {
        i64::try_from(self.m.bits()).unwrap()
    }

    /// The position of the highest set bit: 2^h ≤ |value| < 2^(h + 1). Never called for zero.
    fn highest(&self) -> i64 {
        assert!(!self.is_zero(), "highest bit of zero");
        self.e + self.bits() - 1
    }

    /// The position of the lowest set bit. Never called for zero.
    fn lowest(&self) -> i64 {
        assert!(!self.is_zero(), "lowest bit of zero");
        self.e
    }

    /// −1, 0 or 1 as |self| is below, equal to or above |o|.
    fn cmp_abs(&self, o: &D) -> i32 {
        self.abs().sub(&o.abs()).signum()
    }

    /// Round to nearest, ties to even, to 53 significant bits, on a grid no finer than
    /// 2^`floor` when one is given. No overflow: the result may need any exponent.
    fn round(&self, floor: Option<i64>) -> D {
        if self.is_zero() {
            return self.clone();
        }
        let mut quantum = self.highest() - 52;
        if let Some(f) = floor {
            quantum = quantum.max(f);
        }
        if self.e >= quantum {
            return self.clone();
        }
        let shift = u64::try_from(quantum - self.e).unwrap();
        let negative = self.m.is_negative();
        let magnitude = self.m.abs();
        let mut r: BigInt = &magnitude >> shift;
        let rest: BigInt = &magnitude - (&r << shift);
        let half: BigInt = BigInt::one() << (shift - 1);
        if rest > half || (rest == half && r.is_odd()) {
            r += 1u32;
        }
        D::new(if negative { -r } else { r }, quantum)
    }

    /// Shewchuk's unbounded 53-bit rounding (his section 2.1: no overflow, no underflow).
    fn rn53(&self) -> D {
        self.round(None)
    }

    /// The binary64 result of rounding this value: gradual underflow below 2^−1022, ±∞ from the
    /// midpoint Ω + 2^970 up. A zero result is +0; callers that care about the sign of zero check
    /// it themselves.
    fn rn64(&self) -> f64 {
        let r = self.round(Some(-1074));
        if r.is_zero() {
            return 0.0;
        }
        if r.highest() > 1023 {
            return if r.signum() > 0 {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            };
        }
        r.to_double().unwrap()
    }

    /// The value as a double if it is one, built from bits (no hardware arithmetic).
    fn to_double(&self) -> Option<f64> {
        if self.is_zero() {
            return Some(0.0);
        }
        let n = self.bits();
        let h = self.highest();
        if n > 53 || self.e < -1074 || h > 1023 {
            return None;
        }
        let negative = self.m.is_negative();
        let magnitude = self.m.abs().to_u64().unwrap();
        let bits = if h >= -1022 {
            let significand = magnitude << u32::try_from(53 - n).unwrap();
            (u64::try_from(h + 1023).unwrap() << 52) | (significand & FRAC)
        } else {
            magnitude << u32::try_from(self.e + 1074).unwrap()
        };
        Some(f64::from_bits(bits | (u64::from(negative) << 63)))
    }

    /// The same value as a rational.
    fn rational(&self) -> BigRational {
        let p = BigInt::one() << self.e.unsigned_abs();
        if self.e >= 0 {
            BigRational::from_integer(&self.m * p)
        } else {
            BigRational::new(self.m.clone(), p)
        }
    }
}

/// 2^k as a double, for every k a double holds (−1074 ..= 1023).
fn p2(k: i64) -> f64 {
    D::pow2(k)
        .to_double()
        .unwrap_or_else(|| panic!("2^{k} is not a double"))
}

/// m · 2^k as a double; the test's own constants are all doubles.
fn dyadic(m: i64, k: i64) -> f64 {
    D::int(m, k)
        .to_double()
        .unwrap_or_else(|| panic!("{m} * 2^{k} is not a double"))
}

/// A double whose leading bit is 2^e (−1074 ..= 1023), with the fraction bits below the leading
/// bit taken from the top of `fraction` (all 52 for a normal double, fewer for a subnormal one).
fn with_exp(negative: bool, e: i64, fraction: u64) -> f64 {
    assert!(
        (-1074..=1023).contains(&e),
        "no double has its leading bit at 2^{e}"
    );
    let magnitude = if e >= -1022 {
        (u64::try_from(e + 1023).unwrap() << 52) | (fraction & FRAC)
    } else {
        let lead = u32::try_from(e + 1074).unwrap();
        (1u64 << lead) | ((fraction & FRAC) >> (52 - lead))
    };
    f64::from_bits(magnitude | (u64::from(negative) << 63))
}

/// `x` moved `n` doubles up (n > 0) or down (n < 0) along the line of doubles, through zero;
/// infinities stay. Built on bits, independent of `f64::next_up`.
fn step(x: f64, n: i64) -> f64 {
    let mut y = x;
    for _ in 0..n.unsigned_abs() {
        y = if n > 0 { up(y) } else { -up(-y) };
    }
    y
}

/// The next double up.
fn up(x: f64) -> f64 {
    if x == f64::INFINITY || x.is_nan() {
        x
    } else if x == 0.0 {
        f64::from_bits(1)
    } else if x > 0.0 {
        f64::from_bits(x.to_bits() + 1)
    } else {
        f64::from_bits(x.to_bits() - 1)
    }
}

/// A fixed-seed generator: the same cases on every run and target.
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
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = u64::try_from(hi - lo + 1).unwrap();
        lo + i64::try_from(self.next() % span).unwrap()
    }

    /// A random sign.
    fn sign(&mut self) -> bool {
        self.next() & 1 == 1
    }

    /// 52 fraction bits: random, or a pattern aimed at the split. Veltkamp's high half keeps the
    /// leading bit and fraction bits 51 to 27; fraction bit 26 is its rounding bit.
    fn fraction(&mut self) -> u64 {
        let random = self.next() & FRAC;
        let k = self.next() % 52;
        match self.next() % 14 {
            0 => 0,
            1 => FRAC,
            2 => 1,
            3 => 1 << 51,
            4 => 1 << 26,
            5 => (1 << 26) | 1,
            6 => (1 << 26) - 1,
            7 => FRAC & !((1 << 26) - 1),
            8 => FRAC ^ (1 << 26),
            9 => 0x5555_5555_5555_5555 & FRAC,
            10 => 0xAAAA_AAAA_AAAA_AAAA & FRAC,
            11 => 1 << k,
            12 => random | (1 << 26),
            _ => random,
        }
    }

    /// Any finite double, from random bits.
    fn finite(&mut self) -> f64 {
        loop {
            let x = f64::from_bits(self.next());
            if x.is_finite() {
                return x;
            }
        }
    }

    /// An odd whole number of exactly `n` bits (1 ..= 53).
    fn odd(&mut self, n: u32) -> i64 {
        let mask = (1u64 << n) - 1;
        let m = (self.next() & mask) | 1 | (1u64 << (n - 1));
        i64::try_from(m).unwrap()
    }
}

/// Collects every failing check of a sweep, so one run reports how many failed and the first few.
struct Failures {
    what: &'static str,
    checked: usize,
    failed: usize,
    first: Vec<String>,
}

impl Failures {
    fn new(what: &'static str) -> Failures {
        Failures {
            what,
            checked: 0,
            failed: 0,
            first: Vec::new(),
        }
    }

    fn record(&mut self, result: Check) {
        self.checked += 1;
        if let Err(message) = result {
            self.failed += 1;
            if self.first.len() < 6 {
                self.first.push(message);
            }
        }
    }

    fn finish(self) {
        assert!(self.checked > 0, "{}: nothing was checked", self.what);
        assert!(
            self.failed == 0,
            "{}: {} of {} checks failed; the first:\n  {}",
            self.what,
            self.failed,
            self.checked,
            self.first.join("\n  ")
        );
    }
}

/// Whether `(a, b)` is inside the range `two_prod`'s documentation says takes the direct path.
fn in_direct_range(a: f64, b: f64) -> bool {
    let size = (a * b).abs();
    a.abs() < p2(996) && b.abs() < p2(996) && size >= p2(-968) && size < p2(1022)
}

/// `two_prod(a, b)` against its whole documented contract, for any inputs.
fn check_two_prod(a: f64, b: f64) -> Check {
    let (p, e) = two_prod(a, b);
    let call = format!("two_prod({}, {})", show(a), show(b));
    let hardware = a * b;
    if p.to_bits() != hardware.to_bits() && !(p.is_nan() && hardware.is_nan()) {
        return Err(format!(
            "{call}: expected p = {} (IEEE), got {}",
            show(hardware),
            show(p)
        ));
    }
    if !p.is_finite() {
        if p.is_nan() {
            if !e.is_nan() {
                return Err(format!("{call}: p is NaN, expected e NaN, got {}", show(e)));
            }
        } else if e.to_bits() != (-p).to_bits() {
            return Err(format!(
                "{call}: p = {}, expected e = {}, got {}",
                show(p),
                show(-p),
                show(e)
            ));
        }
        return Ok(());
    }
    let exact = D::of(a).mul(&D::of(b));
    let rounded = exact.rn64();
    let zero_sign = a.is_sign_negative() != b.is_sign_negative();
    if rounded != p || (p == 0.0 && p.is_sign_negative() != zero_sign) {
        return Err(format!(
            "{call}: oracle {} disagrees with the IEEE product {}",
            show(rounded),
            show(p)
        ));
    }
    if e.is_nan() {
        return Err(format!("{call}: e is NaN from finite inputs"));
    }
    let error = exact.sub(&D::of(p));
    if let Some(want) = error.to_double() {
        if !e.is_finite() || D::of(e) != error {
            return Err(format!(
                "{call}: an exact pair exists, expected e = {} (exact error {error}), got {}",
                show(want),
                show(e)
            ));
        }
        check_half_unit(&call, p, e)?;
    } else {
        let want = if error.signum() > 0 {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
        if e.to_bits() != want.to_bits() {
            return Err(format!(
                "{call}: no exact pair exists (exact error {error} is not a double), \
                 expected e = {}, got {}",
                show(want),
                show(e)
            ));
        }
        // The documented frontier: possible only for |a · b| < 2^−969, e_a + e_b ≤ −971.
        let sum = D::of(a).highest() + D::of(b).highest();
        if exact.highest() > -970 || sum > -971 {
            return Err(format!(
                "{call}: no exact pair although |a·b| = {exact} and e_a + e_b = {sum}; the \
                 documentation says that needs |a·b| < 2^-969 and e_a + e_b <= -971"
            ));
        }
    }
    Ok(())
}

/// The documented size of an exact error: at most half a unit in the last place of the rounded
/// part (half its quantum, 2^(`e_p` − 53), or 2^−1075 for a subnormal one; zero beside a zero), and
/// so no overlap: the rounded part plus the error rounds back to the rounded part.
fn check_half_unit(call: &str, r: f64, e: f64) -> Check {
    let (dr, de) = (D::of(r), D::of(e));
    if de.is_zero() {
        return Ok(());
    }
    if dr.is_zero() {
        return Err(format!(
            "{call}: nonzero error {} beside a zero rounded part",
            show(e)
        ));
    }
    let half = D::pow2((dr.highest() - 52).max(-1074) - 1);
    if de.cmp_abs(&half) > 0 {
        return Err(format!(
            "{call}: |e| = {} exceeds half a unit of {}",
            show(e),
            show(r)
        ));
    }
    let back = dr.add(&de).rn64();
    if back != r {
        return Err(format!(
            "{call}: {} + {} rounds to {}: the pair overlaps",
            show(r),
            show(e),
            show(back)
        ));
    }
    Ok(())
}

/// The documented direct path, step by step at its extremes: Dekker's six operations on the halves
/// [`split`] gives, each exact, each a whole multiple of 2^(`e_a` + `e_b` − 104) ≥ 2^−1074 and at
/// most 2^(`e_a` + `e_b` + 2), the partial products of at most 52 bits, and `two_prod` returning
/// exactly the error those operations make, bit for bit, except that a zero error is +0 (it is
/// documented to take this path and then add +0, which turns the −0 Dekker's last step can give
/// into +0).
fn check_dekker_steps(a: f64, b: f64) -> Check {
    let call = format!("two_prod({}, {}) [direct path]", show(a), show(b));
    let p = a * b;
    let (ah, al) = split(a);
    let (bh, bl) = split(b);
    let sum = D::of(a).highest() + D::of(b).highest();
    if !(-970..=1021).contains(&sum) {
        return Err(format!(
            "{call}: e_a + e_b = {sum}, outside the documented -970 ..= 1021"
        ));
    }
    let grid = sum - 104;
    let cap = D::pow2(sum + 2);
    let t1 = ah * bh;
    let err1 = p - t1;
    let t2 = al * bh;
    let err2 = err1 - t2;
    let t3 = ah * bl;
    let err3 = err2 - t3;
    let t4 = al * bl;
    let e = t4 - err3;
    let steps = [
        ("a_hi*b_hi", t1, D::of(ah).mul(&D::of(bh)), true),
        ("p - a_hi*b_hi", err1, D::of(p).sub(&D::of(t1)), false),
        ("a_lo*b_hi", t2, D::of(al).mul(&D::of(bh)), true),
        ("err1 - a_lo*b_hi", err2, D::of(err1).sub(&D::of(t2)), false),
        ("a_hi*b_lo", t3, D::of(ah).mul(&D::of(bl)), true),
        ("err2 - a_hi*b_lo", err3, D::of(err2).sub(&D::of(t3)), false),
        ("a_lo*b_lo", t4, D::of(al).mul(&D::of(bl)), true),
        ("a_lo*b_lo - err3", e, D::of(t4).sub(&D::of(err3)), false),
    ];
    for (name, got, exact, partial) in &steps {
        if !got.is_finite() || D::of(*got) != *exact {
            return Err(format!(
                "{call}: step {name} = {} is not exact; exact {exact}",
                show(*got)
            ));
        }
        if exact.is_zero() {
            continue;
        }
        if exact.lowest() < grid {
            return Err(format!(
                "{call}: step {name} = {exact} has a bit below the claimed grid \
                 2^{grid} = 2^(e_a+e_b-104)"
            ));
        }
        if exact.cmp_abs(&cap) > 0 {
            return Err(format!(
                "{call}: step {name} = {exact} exceeds the claimed 2^(e_a+e_b+2)"
            ));
        }
        if *partial && exact.bits() > 52 {
            return Err(format!(
                "{call}: partial product {name} = {exact} has more than 52 bits"
            ));
        }
    }
    if D::of(p).cmp_abs(&cap) > 0 {
        return Err(format!("{call}: p exceeds the claimed 2^(e_a+e_b+2)"));
    }
    let (_, got) = two_prod(a, b);
    // Written as a comparison, not as `e + 0.0`, so the expectation does not rest on the same
    // IEEE rule the module uses.
    let want = if e == 0.0 { 0.0 } else { e };
    if got.to_bits() != want.to_bits() {
        return Err(format!(
            "{call}: documented to run Dekker's six operations unchanged and then make a zero \
             error +0, which gives e = {}, got {}",
            show(want),
            show(got)
        ));
    }
    Ok(())
}

/// The documented scaled path, step by step, for nonzero finite factors with a finite product
/// outside the direct range: `m_a`, `m_b` doubles in [1, 2); X in [1, 4); `e_m` a double of at most
/// g/2; `p_s` = p · 2^k a double of magnitude at most 4; `p_s` = `p_m` when |a · b| ≥ 2^−1022; when
/// `p_s` ≠ `p_m`: |a · b| < 2^−1022, k > 0, G ≥ 2g, |`p_m` − `p_s`| ≥ g, and the exact error has
/// the sign of `p_m` − `p_s`, which the hardware subtraction keeps; when `p_s` = `p_m` and k ≤ 0
/// the exact error is a double.
fn check_scaled_proof(a: f64, b: f64) -> Check {
    let call = format!("two_prod({}, {}) [scaled path]", show(a), show(b));
    let (da, db) = (D::of(a), D::of(b));
    let (ea, eb) = (da.highest(), db.highest());
    let k = -(ea + eb);
    let ma = da.mul(&D::pow2(-ea));
    let mb = db.mul(&D::pow2(-eb));
    for (name, m) in [("m_a", &ma), ("m_b", &mb)] {
        if m.to_double().is_none() || m.highest() != 0 {
            return Err(format!("{call}: {name} = {m} is not a double in [1, 2)"));
        }
    }
    let x = ma.mul(&mb);
    if !(0..=1).contains(&x.highest()) {
        return Err(format!("{call}: X = {x} is not in [1, 4)"));
    }
    let pm = x.rn53();
    let g = x.highest() - 52;
    let em = x.sub(&pm);
    if em.to_double().is_none() || (!em.is_zero() && em.cmp_abs(&D::pow2(g - 1)) > 0) {
        return Err(format!(
            "{call}: e_m = {em} is not a double of at most g/2 = 2^{}",
            g - 1
        ));
    }
    let p = a * b;
    let ps = D::of(p).mul(&D::pow2(k));
    if ps.to_double().is_none() || ps.cmp_abs(&D::int(4, 0)) > 0 {
        return Err(format!(
            "{call}: p_s = p * 2^{k} = {ps} is not a double of magnitude at most 4"
        ));
    }
    let exact = da.mul(&db);
    let normal = exact.highest() >= -1022;
    if normal && ps != pm {
        return Err(format!(
            "{call}: |a·b| >= 2^-1022 but p_s = {ps} differs from p_m = {pm}"
        ));
    }
    let error = exact.sub(&D::of(p));
    if ps == pm {
        if k <= 0 && error.to_double().is_none() {
            return Err(format!(
                "{call}: k = {k} <= 0 but the exact error {error} is not a double"
            ));
        }
        if error != em.mul(&D::pow2(-k)) {
            return Err(format!(
                "{call}: p_s = p_m but e_m * 2^-k differs from the exact error {error}"
            ));
        }
        return Ok(());
    }
    if normal || k <= 0 {
        return Err(format!("{call}: p_s != p_m with |a·b| = {exact}, k = {k}"));
    }
    let big_g = k - 1074;
    if big_g < g + 1 {
        return Err(format!(
            "{call}: G = 2^{big_g} is not at least 2g = 2^{}",
            g + 1
        ));
    }
    let gap = pm.sub(&ps);
    if gap.cmp_abs(&D::pow2(g)) < 0 {
        return Err(format!("{call}: |p_m - p_s| = {gap} is below g = 2^{g}"));
    }
    if error.signum() != gap.signum() {
        return Err(format!(
            "{call}: the exact error {error} does not have the sign of p_m - p_s = {gap}"
        ));
    }
    let hardware = pm.to_double().unwrap() - ps.to_double().unwrap();
    let hardware_sign = if hardware > 0.0 {
        1
    } else if hardware < 0.0 {
        -1
    } else {
        0
    };
    if hardware_sign != gap.signum() {
        return Err(format!(
            "{call}: the rounded p_m - p_s = {} lost the sign of {gap}",
            show(hardware)
        ));
    }
    Ok(())
}

/// `two_prod(a, b)` and `two_prod(b, a)` must be the same bits (the method is symmetric in its
/// factors, and nothing in the documentation says the order matters); `two_prod(−a, b)` must be
/// `(−p, −e)` bit for bit, except that a zero `e` stays +0 (documented: "One zero").
fn check_two_prod_symmetry(a: f64, b: f64) -> Check {
    let (p, e) = two_prod(a, b);
    let (p2, e2) = two_prod(b, a);
    let same = |x: f64, y: f64| x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan());
    if !same(p, p2) || !same(e, e2) {
        return Err(format!(
            "two_prod({}, {}) = ({}, {}) but two_prod(b, a) = ({}, {})",
            show(a),
            show(b),
            show(p),
            show(e),
            show(p2),
            show(e2)
        ));
    }
    let (p3, e3) = two_prod(-a, b);
    // e is NaN only when p is, so past the first test both are numbers.
    let want_e3 = if e == 0.0 { 0.0 } else { -e };
    if !p.is_nan() && (p3.to_bits() != (-p).to_bits() || e3.to_bits() != want_e3.to_bits()) {
        return Err(format!(
            "two_prod({}, {}) = ({}, {}) but two_prod(-a, b) = ({}, {})",
            show(a),
            show(b),
            show(p),
            show(e),
            show(p3),
            show(e3)
        ));
    }
    Ok(())
}

/// Every check that applies to `(a, b)`.
fn check_two_prod_all(a: f64, b: f64) -> Check {
    check_two_prod(a, b)?;
    check_two_prod_symmetry(a, b)?;
    let p = a * b;
    if !a.is_finite() || !b.is_finite() || !p.is_finite() || a == 0.0 || b == 0.0 {
        return Ok(());
    }
    if in_direct_range(a, b) {
        check_dekker_steps(a, b)
    } else {
        check_scaled_proof(a, b)
    }
}

/// A sum function against its documented contract, for any inputs.
fn check_sum(name: &str, f: fn(f64, f64) -> (f64, f64), a: f64, b: f64) -> Check {
    let (s, e) = f(a, b);
    let call = format!("{name}({}, {})", show(a), show(b));
    let hardware = a + b;
    if s.to_bits() != hardware.to_bits() && !(s.is_nan() && hardware.is_nan()) {
        return Err(format!(
            "{call}: expected s = {} (IEEE), got {}",
            show(hardware),
            show(s)
        ));
    }
    if !s.is_finite() {
        if s.is_nan() {
            if !e.is_nan() {
                return Err(format!("{call}: s is NaN, expected e NaN, got {}", show(e)));
            }
        } else if e.to_bits() != (-s).to_bits() {
            return Err(format!(
                "{call}: s = {}, expected e = {}, got {}",
                show(s),
                show(-s),
                show(e)
            ));
        }
        return Ok(());
    }
    let exact = D::of(a).add(&D::of(b));
    if exact.rn64() != s {
        return Err(format!(
            "{call}: oracle {} disagrees with the IEEE sum {}",
            show(exact.rn64()),
            show(s)
        ));
    }
    let error = exact.sub(&D::of(s));
    let Some(want) = error.to_double() else {
        return Err(format!(
            "{call}: the exact error {error} of a finite sum is not a double"
        ));
    };
    if !e.is_finite() || D::of(e) != error {
        return Err(format!(
            "{call}: expected e = {} (exact error {error}), got {}",
            show(want),
            show(e)
        ));
    }
    check_half_unit(&call, s, e)
}

/// 2^1024 − 2^997: `split` is documented to report exactly the `a` with |a| at or above it.
fn split_threshold() -> D {
    D::pow2(1024).sub(&D::pow2(997))
}

/// Veltkamp's split in Shewchuk's unbounded 53-bit arithmetic (Theorem 17, s = 27).
fn veltkamp_unbounded(a: &D) -> (D, D) {
    let c = D::of(SPLITTER).mul(a).rn53();
    let big = c.sub(a).rn53();
    let hi = c.sub(&big).rn53();
    let lo = a.sub(&hi).rn53();
    (hi, lo)
}

/// `split(a)` against Veltkamp's method in unbounded arithmetic and the documented bit budget.
#[allow(
    clippy::too_many_lines,
    reason = "one oracle, checked step by step against the documented budget"
)]
fn check_split(a: f64) -> Check {
    let (hi, lo) = split(a);
    let call = format!("split({})", show(a));
    if a.is_nan() {
        return if hi.is_nan() && lo.is_nan() {
            Ok(())
        } else {
            Err(format!(
                "{call}: expected (NaN, NaN), got ({}, {})",
                show(hi),
                show(lo)
            ))
        };
    }
    if a.is_infinite() {
        return if hi.to_bits() == a.to_bits() && lo.to_bits() == (-a).to_bits() {
            Ok(())
        } else {
            Err(format!(
                "{call}: expected ({}, {}), got ({}, {})",
                show(a),
                show(-a),
                show(hi),
                show(lo)
            ))
        };
    }
    if a == 0.0 {
        // Documented: split(±0) is (±0, +0).
        return if hi.to_bits() == a.to_bits() && lo.to_bits() == 0 {
            Ok(())
        } else {
            Err(format!(
                "{call}: expected ({}, +0), got ({}, {})",
                show(a),
                show(hi),
                show(lo)
            ))
        };
    }
    let da = D::of(a);
    let (h, l) = veltkamp_unbounded(&da);
    if h.add(&l) != da {
        return Err(format!(
            "{call}: oracle broken: Veltkamp's halves {h} and {l} do not add up"
        ));
    }
    // Documented: reported exactly when |a| >= 2^1024 - 2^997.
    let above = da.cmp_abs(&split_threshold()) >= 0;
    let Some(want_hi) = h.to_double() else {
        if h.abs() != D::pow2(1024) {
            return Err(format!(
                "{call}: oracle: the unbounded high half {h} is not a double or 2^1024"
            ));
        }
        let inf = if a > 0.0 {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
        if hi.to_bits() != inf.to_bits() || lo.to_bits() != (-inf).to_bits() {
            return Err(format!(
                "{call}: the high half is {h}, which no double holds; expected ({}, {}), \
                 got ({}, {})",
                show(inf),
                show(-inf),
                show(hi),
                show(lo)
            ));
        }
        if !above {
            return Err(format!("{call}: reported, but |a| is below 2^1024 - 2^997"));
        }
        return Ok(());
    };
    if above {
        return Err(format!(
            "{call}: |a| >= 2^1024 - 2^997, but the high half {h} is a double"
        ));
    }
    let want_lo = l.to_double().unwrap();
    if !hi.is_finite() || !lo.is_finite() || D::of(hi) != h || D::of(lo) != l {
        return Err(format!(
            "{call}: expected Veltkamp's halves ({}, {}), got ({}, {})",
            show(want_hi),
            show(want_lo),
            show(hi),
            show(lo)
        ));
    }
    if lo.to_bits() == (-0.0f64).to_bits() {
        return Err(format!(
            "{call}: the low half is -0; documented: a zero low half is +0"
        ));
    }
    let ea = da.highest();
    if h.bits() > 26 || (!l.is_zero() && l.bits() > 26) {
        return Err(format!("{call}: a half has more than 26 bits: {h}, {l}"));
    }
    if l.cmp_abs(&h) > 0 {
        return Err(format!("{call}: |hi| < |lo|"));
    }
    if h.highest() != ea && !(h.highest() == ea + 1 && h.bits() == 1) {
        return Err(format!(
            "{call}: hi = {h} has neither the exponent of a ({ea}) nor one more as a single bit"
        ));
    }
    if h.lowest() < ea - 52 || (!l.is_zero() && l.lowest() < ea - 52) {
        return Err(format!(
            "{call}: a half is not a whole multiple of 2^(e_a - 52) = 2^{}",
            ea - 52
        ));
    }
    Ok(())
}

/// Exponent sums `e_a` + `e_b` on an edge: total underflow, the 2^−1075 and 2^−1074 frontiers of
/// the product, the subnormal/normal frontier at 2^−1022, the direct path's lower limit near
/// 2^−968, around zero, and the direct path's upper limit near 2^1022 and overflow.
const EDGE_SUMS: [i64; 31] = [
    -2148, -2100, -1200, -1078, -1077, -1076, -1075, -1074, -1073, -1072, -1071, -1025, -1024,
    -1023, -1022, -1021, -1020, -972, -971, -970, -969, -968, -967, -1, 0, 1, 1019, 1020, 1021,
    1022, 1023,
];

#[test]
fn oracle_matches_num_rational_and_the_hardware() {
    let mut g = Lcg(0x0a11_0001);
    let mut edge = vec![
        0.0,
        -0.0,
        p2(-1074),
        -p2(-1074),
        p2(-1022),
        OMEGA,
        -OMEGA,
        1.0,
        p2(996),
    ];
    edge.push(f64::from_bits(FRAC));
    for _ in 0..2000 {
        edge.push(g.finite());
    }
    for &x in &edge {
        assert_eq!(
            D::of(x).rational(),
            BigRational::from_float(x).unwrap(),
            "D::of({x:e})"
        );
        let canonical = if x == 0.0 { 0.0 } else { x };
        let back = D::of(x).to_double().unwrap();
        assert_eq!(back.to_bits(), canonical.to_bits(), "to_double({x:e})");
    }
    for _ in 0..20_000 {
        let (a, b) = (g.finite(), g.finite());
        let s = D::of(a).add(&D::of(b)).rn64();
        assert!(
            s == a + b,
            "rn64 of {a:e} + {b:e}: {s:e} against {:e}",
            a + b
        );
        let p = D::of(a).mul(&D::of(b)).rn64();
        assert!(
            p == a * b,
            "rn64 of {a:e} * {b:e}: {p:e} against {:e}",
            a * b
        );
        // Products that stay well inside the normal range: unbounded and binary64 rounding agree.
        let sa = with_exp(g.sign(), g.range(-400, 400), g.fraction());
        let sb = with_exp(false, g.range(-400, 400), g.fraction());
        let q = D::of(sa).mul(&D::of(sb));
        assert_eq!(
            q.rn53().to_double().unwrap(),
            sa * sb,
            "rn53 of {sa:e} * {sb:e}"
        );
    }
    // The overflow boundary: the midpoint Ω + 2^970 ties to 2^1024, just below it rounds to Ω.
    let mid = D::of(OMEGA).add(&D::pow2(970));
    assert_eq!(mid.rn64(), f64::INFINITY);
    assert_eq!(mid.sub(&D::pow2(900)).rn64(), OMEGA);
    // Ties to even at the bottom: 2^−1075 rounds to 0, 3 · 2^−1075 to 2^−1073.
    assert_eq!(D::pow2(-1075).rn64(), 0.0);
    assert_eq!(D::int(3, -1075).rn64(), p2(-1073));
}

/// Every exponent split of every edge exponent sum, three times each with fresh signs and
/// split-adversarial significands, subnormal factors at every leading-bit position the sum allows.
#[test]
fn two_prod_every_exponent_split_of_every_edge_sum() {
    let mut g = Lcg(0x0b0b_0002);
    let mut f = Failures::new("two_prod on every exponent split of the edge sums");
    for &sum in &EDGE_SUMS {
        let lo = (-1074i64).max(sum - 1023);
        let hi = 1023i64.min(sum + 1074);
        for ea in lo..=hi {
            for _ in 0..3 {
                let a = with_exp(g.sign(), ea, g.fraction());
                let b = with_exp(g.sign(), sum - ea, g.fraction());
                f.record(check_two_prod_all(a, b));
            }
        }
    }
    f.finish();
}

/// The direct path's factor limit: factors at, just below and just above 2^996 (and ones whose
/// split rounds up to 2^996, and the largest doubles, whose own split overflows), against every
/// exponent of the other factor.
#[test]
fn two_prod_factor_at_two_pow_996_against_every_exponent() {
    let mut g = Lcg(0x0c0c_0003);
    let round_up = FRAC & !((1 << 26) - 1);
    let factors = [
        p2(996),
        step(p2(996), -1),
        step(p2(996), 1),
        step(p2(996), -2),
        with_exp(false, 995, round_up),
        with_exp(false, 995, 1 << 26),
        with_exp(false, 996, FRAC),
        p2(997),
        OMEGA,
        with_exp(false, 1023, round_up),
        with_exp(false, 1023, round_up ^ (1 << 26)),
        with_exp(false, 1022, FRAC),
    ];
    let mut f = Failures::new("two_prod with a factor near 2^996 or Ω");
    for &a in &factors {
        for eb in -1074..=1023 {
            for _ in 0..2 {
                let b = with_exp(g.sign(), eb, g.fraction());
                f.record(check_two_prod_all(a, b));
                f.record(check_two_prod_all(-b, a));
            }
        }
    }
    f.finish();
}

/// Products straddling each threshold: 2^−968 and 2^1022 (the direct range), Ω and the overflow
/// midpoint, 2^−1022 (gradual underflow), 2^−1074 and 2^−1075 (the last subnormal and the tie to
/// zero), and the halves of each. Inexact products must actually round onto 2^−968, 2^1022 and Ω.
#[test]
fn two_prod_products_straddling_every_threshold() {
    let mut g = Lcg(0x0d0d_0004);
    let targets = [
        p2(-968),
        p2(1022),
        OMEGA,
        p2(-1022),
        p2(-1074),
        p2(-1073),
        p2(-969),
    ];
    let mut f = Failures::new("two_prod on products straddling the thresholds");
    let mut rounded_onto = [0usize; 3];
    for &t in &targets {
        let et = D::of(t).highest();
        for _ in 0..1200 {
            let ea = g.range((-1074i64).max(et - 1022), 1023i64.min(et + 1073));
            let a = with_exp(g.sign(), ea, g.fraction());
            for scale in [1.0, 0.5] {
                let b0 = t / a * scale;
                if b0 == 0.0 || !b0.is_finite() {
                    continue;
                }
                for j in -3..=3 {
                    let b = step(b0, j);
                    f.record(check_two_prod_all(a, b));
                    let p = (a * b).abs();
                    let exact = D::of(a).mul(&D::of(b)).abs();
                    for (slot, edge) in [p2(-968), p2(1022), OMEGA].into_iter().enumerate() {
                        if p == edge && exact.cmp_abs(&D::of(edge)) != 0 {
                            rounded_onto[slot] += 1;
                        }
                    }
                }
            }
        }
    }
    f.finish();
    assert!(
        rounded_onto.iter().all(|&n| n > 0),
        "no inexact product rounded onto each of 2^-968, 2^1022 and Ω: {rounded_onto:?}"
    );
}

/// Products that are exactly a threshold, or its neighbour below, under every exponent split:
/// Ω = 6361 · 1416003655831 · 2^971, the overflow midpoint 2^1024 − 2^970 = (2^27 − 1)(2^27 + 1)
/// · 2^970, 2^1022 and its predecessor, 2^−968 and its predecessor (the direct path's lower limit
/// from both sides), the largest subnormal (2^26 − 1)(2^26 + 1) · 2^−1074, and 2^−1074.
#[test]
fn two_prod_products_exactly_at_the_thresholds() {
    assert_eq!(6361i64 * 1_416_003_655_831, (1i64 << 53) - 1);
    let mut f = Failures::new("two_prod on products exactly at a threshold");
    // (m1, m2, exponent of the product): m1 · 2^i times m2 · 2^(t − i).
    let products: [(i64, i64, i64); 8] = [
        (6361, 1_416_003_655_831, 971),
        (6361, 1_416_003_655_831, 969),
        (6361, 1_416_003_655_831, -1021),
        ((1 << 27) - 1, (1 << 27) + 1, 970),
        ((1 << 26) - 1, (1 << 26) + 1, -1074),
        (1, 1, 1022),
        (1, 1, -968),
        (1, 1, -1074),
    ];
    for &(m1, m2, t) in &products {
        let mut covered = 0usize;
        for i in -1200i64..=1200 {
            let a = D::int(m1, i).to_double();
            let b = D::int(m2, t - i).to_double();
            let (Some(a), Some(b)) = (a, b) else {
                continue;
            };
            covered += 1;
            for (x, y) in [(a, b), (-a, b), (b, -a), (-b, -a)] {
                f.record(check_two_prod_all(x, y));
            }
        }
        assert!(
            covered > 0,
            "no exponent split of {m1} * {m2} * 2^{t} is a pair of doubles"
        );
    }
    // The product exactly at the overflow midpoint ties to 2^1024, so it is infinite.
    let (p, e) = two_prod(dyadic((1 << 27) - 1, 900), dyadic((1 << 27) + 1, 70));
    assert!(
        p == f64::INFINITY && e == f64::NEG_INFINITY,
        "(2^27 - 1)(2^27 + 1) * 2^970: expected (+inf, -inf), got ({}, {})",
        show(p),
        show(e)
    );
    f.finish();
}

/// The scaled path's acceptance test at the exact-pair frontier. An exact pair exists exactly when
/// a · b is a whole multiple of 2^−1074, so the factors are built with lowest set bits `l_a` and
/// `l_b` whose sum runs from 2^−1080 to 2^−1068, with every length of significand: the check must
/// accept every exact pair (nonzero errors included) and reject every other, with the sign of the
/// exact error.
#[test]
fn two_prod_acceptance_at_the_exact_pair_frontier() {
    let mut g = Lcg(0x0e0e_0005);
    let mut f = Failures::new("two_prod at the exact-pair frontier");
    let (mut accepted, mut accepted_nonzero, mut rejected) = (0usize, 0usize, 0usize);
    for _ in 0..150_000 {
        let low_sum = g.range(-1080, -1068);
        let na = u32::try_from(g.range(1, 53)).unwrap();
        let nb = u32::try_from(g.range(1, 53)).unwrap();
        let la = g.range(-1074, low_sum + 1074);
        let lb = low_sum - la;
        let a = D::int(g.odd(na), la).to_double();
        let b = D::int(g.odd(nb), lb).to_double();
        let (Some(a), Some(b)) = (a, b) else {
            continue;
        };
        let (a, b) = (if g.sign() { -a } else { a }, if g.sign() { -b } else { b });
        f.record(check_two_prod_all(a, b));
        let (_, e) = two_prod(a, b);
        if !e.is_finite() {
            rejected += 1;
        } else if e == 0.0 {
            accepted += 1;
        } else {
            accepted_nonzero += 1;
        }
    }
    f.finish();
    assert!(
        accepted > 1000 && accepted_nonzero > 1000 && rejected > 1000,
        "frontier not exercised: {accepted} exact, {accepted_nonzero} exact with a nonzero error, \
         {rejected} reported"
    );
}

/// The sign of each reported infinity against the exact error, at the cases where it is easiest to
/// get wrong: the tie to zero 2^−1075 (rounds to 0, error positive), just above it (rounds to
/// 2^−1074, error negative), 3 · 2^−1076 (rounds up), 1.5 · 2^−1074 (ties to even 2^−1073), total
/// underflow of both signs, the documented example `two_prod(2^−600, 2^−600) = (0, +∞)`, and a
/// sweep of tiny products of both signs.
#[test]
fn two_prod_reported_infinity_has_the_sign_of_the_exact_error() {
    let cases = [
        (p2(-537), p2(-538)),
        (p2(-537), dyadic((1 << 52) + 1, -590)),
        (dyadic(3, -538), p2(-538)),
        (dyadic(3, -537), p2(-538)),
        (p2(-1074), 0.5),
        (p2(-1074), 1.5),
        (p2(-1074), 2.5),
        (p2(-1074), p2(-1074)),
        (-p2(-1074), p2(-1074)),
        (OMEGA * p2(-1023), p2(-1074)),
        (dyadic((1 << 53) - 1, -560), dyadic((1 << 53) - 1, -560)),
        (p2(-600), p2(-600)),
        (-p2(-600), p2(-600)),
        (p2(-1000), dyadic(3, -75)),
        (p2(-1000), dyadic(5, -76)),
    ];
    let mut f = Failures::new("two_prod sign of the reported infinity");
    for &(a, b) in &cases {
        for (x, y) in [(a, b), (-a, b), (a, -b), (-a, -b)] {
            f.record(check_two_prod_all(x, y));
        }
    }
    let mut g = Lcg(0x0f0f_0006);
    for _ in 0..80_000 {
        let sum = g.range(-1110, -1018);
        let ea = g.range((-1074i64).max(sum - 1023), 1023i64.min(sum + 1074));
        let a = with_exp(g.sign(), ea, g.fraction());
        let b = with_exp(g.sign(), sum - ea, g.fraction());
        f.record(check_two_prod_all(a, b));
    }
    f.finish();
    let (p, e) = two_prod(p2(-600), p2(-600));
    assert!(
        p.to_bits() == 0 && e == f64::INFINITY,
        "the documented two_prod(2^-600, 2^-600) = (0, +inf): got ({}, {})",
        show(p),
        show(e)
    );
    let (p, e) = two_prod(-p2(-600), p2(-600));
    assert!(
        p.to_bits() == (-0.0f64).to_bits() && e == f64::NEG_INFINITY,
        "two_prod(-2^-600, 2^-600) must be (-0, -inf): got ({}, {})",
        show(p),
        show(e)
    );
}

/// Random finite doubles over the whole range, and random pairs whose product stays finite.
#[test]
fn two_prod_random_over_the_whole_range() {
    let mut g = Lcg(0x1010_0007);
    let mut f = Failures::new("two_prod on random pairs");
    for _ in 0..100_000 {
        let (a, b) = (g.finite(), g.finite());
        f.record(check_two_prod_all(a, b));
        let sum = g.range(-1100, 1023);
        let ea = g.range((-1074i64).max(sum - 1023), 1023i64.min(sum + 1074));
        let a = with_exp(g.sign(), ea, g.next());
        let b = with_exp(g.sign(), sum - ea, g.next());
        f.record(check_two_prod_all(a, b));
    }
    f.finish();
}

/// Products of grid coordinates (whole numbers below 2^41 in magnitude) are documented to take
/// the direct path: Dekker's six operations, bit for bit. A zero coordinate gives (p, +0).
#[test]
fn two_prod_grid_coordinate_products() {
    let mut g = Lcg(0x1818_000f);
    let mut f = Failures::new("two_prod on grid coordinates");
    for _ in 0..50_000 {
        let bits = u32::try_from(g.range(1, 41)).unwrap();
        let a = dyadic(g.odd(bits) << g.range(0, i64::from(41 - bits)), 0);
        let a = if g.sign() { -a } else { a };
        let b = f64::from(i32::try_from(g.next() % (1 << 30)).unwrap()) * 1024.0;
        f.record(check_two_prod_all(a, b));
        if b != 0.0 {
            f.record(if in_direct_range(a, b) {
                check_dekker_steps(a, b)
            } else {
                Err(format!(
                    "two_prod({}, {}) is outside the direct range",
                    show(a),
                    show(b)
                ))
            });
        }
    }
    f.finish();
}

/// A zero factor gives `(p, 0)` (documented), with `p` the IEEE signed zero and the error +0.
#[test]
fn two_prod_zero_factor_gives_signed_p_and_plus_zero() {
    let others = [
        1.0,
        -1.0,
        p2(-1074),
        -p2(-1074),
        OMEGA,
        -OMEGA,
        p2(996),
        p2(-968),
        0.0,
        -0.0,
    ];
    let mut bad = Vec::new();
    for &x in &others {
        for z in [0.0f64, -0.0] {
            for (a, b) in [(z, x), (x, z)] {
                let (p, e) = two_prod(a, b);
                let want_p = a * b;
                if p.to_bits() != want_p.to_bits() || e.to_bits() != 0 {
                    bad.push(format!(
                        "two_prod({}, {}): expected ({}, +0), got ({}, {})",
                        show(a),
                        show(b),
                        show(want_p),
                        show(p),
                        show(e)
                    ));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The sign of an exact product's zero error: +0, as the module documents ("One zero"), the same
/// as `two_sum`'s exact error for every input, a fused multiply-add two-product's
/// fma(a, b, −p) = +0, and a zero factor's +0. Dekker's last step `a_lo·b_lo − err3` alone gives
/// −0 whenever one factor's low half is +0 and the other's is negative, so without the module's
/// fix the zero's sign would depend on how Veltkamp's split falls: `two_prod(1, 1) = (1, +0)` but
/// `two_prod(Ω, 1) = (Ω, −0)`.
#[test]
fn two_prod_exact_product_error_is_plus_zero() {
    let ones = dyadic((1 << 53) - 1, -52);
    let cases = [
        (1.0, 1.0),
        (OMEGA, 1.0),
        (1.0, OMEGA),
        (ones, 1.0),
        (2.0, ones),
        (-2.0, ones),
        (ones, -0.5),
        (p2(1000), ones),
        (p2(-1000), ones),
        (p2(-1022), ones),
        (OMEGA, 0.5),
    ];
    let mut bad = Vec::new();
    for &(a, b) in &cases {
        let (p, e) = two_prod(a, b);
        let exact = D::of(a).mul(&D::of(b));
        assert_eq!(
            exact.sub(&D::of(p)),
            D::int(0, 0),
            "test case {a:e} * {b:e} is not exact"
        );
        if e.to_bits() != 0 {
            bad.push(format!(
                "two_prod({}, {}): exact product, expected e = +0, got {}",
                show(a),
                show(b),
                show(e)
            ));
        }
    }
    let mut g = Lcg(0x1111_0008);
    let mut minus = 0usize;
    let mut first = None;
    for _ in 0..20_000 {
        let bits = u32::try_from(g.range(1, 26)).unwrap();
        let a = dyadic(g.odd(bits), g.range(-40, 40));
        let b = with_exp(g.sign(), g.range(-900, 900), g.fraction());
        let (p, e) = two_prod(a, b);
        if D::of(a).mul(&D::of(b)) == D::of(p) && e.to_bits() != 0 {
            minus += 1;
            first.get_or_insert_with(|| {
                format!(
                    "two_prod({}, {}) = ({}, {})",
                    show(a),
                    show(b),
                    show(p),
                    show(e)
                )
            });
        }
    }
    if let Some(first) = first {
        bad.push(format!(
            "{minus} of 20000 random exact products gave e = -0, first {first}"
        ));
    }
    assert!(
        bad.is_empty(),
        "exact products whose error is not +0:\n{}",
        bad.join("\n")
    );
}

/// `fast_two_sum`'s documentation says its result equals `two_sum`'s. Bit for bit (the kernel's
/// determinism principle is bit-identical outputs), including the sign of a zero error. Value
/// differences and zero-sign differences are counted apart.
#[test]
fn fast_two_sum_equals_two_sum_bit_for_bit() {
    let mut pairs = vec![
        (1.0, -0.0),
        (-0.0, 1.0),
        (-0.0, -0.0),
        (0.0, -0.0),
        (-0.0, 0.0),
        (OMEGA, -0.0),
        (-p2(-1074), -0.0),
        (1.0, -1.0),
        (-1.0, 1.0),
    ];
    let mut g = Lcg(0x1212_0009);
    for _ in 0..50_000 {
        let a = g.finite();
        let b = match g.next() % 4 {
            0 => -0.0,
            1 => 0.0,
            2 => with_exp(g.sign(), g.range(-1074, 1023), g.fraction()),
            _ => g.finite(),
        };
        pairs.push((a, b));
    }
    let same = |x: f64, y: f64| x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan());
    let (mut value, mut zero_sign) = (Vec::new(), Vec::new());
    for &(a, b) in &pairs {
        let fast = fast_two_sum(a, b);
        let knuth = two_sum(a, b);
        if same(fast.0, knuth.0) && same(fast.1, knuth.1) {
            continue;
        }
        let line = format!(
            "({}, {}): fast_two_sum = ({}, {}), two_sum = ({}, {})",
            show(a),
            show(b),
            show(fast.0),
            show(fast.1),
            show(knuth.0),
            show(knuth.1)
        );
        if fast.0 == knuth.0 && fast.1 == knuth.1 {
            zero_sign.push(line);
        } else {
            value.push(line);
        }
    }
    assert!(
        value.is_empty() && zero_sign.is_empty(),
        "{} pairs differ in value, {} only in the sign of a zero; the first:\n{}\n{}",
        value.len(),
        zero_sign.len(),
        value.iter().take(4).cloned().collect::<Vec<_>>().join("\n"),
        zero_sign
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// `two_sum`'s exact error is never −0 (recorded so that a change to it shows).
#[test]
fn two_sum_zero_error_is_plus_zero() {
    let mut g = Lcg(0x1313_000a);
    for _ in 0..20_000 {
        let a = if g.next() % 5 == 0 { -0.0 } else { g.finite() };
        let b = if g.next() % 5 == 0 { -0.0 } else { g.finite() };
        let (_, e) = two_sum(a, b);
        assert!(
            e != 0.0 || e.to_bits() == 0,
            "two_sum({}, {}) has e = -0",
            show(a),
            show(b)
        );
    }
}

/// Sums against the oracle: near Ω (where Knuth's form overflows), in the subnormal range, at
/// every exponent gap, and random.
#[test]
fn sums_against_the_oracle() {
    let mut g = Lcg(0x1414_000b);
    let mut f = Failures::new("two_sum and fast_two_sum against the oracle");
    let push = |a: f64, b: f64, f: &mut Failures| {
        for (x, y) in [(a, b), (b, a), (-a, b), (a, -b)] {
            f.record(check_sum("two_sum", two_sum, x, y));
            f.record(check_sum("fast_two_sum", fast_two_sum, x, y));
        }
    };
    for i in 0..1000 {
        push(
            step(OMEGA, -i),
            with_exp(g.sign(), g.range(960, 1023), g.fraction()),
            &mut f,
        );
        let near = f64::from_bits(OMEGA.to_bits() - g.next() % (1 << 20));
        push(near, dyadic(g.range(-8, 8), 970), &mut f);
        let tiny_a = with_exp(g.sign(), g.range(-1074, -1000), g.next());
        let tiny_b = with_exp(g.sign(), g.range(-1074, -1000), g.next());
        push(tiny_a, tiny_b, &mut f);
    }
    for _ in 0..30_000 {
        let ea = g.range(-1074, 1023);
        let eb = (ea - g.range(-60, 60)).clamp(-1074, 1023);
        push(
            with_exp(g.sign(), ea, g.fraction()),
            with_exp(g.sign(), eb, g.fraction()),
            &mut f,
        );
        push(g.finite(), g.finite(), &mut f);
    }
    f.finish();
}

/// The two-sum overflow example in the documentation, numerically: for a = −3 · 2^970 and b = Ω,
/// s = Ω − 2^971 (a tie, rounded to even) and s − a is Ω + 2^970, which rounds to infinity; the
/// result must still be exact.
#[test]
fn two_sum_documented_overflow_example() {
    let a = dyadic(-3, 970);
    let b = OMEGA;
    let s = a + b;
    assert_eq!(
        D::of(s),
        D::of(OMEGA).sub(&D::pow2(971)),
        "s is not Ω - 2^971"
    );
    assert_eq!(
        s - a,
        f64::INFINITY,
        "the documented intermediate s - a does not overflow"
    );
    for (x, y) in [(a, b), (b, a), (-a, -b), (-b, -a)] {
        check_sum("two_sum", two_sum, x, y).unwrap();
        check_sum("fast_two_sum", fast_two_sum, x, y).unwrap();
    }
}

/// `split` at and around 2^996, where it switches from Veltkamp's four operations to the scaled
/// form, against Veltkamp's method in unbounded arithmetic: every double within 5000 of 2^996, of
/// both signs, and split-adversarial significands at exponents 990 to 1000.
#[test]
fn split_at_and_around_two_pow_996() {
    let mut g = Lcg(0x1515_000c);
    let mut f = Failures::new("split around 2^996");
    let mut x = step(p2(996), -5000);
    for _ in 0..10_001 {
        f.record(check_split(x));
        f.record(check_split(-x));
        x = up(x);
    }
    for _ in 0..20_000 {
        f.record(check_split(with_exp(
            g.sign(),
            g.range(990, 1000),
            g.fraction(),
        )));
    }
    f.finish();
}

/// `split`'s overflow frontier below Ω, against unbounded Veltkamp: every double within 5000 of
/// the threshold 2^1024 − 2^997 (where the high half first rounds to 2^1024) and of Ω, random
/// doubles in [2^1023, Ω], and the documented split of Ω, 2^1024 − 2^971. A factor whose split
/// overflows must still multiply exactly.
#[test]
fn split_overflow_frontier_below_omega() {
    let threshold = D::pow2(1024).sub(&D::pow2(997)).to_double().unwrap();
    let mut f = Failures::new("split near Ω");
    let mut x = step(threshold, -5000);
    for _ in 0..10_001 {
        f.record(check_split(x));
        f.record(check_split(-x));
        x = up(x);
    }
    let mut x = step(OMEGA, -5000);
    for _ in 0..5001 {
        f.record(check_split(x));
        x = up(x);
    }
    let mut g = Lcg(0x1616_000d);
    for _ in 0..20_000 {
        f.record(check_split(with_exp(g.sign(), 1023, g.next())));
    }
    // The threshold itself is the first reported value, its predecessor the last finite one.
    let (h, l) = split(threshold);
    let (h2, l2) = split(step(threshold, -1));
    f.record(
        if h == f64::INFINITY && l == f64::NEG_INFINITY && h2.is_finite() && l2.is_finite() {
            Ok(())
        } else {
            Err(format!(
                "split(2^1024 - 2^997) = ({h:e}, {l:e}), split(its predecessor) = ({h2:e}, {l2:e})"
            ))
        },
    );
    let (h, l) = veltkamp_unbounded(&D::of(OMEGA));
    f.record(if h == D::pow2(1024) && l == D::pow2(971).neg() {
        Ok(())
    } else {
        Err(format!(
            "Ω's halves are documented as 2^1024 and -2^971, the oracle gives {h} and {l}"
        ))
    });
    let factors = [
        0.5,
        p2(-1000),
        dyadic((1 << 53) - 1, -53),
        p2(-1074),
        dyadic(3, -1074),
        -1.0,
    ];
    for a in [OMEGA, threshold, step(threshold, 7)] {
        for &b in &factors {
            f.record(check_two_prod_all(a, b));
        }
    }
    f.finish();
}

/// `split` on subnormals at every leading-bit position, the normal/subnormal frontier, and random
/// doubles over the whole range.
#[test]
fn split_subnormals_and_the_whole_range() {
    let mut g = Lcg(0x1717_000e);
    let mut f = Failures::new("split over the whole range");
    for e in -1074..=-1020 {
        for _ in 0..200 {
            f.record(check_split(with_exp(g.sign(), e, g.fraction())));
        }
    }
    for _ in 0..50_000 {
        f.record(check_split(g.finite()));
        f.record(check_split(with_exp(
            g.sign(),
            g.range(-1074, 1023),
            g.fraction(),
        )));
    }
    f.finish();
}

/// The documented special values of `split`, and its overflow bound below 2^996 numerically: for
/// the largest double below 2^996, SPLITTER · a = 2^1023 + 2^996 − 2^971 stays finite, and the
/// last two of Veltkamp's operations are exact (Sterbenz's lemma).
#[test]
fn split_documented_values_and_bounds() {
    let (h, l) = split(OMEGA);
    assert!(
        h == f64::INFINITY && l == f64::NEG_INFINITY,
        "split(Ω) = ({h:e}, {l:e})"
    );
    let (h, l) = split(-OMEGA);
    assert!(
        h == f64::NEG_INFINITY && l == f64::INFINITY,
        "split(-Ω) = ({h:e}, {l:e})"
    );
    let (h, l) = split(0.0);
    assert!(
        h.to_bits() == 0 && l.to_bits() == 0,
        "split(+0) = ({}, {})",
        show(h),
        show(l)
    );
    let (h, l) = split(-0.0);
    assert!(
        h.to_bits() == (-0.0f64).to_bits() && l.to_bits() == 0,
        "split(-0) = ({}, {}), documented (-0, +0)",
        show(h),
        show(l)
    );
    for x in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        check_split(x).unwrap();
    }
    let below = step(p2(996), -1);
    let c = SPLITTER * below;
    assert!(c.is_finite(), "SPLITTER * pred(2^996) overflows");
    assert_eq!(
        D::of(c),
        D::pow2(1023).add(&D::pow2(996)).sub(&D::pow2(971))
    );
    let big = c - below;
    let hi = c - big;
    assert_eq!(D::of(hi), D::of(c).sub(&D::of(big)), "c - big is not exact");
    assert_eq!(
        D::of(below - hi),
        D::of(below).sub(&D::of(hi)),
        "a - hi is not exact"
    );
}

/// Inputs that are not finite, for every function: the documented `(s, −s)`, `(p, −p)` and
/// `(±∞, ∓∞)`, NaN only where IEEE makes the rounded result NaN; and overflow from finite inputs.
#[test]
fn non_finite_inputs() {
    let specials = [f64::INFINITY, f64::NEG_INFINITY, f64::NAN];
    let others = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        p2(-1074),
        OMEGA,
        -OMEGA,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ];
    let mut f = Failures::new("non-finite inputs");
    for &s in &specials {
        for &o in &others {
            for (a, b) in [(s, o), (o, s)] {
                f.record(check_two_prod(a, b));
                f.record(check_sum("two_sum", two_sum, a, b));
                f.record(check_sum("fast_two_sum", fast_two_sum, a, b));
            }
        }
        f.record(check_split(s));
    }
    for (a, b) in [
        (OMEGA, 2.0),
        (OMEGA, -1.5),
        (p2(1000), p2(100)),
        (OMEGA, OMEGA),
    ] {
        f.record(check_two_prod(a, b));
    }
    for (a, b) in [
        (OMEGA, OMEGA),
        (OMEGA, p2(971)),
        (-OMEGA, -p2(970)),
        (OMEGA, p2(970)),
    ] {
        f.record(check_sum("two_sum", two_sum, a, b));
        f.record(check_sum("fast_two_sum", fast_two_sum, a, b));
    }
    f.finish();
}

/// The two-sum proof, step by step: Knuth's six operations overflow (with a finite rounded sum)
/// only when |b| > |a|; when |a| ≥ |b|, `s − a` and `s − (s − a)` are exact and the latter is
/// exactly `a`. Swept along the overflow frontier (an odd multiple of 2^970 against ±Ω, which ties,
/// in both orders), near Ω, and at random.
#[test]
fn two_sum_knuth_steps_overflow_only_when_b_is_larger() {
    let mut g = Lcg(0x1919_0010);
    let mut f = Failures::new("two_sum proof steps");
    let mut overflowed = 0usize;
    for i in 0..60_000 {
        let (a, b) = match i % 3 {
            0 => {
                let odd = i64::try_from((g.next() % (1 << 52)) * 2 + 1).unwrap();
                let tie = dyadic(odd, 970);
                if g.sign() { (tie, OMEGA) } else { (OMEGA, tie) }
            }
            1 => {
                let big = f64::from_bits(OMEGA.to_bits() - g.next() % (1 << 12));
                let other = with_exp(g.sign(), g.range(940, 1023), g.fraction());
                if g.sign() { (big, other) } else { (other, big) }
            }
            _ => (g.finite(), g.finite()),
        };
        let (a, b) = if g.sign() { (-a, b) } else { (a, -b) };
        let s = a + b;
        if !s.is_finite() {
            continue;
        }
        let b_virtual = s - a;
        let a_virtual = s - b_virtual;
        let call = format!("two_sum({}, {})", show(a), show(b));
        let any_overflow = !b_virtual.is_finite() || !a_virtual.is_finite();
        if any_overflow {
            overflowed += 1;
        }
        f.record(if a.abs() >= b.abs() {
            if any_overflow
                || D::of(b_virtual) != D::of(s).sub(&D::of(a))
                || a_virtual.to_bits() != a.to_bits() && a_virtual != a
            {
                Err(format!(
                    "{call}: |a| >= |b| but s - a = {} and s - (s - a) = {}, documented exact \
                     and = a",
                    show(b_virtual),
                    show(a_virtual)
                ))
            } else {
                Ok(())
            }
        } else {
            Ok(())
        });
    }
    f.finish();
    assert!(overflowed > 0, "the sweep never made Knuth's form overflow");
}

/// The fast two-sum proof, step by step, with `x` the operand of larger magnitude: `z = s − x` is
/// exact, `|s|` lies in [|x|, 2|x|] (same signs) or [|x|/2, |x|] (or `s = x + y` exactly),
/// `e = y − z` is exact, a whole multiple of the quantum of `y` and at most `|y|`.
#[test]
fn fast_two_sum_proof_steps() {
    let mut g = Lcg(0x1a1a_0011);
    let mut f = Failures::new("fast_two_sum proof steps");
    for i in 0..60_000 {
        let (a, b) = match i % 3 {
            0 => (g.finite(), g.finite()),
            1 => {
                let ea = g.range(-1074, 1023);
                let eb = (ea - g.range(-2, 60)).clamp(-1074, 1023);
                (
                    with_exp(g.sign(), ea, g.fraction()),
                    with_exp(g.sign(), eb, g.fraction()),
                )
            }
            _ => (
                step(OMEGA, -g.range(0, 64)),
                with_exp(g.sign(), g.range(960, 1023), g.next()),
            ),
        };
        let (x, y) = if b.abs() > a.abs() { (b, a) } else { (a, b) };
        let s = x + y;
        if !s.is_finite() {
            continue;
        }
        let z = s - x;
        let e = y - z;
        let call = format!("fast_two_sum({}, {})", show(a), show(b));
        let (ds, dx, dy) = (D::of(s), D::of(x), D::of(y));
        let exact_sum = dx.add(&dy);
        let check = || -> Check {
            if D::of(z) != ds.sub(&dx) {
                return Err(format!("{call}: z = s - x = {} is not exact", show(z)));
            }
            if D::of(e) != dy.sub(&D::of(z)) {
                return Err(format!("{call}: e = y - z = {} is not exact", show(e)));
            }
            let same_sign = (x > 0.0) == (y > 0.0) || y == 0.0;
            let (lo, hi) = if same_sign {
                (dx.abs(), dx.abs().add(&dx.abs()))
            } else {
                (D::new(dx.abs().m.clone(), dx.e - 1), dx.abs())
            };
            if ds != exact_sum && (ds.cmp_abs(&lo) < 0 || ds.cmp_abs(&hi) > 0) {
                return Err(format!(
                    "{call}: |s| = {} is outside the documented interval",
                    show(s)
                ));
            }
            let quantum = if y == 0.0 {
                -1074
            } else {
                (D::of(y).highest() - 52).max(-1074)
            };
            if !D::of(e).is_zero() && (D::of(e).lowest() < quantum || D::of(e).cmp_abs(&dy) > 0) {
                return Err(format!(
                    "{call}: e = {} is not a multiple of u_y at most |y|",
                    show(e)
                ));
            }
            Ok(())
        };
        f.record(check());
    }
    f.finish();
}

/// The direct path's lower limit, numerically. The documentation derives `e_a` + `e_b` ≥ −970 from
/// |p| ≥ 2^−968; the bound holds but is one short: with `e_a` + `e_b` = −970 the largest product,
/// (2 − 2^−52)^2 · 2^−970, rounds to the double below 2^−968, so the direct path never sees −970
/// and its error grid is never finer than 2^−1073. Recorded so a change to the limit shows.
#[test]
fn direct_path_lower_limit_is_one_exponent_loose() {
    let top = with_exp(false, -485, FRAC);
    let other = with_exp(false, -485, FRAC);
    assert_eq!(D::of(top).highest() + D::of(other).highest(), -970);
    let p = top * other;
    assert!(
        p < p2(-968),
        "(2 - 2^-52)^2 * 2^-970 rounds to {}, not below 2^-968",
        show(p)
    );
    assert!(!in_direct_range(top, other));
    // At −969 the direct path is reached: 1.5 · 1.5 · 2^−969 = 1.125 · 2^−968.
    let a = with_exp(false, -484, 1 << 51);
    let b = with_exp(false, -485, 1 << 51);
    assert!(
        in_direct_range(a, b),
        "1.5 * 1.5 * 2^-969 should be in the direct range"
    );
    check_dekker_steps(a, b).unwrap();
}
