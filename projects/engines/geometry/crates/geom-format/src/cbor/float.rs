//! Half, single and double precision floats: reading each width exactly, and finding the shortest
//! width that holds a value.
//!
//! What it does: [`half_to_f64`] and [`single_to_f64`] read a half or single precision float into
//! the double that holds it exactly. [`shortest`] says which of the three widths is the shortest
//! that holds a double exactly, with its bits: the one width RFC 8949's preferred serialization
//! allows for a float (section 4.1), which section 4.2.1's deterministic form requires and SKG
//! keeps (24.3).
//!
//! Guarantees:
//!
//! - **Exact both ways.** A half or single precision value reads as the double of exactly the same
//!   value, `−0.0` included; [`shortest`] gives bits that read back to the same double, bit for
//!   bit.
//! - **The shortest width.** [`shortest`] gives half precision whenever a half holds the value,
//!   otherwise single precision whenever a single holds it, otherwise double precision.
//! - **Finite only.** A NaN or an infinity has no width here: the reads return `None` for them,
//!   and [`shortest`] refuses them.
//!
//! Method: IEEE 754-2019's binary16, binary32 and binary64 interchange formats, and RFC 8949
//! section 4.1's preferred serialization of floats (<https://www.rfc-editor.org/rfc/rfc8949#section-4.1>).
//! A half's 15 magnitude bits, shifted up 13 places, are the bits of the single precision float
//! `2^−112` times its value, for normal and subnormal halves alike (the exponent biases differ by
//! 127 − 15 = 112, and both formats' subnormals sit one step below their least normal exponent),
//! so a half is read through the exact conversion of that single and one exact scaling by `2^112`.
//! Whether a single holds a double is Rust's round-to-nearest conversion compared with the
//! double, bit for bit; whether a half holds it is the same map read backwards.
//!
//! Cost: a few integer and float operations; nothing allocates; no loops.

/// 2^112, the ratio between a half precision value and the single precision float whose bits are
/// the half's 15 magnitude bits shifted up 13 places (the exponent biases are 15 and 127): the
/// double with exponent field 1023 + 112 and no fraction bits.
const TWO_POW_112: f64 = f64::from_bits((1023 + 112) << 52);

/// 2^−112, the inverse of [`TWO_POW_112`]: exponent field 1023 − 112.
const TWO_POW_NEG_112: f64 = f64::from_bits((1023 - 112) << 52);

/// A half's exponent field all ones: an infinity or a NaN.
const HALF_EXPONENT: u16 = 0x7c00;

/// The magnitude bits of a half.
const HALF_MAGNITUDE: u16 = 0x7fff;

/// The magnitude bits of a single.
const SINGLE_MAGNITUDE: u32 = 0x7fff_ffff;

/// The single precision bits of the largest finite half, 65504, scaled by 2^−112: its magnitude
/// bits `0x7bff` shifted up 13 places.
const SINGLE_OF_LARGEST_HALF: u32 = 0x0f7f_e000;

/// The 13 low bits a single has and a half does not.
const BELOW_HALF: u32 = 0x1fff;

/// The width and bits of a float in the shortest encoding that holds its value exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    /// Half precision, CBOR's `0xf9` head.
    Half(u16),
    /// Single precision, CBOR's `0xfa` head.
    Single(u32),
    /// Double precision, CBOR's `0xfb` head.
    Double(u64),
}

/// The double holding exactly the value of the half precision float with these bits, or `None`
/// for an infinity or a NaN.
#[must_use]
pub fn half_to_f64(bits: u16) -> Option<f64> {
    if bits & HALF_EXPONENT == HALF_EXPONENT {
        return None;
    }
    let scaled = f32::from_bits(u32::from(bits & HALF_MAGNITUDE) << 13);
    let magnitude = f64::from(scaled) * TWO_POW_112;
    Some(if bits & !HALF_MAGNITUDE == 0 {
        magnitude
    } else {
        -magnitude
    })
}

/// The double holding exactly the value of the single precision float with these bits, or `None`
/// for an infinity or a NaN.
#[must_use]
pub fn single_to_f64(bits: u32) -> Option<f64> {
    let x = f32::from_bits(bits);
    x.is_finite().then(|| f64::from(x))
}

/// The single precision float holding exactly the value of `x`, if one does.
fn single(x: f64) -> Option<f32> {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "rounds to the nearest single; the comparison below keeps it only when no bit was lost"
    )]
    let s = x as f32;
    (f64::from(s).to_bits() == x.to_bits()).then_some(s)
}

