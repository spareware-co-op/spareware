//! Tests of `geom_math::transform` and `geom_math::float`.
//!
//! What this file proves:
//!
//! - Classification: identity, translations, rotations about every axis by many angles, mirrors,
//!   uniform and non-uniform scales, shears and singular maps get the documented kind; the kind
//!   switches between `Rigid` and `NonUniform` at the ladder's angular bound on lengths; the mirror
//!   flag always equals the sign of the exact rational determinant whenever the kind is not
//!   `Degenerate`.
//! - Exactness against grid transforms: the `f64` transform of a grid transform maps grid points
//!   exactly like it, composes exactly like it, inverts exactly like it, and is classified rigid
//!   with the same orientation.
//! - Inverses: a transform composed with its inverse is the identity to rounding; a singular
//!   transform is refused with `Singular`.
//! - Finite values only: NaN and infinities are refused at every entry point, and overflow is
//!   reported as `NotFinite`.

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

use common::{config, exact, pow2f};
use geom_math::grid_transform::TRANSLATION_MAX;
use geom_math::{Axis, F2, F3, GridTransform, MathError, Transform, TransformKind};
use geom_num::grid::{COORD_MAX, P3};
use geom_num::ladder::ANGULAR;
use geom_num::{NumError, math};
use num_rational::BigRational;
use num_traits::Signed;
use proptest::prelude::*;

const PERMS: [[Axis; 3]; 6] = [
    [Axis::X, Axis::Y, Axis::Z],
    [Axis::X, Axis::Z, Axis::Y],
    [Axis::Y, Axis::X, Axis::Z],
    [Axis::Y, Axis::Z, Axis::X],
    [Axis::Z, Axis::X, Axis::Y],
    [Axis::Z, Axis::Y, Axis::X],
];

fn grid_transform() -> BoxedStrategy<GridTransform> {
    let t = -TRANSLATION_MAX..=TRANSLATION_MAX;
    (0usize..6, any::<[bool; 3]>(), [t.clone(), t.clone(), t])
        .prop_map(|(p, n, t)| GridTransform::from_parts(PERMS[p], n, t).unwrap())
        .boxed()
}

fn axis() -> BoxedStrategy<Axis> {
    prop_oneof![Just(Axis::X), Just(Axis::Y), Just(Axis::Z)].boxed()
}

/// The exact determinant of the linear part of a transform.
fn exact_det(t: &Transform) -> BigRational {
    let m = t.rows().map(|r| r.map(exact));
    &m[0][0] * (&m[1][1] * &m[2][2] - &m[1][2] * &m[2][1])
        - &m[0][1] * (&m[1][0] * &m[2][2] - &m[1][2] * &m[2][0])
        + &m[0][2] * (&m[1][0] * &m[2][1] - &m[1][1] * &m[2][0])
}

fn kind(t: &Transform) -> TransformKind {
    t.classify().unwrap().kind
}

