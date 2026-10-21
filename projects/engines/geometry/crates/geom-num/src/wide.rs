//! Fixed-width signed integers for exact predicates.
//!
//! The deepest exact predicates need more than 128 bits: an in-circle test about 168 bits, an
//! in-sphere test about 212, a point made by three planes tested against a fourth plane about 379
//! (chapter 6.2). These types hold such values in a fixed number of 64-bit limbs, on the stack, with
//! no allocation, so they are fast, deterministic and usable in `no_std`.
//!
//! Representation: two's complement, limbs little-endian (limb 0 is the least significant).
//! A `Wide<L>` holds values from −2^(64L−1) to 2^(64L−1) − 1.
//!
//! Every arithmetic operation is checked: it returns `None` when the true result does not fit, and
//! never wraps silently. The kernel proves that its predicates never overflow; these checks turn a
//! wrong proof into a detectable failure instead of a wrong sign.
//!
//! Method: schoolbook addition, subtraction and multiplication on magnitudes with 128-bit
//! intermediates (Knuth, *The Art of Computer Programming*, volume 2, section 4.3.1). There is no
//! division: predicates never need it.

use core::cmp::Ordering;
use core::fmt;

use crate::sign::Sign;

/// A signed integer of `64 × L` bits, two's complement, little-endian limbs.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Wide<const L: usize>([u64; L]);

/// 256-bit signed integer: in-circle and in-sphere tests, three-plane denominators.
pub type I256 = Wide<4>;
/// 320-bit signed integer: three-plane point numerators.
pub type I320 = Wide<5>;
/// 384-bit signed integer: classifying a three-plane point against a plane.
pub type I384 = Wide<6>;
/// 512-bit signed integer: headroom for tests and later predicates.
pub type I512 = Wide<8>;

impl<const L: usize> Wide<L> {
    /// Evaluated when a width is used: a `Wide` needs at least two limbs to hold an `i128`.
    const AT_LEAST_TWO_LIMBS: () = assert!(L >= 2, "Wide<L> needs L >= 2");

    /// Zero.
    pub const ZERO: Self = Self([0; L]);

    /// The limb that sign-extends a value with this sign.
    const fn fill(negative: bool) -> u64 {
        if negative { u64::MAX } else { 0 }
    }

    /// The value one.
    #[must_use]
    pub fn one() -> Self {
        Self::from_i128(1)
    }

    /// The largest value, 2^(64L−1) − 1.
    #[must_use]
    pub fn max_value() -> Self {
        let () = Self::AT_LEAST_TWO_LIMBS;
        let mut limbs = [u64::MAX; L];
        if let Some(top) = limbs.last_mut() {
            *top = u64::MAX >> 1;
        }
        Self(limbs)
    }

    /// The smallest value, −2^(64L−1).
    #[must_use]
    pub fn min_value() -> Self {
        let () = Self::AT_LEAST_TWO_LIMBS;
        let mut limbs = [0u64; L];
        if let Some(top) = limbs.last_mut() {
            *top = 1 << 63;
        }
        Self(limbs)
    }

    /// From an `i128`, exactly.
    #[must_use]
    pub fn from_i128(v: i128) -> Self {
        let () = Self::AT_LEAST_TWO_LIMBS;
        let mut limbs = [Self::fill(v < 0); L];
        // Truncation to the low and high 64 bits is intended: together they are v's two's complement.
        let parts = [v as u64, (v >> 64) as u64];
        for (limb, part) in limbs.iter_mut().zip(parts) {
            *limb = part;
        }
        Self(limbs)
    }

    /// From an `i64`, exactly.
    #[must_use]
    pub fn from_i64(v: i64) -> Self {
        Self::from_i128(i128::from(v))
    }

    /// From raw two's-complement limbs, least significant first.
    #[must_use]
    pub const fn from_le_limbs(limbs: [u64; L]) -> Self {
        Self(limbs)
    }

    /// The raw two's-complement limbs, least significant first.
    #[must_use]
    pub const fn to_le_limbs(self) -> [u64; L] {
        self.0
    }

