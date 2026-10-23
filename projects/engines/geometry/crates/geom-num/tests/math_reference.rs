//! A rigorous reference for the correctly rounded maths functions: the test oracle.
//!
//! Never linked into the kernel. Every value is computed in interval arithmetic on exact dyadic
//! numbers `m · 2^e`, `m` a big integer (`num-bigint`). Each operation rounds its lower bound down
//! and its upper bound up to a working precision of `p` significant bits, and every series is
//! summed with an explicit bound on its remainder, so the true value always lies inside the
//! interval:
//!
//! - `sin`, `cos`: Taylor series; the Lagrange remainder is at most the first omitted term.
//! - `exp`: Taylor series for `|r| ≤ ½` after `x = k·ln 2 + r`; remainder at most twice the first
//!   omitted term (`e^|r| < 2`).
//! - `ln`: `ln f = 2·atanh((f − 1)/(f + 1))` for `f ∈ [0.70, 1.42)`; remainder at most the first
//!   omitted term over `1 − s²`, bounded by twice that term.
//! - `atan`: alternating Taylor series for `|t| ≤ 1/8` (remainder at most the first omitted term),
//!   after `atan t = atan c + atan((t − c)/(1 + tc))` with `c = j/8`. `atan(j/8)` comes from Euler's
//!   series `Σ 2^(2k)(k!)²/(2k+1)! · c^(2k+1)/(1 + c²)^(k+1)`, all of whose terms are positive with
//!   ratio at most ½, so its remainder is at most twice the first omitted term.
//! - `π = 16·atan(1/5) − 4·atan(1/239)` (Machin) and `ln 2 = 2·atanh(1/3)`, each to 4096 bits.
//!
//! The correctly rounded double is read off when both ends of the interval round to the same
//! double: rounding is monotone, so every real in between rounds to it too. Otherwise the
//! precision is doubled. The functions' values at non-trivial binary64 arguments are
//! transcendental (Lindemann–Weierstrass, and Niven's theorem for `sinpi` at dyadic arguments), so
//! the loop ends; the exact cases (`sinpi` at multiples of ½, `exp(0)`, `ln(1)`, zeros and
//! infinities) are answered exactly before any series.
//!
//! The test files include this module with `#[path = "math_reference.rs"] mod reference;`. Cargo
//! also builds it as a test target of its own, with no tests; the oracle's own checks are in
//! `math_reference_checks.rs`.
#![allow(
    dead_code,
    reason = "each test file that includes this module uses only part of it"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "values are named a, b, p and so on, as in the formulas they compute"
)]
// Test code: a malformed input or an undecidable rounding should stop the test loudly, as the
// crate's clippy.toml allows inside test functions; these helpers live outside them.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a malformed input or an undecidable rounding should stop the test loudly"
)]
// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); the oracle is
// test code, and may use them.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]

use core::cmp::Ordering;
use std::sync::OnceLock;

use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};

/// An exact dyadic number `m · 2^e`.
#[derive(Clone, Debug)]
pub(crate) struct Dy {
    pub(crate) m: BigInt,
    pub(crate) e: i64,
}

impl Dy {
    pub(crate) fn zero() -> Dy {
        Dy {
            m: BigInt::zero(),
            e: 0,
        }
    }

    pub(crate) fn int(i: i64) -> Dy {
        Dy {
            m: BigInt::from(i),
            e: 0,
        }
    }

    pub(crate) fn big(m: BigInt) -> Dy {
        Dy { m, e: 0 }
    }

    /// `2^k`.
    pub(crate) fn pow2(k: i64) -> Dy {
        Dy {
            m: BigInt::one(),
            e: k,
        }
    }

    /// The exact value of a finite double.
    pub(crate) fn from_f64(x: f64) -> Dy {
        assert!(x.is_finite(), "Dy::from_f64({x})");
        let bits = x.to_bits();
        let biased = ((bits >> 52) & 0x7ff) as i64;
        let frac = bits & ((1u64 << 52) - 1);
        let (m, e) = if biased == 0 {
            (frac, -1074)
        } else {
            (frac | (1u64 << 52), biased - 1075)
        };
        let m = BigInt::from(m);
        Dy {
            m: if bits >> 63 != 0 { -m } else { m },
            e,
        }
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.m.is_zero()
    }

    pub(crate) fn signum(&self) -> i32 {
        match self.m.sign() {
            Sign::Minus => -1,
            Sign::NoSign => 0,
            Sign::Plus => 1,
        }
    }

    pub(crate) fn neg(&self) -> Dy {
        Dy {
            m: -&self.m,
            e: self.e,
        }
    }

    pub(crate) fn abs(&self) -> Dy {
        Dy {
            m: self.m.abs(),
            e: self.e,
        }
    }

    /// `self · 2^k`, exactly.
    pub(crate) fn scale(&self, k: i64) -> Dy {
        Dy {
            m: self.m.clone(),
            e: self.e + k,
        }
    }

