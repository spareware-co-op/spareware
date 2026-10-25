//! Adversarial tests for piece G05 (issue #174): the correctly rounded `sinpi`, `cospi`, `exp`,
//! `ln` and `atan2` of `geom_num::math`.
//!
//! Every test here tries to show the piece wrong. The reference is this file's own, written
//! independently of the builder's `math_reference.rs`: exact big-integer fixed-point arithmetic
//! with other formulas and reductions (Gauss's formula for π, checked against Machin's; a
//! three-term arccoth formula for ln 2, checked against `2·arccoth 3`; argument halving with
//! integer square roots for `atan`; Taylor and atanh series with explicit error budgets). A result
//! is accepted only when both ends of a rigorous interval round to the same double; otherwise the
//! precision doubles.
//!
//! What is attacked:
//!
//! - correct rounding, bit for bit, on random arguments over every binade and on families built to
//!   sit near rounding boundaries or on the slow paths (near integers and half-integers, every
//!   large-argument table entry, tiny arguments of `exp`, `ln` near 1 and at the top of every
//!   binade, subnormal midpoints of `atan2`);
//! - the edges of every code path and threshold in the ports;
//! - C99 Annex F special values and signs of zero, under the functions' contract (`geom_num::math`:
//!   a `Real` or an error, never NaN; a zero `sinpi` or `cospi` is +0, the crate's one-zero rule);
//! - identities that correct rounding forces exactly (odd and even symmetry, periodicity, the
//!   scale invariance of `atan2`) and monotonicity;
//! - determinism across thread counts, and a golden hash to compare across targets;
//! - the crate's written rules and the chapters' requirements, read from the source.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failing test should stop loudly"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "values are named a, b, x, y and so on, as in the formulas they check"
)]
// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); test code may
// use them, and std's maxima and minima.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]

use geom_num::{NumError, Real, math};

/// What a public maths function owes for arguments `args` whose IEEE 754 value (correctly
/// rounded, with C99 Annex F's special values) is `want`, under its contract (`geom_num::math`):
/// an argument that is not finite is refused as `NotFinite`, checked first; a value that is not a
/// real number (NaN) is refused as `OutsideDomain`; an infinite one (an overflow or a pole) as
/// `NotFinite`; otherwise the value itself.
fn owed(args: &[f64], want: f64) -> Result<f64, NumError> {
    if args.iter().any(|a| !a.is_finite()) {
        Err(NumError::NotFinite)
    } else if want.is_nan() {
        Err(NumError::OutsideDomain)
    } else if want.is_infinite() {
        Err(NumError::NotFinite)
    } else {
        Ok(want)
    }
}

/// A result or an owed outcome as comparable bits: the value's bits, or the error.
fn bits(r: Result<f64, NumError>) -> Result<u64, NumError> {
    r.map(f64::to_bits)
}

/// The value of a result that must exist.
#[track_caller]
fn v(r: Result<Real, NumError>) -> f64 {
    r.unwrap().get()
}

/// Asserts that a public function's result is what it owes, bit for bit.
#[track_caller]
fn check_owed(got: Result<Real, NumError>, args: &[f64], want: f64, what: &str) {
    assert_eq!(
        bits(got.map(Real::get)),
        bits(owed(args, want)),
        "{what}: got {got:?}, want {:?}",
        owed(args, want)
    );
}

// ---------------------------------------------------------------------------------------------
// The reference
// ---------------------------------------------------------------------------------------------

/// An exact reference, independent of the code under test and of the builder's oracle.
mod oracle {
    use std::sync::OnceLock;

    use num_bigint::BigInt;
    use num_integer::Integer;
    use num_traits::{One, Signed, ToPrimitive, Zero};

    /// Working precisions, in bits after the binary point; each failure to decide doubles it.
    const LADDER: [u64; 6] = [192, 384, 768, 1536, 3072, 6144];
    /// Bits carried by the cached constants.
    const QMAX: u64 = 6144 + 8;
    /// Guard bits for the constants' series (their truncation errors are below 2^20 units).
    const GUARD: u64 = 64;

    /// `2^n`.
    pub fn pow2(n: u64) -> BigInt {
        BigInt::one() << n
    }

    /// `atan(1/n)·2^w`, each term truncated: error below `2·terms` units.
    fn arctan_inv(n: u64, w: u64) -> BigInt {
        let n = BigInt::from(n);
        let n2 = &n * &n;
        let mut power = pow2(w) / &n;
        let mut sum = power.clone();
        let mut k = 1u64;
        loop {
            power /= &n2;
            if power.is_zero() {
                return sum;
            }
            let term = &power / BigInt::from(2 * k + 1);
            if k % 2 == 1 {
                sum -= term;
            } else {
                sum += term;
            }
            k += 1;
        }
    }

    /// `arccoth(n)·2^w = atanh(1/n)·2^w`, each term truncated: error below `2·terms` units.
    fn arccoth(n: u64, w: u64) -> BigInt {
        let n = BigInt::from(n);
        let n2 = &n * &n;
        let mut power = pow2(w) / &n;
        let mut sum = power.clone();
        let mut k = 1u64;
        loop {
            power /= &n2;
            if power.is_zero() {
                return sum;
            }
            sum += &power / BigInt::from(2 * k + 1);
            k += 1;
        }
    }

    /// `⌊π·2^bits⌋` (within one unit) by Gauss's formula
    /// `π = 48·atan(1/18) + 32·atan(1/57) − 20·atan(1/239)`.
    pub fn pi_gauss(bits: u64) -> BigInt {
        let w = bits + GUARD;
        (BigInt::from(48) * arctan_inv(18, w) + BigInt::from(32) * arctan_inv(57, w)
            - BigInt::from(20) * arctan_inv(239, w))
            >> GUARD
    }

    /// `π·2^bits` by Machin's formula `π = 16·atan(1/5) − 4·atan(1/239)`, for the
    /// self-check.
    pub fn pi_machin(bits: u64) -> BigInt {
        let w = bits + GUARD;
        (BigInt::from(16) * arctan_inv(5, w) - BigInt::from(4) * arctan_inv(239, w)) >> GUARD
    }

    /// `ln 2·2^bits` by `ln 2 = 18·arccoth 26 − 2·arccoth 4801 + 8·arccoth 8749`.
    pub fn ln2_three_terms(bits: u64) -> BigInt {
        let w = bits + GUARD;
        (BigInt::from(18) * arccoth(26, w) - BigInt::from(2) * arccoth(4801, w)
            + BigInt::from(8) * arccoth(8749, w))
            >> GUARD
    }

    /// `ln 2·2^bits` by `ln 2 = 2·arccoth 3`, for the self-check.
    pub fn ln2_arccoth3(bits: u64) -> BigInt {
        let w = bits + GUARD;
        (BigInt::from(2) * arccoth(3, w)) >> GUARD
    }

    fn pi_max() -> &'static BigInt {
        static C: OnceLock<BigInt> = OnceLock::new();
        C.get_or_init(|| pi_gauss(QMAX))
    }

    fn ln2_max() -> &'static BigInt {
        static C: OnceLock<BigInt> = OnceLock::new();
        C.get_or_init(|| ln2_three_terms(QMAX))
    }

    /// `π·2^q`, within 2 units.
    fn pi_at(q: u64) -> BigInt {
        pi_max() >> (QMAX - q)
    }

    /// `ln 2·2^q`, within 2 units.
    fn ln2_at(q: u64) -> BigInt {
        ln2_max() >> (QMAX - q)
    }

    /// `n·2^−sh` rounded to the nearest binary64 number, ties to even, with gradual underflow
    /// and overflow to infinity.
    pub fn round(n: &BigInt, sh: i64) -> f64 {
        if n.is_zero() {
            return 0.0;
        }
        let negative = n.is_negative();
        let mag = n.abs();
        let e = mag.bits() as i64 - 1 - sh; // 2^e ≤ |value| < 2^(e+1)
        let lsb = if e >= -1022 { e - 52 } else { -1074 };
        let drop = lsb + sh;
        let q = if drop <= 0 {
            mag << ((-drop) as u64)
        } else {
            let d = drop as u64;
            let q = &mag >> d;
            let rem = &mag - (&q << d);
            let half = pow2(d - 1);
            if rem > half || (rem == half && q.is_odd()) {
                q + 1
            } else {
                q
            }
        };
        let q = q.to_u64().unwrap(); // at most 2^53
        // value = q·2^lsb; this bit pattern is right for subnormal and normal q, and carries into
        // the exponent when q = 2^53.
        let bits = ((lsb + 1074) as u128) * (1u128 << 52) + u128::from(q);
        let v = if bits >= 0x7ff0_0000_0000_0000 {
            f64::INFINITY
        } else {
            f64::from_bits(bits as u64)
        };
        if negative { -v } else { v }
    }

    /// The double both ends round to, if they agree.
    fn decide(lo: &BigInt, hi: &BigInt, sh: i64) -> Option<f64> {
        let a = round(lo, sh);
        let b = round(hi, sh);
        (a.to_bits() == b.to_bits()).then_some(a)
    }

    fn escalate(what: &dyn Fn() -> String, f: impl Fn(u64) -> Option<f64>) -> f64 {
        for q in LADDER {
            if let Some(v) = f(q) {
                return v;
            }
        }
        panic!("oracle could not decide {}", what())
    }

    /// `x = m·2^e` exactly, with `m` odd (or zero).
    pub fn decompose(x: f64) -> (BigInt, i64) {
        let bits = x.to_bits();
        let negative = bits >> 63 != 0;
        let biased = ((bits >> 52) & 0x7ff) as i64;
        let frac = bits & ((1 << 52) - 1);
        let (mut m, mut e) = if biased == 0 {
            (frac, -1074)
        } else {
            (frac | (1 << 52), biased - 1075)
        };
        if m != 0 {
            let tz = m.trailing_zeros();
            m >>= tz;
            e += i64::from(tz);
        }
        let m = BigInt::from(m);
        (if negative { -m } else { m }, e)
    }

    /// `sin(π·r/2^n)` for `0 < r/2^n ≤ 1/4`, as `[lo, hi]·2^−sh` with relative width about
    /// `2^−q`.
    fn sin_pi_small(r: &BigInt, n: u64, q: u64) -> (BigInt, BigInt, i64) {
        let p = pi_at(q);
        // θ²·2^q within 2 units (θ = πs, s ≤ 1/4).
        let theta2 = (&p * &p * r * r) >> (q + 2 * n);
        // sin θ / θ = Σ (−1)^k θ^(2k)/(2k+1)!, each term within 1.6 units.
        let one = pow2(q);
        let mut term = one.clone();
        let mut sum = one;
        let mut k = 0u64;
        loop {
            k += 1;
            term = (&term * &theta2) / (BigInt::from((2 * k) * (2 * k + 1)) << q);
            if term.is_zero() {
                break;
            }
            if k % 2 == 1 {
                sum -= &term;
            } else {
                sum += &term;
            }
        }
        let err_sinc = BigInt::from(2 * k + 4);
        // sin θ = π·s·sinc: |error| ≤ s·(2^−q·2 + 4·err_sinc·2^−q).
        let center = &p * r * &sum;
        let err = r * pow2(q) * (BigInt::from(2) + BigInt::from(4) * err_sinc);
        (&center - &err, &center + &err, (2 * q + n) as i64)
    }

    /// `cos(π·t/2^n)` for `0 < t/2^n < 1/4`, as `[lo, hi]·2^−q`.
    fn cos_pi_small(t: &BigInt, n: u64, q: u64) -> (BigInt, BigInt) {
        let p = pi_at(q);
        let theta2 = (&p * &p * t * t) >> (q + 2 * n);
        let one = pow2(q);
        let mut term = one.clone();
        let mut sum = one;
        let mut k = 0u64;
        loop {
            k += 1;
            term = (&term * &theta2) / (BigInt::from((2 * k - 1) * (2 * k)) << q);
            if term.is_zero() {
                break;
            }
            if k % 2 == 1 {
                sum -= &term;
            } else {
                sum += &term;
            }
        }
        let err = BigInt::from(4 * k + 8);
        (&sum - &err, &sum + &err)
    }

    /// `sin(π·r/2^n)` for an odd `r` in `[0, 2^(n+1))`, `n ≥ 2`.
    fn sin_reduced(r: &BigInt, n: u64, q: u64) -> Option<f64> {
        let one = pow2(n);
        let half = pow2(n - 1);
        let quarter = pow2(n - 2);
        let mut r = r.clone();
        let mut negative = false;
        if r >= one {
            negative = true; // sin(π(s + 1)) = −sin(πs)
            r -= &one;
        }
        if r > half {
            r = &one - &r; // sin(π(1 − s)) = sin(πs)
        }
        let v = if r <= quarter {
            let (lo, hi, sh) = sin_pi_small(&r, n, q);
            decide(&lo, &hi, sh)?
        } else {
            let (lo, hi) = cos_pi_small(&(&half - &r), n, q);
            decide(&lo, &hi, q as i64)?
        };
        Some(if negative { -v } else { v })
    }

    /// `sin(πx)` correctly rounded, with IEEE 754-2019 §9.2.1's exact values and signs.
    pub fn sinpi(x: f64) -> f64 {
        assert!(x.is_finite());
        if x == 0.0 {
            return x;
        }
        let (m, e) = decompose(x);
        if e >= 0 {
            return if x > 0.0 { 0.0 } else { -0.0 };
        }
        if e == -1 {
            return if m.mod_floor(&BigInt::from(4)) == BigInt::one() {
                1.0
            } else {
                -1.0
            };
        }
        let n = (-e) as u64;
        let r = m.mod_floor(&pow2(n + 1));
        escalate(&|| format!("sinpi({x:e})"), |q| sin_reduced(&r, n, q))
    }

    /// `cos(πx)` correctly rounded, with IEEE 754-2019 §9.2.1's exact values and signs.
    pub fn cospi(x: f64) -> f64 {
        assert!(x.is_finite());
        if x == 0.0 {
            return 1.0;
        }
        let (m, e) = decompose(x);
        if e >= 1 {
            return 1.0;
        }
        if e == 0 {
            return -1.0;
        }
        if e == -1 {
            return 0.0;
        }
        let n = (-e) as u64;
        let r = (m + pow2(n - 1)).mod_floor(&pow2(n + 1)); // cos(πx) = sin(π(x + ½))
        escalate(&|| format!("cospi({x:e})"), |q| sin_reduced(&r, n, q))
    }

    /// `e^x` correctly rounded.
    pub fn exp(x: f64) -> f64 {
        assert!(!x.is_nan());
        if x == 0.0 {
            return 1.0;
        }
        if x > 1000.0 {
            return f64::INFINITY;
        }
        if x < -1000.0 {
            return 0.0;
        }
        let (m, e) = decompose(x);
        escalate(&|| format!("exp({x:e})"), |q| {
            let k = (x * core::f64::consts::LOG2_E).round() as i64;
            let shift = e + q as i64;
            let (xq, err_x) = if shift >= 0 {
                (&m << shift as u64, 0u64)
            } else {
                (m.div_floor(&pow2((-shift) as u64)), 1)
            };
            let r = xq - BigInt::from(k) * ln2_at(q); // |r| ≤ 0.35·2^q
            let err_r = err_x + 2 * k.unsigned_abs();
            let mut sum = pow2(q);
            let mut term = pow2(q);
            let mut i = 0u64;
            loop {
                i += 1;
                term = (&term * &r) / (BigInt::from(i) << q);
                if term.is_zero() {
                    break;
                }
                sum += &term;
            }
            let err = BigInt::from(2 * i + 6 + 3 * err_r);
            decide(&(&sum - &err), &(&sum + &err), q as i64 - k)
        })
    }

    /// `ln x` correctly rounded, for finite `x > 0`.
    pub fn ln(x: f64) -> f64 {
        assert!(x > 0.0 && x.is_finite());
        if x == 1.0 {
            return 0.0;
        }
        let (m, e) = decompose(x);
        escalate(&|| format!("ln({x:e})"), |q| {
            let b = m.bits() as i64;
            let mut big_e = e + b - 1;
            let mut den = pow2((b - 1) as u64);
            if &m * 2 >= &den * 3 {
                den <<= 1u32;
                big_e += 1;
            }
            // x = (m/den)·2^big_e with m/den in [0.75, 1.5); ln(m/den) = 2·atanh(t).
            let t = ((&m - &den) << q).div_floor(&(&m + &den));
            let t2 = (&t * &t) >> q;
            let mut sum = t.clone();
            let mut power = t;
            let mut k = 0u64;
            loop {
                k += 1;
                power = (&power * &t2) >> q;
                let term = &power / BigInt::from(2 * k + 1);
                if term.is_zero() {
                    break;
                }
                sum += term;
            }
            let center = BigInt::from(big_e) * ln2_at(q) + (sum << 1u32);
            let err = BigInt::from(2 * big_e.unsigned_abs() + 6 * k + 16);
            decide(&(&center - &err), &(&center + &err), q as i64)
        })
    }

    /// `atan(num/den)·2^q` and its error, for `0 < num ≤ den`, by halving the angle until the
    /// argument is at most 1/8 (`atan v = 2·atan(v/(1 + √(1 + v²)))`), then the alternating
    /// series.
    fn atan_abs(num: &BigInt, den: &BigInt, q: u64) -> (BigInt, BigInt) {
        let one = pow2(q);
        let eighth = pow2(q - 3);
        let mut v = (num << q).div_floor(den);
        let mut err_v = 1u64;
        let mut halvings = 0u64;
        while v > eighth {
            let s = (&one * &one + &v * &v).sqrt();
            v = (&v << q) / (&one + &s);
            err_v += 2;
            halvings += 1;
        }
        let t2 = (&v * &v) >> q;
        let mut sum = v.clone();
        let mut power = v;
        let mut k = 0u64;
        loop {
            k += 1;
            power = (&power * &t2) >> q;
            let term = &power / BigInt::from(2 * k + 1);
            if term.is_zero() {
                break;
            }
            if k % 2 == 1 {
                sum -= term;
            } else {
                sum += term;
            }
        }
        let err = BigInt::from(2 * err_v + 3 * k + 8) << halvings;
        (sum << halvings, err)
    }

    /// `atan(num/den)` for `0 < num ≤ den` as `[lo, hi]·2^−sh`, with relative precision for
    /// tiny quotients: `atan q = q·Σ (−1)^k q^(2k)/(2k+1)`.
    fn atan_rel(num: &BigInt, den: &BigInt, q: u64) -> (BigInt, BigInt, i64) {
        if num * 8 > *den {
            let (c, err) = atan_abs(num, den, q);
            return (&c - &err, &c + &err, q as i64);
        }
        let t2 = ((num * num) << q).div_floor(&(den * den));
        let mut sum = pow2(q);
        let mut power = pow2(q);
        let mut k = 0u64;
        loop {
            k += 1;
            power = (&power * &t2) >> q;
            let term = &power / BigInt::from(2 * k + 1);
            if term.is_zero() {
                break;
            }
            if k % 2 == 1 {
                sum -= term;
            } else {
                sum += term;
            }
        }
        let err = BigInt::from(2 * k + 4);
        let w = (den.bits() + 64).saturating_sub(num.bits());
        let lo = ((num * (&sum - &err)) << w).div_floor(den);
        let hi = -((-(num * (&sum + &err)) << w).div_floor(den));
        (lo, hi, (q + w) as i64)
    }

    /// The angle of `(x, y)` correctly rounded, for finite non-zero `x` and `y`.
    pub fn atan2(y: f64, x: f64) -> f64 {
        assert!(y.is_finite() && x.is_finite() && y != 0.0 && x != 0.0);
        let (my, ey) = decompose(y.abs());
        let (mx, ex) = decompose(x.abs());
        let emin = ey.min(ex);
        let a = my << (ey - emin) as u64;
        let b = mx << (ex - emin) as u64;
        let x_negative = x < 0.0;
        let v = escalate(&|| format!("atan2({y:e}, {x:e})"), |q| {
            let swap = a > b;
            let (num, den) = if swap { (&b, &a) } else { (&a, &b) };
            if !swap && !x_negative {
                let (lo, hi, sh) = atan_rel(num, den, q);
                return decide(&lo, &hi, sh);
            }
            let (c, err) = atan_abs(num, den, q);
            let center = if swap && !x_negative {
                pi_at(q - 1) - c // π/2 − atan(x/y)
            } else if !swap {
                pi_at(q) - c // π − atan(y/x)
            } else {
                pi_at(q - 1) + c // π/2 + atan(x/y)
            };
            let err = err + 4;
            decide(&(&center - &err), &(&center + &err), q as i64)
        });
        if y < 0.0 { -v } else { v }
    }
}

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

