//! Tests across the whole range of doubles, against exact rational arithmetic.
//!
//! What this file proves:
//!
//! - Float vectors: `length` is within 2.01u (`F2`) or 2.51u (`F3`) of the exact length, plus the
//!   documented 2^−1070, for vectors of any magnitude; `normalized` gives a unit vector in the right
//!   direction for every nonzero vector, however short or long.
//! - General transforms: classification is unchanged when a matrix is scaled by any power of two
//!   (a rigid one becomes a uniform scale), so huge or tiny determinants are no longer
//!   `Degenerate`; the inverse of a scaled matrix is the scaled inverse, bit for bit.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    clippy::single_match_else,
    clippy::unreadable_literal,
    reason = "test cases and oracles, named as in the formulas they check; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(clippy::unwrap_used)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]

mod common;

use common::{config, exact, int, pow2};
use geom_math::{F2, F3, Transform, TransformKind};
use geom_num::{NumError, math};
use num_rational::BigRational;
use num_traits::{Signed, Zero};
use proptest::prelude::*;

/// A double of any finite magnitude: normal at any exponent, subnormal, or zero.
fn any_double() -> BoxedStrategy<f64> {
    prop_oneof![
        8 => (any::<u64>(), -1022i32..=1023, any::<bool>()).prop_map(|(bits, e, neg)| {
            let frac = match bits % 5 {
                0 => 0,
                1 => (1u64 << 52) - 1,
                _ => bits & ((1u64 << 52) - 1),
            };
            let v = f64::from_bits((u64::try_from(e + 1023).unwrap() << 52) | frac);
            if neg { -v } else { v }
        }),
        1 => (1u64..(1u64 << 52), any::<bool>())
            .prop_map(|(f, neg)| if neg { -f64::from_bits(f) } else { f64::from_bits(f) }),
        1 => Just(0.0f64),
    ]
    .boxed()
}

/// 2^k as a double for −1074 ≤ k ≤ 1023 (subnormal powers included), 0 below.
fn pow2f(k: i32) -> f64 {
    if k >= -1022 {
        f64::from_bits(u64::try_from(k + 1023).unwrap() << 52)
    } else if k >= -1074 {
        f64::from_bits(1u64 << (k + 1074))
    } else {
        0.0
    }
}

/// The largest double, exactly.
fn max_double() -> BigRational {
    exact(f64::MAX)
}

/// |len − √S| ≤ rel·√S + 2^−1073, checked by squaring (S = the exact squared length).
fn length_ok(len: f64, s: &BigRational, rel: &BigRational) -> bool {
    let l = exact(len);
    let lo = (&l - pow2(-1073)) / (int(1) + rel);
    let hi = (&l + pow2(-1073)) / (int(1) - rel);
    let lo = if lo.is_negative() {
        BigRational::zero()
    } else {
        lo
    };
    &lo * &lo <= *s && *s <= &hi * &hi
}

proptest! {
    #![proptest_config(config(4096))]

    #[test]
    fn float_lengths_over_the_whole_range(
        x in any_double(),
        y in any_double(),
        z in any_double(),
        shift in -40i32..=40,
    ) {
        // Components of related magnitude too: z scaled from x.
        let z = if shift == 0 { z } else { x * pow2f(shift) };
        let z = if z.is_finite() { z } else { x };
        let u = pow2(-53);
        let rel2 = int(201) / int(100) * &u + pow2(-1070);
        let rel3 = int(251) / int(100) * &u + pow2(-1070);
        let s2 = exact(x) * exact(x) + exact(y) * exact(y);
        let s3 = &s2 + exact(z) * exact(z);
        let v2 = F2::new(x, y).unwrap();
        let v3 = F3::new(x, y, z).unwrap();
        match v2.length() {
            Ok(l) => prop_assert!(length_ok(l, &s2, &rel2), "F2({x:e}, {y:e}).length() = {l:e}"),
            Err(NumError::NotFinite) => prop_assert!(s2 > max_double() * max_double() * (int(1) - pow2(-50))),
            Err(e) => prop_assert!(false, "{e:?}"),
        }
        match v3.length() {
            Ok(l) => prop_assert!(length_ok(l, &s3, &rel3), "F3 length {l:e}"),
            Err(NumError::NotFinite) => prop_assert!(s3 > max_double() * max_double() * (int(1) - pow2(-50))),
            Err(e) => prop_assert!(false, "{e:?}"),
        }
        // Normalized: a unit vector, parallel to the input, for every nonzero vector.
        for (n, v) in [
            (v2.normalized().map(|n| vec![n.x(), n.y()]), vec![x, y]),
            (v3.normalized().map(|n| n.to_array().to_vec()), vec![x, y, z]),
        ] {
            if v.iter().all(|c| *c == 0.0) {
                prop_assert_eq!(n, Err(NumError::Degenerate));
                continue;
            }
            let n = n.unwrap();
            let norm2: BigRational = n.iter().map(|c| exact(*c) * exact(*c)).sum();
            prop_assert!((norm2 - int(1)).abs() <= pow2(-49), "|{n:?}|² for {v:?}");
            // Parallel: n_i·v_j = n_j·v_i within a relative 2^−49 of the largest product.
            let scale = v.iter().map(|c| exact(*c).abs()).fold(BigRational::zero(), |a, b| if b > a { b } else { a });
            for i in 0..v.len() {
                for j in 0..v.len() {
                    let d = exact(n[i]) * exact(v[j]) - exact(n[j]) * exact(v[i]);
                    prop_assert!(d.abs() <= &scale * pow2(-48), "{n:?} not parallel to {v:?}");
                }
                // Same sign as the input component.
                prop_assert!(n[i] == 0.0 || (n[i] > 0.0) == (v[i] > 0.0));
            }
        }
    }
}

