//! Exact helpers shared by the crate: doubles as exact dyadic numbers, exact scaling of doubles by
//! powers of two, wide integers built from shifted products, exact rounding of a wide integer
//! divided by a power of two, the exact sign of a − b·√n, the correctly rounded double nearest a
//! quotient of integers, and long division of a wide integer by a 128-bit one.
//!
//! Method:
//!
//! - A double is exactly m·2^k with integer m (IEEE 754-2019, section 3.3); scaling by a power of
//!   two is IEEE's scaleB (section 5.3.3), rounded once.
//! - The quotient of two integers is rounded to the nearest double, ties to even (roundTiesToEven,
//!   section 4.3.1), by binary long division that produces the 53 bits of the significand, one
//!   more bit, and a sticky bit for whether anything nonzero remains (D. E. Knuth, *The Art of
//!   Computer Programming*, vol. 2, section 4.3.1); IEEE 754-2019,
//!   <https://doi.org/10.1109/IEEESTD.2019.8766229>.
//! - The sign of a − b·√n is decided by comparing a² with b²·n when a and b have the same sign:
//!   exact geometric computation with algebraic numbers of degree two, evaluated in exact integers
//!   instead of rounded square roots (C. K. Yap, "Towards exact geometric computation",
//!   *Computational Geometry* 7 (1997), 3–23, <https://doi.org/10.1016/0925-7721(95)00040-2>).
//! - Greatest common divisors by Euclid's algorithm, and long division of a wide integer one bit at
//!   a time (Knuth, *The Art of Computer Programming*, vol. 2, sections 4.5.2 and 4.3.1,
//!   <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>).
//!
//! Guarantees:
//!
//! - **Exact, or it says so.** Every integer result is exact, and one that does not fit its type
//!   is `None`, never a wrapped value. Every rounded result is rounded once, to nearest, ties to
//!   even: [`round_scaled`] and [`rational_to_f64`] also say whether they were exact, and
//!   [`scale_b`], like IEEE 754's scaleB, returns ±∞ when the exact result overflows.
//! - **The same result in every build profile.** Every integer addition, subtraction,
//!   multiplication, division by a variable and negation is a checked method that returns `None`
//!   instead of wrapping, or `abs_diff`, which is exact for every input; the only native integer
//!   operations are shifts by amounts below the width and division and remainder by the constant
//!   64, none of which can overflow. So nothing depends on overflow checks being on. A `None` that
//!   the bounds written here rule out is passed on, and the caller reports it as
//!   [`NumError::BudgetExceeded`](geom_num::NumError), never as an answer.
//! - **No panics.** Every shift amount is below the width it shifts (each says why), every limb
//!   access is checked, and every loop has a fixed or proven bound (below). The only casts are of
//!   values whose bound is written beside them.
//!
//! Cost: no allocation. [`pow2`], [`exponent`], [`dyadic`] and [`scale_b`] are a few bit operations
//! and at most two floating-point multiplications. [`pow2_big`], [`shl_into`] and [`round_scaled`]
//! are linear in the number of 64-bit limbs (20 for [`Big`], at most 40 anywhere in the crate).
//! [`sign_minus_sqrt`] is three schoolbook multiplications of L-limb integers (at most L² limb
//! products each) and one subtraction. [`rational_to_f64`] is one 128-bit division and at most 181
//! shift-and-subtract steps. [`gcd_u128`] takes at most 257 remainder steps (the remainder at least
//! halves every two steps). [`divrem_u128`] takes exactly 64·L shift-and-subtract steps.

use geom_num::sign::Sign;
use geom_num::wide::Wide;

use crate::rounded::Rounded;

/// The wide integer of the exact frame decisions: 1280 bits, range ±2^1279. The largest value they
/// form is below 2^1234 (see `frame`), so nothing overflows.
pub(crate) type Big = Wide<20>;

/// The fraction field of a double's bits: the low 52 bits.
const FRACTION_BITS: u64 = (1 << 52) - 1;

/// 2^−1022, the smallest normal power of two: exponent field 1 and nothing else.
const TWO_POW_NEG_1022: f64 = f64::from_bits(1 << 52);

