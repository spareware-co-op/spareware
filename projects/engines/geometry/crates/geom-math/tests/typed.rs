//! Tests of `geom_math::typed` and of `Transform::apply_grid`, against exact rational arithmetic.
//!
//! What this file proves:
//!
//! - Every variant of the typed transform applies to grid points as documented: exact for grid
//!   transforms; the exact image rounded once, ties to even, with the exact flag right, for
//!   rational and certified ones. Certified images are checked against the exact values of the
//!   doubles for entries of every magnitude, including entries near 2^±900 that cancel, subnormal
//!   entries, and values that land exactly on steps and midpoints. An image outside the frame is
//!   refused with its first out-of-range rounded coordinate, exactly, or saturated beyond `i128`.
//! - Rotations get the most exact variant (quarter turns exact, Pythagorean directions rational,
//!   others certified with entries within an ulp of c/√(c² + s²)).
//! - Composition keeps the most exact variant both operands allow and simplifies back to exact;
//!   inverses keep their variant; classification agrees with the variant's own.

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

use std::cmp::Ordering;

use common::{config, exact, int, is_integer, pow2f, round_half_even, sign_minus_sqrt};
use geom_math::{
    Axis, GridTransform, RationalTransform, Rotation, Transform, TransformKind, TypedTransform,
};
use geom_num::NumError;
use geom_num::grid::{COORD_MAX, P3};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive};
use proptest::prelude::*;

/// A double of one of several kinds: small integers, halves and quarters (images on steps and
/// midpoints), random significands at moderate exponents, huge and tiny values, subnormals.
fn entry() -> BoxedStrategy<f64> {
    prop_oneof![
        3 => (-8i32..=8).prop_map(f64::from),
        2 => (-64i32..=64).prop_map(|k| f64::from(k) / 4.0),
        3 => (any::<u64>(), -60i32..=4, any::<bool>()).prop_map(|(bits, e, neg)| {
            let m = (bits & ((1u64 << 52) - 1)) | (1u64 << 52);
            let v = m as f64 * pow2f(e - 52);
            if neg { -v } else { v }
        }),
        1 => (1u64..(1u64 << 52)).prop_map(f64::from_bits),
        1 => prop_oneof![Just(pow2f(900)), Just(-pow2f(900)), Just(pow2f(-900))],
    ]
    .boxed()
}

