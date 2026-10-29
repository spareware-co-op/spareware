//! Attacks on piece G45, 2D transforms and boxes: the kernel's 2D transforms are its 3D ones acting
//! in the plane z = 0 (principle 8, one family for 2D and 3D; chapter 14.10).
//!
//! What this file proves: a transform acts in the plane exactly when it leaves every z unchanged;
//! each kind (grid, rational, certified, typed) applies to a 2D point or vector exactly as to the
//! 3D one at z = 0, bit for bit, errors included; a transform that does not act in the plane is
//! refused with `NotInPlane`, never projected; whole-step translations, quarter turns about z and
//! the four mirrors through the origin (x, y, y = x, y = −x) are exact and match their matrices,
//! four quarter turns and two mirrors give the identity, and the mirrors reverse orientation;
//! applying a composition is applying its parts in turn, for grid, rational and typed transforms;
//! grid boxes move exactly; and `FBox2` holds the image of every point of a box under a certified
//! transform, and round-trips a grid box exactly.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::disallowed_methods,
    clippy::too_many_lines,
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

use common::{config, exact};
use geom_math::grid_transform::TRANSLATION_MAX;
use geom_math::{
    Axis, Box2, F2, F3, FBox2, GridTransform, MathError, RationalTransform, Rotation, Transform,
    TypedTransform, V2, V3,
};
use geom_num::NumError;
use geom_num::grid::{COORD_MAX, Coord, P2, P3};
use num_rational::BigRational;
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

/// Any grid transform.
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