/// 2^k as a double for −1022 ≤ k ≤ 1023 (the normal powers of two), from its bits; `None` for any
/// other k. Every module of the crate uses this one definition.
///
/// The exponent field is k + 1023, from 1 to 2046, computed as the distance of k from −1023
/// (`abs_diff`, exact for every pair of `i32`s, and k ≥ −1023 here); a field below 2^11 shifted
/// left by 52 places stays inside 64 bits.
pub(crate) const fn pow2(k: i32) -> Option<f64> {
    if k < -1022 || k > 1023 {
        return None;
    }
    let field = k.abs_diff(-1023) as u64;
    Some(f64::from_bits(field << 52))
}

/// 2^k for a constant exponent k, for `const` items only: evaluated when the crate compiles, where
/// an exponent outside the normal range stops the build, so no constant can hold a wrong value.
macro_rules! const_pow2 {
    ($k:expr) => {
        match $crate::exact::pow2($k) {
            Some(v) => v,
            None => panic!("not a normal power of two"),
        }
    };
}
pub(crate) use const_pow2;

/// The exponent field of a double's bits: 0 for zero and the subnormals, 1 to 2046 for the normal
/// numbers, 2047 for the infinities and NaN.
fn exponent_field(x: f64) -> i32 {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        reason = "the field is masked to 11 bits, at most 2047, which i32 holds exactly"
    )]
    let field = ((x.to_bits() >> 52) & 0x7ff) as i32;
    field
}

/// The binary exponent e of a finite nonzero double, subnormals included: 2^e ≤ |x| < 2^(e + 1),
/// so e lies between −1074 and 1023. `None` for zero, the infinities and NaN, which have none.
pub(crate) fn exponent(x: f64) -> Option<i32> {
    if x == 0.0 || !x.is_finite() {
        return None;
    }
    let field = exponent_field(x);
    if field != 0 {
        // A normal x: e = field − 1023, from −1022 to 1023.
        return field.checked_sub(1023);
    }
    // A subnormal x is f·2^−1074 with 0 < f < 2^52, so lz(f) is 12 to 63 and the leading bit is
    // bit 63 − lz(f), from 0 to 51.
    let lead = 63u32.checked_sub((x.to_bits() & FRACTION_BITS).leading_zeros())?;
    i32::try_from(lead).ok()?.checked_sub(1074)
}

/// A finite double as an exact dyadic number m·2^k, with m odd (or m = 0 and k = 0) and
/// |m| < 2^53. Subnormal doubles are included (k down to −1074). `None` for the infinities and NaN,
/// which are not dyadic numbers.
///
/// Every step fits: the fraction is below 2^52, the significand with its hidden bit below 2^53,
/// k + tz at most 1075 + 52 in magnitude, and the negation of an |m| below 2^53 fits `i64`.
pub(crate) fn dyadic(x: f64) -> Option<(i64, i32)> {
    if !x.is_finite() {
        return None;
    }
    let bits = x.to_bits();
    let field = exponent_field(x);
    let fraction = i64::try_from(bits & FRACTION_BITS).ok()?;
    let (m, k) = if field == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1i64 << 52), field.checked_sub(1075)?)
    };
    if m == 0 {
        return Some((0, 0));
    }
    // m ≠ 0 and |m| < 2^53, so tz ≤ 52: the shift is below 64.
    let tz = m.trailing_zeros();
    let m = m.checked_shr(tz)?;
    let m = if bits >> 63 == 1 { m.checked_neg()? } else { m };
    Some((m, k.checked_add(i32::try_from(tz).ok()?)?))
}

