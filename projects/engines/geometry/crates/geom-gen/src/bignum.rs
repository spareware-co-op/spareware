//! Exact numbers for the predicate generator: natural numbers of any size, and polynomials in the
//! unit roundoff u = 2^-53 with whole coefficients.
//!
//! What it does: [`Nat`] is a natural number of any size (little-endian 64-bit limbs) with exact
//! addition, multiplication, shifts, comparison and a decimal form; the predicate generator's
//! bit-range certificate holds every bound as one. [`UPoly`] is a polynomial in u with
//! non-negative whole coefficients: every bound of the filter's error analysis is one, because
//! the analysis only adds, multiplies, and multiplies by u or by 1 + u. [`UPoly::round_up_to_f64`]
//! gives the smallest `f64` at least the polynomial's exact value at u = 2^-53, as its bits, and
//! [`Nat::nearest_f64`] the `f64` nearest a natural number, as the number it is: the largest a
//! rounded input's `f64` copy can be.
//!
//! Guarantees:
//!
//! - **Exact or nothing.** Every operation is exact. One whose result would not fit (a
//!   coefficient beyond `u128`, a shift beyond `u32` bits, a value beyond the largest `f64`)
//!   returns `None`, never a rounded or wrapped value; nothing panics.
//! - **Canonical.** A [`Nat`] has no zero top limb and a [`UPoly`] no zero top coefficient, so
//!   equal values compare equal and print the same.
//! - **Rounded only on purpose.** [`UPoly::round_up_to_f64`] rounds towards +∞, so the `f64` it
//!   gives is never below the exact value; [`Nat::nearest_f64`] rounds to nearest with ties to
//!   even, exactly as the filters convert a rounded input. Nothing else here rounds.
//!
//! Method: schoolbook addition and multiplication on limbs with 128-bit intermediates (Knuth,
//! *The Art of Computer Programming*, volume 2, section 4.3.1,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>); decimal digits by repeated short
//! division by 10^19 (Knuth 4.4). The `f64` encoding is IEEE 754-2019's binary64
//! (<https://standards.ieee.org/ieee/754/6210/>): a normal value is m · 2^e with
//! 2^52 ≤ m < 2^53, stored as the biased exponent e + 52 + 1023 above the 52 fraction bits of m.
//!
//! Cost: linear in the number of limbs for addition and shifts, quadratic for multiplication;
//! the generator's numbers have at most a few dozen limbs.

use core::cmp::Ordering;
use core::fmt::{self, Write as _};

/// A natural number of any size: little-endian 64-bit limbs, without zero limbs at the top.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Nat {
    limbs: Vec<u64>,
}

/// The low and high 64-bit halves of `t`: read from its little-endian bytes, with no cast and no
/// shift.
fn halves(t: u128) -> (u64, u64) {
    let mut words = [[0u8; 8]; 2];
    for (byte, &b) in words.iter_mut().flatten().zip(t.to_le_bytes().iter()) {
        *byte = b;
    }
    let [low, high] = words.map(u64::from_le_bytes);
    (low, high)
}

impl Nat {
    /// Zero.
    #[must_use]
    pub fn zero() -> Nat {
        Nat { limbs: Vec::new() }
    }

    /// The number `v`.
    #[must_use]
    pub fn from_u128(v: u128) -> Nat {
        let (low, high) = halves(v);
        Nat::from_limbs(vec![low, high])
    }

    /// From little-endian limbs, with zero top limbs removed.
    fn from_limbs(mut limbs: Vec<u64>) -> Nat {
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        Nat { limbs }
    }

    /// 2^`n`, or `None` if `n` is beyond what a `Vec` of limbs can index.
    #[must_use]
    pub fn pow2(n: u32) -> Option<Nat> {
        Nat::from_u128(1).shl(n)
    }

    /// 2^`bits` − 1: `bits` one bits.
    #[must_use]
    pub fn ones(bits: u32) -> Option<Nat> {
        let whole = usize::try_from(bits.checked_div(64)?).ok()?;
        let part = bits.checked_rem(64)?;
        let mut limbs = vec![u64::MAX; whole];
        limbs.push(1u64.checked_shl(part)?.checked_sub(1)?);
        Some(Nat::from_limbs(limbs))
    }

