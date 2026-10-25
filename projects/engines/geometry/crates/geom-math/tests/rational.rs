//! Tests of `geom_math::rational` against exact rational arithmetic (`num-rational`).
//!
//! What this file proves:
//!
//! - Applying: every image is (M·p + t)/d rounded to the nearest step, ties to even, with the exact
//!   flag right, and out of range exactly when the rounded value leaves ±(2^40 − 1); vectors too.
//! - Composing: the composed transform is exactly the composition of the two maps, in canonical
//!   form; inverting: a transform composed with its inverse is exactly the identity; a singular
//!   matrix is refused.
//! - Classifying: exactly as the exact determinant and Gram matrix say, with no tolerance.
//! - Canonical form, conversions (grid transforms both ways, nearest floating-point transform) and
//!   the bounds of the bit budget, at their edges.

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
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]

mod common;

use common::{config, exact, int, is_integer, is_nearest_double, round_half_even, to_i128};
use geom_math::rational::{RATIONAL_DEN_MAX, RATIONAL_ENTRY_MAX, RATIONAL_TRANSLATION_MAX};
use geom_math::{
    Axis, GridTransform, MathError, RationalTransform, Rotation, Scale, TransformKind, V3,
};
use geom_num::NumError;
use geom_num::grid::{COORD_MAX, P3};
use num_rational::BigRational;
use num_traits::{Signed, Zero};
use proptest::prelude::*;

/// The exact map of a rational transform: (M/d, t/d).
fn exact_map(r: &RationalTransform) -> ([[BigRational; 3]; 3], [BigRational; 3]) {
    let d = int(r.denominator().into());
    let m = r.matrix().map(|row| row.map(|v| int(v.into()) / &d));
    let t = r.translation_part().map(|v| int(v) / &d);
    (m, t)
}

fn mat_mul(a: &[[BigRational; 3]; 3], b: &[[BigRational; 3]; 3]) -> [[BigRational; 3]; 3] {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| (0..3).map(|k| &a[i][k] * &b[k][j]).sum::<BigRational>())
    })
}

fn mat_vec(a: &[[BigRational; 3]; 3], v: &[BigRational; 3]) -> [BigRational; 3] {
    std::array::from_fn(|i| (0..3).map(|k| &a[i][k] * &v[k]).sum::<BigRational>())
}

fn det(m: &[[BigRational; 3]; 3]) -> BigRational {
    &m[0][0] * (&m[1][1] * &m[2][2] - &m[1][2] * &m[2][1])
        - &m[0][1] * (&m[1][0] * &m[2][2] - &m[1][2] * &m[2][0])
        + &m[0][2] * (&m[1][0] * &m[2][1] - &m[1][1] * &m[2][0])
}

/// A rational transform with small entries, a denominator up to 12 and a translation up to 2^60.
fn rational() -> BoxedStrategy<RationalTransform> {
    (
        prop::array::uniform3(prop::array::uniform3(-6i64..=6)),
        prop::array::uniform3(prop_oneof![
            -100i128..100,
            -(1i128 << 60)..(1i128 << 60),
            Just(0i128)
        ]),
        prop_oneof![1i64..=12, -12i64..=-1],
    )
        .prop_map(|(m, t, d)| RationalTransform::new(m, t, d).unwrap())
        .boxed()
}

fn coord() -> BoxedStrategy<i64> {
    prop_oneof![
        -COORD_MAX..=COORD_MAX,
        -1000i64..1000,
        Just(COORD_MAX),
        Just(-COORD_MAX)
    ]
    .boxed()
}

fn check_apply(r: &RationalTransform, p: P3) -> Result<(), TestCaseError> {
    let (m, t) = exact_map(r);
    let v = [p.x.get(), p.y.get(), p.z.get()].map(|c| int(c.into()));
    let image: Vec<BigRational> = (0..3).map(|i| mat_vec(&m, &v)[i].clone() + &t[i]).collect();
    let rounded: Vec<i128> = image.iter().map(|x| to_i128(&round_half_even(x))).collect();
    let out = rounded
        .iter()
        .any(|v| v.unsigned_abs() > COORD_MAX.unsigned_abs().into());
    match r.apply(p) {
        Ok(q) => {
            prop_assert!(!out);
            let got = [q.value().x.get(), q.value().y.get(), q.value().z.get()].map(i128::from);
            prop_assert_eq!(got.to_vec(), rounded);
            prop_assert_eq!(q.is_exact(), image.iter().all(is_integer));
        }
        Err(NumError::OutOfRange { .. }) => prop_assert!(out),
        Err(e) => prop_assert!(false, "unexpected {e:?}"),
    }
    Ok(())
}

