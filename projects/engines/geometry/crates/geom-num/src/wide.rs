//! Fixed-width signed integers for exact predicates.
//!
//! The deepest exact predicates need more than 128 bits: an in-circle test about 168 bits, an
//! in-sphere test about 212, a point made by three planes tested against a fourth plane about 379
//! (chapter 6.2). These types hold such values in a fixed number of 64-bit limbs, on the stack,
//! with no allocation, so they are fast, deterministic and usable in `no_std`.
//!
//! Representation: two's complement, limbs little-endian (limb 0 is the least significant).
//! A `Wide<L>` holds values from −2^(64L−1) to 2^(64L−1) − 1.
//!
//! Guarantees: every arithmetic operation is checked: it returns `None` when the true result does
//! not fit, and never wraps silently. The kernel proves that its predicates never overflow; these
//! checks turn a wrong proof into a detectable failure instead of a wrong sign.
//!
//! The same holds inside, limb by limb, so nothing here depends on overflow checks being on. A
//! limb sum or difference uses `overflowing_add` and `overflowing_sub`, which return the full
//! result as the low 64 bits and the carry or borrow, so nothing is lost: the carry is propagated
//! to the next limb, and only the carry out of the top limb is dropped, as two's complement
//! defines, before the sign test that decides overflow. A limb product uses `checked_mul` and
//! `checked_add` on `u128`, which cannot fail ((2^64 − 1)² + 2 (2^64 − 1) = 2^128 − 1, written
//! beside each), and is split into its two 64-bit halves by a mask and a shift, each half cast to
//! `u64` with its bound written beside it.
//!
//! Proven (chapter 28.7; pieces G37 to G39): Verus proves, for every width `L` from 1 to 4096 limbs
//! and so for [`I256`], [`I320`], [`I384`] and [`I512`], that `checked_add`, `checked_sub`,
//! `checked_neg`, `checked_mul`, the width-aware products `product_i128` and `checked_mul_i128`,
//! and `resize` return the exact two's-complement result whenever it fits and `None` exactly when
//! it does not, that `from_i128` gives the value it is given, and that the order ([`Ord`]) is the
//! order of the values. The
//! proof is in this crate's `verus/wide.rs`, a twin of this module's arithmetic: the same
//! functions with Verus specifications and proof steps added, which `geom-gen`'s
//! `tests/verus_twin.rs` holds token for token to the code here, and which `kernel-dev.yml`'s
//! `verus` job verifies. So the code is written in the part of Rust Verus reads: index loops over
//! `get` and `get_mut`, and `match` where an `Option` combinator would do. Its trusted base is
//! Verus, the Z3 solver it uses, and four facts it is told about the standard library, each its
//! documented behaviour: `u64::overflowing_add`, `u64::overflowing_sub`, `u64::from(bool)` and
//! `i128::unsigned_abs`.
//!
//! Bounded proofs, and what is only tested: Kani proves (piece G41, chapter 28.7) that addition,
//! subtraction, negation, the order and sign, the `i128` conversions and `resize` never panic and
//! keep the laws their proofs state, for every four-limb value and every `i128`, and that
//! `checked_mul` never panics for every pair of two-limb values. At four limbs the multiplication
//! ran past CI's hour; the code is the same for every width, and products of every width are
//! tested against big integers.
//!
//! Method: schoolbook addition, subtraction and multiplication on magnitudes with 128-bit
//! intermediates (Knuth, *The Art of Computer Programming*, volume 2, section 4.3.1,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>). There is no division: predicates
//! never need it.
//!
//! Cost: for `L` limbs, addition, subtraction, negation, comparison and resizing are O(L) limb
//! operations and multiplication O(L²), on the stack with no allocation; the kernel's widths are
//! 4 to 8 limbs.

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
/// 512-bit signed integer: the fallback width of [`crate::plane`], which repeats a classification
/// or a commit in it should a step in [`I384`] not fit (unreachable, by the budgets asserted there
/// when the crate compiles), and a wider width for tests. Chapter 6.2 lists only `I256`, `I320`
/// and `I384` as accepted kernel widths, each wider one needing an owner's acceptance; this one is
/// raised for that list as a fallback that adds no budget of its own.
pub type I512 = Wide<8>;

/// The top bit of a limb: the sign bit of the top limb.
const TOP_BIT: u64 = 1 << 63;