/// The bits of the half precision float holding exactly the value of `x`, if one does.
fn half(x: f64) -> Option<u16> {
    let sign: u16 = if x.is_sign_negative() { 0x8000 } else { 0 };
    if x == 0.0 {
        return Some(sign);
    }
    // A single holds every half, and a nonzero single is at least 2^−149 in magnitude, so
    // x·2^−112 is a normal double and the scaling is exact.
    single(x)?;
    let magnitude = single(x * TWO_POW_NEG_112)?.to_bits() & SINGLE_MAGNITUDE;
    if magnitude & BELOW_HALF != 0 || magnitude > SINGLE_OF_LARGEST_HALF {
        return None;
    }
    u16::try_from(magnitude >> 13).ok().map(|m| m | sign)
}

/// The shortest of half, single and double precision that holds `x` exactly, with its bits; `None`
/// for an infinity or a NaN, which SKG never stores (24.3).
#[must_use]
pub fn shortest(x: f64) -> Option<Width> {
    if !x.is_finite() {
        return None;
    }
    Some(if let Some(h) = half(x) {
        Width::Half(h)
    } else if let Some(s) = single(x) {
        Width::Single(s.to_bits())
    } else {
        Width::Double(x.to_bits())
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::arithmetic_side_effects,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss
    )]

    use super::{TWO_POW_112, TWO_POW_NEG_112, Width, half_to_f64, shortest, single_to_f64};

    /// 2^k exactly, for −1074 ≤ k ≤ 1023, from its bits.
    fn p2(k: i32) -> f64 {
        if k >= -1022 {
            f64::from_bits(((k + 1023) as u64) << 52)
        } else {
            f64::from_bits(1 << (k + 1074))
        }
    }

    /// The half precision value of `bits` from its definition: (−1)^s · 2^(e−15) · (1 + f/1024)
    /// for a normal half, (−1)^s · 2^−14 · f/1024 for a subnormal one.
    fn half_by_definition(bits: u16) -> Option<f64> {
        let s = if bits >> 15 == 1 { -1.0 } else { 1.0 };
        let e = i32::from((bits >> 10) & 0x1f);
        let f = f64::from(bits & 0x3ff);
        match e {
            31 => None,
            0 => Some(s * f / 1024.0 * p2(-14)),
            _ => Some(s * (1.0 + f / 1024.0) * p2(e - 15)),
        }
    }

    #[test]
    fn the_scaling_constants_are_powers_of_two() {
        assert_eq!(TWO_POW_112.to_bits(), (1023 + 112) << 52);
        assert_eq!(TWO_POW_NEG_112.to_bits(), (1023 - 112) << 52);
    }

    #[test]
    fn every_half_reads_as_its_value_and_is_its_own_shortest_width() {
        for bits in 0..=u16::MAX {
            let read = half_to_f64(bits);
            let defined = half_by_definition(bits);
            assert_eq!(
                read.map(f64::to_bits),
                defined.map(f64::to_bits),
                "{bits:#06x}"
            );
            if let Some(x) = read {
                assert_eq!(shortest(x), Some(Width::Half(bits)), "{bits:#06x}");
            }
        }
    }

    #[test]
    fn single_reads_refuse_exactly_the_non_finite() {
        for bits in [
            0x7f80_0000_u32,
            0xff80_0000,
            0x7fc0_0000,
            0x7f80_0001,
            0xffff_ffff,
        ] {
            assert_eq!(single_to_f64(bits), None, "{bits:#010x}");
        }
        assert_eq!(
            single_to_f64(0x8000_0000).map(f64::to_bits),
            Some((-0.0f64).to_bits())
        );
        assert_eq!(single_to_f64(1), Some(p2(-149)));
    }

    #[test]
    fn edges_of_each_width() {
        let half = |x: f64| matches!(shortest(x), Some(Width::Half(_)));
        let single = |x: f64| matches!(shortest(x), Some(Width::Single(_)));
        let double = |x: f64| matches!(shortest(x), Some(Width::Double(_)));
        assert!(half(65504.0) && half(-65504.0) && half(p2(-24)) && half(-0.0));
        assert!(single(65520.0) && single(65505.0) && single(p2(-25)));
        assert!(single(1.0 + p2(-23)) && half(1.0 + p2(-10)));
        assert!(single(1.0 + p2(-11)) && single(p2(-149)));
        assert!(single(f64::from(f32::MAX)) && double(f64::from(f32::MAX) * 2.0));
        assert!(double(p2(-150)) && double(1.0 + p2(-24)) && double(0.1));
        assert!(double(f64::MIN_POSITIVE) && double(5e-324) && double(f64::MAX));
        assert!(shortest(f64::NAN).is_none() && shortest(f64::INFINITY).is_none());
        assert!(shortest(f64::NEG_INFINITY).is_none());
        assert_eq!(shortest(-0.0), Some(Width::Half(0x8000)));
        assert_eq!(shortest(0.0), Some(Width::Half(0)));
    }
}