    pub(crate) fn add(&self, o: &Dy) -> Dy {
        if self.is_zero() {
            return o.clone();
        }
        if o.is_zero() {
            return self.clone();
        }
        let e = self.e.min(o.e);
        let a = &self.m << ((self.e - e) as usize);
        let b = &o.m << ((o.e - e) as usize);
        Dy { m: a + b, e }
    }

    pub(crate) fn sub(&self, o: &Dy) -> Dy {
        self.add(&o.neg())
    }

    pub(crate) fn mul(&self, o: &Dy) -> Dy {
        Dy {
            m: &self.m * &o.m,
            e: self.e + o.e,
        }
    }

    pub(crate) fn cmp(&self, o: &Dy) -> Ordering {
        match self.sub(o).signum() {
            -1 => Ordering::Less,
            0 => Ordering::Equal,
            _ => Ordering::Greater,
        }
    }

    pub(crate) fn eq(&self, o: &Dy) -> bool {
        self.cmp(o) == Ordering::Equal
    }

    /// `2^k ≤ |self| < 2^(k+1)`: returns `k`. Not for zero.
    pub(crate) fn exponent(&self) -> i64 {
        assert!(!self.is_zero());
        self.m.bits() as i64 - 1 + self.e
    }

    /// Rounded to at most `p` significant bits, towards +∞ if `up`, else towards −∞.
    pub(crate) fn round(&self, p: u64, up: bool) -> Dy {
        let bits = self.m.bits();
        if bits <= p {
            return self.clone();
        }
        let sh = bits - p;
        let d = BigInt::one() << (sh as usize);
        let m = if up {
            self.m.div_ceil(&d)
        } else {
            self.m.div_floor(&d)
        };
        Dy {
            m,
            e: self.e + sh as i64,
        }
    }

    /// `a / b` rounded to `p` significant bits, towards +∞ if `up`, else towards −∞. `b ≠ 0`.
    pub(crate) fn div(a: &Dy, b: &Dy, p: u64, up: bool) -> Dy {
        assert!(!b.is_zero(), "division by zero");
        if a.is_zero() {
            return Dy::zero();
        }
        let s = (p as i64 + 2 + b.m.bits() as i64 - a.m.bits() as i64).max(0);
        let num = &a.m << (s as usize);
        // Floor (or ceiling) of an integer quotient, then of a power-of-two quotient: the
        // composition of two floors of positive divisions is the floor of the whole quotient.
        let q = if up {
            num.div_ceil(&b.m)
        } else {
            num.div_floor(&b.m)
        };
        Dy {
            m: q,
            e: a.e - b.e - s,
        }
        .round(p, up)
    }

    /// The nearest integer (ties away from zero; only used to choose reduction points).
    pub(crate) fn round_to_int(&self) -> BigInt {
        if self.e >= 0 {
            return &self.m << (self.e as usize);
        }
        let half = Dy {
            m: BigInt::one(),
            e: -1,
        };
        let shifted = if self.signum() >= 0 {
            self.add(&half)
        } else {
            self.sub(&half)
        };
        let d = BigInt::one() << ((-shifted.e) as usize);
        if shifted.signum() >= 0 {
            shifted.m.div_floor(&d)
        } else {
            shifted.m.div_ceil(&d)
        }
    }

    /// The double nearest to this exact value, ties to even, with IEEE overflow to ±∞ and gradual
    /// underflow. A value that rounds to zero keeps its sign.
    pub(crate) fn to_f64(&self) -> f64 {
        if self.is_zero() {
            return 0.0;
        }
        let negative = self.signum() < 0;
        let mag = self.m.abs();
        let top = mag.bits() as i64 - 1 + self.e; // 2^top ≤ |value| < 2^(top+1)
        let q = (top - 52).max(-1074); // weight of the last kept bit
        let shift = q - self.e;
        let mut mant: BigInt;
        let mut q = q;
        if shift <= 0 {
            mant = &mag << ((-shift) as usize);
        } else {
            let d = BigInt::one() << (shift as usize);
            let (quot, rem) = mag.div_mod_floor(&d);
            mant = quot;
            let half = BigInt::one() << ((shift - 1) as usize);
            match rem.cmp(&half) {
                Ordering::Greater => mant += 1,
                Ordering::Equal if mant.is_odd() => mant += 1,
                _ => {}
            }
        }
        if mant.bits() > 53 {
            mant >>= 1usize;
            q += 1;
        }
        let mant = mant.to_u64().unwrap();
        let bits = if mant == 0 {
            0
        } else if mant < (1u64 << 52) {
            assert_eq!(q, -1074);
            mant
        } else {
            let biased = q + 52 + 1023;
            if biased >= 0x7ff {
                0x7ffu64 << 52
            } else {
                ((biased as u64) << 52) | (mant - (1u64 << 52))
            }
        };
        f64::from_bits(bits | if negative { 1u64 << 63 } else { 0 })
    }
}

/// A closed interval of dyadic numbers that contains the true value.
#[derive(Clone, Debug)]
pub(crate) struct Iv {
    pub(crate) lo: Dy,
    pub(crate) hi: Dy,
}

fn min_dy(a: Dy, b: Dy) -> Dy {
    if a.cmp(&b) == Ordering::Greater { b } else { a }
}