/// x·2^k rounded once to nearest, ties to even (IEEE 754's scaleB): exact whenever the result is a
/// normal double, ±∞ when the exact result overflows, and a single correct rounding when it falls
/// below 2^−1022. Zero, infinities and NaN are returned unchanged. `None` only if a step that the
/// bounds below rule out failed.
///
/// With x = m·2^q exactly (|m| < 2^53, so m is a double), x·2^k = m·2^t with t = q + k:
///
/// - t > 1023: |m·2^t| ≥ 2^1024, which overflows.
/// - −1022 ≤ t ≤ 1023: 2^t is a normal double and m·2^t has at most 53 significant bits, so the
///   one IEEE multiplication is exact, or overflows to ±∞ exactly when the result is beyond the
///   largest double.
/// - t < −1022: y = m·2^−1022 is exact and normal (|m| ≥ 1, |m| < 2^53); the remaining factor
///   2^(t+1022) is a normal double when t + 1022 ≥ −1022, so y·2^(t+1022) is one IEEE
///   multiplication of exact values, correctly rounded; otherwise |x·2^k| < 2^−969 · 2^−1022, far
///   below half the smallest subnormal, and the result is zero with x's sign.
///
/// t is q + k saturated at the ends of `i32`: a saturated t is above 1023, or below −2044, exactly
/// when the exact t is, and gives the same result (±∞, or zero with x's sign), so saturating
/// changes nothing. Then t + 1022 is formed only for t < −1022, where it cannot overflow.
pub(crate) fn scale_b(x: f64, k: i32) -> Option<f64> {
    if x == 0.0 || !x.is_finite() {
        return Some(x);
    }
    let (m, q) = dyadic(x)?;
    #[allow(
        clippy::cast_precision_loss,
        reason = "|m| < 2^53, so the conversion to f64 is exact"
    )]
    let mf = m as f64;
    let t = q.saturating_add(k);
    if t > 1023 {
        return Some(if m < 0 {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    if t >= -1022 {
        return Some(mf * pow2(t)?);
    }
    let y = mf * TWO_POW_NEG_1022;
    let s = t.checked_add(1022)?;
    if s >= -1022 {
        Some(y * pow2(s)?)
    } else {
        Some(0.0 * mf)
    }
}

/// 2^k as a [`Big`], or `None` if it does not fit (k ≥ 1279).
pub(crate) fn pow2_big(k: u32) -> Option<Big> {
    if k >= 64 * 20 - 1 {
        return None;
    }
    let mut limbs = [0u64; 20];
    let slot = limbs.get_mut(usize::try_from(k / 64).ok()?)?;
    // k % 64 is below 64.
    *slot = 1u64 << (k % 64);
    Some(Big::from_le_limbs(limbs))
}

/// The low and high 64-bit halves of `t`, t = high · 2^64 + low: read from its little-endian
/// bytes, so there is no cast and no shift (as `geom_num::wide` reads them).
fn halves(t: u128) -> (u64, u64) {
    let mut words = [[0u8; 8]; 2];
    for (byte, &b) in words.iter_mut().flatten().zip(t.to_le_bytes().iter()) {
        *byte = b;
    }
    let [low, high] = words.map(u64::from_le_bytes);
    (low, high)
}

/// v·2^k as a `Wide<L>`, exactly, or `None` if it does not fit the signed range.
pub(crate) fn shl_into<const L: usize>(v: i128, k: u32) -> Option<Wide<L>> {
    let (low, high) = halves(v.unsigned_abs());
    let bit = k % 64;
    // The magnitude times 2^bit, spread over three limbs (it is below 2^192). With 1 ≤ bit ≤ 63,
    // both bit and 64 − bit are below 64.
    let parts = if bit == 0 {
        [low, high, 0]
    } else {
        let back = 64u32.checked_sub(bit)?;
        [low << bit, (high << bit) | (low >> back), high >> back]
    };
    let word = usize::try_from(k / 64).ok()?;
    let mut limbs = [0u64; L];
    for (i, part) in parts.into_iter().enumerate() {
        if part != 0 {
            *limbs.get_mut(word.checked_add(i)?)? = part;
        }
    }
    // A magnitude with the top bit set does not fit the signed range.
    if limbs.last().is_some_and(|top| top >> 63 == 1) {
        return None;
    }
    let w = Wide::from_le_limbs(limbs);
    if v < 0 { w.checked_neg() } else { Some(w) }
}

/// The limbs of `a` shifted right by `k` bits (zeros shifted in).
///
/// A word offset too large for `usize` lies beyond every limb, so every limb it would read is
/// zero: `usize::MAX` stands for it, and every read past the end is zero.
fn shr_limbs<const L: usize>(a: &[u64; L], k: u32) -> [u64; L] {
    let word = usize::try_from(k / 64).unwrap_or(usize::MAX);
    let bit = k % 64;
    // For 1 ≤ bit ≤ 63, 64 − bit (`abs_diff`, exact) is from 1 to 63: both shifts are below 64.
    let back = bit.abs_diff(64);
    let mut out = [0u64; L];
    for (i, o) in out.iter_mut().enumerate() {
        let at = |j: usize| {
            j.checked_add(word)
                .and_then(|j| a.get(j))
                .copied()
                .unwrap_or(0)
        };
        let (lo, hi) = (at(i), i.checked_add(1).map_or(0, at));
        *o = if bit == 0 {
            lo
        } else {
            (lo >> bit) | (hi << back)
        };
    }
    out
}

/// Whether any of the lowest `k` bits of `a` is set.
fn any_low_bits<const L: usize>(a: &[u64; L], k: u32) -> bool {
    let full = usize::try_from(k / 64).unwrap_or(usize::MAX);
    let partial = k % 64;
    let below = a.iter().take(full).any(|&limb| limb != 0);
    // The lowest `partial` bits (1 ≤ partial ≤ 63, so the shift is below 64): all ones with every
    // bit from `partial` up cleared.
    let mask = !(u64::MAX << partial);
    let part = partial != 0 && a.get(full).is_some_and(|&limb| limb & mask != 0);
    below || part
}

/// w / 2^k rounded to the nearest integer, ties to even, with whether it was exact; `None` if the
/// result does not fit an `i128`.
///
/// On the magnitude: the quotient q is the magnitude shifted right by k, the round bit is bit
/// k − 1 and the sticky bit says whether any lower bit is set. q rounds up exactly when the round
/// bit is set and either the sticky bit is set or q is odd. The sign is applied afterwards, which
/// is right because rounding to nearest, ties to even, is symmetric.
pub(crate) fn round_scaled<const L: usize>(w: &Wide<L>, k: u32) -> Option<Rounded<i128>> {
    let negative = w.is_negative();
    let mag = if negative { w.checked_neg()? } else { *w }.to_le_limbs();
    let shifted = shr_limbs(&mag, k);
    if shifted.iter().skip(2).any(|&limb| limb != 0) {
        return None;
    }
    let [q0, q1] = [0, 1].map(|i| shifted.get(i).copied().unwrap_or(0));
    let q = u128::from(q0) | (u128::from(q1) << 64);
    // No round or sticky bit when k = 0; otherwise the round bit is bit r = k − 1.
    let (round, sticky) = match k.checked_sub(1) {
        None => (false, false),
        Some(r) => {
            let limb = mag.get(usize::try_from(r / 64).ok()?).copied().unwrap_or(0);
            ((limb >> (r % 64)) & 1 == 1, any_low_bits(&mag, r))
        }
    };
    let q = if round && (sticky || q & 1 == 1) {
        q.checked_add(1)?
    } else {
        q
    };
    let value = i128::try_from(q).ok()?;
    let value = if negative {
        value.checked_neg()?
    } else {
        value
    };
    Some(Rounded::new(value, !round && !sticky))
}

/// The sign of a − b·√n for integers a, b and n ≥ 1 (n need not be a square), exactly; `None` if a
/// square does not fit `Wide<L>`.
///
/// If a and b are not both positive or both negative the sign is read off directly: a ≥ 0 ≥ b gives
/// a − b√n ≥ 0, zero only when both are zero, and a ≤ 0 ≤ b gives the opposite. When a, b > 0,
/// a − b√n has the sign of a² − b²n (both sides of a > b√n are positive); when a, b < 0 it has the
/// sign of b²n − a².
pub(crate) fn sign_minus_sqrt<const L: usize>(a: &Wide<L>, b: &Wide<L>, n: u64) -> Option<Sign> {
    let nn = Wide::<L>::from_i128(i128::from(n));
    let squares = || -> Option<(Wide<L>, Wide<L>)> {
        Some((a.checked_mul(a)?, b.checked_mul(b)?.checked_mul(&nn)?))
    };
    match (a.signum(), b.signum()) {
        (Sign::Zero, Sign::Zero) => Some(Sign::Zero),
        (Sign::Positive | Sign::Zero, Sign::Negative | Sign::Zero) => Some(Sign::Positive),
        (Sign::Negative | Sign::Zero, Sign::Positive | Sign::Zero) => Some(Sign::Negative),
        (Sign::Positive, Sign::Positive) => {
            let (a2, b2n) = squares()?;
            Some(a2.checked_sub(&b2n)?.signum())
        }
        (Sign::Negative, Sign::Negative) => {
            let (a2, b2n) = squares()?;
            Some(b2n.checked_sub(&a2)?.signum())
        }
    }
}

/// The double nearest `num / den`, ties to even, and whether it equals the quotient exactly.
/// `None` if `den` is zero or at least 2^127.
///
/// The quotient's magnitude lies between 2^−127 and 2^127 (or is zero), so the result is a normal
/// double and the scaling by a power of two at the end is exact.
///
/// 1. q = n div den and r = n mod den, n = |num|.
/// 2. If q has 54 or more bits, its leading 54 bits form the significand-and-round-bit `mant`;
///    the dropped bits and r make the sticky bit. Otherwise further quotient bits are appended by
///    long division (r ← 2r; a bit is 1 when r ≥ den, which then subtracts den) until `mant` has 54
///    bits; r ≠ 0 at the end is the sticky bit. r < den < 2^127, so 2r never overflows; the leading
///    bit appears within 127 steps and 53 more complete the significand, so 181 steps suffice.
/// 3. Round to nearest, ties to even, on the last bit of `mant`, carrying into the exponent when
///    the significand overflows to 2^53.
pub(crate) fn rational_to_f64(num: i128, den: u128) -> Option<Rounded<f64>> {
    if den == 0 || den >= 1u128 << 127 {
        return None;
    }
    if num == 0 {
        return Some(Rounded::new(0.0, true));
    }
    let n = num.unsigned_abs();
    let q = n.checked_div(den)?;
    let mut r = n.checked_rem(den)?;
    let (mant, exp, sticky) = if q >= 1u128 << 53 {
        // q has 54 to 128 bits, so the shift is from 0 to 74, below 128.
        let shift = 128u32.checked_sub(q.leading_zeros())?.checked_sub(54)?;
        let dropped = q & !(u128::MAX << shift);
        (
            q >> shift,
            i32::try_from(shift).ok()?,
            dropped != 0 || r != 0,
        )
    } else {
        let mut mant = q;
        let mut exp = 0i32;
        for _ in 0..181 {
            if mant >= 1u128 << 53 {
                break;
            }
            // r < den < 2^127 and mant < 2^53: doubling either keeps every bit.
            r <<= 1;
            mant <<= 1;
            if r >= den {
                r = r.checked_sub(den)?;
                mant |= 1;
            }
            exp = exp.checked_sub(1)?;
        }
        (mant, exp, r != 0)
    };
    if mant < 1u128 << 53 {
        // Unreachable by the step count above; refuse rather than return a wrong value.
        return None;
    }
    let round_bit = (mant & 1) == 1;
    let mut m = mant >> 1;
    let mut e = exp.checked_add(1)?;
    if round_bit && (sticky || (m & 1) == 1) {
        m = m.checked_add(1)?;
        if m == 1u128 << 53 {
            m >>= 1;
            e = e.checked_add(1)?;
        }
    }
    // m < 2^53 is a double exactly; e lies between −180 and 75, so 2^e is a normal double and the
    // product is exact.
    #[allow(
        clippy::cast_precision_loss,
        reason = "m < 2^53, so the conversion to f64 is exact"
    )]
    let mf = m as f64;
    let magnitude = mf * pow2(e)?;
    let value = if num < 0 { -magnitude } else { magnitude };
    Some(Rounded::new(value, !round_bit && !sticky))
}