/// Limb-wise addition with carry; the carry out of the top limb is dropped (two's complement
/// wrap), and the caller decides overflow from the signs.
fn wrapping_add_limbs<const L: usize>(a: &[u64; L], b: &[u64; L]) -> [u64; L] {
    let mut out = [0u64; L];
    let mut carry = false;
    for i in 0..L {
        let x = match a.get(i) {
            Some(v) => *v,
            None => 0,
        };
        let y = match b.get(i) {
            Some(v) => *v,
            None => 0,
        };
        let (s1, c1) = x.overflowing_add(y);
        let (s2, c2) = s1.overflowing_add(u64::from(carry));
        if let Some(o) = out.get_mut(i) {
            *o = s2;
        }
        carry = c1 || c2;
    }
    out
}

/// Limb-wise subtraction with borrow; the borrow out of the top limb is dropped, and the caller
/// decides overflow from the signs.
fn wrapping_sub_limbs<const L: usize>(a: &[u64; L], b: &[u64; L]) -> [u64; L] {
    let mut out = [0u64; L];
    let mut borrow = false;
    for i in 0..L {
        let x = match a.get(i) {
            Some(v) => *v,
            None => 0,
        };
        let y = match b.get(i) {
            Some(v) => *v,
            None => 0,
        };
        let (d1, b1) = x.overflowing_sub(y);
        let (d2, b2) = d1.overflowing_sub(u64::from(borrow));
        if let Some(o) = out.get_mut(i) {
            *o = d2;
        }
        borrow = b1 || b2;
    }
    out
}

/// How two limb sequences compare as unsigned numbers: from the top limb down, the first limb that
/// differs decides.
fn compare_limbs<const L: usize>(a: &[u64; L], b: &[u64; L]) -> Ordering {
    let mut i = L;
    while i > 0 {
        // i > 0, so i − 1 never wraps.
        i = i.wrapping_sub(1);
        let high_a = match a.get(i) {
            Some(v) => *v,
            None => 0,
        };
        let high_b = match b.get(i) {
            Some(v) => *v,
            None => 0,
        };
        if high_a != high_b {
            return if high_a < high_b {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
    }
    Ordering::Equal
}

/// The limb that sign-extends a value with this sign.
const fn fill(negative: bool) -> u64 {
    // 0 − 1 wraps to all ones; written without a branch, so a caller's sign fill compiles to a
    // shift of the sign bit rather than a branch on it.
    0u64.wrapping_sub(negative as u64)
}

/// The low and high 64-bit halves of `t`, t = high · 2^64 + low.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the low half is t masked to 64 bits and the high half t shifted down 64, each below 2^64"
)]
fn halves(t: u128) -> (u64, u64) {
    ((t & 0xFFFF_FFFF_FFFF_FFFF) as u64, (t >> 64) as u64)
}

/// The value with magnitude `m` (`M` limbs, little-endian) and the given sign, or
/// `None` if it does not fit: the limbs from `L` on must be zero, and the magnitude must be below
/// 2^(64L−1), or exactly 2^(64L−1) for a negative value (the minimum value), as
/// [`Wide::checked_mul`] decides it.
fn from_magnitude<const L: usize, const M: usize>(m: &[u64; M], negative: bool) -> Option<Wide<L>> {
    let mut out = [0u64; L];
    for k in 0..M {
        let limb = match m.get(k) {
            Some(v) => *v,
            None => 0,
        };
        if let Some(slot) = out.get_mut(k) {
            *slot = limb;
        } else if limb != 0 {
            // A non-zero limb at or above limb L makes the magnitude at least 2^(64L).
            return None;
        }
    }
    let v = Wide(out);
    if !v.is_negative() {
        return Some(if negative { v.wrapping_neg() } else { v });
    }
    // Top bit set: only the magnitude 2^(64L−1) with a negative sign fits (the minimum value), the
    // one non-zero magnitude that negation leaves unchanged.
    let n = v.wrapping_neg();
    if negative && matches!(compare_limbs(&n.0, &v.0), Ordering::Equal) {
        Some(n)
    } else {
        None
    }
}

impl<const L: usize> Wide<L> {
    /// Evaluated when a width is used: a `Wide` needs at least two limbs to hold an `i128`.
    const AT_LEAST_TWO_LIMBS: () = assert!(L >= 2, "Wide<L> needs L >= 2");

