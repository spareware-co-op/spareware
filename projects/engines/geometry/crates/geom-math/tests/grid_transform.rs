//! Tests of `geom_math::grid_transform`: the laws of the group of exact grid transforms.
//!
//! What this file proves: there are exactly 48 linear parts, half of them orientation-reversing;
//! composing with the inverse gives the identity exactly, on both sides; applying a composition is
//! applying its parts in turn; the determinant is multiplicative and matches the matrix; quarter
//! turns follow the right-hand rule and four make the identity; mirrors reverse orientation and are
//! involutions; vectors transform by the linear part only; boxes transform exactly; and every range
//! limit (points at ±(2^40 − 1), translations at ±2(2^40 − 1)) is enforced with `OutOfRange`.

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

use common::config;
use geom_math::grid_transform::TRANSLATION_MAX;
use geom_math::{Axis, Box3, GridTransform, V3};
use geom_num::NumError;
use geom_num::grid::{COORD_MAX, P3};
use proptest::prelude::*;

/// The six permutations of the axes.
const PERMS: [[Axis; 3]; 6] = [
    [Axis::X, Axis::Y, Axis::Z],
    [Axis::X, Axis::Z, Axis::Y],
    [Axis::Y, Axis::X, Axis::Z],
    [Axis::Y, Axis::Z, Axis::X],
    [Axis::Z, Axis::X, Axis::Y],
    [Axis::Z, Axis::Y, Axis::X],
];

fn translation() -> BoxedStrategy<i64> {
    prop_oneof![
        3 => -TRANSLATION_MAX..=TRANSLATION_MAX,
        1 => Just(TRANSLATION_MAX),
        1 => Just(-TRANSLATION_MAX),
        2 => -1000i64..=1000,
        1 => Just(0i64),
    ]
    .boxed()
}

fn transform() -> BoxedStrategy<GridTransform> {
    (
        0usize..6,
        any::<[bool; 3]>(),
        translation(),
        translation(),
        translation(),
    )
        .prop_map(|(p, n, a, b, c)| GridTransform::from_parts(PERMS[p], n, [a, b, c]).unwrap())
        .boxed()
}

fn coord() -> BoxedStrategy<i64> {
    prop_oneof![
        3 => -COORD_MAX..=COORD_MAX,
        1 => Just(COORD_MAX),
        1 => Just(-COORD_MAX),
        1 => -100i64..=100,
    ]
    .boxed()
}

fn point() -> BoxedStrategy<P3> {
    (coord(), coord(), coord())
        .prop_map(|(x, y, z)| P3::new(x, y, z).unwrap())
        .boxed()
}

/// The determinant of a 3 × 3 integer matrix, by cofactor expansion.
fn det(m: [[i8; 3]; 3]) -> i32 {
    let m = m.map(|r| r.map(i32::from));
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// The image of a point by the matrix and translation, in i128, without range checks.
fn reference(t: &GridTransform, p: P3) -> [i128; 3] {
    let v = [p.x.get(), p.y.get(), p.z.get()].map(i128::from);
    let m = t.matrix();
    let tr = t.translation_part();
    [0, 1, 2].map(|i| (0..3).map(|j| i128::from(m[i][j]) * v[j]).sum::<i128>() + i128::from(tr[i]))
}

proptest! {
    #![proptest_config(config(4096))]

    #[test]
    fn inverse_composes_to_identity(t in transform()) {
        let inv = t.inverse().unwrap();
        prop_assert!(t.compose(&inv).unwrap().is_identity());
        prop_assert!(inv.compose(&t).unwrap().is_identity());
        prop_assert_eq!(inv.inverse(), Ok(t));
        prop_assert_eq!(inv.determinant(), t.determinant());
    }

    #[test]
    fn apply_matches_matrix_and_checks_range(t in transform(), p in point()) {
        let want = reference(&t, p);
        match t.apply(p) {
            Ok(q) => {
                prop_assert_eq!([q.x.get(), q.y.get(), q.z.get()].map(i128::from), want);
                prop_assert_eq!(t.inverse().unwrap().apply(q).unwrap(), p);
            }
            Err(NumError::OutOfRange { value }) => {
                let limit = u128::from(COORD_MAX.unsigned_abs());
                prop_assert!(want.iter().any(|&w| w.unsigned_abs() > limit));
                prop_assert!(want.contains(&value));
            }
            Err(e) => prop_assert!(false, "unexpected error {e:?}"),
        }
    }

    #[test]
    fn composition_is_application_in_turn(a in transform(), b in transform(), p in point()) {
        let Ok(ab) = a.compose(&b) else {
            // Only a translation beyond the limit is refused: L_a · t_b + t_a.
            let tb = b.translation_part();
            let limit = u128::from(TRANSLATION_MAX.unsigned_abs());
            let too_far = a.matrix().iter().zip(a.translation_part()).any(|(row, ta)| {
                let moved: i128 =
                    row.iter().zip(tb).map(|(m, x)| i128::from(*m) * i128::from(x)).sum();
                (moved + i128::from(ta)).unsigned_abs() > limit
            });
            prop_assert!(too_far);
            return Ok(());
        };
        prop_assert_eq!(ab.determinant(), a.determinant() * b.determinant());
        let flips = a.reverses_orientation() != b.reverses_orientation();
        prop_assert_eq!(ab.reverses_orientation(), flips);
        if let Ok(bp) = b.apply(p) {
            match (a.apply(bp), ab.apply(p)) {
                (Ok(x), Ok(y)) => prop_assert_eq!(x, y),
                (Err(_), Err(_)) => {}
                (x, y) => prop_assert!(false, "a(b(p)) = {x:?} but (a∘b)(p) = {y:?}"),
            }
        }
    }

    #[test]
    fn vectors_use_the_linear_part(t in transform(), p in point(), q in point()) {
        let v = V3::between(p, q).unwrap();
        if let (Ok(tp), Ok(tq)) = (t.apply(p), t.apply(q)) {
            prop_assert_eq!(V3::between(tp, tq), t.apply_vector(v));
        }
        let moved = t.apply_vector(v).unwrap();
        prop_assert_eq!(t.inverse().unwrap().apply_vector(moved), Ok(v));
        prop_assert_eq!(moved.norm2(), v.norm2());
    }

    #[test]
    fn boxes_transform_exactly(t in transform(), p in point(), q in point()) {
        let bx = Box3::from_points([p, q]).unwrap();
        if let Ok(img) = bx.transformed(&t) {
            let corners = [p.x, q.x].into_iter().flat_map(|x| {
                [p.y, q.y]
                    .into_iter()
                    .flat_map(move |y| [p.z, q.z].into_iter().map(move |z| P3 { x, y, z }))
            });
            let images: Vec<P3> = corners.map(|c| t.apply(c).unwrap()).collect();
            prop_assert_eq!(Box3::from_points(images.iter().copied()).unwrap(), img);
        }
    }
}

#[test]
fn there_are_48_linear_parts() {
    let mut seen = Vec::new();
    let mut reversing = 0;
    for perm in PERMS {
        for bits in 0u8..8 {
            let neg = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0];
            let t = GridTransform::from_parts(perm, neg, [0; 3]).unwrap();
            let m = t.matrix();
            assert_eq!(i32::from(t.determinant()), det(m));
            assert_eq!(t.reverses_orientation(), det(m) < 0);
            if t.reverses_orientation() {
                reversing += 1;
            }
            assert!(!seen.contains(&m));
            seen.push(m);
        }
    }
    assert_eq!(seen.len(), 48);
    assert_eq!(reversing, 24);
}

