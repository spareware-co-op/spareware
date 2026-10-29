//! Fixed-width signed integers in 32-bit limbs, for the width-aware limb variant of the exact
//! paths.
//!
//! WebAssembly's 64-bit multiply returns only the low 64 bits, so a 64 × 64 → 128-bit limb
//! product, the step of [`crate::wide::Wide`]'s multiplication, becomes a call to the library
//! routine `__multi3` whose result travels back through memory (chapter 23.3). A product of two
//! 32-bit limbs fits 64 bits, so here each limb product is one native 64-bit multiply. The
//! generator's limb variant (`crate::generated::limbs`, piece G42) holds each step of an exact
//! path in a `Wide32` of exactly as many limbs as its certificate needs, and multiplies operands of
//! different widths directly, so no operand is widened to the result's width before it is
//! multiplied.
//!
//! Representation: two's complement, limbs little-endian (limb 0 is the least significant), as
//! [`crate::wide::Wide`]. A `Wide32<L>` holds values from −2^(32L−1) to 2^(32L−1) − 1, and `L` is
//! at least 2, so every `i64` fits.
//!
//! Guarantees:
//!
//! - **Checked.** Every arithmetic operation and conversion returns `None` when the true result
//!   does not fit, and never wraps silently: a certificate that was wrong shows as `None`, never
//!   as a wrong sign.
//! - **Checked inside too.** A limb sum or difference uses `overflowing_add` or
//!   `overflowing_sub`, which return the full result as the low 32 bits and the carry or borrow;
//!   only the carry out of the top limb is dropped, as two's complement defines, before the sign
//!   test that decides overflow. A limb product uses `checked_mul` and `checked_add` on `u64`,
//!   which cannot fail ((2^32 − 1)² + 2 (2^32 − 1) = 2^64 − 1, written beside each), split into
//!   its 32-bit halves through its bytes, with no cast and no shift. Nothing depends on overflow
//!   checks being on.
//! - **The same values as `Wide`.** Each operation gives the value [`crate::wide::Wide`]'s gives
//!   on the same integers (tested against it and against big integers below and in
//!   `tests/attack_g42.rs`).
//!
//! Method: schoolbook addition, subtraction and multiplication on magnitudes with 64-bit
//! intermediates (Knuth, *The Art of Computer Programming*, volume 2, section 4.3.1,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>), as [`crate::wide::Wide`] with 32-bit
//! limbs; the products of operands of different widths follow the same algorithm with the
//! operands' own limb counts. There is no division: predicates never need it.
//!
//! Cost: for `L` limbs, addition, subtraction, negation, comparison and conversion are O(L) limb
//! operations; a product of an `A`-limb and a `B`-limb value is A · B limb products, each one
//! 32 × 32 → 64-bit multiply, on the stack with no allocation.

use core::cmp::Ordering;
use core::fmt;

use crate::sign::Sign;
use crate::wide::Wide;

/// A signed integer of `32 × L` bits, two's complement, little-endian 32-bit limbs.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Wide32<const L: usize>([u32; L]);

/// The top bit of a limb: the sign bit of the top limb.
const TOP_BIT: u32 = 1 << 31;

/// The low and high 32-bit halves of `t`, t = high · 2^32 + low: read from its little-endian
/// bytes, so there is no cast and no shift.
const fn halves(t: u64) -> (u32, u32) {
    let [b0, b1, b2, b3, b4, b5, b6, b7] = t.to_le_bytes();
    (
        u32::from_le_bytes([b0, b1, b2, b3]),
        u32::from_le_bytes([b4, b5, b6, b7]),
    )
}

impl<const L: usize> Wide32<L> {
    /// Evaluated when a width is used: a `Wide32` needs at least two limbs to hold an `i64`.
    const AT_LEAST_TWO_LIMBS: () = assert!(L >= 2, "Wide32<L> needs L >= 2");

    /// Zero.
    pub const ZERO: Self = Self([0; L]);

    /// The limb that sign-extends a value with this sign.
    const fn fill(negative: bool) -> u32 {
        if negative { u32::MAX } else { 0 }
    }

