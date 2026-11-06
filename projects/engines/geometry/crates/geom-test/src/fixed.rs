//! Fixed-point numbers of 400 fractional bits: the higher precision of the law catalogue's dense
//! refutation sampler (piece G69, chapter 28.3), in which every bound a kind or a NURBS operation
//! states is refuted if it is wrong.
//!
//! What it does: an [`IBig`] n stands for n / 2^400. [`of`] reads a double exactly but for the
//! bits below 2^−400, [`mul`] and [`div`] cut their results to a whole number of 2^−400, [`sqrt`]
//! is the floor of the root, [`cos_sin`] and [`exp`] sum their Taylor series until a term vanishes,
//! and π is read from its first 100 decimal places. [`cos_sin_pi`], [`tan_pi`] and [`tanh`] are
//! built on them, for the half-turn angles and the tangent half-angle maps of the kinds.
//! [`rational`] and [`of_rational`] pass values to and from the exact rationals of
//! [`crate::nurbs_reference`], and [`within`] decides whether two points are at most a distance
//! apart.
//!
//! Guarantees:
//!
//! - **Far finer than a double.** Each cut is below 2^−400; a series of up to a few hundred terms
//!   adds a few hundred cuts; π's 100 decimal places err by less than 2^−332, which [`cos_sin_pi`]
//!   carries once after reducing t exactly to [−1, 1). Values from a few operations on arguments
//!   below 2^20 are within 2^−300 of the exact ones, against the 2^−53 relative of a double, so a
//!   law checked with a margin of 2^−200 is never decided by the truth's own error.
//! - **Independent of the kernel.** Integer arithmetic and series only: no call into a kernel
//!   crate and no `f64` maths.
//! - **The same bits on every target.** Integer arithmetic throughout.
//!
//! Method: the Taylor series of cos, sin and exp in fixed point, the argument of exp halved until
//! it is at most 1 and the result squared back (R. P. Brent, "Fast multiple-precision evaluation
//! of elementary functions", *Journal of the ACM* 23 (1976), 242–251,
//! <https://doi.org/10.1145/321941.321944>); the integer square root of dashu; the fixed point of
//! piece G61's attack tests (`geom-curve`'s `tests/attack_g61.rs`), which this module generalises
//! for the law catalogue of chapter 28.3
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: a product or quotient of k-bit numbers is O(k²) at these sizes; [`cos_sin`] and [`exp`]
//! about 150 terms each; no recursion.

// The lint is for fixed-width arithmetic, which can overflow. Every operator here is on dashu's
// arbitrary-precision integers, which cannot, and every division is by a value shown nonzero
// beside it (a power of two or ten, a term count, or a divisor the caller names nonzero).
#![allow(
    clippy::arithmetic_side_effects,
    reason = "arbitrary-precision integer arithmetic, which cannot overflow; divisors are shown nonzero"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': a, b and r points and a distance, s a sum, d a difference, x and y coordinates"
)]

use dashu_int::IBig;
use dashu_ratio::RBig;

/// The fractional bits: n stands for n / 2^F.
pub const F: usize = 400;

/// π to 100 decimal places.
const PI_DIGITS: &str = "31415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679";

/// 1.
#[must_use]
pub fn one() -> IBig {
    IBig::ONE << F
}

/// `x` exactly but for the bits below 2^−400; `None` if `x` is not finite.
#[must_use]
pub fn of(x: f64) -> Option<IBig> {
    RBig::try_from(x).ok().map(|q| of_rational(&q))
}

/// n / d, within 2^−400; `None` if d is zero.
#[must_use]
pub fn frac(n: i64, d: i64) -> Option<IBig> {
    (d != 0).then(|| (IBig::from(n) << F) / IBig::from(d))
}

/// a·b, within 2^−400.
#[must_use]
pub fn mul(a: &IBig, b: &IBig) -> IBig {
    (a * b) >> F
}

/// a / b, within 2^−400; `None` if b is zero.
#[must_use]
pub fn div(a: &IBig, b: &IBig) -> Option<IBig> {
    (*b != IBig::ZERO).then(|| (a << F) / b)
}