#[test]
fn quarter_turns_follow_the_right_hand_rule() {
    let p = |x, y, z| P3::new(x, y, z).unwrap();
    let qt = GridTransform::quarter_turns;
    assert_eq!(qt(Axis::Z, 1).apply(p(1, 0, 0)).unwrap(), p(0, 1, 0));
    assert_eq!(qt(Axis::Z, 1).apply(p(0, 1, 0)).unwrap(), p(-1, 0, 0));
    assert_eq!(qt(Axis::X, 1).apply(p(0, 1, 0)).unwrap(), p(0, 0, 1));
    assert_eq!(qt(Axis::Y, 1).apply(p(0, 0, 1)).unwrap(), p(1, 0, 0));
    assert_eq!(qt(Axis::Z, 2).apply(p(3, 4, 5)).unwrap(), p(-3, -4, 5));
    assert_eq!(qt(Axis::Z, -1), qt(Axis::Z, 3));
    for axis in Axis::ALL {
        assert!(qt(axis, 4).is_identity());
        assert!(qt(axis, 0).is_identity());
        for n in 0..4 {
            let t = qt(axis, n);
            assert_eq!(t.determinant(), 1);
            assert!(!t.reverses_orientation());
            assert_eq!(t.compose(&qt(axis, 1)).unwrap(), qt(axis, n + 1));
            assert_eq!(t.inverse(), Ok(qt(axis, -n)));
        }
        let m = GridTransform::mirror(axis);
        assert!(m.reverses_orientation());
        assert!(m.compose(&m).unwrap().is_identity());
        assert_eq!(m.inverse(), Ok(m));
    }
    // The point reflection −I = mirror X ∘ mirror Y ∘ mirror Z reverses orientation in 3D.
    let minus_i = GridTransform::mirror(Axis::X)
        .compose(&GridTransform::mirror(Axis::Y))
        .unwrap()
        .compose(&GridTransform::mirror(Axis::Z))
        .unwrap();
    assert_eq!(minus_i.matrix(), [[-1, 0, 0], [0, -1, 0], [0, 0, -1]]);
    assert!(minus_i.reverses_orientation());
    assert_eq!(GridTransform::default(), GridTransform::IDENTITY);
}

#[test]
fn range_limits_at_the_extremes() {
    let top = P3::new(COORD_MAX, COORD_MAX, COORD_MAX).unwrap();
    let bottom = P3::new(-COORD_MAX, -COORD_MAX, -COORD_MAX).unwrap();
    let mirrored = P3::new(-COORD_MAX, COORD_MAX, COORD_MAX).unwrap();
    assert_eq!(GridTransform::mirror(Axis::X).apply(top).unwrap(), mirrored);
    let across = GridTransform::translation([TRANSLATION_MAX; 3]).unwrap();
    assert_eq!(across.apply(bottom).unwrap(), top);
    assert_eq!(across.inverse().unwrap().apply(top).unwrap(), bottom);
    assert_eq!(
        GridTransform::translation([1, 0, 0]).unwrap().apply(top),
        Err(NumError::OutOfRange { value: 1i128 << 40 })
    );
    assert_eq!(
        GridTransform::translation([0, -TRANSLATION_MAX - 1, 0]),
        Err(NumError::OutOfRange {
            value: -i128::from(TRANSLATION_MAX) - 1
        })
    );
    let twice = 2 * i128::from(TRANSLATION_MAX);
    assert_eq!(
        across.compose(&across),
        Err(NumError::OutOfRange { value: twice })
    );
    assert_eq!(
        GridTransform::from_parts([Axis::X, Axis::X, Axis::Z], [false; 3], [0; 3]),
        Err(NumError::Degenerate)
    );
}