    /// The largest value, 2^(32L−1) − 1.
    #[must_use]
    pub fn max_value() -> Self {
        let () = Self::AT_LEAST_TWO_LIMBS;
        let mut limbs = [u32::MAX; L];
        if let Some(top) = limbs.last_mut() {
            *top = !TOP_BIT;
        }
        Self(limbs)
    }

    /// The smallest value, −2^(32L−1).
    #[must_use]
    pub fn min_value() -> Self {
        let () = Self::AT_LEAST_TWO_LIMBS;
        let mut limbs = [0u32; L];
        if let Some(top) = limbs.last_mut() {
            *top = TOP_BIT;
        }
        Self(limbs)
    }

    /// From an `i64`, exactly: two limbs hold every `i64`.
    #[must_use]
    pub fn from_i64(v: i64) -> Self {
        let () = Self::AT_LEAST_TWO_LIMBS;
        let mut limbs = [Self::fill(v < 0); L];
        let (low, high) = halves(u64::from_le_bytes(v.to_le_bytes()));
        for (limb, part) in limbs.iter_mut().zip([low, high]) {
            *limb = part;
        }
        Self(limbs)
    }

    /// From an `i128`, or `None` if it does not fit `32 × L` bits.
    #[must_use]
    pub fn from_i128(v: i128) -> Option<Self> {
        let () = Self::AT_LEAST_TWO_LIMBS;
        let fill = Self::fill(v < 0);
        let mut parts = [0u32; 4];
        let [
            b0,
            b1,
            b2,
            b3,
            b4,
            b5,
            b6,
            b7,
            b8,
            b9,
            b10,
            b11,
            b12,
            b13,
            b14,
            b15,
        ] = v.to_le_bytes();
        let words = [
            [b0, b1, b2, b3],
            [b4, b5, b6, b7],
            [b8, b9, b10, b11],
            [b12, b13, b14, b15],
        ];
        for (p, w) in parts.iter_mut().zip(words) {
            *p = u32::from_le_bytes(w);
        }
        let mut limbs = [fill; L];
        for (limb, &part) in limbs.iter_mut().zip(parts.iter()) {
            *limb = part;
        }
        // Fewer than four limbs: the dropped parts must be sign extension, and the sign must
        // survive.
        let r = Self(limbs);
        let dropped_ok = parts.iter().skip(L).all(|&p| p == fill);
        (dropped_ok && r.is_negative() == (v < 0)).then_some(r)
    }

    /// From raw two's-complement limbs, least significant first.
    #[must_use]
    pub const fn from_le_limbs(limbs: [u32; L]) -> Self {
        Self(limbs)
    }

    /// The raw two's-complement limbs, least significant first.
    #[must_use]
    pub const fn to_le_limbs(self) -> [u32; L] {
        self.0
    }

    /// Whether the value is below zero: whether the top limb's top bit is set.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.0.last().is_some_and(|top| top & TOP_BIT != 0)
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

    /// Limb-wise addition with carry; the carry out of the top limb is dropped, and the caller
    /// decides overflow from the signs. Each limb: x + y + carry ≤ 2 (2^32 − 1) + 1 = 2^33 − 1, the
    /// low 32 bits and one carry bit; the two `overflowing_add`s cannot both carry, as for
    /// [`crate::wide::Wide`].
    fn wrapping_add_limbs(a: &[u32; L], b: &[u32; L]) -> [u32; L] {
        let mut out = [0u32; L];
        let mut carry = false;
        for ((o, &x), &y) in out.iter_mut().zip(a.iter()).zip(b.iter()) {
            let (s1, c1) = x.overflowing_add(y);
            let (s2, c2) = s1.overflowing_add(u32::from(carry));
            *o = s2;
            carry = c1 || c2;
        }
        out
    }

    /// Limb-wise subtraction with borrow; the borrow out of the top limb is dropped, and the caller
    /// decides overflow from the signs.
    fn wrapping_sub_limbs(a: &[u32; L], b: &[u32; L]) -> [u32; L] {
        let mut out = [0u32; L];
        let mut borrow = false;
        for ((o, &x), &y) in out.iter_mut().zip(a.iter()).zip(b.iter()) {
            let (d1, b1) = x.overflowing_sub(y);
            let (d2, b2) = d1.overflowing_sub(u32::from(borrow));
            *o = d2;
            borrow = b1 || b2;
        }
        out
    }