fn translation() -> BoxedStrategy<f64> {
    prop_oneof![
        3 => (-(1i64 << 40)..(1i64 << 40)).prop_map(|v| v as f64),
        2 => (-(1i64 << 30)..(1i64 << 30)).prop_map(|v| v as f64 + 0.5),
        2 => entry(),
    ]
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

/// The exact image of p under the exact values of the doubles, coordinate by coordinate.
fn exact_image(t: &Transform, p: [i64; 3]) -> [BigRational; 3] {
    let rows = t.rows();
    std::array::from_fn(|i| {
        let [a, b, c, s] = rows[i];
        exact(a) * int(p[0].into())
            + exact(b) * int(p[1].into())
            + exact(c) * int(p[2].into())
            + exact(s)
    })
}

fn check_certified(t: &Transform, p: [i64; 3]) -> Result<(), TestCaseError> {
    let image = exact_image(t, p);
    let rounded: Vec<BigInt> = image.iter().map(round_half_even).collect();
    let max = BigInt::from(COORD_MAX);
    let out = rounded.iter().any(|v| v.abs() > max);
    let q = P3::new(p[0], p[1], p[2]).unwrap();
    match TypedTransform::Certified(*t).apply(q) {
        Ok(r) => {
            prop_assert!(!out, "{t:?} {p:?}: accepted out of range");
            let got = [r.value().x.get(), r.value().y.get(), r.value().z.get()].map(BigInt::from);
            prop_assert_eq!(got.to_vec(), rounded, "{:?} {:?}", t, p);
            prop_assert_eq!(r.is_exact(), image.iter().all(is_integer));
        }
        Err(NumError::OutOfRange { value }) => {
            prop_assert!(out, "{t:?} {p:?}: refused");
            // The error carries the first rounded coordinate outside the frame (x, then y, then
            // z), exactly, or saturated with its sign beyond i128, as geom-num documents
            // `OutOfRange`: never an estimate or a sentinel.
            let first = rounded.iter().find(|v| v.abs() > max).unwrap();
            let saturated = if first.is_negative() {
                i128::MIN
            } else {
                i128::MAX
            };
            let want = first.to_i128().unwrap_or(saturated);
            prop_assert_eq!(value, want, "{:?} {:?}", t, p);
        }
        Err(e) => prop_assert!(false, "unexpected {e:?}"),
    }
    Ok(())
}

proptest! {
    #![proptest_config(config(4096))]

    #[test]
    fn certified_images_round_exactly(
        rows in prop::array::uniform3((prop::array::uniform3(entry()), translation())),
        p in prop::array::uniform3(coord()),
    ) {
        let t = Transform::from_rows(rows.map(|(l, s)| [l[0], l[1], l[2], s])).unwrap();
        check_certified(&t, p)?;
    }

    #[test]
    fn rotations_and_sums_land_on_steps_and_midpoints(
        k in -64i32..=64,
        h in prop_oneof![Just(0.25f64), Just(1.0 / 3.0), Just(0.1), Just(-0.7)],
        p in prop::array::uniform3(-1_000_000i64..1_000_000),
    ) {
        // Halves and quarters added to rotated points: images near steps and midpoints.
        let r = Transform::rotation(Axis::Z, h).unwrap();
        let shift = Transform::from_rows([
            [1.0, 0.0, 0.0, f64::from(k) / 4.0],
            [0.0, 1.0, 0.0, -f64::from(k) / 2.0],
            [0.0, 0.0, 1.0, 0.5],
        ]).unwrap();
        check_certified(&shift.compose(&r).unwrap(), p)?;
        check_certified(&shift, p)?;
    }
}

/// The case proptest once shrank a failure of `certified_images_round_exactly` to, kept in
/// `typed.proptest-regressions`: a single entry of 2^900, whose image is far out of range.
/// Proptest replays that file only where the test can read it, and a test under wasmtime has no
/// file system, so the case is also checked here, on every target.
#[test]
fn persisted_regression_runs_on_every_target() {
    let t = Transform::from_rows([
        [0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.0],
        [0.0, 8.452712498170644e270, 0.0, 0.0],
    ])
    .unwrap();
    check_certified(&t, [0, 1, 0]).unwrap();
}

/// An image outside the frame is refused with its rounded coordinate, exactly, or saturated with
/// its sign beyond `i128`, as geom-num documents `OutOfRange` (whose message says "at least" and
/// "at most" for the saturated ends): never a sentinel, which would read as a false bound.
#[test]
fn out_of_range_images_carry_the_rounded_coordinate() {
    let q = P3::new(COORD_MAX, -COORD_MAX, 3).unwrap();
    // 2.5·(2^40 − 1) = 2^41 + 2^39 − 2.5, a tie that rounds to the even 2^41 + 2^39 − 2; the
    // floating-point filter sees it is out of range, and the exact stage gives the value.
    let stretch = Transform::scale(2.5, 1.0, 1.0).unwrap();
    let want = (1i128 << 41) + (1i128 << 39) - 2;
    assert_eq!(
        stretch.apply_grid(q),
        Err(NumError::OutOfRange { value: want })
    );
    // The same for y, where q is −(2^40 − 1): the image is the opposite tie, which rounds to the
    // opposite even value.
    let flip = Transform::scale(1.0, 2.5, 1.0).unwrap();
    assert_eq!(
        flip.apply_grid(q),
        Err(NumError::OutOfRange { value: -want })
    );
    // Beyond i128, the sign is kept: −2^900 is at most −2^127, +2^900 at least 2^127 − 1.
    let big = pow2f(900);
    for (entry, value) in [(-big, i128::MIN), (big, i128::MAX)] {
        let t = Transform::from_rows([[entry, 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4]]).unwrap();
        assert_eq!(
            t.apply_grid(P3::new(1, 0, 0).unwrap()),
            Err(NumError::OutOfRange { value })
        );
    }
}

#[test]
fn cancelling_huge_entries_and_subnormals() {
    let big = pow2f(900);
    let cases = [
        // big·x − big·y + 0.5·z: exactly 0.5·z when x = y.
        (
            [[big, -big, 0.5, 0.0], [0.0; 4], [0.0; 4]],
            [12_345, 12_345, 3],
        ),
        (
            [[big, -big, 0.5, 0.0], [0.0; 4], [0.0; 4]],
            [12_345, 12_345, 5],
        ),
        // A subnormal entry decides a tie: 2.5 + 2^−1074 rounds up.
        (
            [[f64::from_bits(1), 0.0, 0.0, 2.5], [0.0; 4], [0.0; 4]],
            [1, 0, 0],
        ),
        (
            [[-f64::from_bits(1), 0.0, 0.0, 2.5], [0.0; 4], [0.0; 4]],
            [1, 0, 0],
        ),
        // The largest coordinates with an entry just below one.
        (
            [[1.0 - pow2f(-53), 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4]],
            [COORD_MAX, 0, 0],
        ),
        // Huge entries that do not cancel are out of range.
        ([[big, 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4]], [1, 0, 0]),
        ([[0.0, 0.0, 0.0, f64::MAX], [0.0; 4], [0.0; 4]], [0, 0, 0]),
    ];
    for (rows, p) in cases {
        let t = Transform::from_rows(rows).unwrap();
        check_certified(&t, p).unwrap();
    }
}

#[test]
fn rotations_take_the_most_exact_variant() {
    for axis in Axis::ALL {
        for turns in 0..4 {
            let t = TypedTransform::rotation(axis, Rotation::quarter_turns(turns)).unwrap();
            assert_eq!(
                t,
                TypedTransform::Exact(GridTransform::quarter_turns(axis, turns))
            );
            assert!(t.is_exact());
        }
        let r34 = TypedTransform::rotation(axis, Rotation::direction(3, 4).unwrap()).unwrap();
        assert!(matches!(r34, TypedTransform::Rational(_)));
        assert_eq!(r34.classify().unwrap().kind, TransformKind::Rigid);
        for (c, s) in [(1, 2), (-7, 3), (i32::MAX, 1), (12_345, -6_789)] {
            let rot = Rotation::direction(c, s).unwrap();
            let t = TypedTransform::rotation(axis, rot).unwrap();
            let TypedTransform::Certified(f) = t else {
                panic!("{rot:?} should be certified")
            };
            assert_eq!(t.classify().unwrap().kind, TransformKind::Rigid);
            // The cosine and sine entries are within an ulp of c/√n and s/√n.
            let n = rot.norm2();
            let (cos, sin) = match axis {
                Axis::X => (f.rows()[1][1], f.rows()[2][1]),
                Axis::Y => (f.rows()[0][0], -f.rows()[2][0]),
                Axis::Z => (f.rows()[0][0], f.rows()[1][0]),
            };
            for (d, v) in [(cos, rot.cos()), (sin, rot.sin())] {
                let v = int(v.into());
                let below = exact(d.next_down());
                let above = exact(d.next_up());
                assert_eq!(
                    sign_minus_sqrt(&v, &below, n),
                    Ordering::Greater,
                    "{rot:?} {d}"
                );
                assert_eq!(
                    sign_minus_sqrt(&v, &above, n),
                    Ordering::Less,
                    "{rot:?} {d}"
                );
            }
        }
    }
}

#[test]
fn composition_keeps_the_most_exact_variant() {
    let q = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    let shift = TypedTransform::Exact(GridTransform::translation([3, -4, 5]).unwrap());
    let r34 = TypedTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap();
    let r12 = TypedTransform::rotation(Axis::Z, Rotation::direction(1, 2).unwrap()).unwrap();
    assert!(q.compose(&shift).unwrap().is_exact());
    assert!(matches!(
        q.compose(&r34).unwrap(),
        TypedTransform::Rational(_)
    ));
    assert!(matches!(
        r34.compose(&shift).unwrap(),
        TypedTransform::Rational(_)
    ));
    // A rational rotation composed with its inverse is the identity, simplified back to exact.
    let back = r34.compose(&r34.inverse().unwrap()).unwrap();
    assert_eq!(back, TypedTransform::IDENTITY);
    // Four (3, 4) rotations are not a grid transform; the rational result says so exactly.
    let mut acc = TypedTransform::IDENTITY;
    for _ in 0..4 {
        acc = acc.compose(&r34).unwrap();
    }
    assert!(matches!(acc, TypedTransform::Rational(_)));
    assert_eq!(acc.classify().unwrap().kind, TransformKind::Rigid);
    // Anything with a certified transform is certified.
    assert!(matches!(
        r12.compose(&q).unwrap(),
        TypedTransform::Certified(_)
    ));
    assert!(matches!(
        r34.compose(&r12).unwrap(),
        TypedTransform::Certified(_)
    ));
    // A certified transform that is exactly a grid transform simplifies to it.
    let g = GridTransform::quarter_turns(Axis::X, 3)
        .compose(&GridTransform::mirror(Axis::Y))
        .unwrap();
    let g = g
        .compose(&GridTransform::translation([7, 0, -9]).unwrap())
        .unwrap();
    assert_eq!(
        TypedTransform::from(Transform::from_grid(&g)),
        TypedTransform::Exact(g)
    );
    assert_eq!(
        TypedTransform::from(RationalTransform::from_grid(&g)),
        TypedTransform::Exact(g)
    );
    // A fractional translation stays certified.
    let half = Transform::translation(geom_math::F3::new(0.5, 0.0, 0.0).unwrap());
    assert!(matches!(
        TypedTransform::from(half),
        TypedTransform::Certified(_)
    ));
    // Inverses keep their variant.
    assert!(q.inverse().unwrap().is_exact());
    assert!(matches!(
        r34.inverse().unwrap(),
        TypedTransform::Rational(_)
    ));
    assert!(matches!(
        r12.inverse().unwrap(),
        TypedTransform::Certified(_)
    ));
    assert_eq!(TypedTransform::default(), TypedTransform::IDENTITY);
}

#[test]
fn classification_and_application_by_variant() {
    use TransformKind::*;
    let kind = |t: TypedTransform| (t.classify().unwrap().kind, t.classify().unwrap().mirror);
    assert_eq!(kind(TypedTransform::IDENTITY), (Identity, false));
    let shift = TypedTransform::Exact(GridTransform::translation([1, 0, 0]).unwrap());
    assert_eq!(kind(shift), (Translation, false));
    assert_eq!(
        kind(TypedTransform::Exact(GridTransform::mirror(Axis::Z))),
        (Rigid, true)
    );
    assert_eq!(
        kind(TypedTransform::Exact(GridTransform::quarter_turns(
            Axis::Y,
            1
        ))),
        (Rigid, false)
    );
    let p = P3::new(COORD_MAX - 1, 5, -7).unwrap();
    let r = shift.apply(p).unwrap();
    assert!(r.is_exact() && r.value().x.get() == COORD_MAX);
    assert!(matches!(
        shift.apply(P3::new(COORD_MAX, 0, 0).unwrap()),
        Err(NumError::OutOfRange { .. })
    ));
    // The rational and certified forms of a (3, 4) rotation agree to one step.
    let r34 = TypedTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap();
    let f34 = TypedTransform::Certified(r34.to_transform().unwrap());
    let q = P3::new(1_000_000_007, -3, 11).unwrap();
    let (a, b) = (r34.apply(q).unwrap(), f34.apply(q).unwrap());
    assert!((a.value().x.get() - b.value().x.get()).abs() <= 1);
    assert!((a.value().y.get() - b.value().y.get()).abs() <= 1);
    // x = (3·1_000_000_007 + 12)/5 and y = (4·1_000_000_007 − 9)/5 are not whole steps.
    assert!(!a.is_exact());
    let on_grid = P3::new(5, 0, 0).unwrap();
    let c = r34.apply(on_grid).unwrap();
    assert!(c.is_exact() && (c.value().x.get(), c.value().y.get()) == (3, 4));
}