/// A grid transform of the plane: x and y permuted and negated, z left alone.
fn plane_transform() -> BoxedStrategy<GridTransform> {
    (
        any::<bool>(),
        any::<[bool; 2]>(),
        translation(),
        translation(),
    )
        .prop_map(|(swap, [n0, n1], a, b)| {
            let source = if swap {
                [Axis::Y, Axis::X, Axis::Z]
            } else {
                [Axis::X, Axis::Y, Axis::Z]
            };
            GridTransform::from_parts(source, [n0, n1, false], [a, b, 0]).unwrap()
        })
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

fn point() -> BoxedStrategy<P2> {
    (coord(), coord())
        .prop_map(|(x, y)| P2::new(x, y).unwrap())
        .boxed()
}

/// A coordinate small enough that a rotation, a scale by 5 or a translation keeps it in range.
fn small() -> BoxedStrategy<i64> {
    (-(1i64 << 36)..=(1 << 36)).boxed()
}

fn at_z0(p: P2) -> P3 {
    P3 {
        x: p.x,
        y: p.y,
        z: Coord::ZERO,
    }
}

fn in_plane(q: P3) -> P2 {
    assert_eq!(q.z, Coord::ZERO, "z stays 0");
    P2 { x: q.x, y: q.y }
}

proptest! {
    #![proptest_config(config(4096))]

    /// A grid transform acts in the plane exactly when its z row is +z with no translation, and
    /// then a 2D point and vector move as the 3D ones at z = 0, errors included.
    #[test]
    fn grid_2d_is_3d_at_z0(t in transform(), p in point(), v in (coord(), coord())) {
        let m = t.matrix();
        let keeps = m[2] == [0, 0, 1] && t.translation_part()[2] == 0;
        prop_assert_eq!(t.acts_in_plane(), keeps);
        let v = V2::new(v.0, v.1).unwrap();
        if keeps {
            prop_assert_eq!(t.apply_2d(p), t.apply(at_z0(p)).map(in_plane).map_err(MathError::from));
            let w = t.apply_vector(V3::new(v.x(), v.y(), 0).unwrap()).unwrap();
            prop_assert_eq!(t.apply_vector_2d(v), Ok(V2::new(w.x(), w.y()).unwrap()));
        } else {
            prop_assert_eq!(t.apply_2d(p), Err(MathError::NotInPlane));
            prop_assert_eq!(t.apply_vector_2d(v), Err(MathError::NotInPlane));
            prop_assert_eq!(Box2::new(p, p).unwrap().transformed(&t), Err(MathError::NotInPlane));
        }
    }

    /// Applying a composition of plane transforms is applying its parts in turn, and the inverse
    /// undoes it.
    #[test]
    fn plane_composition_is_application_in_turn(a in plane_transform(), b in plane_transform(), p in point()) {
        let (Ok(ab), Ok(bp)) = (a.compose(&b), b.apply_2d(p)) else {
            return Ok(());
        };
        prop_assert!(ab.acts_in_plane());
        let direct = ab.apply_2d(p);
        prop_assert_eq!(direct, a.apply_2d(bp));
        if let Ok(q) = direct {
            prop_assert_eq!(ab.inverse().unwrap().apply_2d(q), Ok(p));
        }
    }

    /// A grid box moved in the plane is the box of its four corners' images.
    #[test]
    fn plane_boxes_move_exactly(t in plane_transform(), a in point(), b in point()) {
        let lo = P2::new(a.x.get().min(b.x.get()), a.y.get().min(b.y.get())).unwrap();
        let hi = P2::new(a.x.get().max(b.x.get()), a.y.get().max(b.y.get())).unwrap();
        let bx = Box2::new(lo, hi).unwrap();
        let corners = [
            lo,
            hi,
            P2::new(lo.x.get(), hi.y.get()).unwrap(),
            P2::new(hi.x.get(), lo.y.get()).unwrap(),
        ];
        let images: Result<Vec<P2>, MathError> = corners.iter().map(|c| t.apply_2d(*c)).collect();
        match (bx.transformed(&t), images) {
            (Ok(moved), Ok(images)) => {
                prop_assert_eq!(moved, Box2::from_points(images).unwrap());
            }
            (Err(e), Err(_)) => {
                let out_of_range = matches!(e, MathError::Num(NumError::OutOfRange { .. }));
                prop_assert!(out_of_range, "{:?}", e);
            },
            (moved, images) => prop_assert!(false, "{moved:?} against {images:?}"),
        }
    }

    /// A rational transform of the plane (a Pythagorean rotation about z, a translation in
    /// fifths) moves a 2D point as the 3D one at z = 0, and, after an exact grid transform, its
    /// composition with it is applying them in turn.
    #[test]
    fn rational_2d_is_3d_at_z0(
        g in plane_transform(),
        tx in small(),
        ty in small(),
        x in small(),
        y in small(),
    ) {
        let rotation = RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap();
        let shift = RationalTransform::translation([i128::from(tx), i128::from(ty), 0], 5).unwrap();
        let r = shift.compose(&rotation).unwrap();
        prop_assert!(r.acts_in_plane());
        let p = P2::new(x, y).unwrap();
        let three = r.apply(at_z0(p)).unwrap();
        let two = r.apply_2d(p).unwrap();
        prop_assert_eq!(two.value(), in_plane(three.value()));
        prop_assert_eq!(two.is_exact(), three.is_exact());
        // The grid transform's image is exact, so applying in turn rounds once, as the
        // composition does.
        if let Ok(gp) = g.apply_2d(p) {
            let rg = r.compose(&RationalTransform::from_grid(&g)).unwrap();
            prop_assert_eq!(rg.apply_2d(p), r.apply_2d(gp));
        }
        let v = V2::new(x, y).unwrap();
        let w = r.apply_vector(V3::new(x, y, 0).unwrap()).unwrap();
        let w2 = r.apply_vector_2d(v).unwrap();
        prop_assert_eq!(w2.value(), V2::new(w.value().x(), w.value().y()).unwrap());
        prop_assert_eq!(w2.is_exact(), w.is_exact());
    }

    /// A certified transform of the plane moves a 2D point and grid point as the 3D ones at
    /// z = 0, bit for bit, and `FBox2` holds every image of a box's points.
    #[test]
    fn certified_2d_is_3d_at_z0_and_boxes_hold_images(
        half_turns in -2.0f64..2.0,
        tx in -1.0e6f64..1.0e6,
        ty in -1.0e6f64..1.0e6,
        x in small(),
        y in small(),
        w in 0i64..=1_000_000,
        h in 0i64..=1_000_000,
        fx in 0.0f64..=1.0,
        fy in 0.0f64..=1.0,
    ) {
        let t = Transform::translation(F3::new(tx, ty, 0.0).unwrap())
            .compose(&Transform::rotation(Axis::Z, half_turns).unwrap())
            .unwrap();
        prop_assert!(t.acts_in_plane());
        let p = P2::new(x, y).unwrap();
        let three = t.apply_grid(at_z0(p)).unwrap();
        let two = t.apply_grid_2d(p).unwrap();
        prop_assert_eq!(two.value(), in_plane(three.value()));
        prop_assert_eq!(two.is_exact(), three.is_exact());
        let f = F2::from(p);
        let f3 = t.apply_point(F3::new(f.x(), f.y(), 0.0).unwrap()).unwrap();
        let f2 = t.apply_point_2d(f).unwrap();
        prop_assert_eq!(f2.x().to_bits(), f3.x().to_bits());
        prop_assert_eq!(f2.y().to_bits(), f3.y().to_bits());
        // A box and a point inside it (fx, fy of the way across): the moved box holds its image.
        let bx = Box2::new(p, P2::new(x + w, y + h).unwrap()).unwrap();
        let fb = FBox2::from_box2(&bx);
        prop_assert_eq!(fb.to_grid_outward(), Ok(bx));
        let inside = F2::new(x as f64 + fx * w as f64, y as f64 + fy * h as f64).unwrap();
        prop_assert!(fb.contains(inside));
        // The moved box holds the exact image of every corner, and so, the map being affine, of
        // every point of the box.
        let moved = fb.transformed(&t).unwrap();
        let rows = t.rows().map(|r| r.map(exact));
        let (lo, hi) = (moved.lo().to_array().map(exact), moved.hi().to_array().map(exact));
        for c in [fb.lo(), fb.hi(), F2::new(fb.lo().x(), fb.hi().y()).unwrap(), F2::new(fb.hi().x(), fb.lo().y()).unwrap()] {
            let (cx, cy) = (exact(c.x()), exact(c.y()));
            for i in 0..2 {
                let r = &rows[i];
                let v: BigRational = &r[0] * &cx + &r[1] * &cy + &r[3];
                prop_assert!(lo[i] <= v && v <= hi[i], "axis {i}: {v} outside");
            }
        }
    }
}

/// Quarter turns about z and the four mirrors through the origin match their matrices exactly,
/// four quarter turns and two mirrors give the identity, and only the mirrors reverse orientation.
#[test]
fn quarter_turns_and_mirrors_are_exact() {
    let p = P2::new(3, -7).unwrap();
    let turns = [(3, -7), (7, 3), (-3, 7), (-7, -3)];
    for (k, (x, y)) in turns.iter().enumerate() {
        let t = GridTransform::quarter_turns(Axis::Z, k as i32);
        assert!(t.acts_in_plane());
        assert!(!t.reverses_orientation());
        assert_eq!(
            t.apply_2d(p),
            Ok(P2::new(*x, *y).unwrap()),
            "{k} quarter turns"
        );
    }
    let quarter = GridTransform::quarter_turns(Axis::Z, 1);
    let mut four = GridTransform::IDENTITY;
    for _ in 0..4 {
        four = quarter.compose(&four).unwrap();
    }
    assert!(four.is_identity());
    let mirrors = [
        (GridTransform::mirror(Axis::X), (-3, -7)),
        (GridTransform::mirror(Axis::Y), (3, 7)),
        (GridTransform::mirror_diagonal(false), (-7, 3)),
        (GridTransform::mirror_diagonal(true), (7, -3)),
    ];
    for (m, (x, y)) in mirrors {
        assert!(m.acts_in_plane());
        assert!(m.reverses_orientation());
        assert_eq!(m.apply_2d(p), Ok(P2::new(x, y).unwrap()));
        assert!(m.compose(&m).unwrap().is_identity());
    }
    assert_eq!(
        GridTransform::mirror_diagonal(false).matrix(),
        [[0, 1, 0], [1, 0, 0], [0, 0, 1]]
    );
    assert_eq!(
        GridTransform::mirror_diagonal(true).matrix(),
        [[0, -1, 0], [-1, 0, 0], [0, 0, 1]]
    );
    // A mirror about the line x = 5: mirror x, then translate by 10.
    let about = GridTransform::translation([10, 0, 0])
        .unwrap()
        .compose(&GridTransform::mirror(Axis::X))
        .unwrap();
    assert_eq!(
        about.apply_2d(P2::new(5, 2).unwrap()),
        Ok(P2::new(5, 2).unwrap())
    );
    assert_eq!(about.apply_2d(p), Ok(P2::new(7, -7).unwrap()));
    // Quarter turns about x or y, and a mirror of z, do not act in the plane.
    for t in [
        GridTransform::quarter_turns(Axis::X, 1),
        GridTransform::quarter_turns(Axis::Y, 3),
        GridTransform::mirror(Axis::Z),
        GridTransform::translation([0, 0, 1]).unwrap(),
    ] {
        assert!(!t.acts_in_plane());
        assert_eq!(t.apply_2d(p), Err(MathError::NotInPlane));
    }
}

/// The range limits hold in the plane: an image beyond ±(2^40 − 1) is `OutOfRange`, carrying the
/// coordinate.
#[test]
fn plane_range_limits_are_enforced() {
    let edge = P2::new(COORD_MAX, -COORD_MAX).unwrap();
    let step = GridTransform::translation([1, 0, 0]).unwrap();
    assert_eq!(
        step.apply_2d(edge),
        Err(MathError::Num(NumError::OutOfRange {
            value: i128::from(COORD_MAX) + 1
        }))
    );
    let back = GridTransform::translation([-1, 0, 0]).unwrap();
    assert_eq!(
        back.apply_2d(edge),
        Ok(P2::new(COORD_MAX - 1, -COORD_MAX).unwrap())
    );
}

/// The typed transform applies to 2D points by variant, each as its own kind does, and refuses
/// one that does not act in the plane.
#[test]
fn typed_transforms_apply_in_the_plane_by_variant() {
    let p = P2::new(11, -4).unwrap();
    let exact = TypedTransform::rotation(Axis::Z, Rotation::quarter_turns(1)).unwrap();
    assert!(matches!(exact, TypedTransform::Exact(_)));
    let q = exact.apply_2d(p).unwrap();
    assert!(q.is_exact());
    assert_eq!(q.value(), P2::new(4, 11).unwrap());
    let rational = TypedTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap();
    assert!(matches!(rational, TypedTransform::Rational(_)));
    // (3·11 − 4·(−4), 4·11 + 3·(−4))/5 = (49, 32)/5 = (9.8, 6.4), rounded once.
    let q = rational.apply_2d(p).unwrap();
    assert!(!q.is_exact());
    assert_eq!(q.value(), P2::new(10, 6).unwrap());
    let certified = TypedTransform::Certified(Transform::rotation(Axis::Z, 0.25).unwrap());
    assert!(certified.acts_in_plane());
    let r = certified.apply_2d(p).unwrap();
    assert_eq!(
        r.value(),
        TypedTransform::Certified(Transform::rotation(Axis::Z, 0.25).unwrap())
            .apply(at_z0(p))
            .map(|q| in_plane(q.value()))
            .unwrap()
    );
    // Composition in the plane: a quarter turn after the Pythagorean rotation is applied in turn
    // to a point whose rotated image is exact: (5, 0) ↦ (3, 4) ↦ (−4, 3).
    let both = exact.compose(&rational).unwrap();
    assert!(both.acts_in_plane());
    let five = P2::new(5, 0).unwrap();
    assert_eq!(
        both.apply_2d(five).unwrap().value(),
        P2::new(-4, 3).unwrap()
    );
    let tilted = TypedTransform::rotation(Axis::X, Rotation::direction(3, 4).unwrap()).unwrap();
    assert!(!tilted.acts_in_plane());
    assert_eq!(tilted.apply_2d(p), Err(MathError::NotInPlane));
    assert_eq!(
        FBox2::from_point(F2::from(p)).transformed(&Transform::rotation(Axis::Y, 0.5).unwrap()),
        Err(MathError::NotInPlane)
    );
}

/// The edges the mutation gate found unchecked: a certified transform's 2D vectors in and out of
/// the plane, the exactness flag of a 2D grid point when one coordinate rounds and the other does
/// not, the refusal's message, and `FBox2`'s intersection, containment and overlap, touching
/// included.
#[test]
fn certified_vectors_exactness_and_float_boxes_at_their_edges() {
    let turn = Transform::rotation(Axis::Z, 0.5).unwrap();
    let v = F2::new(3.0, -2.0).unwrap();
    let three = turn.apply_vector(F3::new(3.0, -2.0, 0.0).unwrap()).unwrap();
    let two = turn.apply_vector_2d(v).unwrap();
    assert_eq!(
        (two.x().to_bits(), two.y().to_bits()),
        (three.x().to_bits(), three.y().to_bits())
    );
    let tilt = Transform::rotation(Axis::X, 0.5).unwrap();
    assert_eq!(tilt.apply_vector_2d(v), Err(MathError::NotInPlane));
    assert_eq!(
        MathError::NotInPlane.to_string(),
        "the transform changes z, so it does not act in the plane"
    );
    // Halving y: x stays a whole step, y = 3/2 rounds (to 2, ties to even), so the point is not
    // exact; with y = 4 both rows are whole and it is.
    let half_y = Transform::scale(1.0, 0.5, 1.0).unwrap();
    let rounded = half_y.apply_grid_2d(P2::new(2, 3).unwrap()).unwrap();
    assert_eq!(rounded.value(), P2::new(2, 2).unwrap());
    assert!(!rounded.is_exact());
    let whole = half_y.apply_grid_2d(P2::new(2, 4).unwrap()).unwrap();
    assert_eq!(whole.value(), P2::new(2, 2).unwrap());
    assert!(whole.is_exact());
    let half_x = Transform::scale(0.5, 1.0, 1.0).unwrap();
    assert!(
        !half_x
            .apply_grid_2d(P2::new(3, 2).unwrap())
            .unwrap()
            .is_exact()
    );

    let fbox = |x0: f64, y0: f64, x1: f64, y1: f64| {
        FBox2::new(F2::new(x0, y0).unwrap(), F2::new(x1, y1).unwrap()).unwrap()
    };
    let corners = |b: FBox2| (b.lo().to_array(), b.hi().to_array());
    let a = fbox(0.0, 0.5, 2.0, 2.5);
    let b = fbox(1.0, 1.5, 3.0, 3.5);
    assert_eq!(
        a.intersection(&b).map(corners),
        Some(([1.0, 1.5], [2.0, 2.5]))
    );
    // Touching at one corner: the intersection is that point.
    let corner = fbox(2.0, 2.5, 3.0, 4.0);
    assert_eq!(
        a.intersection(&corner).map(corners),
        Some(([2.0, 2.5], [2.0, 2.5]))
    );
    assert!(a.overlaps(&corner) && a.overlaps(&b));
    // Apart along x only, then along y only.
    let right = fbox(2.25, 0.5, 4.0, 2.5);
    let above = fbox(0.0, 2.75, 2.0, 4.0);
    assert_eq!(a.intersection(&right), None);
    assert_eq!(a.intersection(&above), None);
    assert!(!a.overlaps(&right) && !a.overlaps(&above));
    assert!(a.contains(F2::new(1.0, 1.0).unwrap()));
    assert!(a.contains(F2::new(2.0, 0.5).unwrap()));
    assert!(!a.contains(F2::new(2.5, 1.0).unwrap()));
    assert!(!a.contains(F2::new(1.0, 0.25).unwrap()));
}