fn max_dy(a: Dy, b: Dy) -> Dy {
    if a.cmp(&b) == Ordering::Less { b } else { a }
}

impl Iv {
    pub(crate) fn point(d: Dy) -> Iv {
        Iv {
            lo: d.clone(),
            hi: d,
        }
    }

    pub(crate) fn int(i: i64) -> Iv {
        Iv::point(Dy::int(i))
    }

    pub(crate) fn new(lo: Dy, hi: Dy) -> Iv {
        assert!(lo.cmp(&hi) != Ordering::Greater, "empty interval");
        Iv { lo, hi }
    }

    pub(crate) fn neg(&self) -> Iv {
        Iv {
            lo: self.hi.neg(),
            hi: self.lo.neg(),
        }
    }

    pub(crate) fn add(&self, o: &Iv, p: u64) -> Iv {
        Iv {
            lo: self.lo.add(&o.lo).round(p, false),
            hi: self.hi.add(&o.hi).round(p, true),
        }
    }

    pub(crate) fn sub(&self, o: &Iv, p: u64) -> Iv {
        self.add(&o.neg(), p)
    }

    pub(crate) fn mul(&self, o: &Iv, p: u64) -> Iv {
        let c = [
            self.lo.mul(&o.lo),
            self.lo.mul(&o.hi),
            self.hi.mul(&o.lo),
            self.hi.mul(&o.hi),
        ];
        let lo = c.iter().cloned().reduce(min_dy).unwrap();
        let hi = c.iter().cloned().reduce(max_dy).unwrap();
        Iv {
            lo: lo.round(p, false),
            hi: hi.round(p, true),
        }
    }

    pub(crate) fn div(&self, o: &Iv, p: u64) -> Iv {
        assert!(
            o.lo.signum() > 0 || o.hi.signum() < 0,
            "divisor interval contains zero"
        );
        let pairs = [
            (&self.lo, &o.lo),
            (&self.lo, &o.hi),
            (&self.hi, &o.lo),
            (&self.hi, &o.hi),
        ];
        let lo = pairs
            .iter()
            .map(|(a, b)| Dy::div(a, b, p, false))
            .reduce(min_dy)
            .unwrap();
        let hi = pairs
            .iter()
            .map(|(a, b)| Dy::div(a, b, p, true))
            .reduce(max_dy)
            .unwrap();
        Iv { lo, hi }
    }

    pub(crate) fn div_int(&self, k: i64, p: u64) -> Iv {
        self.div(&Iv::int(k), p)
    }

    pub(crate) fn mul_int(&self, k: i64, p: u64) -> Iv {
        self.mul(&Iv::int(k), p)
    }

    /// `self · 2^k`, exactly.
    pub(crate) fn scale(&self, k: i64) -> Iv {
        Iv {
            lo: self.lo.scale(k),
            hi: self.hi.scale(k),
        }
    }

    /// The largest magnitude in the interval.
    pub(crate) fn mag(&self) -> Dy {
        max_dy(self.lo.abs(), self.hi.abs())
    }

    /// The interval widened by `r ≥ 0` on both sides.
    pub(crate) fn widen(&self, r: &Dy, p: u64) -> Iv {
        Iv {
            lo: self.lo.sub(r).round(p, false),
            hi: self.hi.add(r).round(p, true),
        }
    }

    /// The common correctly rounded double of every real in the interval, if there is one.
    pub(crate) fn rounded(&self) -> Option<f64> {
        let a = self.lo.to_f64();
        let b = self.hi.to_f64();
        (a.to_bits() == b.to_bits()).then_some(a)
    }
}

/// The correctly rounded value of the real that `f(p)` encloses for every precision `p`.
pub(crate) fn correctly_rounded(f: impl Fn(u64) -> Iv) -> f64 {
    let mut p = 192;
    loop {
        if let Some(v) = f(p).rounded() {
            return v;
        }
        p *= 2;
        assert!(p <= 2048, "the reference could not decide the rounding");
    }
}

/// `sin(r)` for `|r| ≤ 1`: Taylor series, remainder at most the first omitted term.
pub(crate) fn sin_iv(r: &Iv, p: u64) -> Iv {
    let r2 = r.mul(r, p);
    let threshold = r.mag().scale(-(p as i64) - 8);
    let mut term = r.clone();
    let mut sum = r.clone();
    let mut k: i64 = 1;
    loop {
        term = term.mul(&r2, p).div_int(-(2 * k) * (2 * k + 1), p);
        k += 1;
        let t = term.mag();
        if t.cmp(&threshold) != Ordering::Greater {
            return sum.widen(&t, p);
        }
        sum = sum.add(&term, p);
    }
}