/// `SplitMix64`: a deterministic generator for test inputs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `[lo, hi]`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i64
    }

    /// Uniform in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    fn sign(&mut self) -> f64 {
        if self.next() & 1 == 0 { 1.0 } else { -1.0 }
    }

    /// A random positive double with unbiased exponent `e` (subnormal below −1022).
    fn binade(&mut self, e: i64) -> f64 {
        let frac = self.next() & ((1 << 52) - 1);
        if e < -1022 {
            let shift = (-1022 - e).min(52) as u32;
            f64::from_bits((frac | (1 << 52)) >> shift).max(f64::from_bits(1))
        } else {
            f64::from_bits((((e + 1023) as u64) << 52) | frac)
        }
    }

    /// A random positive double with an exponent drawn uniformly from `[lo, hi]`.
    fn binade_in(&mut self, lo: i64, hi: i64) -> f64 {
        let e = self.range(lo, hi);
        self.binade(e)
    }

    /// A random finite double, uniform over bit patterns.
    fn finite(&mut self) -> f64 {
        loop {
            let v = f64::from_bits(self.next());
            if v.is_finite() {
                return v;
            }
        }
    }
}

/// Collects mismatches against the reference and reports them all at once.
struct Report {
    what: &'static str,
    checked: usize,
    failures: Vec<String>,
}

impl Report {
    fn new(what: &'static str) -> Self {
        Report {
            what,
            checked: 0,
            failures: Vec::new(),
        }
    }

    /// Compares a public function's result with what it owes (see [`owed`]), bit for bit.
    fn same(
        &mut self,
        call: impl Fn() -> String,
        got: Result<Real, NumError>,
        want: Result<f64, NumError>,
    ) {
        self.checked += 1;
        let got = got.map(Real::get);
        if bits(got) != bits(want) {
            self.failures.push(format!(
                "{}: got {got:?} ({:?}), correctly rounded {want:?} ({:?})",
                call(),
                bits(got),
                bits(want)
            ));
        }
    }

    /// `sinpi(x)`; the reference's zero has IEEE 754-2019's sign, the public function's is +0,
    /// which adding +0 to the reference's value gives (and changes nothing else).
    fn sinpi(&mut self, x: f64) {
        self.same(
            || format!("sinpi({x:e} = {:#018x})", x.to_bits()),
            math::sinpi(x),
            owed(&[x], oracle::sinpi(x) + 0.0),
        );
    }

    /// `cospi(x)`, a zero as +0 as for `sinpi`.
    fn cospi(&mut self, x: f64) {
        self.same(
            || format!("cospi({x:e} = {:#018x})", x.to_bits()),
            math::cospi(x),
            owed(&[x], oracle::cospi(x) + 0.0),
        );
    }

    fn exp(&mut self, x: f64) {
        self.same(
            || format!("exp({x:e} = {:#018x})", x.to_bits()),
            math::exp(x),
            owed(&[x], oracle::exp(x)),
        );
    }

    fn ln(&mut self, x: f64) {
        self.same(
            || format!("ln({x:e} = {:#018x})", x.to_bits()),
            math::ln(x),
            owed(&[x], oracle::ln(x)),
        );
    }

    fn atan2(&mut self, y: f64, x: f64) {
        self.same(
            || {
                format!(
                    "atan2({y:e}, {x:e}) = atan2({:#018x}, {:#018x})",
                    y.to_bits(),
                    x.to_bits()
                )
            },
            math::atan2(y, x),
            owed(&[y, x], oracle::atan2(y, x)),
        );
    }