/// A small dyadic entry (at most 20 significant bits), so scaling by 2^k stays exact.
fn small_entry() -> BoxedStrategy<f64> {
    prop_oneof![
        3 => (-8i32..=8).prop_map(f64::from),
        2 => (-(1i32 << 19)..(1i32 << 19)).prop_map(|v| f64::from(v) / 65_536.0),
    ]
    .boxed()
}

proptest! {
    #![proptest_config(config(2048))]

    #[test]
    fn classification_and_inverse_are_scale_invariant(
        m in prop::array::uniform3(prop::array::uniform3(small_entry())),
        k in prop_oneof![-1000i32..=1000, Just(340i32), Just(-366i32)],
        rot in -2.0f64..2.0,
    ) {
        let base = Transform::from_rows(m.map(|r| [r[0], r[1], r[2], 0.0])).unwrap();
        let s = pow2f(k / 2) * pow2f(k - k / 2);
        let scaled = Transform::from_rows(m.map(|r| [r[0] * s, r[1] * s, r[2] * s, 0.0])).unwrap();
        let (cb, cs) = (base.classify().unwrap(), scaled.classify().unwrap());
        prop_assert_eq!(cb.mirror, cs.mirror);
        // Scaling by 2^k ≠ 1 turns a rigid map (or the identity) into a uniform scale and changes
        // nothing else.
        let want = match cb.kind {
            TransformKind::Rigid | TransformKind::Identity | TransformKind::Translation if k != 0 => {
                TransformKind::UniformScale
            }
            other => other,
        };
        prop_assert_eq!(cs.kind, want, "{:?} scaled by 2^{}", m, k);
        // The inverse of the scaled matrix is the scaled inverse, while the entries stay normal.
        if cb.kind != TransformKind::Degenerate && k.abs() <= 900 {
            let (ib, is) = (base.inverse().unwrap(), scaled.inverse().unwrap());
            for (rb, rs) in ib.rows().iter().zip(is.rows().iter()) {
                for (a, b) in rb.iter().zip(rs.iter()).take(3) {
                    let want = a / s;
                    if want == 0.0 || want.is_normal() {
                        prop_assert_eq!(*b, want);
                    }
                }
            }
        }
        // A rotation scaled by 2^k is a uniform scale (rigid when k = 0).
        let r = Transform::rotation(geom_math::Axis::Z, rot).unwrap();
        let rs = Transform::uniform_scale(s).unwrap().compose(&r).unwrap();
        let want = if k == 0 { TransformKind::Rigid } else { TransformKind::UniformScale };
        prop_assert_eq!(rs.classify().unwrap().kind, want);
        prop_assert!(rs.inverse().is_ok());
    }
}

#[test]
fn uniform_scales_of_any_size_invert() {
    for s in [
        1e103,
        1e-110,
        1e300,
        1e-300,
        f64::MAX / 4.0,
        f64::MIN_POSITIVE * 4.0,
        3.0,
    ] {
        let t = Transform::uniform_scale(s).unwrap();
        let c = t.classify().unwrap();
        assert_eq!(
            (c.kind, c.mirror),
            (TransformKind::UniformScale, false),
            "scale {s:e}"
        );
        let inv = t.inverse().unwrap();
        let want = 1.0 / s;
        if want.is_normal() {
            let d = inv.rows()[0][0];
            assert!(
                math::abs(d - want).unwrap().get() <= want * pow2f(-52),
                "1/{s:e}: {d:e}"
            );
        }
        let m = Transform::uniform_scale(-s).unwrap().classify().unwrap();
        assert_eq!((m.kind, m.mirror), (TransformKind::UniformScale, true));
    }
    // Rows of very different sizes: certified by scaling each row.
    let t = Transform::scale(1e300, 1e-300, 1.0).unwrap();
    assert_eq!(t.classify().unwrap().kind, TransformKind::NonUniform);
    let inv = t.inverse().unwrap();
    assert_eq!(inv.rows()[2][2], 1.0);
    assert!(math::abs(inv.rows()[0][0] * 1e300 - 1.0).unwrap().get() < 1e-15);
}