/// `cos(r)` for `|r| ≤ 1`: Taylor series, remainder at most the first omitted term.
pub(crate) fn cos_iv(r: &Iv, p: u64) -> Iv {
    let r2 = r.mul(r, p);
    let threshold = Dy::pow2(-(p as i64) - 8);
    let mut term = Iv::int(1);
    let mut sum = Iv::int(1);
    let mut k: i64 = 1;
    loop {
        term = term.mul(&r2, p).div_int(-(2 * k - 1) * (2 * k), p);
        k += 1;
        let t = term.mag();
        if t.cmp(&threshold) != Ordering::Greater {
            return sum.widen(&t, p);
        }
        sum = sum.add(&term, p);
    }
}

/// `exp(r)` for `|r| ≤ ½`: Taylor series, remainder at most twice the first omitted term.
pub(crate) fn exp_small_iv(r: &Iv, p: u64) -> Iv {
    assert_ne!(r.mag().cmp(&Dy::pow2(-1)), Ordering::Greater);
    let threshold = Dy::pow2(-(p as i64) - 8);
    let mut term = Iv::int(1);
    let mut sum = Iv::int(1);
    let mut n: i64 = 1;
    loop {
        term = term.mul(r, p).div_int(n, p);
        n += 1;
        let t = term.mag();
        if t.cmp(&threshold) != Ordering::Greater {
            return sum.widen(&t.scale(1), p);
        }
        sum = sum.add(&term, p);
    }
}

/// `atanh(s) = Σ s^(2k+1)/(2k+1)` for `|s| ≤ ½`: remainder at most the first omitted term over
/// `1 − s²`, bounded by twice that term.
pub(crate) fn atanh_iv(s: &Iv, p: u64) -> Iv {
    assert_ne!(s.mag().cmp(&Dy::pow2(-1)), Ordering::Greater);
    let s2 = s.mul(s, p);
    let threshold = s.mag().scale(-(p as i64) - 8);
    let mut pw = s.clone();
    let mut sum = s.clone();
    let mut k: i64 = 1;
    loop {
        pw = pw.mul(&s2, p);
        let term = pw.div_int(2 * k + 1, p);
        k += 1;
        let t = term.mag();
        if t.cmp(&threshold) != Ordering::Greater {
            return sum.widen(&t.scale(1), p);
        }
        sum = sum.add(&term, p);
    }
}

/// `atan(t)` for `|t| ≤ 1/8`: alternating Taylor series with decreasing terms, remainder at most
/// the first omitted term.
pub(crate) fn atan_small_iv(t: &Iv, p: u64) -> Iv {
    assert_ne!(t.mag().cmp(&Dy::pow2(-3)), Ordering::Greater);
    let t2 = t.mul(t, p);
    let threshold = t.mag().scale(-(p as i64) - 8);
    let mut pw = t.clone();
    let mut sum = t.clone();
    let mut k: i64 = 1;
    loop {
        pw = pw.mul(&t2, p).neg();
        let term = pw.div_int(2 * k + 1, p);
        k += 1;
        let t = term.mag();
        if t.cmp(&threshold) != Ordering::Greater {
            return sum.widen(&t, p);
        }
        sum = sum.add(&term, p);
    }
}

/// Precision of the cached constants.
pub(crate) const CONST_BITS: u64 = 4096;

/// `atan(1/n)` for an integer `n ≥ 2`: alternating series, remainder at most the first omitted
/// term.
fn atan_inv_int(n: i64, p: u64) -> Iv {
    let n2 = Iv::int(n * n);
    let threshold = Dy::pow2(-(p as i64) - 8);
    let mut pw = Iv::int(1).div_int(n, p); // (−1)^k / n^(2k+1)
    let mut sum = pw.clone();
    let mut k: i64 = 1;
    loop {
        pw = pw.div(&n2, p).neg();
        let term = pw.div_int(2 * k + 1, p);
        k += 1;
        let t = term.mag();
        if t.cmp(&threshold) != Ordering::Greater {
            return sum.widen(&t, p);
        }
        sum = sum.add(&term, p);
    }
}

/// π to [`CONST_BITS`] bits, by Machin's formula.
pub(crate) fn pi() -> &'static Iv {
    static PI: OnceLock<Iv> = OnceLock::new();
    PI.get_or_init(|| {
        let p = CONST_BITS + 32;
        let a = atan_inv_int(5, p).mul_int(16, p);
        let b = atan_inv_int(239, p).mul_int(4, p);
        a.sub(&b, p)
    })
}

/// ln 2 to [`CONST_BITS`] bits, as `2·atanh(1/3)`.
pub(crate) fn ln2() -> &'static Iv {
    static LN2: OnceLock<Iv> = OnceLock::new();
    LN2.get_or_init(|| {
        let p = CONST_BITS + 32;
        atanh_iv(&Iv::int(1).div_int(3, p), p).scale(1)
    })
}