    fn finish(self) {
        let n = self.failures.len();
        let shown: Vec<&String> = self.failures.iter().take(40).collect();
        assert!(
            n == 0,
            "{}: {n} of {} results are not correctly rounded:\n{}",
            self.what,
            self.checked,
            shown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
        eprintln!(
            "{}: {} results, all correctly rounded",
            self.what, self.checked
        );
    }
}

/// How many random cases to try: fewer in debug builds, where overflow checks are on.
fn scale(n: usize) -> usize {
    if cfg!(debug_assertions) {
        (n / 20).max(20)
    } else {
        n
    }
}

/// A C99 hexadecimal literal that is exactly a double, read by the test's own code (not the
/// kernel's reader); panics if the literal is not exactly representable.
fn hf(s: &str) -> f64 {
    use num_bigint::BigInt;
    let (neg, body) = s.strip_prefix('-').map_or((false, s), |t| (true, t));
    let body = body.strip_prefix("0x").unwrap();
    let (mant, exp) = body.split_once('p').unwrap();
    let (int, frac) = mant.split_once('.').unwrap_or((mant, ""));
    let m = u128::from_str_radix(&format!("{int}{frac}"), 16).unwrap();
    let e = i64::from(exp.parse::<i32>().unwrap()) - 4 * frac.len() as i64;
    let v = oracle::round(&BigInt::from(m), -e);
    if m != 0 {
        // v = mm·2^ee must equal m·2^e exactly.
        let (mm, ee) = oracle::decompose(v);
        let low = e.min(ee);
        assert_eq!(
            BigInt::from(m) << (e - low) as u64,
            mm << (ee - low) as u64,
            "{s} is not a double"
        );
    }
    if neg { -v } else { v }
}

/// `2^k` exactly, for `−1074 ≤ k ≤ 1023` (`powi` loses the subnormal powers).
fn p2(k: i32) -> f64 {
    assert!((-1074..=1023).contains(&k), "2^{k} is not a double");
    if k >= -1022 {
        f64::from_bits(((k + 1023) as u64) << 52)
    } else {
        f64::from_bits(1u64 << (k + 1074))
    }
}

// ---------------------------------------------------------------------------------------------
// 1. The reference checks itself
// ---------------------------------------------------------------------------------------------

#[test]
fn oracle_constants_agree_across_formulas_and_with_published_digits() {
    use num_bigint::BigInt;
    let bits = 6200;
    let a = oracle::pi_gauss(bits);
    let b = oracle::pi_machin(bits);
    assert!(
        (&a - &b).magnitude() <= &num_bigint::BigUint::from(2u32),
        "Gauss and Machin disagree"
    );
    let c = oracle::ln2_three_terms(bits);
    let d = oracle::ln2_arccoth3(bits);
    assert!(
        (&c - &d).magnitude() <= &num_bigint::BigUint::from(2u32),
        "the ln 2 formulas disagree"
    );
    // Published hexadecimal digits: π = 3.243F6A8885A308D313198A2E03707344…,
    // ln 2 = 0.B17217F7D1CF79ABC9E3B39803F2F6AF….
    let pi128 = oracle::pi_gauss(128);
    let want = BigInt::parse_bytes(b"3243F6A8885A308D313198A2E03707344", 16).unwrap();
    assert_eq!(pi128, want);
    let ln128 = oracle::ln2_three_terms(128);
    let want = BigInt::parse_bytes(b"B17217F7D1CF79ABC9E3B39803F2F6AF", 16).unwrap();
    assert_eq!(ln128, want);
}

#[test]
fn oracle_reproduces_known_correctly_rounded_values() {
    use core::f64::consts::{E, FRAC_1_SQRT_2, FRAC_PI_2, FRAC_PI_4, LN_2, LN_10, PI};
    // Rust's constants are the doubles nearest the true values; the decimal ones were checked
    // with Python's correctly rounded `decimal` module.
    let cases: [(f64, f64); 16] = [
        (oracle::exp(1.0), E),
        (oracle::exp(-1.0), 0.367_879_441_171_442_33),
        (oracle::ln(2.0), LN_2),
        (oracle::ln(10.0), LN_10),
        (oracle::sinpi(0.25), FRAC_1_SQRT_2),
        (oracle::cospi(0.25), FRAC_1_SQRT_2),
        (oracle::sinpi(1.0 / 6.0), 0.5),
        (oracle::atan2(1.0, 1.0), FRAC_PI_4),
        (oracle::atan2(-1.0, -1.0), -3.0 * FRAC_PI_4),
        (oracle::atan2(1.0, 2.0), 0.463_647_609_000_806_1),
        (oracle::atan2(1.0, 1e-300), FRAC_PI_2),
        (oracle::atan2(1.0, -1e-300), FRAC_PI_2),
        (oracle::atan2(1e-300, -1.0), PI),
        (oracle::exp(-745.0), f64::from_bits(1)),
        (oracle::exp(709.0), 8.218_407_461_554_972e307),
        (oracle::sinpi(f64::from_bits(1)), f64::from_bits(3)),
    ];
    for (i, (got, want)) in cases.iter().enumerate() {
        assert_eq!(
            got.to_bits(),
            want.to_bits(),
            "case {i}: {got:e} vs {want:e}"
        );
    }
}

#[test]
fn oracle_has_teeth_it_catches_the_misroundings_of_libm() {
    // The deterministic but unproven `libm` functions are accurate to about an ulp, not correctly
    // rounded: a reference that cannot tell them apart from the ports would prove nothing.
    let mut rng = Rng(0x5eed_7eef);
    let (mut exp_bad, mut ln_bad, mut atan2_bad, mut sinpi_bad) = (0usize, 0usize, 0usize, 0usize);
    let n = scale(20_000);
    for _ in 0..n {
        let x = -700.0 + rng.unit() * 1400.0;
        exp_bad += usize::from(v(math::unproven::exp(x)).to_bits() != oracle::exp(x).to_bits());
        let y = rng.binade_in(-1000, 1000);
        ln_bad += usize::from(v(math::unproven::ln(y)).to_bits() != oracle::ln(y).to_bits());
        let (a, b) = (
            rng.sign() * rng.binade_in(-10, 10),
            rng.sign() * rng.binade_in(-10, 10),
        );
        atan2_bad +=
            usize::from(v(math::unproven::atan2(a, b)).to_bits() != oracle::atan2(a, b).to_bits());
        let z = rng.unit() * 2.0;
        sinpi_bad +=
            usize::from(v(math::unproven::sinpi(z)).to_bits() != oracle::sinpi(z).to_bits());
    }
    eprintln!(
        "libm misroundings in {n} samples: exp {exp_bad}, ln {ln_bad}, atan2 {atan2_bad}, sinpi {sinpi_bad}"
    );
    assert!(
        sinpi_bad > 0 && exp_bad + ln_bad + atan2_bad > 0,
        "the reference never disagrees with libm"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Correct rounding against the reference
// ---------------------------------------------------------------------------------------------

#[test]
fn sinpi_and_cospi_random_over_every_binade() {
    let mut rng = Rng(0x5eed_0001);
    let mut r = Report::new("sinpi/cospi, every binade");
    let per = scale(30);
    for e in -1074..=60i64 {
        for _ in 0..per {
            let x = rng.sign() * rng.binade(e);
            r.sinpi(x);
            r.cospi(x);
        }
    }
    for _ in 0..scale(30_000) {
        let x = rng.sign() * rng.unit() * 4.0;
        r.sinpi(x);
        r.cospi(x);
    }
    r.finish();
}

#[test]
fn sinpi_and_cospi_near_integers_and_half_integers() {
    // Results near 0 (the accurate path at table indices 0 and 2048) and near ±1.
    let mut rng = Rng(0x5eed_0002);
    let mut r = Report::new("sinpi/cospi near integers and half-integers");
    for _ in 0..scale(60_000) {
        let n = match rng.next() % 4 {
            0 => rng.range(0, 8),
            1 => rng.range(0, 1 << 20),
            2 => rng.range(0, 1 << 39),
            _ => rng.range((1 << 39) - 64, (1 << 40) + 64),
        } as f64;
        let k = rng.range(1, 70);
        let delta = rng.sign() * rng.binade(-k);
        let half = if rng.next() & 1 == 0 { 0.0 } else { 0.5 };
        let x = rng.sign() * (n + half + delta);
        r.sinpi(x);
        r.cospi(x);
    }
    r.finish();
}

#[test]
fn sinpi_and_cospi_dense_on_the_near_zero_paths() {
    // The near-zero fast paths fall back to their own accurate paths (`sinpi_near_zero`,
    // `cospi_near_zero`) only rarely; sample their ranges densely.
    let mut rng = Rng(0x5eed_00a1);
    let mut r = Report::new("sinpi/cospi near-zero paths");
    let (s_lo, s_hi) = (p2(-54), hf("0x1.2p-5"));
    let (c_lo, c_hi) = (hf("0x1.ccf6429be6621p-29"), p2(-12));
    for _ in 0..scale(100_000) {
        let t = rng.unit();
        let x = s_lo * (s_hi / s_lo).powf(t); // log-uniform
        r.sinpi(rng.sign() * x);
        let y = c_lo * (c_hi / c_lo).powf(rng.unit());
        r.cospi(rng.sign() * y);
    }
    r.finish();
}

#[test]
fn every_large_argument_table_entry() {
    // For |x| ≥ 2^41 the result is a table entry rounded with no test; x = 2^41 + j/2^11 reaches
    // every index j, with both signs and both functions.
    let mut r = Report::new("sinpi/cospi table entries for |x| ≥ 2^41");
    let base = p2(41);
    for j in 0..4096 {
        let x = base + f64::from(j) / 2048.0;
        for x in [x, -x, x + 1.0, -(x + 1.0)] {
            r.sinpi(x);
            r.cospi(x);
        }
    }
    // And the largest non-integers: 2^52 − ½ and around.
    for x in [p2(52) - 0.5, p2(52) - 1.5, p2(51) + 0.5, p2(42) + 0.25] {
        r.sinpi(x);
        r.cospi(x);
        r.sinpi(-x);
        r.cospi(-x);
    }
    r.finish();
}

/// `n` doubles on each side of `x`, and `x`.
fn around(x: f64, n: usize) -> Vec<f64> {
    let mut v = vec![x];
    let (mut up, mut down) = (x, x);
    for _ in 0..n {
        up = up.next_up();
        down = down.next_down();
        v.push(up);
        v.push(down);
    }
    v
}

#[test]
fn sinpi_and_cospi_at_every_path_boundary() {
    let mut r = Report::new("sinpi/cospi path boundaries");
    let edges = [
        hf("0x1.2p-5"), // sinpi near-zero fast path
        hf("0x1p-54"),  // sinpi tiny path
        hf("0x1p-53"),
        hf("0x1p-960"), // sinpi scaled tiny path
        hf("0x1p-1022"),
        hf("0x1p-12"),               // cospi near-zero path
        hf("0x1.ccf6429be6621p-29"), // cospi returns 1 below
        hf("0x1p-29"),
        p2(40),
        p2(41), // large-argument path
        p2(52),
        p2(53),
        0.25,
        0.5,
        1.0,
        1.5,
        2.0,
        1.0 / 3.0,
        1.0 / 6.0,
    ];
    for e in edges {
        for x in around(e, 40) {
            r.sinpi(x);
            r.cospi(x);
            r.sinpi(-x);
            r.cospi(-x);
        }
    }
    for x in [
        f64::from_bits(1),
        f64::from_bits(2),
        f64::from_bits(0x000f_ffff_ffff_ffff),
        f64::MAX,
    ] {
        r.sinpi(x);
        r.cospi(x);
        r.sinpi(-x);
        r.cospi(-x);
    }
    r.finish();
}

#[test]
fn sinpi_and_cospi_hard_cases_of_the_port_and_their_neighbours() {
    // The accurate path's listed hard cases, rebuilt from (iq, x): the input is
    // iq/2^11 + x/2^12 half-turns (for cospi, minus ½), in both mirror images.
    let sinpi_hard = [
        (76u32, "-0x1.276b3fef466p-2"),
        (108, "-0x1.33caea0f24cp-2"),
        (490, "-0x1.8a1e8a3e82cp-1"),
        (903, "-0x1.bdd02d1ad60p-2"),
    ];
    let cospi_hard = [
        (903u32, "-0x1.bdd02d1ad60p-2"),
        (1029, "-0x1.a4ad070549dp-3"),
        (1078, "0x1.fbdb79ca3dap-3"),
        (1217, "0x1.c0ccee4ada2p-1"),
        (1025, "0x1.b536647b1fe98p-2"),
        (1026, "-0x1.dc93eaad12a18p-2"),
        (1033, "0x1.e78f0e592b36p-1"),
        (1235, "0x1.f13412a48d8p-2"),
    ];
    let mut r = Report::new("sinpi/cospi listed hard cases");
    for (iq, z) in sinpi_hard {
        let x = f64::from(iq) / 2048.0 + hf(z) / 4096.0;
        for x in around(x, 3) {
            for x in [x, -x, x + 2.0, x + 1024.0, 1.0 - x] {
                r.sinpi(x);
            }
        }
    }
    for (iq, z) in cospi_hard {
        let x = f64::from(iq) / 2048.0 + hf(z) / 4096.0 - 0.5;
        for x in around(x, 3) {
            for x in [x, -x, x + 2.0, x + 1024.0] {
                r.cospi(x);
            }
        }
    }
    r.finish();
}

#[test]
fn exp_random_arguments() {
    let mut rng = Rng(0x5eed_0003);
    let mut r = Report::new("exp, random");
    for _ in 0..scale(100_000) {
        r.exp(-746.0 + rng.unit() * 1456.0);
    }
    for e in -60..=9i64 {
        for _ in 0..scale(500) {
            r.exp(rng.sign() * rng.binade(e));
        }
    }
    // Subnormal and near-overflow results.
    for _ in 0..scale(20_000) {
        r.exp(-745.2 + rng.unit() * (745.2 - 708.3));
        r.exp(709.0 + rng.unit() * 0.79);
    }
    r.finish();
}

#[test]
fn exp_tiny_arguments_near_midpoints() {
    // e^x = 1 + x + x²/2 + …: for x = ±j·2^−p the result sits a hair from a midpoint, a
    // structured family of hard cases below the accurate path's proven error.
    let mut r = Report::new("exp, structured tiny arguments");
    for p in 50..=62 {
        for j in 1..=600u32 {
            let x = f64::from(j) * p2(-p);
            r.exp(x);
            r.exp(-x);
        }
    }
    for k in 20..=52 {
        for j in 0..40u32 {
            let x = (1.0 + f64::from(j) * f64::EPSILON) * p2(-k);
            r.exp(x);
            r.exp(-x);
        }
    }
    r.finish();
}

#[test]
fn exp_thresholds() {
    let mut r = Report::new("exp, thresholds");
    let edges = [
        hf("0x1p-54"),               // returns 1 + x at and below
        hf("0x1.62e42fefa39fp+9"),   // overflow
        hf("-0x1.74910d52d3052p+9"), // underflow to +0
        hf("-0x1.6232bdd7abcd2p+9"), // subnormal results below
        hf("-0x1.6232bdd7abcd3p+9"),
        -708.396_418_532_264,
        -745.133_219_101_941, // ln(2^−1075)
        -744.440_071_921_381, // ln(2^−1074)
        709.0,
        -709.0,
        1.0,
        -1.0,
    ];
    for e in edges {
        for x in around(e, 60) {
            r.exp(x);
        }
    }
    for x in [
        f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE,
        f64::from_bits(1),
        -f64::from_bits(1),
        1e-300,
    ] {
        r.exp(x);
    }
    r.finish();
}

#[test]
fn ln_random_over_every_binade() {
    let mut rng = Rng(0x5eed_0004);
    let mut r = Report::new("ln, every binade");
    for e in -1074..=1023i64 {
        for _ in 0..scale(25) {
            r.ln(rng.binade(e));
        }
    }
    for _ in 0..scale(20_000) {
        r.ln(rng.finite().abs().max(f64::from_bits(1)));
    }
    r.finish();
}

#[test]
fn ln_near_one_and_at_the_top_of_every_binade() {
    // Tiny results always fail the fast test, so these run the accurate path, including the
    // port's own Taylor series on [255/256, 1)·2^E.
    let mut rng = Rng(0x5eed_0005);
    let mut r = Report::new("ln near 1 and at binade tops");
    for _ in 0..scale(60_000) {
        let k = rng.range(1, 60);
        r.ln(1.0 + rng.sign() * rng.binade(-k));
    }
    for _ in 0..scale(30_000) {
        let x = 1.0 - rng.unit() * p2(-8);
        r.ln(x);
        let e = rng.range(-1060, 1023) as i32;
        r.ln(x * p2(e));
    }
    for j in 1..=2048u32 {
        let d = f64::from(j);
        r.ln(1.0 + d * f64::EPSILON);
        r.ln(1.0 - d * f64::EPSILON / 2.0);
    }
    for x in around(1.0 - p2(-8), 30)
        .into_iter()
        .chain(around(1.0 + p2(-8), 30))
    {
        r.ln(x);
    }
    r.finish();
}

#[test]
fn ln_boundaries_powers_of_two_and_subnormals() {
    let mut r = Report::new("ln boundaries");
    for k in -1074..=1023 {
        r.ln(p2(k));
    }
    let edges = [
        core::f64::consts::SQRT_2, // fast path halves above √2
        hf("0x1.6a09e667f3bcdp+0"),
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::from_bits(1),
        f64::from_bits(0x000f_ffff_ffff_ffff),
        core::f64::consts::E,
        hf("0x1.62a88613629b6p+678"), // the hardest known case
    ];
    for e in edges {
        for x in around(e, 30) {
            if x > 0.0 && x.is_finite() {
                r.ln(x);
            }
        }
    }
    r.finish();
}

#[test]
fn atan2_random_pairs() {
    let mut rng = Rng(0x5eed_0006);
    let mut r = Report::new("atan2, random pairs");
    for _ in 0..scale(80_000) {
        let ex = rng.range(-1074, 1023);
        let d = match rng.next() % 3 {
            0 => rng.range(-8, 8),
            1 => rng.range(-70, 70),
            _ => rng.range(-200, 200),
        };
        let ey = (ex + d).clamp(-1074, 1023);
        let x = rng.sign() * rng.binade(ex);
        let y = rng.sign() * rng.binade(ey);
        r.atan2(y, x);
    }
    for _ in 0..scale(15_000) {
        let (y, x) = (rng.finite(), rng.finite());
        if y != 0.0 && x != 0.0 {
            r.atan2(y, x);
        }
    }
    r.finish();
}

#[test]
fn atan2_at_subnormal_midpoints_and_their_neighbours() {
    // y/x = (2k+1)·2^−1075 exactly: atan(y/x) is just below the midpoint, so it rounds towards
    // zero; one unit of y either way moves the quotient off the midpoint.
    let mut rng = Rng(0x5eed_0007);
    let mut r = Report::new("atan2 at subnormal midpoints");
    for _ in 0..scale(2_000) {
        let odd = (rng.next() % (1 << 26)) | 1;
        let mx = (rng.next() % (1 << 26)) | 1;
        let a = rng.range(-1074, -60);
        let y = (odd as f64) * (mx as f64) * p2(a as i32);
        let x = (mx as f64) * p2((a + 1075) as i32);
        if !(y.is_finite() && x.is_finite() && y != 0.0) {
            continue;
        }
        for y in [y, y.next_up(), y.next_down()] {
            for (yy, xx) in [(y, x), (-y, x), (y, -x), (-y, -x), (x, y), (x, -y)] {
                r.atan2(yy, xx);
            }
        }
    }
    r.finish();
}

#[test]
fn atan2_path_boundaries() {
    let mut r = Report::new("atan2 path boundaries");
    let t = hf("0x1.d12ed0af1a27fp-27"); // exact-quotient shortcut limit
    for q in around(t, 20) {
        for x in [1.0, 3.0, p2(-900), p2(900), p2(-1000)] {
            let y = q * x;
            if y.is_finite() && y != 0.0 {
                r.atan2(y, x);
                r.atan2(-y, x);
                r.atan2(y, -x);
            }
        }
    }
    // |y| around 2^−915 with exact quotients; x around 2^−920 and 2^1022 (the scalings).
    for y in around(p2(-915), 10) {
        for x in [p2(-889), p2(-889) * 3.0, 1.0] {
            r.atan2(y, x);
        }
    }
    for x in around(p2(-920), 10).into_iter().chain(around(p2(1022), 10)) {
        for f in [1.0, 0.75, 0.5, 1.0 / 64.0, 0.999_999, 1e-10] {
            r.atan2(x * f, x);
            r.atan2(-x * f, -x);
            r.atan2(x, x * f);
        }
    }
    // Quotients around 2^−95 (the tiny-quotient rule) and exponent gaps around 53.
    let mut rng = Rng(0x5eed_0008);
    for _ in 0..scale(10_000) {
        let x = rng.sign() * rng.binade_in(-900, 900);
        let k = rng.range(40, 110) as i32;
        let y = rng.sign() * x.abs() * rng.binade(-i64::from(k));
        if y != 0.0 && y.is_finite() {
            r.atan2(y, x);
            r.atan2(x, y);
        }
    }
    for (y, x) in [
        (1.0, 1.0),
        (-1.0, 1.0),
        (1.0, -1.0),
        (-1.0, -1.0),
        (f64::MAX, f64::MAX),
        (f64::from_bits(1), f64::from_bits(1)),
        (f64::MAX, f64::from_bits(1)),
        (f64::from_bits(1), f64::MAX),
        (f64::from_bits(1), -f64::MAX),
    ] {
        r.atan2(y, x);
    }
    r.finish();
}

#[test]
fn consecutive_doubles_at_structured_spots() {
    // Runs of consecutive doubles where the exact values have arithmetic structure
    // (ln(1 ± k·u), e^(k·u), sin(π(1 + k·u)), …): the kind of input where systematic hard cases
    // live and where the worst-case lists must be complete.
    let n = scale(20_000);
    let mut r = Report::new("consecutive doubles at structured spots");
    let run = |start: f64, count: usize, up: bool| -> Vec<f64> {
        let mut v = Vec::with_capacity(count);
        let mut x = start;
        for _ in 0..count {
            v.push(x);
            x = if up { x.next_up() } else { x.next_down() };
        }
        v
    };
    for x in run(1.0, n, false)
        .into_iter()
        .chain(run(1.0, n, true))
        .chain(run(2.0, n / 4, false))
        .chain(run(0.5, n / 4, true))
    {
        r.ln(x);
    }
    for x in run(1.0 - p2(-8), n / 4, false)
        .into_iter()
        .chain(run(1.0 - p2(-8), n / 4, true))
    {
        r.ln(x);
    }
    for x in run(p2(-54), n, true)
        .into_iter()
        .chain(run(-p2(-54), n, false))
        .chain(run(p2(-30), n / 4, true))
    {
        r.exp(x);
    }
    for x in run(1.0, n, true)
        .into_iter()
        .chain(run(0.5, n / 2, true))
        .chain(run(p2(-54), n / 4, true))
        .chain(run(1024.0, n / 4, true))
    {
        r.sinpi(x);
    }
    for x in run(0.5, n, true)
        .into_iter()
        .chain(run(1.0, n / 2, true))
        .chain(run(hf("0x1.ccf6429be6621p-29"), n / 4, true))
    {
        r.cospi(x);
    }
    for y in run(p2(-27), n / 2, true)
        .into_iter()
        .chain(run(1.0, n / 2, true))
    {
        r.atan2(y, 1.0);
        r.atan2(1.0, y);
    }
    r.finish();
}

/// The significand fields of set (b) of CORE-MATH's `sinpi.wc` (commit 0ce0cbc1): its 1024 worst
/// cases in [2^−1022, 2^−1021). The file says: "for 1 <= e <= 968, add the inputs from set (b)
/// multiplied by 2^e" to get the worst cases of every binade up to [2^−54, 2^−53).
#[rustfmt::skip]
const SINPI_WC_SET_B: [u64; 1024] = [
    0x0_02d7_417b_5d47, 0x0_056f_3925_d5bd, 0x0_003f_49d0_e4d1, 0x0_1e54_ffa7_093e, 0x0_1925_1052_1852,
    0x0_20ec_f751_81b4, 0x0_1bbd_07fc_90c8, 0x0_168d_18a7_9fdc, 0x0_2384_eefb_fa2a, 0x0_261c_e6a6_72a0,
    0x0_2b4c_d5fb_638c, 0x0_2de4_cda5_dc02, 0x0_28b4_de50_eb16, 0x0_419a_a4d2_1e97, 0x0_46ca_9427_0f83,
    0x0_3f02_ad27_a621, 0x0_4432_9c7c_970d, 0x0_4962_8bd1_87f9, 0x0_4bfa_837c_006f, 0x0_512a_72d0_f15b,
    0x0_565a_6225_e247, 0x0_4e92_7b26_78e5, 0x0_53c2_6a7b_69d1, 0x0_58f2_59d0_5abd, 0x0_6a10_3952_24dc,
    0x0_6f40_28a7_15c8, 0x0_7470_17fc_06b4, 0x0_79a0_0750_f7a0, 0x0_6ca8_30fc_9d52, 0x0_71d8_2051_8e3e,
    0x0_7708_0fa6_7f2a, 0x0_7c37_fefb_7016, 0x0_7ecf_f6a5_e88c, 0x0_8167_ee50_6102, 0x0_9285_cdd2_2b21,
    0x0_97b5_bd27_1c0d, 0x0_9ce5_ac7c_0cf9, 0x0_951d_c57c_a397, 0x0_a215_9bd0_fde5, 0x0_9a4d_b4d1_9483,
    0x0_9f7d_a426_856f, 0x0_a745_8b25_eed1, 0x0_a4ad_937b_765b, 0x0_a9dd_82d0_6747, 0x0_bafb_6252_3166,
    0x0_c02b_51a7_2252, 0x0_c55b_40fc_133e, 0x0_bd93_59fc_a9dc, 0x0_c2c3_4951_9ac8, 0x0_cd23_27fb_7ca0,
    0x0_ca8b_3051_042a, 0x0_cfbb_1fa5_f516, 0x0_d4eb_0efa_e602, 0x0_c7f3_38a6_8bb4, 0x0_d253_1750_6d8c,
    0x0_e8a0_e627_2897, 0x0_e608_ee7c_b021, 0x0_eb38_ddd1_a10d, 0x0_edd0_d57c_1983, 0x0_f300_c4d1_0a6f,
    0x0_f830_b425_fb5b, 0x0_fd60_a37a_ec47, 0x0_f068_cd26_91f9, 0x0_f598_bc7b_82e5, 0x0_fac8_abd0_73d1,
    0x1_0e7e_82fc_b666, 0x1_1116_7aa7_2edc, 0x1_1646_69fc_1fc8, 0x1_1b76_5951_10b4, 0x1_20a6_48a6_01a0,
    0x1_13ae_7251_a752, 0x1_25d6_37fa_f28c, 0x1_18de_61a6_983e, 0x1_1e0e_50fb_892a, 0x1_233e_4050_7a16,
    0x1_36f4_177c_bcab, 0x1_3c24_06d1_ad97, 0x1_398c_0f27_3521, 0x1_3ebb_fe7c_260d, 0x1_43eb_edd1_16f9,
    0x1_4153_f626_9e83, 0x1_491b_dd26_07e5, 0x1_4683_e57b_8f6f, 0x1_4e4b_cc7a_f8d1, 0x1_4bb3_d4d0_805b,
    0x1_5f69_abfc_c2f0, 0x1_6499_9b51_b3dc, 0x1_69c9_8aa6_a4c8, 0x1_6201_a3a7_3b66, 0x1_6ef9_79fb_95b4,
    0x1_6731_92fc_2c52, 0x1_6c61_8251_1d3e, 0x1_7429_6950_86a0, 0x1_7959_58a5_778c, 0x1_7191_71a6_0e2a,
    0x1_76c1_60fa_ff16, 0x1_8d0f_2fd1_ba21, 0x1_923f_1f26_ab0d, 0x1_8a77_3827_41ab, 0x1_976f_0e7b_9bf9,
    0x1_8fa7_277c_3297, 0x1_9c9e_fdd0_8ce5, 0x1_94d7_16d1_2383, 0x1_a1ce_ed25_7dd1, 0x1_9a07_0626_146f,
    0x1_9f36_f57b_055b, 0x1_b584_c451_c066, 0x1_bab4_b3a6_b152, 0x1_b2ec_cca7_47f0, 0x1_bfe4_a2fb_a23e,
    0x1_b81c_bbfc_38dc, 0x1_c514_9250_932a, 0x1_bd4c_ab51_29c8, 0x1_ca44_81a5_8416, 0x1_c27c_9aa6_1ab4,
    0x1_c7ac_89fb_0ba0, 0x1_ddfa_58d1_c6ab, 0x1_e32a_4826_b797, 0x1_db62_6127_4e35, 0x1_e85a_377b_a883,
    0x1_e092_507c_3f21, 0x1_ed8a_26d0_996f, 0x1_e5c2_3fd1_300d, 0x1_f2ba_1625_8a5b, 0x1_eaf2_2f26_20f9,
    0x1_f022_1e7b_11e5, 0x1_f552_0dd0_02d1, 0x2_066f_ed51_ccf0, 0x2_0b9f_dca6_bddc, 0x2_10cf_cbfb_aec8,
    0x2_0907_e4fc_4566, 0x2_15ff_bb50_9fb4, 0x2_1b2f_aaa5_90a0, 0x2_0e37_d451_3652, 0x2_1367_c3a6_273e,
    0x2_1897_b2fb_182a, 0x2_1dc7_a250_0916, 0x2_2ee5_81d1_d335, 0x2_3415_7126_c421, 0x2_3945_607b_b50d,
    0x2_3e75_4fd0_a5f9, 0x2_317d_797c_4bab, 0x2_36ad_68d1_3c97, 0x2_3bdd_5826_2d83, 0x2_43a5_3f25_96e5,
    0x2_410d_477b_1e6f, 0x2_463d_36d0_0f5b, 0x2_575b_1651_d97a, 0x2_59f3_0dfc_51f0, 0x2_5c8b_05a6_ca66,
    0x2_61ba_f4fb_bb52, 0x2_6982_dbfb_24b4, 0x2_5f22_fd51_42dc, 0x2_66ea_e450_ac3e, 0x2_6452_eca6_33c8,
    0x2_6c1a_d3a5_9d2a, 0x2_714a_c2fa_8e16, 0x2_6eb2_cb50_15a0, 0x2_8268_a27c_5835, 0x2_8798_91d1_4921,
    0x2_8500_9a26_d0ab, 0x2_8cc8_8126_3a0d, 0x2_91f8_707b_2af9, 0x2_8a30_897b_c197, 0x2_9728_5fd0_1be5,
    0x2_8f60_78d0_b283, 0x2_9490_6825_a36f, 0x2_99c0_577a_945b, 0x2_aade_36fc_5e7a, 0x2_b00e_2651_4f66,
    0x2_ad76_2ea6_d6f0, 0x2_b53e_15a6_4052, 0x2_ba6e_04fb_313e, 0x2_b2a6_1dfb_c7dc, 0x2_bf9d_f450_222a,
    0x2_b7d6_0d50_b8c8, 0x2_bd05_fca5_a9b4, 0x2_c235_ebfa_9aa0, 0x2_d353_cb7c_64bf, 0x2_d883_bad1_55ab,
    0x2_ddb3_aa26_4697, 0x2_e2e3_997b_3783, 0x2_d5eb_c326_dd35, 0x2_db1b_b27b_ce21, 0x2_e04b_a1d0_bf0d,
    0x2_e813_88d0_286f, 0x2_ed43_7825_195b, 0x2_e57b_9125_aff9, 0x2_eaab_807a_a0e5, 0x3_00f9_4f51_5bf0,
    0x3_0629_3ea6_4cdc, 0x2_fe61_57a6_e37a, 0x3_0391_46fb_d466, 0x3_0b59_2dfb_3dc8, 0x3_08c1_3650_c552,
    0x3_1089_1d50_2eb4, 0x3_15b9_0ca5_1fa0, 0x3_0df1_25a5_b63e, 0x3_1321_14fa_a72a, 0x3_26d6_ec26_e9bf,
    0x3_296e_e3d1_6235, 0x3_3666_ba25_bc83, 0x3_2c06_db7b_daab, 0x3_2e9e_d326_5321, 0x3_33ce_c27b_440d,
    0x3_38fe_b1d0_34f9, 0x3_3e2e_a125_25e5, 0x3_3136_cad0_cb97, 0x3_3b96_a97a_ad6f, 0x3_51e4_7851_687a,
    0x3_4f4c_80a6_f004, 0x3_5714_67a6_5966, 0x3_5c44_56fb_4a52, 0x3_6174_4650_3b3e, 0x3_547c_6ffb_e0f0,
    0x3_59ac_5f50_d1dc, 0x3_5edc_4ea5_c2c8, 0x3_66a4_35a5_2c2a, 0x3_640c_3dfa_b3b4, 0x3_693c_2d4f_a4a0,
    0x3_7a5a_0cd1_6ebf, 0x3_7f89_fc26_5fab, 0x3_84b9_eb7b_5097, 0x3_8c81_d27a_b9f9, 0x3_7cf2_047b_e735,
    0x3_8221_f3d0_d821, 0x3_89e9_dad0_4183, 0x3_8751_e325_c90d, 0x3_8f19_ca25_326f, 0x3_91b1_c1cf_aae5,
    0x3_a567_98fb_ed7a, 0x3_aa97_8850_de66, 0x3_a2cf_a151_7504, 0x3_a7ff_90a6_65f0, 0x3_afc7_77a5_cf52,
    0x3_ad2f_7ffb_56dc, 0x3_b4f7_66fa_c03e, 0x3_ba27_564f_b12a, 0x3_b25f_6f50_47c8, 0x3_b78f_5ea5_38b4,
    0x3_cddd_2d7b_f3bf, 0x3_d30d_1cd0_e4ab, 0x3_cb45_35d1_7b49, 0x3_d075_2526_6c35, 0x3_d83d_0c25_d597,
    0x3_dd6c_fb7a_c683, 0x3_e29c_eacf_b76f, 0x3_d5a5_147b_5d21, 0x3_dad5_03d0_4e0d, 0x3_e004_f325_3ef9,
    0x3_e534_e27a_2fe5, 0x3_f652_c1fb_fa04, 0x3_fb82_b150_eaf0, 0x4_00b2_a0a5_dbdc, 0x4_05e2_8ffa_ccc8,
    0x3_f8ea_b9a6_727a, 0x3_fe1a_a8fb_6366, 0x4_0b12_7f4f_bdb4, 0x4_034a_9850_5452, 0x4_087a_87a5_453e,
    0x4_0daa_76fa_362a, 0x4_1ec8_567c_0049, 0x4_23f8_45d0_f135, 0x4_2928_3525_e221, 0x4_2e58_247a_d30d,
    0x4_2160_4e26_78bf, 0x4_2690_3d7b_69ab, 0x4_2bc0_2cd0_5a97, 0x4_3388_13cf_c3f9, 0x4_30f0_1c25_4b83,
    0x4_3620_0b7a_3c6f, 0x4_473d_eafc_068e, 0x4_4c6d_da50_f77a, 0x4_5435_c150_60dc, 0x4_519d_c9a5_e866,
    0x4_49d5_e2a6_7f04, 0x4_56cd_b8fa_d952, 0x4_4f05_d1fb_6ff0, 0x4_5bfd_a84f_ca3e, 0x4_5965_b0a5_51c8,
    0x4_5e95_9ffa_42b4, 0x4_6b8d_764e_9d02, 0x4_67eb_987c_a371, 0x4_8dc9_3552_3140, 0x4_6d1b_87d1_945d,
    0x4_724b_7726_8549, 0x4_777b_667b_7635, 0x4_7cab_55d0_6721, 0x4_81db_4525_580d, 0x4_612d_97a4_bb2a,
    0x4_870b_347a_48f9, 0x4_665d_86f9_ac16, 0x4_8c3b_23cf_39e5, 0x4_b218_c0a4_c7b4, 0x4_916b_1324_2ad1,
    0x4_92f9_24a7_222c, 0x4_b748_aff9_b8a0, 0x4_969b_0279_1bbd, 0x4_b8d6_c17c_affb, 0x4_9829_13fc_1318,
    0x4_bc78_9f4e_a98c, 0x4_9d59_0351_0404, 0x4_be06_b0d1_a0e7, 0x4_a288_f2a5_f4f0, 0x4_a7b8_e1fa_e5dc,
    0x4_ace8_d14f_d6c8, 0x4_d2c6_6e25_6497, 0x4_d7f6_5d7a_5583, 0x4_dd26_4ccf_466f, 0x4_e256_3c24_375b,
    0x4_deb4_5e52_3dca, 0x4_c336_a026_91d3, 0x4_e3e4_4da7_2eb6, 0x4_e786_2b79_2847, 0x4_c866_8f7b_82bf,
    0x4_e914_3cfc_1fa2, 0x4_cd96_7ed0_73ab, 0x4_ee44_2c51_108e, 0x4_f374_1ba6_017a, 0x4_f8a4_0afa_f266,
    0x5_1951_b87b_8f49, 0x4_fdd3_fa4f_e352, 0x5_0303_e9a4_d43e, 0x5_0833_d8f9_c52a, 0x5_09c1_ea7c_bc85,
    0x5_0d63_c84e_b616, 0x5_0ef1_d9d1_ad71, 0x5_1293_b7a3_a702, 0x5_1421_c926_9e5d, 0x5_39ff_65fc_2c2c,
    0x5_1e81_a7d0_8035, 0x5_3f2f_5551_1d18, 0x5_23b1_9725_7121, 0x5_445f_44a6_0e04, 0x5_28e1_867a_620d,
    0x5_498f_33fa_fef0, 0x5_2e11_75cf_52f9, 0x5_3341_6524_43e5, 0x5_34cf_76a7_3b40, 0x5_3871_5479_34d1,
    0x5_637e_e0a3_b38c, 0x5_5aad_137c_c90f, 0x5_5e4e_f14e_c2a0, 0x5_5fdd_02d1_b9fb, 0x5_650c_f226_aae7,
    0x5_6a3c_e17b_9bd3, 0x5_4ebf_234f_efdc, 0x5_53ef_12a4_e0c8, 0x5_6f6c_d0d0_8cbf, 0x5_749c_c025_7dab,
    0x5_591f_01f9_d1b4, 0x5_7efc_9ecf_5f83, 0x5_79cc_af7a_6e97, 0x5_842c_8e24_506f, 0x5_a4da_3ba4_ed52,
    0x5_85ba_9fa7_47ca, 0x5_895c_7d79_415b, 0x5_aa0a_2af9_de3e, 0x5_8aea_8efc_38b6, 0x5_8e8c_6cce_3247,
    0x5_901a_7e51_29a2, 0x5_954a_6da6_1a8e, 0x5_9a7a_5cfb_0b7a, 0x5_b0c8_2bd1_c685, 0x5_9faa_4c4f_fc66,
    0x5_c587_e925_8a35, 0x5_cab7_d87a_7b21, 0x5_af3a_1a4e_cf2a, 0x5_cfe7_c7cf_6c0d, 0x5_b46a_09a3_c016,
    0x5_d517_b724_5cf9, 0x5_b5f8_1b26_b771, 0x5_d6a5_c8a7_5454, 0x5_da47_a679_4de5, 0x5_bb28_0a7b_a85d,
    0x5_c057_f9d0_9949, 0x5_e635_96a6_2718, 0x5_eb65_85fb_1804, 0x5_f095_7550_08f0, 0x5_f5c5_64a4_f9dc,
    0x5_dbd5_b7fc_4540, 0x5_df77_95ce_3ed1, 0x5_faf5_53f9_eac8, 0x5_e105_a751_362c, 0x6_0025_434e_dbb4,
    0x6_0c13_337b_b4e7, 0x6_01b3_54d1_d30f, 0x6_0555_32a3_cca0, 0x6_06e3_4426_c3fb, 0x6_0a85_21f8_bd8c,
    0x6_1143_22d0_a5d3, 0x6_2cc0_e0fc_51ca, 0x6_3062_bece_4b5b, 0x6_1673_1225_96bf, 0x6_31f0_d051_42b6,
    0x6_1ba3_017a_87ab, 0x6_3720_bfa6_33a2, 0x6_20d2_f0cf_7897, 0x6_2602_e024_6983, 0x6_5110_6c4e_e83e,
    0x6_2b32_cf79_5a6f, 0x6_529e_7dd1_df99, 0x6_5640_5ba3_d92a, 0x6_3c50_aefb_248e, 0x6_57ce_6d26_d085,
    0x6_5b70_4af8_ca16, 0x6_4180_9e50_157a, 0x6_5cfe_5c7b_c171, 0x6_46b0_8da5_0666, 0x6_622e_4bd0_b25d,
    0x6_4be0_7cf9_f752, 0x6_675e_3b25_a349, 0x6_71be_19cf_8521, 0x6_76ee_0924_760d, 0x6_7c1d_f879_66f9,
    0x6_979b_b6a5_12f0, 0x6_7dac_09fc_5e54, 0x6_814d_e7ce_57e5, 0x6_82db_f951_4f40, 0x6_867d_d723_48d1,
    0x6_6c8e_2a7a_9435, 0x6_880b_e8a6_402c, 0x6_8d3b_d7fb_3118, 0x6_9ccb_a5fa_03dc, 0x6_926b_c750_2204,
    0x6_b849_6425_afd3, 0x6_a1fb_954e_f4c8, 0x6_bd79_537a_a0bf, 0x6_a72b_84a3_e5b4, 0x6_c2a9_42cf_91ab,
    0x6_a8b9_9626_dd0f, 0x6_ac5b_73f8_d6a0, 0x6_c7d9_3224_8297, 0x6_ade9_857b_cdfb, 0x6_d8f7_11a6_4cb6,
    0x6_b319_74d0_bee7, 0x6_de27_00fb_3da2, 0x6_e356_f050_2e8e, 0x6_cd09_2179_7383, 0x6_e886_dfa5_1f7a,
    0x6_ce97_32fc_6ade, 0x6_d239_10ce_646f, 0x6_edb6_cefa_1066, 0x6_d3c7_2251_5bca, 0x6_f9a4_bf26_e999,
    0x6_d769_0023_555b, 0x6_fd46_9cf8_e32a, 0x6_f2e6_be4f_0152, 0x6_fed4_ae7b_da85, 0x7_0276_8c4d_d416,
    0x7_0404_9dd0_cb71, 0x7_2324_39ce_70f9, 0x7_0934_8d25_bc5d, 0x6_f816_ada3_f23e, 0x7_24b2_4b51_6854,
    0x7_0e64_7c7a_ad49, 0x7_2854_2923_61e5, 0x7_1394_6bcf_9e35, 0x7_18c4_5b24_8f21, 0x7_29e2_3aa6_5940,
    0x7_1df4_4a79_800d, 0x7_43d1_e74f_0ddc, 0x7_4e31_c5f8_efb4, 0x7_2f12_29fb_4a2c, 0x7_4901_d6a3_fec8,
    0x7_4a8f_e826_f623, 0x7_3442_1950_3b18, 0x7_4fbf_d77b_e70f, 0x7_3972_08a5_2c04, 0x7_5361_b54d_e0a0,
    0x7_54ef_c6d0_d7fb, 0x7_3ea1_f7fa_1cf0, 0x7_647f_94cf_aabf, 0x7_69af_8424_9bab, 0x7_6edf_7379_8c97,
    0x7_852d_4250_47a2, 0x7_5a1f_b625_c8e7, 0x7_740f_62ce_7d83, 0x7_5f4f_a57a_b9d3, 0x7_759d_7451_74de,
    0x7_793f_5223_6e6f, 0x7_7acd_63a6_65ca, 0x7_8a5d_31a5_388e, 0x7_7e6f_4178_5f5b, 0x7_7ffd_52fb_56b6,
    0x7_8f8d_20fa_297a, 0x7_94bd_104f_1a66, 0x7_ab0a_df25_d571, 0x7_99ec_ffa4_0b52, 0x7_b03a_ce7a_c65d,
    0x7_b56a_bdcf_b749, 0x7_9f1c_eef8_fc3e, 0x7_ca2a_7b23_7af9, 0x7_a0ab_007b_f399, 0x7_cbb8_8ca6_7254,
    0x7_a44c_de4d_ed2a, 0x7_cf5a_6a78_6be5, 0x7_a5da_efd0_e485, 0x7_ba9a_ad24_a835, 0x7_d0e8_7bfb_6340,
    0x7_d618_6b50_542c, 0x7_bfca_9c79_9921, 0x7_c4fa_8bce_8a0d, 0x7_db48_5aa5_4518, 0x7_c688_9d51_8168,
    0x7_e078_49fa_3604, 0x7_f008_17f9_08c8, 0x7_f196_297c_0023, 0x7_e5a8_394f_26f0, 0x7_f538_074d_f9b4,
    0x7_f6c6_18d0_f10f, 0x7_fa67_f6a2_eaa0, 0x8_10b5_c579_a5ab, 0x7_fbf6_0825_e1fb, 0x8_15e5_b4ce_9697,
    0x7_ead8_28a4_17dc, 0x8_0125_f77a_d2e7, 0x8_0655_e6cf_c3d3, 0x8_1b15_a423_8783, 0x8_0b85_d624_b4bf,
    0x8_1ca3_b5a6_7ede, 0x8_2045_9378_786f, 0x8_3693_624f_337a, 0x8_21d3_a4fb_6fca, 0x8_3bc3_51a4_2466,
    0x8_2703_9450_60b6, 0x8_40f3_40f9_1552, 0x8_2c33_83a5_51a2, 0x8_4281_527c_0cad, 0x8_5741_0fcf_d05d,
    0x8_4623_304e_063e, 0x8_3163_72fa_428e, 0x8_5c70_ff24_c149, 0x8_47b1_41d0_fd99, 0x8_4b53_1fa2_f72a,
    0x8_61a0_ee79_b235, 0x8_4ce1_3125_ee85, 0x8_66d0_ddce_a321, 0x8_77ee_bd50_6d40, 0x8_5211_207a_df71,
    0x8_6c00_cd23_940d, 0x8_7d1e_aca5_5e2c, 0x8_6d8e_dea6_8b68, 0x8_7130_bc78_84f9, 0x8_91de_69f9_21dc,
    0x8_824e_9bfa_4f18, 0x8_72be_cdfb_7c54, 0x8_989c_6ad1_0a23, 0x8_9c3e_48a3_03b4, 0x8_877e_8b4f_4004,
    0x8_9dcc_5a25_fb0f, 0x8_8cae_7aa4_30f0, 0x8_a2fc_497a_ebfb, 0x8_7660_abcd_75e5, 0x8_970e_594e_12c8,
    0x8_a82c_38cf_dce7, 0x8_be7a_07a6_97f2, 0x8_c74b_d4cd_826f, 0x8_c21b_e578_9183, 0x8_ad5c_2824_cdd3,
    0x8_c3a9_f6fb_88de, 0x8_b28c_1779_bebf, 0x8_c8d9_e650_79ca, 0x8_b7bc_06ce_afab, 0x8_ce09_d5a5_6ab6,
    0x8_e2c9_92f9_2e66, 0x8_bceb_f623_a097, 0x8_d339_c4fa_5ba2, 0x8_e7f9_824e_1f52, 0x8_e987_93d1_16ad,
    0x8_ed29_71a3_103e, 0x8_d869_b44f_4c8e, 0x9_0377_4079_cb49, 0x8_eeb7_8326_0799, 0x8_f259_60f8_012a,
    0x8_dd99_a3a4_3d7a, 0x8_f3e7_727a_f885, 0x9_08a7_2fce_bc35, 0x8_f917_61cf_e971, 0x9_0dd7_1f23_ad21,
    0x8_fe47_5124_da5d, 0x9_1307_0e78_9e0d, 0x9_1495_1ffb_9568, 0x9_2954_dd4f_5918, 0x9_1836_fdcd_8ef9,
    0x9_2e84_cca4_4a04, 0x9_19c5_0f50_8654, 0x9_33b4_bbf9_3af0, 0x9_4a02_8acf_f5fb, 0x9_1ef4_fea5_7740,
    0x9_2424_edfa_682c, 0x9_38e4_ab4e_2bdc, 0x9_4f32_7a24_e6e7, 0x9_3a72_bcd1_2337, 0x9_3e14_9aa3_1cc8,
    0x9_5462_6979_d7d3, 0x9_3fa2_ac26_1423, 0x9_4344_89f8_0db4, 0x9_6ab0_3850_92de, 0x9_44d2_9b7b_050f,
    0x9_5992_58ce_c8bf, 0x9_6e52_1622_8c6f, 0x9_5ec2_4823_b9ab, 0x9_6fe0_27a5_83ca, 0x9_7510_16fa_74b6,
    0x9_63f2_3778_aa97, 0x9_8eff_c3a3_2952, 0x9_7a40_064f_65a2, 0x9_6580_48fb_a1f2, 0x9_908d_d526_20ad,
    0x9_6922_26cd_9b83, 0x9_942f_b2f8_1a3e, 0x9_7f6f_f5a4_568e, 0x9_95bd_c47b_1199, 0x9_849f_e4f9_477a,
    0x9_9aed_b3d0_0285, 0x9_89cf_d44e_3866, 0x9_a01d_a324_f371, 0x9_b4dd_6078_b721, 0x9_a54d_9279_e45d,
    0x9_b66b_71fb_ae7c, 0x9_ba0d_4fcd_a80d, 0x9_bb9b_6150_9f68, 0x9_bf3d_3f22_98f9, 0x9_aa7d_81ce_d549,
    0x9_d58b_0df9_5404, 0x9_c0cb_50a5_9054, 0x9_afad_7123_c635, 0x9_dfea_eca3_35dc, 0x9_daba_fd4e_44f0,
    0x9_c5fb_3ffa_8140, 0x9_cb2b_2f4f_722c, 0x9_d05b_1ea4_6318, 0x9_f638_bb79_f0e7, 0x9_e178_fe26_2d37,
    0x9_e51a_dbf8_26c8, 0x9_fb68_aace_e1d3, 0x9_e6a8_ed7b_1e23, 0x9_ea4a_cb4d_17b4, 0xa_0098_9a23_d2bf,
    0x9_ebd8_dcd0_0f0f, 0xa_05c8_8978_c3ab, 0x9_f108_cc24_fffb, 0xa_16e6_68fa_8dca, 0xa_1c16_584f_7eb6,
    0xa_0af8_78cd_b497, 0xa_2146_47a4_6fa2, 0xa_30d6_15a3_4266, 0xa_3606_04f8_3352, 0xa_0c86_8a50_abf2,
    0xa_1028_6822_a583, 0xa_2676_36f9_608e, 0xa_11b6_79a5_9cde, 0xa_2ba6_264e_517a, 0xa_3cc4_05d0_1b99,
    0xa_41f3_f525_0c85, 0xa_3264_2726_39c1, 0xa_4723_e479_fd71, 0xa_3794_167b_2aad, 0xa_5d71_b350_b87c,
    0xa_6113_9122_b20d, 0xa_4c53_d3ce_ee5d, 0xa_62a1_a2a5_a968, 0xa_6643_8077_a2f9, 0xa_3b35_f44d_243e,
    0xa_5183_c323_df49, 0xa_67d1_91fa_9a54, 0xa_56b3_b278_d035, 0xa_81c1_3ea3_4ef0, 0xa_5be3_a1cd_c121,
    0xa_6d01_814f_8b40, 0xa_86f1_2df8_3fdc, 0xa_7231_70a4_7c2c, 0xa_887f_3f7b_3737, 0xa_8c21_1d4d_30c8,
    0xa_7761_5ff9_6d18, 0xa_8daf_2ed0_2823, 0xa_7c91_4f4e_5e04, 0xa_a26e_ec23_ebd3, 0xa_92df_1e25_190f,
    0xa_a79e_db78_dcbf, 0xa_acce_cacd_cdab, 0xa_980f_0d7a_09fb, 0xa_ae5c_dc50_c506, 0xa_b1fe_ba22_be97,
    0xa_9d3e_fcce_fae7, 0xa_b38c_cba5_b5f2, 0xa_b72e_a977_af83, 0xa_c84c_88f9_79a2, 0xa_b8bc_bafa_a6de,
    0xa_cd7c_784e_6a8e, 0xa_bdec_aa4f_97ca, 0xa_c31c_99a4_88b6, 0xa_d2ac_67a3_5b7a, 0xa_e8fa_367a_1685,
    0xa_d7dc_56f8_4c66, 0xa_ee2a_25cf_0771, 0xa_d96a_687b_43c1, 0xa_dd0c_464d_3d52, 0xa_f35a_1523_f85d,
    0xa_de9a_57d0_34ad, 0xa_e23c_35a2_2e3e, 0xb_09a7_e3fa_b368, 0xa_e3ca_4725_2599, 0xa_f88a_0478_e949,
    0xb_0ed7_d34f_a454, 0xb_1407_c2a4_9540, 0xa_fdb9_f3cd_da35, 0xb_1937_b1f9_862c, 0xb_02e9_e322_cb21,
    0xb_2a55_917b_504b, 0xb_0477_f4a5_c27c, 0xb_0819_d277_bc0d, 0xb_1e67_a14e_7718, 0xb_2df7_6f4d_49dc,
    0xb_2f85_80d0_4137, 0xb_3327_5ea2_3ac8, 0xb_34b5_7025_3223, 0xb_2397_90a3_6804, 0xb_39e5_5f7a_230f,
    0xb_28c7_7ff8_58f0, 0xb_53d5_0c22_d7ab, 0xb_3f15_4ecf_13fb, 0xb_4445_3e24_04e7, 0xb_5563_1da5_cf06,
    0xb_4975_2d78_f5d3, 0xb_5904_fb77_c897, 0xb_5a93_0cfa_bff2, 0xb_4ea5_1ccd_e6bf, 0xb_7482_b9a3_748e,
    0xb_5e34_eacc_b983, 0xb_79b2_a8f8_657a, 0xb_5fc2_fc4f_b0de, 0xb_64f2_eba4_a1ca, 0xb_7ee2_984d_5666,
    0xb_6a22_daf9_92b6, 0xb_8070_a9d0_4dc1, 0xb_9530_6724_1171, 0xb_6f52_ca4e_83a2, 0xb_8412_87a2_4752,
    0xb_9a60_5679_025d, 0xb_85a0_9925_3ead, 0xb_9f90_45cd_f349, 0xb_a4c0_3522_e435, 0xb_8ad0_887a_2f99,
    0xb_a64e_46a5_db90, 0xb_b5de_14a4_ae54, 0xb_a9f0_2477_d521, 0xb_9000_77cf_2085, 0xb_ab7e_35fa_cc7c,
    0xb_bb0e_03f9_9f40, 0xb_af20_13cc_c60d, 0xb_c56d_e2a3_8118, 0xb_b0ae_254f_bd68, 0xb_c03d_f34e_902c,
    0xb_dbbb_b17a_3c23, 0xb_ca9d_d1f8_7204, 0xb_e0eb_a0cf_2d0f, 0xb_cfcd_c14d_62f0, 0xb_d15b_d2d0_5a4b,
    0xb_d4fd_b0a2_53dc, 0xb_d68b_c225_4b37, 0xb_e61b_9024_1dfb, 0xb_fc69_5efa_d906, 0xc_000b_3ccc_d297,
    0xb_da2d_9ff7_44c8, 0xc_0199_4e4f_c9f2, 0xb_eb4b_7f79_0ee7, 0xc_06c9_3da4_bade, 0xb_f07b_6ecd_ffd3,
    0xc_0bf9_2cf9_abca, 0xb_f5ab_5e22_f0bf, 0xb_fadb_4d77_e1ab, 0xc_20b8_ea4d_6f7a, 0xc_2246_fbd0_66d5,
    0xc_1129_1c4e_9cb6, 0xc_25e8_d9a2_6066, 0xc_2776_eb25_57c1, 0xc_2b18_c8f7_5152, 0xc_2ca6_da7a_48ad,
    0xc_1659_0ba3_8da2, 0xc_31d6_c9cf_3999, 0xc_1b88_faf8_7e8e, 0xc_3706_b924_2a85, 0xc_4696_8722_fd49,
    0xc_3c36_a879_1b71, 0xc_4bc6_7677_ee35, 0xc_6744_34a3_9a2c, 0xc_4d54_87fa_e590, 0xc_50f6_65cc_df21,
    0xc_6c74_23f8_8b18, 0xc_5284_774f_d67c, 0xc_5626_5521_d00d, 0xc_4166_97ce_0c5d, 0xc_57b4_66a4_c768,
    0xc_5ce4_55f9_b854, 0xc_6214_454e_a940, 0xc_71a4_134d_7c04, 0xc_87f1_e224_370f, 0xc_8d21_d179_27fb,
    0xc_76d4_02a2_6cf0, 0xc_7c03_f1f7_5ddc, 0xc_9251_c0ce_18e7, 0xc_7862_1425_644b, 0xc_9781_b023_09d3,
    0xc_7d92_037a_5537, 0xc_a89f_8fa4_d3f2, 0xc_82c1_f2cf_4623, 0xc_9cb1_9f77_fabf, 0xc_adcf_7ef9_c4de,
    0xc_9e3f_b0fa_f21a, 0xc_b2ff_6e4e_b5ca, 0xc_b82f_5da3_a6b6, 0xc_a1e1_8ecc_ebab, 0xc_c28f_3c4d_888e,
    0xc_bd5f_4cf8_97a2, 0xc_a36f_a04f_e306, 0xc_a711_7e21_dc97, 0xc_ce7d_2c7a_61c1, 0xc_d21f_0a4c_5b52,
    0xc_c7bf_2ba2_797a, 0xc_d3ad_1bcf_52ad, 0xc_c94d_3d25_70d5, 0xc_f2cc_b7cc_f835, 0xc_d8dd_0b24_4399,
    0xc_f45a_c94f_ef90, 0xc_f7fc_a721_e921, 0xc_de0c_fa79_3485, 0xc_ccef_1af7_6a66, 0xc_f98a_b8a4_e07c,
    0xc_e33c_e9ce_2571, 0xc_e86c_d923_165d, 0xc_ed9c_c878_0749, 0xd_137a_654d_9518, 0xc_feba_a7f9_d168,
    0xd_18aa_54a2_8604, 0xd_1dda_43f7_76f0, 0xd_1a38_6625_7d5f, 0xd_03ea_974e_c254, 0xd_1f68_557a_6e4b,
    0xd_230a_334c_67dc, 0xd_091a_86a3_b340, 0xd_2498_44cf_5f37, 0xd_3428_12ce_31fb, 0xd_0e4a_75f8_a42c,
    0xd_3958_0223_22e7, 0xd_29c8_3424_5023, 0xd_3e87_f178_13d3, 0xd_43b7_e0cd_04bf, 0xd_2ef8_2379_410f,
    0xd_4545_f24f_fc1a, 0xd_48e7_d021_f5ab, 0xd_5a05_afa3_bfca, 0xd_4a75_e1a4_ed06, 0xd_4e17_bf76_e697,
    0xd_4fa5_d0f9_ddf2, 0xd_5f35_9ef8_b0b6, 0xd_7ab3_5d24_5cad, 0xd_54d5_c04e_cede, 0xd_6465_8e4d_a1a2,
    0xd_7fe3_4c79_4d99, 0xd_6995_7da2_928e, 0xd_8513_3bce_3e85, 0xd_6ec5_6cf7_837a, 0xd_7053_7e7a_7ad5,
    0xd_73f5_5c4c_7466, 0xd_7583_6dcf_6bc1, 0xd_9f02_e876_f321, 0xd_9b61_0aa4_f990, 0xd_8a43_2b23_2f71,
    0xd_a090_f9f9_ea7c, 0xd_a5c0_e94e_db68, 0xd_8f73_1a78_205d, 0xd_aaf0_d8a3_cc54, 0xd_94a3_09cd_1149,
    0xd_b020_c7f8_bd40, 0xd_bfb0_95f7_9004, 0xd_9631_1b50_08a4, 0xd_99d2_f922_0235, 0xd_b550_b74d_ae2c,
    0xd_c13e_a77a_875f, 0xd_c4e0_854c_80f0, 0xd_c66e_96cf_784b, 0xd_ca10_74a1_71dc, 0xd_e58e_32cd_1dd3,
    0xd_cb9e_8624_6937, 0xd_ba80_a6a2_9f18, 0xd_d0ce_7579_5a23, 0xd_d5fe_64ce_4b0f, 0xd_eabe_2222_0ebf,
    0xd_db2e_5423_3bfb, 0xd_e05e_4378_2ce7, 0xd_ec4c_33a5_061a, 0xe_063b_e04d_bab6, 0xd_efee_1176_ffab,
    0xe_0b6b_cfa2_aba2, 0xd_f17c_22f9_f706, 0xe_109b_bef7_9c8e, 0xd_f6ac_124e_e7f2, 0xe_1229_d07a_93e9,
    0xe_15cb_ae4c_8d7a, 0xd_fbdc_01a3_d8de, 0xe_010b_f0f8_c9ca, 0xe_26e9_8dce_5799, 0xe_1759_bfcf_84d5,
    0xe_2c19_7d23_4885, 0xe_1afb_9da1_7e66, 0xe_3149_6c78_3971, 0xe_1c89_af24_75c1, 0xe_3679_5bcd_2a5d,
    0xe_4797_3b4e_f47c, 0xe_3ba9_4b22_1b49, 0xe_21b9_9e79_66ad, 0xe_3d37_5ca5_12a4, 0xe_40d9_3a77_0c35,
    0xe_4cc7_2aa3_e568, 0xe_4267_4bfa_0390, 0xe_51f7_19f8_d654, 0xe_6d74_d824_824b, 0xe_5727_094d_c740,
    0xe_4609_29cb_fd21, 0xe_72a4_c779_7337, 0xe_5c56_f8a2_b82c, 0xe_6186_e7f7_a918, 0xe_66b6_d74c_9a04,
    0xe_77d4_b6ce_6423, 0xe_9352_74fa_101a, 0xe_6844_e8cf_915f, 0xe_8e22_85a5_1f2e, 0xe_6be6_c6a1_8af0,
    0xe_91c4_6377_18bf, 0xe_7d04_a623_550f, 0xe_96f4_52cc_09ab, 0xe_9882_644f_0106, 0xe_8234_9578_45fb,
    0xe_9db2_53a3_f1f2, 0xe_8764_84cd_36e7, 0xe_a2e2_42f8_e2de, 0xe_8c94_7422_27d3, 0xe_b272_10f7_b5a2,
    0xe_b7a2_004c_a68e, 0xe_b930_11cf_9de9, 0xe_bcd1_efa1_977a, 0xe_a812_324d_d3ca, 0xe_d31f_be78_5285,
    0xe_be60_0124_8ed5, 0xe_c201_def6_8866, 0xe_c38f_f079_7fc1, 0xe_ad42_21a2_c4b6, 0xe_d84f_adcd_4371,
    0xe_c8bf_dfce_70ad, 0xe_cdef_cf23_6199, 0xe_dd7f_9d22_345d, 0xe_f8fd_5b4d_e054, 0xe_e2af_8c77_2549,
    0xe_fe2d_4aa2_d140, 0xe_e43d_9dfa_1ca4, 0xe_e7df_7bcc_1635, 0xe_e96d_8d4f_0d90, 0xf_035d_39f7_c22c,
    0xe_ee9d_7ca3_fe7c, 0xe_f3cd_6bf8_ef68, 0xf_19ab_08ce_7d37, 0xf_088d_294c_b318, 0xf_1eda_f823_6e23,
    0xf_0a1b_3acf_aa73, 0xf_240a_e778_5f0f, 0xf_0dbd_18a1_a404, 0xf_2e6a_c622_40e7, 0xf_0f4b_2a24_9b5f,
    0xf_293a_d6cd_4ffb, 0xf_12ed_07f6_94f0, 0xf_3a58_b64f_1a1a, 0xf_147b_1979_8c4b, 0xf_3dfa_9421_13ab,
    0xf_3f88_a5a4_0b06, 0xf_44b8_94f8_fbf2, 0xf_339a_b577_31d3, 0xf_6036_5324_a7e9, 0xf_49e8_844d_ecde,
    0xf_3528_c6fa_292e, 0xf_63d8_30f6_a17a, 0xf_38ca_a4cc_22bf, 0xf_4f18_73a2_ddca, 0xf_6566_4279_98d5,
    0xf_5448_62f7_ceb6, 0xf_5978_524c_bfa2, 0xf_6a96_31ce_89c1, 0xf_5ea8_41a1_b08e, 0xf_8485_de77_3e5d,
    0xf_8613_effa_35b8, 0xf_89b5_cdcc_2f49, 0xf_6fc6_2123_7aad, 0xf_8b43_df4f_26a4, 0xf_8ee5_bd21_2035,
    0xf_74f6_1078_6b99, 0xf_9073_cea4_1790, 0xf_7a25_ffcd_5c85, 0xf_a533_8bf7_db40, 0xf_7f55_ef22_4d71,
    0xf_aa63_7b4c_cc2c, 0xf_95a3_bdf9_087c, 0xf_af93_6aa1_bd18, 0xf_9ad3_ad4d_f968, 0xf_b121_7c24_b473,
    0xf_b4c3_59f6_ae04, 0xf_c5e1_3978_7823, 0xf_b651_6b79_a55f, 0xf_b9f3_494b_9ef0, 0xf_a003_9ca2_ea54,
    0xf_bb81_5ace_964b, 0xf_cb11_28cd_690f, 0xf_c0b1_4a23_8737, 0xf_d041_1822_59fb, 0xf_ebbe_d64e_05f2,
    0xf_d571_0777_4ae7, 0xf_daa0_f6cc_3bd3, 0xf_f0ee_c5a2_f6de, 0xf_dc2f_084f_332e, 0xf_dfd0_e621_2cbf,
    0xf_e15e_f7a4_241a, 0xf_f61e_b4f7_e7ca, 0xf_e68e_e6f9_1506, 0xf_fb4e_a44c_d8b6,
];

#[test]
fn sinpi_every_scaled_worst_case_near_zero() {
    // The proof of sinpi's tiny path (|x| < 2^−54, no rounding test) and of its near-zero
    // accurate path applies Lemma 2 with the worst cases of every binade below 2^−53. Those are set (b)
    // scaled by 2^e for e = 1..968, about a million inputs; the port's tests check four random
    // scalings of a sample, and its full worst-case run reads only the file's literal lines.
    // Near the top binades sin(πx) also departs from πx by up to 2^−54 ulp, so the hardness of
    // the scaled cases is not exactly that of set (b).
    let exponents: Vec<u64> = if cfg!(debug_assertions) {
        (1..=968).step_by(61).chain(955..=968).collect()
    } else {
        (1..=968).collect()
    };
    let check = |es: &[u64]| -> (usize, Vec<String>) {
        let mut r = Report::new("sinpi, scaled worst cases of sinpi.wc set (b)");
        for &e in es {
            for &m in &SINPI_WC_SET_B {
                let x = f64::from_bits(((1 + e) << 52) | m);
                r.sinpi(x);
            }
        }
        (r.checked, r.failures)
    };
    let (checked, failures): (usize, Vec<String>) = if cfg!(target_family = "wasm") {
        check(&exponents)
    } else {
        let chunks: Vec<&[u64]> = exponents.chunks(exponents.len().div_ceil(16)).collect();
        std::thread::scope(|s| {
            let handles: Vec<_> = chunks.iter().map(|c| s.spawn(|| check(c))).collect();
            handles.into_iter().map(|h| h.join().unwrap()).fold(
                (0, Vec::new()),
                |(n, mut f), (m, g)| {
                    f.extend(g);
                    (n + m, f)
                },
            )
        })
    };
    let mut r = Report::new("sinpi, scaled worst cases of sinpi.wc set (b)");
    r.checked = checked;
    r.failures = failures;
    r.finish();
}

#[test]
fn exp_near_the_midpoints_next_to_one() {
    // e^x is near the midpoint 1 + (2m+1)·2^−53 (or 1 − (2m+1)·2^−54 below 1) when x is
    // near ln of it: the inputs a few ulps around those points have 40 to 55 identical bits after the
    // round bit, more than the accurate path's proven error resolves.
    let mut r = Report::new("exp near midpoints next to 1");
    for m in 0..600u32 {
        let c = f64::from(2 * m + 1) * p2(-53);
        let up = c - c * c / 2.0 + c * c * c / 3.0; // ln(1 + c), within an ulp or two
        let c2 = f64::from(2 * m + 1) * p2(-54);
        let down = -c2 - c2 * c2 / 2.0 - c2 * c2 * c2 / 3.0; // ln(1 − c2)
        for centre in [up, down, c, -c2] {
            for x in around(centre, if m < 64 { 24 } else { 4 }) {
                r.exp(x);
            }
        }
    }
    r.finish();
}

// ---------------------------------------------------------------------------------------------
// 3. Special values
// ---------------------------------------------------------------------------------------------

fn same_bits(got: f64, want: f64, what: &str) {
    assert_eq!(
        got.to_bits(),
        want.to_bits(),
        "{what}: got {got:e} ({:#018x}), want {want:e}",
        got.to_bits()
    );
}

#[test]
fn sinpi_and_cospi_exact_values_and_signs_of_zero() {
    // The crate's one-zero rule: every zero is +0, sinPi(±n) = +0 and cosPi(n + ½) = +0. (IEEE
    // 754-2019 §9.2.1's sinPi(−n) = −0 is the port's, which `math.rs`'s unit tests check; the
    // public function adds +0.)
    let two41 = p2(41);
    let two51 = p2(51);
    let sin_cases = [
        (0.0, 0.0),
        (-0.0, 0.0),
        (1.0, 0.0),
        (-1.0, 0.0),
        (2.0, 0.0),
        (-2.0, 0.0),
        (two41, 0.0),
        (-two41, 0.0),
        (two41 + 1.0, 0.0),
        (-(two41 + 1.0), 0.0),
        (p2(52) + 1.0, 0.0),
        (-(p2(52) + 1.0), 0.0),
        (f64::MAX, 0.0),
        (-f64::MAX, 0.0),
        (0.5, 1.0),
        (-0.5, -1.0),
        (1.5, -1.0),
        (-1.5, 1.0),
        (2.5, 1.0),
        (-2.5, -1.0),
        (two41 + 0.5, 1.0),
        (two41 + 1.5, -1.0),
        (-(two41 + 0.5), -1.0),
        (-(two41 + 1.5), 1.0),
        (two51 + 0.5, 1.0),
        (two51 + 1.5, -1.0),
        (-(two51 + 0.5), -1.0),
        (p2(40) + 0.5, 1.0),
        (-(p2(40) + 1.5), 1.0),
    ];
    for (x, want) in sin_cases {
        same_bits(v(math::sinpi(x)), want, &format!("sinpi({x:e})"));
    }
    let cos_cases = [
        (0.0, 1.0),
        (-0.0, 1.0),
        (1.0, -1.0),
        (-1.0, -1.0),
        (2.0, 1.0),
        (3.0, -1.0),
        (0.5, 0.0),
        (-0.5, 0.0),
        (1.5, 0.0),
        (-1.5, 0.0),
        (two51 + 0.5, 0.0),
        (-(two51 + 0.5), 0.0),
        (two41 + 0.5, 0.0),
        (-(two41 + 0.5), 0.0),
        (two41 + 1.0, -1.0),
        (-(two41 + 1.0), -1.0),
        (two41 + 2.0, 1.0),
        (p2(52) + 1.0, -1.0),
        (p2(53), 1.0),
        (f64::MAX, 1.0),
        (-f64::MAX, 1.0),
        (p2(40) + 1.0, -1.0),
    ];
    for (x, want) in cos_cases {
        same_bits(v(math::cospi(x)), want, &format!("cospi({x:e})"));
    }
}

#[test]
fn exp_ln_and_atan2_special_values() {
    let pi = hf("0x1.921fb54442d18p+1");
    let half_pi = hf("0x1.921fb54442d18p+0");
    let quarter_pi = hf("0x1.921fb54442d18p-1");
    let three_quarter_pi = hf("0x1.2d97c7f3321d2p+1");
    let inf = f64::INFINITY;
    for (x, want) in [
        (0.0, 1.0),
        (-0.0, 1.0),
        (inf, inf),
        (-inf, 0.0),
        (f64::MAX, inf),
        (-f64::MAX, 0.0),
        (1000.0, inf),
        (-1000.0, 0.0),
    ] {
        check_owed(math::exp(x), &[x], want, &format!("exp({x:e})"));
    }
    for (x, want) in [(1.0, 0.0), (0.0, -inf), (-0.0, -inf), (inf, inf)] {
        check_owed(math::ln(x), &[x], want, &format!("ln({x:e})"));
    }
    // C99 Annex F.10.1.4, under the contract: a finite pair owes its value, an infinite argument
    // is refused.
    let y_fin = 3.5;
    let cases = [
        (0.0, -0.0, pi),
        (-0.0, -0.0, -pi),
        (0.0, 0.0, 0.0),
        (-0.0, 0.0, -0.0),
        (0.0, -2.0, pi),
        (-0.0, -2.0, -pi),
        (0.0, 2.0, 0.0),
        (-0.0, 2.0, -0.0),
        (-y_fin, 0.0, -half_pi),
        (-y_fin, -0.0, -half_pi),
        (y_fin, 0.0, half_pi),
        (y_fin, -0.0, half_pi),
        (y_fin, -inf, pi),
        (-y_fin, -inf, -pi),
        (y_fin, inf, 0.0),
        (-y_fin, inf, -0.0),
        (inf, 2.0, half_pi),
        (-inf, 2.0, -half_pi),
        (inf, -2.0, half_pi),
        (-inf, -2.0, -half_pi),
        (inf, -inf, three_quarter_pi),
        (-inf, -inf, -three_quarter_pi),
        (inf, inf, quarter_pi),
        (-inf, inf, -quarter_pi),
        (0.0, inf, 0.0),
        (-0.0, -inf, -pi),
        (inf, 0.0, half_pi),
        (-inf, -0.0, -half_pi),
        (f64::from_bits(1), inf, 0.0),
        (f64::MAX, -inf, pi),
    ];
    for (y, x, want) in cases {
        check_owed(
            math::atan2(y, x),
            &[y, x],
            want,
            &format!("atan2({y:e}, {x:e})"),
        );
    }
}

/// NaN inputs with assorted signs and payloads, quiet and signalling.
fn nans() -> [f64; 6] {
    [
        f64::NAN,
        -f64::NAN,
        f64::from_bits(0x7ff8_0000_0000_0001),
        f64::from_bits(0xfff4_0000_dead_beef),
        f64::from_bits(0x7ff0_0000_0000_0001), // signalling
        f64::from_bits(0xfff0_0000_0000_0001),
    ]
}

#[test]
fn proven_functions_never_return_nan_and_refuse_what_has_no_value() {
    // math.rs: a `Real` or an error, never NaN, whatever the input's sign or payload: an argument
    // that is not finite is `NotFinite`; a logarithm of a negative number `OutsideDomain`.
    let mut bad = Vec::new();
    let mut check = |what: String, got: Result<Real, NumError>, want: NumError| {
        if got != Err(want) {
            bad.push(format!("{what} = {got:?}, want {want:?}"));
        }
    };
    let not_finite = NumError::NotFinite;
    for n in nans() {
        check(
            format!("sinpi({:#x})", n.to_bits()),
            math::sinpi(n),
            not_finite,
        );
        check(
            format!("cospi({:#x})", n.to_bits()),
            math::cospi(n),
            not_finite,
        );
        check(format!("exp({:#x})", n.to_bits()), math::exp(n), not_finite);
        check(format!("ln({:#x})", n.to_bits()), math::ln(n), not_finite);
        for w in [1.0, -1.0, 0.0, f64::INFINITY, n] {
            check(
                format!("atan2({:#x}, {w:e})", n.to_bits()),
                math::atan2(n, w),
                not_finite,
            );
            check(
                format!("atan2({w:e}, {:#x})", n.to_bits()),
                math::atan2(w, n),
                not_finite,
            );
        }
    }
    for x in [f64::INFINITY, -f64::INFINITY] {
        check(format!("sinpi({x})"), math::sinpi(x), not_finite);
        check(format!("cospi({x})"), math::cospi(x), not_finite);
        check(format!("exp({x})"), math::exp(x), not_finite);
        check(format!("ln({x})"), math::ln(x), not_finite);
    }
    for x in [-1.0, -f64::from_bits(1), -f64::MAX] {
        check(format!("ln({x:e})"), math::ln(x), NumError::OutsideDomain);
    }
    assert!(
        bad.is_empty(),
        "not refused as documented:\n{}",
        bad.join("\n")
    );
}

#[test]
fn every_function_in_the_use_these_table_refuses_instead_of_returning_nan() {
    // math.rs's "Use these" table lists `sqrt`, `fma`, `abs`, `floor`, `ceil` and
    // `round_ties_even` beside the five ports, and chapter 21 says NaN sign and payload are not
    // deterministic across targets. A NaN made by the hardware (the square root of a negative
    // number) has the platform's default bits, and a NaN argument's payload passes through `floor`
    // and friends differently; so none may come out. Each is refused instead, the same error on
    // every target.
    let payload = f64::from_bits(0xfff4_0000_dead_beef);
    let mut cases: Vec<(String, Result<Real, NumError>, NumError)> = vec![
        ("sqrt(-1)".into(), math::sqrt(-1.0), NumError::OutsideDomain),
        (
            "sqrt(-inf)".into(),
            math::sqrt(f64::NEG_INFINITY),
            NumError::NotFinite,
        ),
        (
            "sqrt(-0x1p-1074)".into(),
            math::sqrt(-f64::from_bits(1)),
            NumError::OutsideDomain,
        ),
        (
            "sqrt(NaN with a payload)".into(),
            math::sqrt(payload),
            NumError::NotFinite,
        ),
    ];
    for n in nans() {
        let b = n.to_bits();
        let nf = NumError::NotFinite;
        cases.push((format!("sqrt({b:#x})"), math::sqrt(n), nf));
        cases.push((format!("fma({b:#x}, 1, 1)"), math::fma(n, 1.0, 1.0), nf));
        cases.push((format!("abs({b:#x})"), math::abs(n), nf));
        cases.push((format!("floor({b:#x})"), math::floor(n), nf));
        cases.push((format!("ceil({b:#x})"), math::ceil(n), nf));
        cases.push((
            format!("round_ties_even({b:#x})"),
            math::round_ties_even(n),
            nf,
        ));
    }
    let bad: Vec<String> = cases
        .iter()
        .filter(|(_, got, want)| *got != Err(*want))
        .map(|(what, got, want)| format!("{what} = {got:?}, want {want:?}"))
        .collect();
    assert!(
        bad.is_empty(),
        "a recommended function did not refuse as documented:\n{}",
        bad.join("\n")
    );
    // −0 is not negative: sqrt(−0) = −0, as IEEE 754 requires.
    assert_eq!(v(math::sqrt(-0.0)).to_bits(), (-0.0f64).to_bits());
}

#[test]
fn unproven_functions_refuse_instead_of_returning_nan() {
    // math::unproven promises the same bits on every target and keeps math's contract, so where
    // libm would make a NaN (an invalid operation, or a NaN argument), it refuses instead.
    use geom_num::math::unproven as un;
    let (nf, od) = (NumError::NotFinite, NumError::OutsideDomain);
    let mut cases: Vec<(String, Result<Real, NumError>, NumError)> = vec![
        ("sin(inf)".into(), un::sin(f64::INFINITY), nf),
        ("cos(-inf)".into(), un::cos(f64::NEG_INFINITY), nf),
        ("tan(inf)".into(), un::tan(f64::INFINITY), nf),
        ("asin(2)".into(), un::asin(2.0), od),
        ("acos(-2)".into(), un::acos(-2.0), od),
        ("ln(-1)".into(), un::ln(-1.0), od),
        ("log2(-1)".into(), un::log2(-1.0), od),
        ("log10(-1)".into(), un::log10(-1.0), od),
        ("pow(-1, 0.5)".into(), un::pow(-1.0, 0.5), od),
        ("sinpi(inf)".into(), un::sinpi(f64::INFINITY), nf),
        ("cospi(-inf)".into(), un::cospi(f64::NEG_INFINITY), nf),
    ];
    for n in nans() {
        let b = n.to_bits();
        cases.push((format!("sin({b:#x})"), un::sin(n), nf));
        cases.push((format!("exp({b:#x})"), un::exp(n), nf));
        cases.push((format!("atan2({b:#x}, 1)"), un::atan2(n, 1.0), nf));
        cases.push((format!("hypot({b:#x}, 1)"), un::hypot(n, 1.0), nf));
        cases.push((format!("cbrt({b:#x})"), un::cbrt(n), nf));
        cases.push((format!("atan({b:#x})"), un::atan(n), nf));
        cases.push((format!("powi({b:#x}, 2)"), un::powi(n, 2), nf));
    }
    let bad: Vec<String> = cases
        .iter()
        .filter(|(_, got, want)| *got != Err(*want))
        .map(|(what, got, want)| format!("{what} = {got:?}, want {want:?}"))
        .collect();
    assert!(
        bad.is_empty(),
        "not refused as documented:\n{}",
        bad.join("\n")
    );
}

#[test]
fn chapter_21_the_kernel_never_produces_nan() {
    // Chapter 21: "The kernel never produces NaN. Scalars are checked at construction", enforced
    // by "a NaN-free scalar type; fuzzing; tripwires in debug builds". Chapter 16.2 says how for
    // partial operations: "For each square root, division, logarithm and so on, the interval of
    // its argument over the field's domain must lie where the operation is defined ... So
    // evaluation never meets an undefined point and never makes a NaN." The point functions
    // check their domains first and return a `Real` (finite by construction) or an error, so no
    // NaN can come out at all (the two tests before this one: refused as documented).
    //
    // Checked here: (1) every point function gives a value on its whole domain, and refuses only
    // what the contract says (an overflow of `exp`, the pole of `ln` at zero); (2) the interval
    // versions, the certified API, never produce a NaN or an invalid interval for any interval,
    // inside or outside the domain: they return an error.
    use geom_num::interval::Interval;
    let mut rng = Rng(0x5eed_0021);
    let mut bad = Vec::new();
    for _ in 0..scale(200_000) {
        let a = rng.finite();
        let b = rng.finite();
        let pos = a.abs();
        // exp overflows from 0x1.62e42fefa39fp+9 up; ln has its pole at 0.
        let exp_overflows = a >= f64::from_bits(0x4086_2e42_fefa_39f0);
        for (what, r, refusable) in [
            ("sinpi", math::sinpi(a), false),
            ("cospi", math::cospi(a), false),
            ("exp", math::exp(a), exp_overflows),
            ("ln", math::ln(pos), pos == 0.0),
            ("sqrt", math::sqrt(pos), false),
            ("atan2", math::atan2(a, b), false),
            ("floor", math::floor(a), false),
            ("abs", math::abs(a), false),
        ] {
            match r {
                Ok(x) if x.get().is_finite() && !refusable => {}
                Err(NumError::NotFinite) if refusable => {}
                _ => bad.push(format!("{what} at {a:e}, {b:e} (in its domain): {r:?}")),
            }
        }
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let x = Interval::new(lo, hi).unwrap();
        let narrow = Interval::new(lo, lo.next_up().min(f64::MAX)).unwrap();
        for i in [x, narrow] {
            for (what, r) in [
                ("sinpi", i.sinpi()),
                ("cospi", i.cospi()),
                ("exp", i.exp()),
                ("ln", i.ln()),
                ("sqrt", i.sqrt()),
            ] {
                if let Ok(v) = r
                    && !(v.lo().is_finite() && v.hi().is_finite() && v.lo() <= v.hi())
                {
                    bad.push(format!(
                        "Interval::{what}([{:e}, {:e}]) = [{:e}, {:e}]",
                        i.lo(),
                        i.hi(),
                        v.lo(),
                        v.hi()
                    ));
                }
            }
        }
        if bad.len() > 20 {
            break;
        }
    }
    assert!(bad.is_empty(), "NaN or invalid bounds:\n{}", bad.join("\n"));
}

#[test]
fn total_on_every_bit_pattern_and_refused_only_where_documented() {
    // No input panics (run this file in a debug build too: overflow checks are on there), and an
    // error comes only for an argument that is not finite, outside the documented domain, at a
    // pole or past an overflow: exactly the contract of `geom_num::math`.
    let mut rng = Rng(0x5eed_70a1);
    let mut bad = Vec::new();
    let (nf, od) = (NumError::NotFinite, NumError::OutsideDomain);
    // A result's outcome: Ok(()) for a value, or its error.
    let status = |r: Result<Real, NumError>| r.map(|_| ());
    for _ in 0..scale(1_000_000) {
        let a = f64::from_bits(rng.next());
        let b = f64::from_bits(rng.next());
        let s = math::sinpi(a);
        let c = math::cospi(a);
        let e = math::exp(a);
        let l = math::ln(a);
        let t = math::atan2(a, b);
        let finite_or = |e: NumError| if a.is_finite() { Ok(()) } else { Err(e) };
        if status(s) != finite_or(nf) || status(c) != finite_or(nf) {
            bad.push(format!("sinpi/cospi refused wrongly at {a:e}: {s:?} {c:?}"));
        }
        let e_owed = if !a.is_finite() || a >= f64::from_bits(0x4086_2e42_fefa_39f0) {
            Err(nf)
        } else {
            Ok(())
        };
        if status(e) != e_owed {
            bad.push(format!("exp refused wrongly at {a:e}: {e:?}"));
        }
        let l_owed = if !a.is_finite() || a == 0.0 {
            Err(nf)
        } else if a < 0.0 {
            Err(od)
        } else {
            Ok(())
        };
        if status(l) != l_owed {
            bad.push(format!("ln refused wrongly at {a:e}: {l:?}"));
        }
        let t_owed = if a.is_finite() && b.is_finite() {
            Ok(())
        } else {
            Err(nf)
        };
        if status(t) != t_owed {
            bad.push(format!("atan2 refused wrongly at ({a:e}, {b:e}): {t:?}"));
        }
        if let Ok(s) = s
            && !(-1.0..=1.0).contains(&s.get())
        {
            bad.push(format!("sinpi({a:e}) = {s:?} outside [−1, 1]"));
        }
        if let Ok(t) = t
            && t.get().abs() > core::f64::consts::PI
        {
            bad.push(format!("atan2({a:e}, {b:e}) = {t:?} outside [−π, π]"));
        }
        if bad.len() > 20 {
            break;
        }
    }
    assert!(
        bad.is_empty(),
        "{}",
        bad.join(
            "
"
        )
    );
}

// ---------------------------------------------------------------------------------------------
// 4. Identities and monotonicity that correct rounding forces
// ---------------------------------------------------------------------------------------------

#[test]
fn symmetries_and_periodicity_hold_bit_for_bit() {
    let mut rng = Rng(0x5eed_0009);
    let mut bad = Vec::new();
    for i in 0..scale(400_000) {
        let x = match i % 4 {
            0 => rng.sign() * rng.unit() * 4.0,
            1 => rng.sign() * rng.binade_in(-80, 60),
            2 => rng.sign() * (rng.range(0, 1 << 30) as f64 + rng.unit()),
            _ => rng.sign() * rng.binade_in(-1074, 1023),
        };
        let s = v(math::sinpi(x));
        // −sinpi(x), with a zero as +0 (the one-zero rule): adding +0 changes only −0.
        if v(math::sinpi(-x)).to_bits() != (-s + 0.0).to_bits() {
            bad.push(format!("sinpi(−x) ≠ −sinpi(x) at {x:e}"));
        }
        if v(math::cospi(-x)).to_bits() != v(math::cospi(x)).to_bits() {
            bad.push(format!("cospi(−x) ≠ cospi(x) at {x:e}"));
        }
        let x2 = x + 2.0;
        if x2 - 2.0 == x && x != 0.0 && (x2 != 0.0) && x.fract() != 0.0 {
            if v(math::sinpi(x2)).to_bits() != s.to_bits() {
                bad.push(format!("sinpi(x + 2) ≠ sinpi(x) at {x:e}"));
            }
            if v(math::cospi(x2)).to_bits() != v(math::cospi(x)).to_bits() {
                bad.push(format!("cospi(x + 2) ≠ cospi(x) at {x:e}"));
            }
        }
        let h = x + 0.5;
        if h != x
            && h - 0.5 == x
            && (2.0 * x).fract() != 0.0
            && v(math::cospi(x)).to_bits() != v(math::sinpi(h)).to_bits()
        {
            bad.push(format!("cospi(x) ≠ sinpi(x + ½) at {x:e}"));
        }
        let m = 1.0 - x;
        if 1.0 - m == x && x.fract() != 0.0 && v(math::sinpi(m)).to_bits() != s.to_bits() {
            bad.push(format!("sinpi(1 − x) ≠ sinpi(x) at {x:e}"));
        }
        let y = rng.sign() * rng.binade_in(-1074, 1023);
        if v(math::atan2(-y, x)).to_bits() != (-v(math::atan2(y, x))).to_bits() {
            bad.push(format!("atan2(−y, x) ≠ −atan2(y, x) at ({y:e}, {x:e})"));
        }
        if bad.len() > 40 {
            break;
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn atan2_is_invariant_under_exact_power_of_two_scaling() {
    // atan2(2^k·y, 2^k·x) is the same real number, so correct rounding gives the same bits. This
    // drives the 2^920 and ½ rescalings and subnormal inputs without a reference.
    let mut rng = Rng(0x5eed_000a);
    let mut bad = Vec::new();
    for _ in 0..scale(300_000) {
        let ex = rng.range(-1074, 1023);
        let ey = (ex + rng.range(-60, 60)).clamp(-1074, 1023);
        let x = rng.sign() * rng.binade(ex);
        let y = rng.sign() * rng.binade(ey);
        let k = rng.range(-1100, 1100) as i32;
        let s = if k >= 0 {
            p2(k.min(1023))
        } else {
            p2(k.max(-1022))
        };
        let (ys, xs) = (y * s, x * s);
        if !(ys.is_finite() && xs.is_finite())
            || ys == 0.0
            || xs == 0.0
            || ys / s != y
            || xs / s != x
        {
            continue;
        }
        let a = v(math::atan2(y, x));
        let b = v(math::atan2(ys, xs));
        if a.to_bits() != b.to_bits() {
            bad.push(format!(
                "atan2({y:e}, {x:e}) = {a:e} but scaled by 2^{k}: {b:e} (truth {:e})",
                oracle::atan2(y, x)
            ));
            if bad.len() > 20 {
                break;
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Checks that `f` is monotone on the `n` doubles from `start` upwards.
fn sweep(
    bad: &mut Vec<String>,
    name: &str,
    f: &dyn Fn(f64) -> f64,
    start: f64,
    n: usize,
    increasing: bool,
) {
    let mut x = start;
    let mut prev = f(x);
    for _ in 0..n {
        let nx = x.next_up();
        let v = f(nx);
        let ok = if increasing { v >= prev } else { v <= prev };
        if !ok {
            bad.push(format!(
                "{name} not monotone between {x:e} ({prev:e}) and {nx:e} ({v:e})"
            ));
        }
        prev = v;
        x = nx;
    }
}

/// `x` moved down by `n` doubles.
fn down(x: f64, n: usize) -> f64 {
    (0..n).fold(x, |v, _| v.next_down())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one family of checks, each spot listed beside its reason"
)]
fn monotone_on_consecutive_doubles() {
    let mut bad = Vec::new();
    let n = scale(20_000);
    let h = n / 2;
    // sinpi increases on [−½, ½]; every sweep stays inside.
    for start in [
        hf("0x1.2p-5"),
        hf("-0x1.2p-5"),
        1e-300,
        0.25,
        hf("0x1p-54"),
        hf("0x1p-960"),
        hf("0x1p-1022"),
    ] {
        sweep(
            &mut bad,
            "sinpi",
            &|x| v(math::sinpi(x)),
            down(start, h),
            n,
            true,
        );
    }
    sweep(&mut bad, "sinpi", &|x| v(math::sinpi(x)), -0.5, n, true);
    sweep(
        &mut bad,
        "sinpi",
        &|x| v(math::sinpi(x)),
        down(0.5, n),
        n,
        true,
    );
    // cospi decreases on [0, 1].
    for start in [hf("0x1p-12"), hf("0x1.ccf6429be6621p-29"), 0.5, 0.75] {
        sweep(
            &mut bad,
            "cospi",
            &|x| v(math::cospi(x)),
            down(start, h),
            n,
            false,
        );
    }
    sweep(&mut bad, "cospi", &|x| v(math::cospi(x)), 0.0, n, false);
    sweep(
        &mut bad,
        "cospi",
        &|x| v(math::cospi(x)),
        down(1.0, n),
        n,
        false,
    );
    // exp and ln increase everywhere. An overflow of exp, refused as `NotFinite`, stands for +∞,
    // which is above every finite value; no other error can come from a finite argument.
    for start in [
        hf("0x1.62e42fefa39fp+9"),
        hf("-0x1.74910d52d3052p+9"),
        hf("-0x1.6232bdd7abcd2p+9"),
        hf("0x1p-54"),
        hf("-0x1p-54"),
        0.0,
        1.0,
    ] {
        sweep(
            &mut bad,
            "exp",
            &|x| math::exp(x).map_or(f64::INFINITY, Real::get),
            down(start, h),
            n,
            true,
        );
    }
    for start in [
        1.0,
        1.0 - p2(-8),
        core::f64::consts::SQRT_2,
        f64::MIN_POSITIVE,
        0.5,
        2.0,
    ] {
        sweep(&mut bad, "ln", &|x| v(math::ln(x)), down(start, h), n, true);
    }
    // atan2(y, 1) increases and atan2(y, −1) decreases in y > 0.
    for start in [
        hf("0x1.d12ed0af1a27fp-27"),
        1.0,
        p2(-95),
        p2(-1022),
        p2(-54),
    ] {
        sweep(
            &mut bad,
            "atan2(y, 1)",
            &|y| v(math::atan2(y, 1.0)),
            down(start, h),
            n,
            true,
        );
        sweep(
            &mut bad,
            "atan2(y, -1)",
            &|y| v(math::atan2(y, -1.0)),
            down(start, h),
            n,
            false,
        );
        sweep(
            &mut bad,
            "atan2(1, x)",
            &|x| v(math::atan2(1.0, x)),
            down(start, h),
            n,
            false,
        );
    }
    assert!(
        bad.is_empty(),
        "{}",
        bad.iter().take(30).cloned().collect::<Vec<_>>().join(
            "
"
        )
    );
}

// ---------------------------------------------------------------------------------------------
// 5. Determinism
// ---------------------------------------------------------------------------------------------

/// The inputs of the determinism checks: a fixed, generated set covering every path.
fn determinism_inputs(n: usize) -> Vec<(f64, f64)> {
    let mut rng = Rng(0xd37e_2a11);
    (0..n)
        .map(|i| match i % 5 {
            0 => (
                rng.sign() * rng.binade_in(-1074, 60),
                rng.sign() * rng.binade_in(-1074, 1023),
            ),
            1 => (
                rng.sign() * rng.unit() * 4.0,
                rng.sign() * rng.binade_in(-60, 60),
            ),
            2 => (-746.0 + rng.unit() * 1456.0, rng.finite()),
            3 => (
                1.0 + rng.sign() * rng.binade_in(-60, -1),
                rng.sign() * rng.binade_in(-900, 900),
            ),
            _ => (rng.finite(), rng.finite()),
        })
        .collect()
}

/// One result as a 64-bit word: a value's bits, or for an error a NaN pattern that no value has
/// (a `Real` is never NaN): `0x7ff8_0000_0000_0000` plus the error's number in
/// `tests/determinism.rs` (3 not finite, 8 outside a domain).
fn word(r: Result<Real, NumError>) -> u64 {
    match r {
        Ok(x) => x.get().to_bits(),
        Err(NumError::NotFinite) => 0x7ff8_0000_0000_0003,
        Err(NumError::OutsideDomain) => 0x7ff8_0000_0000_0008,
        Err(e) => panic!("a maths function returned {e:?}"),
    }
}

/// All five functions' results on one input pair, in a fixed order.
fn outputs(a: f64, b: f64) -> [u64; 5] {
    [
        word(math::sinpi(a)),
        word(math::cospi(a)),
        word(math::exp(a)),
        word(math::ln(a.abs())),
        word(math::atan2(a, b)),
    ]
}

use geom_test::golden;

/// FNV-1a over a stream of 64-bit words.
fn fnv(words: impl Iterator<Item = u64>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for w in words {
        for byte in w.to_le_bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "no threads on wasm32-wasip1")]
fn identical_bits_on_every_thread_count() {
    let inputs = determinism_inputs(scale(200_000));
    let serial: Vec<[u64; 5]> = inputs.iter().map(|&(a, b)| outputs(a, b)).collect();
    for threads in [2usize, 3, 8, 16] {
        let chunk = inputs.len().div_ceil(threads);
        let parallel: Vec<[u64; 5]> = std::thread::scope(|s| {
            let handles: Vec<_> = inputs
                .chunks(chunk)
                .map(|c| s.spawn(move || c.iter().map(|&(a, b)| outputs(a, b)).collect::<Vec<_>>()))
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        });
        assert!(parallel == serial, "results differ with {threads} threads");
    }
}

/// The golden in the golden store (`geom-test/golden/store.json`): the SHA-256 of the words
/// [`FNV_GOLDEN`] hashes, each as eight little-endian bytes, in order. Piece G19 moved this golden
/// to SHA-256, the hash chapters 21 and 28.13 give every golden, over the same bytes; the FNV-1a
/// value is kept and still checked, to show the bytes are the ones it was fixed on. `kernel.yml`
/// runs it on every target and build mode (`golden run`), each time twice in the process on two
/// differently perturbed heaps (`geom_test::golden::check`).
const GOLDEN: &str = "geom-num/proven-maths";

/// The 64-bit FNV-1a hash of the five functions over `determinism_inputs(100_000)`, each result as
/// a `word` (a value's bits, or a marker for its error), recorded on x86_64-pc-windows-gnu: the
/// golden until piece G19 moved it to SHA-256 ([`GOLDEN`]). Correct rounding fixes every bit, so
/// every target must reproduce it. The same value was reproduced
/// with `-C target-cpu=native` (hardware FMA in `libm::fma`), and this test passes on
/// wasm32-wasip1 under wasmtime (`proptest` is a dev-dependency without its fork and timeout
/// features, so the test targets build there).
///
/// It was `0x6053_4724_d7f0_92a9` while the functions returned raw doubles. With piece G02's
/// contract (`geom_num::math`: a `Real` or an error) the arguments that are not finite, the
/// overflows of `exp` and the pole of `ln` are errors where they were infinities or NaN, and a zero
/// `sinpi` is +0 where it was −0 for a negative whole argument; every finite value is the same.
const FNV_GOLDEN: u64 = 0x0f3a_941b_17ab_4c66;

#[test]
fn golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let inputs = determinism_inputs(100_000);
        let words: Vec<u64> = inputs.iter().flat_map(|&(a, b)| outputs(a, b)).collect();
        let mut sha = golden::Sha256::new();
        for w in &words {
            sha.write(&w.to_le_bytes());
        }
        (sha.finish(), words)
    });
    let h = fnv(checked.value.iter().copied());
    eprintln!(
        "golden hash on this target: SHA-256 {}, FNV-1a {h:#018x}",
        checked.digest
    );
    assert_eq!(
        h, FNV_GOLDEN,
        "FNV-1a hash differs on this target: {h:#018x}"
    );
    checked.verdict().unwrap_or_else(|e| panic!("{e}"));
}

// ---------------------------------------------------------------------------------------------
// 6. The written rules, read from the source
// ---------------------------------------------------------------------------------------------

const SOURCES: [(&str, &str); 9] = [
    ("src/math.rs", include_str!("../src/math.rs")),
    (
        "src/math/enclose.rs",
        include_str!("../src/math/enclose.rs"),
    ),
    (
        "src/math/sincospi.rs",
        include_str!("../src/math/sincospi.rs"),
    ),
    ("src/math/exp.rs", include_str!("../src/math/exp.rs")),
    ("src/math/log.rs", include_str!("../src/math/log.rs")),
    ("src/math/atan2.rs", include_str!("../src/math/atan2.rs")),
    ("src/math/hexf.rs", include_str!("../src/math/hexf.rs")),
    (
        "src/math/support.rs",
        include_str!("../src/math/support.rs"),
    ),
    (
        "src/math/unproven.rs",
        include_str!("../src/math/unproven.rs"),
    ),
];

/// The kernel (non-test) lines of a source file with their line numbers: everything before
/// `mod tests`, without items marked `#[cfg(test)]`.
fn kernel_lines(src: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut skip_item = false;
    for (i, line) in src.lines().enumerate() {
        let t = line.trim();
        if t == "#[cfg(test)]" {
            skip_item = true;
            continue;
        }
        if skip_item {
            if t.starts_with("mod tests") {
                break;
            }
            if t.starts_with("#[") {
                continue;
            }
            if t.ends_with(';') {
                skip_item = false;
            }
            continue;
        }
        if t.starts_with("//") {
            continue; // comments and docs
        }
        out.push((i + 1, line));
    }
    out
}

#[test]
fn hex_float_reader_is_only_evaluated_at_compile_time() {
    // hexf.rs: "[`hf`] is only ever called to initialise `const` items, so that assertion is
    // evaluated by the compiler: a bad literal is a build error, never a run-time panic." A call
    // in a function body is an ordinary run-time call of a function containing `assert!`.
    let mut runtime = Vec::new();
    for (file, src) in SOURCES {
        let mut in_const = false;
        for (n, line) in kernel_lines(src) {
            let t = line.trim_start();
            let starts_const = t.starts_with("const ")
                || t.starts_with("pub const ")
                || t.starts_with("pub(crate) const ")
                || t.starts_with("static ");
            let is_fn = t.contains("const fn ");
            if starts_const && !is_fn && t.contains('=') {
                in_const = true;
            }
            if line.contains("hf(\"") && !in_const {
                runtime.push(format!("{file}:{n}: {}", line.trim()));
            }
            if in_const && t.ends_with(';') {
                in_const = false;
            }
        }
    }
    // The fix makes every use the macro `hf!`, which expands to an inline `const` block, so the
    // reader can only run at compile time: check that, and that nothing calls the reader itself.
    let hexf = include_str!("../src/math/hexf.rs");
    assert!(
        hexf.contains("const { $crate::math::hexf::parse($s) }"),
        "hf! must expand to an inline const block"
    );
    for (file, src) in SOURCES {
        if file.ends_with("hexf.rs") {
            continue;
        }
        for (n, line) in kernel_lines(src) {
            if line.contains("parse(") || (line.contains("hexf::") && !line.contains("hexf::hf")) {
                runtime.push(format!("{file}:{n}: {}", line.trim()));
            }
        }
    }
    assert!(
        runtime.is_empty(),
        "{} run-time calls of the hex-float reader (a const fn that asserts) in kernel code:\n{}",
        runtime.len(),
        runtime.join("\n")
    );
}

#[test]
fn kernel_maths_code_has_no_panicking_or_nondeterministic_constructs() {
    let forbidden = [
        "unsafe",
        ".unwrap()",
        ".expect(",
        "panic!",
        "unreachable!",
        "todo!",
        "unimplemented!",
        "HashMap",
        "HashSet",
        "rand::",
        "std::",
        "mul_add",
        ".powi(",
        ".powf(",
        ".sin(",
        ".cos(",
        ".exp(",
        ".ln(",
        ".sqrt(",
        "SystemTime",
        "Instant",
    ];
    let mut found = Vec::new();
    for (file, src) in SOURCES {
        for (n, line) in kernel_lines(src) {
            for f in forbidden {
                if line.contains(f) {
                    found.push(format!("{file}:{n}: `{f}` in {}", line.trim()));
                }
            }
            if !file.ends_with("hexf.rs")
                && (line.contains("assert!") || line.contains("assert_eq!"))
            {
                found.push(format!(
                    "{file}:{n}: assertion in kernel code: {}",
                    line.trim()
                ));
            }
            // Indexing with a run-time index, `name[i]`.
            let b = line.as_bytes();
            for w in b.windows(3) {
                if (w[0].is_ascii_alphanumeric() || w[0] == b'_' || w[0] == b')')
                    && w[1] == b'['
                    && (w[2].is_ascii_lowercase() || w[2] == b'_')
                {
                    found.push(format!("{file}:{n}: run-time indexing in {}", line.trim()));
                    break;
                }
            }
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn every_maths_module_names_its_published_method_with_a_link() {
    // Kernel rule: module docs name the published method followed, with a link.
    let mut missing = Vec::new();
    for (file, src) in SOURCES {
        let docs: String = src
            .lines()
            .take_while(|l| l.starts_with("//!") || l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if !docs.contains("https://") && !docs.contains("http://") {
            missing.push(file);
        }
    }
    assert!(
        missing.is_empty(),
        "module docs without a link to the method they follow: {missing:?}"
    );
}

#[test]
fn crate_docs_and_manifest_describe_the_ported_functions() {
    // The piece changed where the maths comes from. Before it, the crate root and the manifest
    // said every maths function comes from libm. The crate root now says the maths comes from
    // correctly rounded CORE-MATH ports and the pure-Rust `libm` crate, the `libm` functions
    // without a proven bound kept apart in `math::unproven`; and, with piece G02's contract, that
    // no floating-point value produced from finite inputs is NaN (`math` returns a `Real` or an
    // error; see `chapter_21_the_kernel_never_produces_nan`). The manifest says `libm` is the
    // source of the unproven maths functions, the fused multiply-add, the square root and the
    // exact rounding functions. This test keeps the old statements from coming back.
    let lib = include_str!("../src/lib.rs");
    let manifest = include_str!("../Cargo.toml");
    let mut stale = Vec::new();
    if lib.contains("which uses the pure-Rust `libm` crate") {
        stale.push("src/lib.rs: \"every maths function comes from [`math`], which uses the pure-Rust `libm` crate\"");
    }
    if lib.contains("No floating-point value produced here is ever NaN.") {
        stale.push("src/lib.rs: \"No floating-point value produced here is ever NaN.\"");
    }
    if manifest.contains("The only source of maths functions (sin, cos, atan2, exp and so on)") {
        stale.push("Cargo.toml: libm is \"The only source of maths functions (sin, cos, atan2, exp and so on)\"");
    }
    assert!(stale.is_empty(), "stale statements:\n{}", stale.join("\n"));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one family of checks, each spot listed beside its reason"
)]
fn chapter_16_interval_versions_of_the_proven_functions_exist() {
    // Chapter 16: "a build piece of section 21 adds proven-bound sinpi, cospi, exp and log with
    // interval versions"; piece G47: "interval versions with exact peak tests". Corrected from the
    // adversary's first version, which looked for the functions' names in src/interval.rs: they
    // are methods of `Interval` defined in src/math/enclose.rs (an inherent impl may live in any
    // module of its crate, and the fixer may not edit src/interval.rs). This version calls them,
    // and checks that they enclose the correctly rounded value at sampled points (an enclosure of
    // the exact value with double bounds encloses its rounding too), and the exact peaks.
    use geom_num::interval::Interval;
    let iv = |lo: f64, hi: f64| Interval::new(lo, hi).unwrap();
    let mut rng = Rng(0x5eed_0016);
    let mut bad = Vec::new();
    for _ in 0..scale(3000) {
        let lo = rng.sign() * rng.binade_in(-60, 60);
        let width = [0.0, 1e-9, 0.3, 1.0, 1.9, 2.5][rng.range(0, 5) as usize];
        let hi = lo + width * lo.abs().max(1.0) * rng.unit();
        let hi = if hi < lo { lo } else { hi };
        let x = iv(lo, hi);
        let s = x.sinpi().unwrap();
        let c = x.cospi().unwrap();
        let mut points = vec![lo, hi];
        for _ in 0..3 {
            let t = lo + (hi - lo) * rng.unit();
            if (lo..=hi).contains(&t) {
                points.push(t);
            }
        }
        for t in points {
            if !s.contains(oracle::sinpi(t)) {
                bad.push(format!(
                    "sinpi({t:e}) outside Interval::sinpi([{lo:e}, {hi:e}])"
                ));
            }
            if !c.contains(oracle::cospi(t)) {
                bad.push(format!(
                    "cospi({t:e}) outside Interval::cospi([{lo:e}, {hi:e}])"
                ));
            }
        }
        let el = rng.sign() * rng.binade_in(-40, 9);
        let eh = el + rng.unit() * 4.0;
        if let Ok(e) = iv(el, eh).exp() {
            for t in [el, eh, el.midpoint(eh)] {
                if !e.contains(oracle::exp(t)) {
                    bad.push(format!(
                        "exp({t:e}) outside Interval::exp([{el:e}, {eh:e}])"
                    ));
                }
            }
        }
        let ll = rng.binade_in(-1074, 1023);
        let lh = (ll * (1.0 + 4.0 * rng.unit())).min(f64::MAX);
        let l = iv(ll, lh).ln().unwrap();
        for t in [ll, lh] {
            if !l.contains(oracle::ln(t)) {
                bad.push(format!("ln({t:e}) outside Interval::ln([{ll:e}, {lh:e}])"));
            }
        }
        if bad.len() > 20 {
            break;
        }
    }
    // Exact peak tests: ±1 exactly when a peak is inside, at every scale, never otherwise.
    let big = 9_007_199_254_740_992.0; // 2^53: every double here is even
    let checks = [
        (
            "sinpi [0.25, 0.75] reaches 1",
            iv(0.25, 0.75).sinpi().unwrap().hi() == 1.0,
        ),
        (
            "sinpi [1.25, 1.75] reaches -1",
            iv(1.25, 1.75).sinpi().unwrap().lo() == -1.0,
        ),
        ("sinpi [0.6, 1.4] has no peak", {
            let v = iv(0.6, 1.4).sinpi().unwrap();
            v.hi() < 1.0 && v.lo() > -1.0
        }),
        (
            "cospi [-0.1, 0.1] reaches 1",
            iv(-0.1, 0.1).cospi().unwrap().hi() == 1.0,
        ),
        (
            "cospi [0.9, 1.1] reaches -1",
            iv(0.9, 1.1).cospi().unwrap().lo() == -1.0,
        ),
        ("sinpi [2^53, 2^53 + 2] is a whole turn", {
            let v = iv(big, big + 2.0).sinpi().unwrap();
            (v.lo(), v.hi()) == (-1.0, 1.0)
        }),
        (
            "sinpi at ½ is exactly 1",
            iv(0.5, 0.5).sinpi().unwrap() == iv(1.0, 1.0),
        ),
        (
            "cospi at 1 is exactly -1",
            iv(1.0, 1.0).cospi().unwrap() == iv(-1.0, -1.0),
        ),
        ("ln refuses [0, 1]", iv(0.0, 1.0).ln().is_err()),
        ("ln refuses [-1, 1]", iv(-1.0, 1.0).ln().is_err()),
        (
            "exp refuses an overflowing bound",
            iv(0.0, 710.0).exp().is_err(),
        ),
    ];
    for (what, ok) in checks {
        if !ok {
            bad.push(what.to_string());
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn principle_12_the_maths_functions_ship_with_a_benchmark() {
    // Principle 12: "Every operation ships with unit, property and fuzz tests and a benchmark";
    // risk R13 and piece G47: "within 2x of libm". Corrected from the adversary's first version,
    // which required a `[[bench]]` section in Cargo.toml: principle 12 asks for a benchmark, not a
    // manifest entry, and the fixer may change the manifest only to add dev-dependencies. The
    // benchmark is tests/math_bench.rs, an ignored release-mode test that times each function
    // against libm on the same arguments and fails above the 2x target.
    let bench = include_str!("math_bench.rs");
    let mut missing = Vec::new();
    for (ours, theirs) in [
        ("math::sinpi", "libm::sin"),
        ("math::cospi", "libm::cos"),
        ("math::exp", "libm::exp"),
        ("math::ln", "libm::log"),
        ("math::atan2", "libm::atan2"),
    ] {
        if !bench.contains(ours) || !bench.contains(theirs) {
            missing.push(format!("{ours} against {theirs}"));
        }
    }
    if !bench.contains("ratio > 2.0") {
        missing.push("the 2x target".to_string());
    }
    assert!(
        missing.is_empty(),
        "tests/math_bench.rs does not benchmark: {missing:?}"
    );
}