    /// Whether it is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    /// The number of bits: the least b with self < 2^b (0 for zero).
    #[must_use]
    pub fn bit_len(&self) -> Option<u32> {
        let Some(&top) = self.limbs.last() else {
            return Some(0);
        };
        let below = u32::try_from(self.limbs.len().checked_sub(1)?).ok()?;
        below
            .checked_mul(64)?
            .checked_add(u64::BITS.checked_sub(top.leading_zeros())?)
    }

    /// The value as a `u64`, if it fits.
    #[must_use]
    pub fn to_u64(&self) -> Option<u64> {
        match self.limbs.as_slice() {
            [] => Some(0),
            [one] => Some(*one),
            _ => None,
        }
    }

    /// self + other.
    #[must_use]
    pub fn add(&self, other: &Nat) -> Option<Nat> {
        let len = self.limbs.len().max(other.limbs.len());
        let mut out = Vec::with_capacity(len.checked_add(1)?);
        let mut carry = false;
        let mut left = self.limbs.iter();
        let mut right = other.limbs.iter();
        for _ in 0..len {
            let x = left.next().copied().unwrap_or(0);
            let y = right.next().copied().unwrap_or(0);
            let (s1, c1) = x.overflowing_add(y);
            let (s2, c2) = s1.overflowing_add(u64::from(carry));
            out.push(s2);
            // The two additions cannot both carry: if x + y carries, its low 64 bits are at most
            // 2^64 − 2, and adding one more does not carry again.
            carry = c1 || c2;
        }
        out.push(u64::from(carry));
        Some(Nat::from_limbs(out))
    }

    /// self × other.
    #[must_use]
    pub fn mul(&self, other: &Nat) -> Option<Nat> {
        let len = self.limbs.len().checked_add(other.limbs.len())?;
        let mut out = vec![0u64; len];
        for (i, &x) in self.limbs.iter().enumerate() {
            let mut carry = 0u64;
            let row = out.get_mut(i..)?;
            let mut slots = row.iter_mut();
            for &y in &other.limbs {
                let slot = slots.next()?;
                // At most (2^64 − 1)² + 2 (2^64 − 1) = 2^128 − 1: never fails.
                let t = u128::from(x)
                    .checked_mul(u128::from(y))?
                    .checked_add(u128::from(*slot))?
                    .checked_add(u128::from(carry))?;
                let (low, high) = halves(t);
                *slot = low;
                carry = high;
            }
            // The slot above this row's last product holds nothing yet from this row.
            let slot = slots.next()?;
            *slot = carry;
        }
        Some(Nat::from_limbs(out))
    }

    /// self × 2^`bits`.
    #[must_use]
    pub fn shl(&self, bits: u32) -> Option<Nat> {
        if self.is_zero() {
            return Some(Nat::zero());
        }
        let whole = usize::try_from(bits.checked_div(64)?).ok()?;
        let part = bits.checked_rem(64)?;
        let mut out = vec![0u64; whole];
        let mut carry = 0u64;
        for &x in &self.limbs {
            if part == 0 {
                out.push(x);
            } else {
                out.push(x.checked_shl(part)? | carry);
                carry = x.checked_shr(64u32.checked_sub(part)?)?;
            }
        }
        out.push(carry);
        Some(Nat::from_limbs(out))
    }

    /// The `f64` nearest to self, ties to even, as the number it is: IEEE 754's rounding to
    /// nearest, which Rust's integer-to-`f64` conversion performs. `None` if the nearest is 2^1024
    /// or more, beyond every finite `f64`.
    #[must_use]
    pub fn nearest_f64(&self) -> Option<Nat> {
        let bits = self.bit_len()?;
        if bits <= 53 {
            return Some(self.clone());
        }
        // The top 54 bits: the 53 an f64 keeps and the one below them; and whether any lower bit
        // is set.
        let below = bits.checked_sub(54)?;
        let (top, sticky) = self.shr(below)?;
        let top = top.to_u64()?;
        let kept = top.checked_shr(1)?;
        let up = top & 1 == 1 && (sticky || kept & 1 == 1);
        let kept = if up { kept.checked_add(1)? } else { kept };
        let nearest = Nat::from_u128(u128::from(kept)).shl(below.checked_add(1)?)?;
        (nearest.bit_len()? <= 1024).then_some(nearest)
    }

