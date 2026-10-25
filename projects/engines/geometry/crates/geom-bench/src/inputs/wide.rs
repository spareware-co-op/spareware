//! Inputs for fixed-width integers, `Wide<L>`: [`add`], [`sub`], [`mul`], [`neg`], [`cmp`],
//! [`signum`], [`to_i128`], [`from_i128`] and [`resize`], for any width `Wide` allows (two limbs
//! or more; the catalogue uses I256, I320, I384 and I512, and 2, 4 and 8 limbs for the growth
//! checks). Where a class cannot exist at a width its batch is empty, as for any class an operation
//! lacks: typical products need four limbs (two typical operands make up to 252 bits), and at two
//! limbs every value fits an `i128`, so nothing overflows `to_i128`.
//!
//! Classes:
//!
//! - `typical`: operands that fit an `i128`, of 64 to 126 bits, as the predicates feed them (every
//!   in-circle and in-sphere product is of two `i128` factors). Their high limbs are only sign
//!   extension, which `checked_mul` skips. For `signum`, negative, zero and positive values in
//!   rotation, including positive values with only the top limb set, the longest scan for zero.
//!   For `resize`, values that fit the target width.
//! - `extreme`: every limb in use. For `add` and `sub`, operands of 64L − 2 bits, so the result
//!   still fits; for `mul`, operands of 32L − 1 bits, so the product has 64L − 3 or 64L − 2 bits;
//!   for `neg`, magnitudes of 64L − 1 bits; for `cmp`, pairs that agree in every limb but the
//!   lowest (or in every limb), the longest comparison.
//! - `overflow`: the exact result does not fit and the operation returns `None`. For `add`,
//!   operands of the same sign with 64L − 1 bits; for `sub`, of opposite signs; for `mul`,
//!   operands of 32L + 1 bits, whose product needs at least 64L + 1; for `neg`, the minimum value,
//!   the only one whose negation does not fit; for `to_i128` and a narrowing `resize`, values
//!   wider than the target.
//!
//! "n bits" means the magnitude's highest set bit is bit n − 1; the sign is pseudo-random unless a
//! class needs it fixed.
//!
//! Method: the operand classes that take each path of schoolbook multiple-precision arithmetic
//! (Knuth, *The Art of Computer Programming*, volume 2, section 4.3.1,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>): short operands, full-width operands,
//! carries out of the top limb.
//!
//! Guarantees: every batch is full, the same on every target, and its classes are what they
//! claim (`tests/inputs.rs`); every result is checked against exact integers by
//! `tests/attack_g06.rs`.
//!
//! Cost: a few draws of [`crate::rng::SplitMix64`] per input, once per batch, outside every
//! measured run.

use geom_num::wide::Wide;

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// A non-negative `Wide<L>` with exactly `bits` significant bits (bit `bits − 1` set, the bits
/// below it pseudo-random). `bits` must be in 1..=64L − 1 so that the value and its negation fit.
fn magnitude<const L: usize>(g: &mut SplitMix64, bits: u32) -> Wide<L> {
    let mut limbs = [0u64; L];
    let mut low = 0u32; // the index of this limb's lowest bit
    for limb in &mut limbs {
        let keep = bits.saturating_sub(low).min(64);
        *limb = match keep {
            0 => 0,
            64 => g.next_u64(),
            n => g.next_u64() >> (64 - n),
        };
        if bits > low && bits <= low + 64 {
            *limb |= 1 << (bits - 1 - low);
        }
        low += 64;
    }
    Wide::from_le_limbs(limbs)
}

/// `w` or `−w`. Every magnitude built here is below 2^(64L − 1), so the negation always fits.
fn with_sign<const L: usize>(w: Wide<L>, negative: bool) -> Wide<L> {
    if negative {
        w.checked_neg().unwrap_or(w)
    } else {
        w
    }
}

/// A `Wide<L>` with `bits` significant bits and a pseudo-random sign.
fn signed<const L: usize>(g: &mut SplitMix64, bits: u32) -> Wide<L> {
    let w = magnitude::<L>(g, bits);
    with_sign(w, g.sign() < 0)
}

/// Bits of a typical operand: an `i128` value of 64 to 126 bits.
fn typical_bits(g: &mut SplitMix64) -> u32 {
    // 64..=126 fits u32 exactly.
    u32::try_from(g.range(64, 126)).unwrap_or(64)
}

/// The bit width of `Wide<L>`, 64L (at most 512 here, so the conversion is exact).
const fn width<const L: usize>() -> u32 {
    // L is at most 8 in this crate, so 64L fits u32.
    (64 * L) as u32
}