#[test]
fn named_transforms_classify_as_documented() {
    use TransformKind::*;
    let c = |t: Transform| t.classify().unwrap();
    assert_eq!(c(Transform::IDENTITY).kind, Identity);
    assert_eq!(c(Transform::default()).kind, Identity);
    assert_eq!(
        c(Transform::translation(F3::new(1.0, -2.0, 0.5).unwrap())).kind,
        Translation
    );
    for axis in Axis::ALL {
        for h in [0.5, 1.0, 1.5, 1.0 / 3.0, 1.0 / 6.0, 0.1, -0.7, 123.456] {
            let r = Transform::rotation(axis, h).unwrap();
            let cl = r.classify().unwrap();
            assert_eq!(cl.kind, Rigid, "rotation {h} about {axis:?}");
            assert!(!cl.mirror);
            assert!(cl.keeps_circles() && cl.keeps_lengths());
        }
        let m = Transform::mirror(axis).classify().unwrap();
        assert_eq!(m.kind, Rigid);
        assert!(m.mirror && m.reverses_orientation());
    }
    // A quarter turn has exact entries.
    assert_eq!(
        Transform::rotation(Axis::Z, 0.5).unwrap().rows(),
        [
            [0.0, -1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0]
        ]
    );
    let s2 = c(Transform::uniform_scale(2.0).unwrap());
    assert_eq!((s2.kind, s2.mirror), (UniformScale, false));
    assert!(s2.keeps_circles() && !s2.keeps_lengths());
    let sm = c(Transform::uniform_scale(-0.25).unwrap());
    assert_eq!((sm.kind, sm.mirror), (UniformScale, true));
    let nu = c(Transform::scale(1.0, 2.0, 1.0).unwrap());
    assert_eq!((nu.kind, nu.mirror), (NonUniform, false));
    assert!(!nu.keeps_circles());
    let shear = Transform::from_rows([
        [1.0, 0.5, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ])
    .unwrap();
    assert_eq!(kind(&shear), NonUniform);
    for singular in [
        Transform::scale(1.0, 1.0, 0.0).unwrap(),
        Transform::from_rows([
            [1.0, 2.0, 3.0, 0.0],
            [2.0, 4.0, 6.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ])
        .unwrap(),
        Transform::from_rows([[0.0; 4]; 3]).unwrap(),
    ] {
        let cl = singular.classify().unwrap();
        assert_eq!((cl.kind, cl.mirror), (Degenerate, false));
        assert!(!cl.keeps_circles());
        assert_eq!(singular.inverse(), Err(MathError::Singular));
    }
}

#[test]
fn rigid_switches_to_non_uniform_at_the_angular_bound() {
    // A stretch of 1 + 2^−36 changes lengths by at most 2^−36 < ε_ang (G₂₂ − 1 is about 2^−35,
    // within the certified row-sum bound 2ε_ang − ε_ang²): still rigid.
    let tiny = 1.0 + pow2f(-36);
    assert_eq!(
        kind(&Transform::scale(1.0, 1.0, tiny).unwrap()),
        TransformKind::Rigid
    );
    // A stretch of 1 + 2^−30 changes lengths by 2^−30 > ε_ang: non-uniform.
    let big = 1.0 + pow2f(-30);
    assert_eq!(
        kind(&Transform::scale(1.0, 1.0, big).unwrap()),
        TransformKind::NonUniform
    );
    // The same stretch on every axis is a uniform scale.
    assert_eq!(
        kind(&Transform::scale(big, big, big).unwrap()),
        TransformKind::UniformScale
    );
    assert!(ANGULAR.get() > pow2f(-35) && ANGULAR.get() < pow2f(-29));
}

#[test]
fn finite_values_only() {
    assert_eq!(F3::new(f64::NAN, 0.0, 0.0), Err(NumError::NotFinite));
    assert_eq!(F2::new(0.0, f64::INFINITY), Err(NumError::NotFinite));
    assert_eq!(
        Transform::scale(1.0, f64::NAN, 1.0),
        Err(NumError::NotFinite)
    );
    assert_eq!(
        Transform::rotation(Axis::X, f64::INFINITY),
        Err(NumError::NotFinite)
    );
    let big = F3::new(f64::MAX, 0.0, 0.0).unwrap();
    assert_eq!(big.add(big), Err(NumError::NotFinite));
    assert_eq!(big.dot(big), Err(NumError::NotFinite));
    assert_eq!(big.scale(2.0), Err(NumError::NotFinite));
    assert_eq!(F3::ZERO.normalized(), Err(NumError::Degenerate));
    let s = Transform::uniform_scale(f64::MAX).unwrap();
    assert_eq!(
        s.apply_point(F3::new(2.0, 0.0, 0.0).unwrap()),
        Err(NumError::NotFinite)
    );
    assert_eq!(s.compose(&s), Err(NumError::NotFinite));
}

#[test]
fn float_vectors_compute_in_a_fixed_order() {
    let a = F3::new(1.0, 2.0, 3.0).unwrap();
    let b = F3::new(-4.0, 0.5, 2.0).unwrap();
    assert_eq!(a.dot(b).unwrap(), -4.0 + 1.0 + 6.0);
    assert_eq!(
        a.cross(b).unwrap().to_array(),
        [4.0 - 1.5, -12.0 - 2.0, 0.5 + 8.0]
    );
    assert_eq!(F3::new(3.0, 4.0, 12.0).unwrap().length().unwrap(), 13.0);
    let n = F3::new(0.0, 3.0, 4.0).unwrap().normalized().unwrap();
    assert_eq!(n.to_array(), [0.0, 0.6, 0.8]);
    assert_eq!((-a).to_array(), [-1.0, -2.0, -3.0]);
    let p = F2::new(3.0, 4.0).unwrap();
    assert_eq!(p.length().unwrap(), 5.0);
    assert_eq!(p.cross(F2::new(1.0, 0.0).unwrap()).unwrap(), -4.0);
    let m = COORD_MAX as f64;
    assert_eq!(
        F3::from(P3::new(COORD_MAX, -COORD_MAX, 7).unwrap()).to_array(),
        [m, -m, 7.0]
    );
}

proptest! {
    #![proptest_config(config(2048))]

    #[test]
    fn grid_transforms_are_exact_as_float_transforms(g in grid_transform(), h in grid_transform(),
        p in (-COORD_MAX..=COORD_MAX, -COORD_MAX..=COORD_MAX, -COORD_MAX..=COORD_MAX)) {
        let t = Transform::from_grid(&g);
        let cl = t.classify().unwrap();
        prop_assert!(matches!(
            cl.kind,
            TransformKind::Rigid | TransformKind::Identity | TransformKind::Translation
        ));
        prop_assert_eq!(cl.mirror, g.reverses_orientation());
        // Points: every value is an integer below 2^42, so the float result is exact.
        let p = P3::new(p.0, p.1, p.2).unwrap();
        let fp = t.apply_point(F3::from(p)).unwrap();
        match g.apply(p) {
            Ok(q) => prop_assert_eq!(fp, F3::from(q)),
            Err(NumError::OutOfRange { .. }) => {
                prop_assert!(fp.to_array().iter().any(|v| v.abs() > COORD_MAX as f64));
            }
            Err(e) => prop_assert!(false, "unexpected {e:?}"),
        }
        // Composition and inverse are exact too.
        if let Ok(gh) = g.compose(&h) {
            let th = t.compose(&Transform::from_grid(&h)).unwrap();
            prop_assert_eq!(Transform::from_grid(&gh), th);
        }
        prop_assert_eq!(t.inverse().unwrap(), Transform::from_grid(&g.inverse().unwrap()));
    }

    #[test]
    fn rotations_and_scales_classify(
        a in axis(),
        h in -4.0f64..4.0,
        s in prop_oneof![0.01f64..0.99, 1.01f64..100.0],
        mirror in any::<bool>(),
        b in axis(),
        k in 1.001f64..10.0,
    ) {
        let r = Transform::rotation(a, h).unwrap();
        let r = if mirror { Transform::mirror(b).compose(&r).unwrap() } else { r };
        let cl = r.classify().unwrap();
        prop_assert_eq!((cl.kind, cl.mirror), (TransformKind::Rigid, mirror));
        let rs = Transform::uniform_scale(s).unwrap().compose(&r).unwrap();
        let cl = rs.classify().unwrap();
        prop_assert_eq!((cl.kind, cl.mirror), (TransformKind::UniformScale, mirror));
        let mut f = [1.0, 1.0, 1.0];
        f[b.index() as usize] = k;
        let nu = Transform::scale(f[0], f[1], f[2]).unwrap().compose(&r).unwrap();
        let cl = nu.classify().unwrap();
        prop_assert_eq!((cl.kind, cl.mirror), (TransformKind::NonUniform, mirror));
        // The inverse undoes the transform to rounding.
        let id = rs.compose(&rs.inverse().unwrap()).unwrap();
        let eye = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        for (row, want) in id.rows().iter().zip(eye) {
            for (x, w) in row.iter().zip(want) {
                prop_assert!(math::abs(x - w).unwrap().get() < 1e-13, "{x} vs {w}");
            }
        }
    }

    #[test]
    fn mirror_flag_is_the_exact_determinant_sign(
        m in prop::array::uniform3(prop::array::uniform4(-8.0f64..8.0)),
    ) {
        let t = Transform::from_rows(m).unwrap();
        let cl = t.classify().unwrap();
        let d = exact_det(&t);
        if cl.kind == TransformKind::Degenerate {
            // Refused only when the determinant is tiny relative to the entries.
            prop_assert!(d.abs() < exact(1e-10));
            prop_assert_eq!(t.inverse(), Err(MathError::Singular));
        } else {
            prop_assert_eq!(cl.mirror, d.is_negative());
            let inv = t.inverse().unwrap();
            let p = F3::new(0.25, -1.5, 3.0).unwrap();
            let back = inv.apply_point(t.apply_point(p).unwrap()).unwrap();
            let err = back.sub(p).unwrap().length().unwrap();
            // Conditioning can amplify rounding; random matrices here are mild.
            prop_assert!(err < 1e-6 * (1.0 + 1.0 / exact_to_f64(&d.abs())), "error {err}");
        }
    }
}

fn exact_to_f64(r: &BigRational) -> f64 {
    use num_traits::ToPrimitive;
    r.to_f64().unwrap()
}