proptest! {
    #![proptest_config(config(2048))]

    #[test]
    fn images_round_exactly(r in rational(), p in prop::array::uniform3(coord())) {
        check_apply(&r, P3::new(p[0], p[1], p[2]).unwrap())?;
    }

    #[test]
    fn vectors_round_exactly(r in rational(), v in prop::array::uniform3(-(1i64 << 41) + 2..(1i64 << 41) - 1)) {
        let v = V3::new(v[0], v[1], v[2]).unwrap();
        let (m, _) = exact_map(&r);
        let image = mat_vec(&m, &v.to_array().map(|c| int(c.into())));
        let rounded = image.clone().map(|x| to_i128(&round_half_even(&x)));
        match r.apply_vector(v) {
            Ok(w) => {
                prop_assert_eq!(w.value().to_array().map(i128::from), rounded);
                prop_assert_eq!(w.is_exact(), image.iter().all(is_integer));
            }
            Err(NumError::OutOfRange { .. }) => {
                let lim = i128::from(geom_math::vector::VEC_MAX);
                prop_assert!(rounded.iter().any(|v| v.abs() > lim));
            }
            Err(e) => prop_assert!(false, "unexpected {e:?}"),
        }
    }

    #[test]
    fn composition_is_exact(a in rational(), b in rational()) {
        let Ok(c) = a.compose(&b) else { return Ok(()) };
        let ((ma, ta), (mb, tb), (mc, tc)) = (exact_map(&a), exact_map(&b), exact_map(&c));
        prop_assert_eq!(mat_mul(&ma, &mb), mc);
        let want: Vec<BigRational> = (0..3).map(|i| mat_vec(&ma, &tb)[i].clone() + &ta[i]).collect();
        prop_assert_eq!(want, tc.to_vec());
        // Canonical: rebuilding from the parts changes nothing.
        let again = RationalTransform::new(c.matrix(), c.translation_part(), c.denominator());
        prop_assert_eq!(again.unwrap(), c);
        prop_assert!(c.denominator() > 0);
    }

    #[test]
    fn inverses_are_exact(a in rational()) {
        let (m, _) = exact_map(&a);
        match a.inverse() {
            Ok(inv) => {
                prop_assert!(!det(&m).is_zero());
                prop_assert_eq!(inv.compose(&a).unwrap(), RationalTransform::IDENTITY);
                prop_assert_eq!(a.compose(&inv).unwrap(), RationalTransform::IDENTITY);
            }
            Err(MathError::Singular) => prop_assert!(det(&m).is_zero()),
            Err(MathError::Num(NumError::OutOfRange { .. })) => {}
            Err(e) => prop_assert!(false, "unexpected {e:?}"),
        }
    }

    #[test]
    fn classification_is_exact(a in rational()) {
        let (m, t) = exact_map(&a);
        let cl = a.classify().unwrap();
        let d = det(&m);
        let gram = |i: usize, j: usize| (0..3).map(|k| &m[k][i] * &m[k][j]).sum::<BigRational>();
        let g00 = gram(0, 0);
        let scaled_identity = (0..3).all(|i| {
            (0..3).all(|j| gram(i, j) == if i == j { g00.clone() } else { BigRational::zero() })
        });
        let identity = (0..3).all(|i| (0..3).all(|j| m[i][j] == int(i128::from(i == j))));
        let want = if identity {
            if t.iter().all(Zero::is_zero) { TransformKind::Identity } else { TransformKind::Translation }
        } else if d.is_zero() {
            TransformKind::Degenerate
        } else if scaled_identity && g00 == int(1) {
            TransformKind::Rigid
        } else if scaled_identity {
            TransformKind::UniformScale
        } else {
            TransformKind::NonUniform
        };
        prop_assert_eq!(cl.kind, want);
        prop_assert_eq!(cl.mirror, d.is_negative());
    }
}