/// `atan(j/8)` for `1 ≤ j ≤ 8`, by Euler's series.
pub(crate) fn atan_eighths(j: i64) -> &'static Iv {
    static T: OnceLock<Vec<Iv>> = OnceLock::new();
    let table = T.get_or_init(|| {
        let p = CONST_BITS + 32;
        (1..=8)
            .map(|j| {
                // term_0 = c/(1 + c²) = 8j/(64 + j²); term_{k+1} = term_k · (2k+2)/(2k+3) · j²/(64 + j²)
                let den = 64 + j * j;
                let ratio = Iv::int(j * j).div_int(den, p);
                let threshold = Dy::pow2(-(p as i64) - 8);
                let mut term = Iv::int(8 * j).div_int(den, p);
                let mut sum = term.clone();
                let mut k: i64 = 0;
                loop {
                    term = term
                        .mul(&ratio, p)
                        .mul_int(2 * k + 2, p)
                        .div_int(2 * k + 3, p);
                    k += 1;
                    let t = term.mag();
                    if t.cmp(&threshold) != Ordering::Greater {
                        break sum.widen(&t.scale(1), p);
                    }
                    sum = sum.add(&term, p);
                }
            })
            .collect()
    });
    &table[(j - 1) as usize]
}

/// `atan(t)` for `0 ≤ t ≤ 1`.
pub(crate) fn atan_unit_iv(t: &Iv, p: u64) -> Iv {
    if t.mag().cmp(&Dy::pow2(-3)) != Ordering::Greater {
        return atan_small_iv(t, p);
    }
    let j = t.hi.scale(3).round_to_int().to_i64().unwrap().clamp(1, 8);
    let c = Iv::point(Dy {
        m: BigInt::from(j),
        e: -3,
    });
    let u = t.sub(&c, p).div(&Iv::int(1).add(&t.mul(&c, p), p), p);
    atan_eighths(j).add(&atan_small_iv(&u, p), p)
}

/// A finite double reduced exactly modulo 2: `r = x − 2n ∈ [−1, 1)`.
pub(crate) fn reduce_mod2(x: &Dy) -> Dy {
    if x.e >= 1 {
        return Dy::zero();
    }
    // n = ⌊x/2 + ½⌋
    let num = x.add(&Dy::pow2(0)); // x + 1
    let d = BigInt::one() << ((1 - num.e) as usize); // 2^(1−e): num / d = (x + 1)/2
    let n = num.m.div_floor(&d);
    x.sub(&Dy::big(n).scale(1))
}

/// `sin(π·r)` for a dyadic `r ∈ (−1, 1)` that is not a multiple of ½.
fn sin_pi_iv(r: &Dy, p: u64) -> Iv {
    let negative = r.signum() < 0;
    let mut a = r.abs();
    let half = Dy::pow2(-1);
    if a.cmp(&half) == Ordering::Greater {
        a = Dy::int(1).sub(&a); // sin(π(1 − a)) = sin(πa)
    }
    let v = if a.cmp(&Dy::pow2(-2)) == Ordering::Greater {
        cos_iv(&pi().mul(&Iv::point(half.sub(&a)), p), p) // sin(πa) = cos(π(½ − a))
    } else {
        sin_iv(&pi().mul(&Iv::point(a), p), p)
    };
    if negative { v.neg() } else { v }
}

/// Correctly rounded `sin(πx)`, with IEEE 754-2019's exact values and signed zeros.
pub(crate) fn sinpi(x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    if x == 0.0 {
        return x;
    }
    let r = reduce_mod2(&Dy::from_f64(x));
    if r.is_zero() || r.eq(&Dy::int(-1)) {
        return 0.0f64.copysign(x);
    }
    if r.eq(&Dy::pow2(-1)) {
        return 1.0;
    }
    if r.eq(&Dy::pow2(-1).neg()) {
        return -1.0;
    }
    correctly_rounded(|p| sin_pi_iv(&r, p))
}

/// Correctly rounded `cos(πx) = sin(π(x + ½))`, with IEEE 754-2019's exact values.
pub(crate) fn cospi(x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    let r = reduce_mod2(&Dy::from_f64(x).add(&Dy::pow2(-1)));
    if r.is_zero() || r.eq(&Dy::int(-1)) {
        return 0.0;
    }
    if r.eq(&Dy::pow2(-1)) {
        return 1.0;
    }
    if r.eq(&Dy::pow2(-1).neg()) {
        return -1.0;
    }
    correctly_rounded(|p| sin_pi_iv(&r, p))
}

/// `exp(x)` for an exact dyadic `x`, as `2^k · exp(x − k·ln 2)`.
fn exp_dy(x: &Dy, p: u64) -> Iv {
    let k = Dy::div(x, &ln2().lo, 64, false).round_to_int();
    let pr = p + 128;
    let r = Iv::point(x.clone()).sub(&ln2().mul(&Iv::point(Dy::big(k.clone())), pr), pr);
    let k = k.to_i64().unwrap();
    exp_small_iv(&r, p + 16).scale(k)
}

/// Correctly rounded `e^x`, with IEEE overflow and underflow.
pub(crate) fn exp(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x == f64::INFINITY {
        return x;
    }
    if x == f64::NEG_INFINITY {
        return 0.0;
    }
    if x == 0.0 {
        return 1.0;
    }
    if x > 1000.0 {
        return f64::INFINITY;
    }
    if x < -1000.0 {
        return 0.0;
    }
    let d = Dy::from_f64(x);
    correctly_rounded(|p| exp_dy(&d, p))
}