    /// Two's-complement negation, wrapping at the minimum value.
    fn wrapping_neg(&self) -> Self {
        let mut out = [0u32; L];
        let mut carry = true;
        for (o, &x) in out.iter_mut().zip(self.0.iter()) {
            let (s, c) = (!x).overflowing_add(u32::from(carry));
            *o = s;
            carry = c;
        }
        Self(out)
    }

    /// The magnitude as an unsigned `32 × L`-bit number (the minimum value's magnitude,
    /// 2^(32L−1), fits as unsigned).
    fn magnitude(&self) -> [u32; L] {
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
        // Overflow happens exactly when the operands differ in sign and the result's sign is not
        // self's.
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

    /// The exact product a · b of an `A`-limb and a `B`-limb value as a `Wide32<L>`, or `None` if
    /// it does not fit: A · B limb products, each one 32 × 32 → 64-bit multiply, with neither
    /// operand widened to `L` limbs first. The generator chooses `L` from the operands' certified
    /// bits, so in its exact paths this never returns `None`.
    ///
    /// Each limb step: a limb product is at most (2^32 − 1)², below 2^64, so `checked_mul` on
    /// `u64` never fails; with the limb it lands on and the carry in, both below 2^32, the sum is at
    /// most (2^32 − 1)² + 2 (2^32 − 1) = 2^64 − 1, so neither `checked_add` fails. A row's last
    /// carry lands on limb i + B, which no earlier row has reached; a non-zero limb at or above `L`
    /// makes the magnitude at least 2^(32L), and the sign is applied last, as
    /// [`crate::wide::Wide::checked_mul`] applies it.
    #[must_use]
    pub fn product<const A: usize, const B: usize>(a: &Wide32<A>, b: &Wide32<B>) -> Option<Self> {
        let () = Self::AT_LEAST_TWO_LIMBS;
        let negative = a.is_negative() != b.is_negative();
        let left = a.magnitude();
        let right = b.magnitude();
        let mut out = [0u32; L];
        for (i, &li) in left.iter().enumerate() {
            if li == 0 {
                continue;
            }
            let mut carry = 0u32;
            for (j, &rj) in right.iter().enumerate() {
                // At most (2^32 − 1)², below 2^64: never fails.
                let prod = u64::from(li).checked_mul(u64::from(rj))?;
                // Limb indices are below A and B, so their sum is below A + B: never fails.
                match out.get_mut(i.checked_add(j)?) {
                    Some(slot) => {
                        // At most (2^32 − 1)² + 2 (2^32 − 1) = 2^64 − 1: never fails.
                        let t = prod
                            .checked_add(u64::from(*slot))?
                            .checked_add(u64::from(carry))?;
                        (*slot, carry) = halves(t);
                    }
                    None => {
                        if prod != 0 || carry != 0 {
                            return None;
                        }
                    }
                }
            }
            match out.get_mut(i.checked_add(B)?) {
                Some(slot) => *slot = carry,
                None => {
                    if carry != 0 {
                        return None;
                    }
                }
            }
        }
        Self::from_magnitude(out, negative)
    }

    /// The exact product of two `i64` values, or `None` if it does not fit: [`Wide32::product`] of
    /// their two-limb forms, four limb products.
    #[must_use]
    pub fn product_i64(a: i64, b: i64) -> Option<Self> {
        Self::product(&Wide32::<2>::from_i64(a), &Wide32::<2>::from_i64(b))
    }

    /// The exact product of an `A`-limb value and an `i64`, or `None` if it does not fit:
    /// [`Wide32::product`] with the `i64` in two limbs, 2 · A limb products.
    #[must_use]
    pub fn product_by_i64<const A: usize>(a: &Wide32<A>, n: i64) -> Option<Self> {
        Self::product(a, &Wide32::<2>::from_i64(n))
    }

    /// The value with magnitude `m` and the given sign, or `None` if it does not fit the signed
    /// range: below 2^(32L−1), or exactly 2^(32L−1) for a negative value (the minimum value).
    fn from_magnitude(m: [u32; L], negative: bool) -> Option<Self> {
        let top_bit_set = m.last().is_some_and(|top| top & TOP_BIT != 0);
        if !top_bit_set {
            let v = Self(m);
            return Some(if negative { v.wrapping_neg() } else { v });
        }
        let is_power = m
            .iter()
            .rev()
            .enumerate()
            .all(|(k, &limb)| if k == 0 { limb == TOP_BIT } else { limb == 0 });
        (negative && is_power).then(Self::min_value)
    }

    /// The same value in a `Wide32<M>`, or `None` if it does not fit.
    #[must_use]
    pub fn resize<const M: usize>(&self) -> Option<Wide32<M>> {
        let () = Wide32::<M>::AT_LEAST_TWO_LIMBS;
        let fill = Self::fill(self.is_negative());
        let mut out = [fill; M];
        for (o, &x) in out.iter_mut().zip(self.0.iter()) {
            *o = x;
        }
        let r = Wide32::<M>(out);
        // Narrowing: the dropped limbs must be pure sign extension, and the sign must survive.
        let dropped_ok = self.0.iter().skip(M).all(|&x| x == fill);
        (dropped_ok && r.is_negative() == self.is_negative()).then_some(r)
    }

    /// The same value as a [`Wide`] of `M` 64-bit limbs, or `None` if it does not fit: limbs
    /// 2k and 2k + 1 make limb k.
    #[must_use]
    pub fn to_wide<const M: usize>(&self) -> Option<Wide<M>> {
        let fill = Self::fill(self.is_negative());
        let limb = |i: usize| self.0.get(i).copied().unwrap_or(fill);
        let mut out = [0u64; M];
        for (k, o) in out.iter_mut().enumerate() {
            let low = k.checked_mul(2)?;
            let [b0, b1, b2, b3] = limb(low).to_le_bytes();
            let [b4, b5, b6, b7] = limb(low.checked_add(1)?).to_le_bytes();
            *o = u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7]);
        }
        let r = Wide::<M>::from_le_limbs(out);
        let dropped_ok = self.0.iter().skip(M.checked_mul(2)?).all(|&x| x == fill);
        (dropped_ok && r.is_negative() == self.is_negative()).then_some(r)
    }

    /// The value as an `i128`, if it fits.
    #[must_use]
    pub fn to_i128(&self) -> Option<i128> {
        let fill = Self::fill(self.is_negative());
        let mut parts = [fill; 4];
        for (p, &x) in parts.iter_mut().zip(self.0.iter()) {
            *p = x;
        }
        let mut bytes = [0u8; 16];
        for (byte, b) in bytes
            .iter_mut()
            .zip(parts.iter().flat_map(|p| p.to_le_bytes()))
        {
            *byte = b;
        }
        let v = i128::from_le_bytes(bytes);
        let dropped_ok = self.0.iter().skip(4).all(|&x| x == fill);
        (dropped_ok && (v < 0) == self.is_negative()).then_some(v)
    }
}