#[test]
fn pythagorean_rotations_are_rigid_and_exact() {
    for (c, s) in [
        (3, 4),
        (-5, 12),
        (8, -15),
        (20, 21),
        (2_147_395_599, 92_680),
        (0, 1),
        (-1, 0),
    ] {
        let rot = Rotation::direction(c, s).unwrap();
        for axis in Axis::ALL {
            let r = RationalTransform::rotation(axis, rot).unwrap();
            let cl = r.classify().unwrap();
            let quarter = rot.quarter_turn_count();
            if quarter == Some(0) {
                assert_eq!(cl.kind, TransformKind::Identity);
            } else {
                assert_eq!(
                    (cl.kind, cl.mirror),
                    (TransformKind::Rigid, false),
                    "{rot:?} {axis:?}"
                );
            }
            // A quarter turn is a grid transform, and agrees with the grid's quarter turn.
            if let Some(q) = quarter {
                let g = GridTransform::quarter_turns(axis, i32::from(q));
                assert_eq!(r.to_grid(), Some(g));
                assert_eq!(RationalTransform::from_grid(&g), r);
            } else {
                assert_eq!(r.to_grid(), None);
            }
            let inv = r.inverse().unwrap();
            assert_eq!(inv.compose(&r).unwrap(), RationalTransform::IDENTITY);
            for p in [
                P3::new(5, -7, 11).unwrap(),
                P3::new(COORD_MAX / 2, 3, -COORD_MAX / 3).unwrap(),
            ] {
                let q = r.apply(p).unwrap();
                // Rounding the image and mapping back lands within one step per axis.
                let back = inv.apply(q.value()).unwrap().value();
                for (a, b) in [(back.x, p.x), (back.y, p.y), (back.z, p.z)] {
                    assert!(
                        (a.get() - b.get()).abs() <= 1,
                        "{rot:?}: {p:?} → {q:?} → {back:?}"
                    );
                }
            }
        }
    }
    assert_eq!(
        RationalTransform::rotation(Axis::Z, Rotation::direction(1, 2).unwrap()),
        Err(NumError::Degenerate)
    );
}

#[test]
fn scales_translations_and_canonical_form() {
    let k = Scale::new(99_987_654, 100_000_000).unwrap();
    let s = RationalTransform::uniform_scale(k).unwrap();
    assert_eq!(s.classify().unwrap().kind, TransformKind::UniformScale);
    assert_eq!(s.denominator(), 50_000_000);
    let p = P3::new(1_000_000_000, -3, 7).unwrap();
    let q = s.apply(p).unwrap();
    // 10^9 · 0.99987654 = 999,876,540 exactly; −3·k rounds to −3; 7·k rounds to 7.
    assert_eq!(
        (q.value().x.get(), q.value().y.get(), q.value().z.get()),
        (999_876_540, -3, 7)
    );
    assert!(!q.is_exact());
    let half = RationalTransform::translation([1, -1, 3], 2).unwrap();
    assert_eq!(half.classify().unwrap().kind, TransformKind::Translation);
    // 0.5 → 0, −0.5 → 0 and 1.5 → 2: ties to even.
    let q = half.apply(P3::new(0, 0, 0).unwrap()).unwrap();
    assert_eq!(
        (
            q.value().x.get(),
            q.value().y.get(),
            q.value().z.get(),
            q.is_exact()
        ),
        (0, 0, 2, false)
    );
    let q = half.apply(P3::new(1, 0, 0).unwrap()).unwrap();
    assert_eq!(q.value().x.get(), 2); // 1.5 → 2
    // Canonical form: common factors and the denominator's sign are removed.
    let a = RationalTransform::new([[2, 0, 0], [0, 4, 0], [0, 0, 6]], [8, 0, 2], 2).unwrap();
    let b = RationalTransform::new([[-1, 0, 0], [0, -2, 0], [0, 0, -3]], [-4, 0, -1], -1).unwrap();
    assert_eq!(a, b);
    assert_eq!(
        (a.matrix(), a.translation_part(), a.denominator()),
        ([[1, 0, 0], [0, 2, 0], [0, 0, 3]], [4, 0, 1], 1)
    );
    assert_eq!(RationalTransform::default(), RationalTransform::IDENTITY);
    // The nearest floating-point transform.
    let f = RationalTransform::new([[1, 2, 0], [0, 3, 0], [0, 0, 7]], [1, 0, 0], 3)
        .unwrap()
        .to_transform()
        .unwrap();
    let rows = f.rows();
    assert!(is_nearest_double(rows[0][1], &(int(2) / int(3))));
    assert_eq!(rows[1][1], 1.0);
    assert!(is_nearest_double(rows[0][3], &(int(1) / int(3))));
    assert_eq!(exact(rows[2][2]), exact(7.0 / 3.0));
}