/// Correctly rounded natural logarithm, with IEEE's special values.
pub(crate) fn ln(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x == f64::INFINITY {
        return x;
    }
    if x == 1.0 {
        return 0.0;
    }
    let d = Dy::from_f64(x);
    let mut big_e = d.exponent();
    let mut f = d.scale(-big_e); // 1 ≤ f < 2
    if f.cmp(&Dy {
        m: BigInt::from(181),
        e: -7,
    }) == Ordering::Greater
    {
        f = f.scale(-1); // f ∈ (0.70, 1)
        big_e += 1;
    }
    correctly_rounded(|p| {
        let pp = p + 32;
        let s = Iv::point(f.sub(&Dy::int(1))).div(&Iv::point(f.add(&Dy::int(1))), pp);
        let lf = atanh_iv(&s, pp).scale(1);
        ln2().mul_int(big_e, pp).add(&lf, p)
    })
}

/// `k·π/4`, correctly rounded, from the oracle's own π.
fn quarter_pi_multiple(k: i64) -> f64 {
    correctly_rounded(|p| pi().mul_int(k, p).scale(-2))
}

/// Correctly rounded `atan2(y, x)`, with C99 Annex F's special values.
pub(crate) fn atan2(y: f64, x: f64) -> f64 {
    if y.is_nan() || x.is_nan() {
        return f64::NAN;
    }
    if y == 0.0 {
        return if x.is_sign_positive() {
            y
        } else {
            quarter_pi_multiple(4).copysign(y)
        };
    }
    if x == 0.0 {
        return quarter_pi_multiple(2).copysign(y);
    }
    if y.is_infinite() {
        let k = if x.is_infinite() {
            if x > 0.0 { 1 } else { 3 }
        } else {
            2
        };
        return quarter_pi_multiple(k).copysign(y);
    }
    if x.is_infinite() {
        return if x > 0.0 {
            0.0f64.copysign(y)
        } else {
            quarter_pi_multiple(4).copysign(y)
        };
    }
    let (ay, ax) = (Dy::from_f64(y.abs()), Dy::from_f64(x.abs()));
    let inv = ay.cmp(&ax) == Ordering::Greater;
    if x > 0.0 && !inv {
        // If q = |y/x| is exactly an odd multiple of 2^−1075 (a midpoint between subnormals),
        // the interval method cannot separate atan(q) from q at any practical precision. But
        // q − q³/3 < atan(q) < q, and q³ is far smaller than 2^−1075, so atan(q) rounds to the
        // subnormal just below q.
        let shift = ay.e - ax.e + 1075;
        let (num, den) = if shift >= 0 {
            (&ay.m << (shift as usize), ax.m.clone())
        } else {
            (ay.m.clone(), &ax.m << ((-shift) as usize))
        };
        let (k, rem) = num.div_mod_floor(&den);
        if rem.is_zero() && k.is_odd() {
            let below = Dy {
                m: (k - 1) / 2,
                e: -1074,
            }
            .to_f64();
            return below.copysign(y);
        }
    }
    correctly_rounded(|p| {
        let pp = p + 16;
        let t = if inv {
            Iv::point(ax.clone()).div(&Iv::point(ay.clone()), pp)
        } else {
            Iv::point(ay.clone()).div(&Iv::point(ax.clone()), pp)
        };
        let mut a = atan_unit_iv(&t, pp);
        if inv {
            a = pi().scale(-1).sub(&a, pp);
        }
        if x < 0.0 {
            a = pi().sub(&a, pp);
        }
        if y < 0.0 { a.neg() } else { a }
    })
}

/// `x − k·π/2` and `k mod 4`, for a finite double `x`.
fn reduce_half_pi(x: &Dy, p: u64) -> (Iv, i64) {
    let half_pi = pi().scale(-1);
    let k = Dy::div(x, &half_pi.lo, x.exponent().max(0) as u64 + 64, false).round_to_int();
    let pr = p + 96 + x.exponent().max(0) as u64;
    let r = Iv::point(x.clone()).sub(&half_pi.mul(&Iv::point(Dy::big(k.clone())), pr), pr);
    let q = k.mod_floor(&BigInt::from(4)).to_i64().unwrap();
    (r, q)
}

/// Correctly rounded `sin(x)`, `x` in radians.
pub(crate) fn sin(x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    if x == 0.0 {
        return x;
    }
    let d = Dy::from_f64(x);
    correctly_rounded(|p| {
        let (r, q) = reduce_half_pi(&d, p);
        match q {
            0 => sin_iv(&r, p),
            1 => cos_iv(&r, p),
            2 => sin_iv(&r, p).neg(),
            _ => cos_iv(&r, p).neg(),
        }
    })
}

/// Correctly rounded `cos(x)`, `x` in radians.
pub(crate) fn cos(x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    if x == 0.0 {
        return 1.0;
    }
    let d = Dy::from_f64(x);
    correctly_rounded(|p| {
        let (r, q) = reduce_half_pi(&d, p);
        match q {
            0 => cos_iv(&r, p),
            1 => sin_iv(&r, p).neg(),
            2 => cos_iv(&r, p).neg(),
            _ => sin_iv(&r, p),
        }
    })
}