impl<const L: usize> Ord for Wide32<L> {
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

impl<const L: usize> PartialOrd for Wide32<L> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const L: usize> fmt::Debug for Wide32<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Wide32<{L}>(0x")?;
        for limb in self.0.iter().rev() {
            write!(f, "{limb:08x}")?;
        }
        f.write_str(")")
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests: oracles and generators"
)]
mod tests {
    use super::*;
    use num_bigint::BigInt;

    /// `SplitMix64` (Steele, Lea and Flood, 2014), for the inputs of these tests.
    struct Mix(u64);

    impl Mix {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        /// A value of a random bit length and sign, or the largest or smallest value.
        fn wide32<const L: usize>(&mut self) -> Wide32<L> {
            match self.next() % 8 {
                0 => Wide32::max_value(),
                1 => Wide32::min_value(),
                2 => Wide32::ZERO,
                _ => {
                    let keep = (self.next() % (32 * L as u64)) as usize;
                    let mut limbs = [0u32; L];
                    for (k, limb) in limbs.iter_mut().enumerate() {
                        let low = 32 * k;
                        let word = self.next() as u32;
                        *limb = if keep <= low {
                            0
                        } else if keep >= low + 32 {
                            word
                        } else {
                            word >> (32 - (keep - low))
                        };
                    }
                    let v = Wide32::from_le_limbs(limbs);
                    let v = if v.is_negative() { v.wrapping_neg() } else { v };
                    if self.next() % 2 == 0 {
                        v
                    } else {
                        v.wrapping_neg()
                    }
                }
            }
        }
    }