#[test]
fn bounds_and_errors() {
    let id = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];
    assert_eq!(
        RationalTransform::new(id, [0; 3], 0),
        Err(NumError::Degenerate)
    );
    let e = RATIONAL_ENTRY_MAX;
    assert!(RationalTransform::new([[e, 0, 0], [0, 1, 0], [0, 0, 1]], [0; 3], 1).is_ok());
    assert!(matches!(
        RationalTransform::new([[e + 1, 0, 0], [0, 1, 0], [0, 0, 1]], [0; 3], 1),
        Err(NumError::OutOfRange { .. })
    ));
    // A large entry with a common factor reduces into range.
    assert!(RationalTransform::new([[2 * e, 0, 0], [0, 2, 0], [0, 0, 2]], [0; 3], 2).is_ok());
    assert!(RationalTransform::new(id, [0; 3], RATIONAL_DEN_MAX).is_ok());
    assert!(RationalTransform::new(id, [0; 3], RATIONAL_DEN_MAX + 1).is_err());
    let tmax = RATIONAL_TRANSLATION_MAX;
    assert!(RationalTransform::new(id, [tmax, -tmax, 0], 1).is_ok());
    assert!(RationalTransform::new(id, [tmax + 1, 0, 0], 1).is_err());
    assert!(RationalTransform::new(id, [i128::MIN, 0, 0], 1).is_err());
    // Singular matrices have no inverse.
    let flat = RationalTransform::new([[1, 2, 3], [2, 4, 6], [0, 0, 1]], [0; 3], 1).unwrap();
    assert_eq!(flat.classify().unwrap().kind, TransformKind::Degenerate);
    assert_eq!(flat.inverse(), Err(MathError::Singular));
    // The largest entries still apply and invert within the budget.
    let big = RationalTransform::new(
        [[e, -e, e], [e, e, -e], [-e, e, e]],
        [tmax; 3],
        RATIONAL_DEN_MAX - 1,
    )
    .unwrap();
    let p = P3::new(COORD_MAX, -COORD_MAX, COORD_MAX).unwrap();
    check_apply(&big, p).unwrap();
    let inv = big.inverse();
    if let Ok(inv) = inv {
        assert_eq!(inv.compose(&big).unwrap(), RationalTransform::IDENTITY);
    } else {
        assert!(matches!(
            inv,
            Err(MathError::Num(NumError::OutOfRange { .. }))
        ));
    }
    // Composition beyond the budget is refused, never wrapped.
    let huge = RationalTransform::new([[e, 0, 0], [0, e, 0], [0, 0, e]], [0; 3], 1).unwrap();
    assert!(matches!(
        huge.compose(&huge),
        Err(NumError::OutOfRange { .. })
    ));
    // Grid transforms convert both ways for all 48 signed permutations.
    for p in [
        [Axis::X, Axis::Y, Axis::Z],
        [Axis::Z, Axis::X, Axis::Y],
        [Axis::Y, Axis::X, Axis::Z],
    ] {
        for bits in 0..8u8 {
            let neg = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0];
            let g = GridTransform::from_parts(p, neg, [5, -COORD_MAX, 2 * COORD_MAX]).unwrap();
            let r = RationalTransform::from_grid(&g);
            assert_eq!(r.to_grid(), Some(g));
            assert_eq!(r.classify().unwrap().mirror, g.reverses_orientation());
            let q = P3::new(1, 2, -3).unwrap();
            match g.apply(q) {
                Ok(img) => assert_eq!(r.apply(q).unwrap().value(), img),
                Err(_) => assert!(r.apply(q).is_err()),
            }
        }
    }
}
