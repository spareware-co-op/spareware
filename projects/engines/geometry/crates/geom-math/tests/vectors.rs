//! Tests of `geom_math::vector` against exact big integers (`num-bigint`).
//!
//! What this file proves: dot, cross, triple products and squared lengths of grid vectors are the
//! exact integers, for random vectors and at every extreme (each component at ±(2^41 − 2), the
//! largest difference of two coordinates at ±(2^40 − 1)), and stay within their documented bit
//! budgets there; every one of them is `Ok`, so the checked arithmetic never reports
//! `BudgetExceeded` for a grid vector; differences of grid points are always in range; sums,
//! differences and moved points out of range are refused with `OutOfRange` carrying the value.

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

use common::config;
use geom_math::vector::VEC_MAX;
use geom_math::{V2, V3};
use geom_num::NumError;
use geom_num::grid::{COORD_MAX, P2, P3};
use num_bigint::BigInt;
use proptest::prelude::*;

/// A vector component, weighted towards the extremes and zero.
fn comp() -> BoxedStrategy<i64> {
    prop_oneof![
        4 => -VEC_MAX..=VEC_MAX,
        1 => Just(VEC_MAX),
        1 => Just(-VEC_MAX),
        1 => Just(0i64),
        1 => -3i64..=3,
    ]
    .boxed()
}

fn v3() -> BoxedStrategy<V3> {
    (comp(), comp(), comp())
        .prop_map(|(x, y, z)| V3::new(x, y, z).unwrap())
        .boxed()
}

fn v2() -> BoxedStrategy<V2> {
    (comp(), comp())
        .prop_map(|(x, y)| V2::new(x, y).unwrap())
        .boxed()
}

fn b(v: i64) -> BigInt {
    BigInt::from(v)
}

fn dot3(a: V3, c: V3) -> BigInt {
    b(a.x()) * b(c.x()) + b(a.y()) * b(c.y()) + b(a.z()) * b(c.z())
}

fn cross3(a: V3, c: V3) -> [BigInt; 3] {
    [
        b(a.y()) * b(c.z()) - b(a.z()) * b(c.y()),
        b(a.z()) * b(c.x()) - b(a.x()) * b(c.z()),
        b(a.x()) * b(c.y()) - b(a.y()) * b(c.x()),
    ]
}

/// The largest magnitude the documented budgets allow, as 2^bits.
fn below_pow2(v: i128, bits: u32) -> bool {
    v.unsigned_abs() < 1u128 << bits
}

proptest! {
    #![proptest_config(config(4096))]

    #[test]
    fn products_3d_are_exact(a in v3(), c in v3(), d in v3()) {
        prop_assert_eq!(BigInt::from(a.dot(c).unwrap()), dot3(a, c));
        let w = a.cross(c).unwrap();
        let [cx, cy, cz] = cross3(a, c);
        prop_assert_eq!(BigInt::from(w.x()), cx.clone());
        prop_assert_eq!(BigInt::from(w.y()), cy.clone());
        prop_assert_eq!(BigInt::from(w.z()), cz.clone());
        let triple = b(d.x()) * cx + b(d.y()) * cy + b(d.z()) * cz;
        prop_assert_eq!(BigInt::from(d.triple(a, c).unwrap()), triple.clone());
        prop_assert_eq!(BigInt::from(w.dot(d).unwrap()), triple);
        prop_assert_eq!(BigInt::from(a.norm2().unwrap()), dot3(a, a));
        prop_assert!(below_pow2(a.dot(c).unwrap(), 84));
        prop_assert!(below_pow2(w.x(), 83) && below_pow2(w.y(), 83) && below_pow2(w.z(), 83));
        prop_assert!(below_pow2(d.triple(a, c).unwrap(), 126));
        prop_assert_eq!(w.is_zero(), cross3(a, c).iter().all(|v| *v == BigInt::from(0)));
        let r = c.cross(a).unwrap();
        prop_assert_eq!((r.x(), r.y(), r.z()), (-w.x(), -w.y(), -w.z()));
    }

    #[test]
    fn products_2d_are_exact(a in v2(), c in v2()) {
        let (dot, cross) = (a.dot(c).unwrap(), a.cross(c).unwrap());
        prop_assert_eq!(BigInt::from(dot), b(a.x()) * b(c.x()) + b(a.y()) * b(c.y()));
        prop_assert_eq!(BigInt::from(cross), b(a.x()) * b(c.y()) - b(a.y()) * b(c.x()));
        prop_assert_eq!(cross, -c.cross(a).unwrap());
        prop_assert!(below_pow2(dot, 83) && below_pow2(cross, 83));
    }

    #[test]
    fn sums_are_checked(a in v3(), c in v3()) {
        let expect = |x: i64, y: i64| i128::from(x) + i128::from(y);
        match a.add(c) {
            Ok(s) => {
                prop_assert_eq!(i128::from(s.x()), expect(a.x(), c.x()));
                prop_assert_eq!(i128::from(s.y()), expect(a.y(), c.y()));
                prop_assert_eq!(i128::from(s.z()), expect(a.z(), c.z()));
            }
            Err(NumError::OutOfRange { value }) => {
                prop_assert!(value.unsigned_abs() > VEC_MAX.unsigned_abs().into());
            }
            Err(e) => prop_assert!(false, "unexpected error {e:?}"),
        }
        prop_assert_eq!(a.sub(c).ok(), a.add(c.neg().unwrap()).ok());
    }

    #[test]
    fn between_and_offset_round_trip(
        p in (-COORD_MAX..=COORD_MAX, -COORD_MAX..=COORD_MAX, -COORD_MAX..=COORD_MAX),
        q in (-COORD_MAX..=COORD_MAX, -COORD_MAX..=COORD_MAX, -COORD_MAX..=COORD_MAX),
    ) {
        let p = P3::new(p.0, p.1, p.2).unwrap();
        let q = P3::new(q.0, q.1, q.2).unwrap();
        let v = V3::between(p, q).unwrap();
        prop_assert_eq!(v.offset(p).unwrap(), q);
        prop_assert_eq!(v.neg().unwrap().offset(q).unwrap(), p);
        let p2 = P2 { x: p.x, y: p.y };
        let q2 = P2 { x: q.x, y: q.y };
        prop_assert_eq!(V2::between(p2, q2).unwrap().offset(p2).unwrap(), q2);
    }
}