    /// The exact value of a `Wide32`.
    fn big<const L: usize>(v: Wide32<L>) -> BigInt {
        let mut m = BigInt::from(0);
        for &limb in v.to_le_limbs().iter().rev() {
            m = (m << 32) + BigInt::from(limb);
        }
        if v.is_negative() {
            m - (BigInt::from(1) << (32 * L))
        } else {
            m
        }
    }

    /// The `Wide32<L>` holding `v`, or `None` if it does not fit.
    fn fits<const L: usize>(v: &BigInt) -> Option<Wide32<L>> {
        let limit = BigInt::from(1) << (32 * L - 1);
        if *v < -limit.clone() || *v >= limit {
            return None;
        }
        let modulus = BigInt::from(1) << (32 * L);
        let mut u = if *v < BigInt::from(0) {
            v + &modulus
        } else {
            v.clone()
        };
        let mut limbs = [0u32; L];
        for limb in &mut limbs {
            let low: BigInt = &u % (BigInt::from(1) << 32);
            *limb = u32::try_from(low).unwrap();
            u >>= 32;
        }
        Some(Wide32::from_le_limbs(limbs))
    }

    fn arithmetic_agrees<const L: usize>(mix: &mut Mix) {
        for _ in 0..4_000 {
            let (a, b) = (mix.wide32::<L>(), mix.wide32::<L>());
            let (x, y) = (big(a), big(b));
            assert_eq!(a.checked_add(&b), fits::<L>(&(&x + &y)), "{a:?} + {b:?}");
            assert_eq!(a.checked_sub(&b), fits::<L>(&(&x - &y)), "{a:?} - {b:?}");
            assert_eq!(
                Wide32::<L>::product(&a, &b),
                fits::<L>(&(&x * &y)),
                "{a:?} * {b:?}"
            );
            assert_eq!(a.checked_neg(), fits::<L>(&-&x), "-{a:?}");
            assert_eq!(a.cmp(&b), x.cmp(&y));
        }
    }

    #[test]
    fn arithmetic_agrees_with_big_integers_at_every_width() {
        let mut mix = Mix(0x0642_3200_0000_0001);
        arithmetic_agrees::<2>(&mut mix);
        arithmetic_agrees::<3>(&mut mix);
        arithmetic_agrees::<4>(&mut mix);
        arithmetic_agrees::<6>(&mut mix);
        arithmetic_agrees::<12>(&mut mix);
    }

    fn products_agree<const A: usize, const B: usize, const L: usize>(mix: &mut Mix) {
        for _ in 0..4_000 {
            let (a, b) = (mix.wide32::<A>(), mix.wide32::<B>());
            let p = big(a) * big(b);
            assert_eq!(Wide32::<L>::product(&a, &b), fits::<L>(&p), "{a:?} * {b:?}");
        }
    }

    #[test]
    fn products_of_different_widths_agree_with_big_integers() {
        let mut mix = Mix(0x0642_3200_0000_0002);
        products_agree::<2, 2, 3>(&mut mix);
        products_agree::<2, 2, 4>(&mut mix);
        products_agree::<2, 3, 4>(&mut mix);
        products_agree::<3, 2, 5>(&mut mix);
        products_agree::<3, 4, 6>(&mut mix);
        products_agree::<4, 4, 4>(&mut mix);
        products_agree::<4, 6, 12>(&mut mix);
        products_agree::<6, 6, 3>(&mut mix);
    }