    /// Whether the value is below zero.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.0.last().is_some_and(|top| top >> 63 == 1)
    }

    /// Whether the value is exactly zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&limb| limb == 0)
    }

    /// The sign of the value.
    #[must_use]
    pub fn signum(&self) -> Sign {
        if self.is_negative() {
            Sign::Negative
        } else if self.is_zero() {
            Sign::Zero
        } else {
            Sign::Positive
        }
    }

    /// Limb-wise addition with carry; the carry out of the top limb is dropped (two's complement wrap).
    fn wrapping_add_limbs(a: &[u64; L], b: &[u64; L]) -> [u64; L] {
        let mut out = [0u64; L];
        let mut carry = 0u128;
        for ((o, &x), &y) in out.iter_mut().zip(a.iter()).zip(b.iter()) {
            let s = u128::from(x) + u128::from(y) + carry;
            *o = s as u64;
            carry = s >> 64;
        }
        out
    }

    /// Limb-wise subtraction with borrow; the borrow out of the top limb is dropped.
    fn wrapping_sub_limbs(a: &[u64; L], b: &[u64; L]) -> [u64; L] {
        let mut out = [0u64; L];
        let mut borrow = false;
        for ((o, &x), &y) in out.iter_mut().zip(a.iter()).zip(b.iter()) {
            let (d1, b1) = x.overflowing_sub(y);
            let (d2, b2) = d1.overflowing_sub(u64::from(borrow));
            *o = d2;
            borrow = b1 || b2;
        }
        out
    }

    /// Two's-complement negation, wrapping at the minimum value.
    fn wrapping_neg(&self) -> Self {
        let mut out = [0u64; L];
        let mut carry = 1u128;
        for (o, &x) in out.iter_mut().zip(self.0.iter()) {
            let s = u128::from(!x) + carry;
            *o = s as u64;
            carry = s >> 64;
        }
        Self(out)
    }

    /// The magnitude as an unsigned `64 × L`-bit number (the minimum value's magnitude, 2^(64L−1),
    /// fits as unsigned).
    fn magnitude(&self) -> [u64; L] {
        if self.is_negative() {
            self.wrapping_neg().0
        } else {
            self.0
        }
    }

    /// `self + other`, or `None` if the true sum does not fit.
    #[must_use]
    pub fn checked_add(&self, other: &Self) -> Option<Self> {
        let r = Self(Self::wrapping_add_limbs(&self.0, &other.0));
        // Overflow happens exactly when both operands share a sign and the result does not.
        let overflow =
            self.is_negative() == other.is_negative() && r.is_negative() != self.is_negative();
        (!overflow).then_some(r)
    }

    /// `self − other`, or `None` if the true difference does not fit.
    #[must_use]
    pub fn checked_sub(&self, other: &Self) -> Option<Self> {
        let r = Self(Self::wrapping_sub_limbs(&self.0, &other.0));
        // Overflow happens exactly when the operands differ in sign and the result's sign is not self's.
        let overflow =
            self.is_negative() != other.is_negative() && r.is_negative() != self.is_negative();
        (!overflow).then_some(r)
    }

    /// `−self`, or `None` for the minimum value, whose negation does not fit.
    #[must_use]
    pub fn checked_neg(&self) -> Option<Self> {
        let r = self.wrapping_neg();
        // Only the minimum value is negative and stays negative when negated.
        let overflow = self.is_negative() && r.is_negative();
        (!overflow).then_some(r)
    }

    /// `self × other`, or `None` if the true product does not fit.
    #[must_use]
    pub fn checked_mul(&self, other: &Self) -> Option<Self> {
        let negative = self.is_negative() != other.is_negative();
        let a = self.magnitude();
        let b = other.magnitude();
        let mut out = [0u64; L];
        for (i, &ai) in a.iter().enumerate() {
            if ai == 0 {
                continue;
            }
            let mut carry = 0u128;
            for (j, &bj) in b.iter().enumerate() {
                let prod = u128::from(ai) * u128::from(bj);
                match out.get_mut(i + j) {
                    Some(slot) => {
                        // slot < 2^64, prod <= (2^64 − 1)^2, carry < 2^64: the sum is below 2^128.
                        let t = u128::from(*slot) + prod + carry;
                        *slot = t as u64;
                        carry = t >> 64;
                    }
                    // Any non-zero contribution at or above limb L makes the magnitude at least 2^(64L).
                    None => {
                        if prod != 0 || carry != 0 {
                            return None;
                        }
                    }
                }
            }
            if carry != 0 {
                return None;
            }
        }
        // The magnitude fits in 64 × L unsigned bits. It must also fit the signed range: below 2^(64L−1)
        // for a positive result, at most 2^(64L−1) for a negative one.
        let top_bit_set = out.last().is_some_and(|top| top >> 63 == 1);
        if !top_bit_set {
            let m = Self(out);
            return Some(if negative { m.wrapping_neg() } else { m });
        }
        // Top bit set: only the exact magnitude 2^(64L−1) with a negative sign fits (the minimum value).
        let is_power = out
            .iter()
            .rev()
            .enumerate()
            .all(|(k, &limb)| if k == 0 { limb == 1 << 63 } else { limb == 0 });
        if negative && is_power {
            Some(Self::min_value())
        } else {
            None
        }
    }

    /// The same value in a `Wide<M>`, or `None` if it does not fit.
    #[must_use]
    pub fn resize<const M: usize>(&self) -> Option<Wide<M>> {
        let () = Wide::<M>::AT_LEAST_TWO_LIMBS;
        let fill = Self::fill(self.is_negative());
        let mut out = [fill; M];
        for (o, &x) in out.iter_mut().zip(self.0.iter()) {
            *o = x;
        }
        let r = Wide::<M>(out);
        if M < L {
            // Narrowing: the dropped limbs must be pure sign extension, and the sign must survive.
            let dropped_ok = self.0.iter().skip(M).all(|&x| x == fill);
            if !dropped_ok || r.is_negative() != self.is_negative() {
                return None;
            }
        }
        Some(r)
    }

    /// The value as an `i128`, if it fits.
    #[must_use]
    pub fn to_i128(&self) -> Option<i128> {
        let fill = Self::fill(self.is_negative());
        if !self.0.iter().skip(2).all(|&x| x == fill) {
            return None;
        }
        // Start from the sign fill, so a value with fewer than two limbs is sign-extended, not zero-extended.
        let mut parts = [fill; 2];
        for (p, &x) in parts.iter_mut().zip(self.0.iter()) {
            *p = x;
        }
        let [lo, hi] = parts;
        let v = (i128::from(hi as i64) << 64) | i128::from(lo);
        // The sign of the 128-bit value must match.
        (Sign::of_i128(v) == self.signum()).then_some(v)
    }
}

