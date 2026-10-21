//! Tests across the whole range of doubles, against exact rational arithmetic.
//!
//! What this file proves:
//!
//! - Double-double: every operation on operands of any magnitude, subnormal leading parts
//!   included, is within its proven bound from the `dd` module's table, plus the documented
//!   2^−1070 relative and 2^−1073 absolute terms; it is refused only when the exact result
//!   overflows; every result is normalized.
//! - Float vectors: `length` is within 2.01u (`F2`) or 2.51u (`F3`) of the exact length, plus the
//!   documented 2^−1070, for vectors of any magnitude; `normalized` gives a unit vector in the right
//!   direction for every nonzero vector, however short or long.
//! - General transforms: classification is unchanged when a matrix is scaled by any power of two
//!   (a rigid one becomes a uniform scale), so huge or tiny determinants are no longer
//!   `Degenerate`; the inverse of a scaled matrix is the scaled inverse, bit for bit.

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
use geom_math::{DD, F2, F3, Transform, TransformKind};
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

/// A normalized double-double whose leading part may be anything, with a trailing part that is
/// zero, half an ulp, or random.
fn any_dd() -> BoxedStrategy<DD> {
    (any_double(), any::<u64>(), 0u32..4)
        .prop_map(|(hi, bits, kind)| {
            if hi == 0.0 || !hi.is_normal() {
                return DD::from_f64(hi).unwrap();
            }
            let e = ((hi.to_bits() >> 52) & 0x7ff) as i32 - 1023;
            let lo = match kind {
                0 => 0.0,
                // Half an ulp of hi (a tie), when it is a double.
                1 => pow2f(e - 53),
                _ => {
                    let k = e - 54 - i32::try_from(bits % 60).unwrap();
                    let m = (bits >> 8) & ((1u64 << 52) - 1);
                    if k >= -1022 {
                        f64::from_bits((u64::try_from(k + 1023).unwrap() << 52) | m)
                    } else {
                        pow2f(k)
                    }
                }
            };
            let lo = if bits & 1 == 1 { -lo } else { lo };
            DD::from_sum(hi, lo).unwrap_or(DD::from_f64(hi).unwrap())
        })
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

fn value(d: DD) -> BigRational {
    exact(d.hi()) + exact(d.lo())
}

fn normalized(d: DD) -> bool {
    d.hi() + d.lo() == d.hi() && (d.hi() != 0.0 || d.lo() == 0.0)
}

fn u2() -> BigRational {
    pow2(-106)
}

fn u3() -> BigRational {
    pow2(-159)
}

/// The largest double, exactly.
fn max_double() -> BigRational {
    exact(f64::MAX)
}

/// Checks one result: within bound·|want| + 2^−1070·|want| + 2^−1073 of `want`, normalized; or
/// refused as not finite only when `want` is at least about the largest double.
fn check(
    name: &str,
    got: Result<DD, NumError>,
    want: &BigRational,
    bound: &BigRational,
) -> Result<(), TestCaseError> {
    match got {
        Ok(d) => {
            prop_assert!(normalized(d), "{name}: not normalized {d:?}");
            let tol = bound * want.abs() + pow2(-1070) * want.abs() + pow2(-1073);
            let err = (value(d) - want).abs();
            prop_assert!(
                err <= tol,
                "{name}: {d:?} off by {} of {}",
                err_f(&err),
                err_f(want)
            );
        }
        Err(NumError::NotFinite) => {
            let edge = max_double() * (int(1) - pow2(-90));
            prop_assert!(
                want.abs() >= edge,
                "{name}: refused a finite result {}",
                err_f(want)
            );
        }
        Err(e) => prop_assert!(false, "{name}: unexpected {e:?}"),
    }
    Ok(())
}

fn err_f(x: &BigRational) -> f64 {
    use num_traits::ToPrimitive;
    x.to_f64().unwrap_or(f64::NAN)
}

proptest! {
    #![proptest_config(config(3000))]

    #[test]
    fn double_double_over_the_whole_range(a in any_dd(), b in any_dd(), y in any_double()) {
        let (va, vb, vy) = (value(a), value(b), exact(y));
        let b_add = int(6) * u2() + int(45) * u3();
        let b_addf = int(3) * u2() + int(10) * u3();
        let b_mulf = int(3) * u2() + int(7) * u3();
        let b_mul = int(8) * u2() + int(23) * u3();
        let b_div = int(6) * u2() + int(79) * u3();
        // Additions lose nothing to underflow: no extra terms are needed, but the same check
        // with them is used for uniformity.
        check("add", a.add(b), &(&va + &vb), &b_add)?;
        check("add_f64", a.add_f64(y), &(&va + &vy), &b_addf)?;
        check("mul_f64", a.mul_f64(y), &(&va * &vy), &b_mulf)?;
        check("mul", a.mul(b), &(&va * &vb), &b_mul)?;
        if !vb.is_zero() {
            check("div", a.div(b), &(&va / &vb), &b_div)?;
        }
        // The square root, by squaring: r within (4.13u² + 2^−1070)·√a + 2^−1073 of √a.
        let pa = if a.hi() < 0.0 { -a } else { a };
        let r = pa.sqrt().unwrap();
        prop_assert!(normalized(r));
        let x = value(pa);
        let rv = value(r);
        let beta = BigRational::new(413.into(), 100.into()) * u2() + pow2(-1070);
        // |r − √x| ≤ β·√x + 2^−1073 ⇔ √x ∈ [(r − 2^−1073)/(1 + β), (r + 2^−1073)/(1 − β)].
        let lo = (&rv - pow2(-1073)) / (int(1) + &beta);
        let hi = (&rv + pow2(-1073)) / (int(1) - &beta);
        let lo = if lo.is_negative() { BigRational::zero() } else { lo };
        prop_assert!(&lo * &lo <= x && x <= &hi * &hi, "sqrt({pa:?}) = {r:?}");
    }
}

#[test]
fn double_double_documented_edges() {
    let p = pow2f;
    // Products and quotients that the unscaled algorithms refused (two_prod's range).
    let big = DD::from_f64(p(1000)).unwrap();
    assert_eq!(big.mul_f64(0.5).unwrap().to_f64(), p(999));
    assert_eq!(
        DD::from_f64(p(997))
            .unwrap()
            .mul(DD::from_f64(0.25).unwrap())
            .unwrap()
            .to_f64(),
        p(995)
    );
    assert_eq!(
        big.div(DD::from_f64(4.0).unwrap()).unwrap().to_f64(),
        p(998)
    );
    assert_eq!(
        DD::from_f64(1.0).unwrap().div(big).unwrap().to_f64(),
        p(-1000)
    );
    assert!(DD::from_f64(f64::MAX).unwrap().sqrt().is_ok());
    // A product of subnormal and huge operands.
    let tiny = DD::from_f64(f64::from_bits(3)).unwrap();
    assert_eq!(tiny.mul_f64(p(1023)).unwrap().to_f64(), 3.0 * p(-51));
    assert_eq!(
        tiny.mul(DD::from_f64(p(1000)).unwrap()).unwrap().to_f64(),
        3.0 * p(-74)
    );
    assert_eq!(
        tiny.sqrt().unwrap().to_f64(),
        math::sqrt(3.0).unwrap().get() * p(-537)
    );
    // Overflow is refused; a product that underflows to zero is zero.
    assert_eq!(big.mul(big), Err(NumError::NotFinite));
    assert_eq!(
        DD::from_f64(f64::MAX)
            .unwrap()
            .div(DD::from_f64(0.5).unwrap()),
        Err(NumError::NotFinite)
    );
    assert_eq!(tiny.mul_f64(p(-100)).unwrap(), DD::ZERO);
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
