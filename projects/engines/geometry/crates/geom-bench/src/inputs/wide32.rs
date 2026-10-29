//! Inputs for the fixed-width integers in 32-bit limbs, `Wide32<M>` (piece G42): for each
//! operation, the batch of the `Wide` operation of the same bit width ([`super::wide`]), drawn
//! from the `Wide32` benchmark's own seed, each value split into twice as many 32-bit limbs; and
//! pairs of `i64` for [`Wide32::product_i64`].
//!
//! Classes: as [`super::wide`] describes them for each operation, at 64L = 32M bits; for
//! `product_i64` into three limbs: `typical`, two operands of 41 bits, the products of grid
//! translations the limb variant forms most; `extreme`, operands of 47 and 48 bits, whose product
//! has 94 or 95 bits and fits; `overflow`, operands of 48 and 49 bits, whose product is at least
//! 2^95 and does not.
//!
//! Method: the operand classes of schoolbook multiple-precision arithmetic, as [`super::wide`]
//! (Knuth, *The Art of Computer Programming*, volume 2, section 4.3.1,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>); a value's 64-bit limb k is its 32-bit
//! limbs 2k and 2k + 1.
//!
//! Guarantees: every batch is full where its class exists, and the same on every target; each
//! value is the value of the `Wide` input it was split from, so its class is that input's
//! (`tests/inputs.rs`); every result is checked by `tests/attack_g06.rs`.
//!
//! Cost: the `Wide` batch's draws and one pass to split it, outside every measured run.

use geom_num::wide::Wide;
use geom_num::wide32::Wide32;

use super::wide as w;
use super::{Class, batch_of, rng};

/// `v`, its 64-bit limbs split into 32-bit limbs, sign-extended to `M` limbs (`M` is 2L in every
/// batch here).
#[must_use]
pub fn split<const L: usize, const M: usize>(v: &Wide<L>) -> Wide32<M> {
    let fill = if v.is_negative() { u32::MAX } else { 0 };
    let mut out = [fill; M];
    for (k, limb) in v.to_le_limbs().iter().enumerate() {
        let [b0, b1, b2, b3, b4, b5, b6, b7] = limb.to_le_bytes();
        if let Some(low) = out.get_mut(2 * k) {
            *low = u32::from_le_bytes([b0, b1, b2, b3]);
        }
        if let Some(high) = out.get_mut(2 * k + 1) {
            *high = u32::from_le_bytes([b4, b5, b6, b7]);
        }
    }
    Wide32::from_le_limbs(out)
}

fn pairs<const L: usize, const M: usize>(v: &[(Wide<L>, Wide<L>)]) -> Vec<(Wide32<M>, Wide32<M>)> {
    v.iter().map(|(x, y)| (split(x), split(y))).collect()
}

fn values<const L: usize, const M: usize>(v: &[Wide<L>]) -> Vec<Wide32<M>> {
    v.iter().map(split).collect()
}

/// Operand pairs for [`Wide32::checked_add`] at 32M = 64L bits.
#[must_use]
pub fn add<const L: usize, const M: usize>(class: Class) -> Vec<(Wide32<M>, Wide32<M>)> {
    pairs(&w::sum_pairs::<L>(
        &format!("wide32::w{M}_add"),
        class,
        false,
    ))
}

/// Operand pairs for [`Wide32::checked_sub`].
#[must_use]
pub fn sub<const L: usize, const M: usize>(class: Class) -> Vec<(Wide32<M>, Wide32<M>)> {
    pairs(&w::sum_pairs::<L>(
        &format!("wide32::w{M}_sub"),
        class,
        true,
    ))
}

/// Operand pairs for [`Wide32::product`] at one width.
#[must_use]
pub fn product<const L: usize, const M: usize>(class: Class) -> Vec<(Wide32<M>, Wide32<M>)> {
    pairs(&w::mul_pairs::<L>(&format!("wide32::w{M}_product"), class))
}

/// Values for [`Wide32::checked_neg`].
#[must_use]
pub fn neg<const L: usize, const M: usize>(class: Class) -> Vec<Wide32<M>> {
    values(&w::neg_values::<L>(&format!("wide32::w{M}_neg"), class))
}

/// Pairs for [`Ord::cmp`] on `Wide32<M>`.
#[must_use]
pub fn cmp<const L: usize, const M: usize>(class: Class) -> Vec<(Wide32<M>, Wide32<M>)> {
    pairs(&w::cmp_pairs::<L>(&format!("wide32::w{M}_cmp"), class))
}

/// Values for [`Wide32::signum`].
#[must_use]
pub fn signum<const L: usize, const M: usize>(class: Class) -> Vec<Wide32<M>> {
    values(&w::signum_values::<L>(
        &format!("wide32::w{M}_signum"),
        class,
    ))
}

/// Values for [`Wide32::to_i128`].
#[must_use]
pub fn to_i128<const L: usize, const M: usize>(class: Class) -> Vec<Wide32<M>> {
    values(&w::to_i128_values::<L>(
        &format!("wide32::w{M}_to_i128"),
        class,
    ))
}

/// Values for [`Wide32::from_i128`].
#[must_use]
pub fn from_i128<const M: usize>(class: Class) -> Vec<i128> {
    w::from_i128_values(&format!("wide32::w{M}_from_i128"), class)
}

/// Values for [`Wide32::resize`] from 2L to 2K limbs, as [`super::wide::resize`] from `Wide<L>`
/// to `Wide<K>`.
#[must_use]
pub fn resize<const L: usize, const K: usize, const M: usize>(class: Class) -> Vec<Wide32<M>> {
    values(&w::resize_values::<L, K>(
        &format!("wide32::w{M}_to_w{}", 2 * K),
        class,
    ))
}

/// Values for [`Wide32::to_wide`] at one bit width: every value fits.
#[must_use]
pub fn to_wide<const L: usize, const M: usize>(class: Class) -> Vec<Wide32<M>> {
    values(&w::resize_values::<L, L>(
        &format!("wide32::w{M}_to_wide"),
        class,
    ))
}

/// Pairs for [`Wide32::product_i64`] into three limbs.
#[must_use]
pub fn product_i64(class: Class) -> Vec<(i64, i64)> {
    let mut g = rng("wide32::w3_product_i64", class);
    let (a, b) = match class {
        Class::Typical => (41, 41),
        Class::Extreme => (47, 48),
        Class::Overflow => (48, 49),
        Class::Degenerate | Class::NearlyDegenerate | Class::OnGrid | Class::WrongDirection => {
            return Vec::new();
        }
    };
    // A value with exactly `bits` significant bits and a pseudo-random sign.
    let mut operand = |bits: u32| {
        let low = i64::try_from(g.next_u64() >> (65 - bits)).unwrap_or(0);
        let v = low | (1i64 << (bits - 1));
        if g.sign() < 0 { -v } else { v }
    };
    batch_of(|_| Some((operand(a), operand(b))))
}