    fn conversions_agree<const L: usize>(mix: &mut Mix) {
        for _ in 0..4_000 {
            let v = mix.wide32::<L>();
            let x = big(v);
            assert_eq!(v.resize::<2>().map(big), fits::<2>(&x).map(big));
            assert_eq!(v.resize::<5>().map(big), fits::<5>(&x).map(big));
            let as_i128 = i128::try_from(x.clone()).ok();
            assert_eq!(v.to_i128(), as_i128, "{v:?}");
            let wide = v.to_wide::<2>();
            assert_eq!(wide.and_then(|w| w.to_i128()), as_i128, "{v:?}");
            let w4 = v.to_wide::<4>().map(Wide::to_le_limbs);
            let expected = if L <= 8 {
                let fill = if v.is_negative() { u32::MAX } else { 0 };
                let mut limbs = [0u64; 4];
                for (k, l) in limbs.iter_mut().enumerate() {
                    let at = |i: usize| u64::from(v.to_le_limbs().get(i).copied().unwrap_or(fill));
                    *l = at(2 * k) | at(2 * k + 1) << 32;
                }
                Some(limbs)
            } else {
                fits::<8>(&x).map(|f| {
                    let l = f.to_le_limbs();
                    core::array::from_fn(|k| u64::from(l[2 * k]) | u64::from(l[2 * k + 1]) << 32)
                })
            };
            assert_eq!(w4, expected, "{v:?}");
        }
    }

    #[test]
    fn conversions_agree_with_big_integers() {
        let mut mix = Mix(0x0642_3200_0000_0003);
        conversions_agree::<2>(&mut mix);
        conversions_agree::<3>(&mut mix);
        conversions_agree::<4>(&mut mix);
        conversions_agree::<6>(&mut mix);
        conversions_agree::<8>(&mut mix);
        conversions_agree::<12>(&mut mix);
        for v in [
            0,
            1,
            -1,
            i64::MAX,
            i64::MIN,
            1 << 31,
            -(1 << 31),
            (1 << 32) - 1,
        ] {
            assert_eq!(big(Wide32::<2>::from_i64(v)), BigInt::from(v));
            assert_eq!(big(Wide32::<5>::from_i64(v)), BigInt::from(v));
        }
        for v in [
            0,
            1,
            -1,
            i128::MAX,
            i128::MIN,
            1 << 95,
            -(1 << 95),
            (1 << 95) - 1,
        ] {
            let x = BigInt::from(v);
            assert_eq!(
                Wide32::<3>::from_i128(v).map(big),
                fits::<3>(&x).map(big),
                "{v}"
            );
            assert_eq!(Wide32::<4>::from_i128(v).map(big), Some(x.clone()));
            assert_eq!(Wide32::<6>::from_i128(v).map(big), Some(x));
        }
    }

    #[test]
    fn the_extremes() {
        let min = Wide32::<3>::min_value();
        let max = Wide32::<3>::max_value();
        assert_eq!(max.checked_add(&Wide32::from_i64(1)), None);
        assert_eq!(min.checked_sub(&Wide32::from_i64(1)), None);
        assert_eq!(min.checked_neg(), None);
        assert_eq!(Wide32::<3>::product(&min, &Wide32::<2>::from_i64(-1)), None);
        assert_eq!(
            Wide32::<3>::product(&min, &Wide32::<2>::from_i64(1)),
            Some(min)
        );
        // −2^95 = 2^47 · (−2^48): reached from operands of two limbs each.
        let p = Wide32::<3>::product(
            &Wide32::<2>::from_i64(1 << 47),
            &Wide32::<2>::from_i64(-(1 << 48)),
        );
        assert_eq!(p, Some(min));
        let q = Wide32::<3>::product(
            &Wide32::<2>::from_i64(1 << 47),
            &Wide32::<2>::from_i64(1 << 48),
        );
        assert_eq!(q, None);
        // The largest product of two `i64` values fits four limbs and never three.
        let m = Wide32::<2>::from_i64(i64::MIN);
        assert_eq!(
            Wide32::<4>::product(&m, &m).map(big),
            Some(BigInt::from(1) << 126)
        );
        assert_eq!(Wide32::<3>::product(&m, &m), None);
        assert_eq!(Wide32::<2>::ZERO.signum(), Sign::Zero);
        assert_eq!(min.signum(), Sign::Negative);
        assert_eq!(max.signum(), Sign::Positive);
    }
}