/// The greatest common divisor of two unsigned integers, by Euclid's algorithm (each step strictly
/// decreases `b`); gcd(0, 0) = 0. The remainder is `None` exactly when `b` is zero, which ends the
/// loop, so every remainder taken is of a nonzero divisor.
pub(crate) const fn gcd_u128(mut a: u128, mut b: u128) -> u128 {
    while let Some(t) = a.checked_rem(b) {
        a = b;
        b = t;
    }
    a
}

/// The magnitude of `w` divided by `d` (0 < d < 2^127): the quotient's limbs and the remainder, by
/// binary long division, one bit at a time from the top. The remainder stays below d < 2^127, so
/// doubling it and adding a bit never overflows a `u128`. `None` for the minimum value of
/// `Wide<L>`, whose magnitude does not fit, or a divisor out of range.
pub(crate) fn divrem_u128<const L: usize>(w: &Wide<L>, d: u128) -> Option<(Wide<L>, u128)> {
    if d == 0 || d >= 1u128 << 127 {
        return None;
    }
    let negative = w.is_negative();
    let mag = if negative { w.checked_neg()? } else { *w }.to_le_limbs();
    let mut quotient = [0u64; L];
    let mut r = 0u128;
    for (q, &limb) in quotient.iter_mut().zip(mag.iter()).rev() {
        for b in (0u32..64).rev() {
            r = (r << 1) | u128::from((limb >> b) & 1);
            if r >= d {
                r = r.checked_sub(d)?;
                *q |= 1u64 << b;
            }
        }
    }
    let q = Wide::from_le_limbs(quotient);
    Some((if negative { q.checked_neg()? } else { q }, r))
}