impl<const L: usize> Ord for Wide<L> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.is_negative(), other.is_negative()) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            // Same sign: two's-complement values order like their unsigned bit patterns.
            _ => {
                for (x, y) in self.0.iter().rev().zip(other.0.iter().rev()) {
                    match x.cmp(y) {
                        Ordering::Equal => {}
                        ord => return ord,
                    }
                }
                Ordering::Equal
            }
        }
    }
}

impl<const L: usize> PartialOrd for Wide<L> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const L: usize> fmt::Debug for Wide<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Wide<{L}>(0x")?;
        for limb in self.0.iter().rev() {
            write!(f, "{limb:016x}")?;
        }
        f.write_str(")")
    }
}

#[cfg(test)]
mod tests {
    use super::{I256, I384, Wide};
    use crate::sign::Sign;

    #[test]
    fn from_i128_round_trips() {
        for v in [
            0i128,
            1,
            -1,
            i128::MAX,
            i128::MIN,
            1 << 100,
            -(1 << 100) + 7,
        ] {
            assert_eq!(I256::from_i128(v).to_i128(), Some(v));
            assert_eq!(I256::from_i128(v).signum(), Sign::of_i128(v));
        }
    }

    #[test]
    fn overflow_at_the_edges() {
        let max = I256::max_value();
        let min = I256::min_value();
        assert_eq!(max.checked_add(&I256::one()), None);
        assert_eq!(min.checked_sub(&I256::one()), None);
        assert_eq!(min.checked_neg(), None);
        assert_eq!(min.checked_mul(&I256::from_i64(-1)), None);
        assert_eq!(
            max.checked_mul(&I256::from_i64(-1))
                .and_then(|v| v.checked_sub(&I256::one())),
            Some(min)
        );
        assert!(min < max);
        // 2^127 × 2^127 = 2^254 fits; 2^127 × 2^128 = 2^255 does not (positive).
        let p127 = I256::from_le_limbs([0, 1 << 63, 0, 0]);
        let p128 = I256::from_le_limbs([0, 0, 1, 0]);
        assert!(p127.checked_mul(&p127).is_some());
        assert_eq!(p127.checked_mul(&p128), None);
        // −2^127 × 2^128 = −2^255 is exactly the minimum.
        assert_eq!(
            p127.checked_neg().and_then(|n| n.checked_mul(&p128)),
            Some(min)
        );
    }

    #[test]
    fn resize_widens_and_narrows() {
        let v = I256::from_i128(-123_456_789);
        let w: Option<I384> = v.resize();
        assert_eq!(w.and_then(|w| w.resize::<4>()), Some(v));
        let big = I384::max_value();
        assert_eq!(big.resize::<4>(), None);
        assert_eq!(Wide::<6>::min_value().resize::<4>(), None);
    }
}