    /// ⌊self / 2^`bits`⌋, and whether any bit shifted out was set.
    #[must_use]
    pub fn shr(&self, bits: u32) -> Option<(Nat, bool)> {
        let whole = usize::try_from(bits.checked_div(64)?).ok()?;
        let part = bits.checked_rem(64)?;
        let mut lost = self.limbs.iter().take(whole).any(|&x| x != 0);
        let kept: Vec<u64> = self.limbs.iter().skip(whole).copied().collect();
        if part == 0 {
            return Some((Nat::from_limbs(kept), lost));
        }
        let mask = 1u64.checked_shl(part)?.checked_sub(1)?;
        let mut out = Vec::with_capacity(kept.len());
        let mut upper = kept.iter().skip(1);
        for &x in &kept {
            let next = upper.next().copied().unwrap_or(0);
            out.push(x.checked_shr(part)? | next.checked_shl(64u32.checked_sub(part)?)?);
        }
        if let Some(&first) = kept.first() {
            lost = lost || first & mask != 0;
        }
        Some((Nat::from_limbs(out), lost))
    }

    /// The quotient and remainder of self / `d`, for `d` ≥ 1.
    #[must_use]
    pub fn divmod_small(&self, d: u64) -> Option<(Nat, u64)> {
        if d == 0 {
            return None;
        }
        let mut out = vec![0u64; self.limbs.len()];
        let mut rem = 0u64;
        for (o, &x) in out.iter_mut().zip(self.limbs.iter()).rev() {
            // rem < d, so (rem · 2^64 + x) / d < 2^64: the quotient digit fits a limb.
            let t = u128::from(rem)
                .checked_mul(1u128.checked_shl(64)?)?
                .checked_add(u128::from(x))?;
            let q = t.checked_div(u128::from(d))?;
            *o = u64::try_from(q).ok()?;
            rem = u64::try_from(t.checked_rem(u128::from(d))?).ok()?;
        }
        Some((Nat::from_limbs(out), rem))
    }

    /// The decimal digits.
    #[must_use]
    pub fn to_decimal(&self) -> Option<String> {
        const CHUNK: u64 = 10_000_000_000_000_000_000;
        if self.is_zero() {
            return Some("0".to_owned());
        }
        let mut chunks = Vec::new();
        let mut n = self.clone();
        while !n.is_zero() {
            let (q, r) = n.divmod_small(CHUNK)?;
            chunks.push(r);
            n = q;
        }
        let mut digits = chunks.pop()?.to_string();
        for c in chunks.iter().rev() {
            write!(digits, "{c:019}").ok()?;
        }
        Some(digits)
    }
}

impl Ord for Nat {
    fn cmp(&self, other: &Nat) -> Ordering {
        self.limbs
            .len()
            .cmp(&other.limbs.len())
            .then_with(|| self.limbs.iter().rev().cmp(other.limbs.iter().rev()))
    }
}

impl PartialOrd for Nat {
    fn partial_cmp(&self, other: &Nat) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The exponent of u = 2^-53, the unit roundoff of binary64 rounding to nearest: `f64`'s 53-bit
/// significand (`f64::MANTISSA_DIGITS`).
pub const U_EXPONENT: u32 = f64::MANTISSA_DIGITS;

/// A polynomial in u = 2^-53 with non-negative whole coefficients: c₀ + c₁u + c₂u² + ….
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct UPoly {
    /// Coefficient i is that of u^i; no zero coefficient at the top.
    coeffs: Vec<u128>,
}

impl UPoly {
    /// The polynomial with these coefficients, lowest power first.
    #[must_use]
    pub fn from_coeffs(mut coeffs: Vec<u128>) -> UPoly {
        while coeffs.last() == Some(&0) {
            coeffs.pop();
        }
        UPoly { coeffs }
    }