/// The exact value of a C99 hexadecimal float literal (panics if malformed); a missing binary
/// exponent means `p0`, as `strtod` reads it.
pub(crate) fn hex_value(s: &str) -> (Dy, bool) {
    let s = s.trim();
    let (negative, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let body = body
        .strip_prefix("0x")
        .or_else(|| body.strip_prefix("0X"))
        .expect("0x prefix");
    let (digits, exp) = body.split_once(['p', 'P']).unwrap_or((body, "0"));
    let exp: i64 = exp.parse().expect("exponent");
    let (int_part, frac_part) = digits.split_once('.').unwrap_or((digits, ""));
    let all = format!("{int_part}{frac_part}");
    let m = BigInt::parse_bytes(all.as_bytes(), 16).expect("hex digits");
    (
        Dy {
            m: if negative { -m } else { m },
            e: exp - 4 * frac_part.len() as i64,
        },
        negative,
    )
}

/// A C99 hexadecimal float literal, exactly (panics if malformed or inexact). For test data.
pub(crate) fn hx(s: &str) -> f64 {
    let (d, negative) = hex_value(s);
    let v = d.to_f64();
    assert!(Dy::from_f64(v).eq(&d), "{s} is not exactly a double");
    if d.is_zero() && negative { -0.0 } else { v }
}

/// A C99 hexadecimal float literal rounded to the nearest double, as C's `strtod` reads it.
pub(crate) fn hx_rounded(s: &str) -> f64 {
    let (d, negative) = hex_value(s);
    let v = d.to_f64();
    if d.is_zero() && negative { -0.0 } else { v }
}

/// A deterministic pseudo-random generator (`SplitMix64`), so every test run sees the same inputs.
pub(crate) struct Rng(pub(crate) u64);

impl Rng {
    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A uniformly random finite double over all bit patterns (every binade equally likely).
    pub(crate) fn any_finite(&mut self) -> f64 {
        loop {
            let v = f64::from_bits(self.next_u64());
            if v.is_finite() {
                return v;
            }
        }
    }

    /// A double uniform in `[lo, hi)`.
    pub(crate) fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        let u = (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
        lo + (hi - lo) * u
    }

    /// A double with random significand and an exponent uniform in `[emin, emax]`, random sign.
    pub(crate) fn log_uniform(&mut self, emin: i32, emax: i32) -> f64 {
        let span = (emax - emin + 1) as u64;
        let e = emin + (self.next_u64() % span) as i32;
        let frac = self.next_u64() & ((1u64 << 52) - 1);
        let sign = self.next_u64() & (1u64 << 63);
        f64::from_bits(sign | (((e + 1023) as u64) << 52) | frac)
    }
}

/// Distance in ulps between two doubles of the same sign (for sanity checks only).
pub(crate) fn ulps(a: f64, b: f64) -> u64 {
    let (a, b) = (a.to_bits() as i64, b.to_bits() as i64);
    (a - b).unsigned_abs()
}

/// Asserts bit equality, treating every NaN as equal to every NaN.
#[track_caller]
pub(crate) fn assert_same(got: f64, want: f64, what: &str) {
    if want.is_nan() {
        assert!(
            got.is_nan(),
            "{what}: got {got:e} ({:#018x}), want NaN",
            got.to_bits()
        );
    } else {
        assert!(
            got.to_bits() == want.to_bits(),
            "{what}: got {got:e} ({:#018x}), want {want:e} ({:#018x})",
            got.to_bits(),
            want.to_bits()
        );
    }
}

/// What a public function of `geom_num::math` owes for arguments `args` whose IEEE 754 value
/// (correctly rounded, with C99 Annex F's special values) is `want`, under the functions' contract
/// (the module documentation of `geom_num::math`): an argument that is not finite is refused as
/// `NotFinite`, checked first; then a value that is not a real number (`want` is NaN) is refused as
/// `OutsideDomain`, and an infinite one (an overflow or a pole) as `NotFinite`; otherwise the
/// value itself, bit for bit.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Owed {
    /// A finite value, bit for bit.
    Value(f64),
    /// `NumError::NotFinite`.
    NotFinite,
    /// `NumError::OutsideDomain`.
    OutsideDomain,
}

/// What a public maths function owes for `args` whose IEEE 754 value is `want` (see [`Owed`]).
pub(crate) fn owed(args: &[f64], want: f64) -> Owed {
    if args.iter().any(|a| !a.is_finite()) {
        Owed::NotFinite
    } else if want.is_nan() {
        Owed::OutsideDomain
    } else if want.is_infinite() {
        Owed::NotFinite
    } else {
        Owed::Value(want)
    }
}

/// Asserts that a public maths function's outcome is what it owes: the same error, or the same
/// value bit for bit.
#[track_caller]
pub(crate) fn assert_owed(got: Owed, want: Owed, what: &str) {
    match (got, want) {
        (Owed::Value(g), Owed::Value(w)) => assert_same(g, w, what),
        (Owed::NotFinite, Owed::NotFinite) | (Owed::OutsideDomain, Owed::OutsideDomain) => {}
        _ => panic!("{what}: got {got:?}, want {want:?}"),
    }
}