/// Operand pairs for [`Wide::checked_add`].
#[must_use]
pub fn add<const L: usize>(class: Class) -> Vec<(Wide<L>, Wide<L>)> {
    sum_pairs::<L>(&format!("wide::add::{L}"), class, false)
}

/// Operand pairs for [`Wide::checked_sub`].
#[must_use]
pub fn sub<const L: usize>(class: Class) -> Vec<(Wide<L>, Wide<L>)> {
    sum_pairs::<L>(&format!("wide::sub::{L}"), class, true)
}

/// Pairs for addition (`subtract` false) or subtraction (`subtract` true), from the seed `op`.
pub(crate) fn sum_pairs<const L: usize>(
    op: &str,
    class: Class,
    subtract: bool,
) -> Vec<(Wide<L>, Wide<L>)> {
    let mut g = rng(op, class);
    let w = width::<L>();
    match class {
        Class::Typical => batch_of(|_| {
            let (x, y) = (typical_bits(&mut g), typical_bits(&mut g));
            Some((signed::<L>(&mut g, x), signed::<L>(&mut g, y)))
        }),
        // Both magnitudes below 2^(64L − 2): the sum or difference is below 2^(64L − 1).
        Class::Extreme => {
            batch_of(|_| Some((signed::<L>(&mut g, w - 2), signed::<L>(&mut g, w - 2))))
        }
        // Both magnitudes at least 2^(64L − 2), combined so they add up: the exact result is at
        // least 2^(64L − 1) in magnitude, and it equals the minimum only if both magnitudes are
        // exactly 2^(64L − 2), which the pseudo-random low bits rule out (the tests check it).
        Class::Overflow => batch_of(|_| {
            let negative = g.sign() < 0;
            let x = with_sign(magnitude::<L>(&mut g, w - 1), negative);
            let y = with_sign(magnitude::<L>(&mut g, w - 1), negative != subtract);
            Some((x, y))
        }),
        Class::Degenerate | Class::NearlyDegenerate | Class::OnGrid | Class::WrongDirection => {
            Vec::new()
        }
    }
}

/// Operand pairs for [`Wide::checked_mul`].
#[must_use]
pub fn mul<const L: usize>(class: Class) -> Vec<(Wide<L>, Wide<L>)> {
    mul_pairs::<L>(&format!("wide::mul::{L}"), class)
}

/// Pairs for multiplication, from the seed `op`.
pub(crate) fn mul_pairs<const L: usize>(op: &str, class: Class) -> Vec<(Wide<L>, Wide<L>)> {
    let mut g = rng(op, class);
    let half = width::<L>() / 2;
    match class {
        // Operands of up to 126 bits multiply to up to 252: four limbs or more hold every product.
        Class::Typical if width::<L>() >= 256 => batch_of(|_| {
            let (x, y) = (typical_bits(&mut g), typical_bits(&mut g));
            Some((signed::<L>(&mut g, x), signed::<L>(&mut g, y)))
        }),
        // Magnitudes in [2^(32L − 2), 2^(32L − 1)): the product is below 2^(64L − 2).
        Class::Extreme => {
            batch_of(|_| Some((signed::<L>(&mut g, half - 1), signed::<L>(&mut g, half - 1))))
        }
        // Magnitudes at least 2^(32L): the product is at least 2^(64L).
        Class::Overflow => {
            batch_of(|_| Some((signed::<L>(&mut g, half + 1), signed::<L>(&mut g, half + 1))))
        }
        Class::Typical
        | Class::Degenerate
        | Class::NearlyDegenerate
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Values for [`Wide::checked_neg`].
#[must_use]
pub fn neg<const L: usize>(class: Class) -> Vec<Wide<L>> {
    neg_values::<L>(&format!("wide::neg::{L}"), class)
}

/// Values for negation, from the seed `op`.
pub(crate) fn neg_values<const L: usize>(op: &str, class: Class) -> Vec<Wide<L>> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let bits = typical_bits(&mut g);
            Some(signed::<L>(&mut g, bits))
        }),
        Class::Extreme => batch_of(|_| Some(signed::<L>(&mut g, width::<L>() - 1))),
        Class::Overflow => batch_of(|_| Some(Wide::<L>::min_value())),
        Class::Degenerate | Class::NearlyDegenerate | Class::OnGrid | Class::WrongDirection => {
            Vec::new()
        }
    }
}

/// Pairs for [`Ord::cmp`] on `Wide<L>`.
#[must_use]
pub fn cmp<const L: usize>(class: Class) -> Vec<(Wide<L>, Wide<L>)> {
    cmp_pairs::<L>(&format!("wide::cmp::{L}"), class)
}