    /// 0.
    #[must_use]
    pub fn zero() -> UPoly {
        UPoly { coeffs: Vec::new() }
    }

    /// The constant `c`.
    #[must_use]
    pub fn constant(c: u128) -> UPoly {
        UPoly::from_coeffs(vec![c])
    }

    /// u itself.
    #[must_use]
    pub fn u() -> UPoly {
        UPoly::from_coeffs(vec![0, 1])
    }

    /// The coefficients, lowest power first.
    #[must_use]
    pub fn coeffs(&self) -> &[u128] {
        &self.coeffs
    }

    /// Whether it is the zero polynomial.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.coeffs.is_empty()
    }

    /// self + other.
    #[must_use]
    pub fn add(&self, other: &UPoly) -> Option<UPoly> {
        let len = self.coeffs.len().max(other.coeffs.len());
        let mut out = Vec::with_capacity(len);
        let mut left = self.coeffs.iter();
        let mut right = other.coeffs.iter();
        for _ in 0..len {
            let x = left.next().copied().unwrap_or(0);
            let y = right.next().copied().unwrap_or(0);
            out.push(x.checked_add(y)?);
        }
        Some(UPoly::from_coeffs(out))
    }

    /// self × other.
    #[must_use]
    pub fn mul(&self, other: &UPoly) -> Option<UPoly> {
        if self.is_zero() || other.is_zero() {
            return Some(UPoly::zero());
        }
        let len = self.coeffs.len().checked_add(other.coeffs.len())?;
        let mut out = vec![0u128; len];
        for (i, &x) in self.coeffs.iter().enumerate() {
            let row = out.get_mut(i..)?;
            for (slot, &y) in row.iter_mut().zip(other.coeffs.iter()) {
                *slot = slot.checked_add(x.checked_mul(y)?)?;
            }
        }
        Some(UPoly::from_coeffs(out))
    }

    /// self × u.
    #[must_use]
    pub fn times_u(&self) -> UPoly {
        if self.is_zero() {
            return UPoly::zero();
        }
        let mut out = Vec::with_capacity(self.coeffs.len().saturating_add(1));
        out.push(0);
        out.extend_from_slice(&self.coeffs);
        UPoly::from_coeffs(out)
    }

    /// self × (1 + u).
    #[must_use]
    pub fn times_one_plus_u(&self) -> Option<UPoly> {
        self.add(&self.times_u())
    }

    /// self^`n`.
    #[must_use]
    pub fn pow(&self, n: u32) -> Option<UPoly> {
        let mut out = UPoly::constant(1);
        for _ in 0..n {
            out = out.mul(self)?;
        }
        Some(out)
    }

    /// The sum of the coefficients: the value at u = 1, an upper bound on the value at
    /// u = 2^-53, since every coefficient is non-negative.
    #[must_use]
    pub fn at_one(&self) -> Option<u128> {
        self.coeffs.iter().try_fold(0u128, |s, &c| s.checked_add(c))
    }

    /// The exact value at u = 2^-53 as N / 2^(53·d), with d the degree: (N, 53·d).
    #[must_use]
    pub fn exact_value(&self) -> Option<(Nat, u32)> {
        let degree = u32::try_from(self.coeffs.len().saturating_sub(1)).ok()?;
        let denominator = U_EXPONENT.checked_mul(degree)?;
        let mut n = Nat::zero();
        for (i, &c) in self.coeffs.iter().enumerate() {
            let i = u32::try_from(i).ok()?;
            // c · u^i = c · 2^(53(d − i)) / 2^(53d).
            let shift = U_EXPONENT.checked_mul(degree.checked_sub(i)?)?;
            n = n.add(&Nat::from_u128(c).shl(shift)?)?;
        }
        Some((n, denominator))
    }