/// |a|.
#[must_use]
pub fn abs(a: &IBig) -> IBig {
    if *a < IBig::ZERO {
        -a.clone()
    } else {
        a.clone()
    }
}

/// √a, cut toward 0; `None` if a is negative.
#[must_use]
pub fn sqrt(a: &IBig) -> Option<IBig> {
    (*a >= IBig::ZERO).then(|| (a << F).nth_root(2))
}

/// π, within 2^−332.
#[must_use]
pub fn pi() -> IBig {
    let digits: IBig = PI_DIGITS.parse().unwrap_or(IBig::ZERO);
    (digits << F) / IBig::from(10u8).pow(100)
}

/// cos x and sin x by their series, for |x| ≤ 4 (more terms are summed as long as one counts).
#[must_use]
pub fn cos_sin(x: &IBig) -> (IBig, IBig) {
    let mut term = one();
    let (mut c, mut s) = (IBig::ZERO, IBig::ZERO);
    for n in 0..1000usize {
        match n % 4 {
            0 => c += &term,
            1 => s += &term,
            2 => c -= &term,
            _ => s -= &term,
        }
        term = mul(&term, x) / IBig::from(n + 1);
        if term == IBig::ZERO {
            break;
        }
    }
    (c, s)
}

/// cos πt and sin πt, t reduced exactly to [−1, 1) first.
#[must_use]
pub fn cos_sin_pi(t: &IBig) -> (IBig, IBig) {
    let k = (t + one()) >> (F + 1);
    let r = t - (k << (F + 1));
    cos_sin(&mul(&r, &pi()))
}

/// tan πt; `None` where cos πt is zero in the fixed point.
#[must_use]
pub fn tan_pi(t: &IBig) -> Option<IBig> {
    let (c, s) = cos_sin_pi(t);
    div(&s, &c)
}

/// e^x: the series at x/2^k with |x/2^k| ≤ 1, squared back k times.
#[must_use]
pub fn exp(x: &IBig) -> IBig {
    let mut k = 0usize;
    let mut y = x.clone();
    while abs(&y) > one() {
        y >>= 1;
        k += 1;
    }
    let mut term = one();
    let mut s = IBig::ZERO;
    for n in 0..1000usize {
        s += &term;
        term = mul(&term, &y) / IBig::from(n + 1);
        if term == IBig::ZERO {
            break;
        }
    }
    for _ in 0..k {
        s = mul(&s, &s);
    }
    s
}

/// tanh x = (e^{2x} − 1)/(e^{2x} + 1).
#[must_use]
pub fn tanh(x: &IBig) -> IBig {
    let e = exp(&(x << 1));
    div(&(&e - one()), &(&e + one())).unwrap_or(IBig::ZERO)
}

/// `a` as the exact rational it stands for, a / 2^400.
#[must_use]
pub fn rational(a: &IBig) -> RBig {
    RBig::from_parts(a.clone(), dashu_int::UBig::ONE << F)
}

/// `q`, within 2^−400.
#[must_use]
pub fn of_rational(q: &RBig) -> IBig {
    (q.numerator() << F) / IBig::from(q.denominator().clone())
}

/// Whether |a − b| ≤ r, decided in the fixed point (the squares are cut, so a distance within
/// 2^−390 of r may fall either side); false if the dimensions differ.
#[must_use]
pub fn within(a: &[IBig], b: &[IBig], r: &IBig) -> bool {
    let mut s = IBig::ZERO;
    for (x, y) in a.iter().zip(b) {
        let d = x - y;
        s += mul(&d, &d);
    }
    a.len() == b.len() && s <= mul(r, r)
}

/// |a − b|, for messages: the root of the summed squares, as a double (rounded).
#[must_use]
pub fn distance(a: &[IBig], b: &[IBig]) -> f64 {
    let mut s = IBig::ZERO;
    for (x, y) in a.iter().zip(b) {
        let d = x - y;
        s += mul(&d, &d);
    }
    sqrt(&s).map_or(f64::NAN, |r| rational(&r).to_f64().value())
}