/// The largest of two non-negative dyadics.
pub(crate) fn dmax(a: Dy, b: Dy) -> Dy {
    max_dy(a, b)
}

/// An upper bound for `|v − (h + l)|` over every real `v` in the interval.
pub(crate) fn dd_error(h: f64, l: f64, v: &Iv) -> Dy {
    let s = Dy::from_f64(h).add(&Dy::from_f64(l));
    max_dy(s.sub(&v.lo).abs(), s.sub(&v.hi).abs())
}

/// `sin(a)` and `cos(a)` for any interval `a` of moderate size, by reduction modulo π/2.
pub(crate) fn sin_cos_iv(a: &Iv, p: u64) -> (Iv, Iv) {
    let half_pi = pi().scale(-1);
    let k = Dy::div(&a.hi, &half_pi.lo, 64, false).round_to_int();
    let r = a.sub(&half_pi.mul(&Iv::point(Dy::big(k.clone())), p + 32), p + 32);
    let (s, c) = (sin_iv(&r, p), cos_iv(&r, p));
    match k.mod_floor(&BigInt::from(4)).to_i64().unwrap() {
        0 => (s, c),
        1 => (c, s.neg()),
        2 => (s.neg(), c.neg()),
        _ => (c.neg(), s),
    }
}

/// `n!` as an exact integer.
pub(crate) fn factorial(n: u64) -> BigInt {
    (1..=n).fold(BigInt::one(), |a, k| a * BigInt::from(k))
}

/// An upper bound on `max |Σ t_j·x^j − Σ c_j·x^j|` over `|x| ≤ x_max`, by the triangle
/// inequality: `Σ |t_j − c_j|·x_max^j`, with `target[j]` an interval for the target's
/// coefficient of degree `j` and `coeffs` the polynomial's `(degree, value)` pairs. The
/// target's own remainder beyond the listed degrees is not included; add it separately.
pub(crate) fn coefficient_error_bound(target: &[Iv], coeffs: &[(usize, f64)], x_max: &Dy) -> Dy {
    let mut c = vec![Dy::zero(); target.len()];
    for &(j, v) in coeffs {
        assert!(
            j < target.len(),
            "coefficient degree beyond the target's list"
        );
        c[j] = Dy::from_f64(v);
    }
    let mut total = Dy::zero();
    let mut pw = Dy::int(1);
    for (j, t) in target.iter().enumerate() {
        let d = max_dy(t.lo.sub(&c[j]).abs(), t.hi.sub(&c[j]).abs());
        total = total.add(&d.mul(&pw));
        let _ = j;
        pw = pw.mul(x_max);
    }
    total
}

/// `2^k` as a dyadic, for writing bounds.
pub(crate) fn p2(k: i64) -> Dy {
    Dy::pow2(k)
}

/// A dyadic upper bound for `log2(d)`, for messages (approximate).
pub(crate) fn log2_approx(d: &Dy) -> f64 {
    if d.is_zero() {
        return f64::NEG_INFINITY;
    }
    let e = d.exponent();
    let m = d.scale(-e).to_f64();
    e as f64 + m.log2()
}

/// An upper bound on `max |Σ q_j·y^j|` over `0 ≤ y ≤ y_max`: the interval `[0, y_max]` is cut
/// into `pieces` equal parts and the polynomial is evaluated on each by Horner's rule in interval
/// arithmetic, which encloses its range there. Rigorous; the bound tightens as `pieces` grows.
pub(crate) fn poly_sup_bound(q: &[Iv], y_max: &Dy, pieces: u64, p: u64) -> Dy {
    let mut best = Dy::zero();
    let n = Dy::int(pieces as i64);
    for i in 0..pieces {
        let lo = Dy::div(&y_max.mul(&Dy::int(i as i64)), &n, p, false);
        let hi = Dy::div(&y_max.mul(&Dy::int(i as i64 + 1)), &n, p, true);
        let y = Iv::new(lo, hi);
        let mut acc = Iv::int(0);
        for c in q.iter().rev() {
            acc = acc.mul(&y, p).add(c, p);
        }
        best = max_dy(best, acc.mag());
    }
    best
}

/// The coefficients of `Σ (t_j − c_j)·x^j` for `j ≥ shift`, divided by `x^shift`, as a polynomial
/// in `y = x^step` (`step` = 2 for odd or even series): the polynomial whose supremum bounds the
/// approximation error relative to `|x|^shift`.
pub(crate) fn error_poly(
    target: &[Iv],
    coeffs: &[(usize, f64)],
    shift: usize,
    step: usize,
) -> Vec<Iv> {
    let mut d: Vec<Iv> = target.to_vec();
    for &(j, v) in coeffs {
        d[j] = d[j].sub(&Iv::point(Dy::from_f64(v)), 4096);
    }
    (shift..d.len())
        .step_by(step)
        .map(|j| d[j].clone())
        .collect()
}