    /// The bits of the smallest `f64` at least the exact value at u = 2^-53; `None` if that is
    /// zero, below the smallest normal `f64` or above the largest.
    #[must_use]
    pub fn round_up_to_f64(&self) -> Option<u64> {
        let (n, denominator) = self.exact_value()?;
        round_up_to_f64(&n, denominator)
    }
}

/// The bits of the smallest normal `f64` at least n / 2^`denominator`; `None` for zero or out of
/// the normal range.
#[must_use]
pub fn round_up_to_f64(n: &Nat, denominator: u32) -> Option<u64> {
    const SIGNIFICAND_BITS: u32 = f64::MANTISSA_DIGITS;
    const FRACTION_BITS: u32 = 52;
    const BIAS: i64 = 1_023;
    if n.is_zero() {
        return None;
    }
    let len = n.bit_len()?;
    // n / 2^denominator = m · 2^e with 2^52 ≤ m < 2^53 after rounding m up.
    let (mut m, mut e) = if len > SIGNIFICAND_BITS {
        let drop = len.checked_sub(SIGNIFICAND_BITS)?;
        let (q, lost) = n.shr(drop)?;
        let q = if lost { q.add(&Nat::from_u128(1))? } else { q };
        (q, i64::from(drop))
    } else {
        let lift = SIGNIFICAND_BITS.checked_sub(len)?;
        (n.shl(lift)?, i64::from(lift).checked_neg()?)
    };
    e = e.checked_sub(i64::from(denominator))?;
    // Rounding up may carry to 2^53: then the value is 2^52 · 2^(e + 1).
    if m.bit_len()? > SIGNIFICAND_BITS {
        m = m.shr(1)?.0;
        e = e.checked_add(1)?;
    }
    let m = m.to_u64()?;
    let biased = e.checked_add(i64::from(FRACTION_BITS))?.checked_add(BIAS)?;
    if !(1..=2_046).contains(&biased) {
        return None;
    }
    let biased = u64::try_from(biased).ok()?;
    let fraction = m.checked_sub(1u64.checked_shl(FRACTION_BITS)?)?;
    Some(biased.checked_shl(FRACTION_BITS)? | fraction)
}