    /// Zero.
    pub const ZERO: Self = Self([0; L]);

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
            *top = !TOP_BIT;
        }
        Self(limbs)
    }

    /// The smallest value, −2^(64L−1).
    #[must_use]
    pub fn min_value() -> Self {
        let () = Self::AT_LEAST_TWO_LIMBS;
        let mut limbs = [0u64; L];
        if let Some(top) = limbs.last_mut() {
            *top = TOP_BIT;
        }
        Self(limbs)
    }

    /// From an `i128`, exactly.
    #[must_use]
    pub fn from_i128(v: i128) -> Self {
        let mut limbs = [fill(v < 0); L];
        // v's two's complement in 128 bits, read as unsigned: v itself when v is not negative, and
        // v + 2^128 when it is, made as (v − i128::MIN) + 2^127 so that no cast loses a value.
        #[allow(clippy::cast_sign_loss, reason = "each cast value is at least zero")]
        let u = if v < 0 {
            match v.checked_sub(i128::MIN) {
                Some(d) => (d as u128) | (1u128 << 127),
                None => 0,
            }
        } else {
            v as u128
        };
        let (low, high) = halves(u);
        if let Some(limb) = limbs.get_mut(0) {
            *limb = low;
        }
        if let Some(limb) = limbs.get_mut(1) {
            *limb = high;
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

    /// Whether the value is below zero: whether the top limb's top bit is set.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        match self.0.last() {
            Some(top) => top & TOP_BIT != 0,
            None => false,
        }
    }

    /// Whether the value is exactly zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        for i in 0..L {
            let limb = match self.0.get(i) {
                Some(v) => *v,
                None => 0,
            };
            if limb != 0 {
                return false;
            }
        }
        true
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

    /// Two's-complement negation, wrapping at the minimum value: the complement of every limb, plus
    /// one, carried limb by limb.
    fn wrapping_neg(&self) -> Self {
        let mut out = [0u64; L];
        let mut carry = true;
        for i in 0..L {
            let x = match self.0.get(i) {
                Some(v) => *v,
                None => 0,
            };
            let (s, c) = u64::from(carry).overflowing_add(!x);
            if let Some(o) = out.get_mut(i) {
                *o = s;
            }
            carry = c;
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
        let r = Self(wrapping_add_limbs(&self.0, &other.0));
        // Overflow happens exactly when both operands share a sign and the result does not.
        let overflow =
            self.is_negative() == other.is_negative() && r.is_negative() != self.is_negative();
        if overflow { None } else { Some(r) }
    }

    /// `self − other`, or `None` if the true difference does not fit.
    #[must_use]
    pub fn checked_sub(&self, other: &Self) -> Option<Self> {
        let r = Self(wrapping_sub_limbs(&self.0, &other.0));
        // Overflow happens exactly when the operands differ in sign and the result's sign is not
        // self's.
        let overflow =
            self.is_negative() != other.is_negative() && r.is_negative() != self.is_negative();
        if overflow { None } else { Some(r) }
    }

    /// `−self`, or `None` for the minimum value, whose negation does not fit.
    #[must_use]
    pub fn checked_neg(&self) -> Option<Self> {
        let r = self.wrapping_neg();
        // Only the minimum value is negative and stays negative when negated.
        let overflow = self.is_negative() & r.is_negative();
        if overflow { None } else { Some(r) }
    }

    /// How this value compares with `other`: by sign first, and for the same sign by the limbs,
    /// since two's-complement values of one sign order like their unsigned bit patterns.
    fn compare(&self, other: &Self) -> Ordering {
        match (self.is_negative(), other.is_negative()) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => compare_limbs(&self.0, &other.0),
        }
    }

    /// `self × other`, or `None` if the true product does not fit.
    #[must_use]
    pub fn checked_mul(&self, other: &Self) -> Option<Self> {
        let negative = self.is_negative() != other.is_negative();
        let a = self.magnitude();
        let b = other.magnitude();
        let mut out = [0u64; L];
        for i in 0..L {
            let ai = match a.get(i) {
                Some(v) => *v,
                None => 0,
            };
            if ai != 0 {
                let mut carry = 0u64;
                for j in 0..L {
                    let bj = match b.get(j) {
                        Some(v) => *v,
                        None => 0,
                    };
                    // At most (2^64 − 1)², below 2^128: `checked_mul` never fails here.
                    let prod = u128::from(ai).checked_mul(u128::from(bj))?;
                    // Limb indices are below L, so their sum is below 2L: never fails.
                    let k = i.checked_add(j)?;
                    match out.get(k) {
                        Some(&old) => {
                            // old and carry are below 2^64 and prod at most (2^64 − 1)², so the
                            // sum is at most (2^64 − 1)² + 2 (2^64 − 1) = 2^128 − 1: never fails.
                            let t = prod
                                .checked_add(u128::from(old))?
                                .checked_add(u128::from(carry))?;
                            let (low, high) = halves(t);
                            if let Some(slot) = out.get_mut(k) {
                                *slot = low;
                            }
                            carry = high;
                        }
                        // Any non-zero contribution at or above limb L makes the magnitude at least
                        // 2^(64L).
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
        }
        // The magnitude fits in 64 × L unsigned bits. It must also fit the signed range: below
        // 2^(64L−1) for a positive result, at most 2^(64L−1) for a negative one.
        let m = Self(out);
        if !m.is_negative() {
            return Some(if negative { m.wrapping_neg() } else { m });
        }
        // Top bit set: only the exact magnitude 2^(64L−1) with a negative sign fits (the minimum
        // value), the one non-zero magnitude that negation leaves unchanged.
        let n = m.wrapping_neg();
        if negative && matches!(compare_limbs(&n.0, &m.0), Ordering::Equal) {
            Some(n)
        } else {
            None
        }
    }

    /// The exact product a · b of two `i128` values as a `Wide<L>`, or `None` if it does not fit:
    /// from four limbs it always fits, since |a · b| ≤ 2^127 · 2^127 = 2^254 < 2^255.
    ///
    /// The width-aware product of the generated exact paths (chapter 23.3): the same value as
    /// `Wide::from_i128(a).checked_mul(&Wide::from_i128(b))`, in four 64 × 64 → 128-bit limb
    /// products instead of [`Wide::checked_mul`]'s loops over every limb of two widened operands.
    ///
    /// Method and budget: the magnitudes' 64-bit halves |a| = a₁·2^64 + a₀ and |b| = b₁·2^64 + b₀
    /// multiply pairwise (Knuth, volume 2, section 4.3.1). Each partial product is at most
    /// (2^64 − 1)², below 2^128, so `checked_mul` on `u128` never fails. Limb 1 sums the high half
    /// of a₀b₀ and the low halves of a₀b₁ and a₁b₀: at most 3 (2^64 − 1) < 2^66, its carry at most
    /// 2. Limbs 2 and 3 sum a₁b₁, the high halves of a₀b₁ and a₁b₀ (each at most 2^64 − 2) and that
    /// carry: at most (2^64 − 1)² + 2 (2^64 − 2) + 2 = 2^128 − 1, so no `checked_add` fails either.
    /// The sign is applied last, as [`Wide::checked_mul`] applies it.
    #[must_use]
    pub(crate) fn product_i128(a: i128, b: i128) -> Option<Self> {
        let negative = (a < 0) != (b < 0);
        let (a0, a1) = halves(a.unsigned_abs());
        let (b0, b1) = halves(b.unsigned_abs());
        let p00 = u128::from(a0).checked_mul(u128::from(b0))?;
        let p01 = u128::from(a0).checked_mul(u128::from(b1))?;
        let p10 = u128::from(a1).checked_mul(u128::from(b0))?;
        let p11 = u128::from(a1).checked_mul(u128::from(b1))?;
        let (l0, h00) = halves(p00);
        let (l01, h01) = halves(p01);
        let (l10, h10) = halves(p10);
        let middle = u128::from(h00)
            .checked_add(u128::from(l01))?
            .checked_add(u128::from(l10))?;
        let (l1, carry) = halves(middle);
        let upper = p11
            .checked_add(u128::from(h01))?
            .checked_add(u128::from(h10))?
            .checked_add(u128::from(carry))?;
        let (l2, l3) = halves(upper);
        from_magnitude(&[l0, l1, l2, l3], negative)
    }

    /// `self × b` for an `i128` b, or `None` if the true product does not fit.
    ///
    /// The width-aware product of the generated exact paths (chapter 23.3): the same value as
    /// `self.checked_mul(&Wide::from_i128(b))`, by schoolbook multiplication of the magnitude's
    /// `L` limbs by the two 64-bit halves of |b| (Knuth, volume 2, section 4.3.1), with no
    /// widening of b and no pass over its sign-extension limbs. Each limb step's bound is the one
    /// [`Wide::checked_mul`] writes beside its own: a product at most (2^64 − 1)², plus a limb and
    /// a carry, at most 2^128 − 1.
    #[must_use]
    pub(crate) fn checked_mul_i128(&self, b: i128) -> Option<Self> {
        let negative = self.is_negative() != (b < 0);
        let a = self.magnitude();
        let (b0, b1) = halves(b.unsigned_abs());
        let mut out = [0u64; L];
        for shift in 0..2 {
            let bj = if shift == 0 { b0 } else { b1 };
            if bj != 0 {
                let mut carry = 0u64;
                for i in 0..L {
                    let ai = match a.get(i) {
                        Some(v) => *v,
                        None => 0,
                    };
                    // At most (2^64 − 1)², below 2^128: `checked_mul` never fails here.
                    let prod = u128::from(ai).checked_mul(u128::from(bj))?;
                    // Limb indices are below L and the shift at most 1: never fails.
                    let k = i.checked_add(shift)?;
                    match out.get(k) {
                        Some(&old) => {
                            // At most (2^64 − 1)² + 2 (2^64 − 1) = 2^128 − 1: never fails.
                            let t = prod
                                .checked_add(u128::from(old))?
                                .checked_add(u128::from(carry))?;
                            let (low, high) = halves(t);
                            if let Some(slot) = out.get_mut(k) {
                                *slot = low;
                            }
                            carry = high;
                        }
                        // A non-zero contribution at limb L makes the magnitude at least 2^(64L).
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
        }
        from_magnitude(&out, negative)
    }

    /// The same value in a `Wide<M>`, or `None` if it does not fit.
    #[must_use]
    pub fn resize<const M: usize>(&self) -> Option<Wide<M>> {
        let negative = self.is_negative();
        let sign = fill(negative);
        let mut out = [sign; M];
        // The limbs both widths hold, copied; above them, the sign fill. The loop runs over exactly
        // those limbs, a constant for each pair of widths, so it unrolls with no bounds checks.
        let shared = if M < L { M } else { L };
        for i in 0..shared {
            let x = match self.0.get(i) {
                Some(v) => *v,
                None => 0,
            };
            if let Some(slot) = out.get_mut(i) {
                *slot = x;
            }
        }
        if M < L {
            // Narrowing: every dropped limb must be pure sign extension.
            for i in M..L {
                let x = match self.0.get(i) {
                    Some(v) => *v,
                    None => 0,
                };
                if x != sign {
                    return None;
                }
            }
        }
        let r = Wide::<M>(out);
        // Narrowing, the sign must survive. Widening keeps it always, and the test is left out
        // there (a constant for each pair of widths), as it costs every widening a branch.
        if M < L && r.is_negative() != negative {
            return None;
        }
        Some(r)
    }

    /// The value as an `i128`, if it fits.
    #[must_use]
    pub fn to_i128(&self) -> Option<i128> {
        let fill = fill(self.is_negative());
        if !self.0.iter().skip(2).all(|&x| x == fill) {
            return None;
        }
        // Start from the sign fill, so a value with fewer than two limbs is sign-extended, not
        // zero-extended.
        let mut parts = [fill; 2];
        for (p, &x) in parts.iter_mut().zip(self.0.iter()) {
            *p = x;
        }
        // The two limbs' bytes, low limb first, are v's two's complement in 128 bits.
        let mut bytes = [0u8; 16];
        let limb_bytes = parts.iter().flat_map(|limb| limb.to_le_bytes());
        for (byte, b) in bytes.iter_mut().zip(limb_bytes) {
            *byte = b;
        }
        let v = i128::from_le_bytes(bytes);
        // The sign of the 128-bit value must match.
        (Sign::of_i128(v) == self.signum()).then_some(v)
    }
}

impl<const L: usize> Ord for Wide<L> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
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

    /// `SplitMix64` (Steele, Lea and Flood, 2014), for the inputs of the width-aware products' tests.
    struct Mix(u64);

    impl Mix {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        /// An `i128` of a random bit length, sign and pattern, or one of the edge values.
        fn i128(&mut self) -> i128 {
            const EDGES: [i128; 9] = [
                0,
                1,
                -1,
                i128::MAX,
                i128::MIN,
                i128::MAX - 1,
                i128::MIN + 1,
                1 << 64,
                -(1 << 64),
            ];
            let pick = self.next();
            if pick % 4 == 0 {
                return EDGES[(pick / 4 % 9) as usize];
            }
            let raw = (u128::from(self.next()) << 64 | u128::from(self.next())) as i128;
            raw >> (self.next() % 128)
        }

        /// A `Wide<L>` of a random bit length and sign, or the largest or smallest value.
        fn wide<const L: usize>(&mut self) -> Wide<L> {
            match self.next() % 8 {
                0 => Wide::max_value(),
                1 => Wide::min_value(),
                _ => {
                    let keep = (self.next() % (64 * L as u64)) as usize;
                    let mut limbs = [0u64; L];
                    for (k, limb) in limbs.iter_mut().enumerate() {
                        let low = 64 * k;
                        *limb = if keep <= low {
                            0
                        } else if keep >= low + 64 {
                            self.next()
                        } else {
                            self.next() >> (64 - (keep - low))
                        };
                    }
                    let v = Wide::from_le_limbs(limbs);
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

    /// `product_i128` and `checked_mul_i128` give exactly what the general product gives (tested
    /// against big integers in `tests/wide.rs`), `None` included, at every width.
    fn width_aware_products_agree<const L: usize>(mix: &mut Mix) {
        for _ in 0..20_000 {
            let (a, b) = (mix.i128(), mix.i128());
            let general = Wide::<L>::from_i128(a).checked_mul(&Wide::<L>::from_i128(b));
            assert_eq!(
                Wide::<L>::product_i128(a, b),
                general,
                "{a} · {b} in {L} limbs"
            );
            let w = mix.wide::<L>();
            let general = w.checked_mul(&Wide::<L>::from_i128(b));
            assert_eq!(w.checked_mul_i128(b), general, "{w:?} · {b}");
        }
    }

    #[test]
    fn width_aware_products_agree_with_the_general_product() {
        let mut mix = Mix(0x0629_0000_0000_0001);
        width_aware_products_agree::<2>(&mut mix);
        width_aware_products_agree::<3>(&mut mix);
        width_aware_products_agree::<4>(&mut mix);
        width_aware_products_agree::<5>(&mut mix);
        width_aware_products_agree::<6>(&mut mix);
        width_aware_products_agree::<8>(&mut mix);
    }

    #[test]
    fn width_aware_products_at_the_edges() {
        // 2^127 · 2^127 = 2^254 fits I256; −2^127 · 2^127 too, and never `i128`'s two limbs.
        let p254 = I256::from_le_limbs([0, 0, 0, 1 << 62]);
        assert_eq!(I256::product_i128(i128::MIN, i128::MIN), Some(p254));
        assert_eq!(
            I256::product_i128(i128::MIN, i128::MAX),
            I256::from_i128(i128::MIN).checked_mul(&I256::from_i128(i128::MAX))
        );
        assert_eq!(
            Wide::<2>::product_i128(i128::MIN, 1),
            Some(Wide::min_value())
        );
        assert_eq!(Wide::<2>::product_i128(i128::MIN, -1), None);
        assert_eq!(Wide::<2>::product_i128(1 << 64, 1 << 63), None);
        // The minimum value is reached by a negative product and refused for a positive one.
        let min = I256::min_value();
        let half = I256::from_le_limbs([0, 0, 0, 1 << 62]);
        assert_eq!(half.checked_mul_i128(-2), Some(min));
        assert_eq!(half.checked_mul_i128(2), None);
        assert_eq!(min.checked_mul_i128(1), Some(min));
        assert_eq!(min.checked_mul_i128(-1), None);
        assert_eq!(I256::max_value().checked_mul_i128(0), Some(I256::ZERO));
        assert_eq!(I384::max_value().checked_mul_i128(i128::MIN), None);
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

/// Bounded proofs (chapter 28.7; piece G41) at four limbs (`I256`): no panic and no overflow in
/// the checked arithmetic, for every pair of values, with each result checked against a second
/// computation where Kani can afford one. `geom-num`'s Verus twin (`verus/wide.rs`, pieces G37 to
/// G39, where present) proves exactness for every width; these harnesses prove the shipped
/// machine code free of panics and overflow.
#[cfg(kani)]
// cargo-mutants reads this attribute from the source and skips the module (its harnesses are
// proven by Kani, and no test build compiles them); no build evaluates it, since the module
// exists only under Kani, where `test` is off.
#[cfg_attr(test, mutants::skip)]
mod proofs {
    use super::*;

    /// A symbolic `I256`.
    fn any_i256() -> I256 {
        Wide::from_le_limbs([kani::any(), kani::any(), kani::any(), kani::any()])
    }

    /// `checked_add` and `checked_sub` never panic, and agree with each other: a + b − b is a
    /// whenever a + b fits.
    #[kani::proof]
    fn add_and_sub_never_panic_and_invert() {
        let (a, b) = (any_i256(), any_i256());
        if let Some(s) = a.checked_add(&b) {
            assert!(s.checked_sub(&b) == Some(a));
        }
        let _ = a.checked_sub(&b);
    }

    /// `checked_neg` never panics, refuses exactly the most negative value, and is its own
    /// inverse.
    #[kani::proof]
    fn neg_refuses_exactly_the_minimum() {
        let a = any_i256();
        match a.checked_neg() {
            Some(n) => assert!(n.checked_neg() == Some(a)),
            None => assert!(a == Wide::min_value()),
        }
    }

    /// The order never panics and is antisymmetric, with the sign agreeing with the order against
    /// zero. Unwound at most 17 times: the longest loop it reaches is `from_i128`'s 16 bytes.
    #[kani::proof]
    #[kani::unwind(17)]
    fn order_and_sign_agree() {
        let (a, b) = (any_i256(), any_i256());
        assert!(a.cmp(&b) == b.cmp(&a).reverse());
        let zero = Wide::from_i128(0);
        let sign = match a.cmp(&zero) {
            Ordering::Less => Sign::Negative,
            Ordering::Equal => Sign::Zero,
            Ordering::Greater => Sign::Positive,
        };
        assert!(a.signum() == sign && a.is_negative() == (sign == Sign::Negative));
    }

    /// `from_i128` and `to_i128` are inverse for every `i128`, and `resize` to two limbs and back
    /// keeps every value it accepts. Unwound at most 33 times: the longest loop is `memcmp`'s over
    /// the 32 bytes of the two four-limb values `==` compares.
    #[kani::proof]
    #[kani::unwind(33)]
    fn i128_conversions_and_resize_round_trip() {
        let v: i128 = kani::any();
        let w = I256::from_i128(v);
        assert!(w.to_i128() == Some(v));
        let a = any_i256();
        if let Some(narrow) = a.resize::<2>() {
            assert!(narrow.resize::<4>() == Some(a));
        }
    }

    /// The extremes are the limits: one past the largest value and the negation of the smallest
    /// do not fit, and the largest is the negation of one above the smallest. Unwound at most 33
    /// times, past `memcmp`'s 32 bytes: unbounded, the solver ran past CI's hour.
    #[kani::proof]
    #[kani::unwind(33)]
    fn extremes_are_the_limits() {
        let (max, min) = (I256::max_value(), I256::min_value());
        assert!(max.checked_add(&I256::one()).is_none());
        assert!(min.checked_sub(&I256::one()).is_none());
        assert!(min.checked_neg().is_none());
        assert!(min.checked_add(&I256::one()).and_then(|m| m.checked_neg()) == Some(max));
        assert!(max.signum() == Sign::Positive && min.signum() == Sign::Negative);
    }

    /// `checked_mul` never panics or overflows, for every pair of two-limb values. At four limbs
    /// the solver ran out of CI's hour (16 symbolic 128-bit limb products and their checks); the
    /// code is the same for every width, and every width is proven by Verus (piece G37). Unwound
    /// at most 17 times, past every loop it reaches.
    #[kani::proof]
    #[kani::unwind(17)]
    fn mul_never_panics() {
        let a = Wide::<2>::from_le_limbs([kani::any(), kani::any()]);
        let b = Wide::<2>::from_le_limbs([kani::any(), kani::any()]);
        let _ = a.checked_mul(&b);
    }
}
