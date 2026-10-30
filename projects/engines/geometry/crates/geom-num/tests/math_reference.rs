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
    reason = "tests may use native arithmetic, casts and std reductions"
)]

use core::cmp::Ordering;

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

/// π to [`CONST_BITS`] bits, by Machin's formula: the series [`pi`]'s stored value comes from
/// (`tests/math_constants.rs` checks the two agree).
pub(crate) fn pi_series() -> Iv {
    let p = CONST_BITS + 32;
    let a = atan_inv_int(5, p).mul_int(16, p);
    let b = atan_inv_int(239, p).mul_int(4, p);
    a.sub(&b, p)
}

/// ln 2 to [`CONST_BITS`] bits, as `2·atanh(1/3)`: the series [`ln2`]'s stored value comes from.
pub(crate) fn ln2_series() -> Iv {
    let p = CONST_BITS + 32;
    atanh_iv(&Iv::int(1).div_int(3, p), p).scale(1)
}

/// `atan(j/8)` for `1 ≤ j ≤ 8`, by Euler's series: the series [`atan_eighths`]'s stored values
/// come from.
pub(crate) fn atan_eighths_series() -> Vec<Iv> {
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
}

/// A stored constant: its interval's two ends, each a positive significand in hexadecimal and its
/// binary exponent, exactly as the series above computed them. Stored, not cached: a crate that
/// ships holds no `static`, even in a test build (the merge gate, G12), and a constant table is
/// a `const`.
struct Stored {
    lo: (&'static str, i64),
    hi: (&'static str, i64),
}

/// The interval a stored constant holds.
fn from_stored(c: &Stored) -> Iv {
    let end = |(hex, e): (&str, i64)| Dy {
        m: BigInt::parse_bytes(hex.as_bytes(), 16).expect("stored hexadecimal"),
        e,
    };
    Iv {
        lo: end(c.lo),
        hi: end(c.hi),
    }
}

/// π to [`CONST_BITS`] bits ([`pi_series`]).
const PI_STORED: Stored = Stored {
    lo: (
        concat!(
            "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b139b22514a08798e3404dd",
            "ef9519b3cd3a431b302b0a6df25f14374fe1356d6d51c245e485b576625e7ec6f44c42e9a637ed6b",
            "0bff5cb6f406b7edee386bfb5a899fa5ae9f24117c4b1fe649286651ece45b3dc2007cb8a163bf05",
            "98da48361c55d39a69163fa8fd24cf5f83655d23dca3ad961c62f356208552bb9ed529077096966d",
            "670c354e4abc9804f1746c08ca18217c32905e462e36ce3be39e772c180e86039b2783a2ec07a28f",
            "b5c55df06f4c52c9de2bcbf6955817183995497cea956ae515d2261898fa051015728e5a8aaac42d",
            "ad33170d04507a33a85521abdf1cba64ecfb850458dbef0a8aea71575d060c7db3970f85a6e1e4c7",
            "abf5ae8cdb0933d71e8c94e04a25619dcee3d2261ad2ee6bf12ffa06d98a0864d87602733ec86a64",
            "521f2b18177b200cbbe117577a615d6c770988c0bad946e208e24fa074e5ab3143db5bfce0fd108e",
            "4b82d120a92108011a723c12a787e6d788719a10bdba5b2699c327186af4e23c1a946834b6150bda",
            "2583e9ca2ad44ce8dbbbc2db04de8ef92e8efc141fbecaa6287c59474e6bc05d99b2964fa090c3a2",
            "233ba186515be7ed1f612970cee2d7afb81bdd762170481cd0069127d5b05aa993b4ea988d8fddc1",
            "86ffb7dc90a6c08f4df435c93402849236c3fab4d27c7026c1d4dcb2602646dec9751cb4",
        ),
        -4126,
    ),
    hi: (
        concat!(
            "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b139b22514a08798e3404dd",
            "ef9519b3cd3a431b302b0a6df25f14374fe1356d6d51c245e485b576625e7ec6f44c42e9a637ed6b",
            "0bff5cb6f406b7edee386bfb5a899fa5ae9f24117c4b1fe649286651ece45b3dc2007cb8a163bf05",
            "98da48361c55d39a69163fa8fd24cf5f83655d23dca3ad961c62f356208552bb9ed529077096966d",
            "670c354e4abc9804f1746c08ca18217c32905e462e36ce3be39e772c180e86039b2783a2ec07a28f",
            "b5c55df06f4c52c9de2bcbf6955817183995497cea956ae515d2261898fa051015728e5a8aaac42d",
            "ad33170d04507a33a85521abdf1cba64ecfb850458dbef0a8aea71575d060c7db3970f85a6e1e4c7",
            "abf5ae8cdb0933d71e8c94e04a25619dcee3d2261ad2ee6bf12ffa06d98a0864d87602733ec86a64",
            "521f2b18177b200cbbe117577a615d6c770988c0bad946e208e24fa074e5ab3143db5bfce0fd108e",
            "4b82d120a92108011a723c12a787e6d788719a10bdba5b2699c327186af4e23c1a946834b6150bda",
            "2583e9ca2ad44ce8dbbbc2db04de8ef92e8efc141fbecaa6287c59474e6bc05d99b2964fa090c3a2",
            "233ba186515be7ed1f612970cee2d7afb81bdd762170481cd0069127d5b05aa993b4ea988d8fddc1",
            "86ffb7dc90a6c08f4df435c93402849236c3fab4d27c7026c1d4dcb2602646dec9752031",
        ),
        -4126,
    ),
};

/// ln 2 to [`CONST_BITS`] bits ([`ln2_series`]).
const LN2_STORED: Stored = Stored {
    lo: (
        concat!(
            "b17217f7d1cf79abc9e3b39803f2f6af40f343267298b62d8a0d175b8baafa2be7b876206debac98",
            "559552fb4afa1b10ed2eae35c138214427573b291169b8253e96ca16224ae8c51acbda11317c387e",
            "b9ea9bc3b136603b256fa0ec7657f74b72ce87b19d6548caf5dfa6bd38303248655fa1872f20e3a2",
            "da2d97c50f3fd5c607f4ca11fb5bfb90610d30f88fe551a2ee569d6dfc1efa157d2e23de1400b396",
            "17460775db8990e5c943e732b479cd33cccc4e659393514c4c1a1e0bd1d6095d25669b333564a337",
            "6a9c7f8a5e148e82074db6015cfe7aa30c480a5417350d2c955d5179b1e17b9dae313cdb6c606cb1",
            "078f735d1b2db31b5f50b5185064c18b4d162db3b365853d7598a1951ae273ee5570b6c68f969834",
            "96d4e6d330af889b44a02554731cdc8ea17293d1228a4ef98d6f5177fbcf0755268a5c1f9538b982",
            "61affd446b1ca3cf5e9222b88c66d3c5422183edc99421090bbb16faf3d949f236e02b20cee886b9",
            "05c128d53d0bd2f9621363196af503020060e49908391a0c57339ba2beba7d052ac5b61cc4e9207c",
            "ef2f0ce2d7373958d7622658901e646a95184460dc4e7487156e0c292413d5e361c1696dd24aaebd",
            "473826fda0c238b90ab111bbbd67c724972cd18bfbbd9d426c472096e76115c05f6f7cebac9f45ae",
            "cecb72f19c38339d8f6826250dea891ef07afff3a892374e175eb4afc8daadd885db682e",
        ),
        -4128,
    ),
    hi: (
        concat!(
            "b17217f7d1cf79abc9e3b39803f2f6af40f343267298b62d8a0d175b8baafa2be7b876206debac98",
            "559552fb4afa1b10ed2eae35c138214427573b291169b8253e96ca16224ae8c51acbda11317c387e",
            "b9ea9bc3b136603b256fa0ec7657f74b72ce87b19d6548caf5dfa6bd38303248655fa1872f20e3a2",
            "da2d97c50f3fd5c607f4ca11fb5bfb90610d30f88fe551a2ee569d6dfc1efa157d2e23de1400b396",
            "17460775db8990e5c943e732b479cd33cccc4e659393514c4c1a1e0bd1d6095d25669b333564a337",
            "6a9c7f8a5e148e82074db6015cfe7aa30c480a5417350d2c955d5179b1e17b9dae313cdb6c606cb1",
            "078f735d1b2db31b5f50b5185064c18b4d162db3b365853d7598a1951ae273ee5570b6c68f969834",
            "96d4e6d330af889b44a02554731cdc8ea17293d1228a4ef98d6f5177fbcf0755268a5c1f9538b982",
            "61affd446b1ca3cf5e9222b88c66d3c5422183edc99421090bbb16faf3d949f236e02b20cee886b9",
            "05c128d53d0bd2f9621363196af503020060e49908391a0c57339ba2beba7d052ac5b61cc4e9207c",
            "ef2f0ce2d7373958d7622658901e646a95184460dc4e7487156e0c292413d5e361c1696dd24aaebd",
            "473826fda0c238b90ab111bbbd67c724972cd18bfbbd9d426c472096e76115c05f6f7cebac9f45ae",
            "cecb72f19c38339d8f6826250dea891ef07afff3a892374e175eb4afc8daadd885db6d46",
        ),
        -4128,
    ),
};

/// `atan(j/8)` for `j` from 1 to 8, to [`CONST_BITS`] bits ([`atan_eighths_series`]).
const ATAN_EIGHTHS_STORED: [Stored; 8] = [
    Stored {
        lo: (
            concat!(
                "feadd4d5617b6e32c897989f3e888ef78b3957d95d9ad922ec93577dfb6c1768d4d0080201d9c265",
                "3ffa5d51e234d71d5d73c989f29afbfd2a6cd88abf846f6ddf73a28dfd41142d7d5842cbcb50ebae",
                "e00ca5a509e8bf8084f277c04b4155a398e8bcb0298c11696d1be70a0790499b02743e1d11b502f8",
                "9ab89060da2e3f9f4140485afd77c58f2997c78866798fd45f9ef5d1f1b4bc6450aee0cbdec78cf0",
                "e0c682fe45d8d2f5701377f7d9986f86b845ec3968f964b25814c3b5f124f50121821cea3c6d4d98",
                "cabb3109de9491f7e8332c52ef67b23e5b6da7f7c5e892334a265ef39395f8f1ab6a8d4283bd146d",
                "a89778c5d35cd368c55b9f2f8d95f9f5120751bc2d7afe56690003e27533eafa1110fa66cd281524",
                "028c82fae1c42578c3caa33971db95ffc07a730ccc5b6c37442044590cddd824d5c0f91600b6ff50",
                "1a742051cc0075066691d9c81ecc9a24bfb6559f9d36af0997ef6488b5720c76b03663f2a965b5f4",
                "bc41b543d3a36d24785c63f653978680c4ee03eea5f81ac5ac7de9db583da9ed15c9bfd872335ff4",
                "22ec33c491b6d138170e43915b6e64ebc966e46779ba9c19364126b83c36147a9be4d1055689805f",
                "dda98c7b84c4b513d697632e7ef6d2ade4d525a3549c4b9da9e553f76b0076e120338cf621bf51e3",
                "40d7661f3dee39da57d31783f8f407ae805e8eac6aa45ace48bd9a4dfd51aaa2359882c9",
            ),
            -4131,
        ),
        hi: (
            concat!(
                "feadd4d5617b6e32c897989f3e888ef78b3957d95d9ad922ec93577dfb6c1768d4d0080201d9c265",
                "3ffa5d51e234d71d5d73c989f29afbfd2a6cd88abf846f6ddf73a28dfd41142d7d5842cbcb50ebae",
                "e00ca5a509e8bf8084f277c04b4155a398e8bcb0298c11696d1be70a0790499b02743e1d11b502f8",
                "9ab89060da2e3f9f4140485afd77c58f2997c78866798fd45f9ef5d1f1b4bc6450aee0cbdec78cf0",
                "e0c682fe45d8d2f5701377f7d9986f86b845ec3968f964b25814c3b5f124f50121821cea3c6d4d98",
                "cabb3109de9491f7e8332c52ef67b23e5b6da7f7c5e892334a265ef39395f8f1ab6a8d4283bd146d",
                "a89778c5d35cd368c55b9f2f8d95f9f5120751bc2d7afe56690003e27533eafa1110fa66cd281524",
                "028c82fae1c42578c3caa33971db95ffc07a730ccc5b6c37442044590cddd824d5c0f91600b6ff50",
                "1a742051cc0075066691d9c81ecc9a24bfb6559f9d36af0997ef6488b5720c76b03663f2a965b5f4",
                "bc41b543d3a36d24785c63f653978680c4ee03eea5f81ac5ac7de9db583da9ed15c9bfd872335ff4",
                "22ec33c491b6d138170e43915b6e64ebc966e46779ba9c19364126b83c36147a9be4d1055689805f",
                "dda98c7b84c4b513d697632e7ef6d2ade4d525a3549c4b9da9e553f76b0076e120338cf621bf51e3",
                "40d7661f3dee39da57d31783f8f407ae805e8eac6aa45ace48bd9a4dfd51aaa235988579",
            ),
            -4131,
        ),
    },
    Stored {
        lo: (
            concat!(
                "fadbafc96406eb156dc79ef5f7a217e5aa7fa90388b3836b7a3a767c9449a76592b9251668e57653",
                "05be8c5ba5831a3e3c2bc227071e4f9a0f41c3ab039983799e8ab743e3f29a57f689c8c55d5bdc90",
                "08d12357ace27207e121ce3f3e5bcb2dbb94779f67a848130c6c29964e0918fcd4808c045cb7f4f9",
                "d85a2adbd4b30439bcf4a40cd8e290f9ded155d017a29674f80c5af85506ba9a3b2438dea2af887b",
                "53dd19486fb5374ce2c5431d96a6da09a4a22a87ec149c536fec2877f3f2d4a3837ddefc641a6a57",
                "68dc3983bae861d524ba5431fd598f3af289121c9c0d7d4d1d22649fe808f40c3d05899ed2ecf944",
                "a1843a9094661a08f96b3800b947bc9ccbeb14370e35900dfbb7d11e061e3e87cab70665410180c2",
                "6a3f27a2acb45c3e2bbf9bbdd20be923957fa8108f1cc79a03a5e20012515bb05f0f3bc52ea1e7a3",
                "2f724590478f2f948c067c8691f0b5756f38f75f2f3ad64d7b79bc9fd783668f6ebcc04d3c34ce2d",
                "5e6babe13aae072d2f80a72251526ceb56d34315ac43a0b7c5a2719fea8570e7fd811d422412b5fe",
                "0630a640968d31d7d68dc156eb988e6708cc15d570cdce0054c9cd2f4b52f0aecebb60dc9380b0d1",
                "4f75309a84d7f396691c1a0da838bd7028ed13f6bf0aee78761b6ee58add67fef5bf93546fbc928c",
                "243cf4bfc23e5cf84f60e5cd5e5f59890f81569862c70d888e862ab4394b4ed02d7b7e67",
            ),
            -4130,
        ),
        hi: (
            concat!(
                "fadbafc96406eb156dc79ef5f7a217e5aa7fa90388b3836b7a3a767c9449a76592b9251668e57653",
                "05be8c5ba5831a3e3c2bc227071e4f9a0f41c3ab039983799e8ab743e3f29a57f689c8c55d5bdc90",
                "08d12357ace27207e121ce3f3e5bcb2dbb94779f67a848130c6c29964e0918fcd4808c045cb7f4f9",
                "d85a2adbd4b30439bcf4a40cd8e290f9ded155d017a29674f80c5af85506ba9a3b2438dea2af887b",
                "53dd19486fb5374ce2c5431d96a6da09a4a22a87ec149c536fec2877f3f2d4a3837ddefc641a6a57",
                "68dc3983bae861d524ba5431fd598f3af289121c9c0d7d4d1d22649fe808f40c3d05899ed2ecf944",
                "a1843a9094661a08f96b3800b947bc9ccbeb14370e35900dfbb7d11e061e3e87cab70665410180c2",
                "6a3f27a2acb45c3e2bbf9bbdd20be923957fa8108f1cc79a03a5e20012515bb05f0f3bc52ea1e7a3",
                "2f724590478f2f948c067c8691f0b5756f38f75f2f3ad64d7b79bc9fd783668f6ebcc04d3c34ce2d",
                "5e6babe13aae072d2f80a72251526ceb56d34315ac43a0b7c5a2719fea8570e7fd811d422412b5fe",
                "0630a640968d31d7d68dc156eb988e6708cc15d570cdce0054c9cd2f4b52f0aecebb60dc9380b0d1",
                "4f75309a84d7f396691c1a0da838bd7028ed13f6bf0aee78761b6ee58add67fef5bf93546fbc928c",
                "243cf4bfc23e5cf84f60e5cd5e5f59890f81569862c70d888e862ab4394b4ed02d7b825c",
            ),
            -4130,
        ),
    },
    Stored {
        lo: (
            concat!(
                "b7b0ca0f26f784738aa32122dcfe44833d843977ae5455cc4733e5ca334908e064cc582039fdc0d6",
                "dccce4a800cf73d28616ee077afa1a60487d5aad34939d5dbe774cd6c1c5e701dfbc2b6487386d59",
                "dff6921602e38295b6ea50b949d0f6746ad2bf3aebfb3337e090df743e49e9f691c3ecf34f8328e5",
                "54ab50f87782b47c1d60992a752c83b5fe7979c8e7f33b1dea95bcfd17b74bc6611862eaaf8ac72b",
                "4222281cc44da737689d4d3ab5448db0660079deb1006f7c21d519cc42e58190cf4af3f6fb732f04",
                "54e8cfe0421d6c2cb019810930f660ddb54209b21c47ac16e6c787b03c4aba129bf8cf118a208929",
                "767da715e669580936796377cb1fc01f03ca0398bf8f8a359b4de097636a5f8e5081b6f316c09c38",
                "ec713873a53ffe481ef577fc65b76e0067a1d55dbf20ddbf07c4dd42897d9d9c77d2e009a2dca856",
                "57d41f2ea8761af15da903464a76b805260d955ccc23d223bd8a8fbf9808977a870ed2242ef5c5d8",
                "de54321c2e15b0cda75734a6e6c784d7be34ed08d8676345ebed1a99f68648425ca921892bb11739",
                "84165bb6f6e0400b127e6cbb33e314f00cd06949c298d04881a6a617fb8a65b1264c9e9a258acb4f",
                "9f113c344c7c37467c841fe067ebd308d19184c437c93bfa404ab18f4b80ec874f8e6e2f8b68f73c",
                "3edeb332ed8cf59e4ab6f0c3913fb6e408c6d775af6d23b138e3b9e8c4cb31393b7b53c2",
            ),
            -4129,
        ),
        hi: (
            concat!(
                "b7b0ca0f26f784738aa32122dcfe44833d843977ae5455cc4733e5ca334908e064cc582039fdc0d6",
                "dccce4a800cf73d28616ee077afa1a60487d5aad34939d5dbe774cd6c1c5e701dfbc2b6487386d59",
                "dff6921602e38295b6ea50b949d0f6746ad2bf3aebfb3337e090df743e49e9f691c3ecf34f8328e5",
                "54ab50f87782b47c1d60992a752c83b5fe7979c8e7f33b1dea95bcfd17b74bc6611862eaaf8ac72b",
                "4222281cc44da737689d4d3ab5448db0660079deb1006f7c21d519cc42e58190cf4af3f6fb732f04",
                "54e8cfe0421d6c2cb019810930f660ddb54209b21c47ac16e6c787b03c4aba129bf8cf118a208929",
                "767da715e669580936796377cb1fc01f03ca0398bf8f8a359b4de097636a5f8e5081b6f316c09c38",
                "ec713873a53ffe481ef577fc65b76e0067a1d55dbf20ddbf07c4dd42897d9d9c77d2e009a2dca856",
                "57d41f2ea8761af15da903464a76b805260d955ccc23d223bd8a8fbf9808977a870ed2242ef5c5d8",
                "de54321c2e15b0cda75734a6e6c784d7be34ed08d8676345ebed1a99f68648425ca921892bb11739",
                "84165bb6f6e0400b127e6cbb33e314f00cd06949c298d04881a6a617fb8a65b1264c9e9a258acb4f",
                "9f113c344c7c37467c841fe067ebd308d19184c437c93bfa404ab18f4b80ec874f8e6e2f8b68f73c",
                "3edeb332ed8cf59e4ab6f0c3913fb6e408c6d775af6d23b138e3b9e8c4cb31393b7b591c",
            ),
            -4129,
        ),
    },
    Stored {
        lo: (
            concat!(
                "ed63382b0dda7b456fe445ecbc3a8d036e141587261cdf00e2cf16e6e9624709fa9c5917892b516c",
                "87c812f8c6a46182cee1e80efd3c0013d6d85e3686228dbd7913475341b29b656ebfea02086ccb07",
                "5336dd0907697538dfd26c5be1547791c1689ec34bd7be5befaeb8b1bfee054d3c36a6b0400bd856",
                "f0d36426a3baf6a7196cf7ba12c6925e333ec575d9967b05cc47e00763c7bb3b2beed3b88a75fb6a",
                "be2f2645c8913e64c3ddf33a81c6e1b23957e40f46313b0e2a0b588f9c875376b7f9d6d6828e3e8f",
                "0ee6e2923889d0ba493bb9721de2d5809c6b61e92b3329d1bdc3044455811a9267e2298b3447ed0c",
                "f40339dd0b54957a04ef35b342113e3b93b9ceb6d527472f25860ecb3ecf9f23c950fdf8a412c030",
                "0c2e88b259314d0cce4f48d4b7fb97d100ca90660ca00b8a7e44e68c4b04df22da8ff38f518c1a22",
                "4a4193bf7b1651fdf2cd895a58333d5584286da04fa09ca4271c23e5f59f28c1ab649e1f649730af",
                "232c8de18bef78c0a1c60fa2159881867a3f4622a362a873fb7c73f4649cf9853a7c776ec7d54459",
                "d8a5623c8d135aa61fcdc1aca02a8d25e0e98789a1e45edd12a0ebc624726b8f00f2a38a22a3aa60",
                "4b7d1c91d87717230f5a150a64f380dece05ffc2f42d6f8d0c7f4f18f998760699dad46b02f76596",
                "595df2fa9a7fb7172ec888154391f22cc1a69b856dd1aa3e052aa50c63a9fc1d21a9fc07",
            ),
            -4129,
        ),
        hi: (
            concat!(
                "ed63382b0dda7b456fe445ecbc3a8d036e141587261cdf00e2cf16e6e9624709fa9c5917892b516c",
                "87c812f8c6a46182cee1e80efd3c0013d6d85e3686228dbd7913475341b29b656ebfea02086ccb07",
                "5336dd0907697538dfd26c5be1547791c1689ec34bd7be5befaeb8b1bfee054d3c36a6b0400bd856",
                "f0d36426a3baf6a7196cf7ba12c6925e333ec575d9967b05cc47e00763c7bb3b2beed3b88a75fb6a",
                "be2f2645c8913e64c3ddf33a81c6e1b23957e40f46313b0e2a0b588f9c875376b7f9d6d6828e3e8f",
                "0ee6e2923889d0ba493bb9721de2d5809c6b61e92b3329d1bdc3044455811a9267e2298b3447ed0c",
                "f40339dd0b54957a04ef35b342113e3b93b9ceb6d527472f25860ecb3ecf9f23c950fdf8a412c030",
                "0c2e88b259314d0cce4f48d4b7fb97d100ca90660ca00b8a7e44e68c4b04df22da8ff38f518c1a22",
                "4a4193bf7b1651fdf2cd895a58333d5584286da04fa09ca4271c23e5f59f28c1ab649e1f649730af",
                "232c8de18bef78c0a1c60fa2159881867a3f4622a362a873fb7c73f4649cf9853a7c776ec7d54459",
                "d8a5623c8d135aa61fcdc1aca02a8d25e0e98789a1e45edd12a0ebc624726b8f00f2a38a22a3aa60",
                "4b7d1c91d87717230f5a150a64f380dece05ffc2f42d6f8d0c7f4f18f998760699dad46b02f76596",
                "595df2fa9a7fb7172ec888154391f22cc1a69b856dd1aa3e052aa50c63a9fc1d21aa02fc",
            ),
            -4129,
        ),
    },
    Stored {
        lo: (
            concat!(
                "8f005d5ef7f59f9b5c835e1665c43747918a67e0652b375cf53da46d13389eb23669dcd3918d712b",
                "66cd7dd3073d6a5829c4ed1d7e523acc358f6440a4333141b537a12970b8e8b14c84abda2b7c78e3",
                "a6e8474afffebb461d3c5168ed9c5298ac46b084f3cc4a1d1f4e7c2a0cd9e15f5eacd843bc90c7e2",
                "2b6cd2696f27ec33aa0ae62ae1bc6398ed1ad15ebe2c0bfeb2f16db8fc66530bd7c55a2920ab4f9f",
                "341b6c8159d7338afb5db36199c4d8d6e3fd231f70ab2f55c6d415ba14246e901dedd69b8a57afd3",
                "61259e841b55bcd418ca874f5cb2f727f462e026e8ee8b23da3a261f3cb604a6943cfa7f7d3bf76d",
                "7eb196d347e97254162e6e280fce6b7d0a9d5502755419f8c793c5a23052d9f63130e701400cfc00",
                "fe015848f117c8824e8a732fc259b5898c062ce3c21f3fde745d08e8e652804a05075958d922f0f5",
                "db924f311b03337067698a982165673550c66baedab19a6df20a6e5dab16409bef89efc136aef9f9",
                "234daf4d319b9ae32200b3c269d7f17c5536076af2324a1cb66361850612643e85d047c836328870",
                "348a3b172fa1a54bc729e0aa3c5f0b311756d6348a79f696d7288a074c40a33a8b9dd91d33494168",
                "34df8b201c3fee63d702f379fece172ad6948b52fe4f4020a87e4befa7f09f6cf94e70f5503ab9cb",
                "1ab3fa5cc57277a63f64c51e93a94329ed5d96c814f8b9e82e30d0226f534d1b8d39224c",
            ),
            -4128,
        ),
        hi: (
            concat!(
                "8f005d5ef7f59f9b5c835e1665c43747918a67e0652b375cf53da46d13389eb23669dcd3918d712b",
                "66cd7dd3073d6a5829c4ed1d7e523acc358f6440a4333141b537a12970b8e8b14c84abda2b7c78e3",
                "a6e8474afffebb461d3c5168ed9c5298ac46b084f3cc4a1d1f4e7c2a0cd9e15f5eacd843bc90c7e2",
                "2b6cd2696f27ec33aa0ae62ae1bc6398ed1ad15ebe2c0bfeb2f16db8fc66530bd7c55a2920ab4f9f",
                "341b6c8159d7338afb5db36199c4d8d6e3fd231f70ab2f55c6d415ba14246e901dedd69b8a57afd3",
                "61259e841b55bcd418ca874f5cb2f727f462e026e8ee8b23da3a261f3cb604a6943cfa7f7d3bf76d",
                "7eb196d347e97254162e6e280fce6b7d0a9d5502755419f8c793c5a23052d9f63130e701400cfc00",
                "fe015848f117c8824e8a732fc259b5898c062ce3c21f3fde745d08e8e652804a05075958d922f0f5",
                "db924f311b03337067698a982165673550c66baedab19a6df20a6e5dab16409bef89efc136aef9f9",
                "234daf4d319b9ae32200b3c269d7f17c5536076af2324a1cb66361850612643e85d047c836328870",
                "348a3b172fa1a54bc729e0aa3c5f0b311756d6348a79f696d7288a074c40a33a8b9dd91d33494168",
                "34df8b201c3fee63d702f379fece172ad6948b52fe4f4020a87e4befa7f09f6cf94e70f5503ab9cb",
                "1ab3fa5cc57277a63f64c51e93a94329ed5d96c814f8b9e82e30d0226f534d1b8d392b1e",
            ),
            -4128,
        ),
    },
    Stored {
        lo: (
            concat!(
                "a4bc7d1934f7092419a87f2a457dac9ee3f08689eeb2b9e7214866658cc4ef3aa7f7b7db933cb84f",
                "5762206ed3d024b391742ccce782285ac8ea0ca45480f6ce4ff82399830a622879d89bd144030fce",
                "c4c7dc64e0a3faa2fc9e6b9ad3bec7b99bd5a95facbe8170a2a213f219dab12e47ca52c102bba5b4",
                "40e12c4594f0b08db8bf8797e7830c60d38bf4d1dfb0e0266c7e06a4dd42ea3c11bb7e5656b73170",
                "0fe94456cce7f1a51f0ae4d7126961462bc8d87d163c61699d3195c89395b8907e55306f55810690",
                "5ca3d94ea60ed4d9731bde7b0ccd58afd6bf3110a9f7abf86de147ecdc72ef8dc302f329e10d9b4e",
                "6662f43cfd4c5eed4bbb0da47c28368e463d3b51dc9096e5f04ed3e37b3c79d79ddd2112a9b1095f",
                "4bbcd4675ce11aa16ec9e0ebdc4f2b6a9cfd13e62905d14d641b0d81680f31a6d65c11572c04baa6",
                "59fcc270b3dfee1b84f4a5549c8f7d8369eaa3e12611f11feaa87b5af42c2da0dc5219da5d62f06d",
                "73d9145fc6529741931e688339774c2896a3edfed8120dd93809da3c714ccaf2faac58faa454d35a",
                "72627157c8953f2b97a9c409699290cc7c34709e9d99366f3e57c6c87865152c327289151e7ddce3",
                "fafa267aca40b8b72f683dd738d22e80a231bb294eb320ac938dd336b1c83f4c8d8f00c6182855ec",
                "b4a17cbe86cdca076d1fe37d247316f7abe159e43727360f7e7f14585ca291a071403803",
            ),
            -4128,
        ),
        hi: (
            concat!(
                "a4bc7d1934f7092419a87f2a457dac9ee3f08689eeb2b9e7214866658cc4ef3aa7f7b7db933cb84f",
                "5762206ed3d024b391742ccce782285ac8ea0ca45480f6ce4ff82399830a622879d89bd144030fce",
                "c4c7dc64e0a3faa2fc9e6b9ad3bec7b99bd5a95facbe8170a2a213f219dab12e47ca52c102bba5b4",
                "40e12c4594f0b08db8bf8797e7830c60d38bf4d1dfb0e0266c7e06a4dd42ea3c11bb7e5656b73170",
                "0fe94456cce7f1a51f0ae4d7126961462bc8d87d163c61699d3195c89395b8907e55306f55810690",
                "5ca3d94ea60ed4d9731bde7b0ccd58afd6bf3110a9f7abf86de147ecdc72ef8dc302f329e10d9b4e",
                "6662f43cfd4c5eed4bbb0da47c28368e463d3b51dc9096e5f04ed3e37b3c79d79ddd2112a9b1095f",
                "4bbcd4675ce11aa16ec9e0ebdc4f2b6a9cfd13e62905d14d641b0d81680f31a6d65c11572c04baa6",
                "59fcc270b3dfee1b84f4a5549c8f7d8369eaa3e12611f11feaa87b5af42c2da0dc5219da5d62f06d",
                "73d9145fc6529741931e688339774c2896a3edfed8120dd93809da3c714ccaf2faac58faa454d35a",
                "72627157c8953f2b97a9c409699290cc7c34709e9d99366f3e57c6c87865152c327289151e7ddce3",
                "fafa267aca40b8b72f683dd738d22e80a231bb294eb320ac938dd336b1c83f4c8d8f00c6182855ec",
                "b4a17cbe86cdca076d1fe37d247316f7abe159e43727360f7e7f14585ca291a0714042f6",
            ),
            -4128,
        ),
    },
    Stored {
        lo: (
            concat!(
                "b8053e2bc2319e73cb2da55210a4443d3d7aecc114c79a80a012155f64cae5304655f065cec98182",
                "49c49ec72668272ce8c53e214380781c38257e41945e8cf3b4ed0b1b38a6203d146cd2b24e757ad2",
                "f67a0a719d2b4cea8817ff1d952a0849f51d7e623bad774e8db35060de1b70dc71452f3b3973153e",
                "baa246e3d542c0195d9a684203fec39c1adce0bdc1776a0b582ea46ce47d21107d90fa2d8c064a69",
                "b3d2aa71a96198d85662675d8df67aca11fd8071062bdf11f4ad6779ddf847807088ecdcc8521a32",
                "6a36a59efc47098029ff348d2c7371156c25ae97ec3bfa5173a96eb69212e000b3527466908c1548",
                "68e875d9f6c8679d4b9e90f0fa231cbdb847f6cd9e8a169f55aa46fac5a9d03a4c64cc00a1e219f3",
                "4c6f36cb5925de0e276f11cb5a4f43ea3ddd3354a56d056acfe4cadf5abf6f4ff4428521bcfb2f08",
                "ffb5cfa75e6d7d76e880352dba6a6cc8f18f4ca7f8663f10c54c14099536db0638cd65420a546345",
                "33dc302961acbee60e6235807479133ba78d52791df6340d12186d8c7c29621c8e22b3e693998c9c",
                "07d9719f8cfddfb272b8884690ce8ca3df4dce26a5eb0431d5b79ce995a6f5b35d17b0335aba4314",
                "f17685d63ee5dd8def79037044ae42469d315cdff68fe7d240aed9d449618b7570bc8beb73b0ce2e",
                "d03950a2e9ba4420b06ce708f372b62518b029b4e724d190a93885f83a8f9a0312fa38ca",
            ),
            -4128,
        ),
        hi: (
            concat!(
                "b8053e2bc2319e73cb2da55210a4443d3d7aecc114c79a80a012155f64cae5304655f065cec98182",
                "49c49ec72668272ce8c53e214380781c38257e41945e8cf3b4ed0b1b38a6203d146cd2b24e757ad2",
                "f67a0a719d2b4cea8817ff1d952a0849f51d7e623bad774e8db35060de1b70dc71452f3b3973153e",
                "baa246e3d542c0195d9a684203fec39c1adce0bdc1776a0b582ea46ce47d21107d90fa2d8c064a69",
                "b3d2aa71a96198d85662675d8df67aca11fd8071062bdf11f4ad6779ddf847807088ecdcc8521a32",
                "6a36a59efc47098029ff348d2c7371156c25ae97ec3bfa5173a96eb69212e000b3527466908c1548",
                "68e875d9f6c8679d4b9e90f0fa231cbdb847f6cd9e8a169f55aa46fac5a9d03a4c64cc00a1e219f3",
                "4c6f36cb5925de0e276f11cb5a4f43ea3ddd3354a56d056acfe4cadf5abf6f4ff4428521bcfb2f08",
                "ffb5cfa75e6d7d76e880352dba6a6cc8f18f4ca7f8663f10c54c14099536db0638cd65420a546345",
                "33dc302961acbee60e6235807479133ba78d52791df6340d12186d8c7c29621c8e22b3e693998c9c",
                "07d9719f8cfddfb272b8884690ce8ca3df4dce26a5eb0431d5b79ce995a6f5b35d17b0335aba4314",
                "f17685d63ee5dd8def79037044ae42469d315cdff68fe7d240aed9d449618b7570bc8beb73b0ce2e",
                "d03950a2e9ba4420b06ce708f372b62518b029b4e724d190a93885f83a8f9a0312fa462e",
            ),
            -4128,
        ),
    },
    Stored {
        lo: (
            concat!(
                "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b139b22514a08798e3404dd",
                "ef9519b3cd3a431b302b0a6df25f14374fe1356d6d51c245e485b576625e7ec6f44c42e9a637ed6b",
                "0bff5cb6f406b7edee386bfb5a899fa5ae9f24117c4b1fe649286651ece45b3dc2007cb8a163bf05",
                "98da48361c55d39a69163fa8fd24cf5f83655d23dca3ad961c62f356208552bb9ed529077096966d",
                "670c354e4abc9804f1746c08ca18217c32905e462e36ce3be39e772c180e86039b2783a2ec07a28f",
                "b5c55df06f4c52c9de2bcbf6955817183995497cea956ae515d2261898fa051015728e5a8aaac42d",
                "ad33170d04507a33a85521abdf1cba64ecfb850458dbef0a8aea71575d060c7db3970f85a6e1e4c7",
                "abf5ae8cdb0933d71e8c94e04a25619dcee3d2261ad2ee6bf12ffa06d98a0864d87602733ec86a64",
                "521f2b18177b200cbbe117577a615d6c770988c0bad946e208e24fa074e5ab3143db5bfce0fd108e",
                "4b82d120a92108011a723c12a787e6d788719a10bdba5b2699c327186af4e23c1a946834b6150bda",
                "2583e9ca2ad44ce8dbbbc2db04de8ef92e8efc141fbecaa6287c59474e6bc05d99b2964fa090c3a2",
                "233ba186515be7ed1f612970cee2d7afb81bdd762170481cd0069127d5b05aa993b4ea988d8fddc1",
                "86ffb7dc90a6c08f4df435c93402849236c3fab4d27c7026c1d4dcb2602646dec975167b",
            ),
            -4128,
        ),
        hi: (
            concat!(
                "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b139b22514a08798e3404dd",
                "ef9519b3cd3a431b302b0a6df25f14374fe1356d6d51c245e485b576625e7ec6f44c42e9a637ed6b",
                "0bff5cb6f406b7edee386bfb5a899fa5ae9f24117c4b1fe649286651ece45b3dc2007cb8a163bf05",
                "98da48361c55d39a69163fa8fd24cf5f83655d23dca3ad961c62f356208552bb9ed529077096966d",
                "670c354e4abc9804f1746c08ca18217c32905e462e36ce3be39e772c180e86039b2783a2ec07a28f",
                "b5c55df06f4c52c9de2bcbf6955817183995497cea956ae515d2261898fa051015728e5a8aaac42d",
                "ad33170d04507a33a85521abdf1cba64ecfb850458dbef0a8aea71575d060c7db3970f85a6e1e4c7",
                "abf5ae8cdb0933d71e8c94e04a25619dcee3d2261ad2ee6bf12ffa06d98a0864d87602733ec86a64",
                "521f2b18177b200cbbe117577a615d6c770988c0bad946e208e24fa074e5ab3143db5bfce0fd108e",
                "4b82d120a92108011a723c12a787e6d788719a10bdba5b2699c327186af4e23c1a946834b6150bda",
                "2583e9ca2ad44ce8dbbbc2db04de8ef92e8efc141fbecaa6287c59474e6bc05d99b2964fa090c3a2",
                "233ba186515be7ed1f612970cee2d7afb81bdd762170481cd0069127d5b05aa993b4ea988d8fddc1",
                "86ffb7dc90a6c08f4df435c93402849236c3fab4d27c7026c1d4dcb2602646dec975269d",
            ),
            -4128,
        ),
    },
];

/// π to [`CONST_BITS`] bits.
pub(crate) fn pi() -> Iv {
    from_stored(&PI_STORED)
}

/// ln 2 to [`CONST_BITS`] bits.
pub(crate) fn ln2() -> Iv {
    from_stored(&LN2_STORED)
}

/// `atan(j/8)` for `1 ≤ j ≤ 8`.
pub(crate) fn atan_eighths(j: i64) -> Iv {
    from_stored(&ATAN_EIGHTHS_STORED[(j - 1) as usize])
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
    let mut m = d.scale(-e).to_f64();
    // log2 m for m in [1, 2), one bit per squaring: only multiplications, which IEEE 754 rounds
    // correctly, so the same report everywhere (not `log2`, which it does not).
    let (mut l, mut bit) = (0.0, 0.5);
    for _ in 0..30 {
        m *= m;
        if m >= 2.0 {
            m *= 0.5;
            l += bit;
        }
        bit *= 0.5;
    }
    e as f64 + l
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