/// Pairs for comparison, from the seed `op`.
pub(crate) fn cmp_pairs<const L: usize>(op: &str, class: Class) -> Vec<(Wide<L>, Wide<L>)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let (x, y) = (typical_bits(&mut g), typical_bits(&mut g));
            Some((signed::<L>(&mut g, x), signed::<L>(&mut g, y)))
        }),
        // Same sign and the same limbs except the lowest (every fourth pair is equal): the
        // comparison reads every limb.
        Class::Extreme => batch_of(|n| {
            let x = signed::<L>(&mut g, width::<L>() - 1);
            let mut limbs = x.to_le_limbs();
            if n % 4 != 3
                && let Some(low) = limbs.first_mut()
            {
                *low = g.next_u64();
            }
            let y = Wide::from_le_limbs(limbs);
            (y.is_negative() == x.is_negative()).then_some((x, y))
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Values for [`Wide::signum`]: negative full-width values, zero, positive `i128` values and
/// positive values with only the top limb set, in rotation.
#[must_use]
pub fn signum<const L: usize>(class: Class) -> Vec<Wide<L>> {
    let mut g = rng(&format!("wide::signum::{L}"), class);
    match class {
        Class::Typical => batch_of(|n| {
            Some(match n % 4 {
                0 => with_sign(magnitude::<L>(&mut g, width::<L>() - 1), true),
                1 => Wide::ZERO,
                2 => {
                    let bits = typical_bits(&mut g);
                    magnitude::<L>(&mut g, bits)
                }
                _ => {
                    let mut limbs = [0u64; L];
                    if let Some(top) = limbs.last_mut() {
                        *top = (g.next_u64() >> 1).max(1);
                    }
                    Wide::from_le_limbs(limbs)
                }
            })
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Values for [`Wide::to_i128`]: `typical`, values that fit (the extremes `i128::MIN` and
/// `i128::MAX` first); `overflow`, values of 128 to 64L − 1 bits, which do not. At two limbs every
/// value fits an `i128`, so that overflow batch is empty.
#[must_use]
pub fn to_i128<const L: usize>(class: Class) -> Vec<Wide<L>> {
    let mut g = rng(&format!("wide::to_i128::{L}"), class);
    match class {
        Class::Typical => batch_of(|n| {
            Some(match n {
                0 => Wide::from_i128(i128::MIN),
                1 => Wide::from_i128(i128::MAX),
                _ => {
                    let bits = typical_bits(&mut g);
                    signed::<L>(&mut g, bits)
                }
            })
        }),
        // 128 ≤ bits ≤ 64L − 1 needs L ≥ 3.
        Class::Overflow if width::<L>() > 128 => batch_of(|_| {
            let top = i64::from(width::<L>() - 1);
            let bits = u32::try_from(g.range(128, top)).unwrap_or(128);
            Some(signed::<L>(&mut g, bits))
        }),
        Class::Overflow
        | Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Values for [`Wide::from_i128`]: magnitudes of 1 to 127 bits with pseudo-random signs, with
/// `i128::MIN` and `i128::MAX` first.
#[must_use]
pub fn from_i128<const L: usize>(class: Class) -> Vec<i128> {
    let mut g = rng(&format!("wide::from_i128::{L}"), class);
    match class {
        Class::Typical => batch_of(|n| {
            Some(match n {
                0 => i128::MIN,
                1 => i128::MAX,
                _ => {
                    // A magnitude with exactly `bits` significant bits, 1 ≤ bits ≤ 127, so it and
                    // its negation fit an i128.
                    let bits = u32::try_from(g.range(1, 127)).unwrap_or(1);
                    let random = (u128::from(g.next_u64()) << 64) | u128::from(g.next_u64());
                    let low = i128::try_from(random >> (128 - bits)).unwrap_or(0);
                    let v = low | (1i128 << (bits - 1));
                    if g.sign() < 0 { -v } else { v }
                }
            })
        }),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// Values for [`Wide::resize`] from `Wide<L>` to `Wide<M>`: `typical`, values that fit `Wide<M>`
/// using every limb they can (full width when widening); `overflow` (narrowing only), values too
/// wide for `Wide<M>`.
#[must_use]
pub fn resize<const L: usize, const M: usize>(class: Class) -> Vec<Wide<L>> {
    let mut g = rng(&format!("wide::to_i{}::{L}", 64 * M), class);
    let fits = width::<L>().min(width::<M>()) - 1;
    match class {
        Class::Typical => batch_of(|_| Some(signed::<L>(&mut g, fits))),
        Class::Overflow if M < L => batch_of(|_| {
            let low = i64::from(width::<M>());
            let high = i64::from(width::<L>() - 1);
            let bits = u32::try_from(g.range(low, high)).unwrap_or(width::<M>());
            Some(signed::<L>(&mut g, bits))
        }),
        Class::Overflow
        | Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Extreme
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}