impl fmt::Display for UPoly {
    /// `0`, or the non-zero terms lowest power first: `4u + 2u^2`, `1 + u`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return f.write_str("0");
        }
        let mut first = true;
        for (i, &c) in self.coeffs.iter().enumerate() {
            if c == 0 {
                continue;
            }
            if !first {
                f.write_str(" + ")?;
            }
            first = false;
            match (i, c) {
                (0, c) => write!(f, "{c}")?,
                (1, 1) => f.write_str("u")?,
                (1, c) => write!(f, "{c}u")?,
                (i, 1) => write!(f, "u^{i}")?,
                (i, c) => write!(f, "{c}u^{i}")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;

    fn n(v: u128) -> Nat {
        Nat::from_u128(v)
    }

    #[test]
    fn arithmetic_matches_u128() {
        let values = [
            0u128,
            1,
            2,
            3,
            u128::from(u64::MAX),
            1 << 64,
            (1 << 100) + 7,
        ];
        for &a in &values {
            for &b in &values {
                if let Some(s) = a.checked_add(b) {
                    assert_eq!(n(a).add(&n(b)).unwrap(), n(s));
                }
                if let Some(p) = a.checked_mul(b) {
                    assert_eq!(n(a).mul(&n(b)).unwrap(), n(p));
                }
                assert_eq!(n(a).cmp(&n(b)), a.cmp(&b));
            }
            for s in [0u32, 1, 13, 63, 64, 65] {
                if a.leading_zeros() >= s {
                    assert_eq!(n(a).shl(s).unwrap(), n(a << s));
                }
                let (q, lost) = n(a).shr(s).unwrap();
                assert_eq!(q, n(a >> s));
                assert_eq!(lost, a & ((1u128 << s) - 1) != 0);
            }
            assert_eq!(n(a).bit_len().unwrap(), 128 - a.leading_zeros());
            assert_eq!(n(a).to_decimal().unwrap(), a.to_string());
        }
    }

    #[test]
    fn big_values_and_decimals() {
        // (2^64 − 1)^2 = 2^128 − 2^65 + 1.
        let m = n(u128::from(u64::MAX));
        let sq = m.mul(&m).unwrap();
        assert_eq!(sq, n(u128::MAX - (1u128 << 65) + 2));
        let big = Nat::pow2(200).unwrap();
        assert_eq!(big.bit_len(), Some(201));
        assert_eq!(
            big.to_decimal().unwrap(),
            "1606938044258990275541962092341162602522202993782792835301376"
        );
        assert_eq!(big.shr(200).unwrap(), (n(1), false));
    }

    #[test]
    fn polynomials_in_u() {
        let one_plus_u = UPoly::from_coeffs(vec![1, 1]);
        assert_eq!(one_plus_u.to_string(), "1 + u");
        assert_eq!(one_plus_u.pow(2).unwrap().to_string(), "1 + 2u + u^2");
        let p = UPoly::from_coeffs(vec![0, 4, 2]);
        assert_eq!(p.to_string(), "4u + 2u^2");
        assert_eq!(UPoly::zero().to_string(), "0");
        assert_eq!(p.times_u().to_string(), "4u^2 + 2u^3");
        assert_eq!(p.at_one(), Some(6));
    }

    #[test]
    fn rounding_up_to_f64_is_exact_or_the_next_double() {
        // u itself is 2^-53 exactly.
        assert_eq!(
            UPoly::u().round_up_to_f64(),
            Some((1.0f64 / 9_007_199_254_740_992.0).to_bits())
        );
        // 4u: exact.
        assert_eq!(
            UPoly::constant(4).times_u().round_up_to_f64(),
            Some((4.0f64 / 9_007_199_254_740_992.0).to_bits())
        );
        // 4u + 2u^2 lies just above 4u: the next double up.
        let p = UPoly::from_coeffs(vec![0, 4, 2]);
        assert_eq!(
            p.round_up_to_f64(),
            Some((4.0f64 / 9_007_199_254_740_992.0).to_bits() + 1)
        );
        // 3: exact, and 2^53 + 1 rounds up to 2^53 + 2.
        assert_eq!(UPoly::constant(3).round_up_to_f64(), Some(3.0f64.to_bits()));
        let odd = (1u128 << 53) + 1;
        assert_eq!(
            UPoly::constant(odd).round_up_to_f64(),
            Some(9_007_199_254_740_994.0_f64.to_bits())
        );
        // 2^54 − 1 rounds up to 2^54: the carry into the exponent.
        let all_ones = (1u128 << 54) - 1;
        assert_eq!(
            UPoly::constant(all_ones).round_up_to_f64(),
            Some(18_014_398_509_481_984.0_f64.to_bits())
        );
        assert_eq!(UPoly::zero().round_up_to_f64(), None);
    }

    #[test]
    fn nearest_f64_rounds_as_the_conversion_does() {
        // Every value is checked against Rust's conversion, which rounds to nearest, ties to even.
        let cases: [u128; 8] = [
            0,
            1,
            (1 << 53) - 1,
            1 << 53,
            (1 << 53) + 1, // a tie: down to the even 2^53
            (1 << 53) + 3, // a tie: up to the even 2^53 + 4
            (1 << 83) - 1, // a rounded normal's bound: up to 2^83
            (1 << 125) - 1,
        ];
        for v in cases {
            let want = Nat::from_u128(v as f64 as u128);
            assert_eq!(Nat::from_u128(v).nearest_f64(), Some(want), "{v}");
        }
        // u128::MAX rounds to 2^128, which no u128 holds: checked by its bits.
        assert_eq!(
            Nat::from_u128(u128::MAX)
                .nearest_f64()
                .and_then(|n| n.bit_len()),
            Some(129)
        );
        // 2^1024 − 1 rounds to 2^1024, beyond every finite f64.
        assert_eq!(Nat::ones(1024).and_then(|n| n.nearest_f64()), None);
        assert!(Nat::ones(1023).and_then(|n| n.nearest_f64()).is_some());
    }
}