#[test]
fn extremes_stay_within_budget() {
    let ends = [-VEC_MAX, VEC_MAX];
    let mut largest_triple = 0u128;
    let mut largest_cross = 0u128;
    for ax in ends {
        for ay in ends {
            for az in ends {
                let a = V3::new(ax, ay, az).unwrap();
                for cx in ends {
                    for cy in ends {
                        for cz in ends {
                            let c = V3::new(cx, cy, cz).unwrap();
                            assert_eq!(BigInt::from(a.dot(c).unwrap()), dot3(a, c));
                            let w = a.cross(c).unwrap();
                            largest_cross = largest_cross.max(w.x().unsigned_abs());
                            let fixed = V3::new(VEC_MAX, -VEC_MAX, VEC_MAX).unwrap();
                            for d in [fixed, V3::new(cy, cz, cx).unwrap()] {
                                let t = d.triple(a, c).unwrap();
                                let [x, y, z] = cross3(a, c);
                                let want = b(d.x()) * x + b(d.y()) * y + b(d.z()) * z;
                                assert_eq!(BigInt::from(t), want);
                                largest_triple = largest_triple.max(t.unsigned_abs());
                            }
                        }
                    }
                }
            }
        }
    }
    // The extremes reach the budgets' orders of magnitude: 2δ² for a cross component, 4δ³ for the
    // triple product of these corner vectors (6δ³ is the bound).
    let delta = u128::from(VEC_MAX.unsigned_abs());
    assert_eq!(largest_cross, 2 * delta * delta);
    assert!(largest_triple >= 4 * delta * delta * delta);
    assert!(largest_triple < 1u128 << 126);
}

#[test]
fn differences_of_extreme_points() {
    let lo = P3::new(-COORD_MAX, -COORD_MAX, -COORD_MAX).unwrap();
    let hi = P3::new(COORD_MAX, COORD_MAX, COORD_MAX).unwrap();
    let v = V3::between(lo, hi).unwrap();
    assert_eq!(v.to_array(), [VEC_MAX; 3]);
    assert_eq!(VEC_MAX, (1i64 << 41) - 2);
    assert_eq!(
        v.norm2().unwrap(),
        3 * i128::from(VEC_MAX) * i128::from(VEC_MAX)
    );
    assert_eq!(v.offset(lo).unwrap(), hi);
    // Moving the top corner by one more step leaves the frame.
    let one = V3::new(1, 0, 0).unwrap();
    assert_eq!(
        one.offset(hi),
        Err(NumError::OutOfRange { value: 1i128 << 40 })
    );
    assert_eq!(
        V3::new(VEC_MAX + 1, 0, 0),
        Err(NumError::OutOfRange {
            value: i128::from(VEC_MAX) + 1
        })
    );
    assert_eq!(
        v.add(one),
        Err(NumError::OutOfRange {
            value: i128::from(VEC_MAX) + 1
        })
    );
    let below = -i128::from(VEC_MAX) - 1;
    assert_eq!(
        V2::new(0, -VEC_MAX - 1),
        Err(NumError::OutOfRange { value: below })
    );
    assert!(V3::ZERO.cross(v).unwrap().is_zero());
    // Negation is exact at both ends of the range.
    assert_eq!(v.neg().unwrap().to_array(), [-VEC_MAX; 3]);
    assert_eq!(v.neg().unwrap().neg(), Ok(v));
}
