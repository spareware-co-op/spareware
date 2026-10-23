//! Tests of `geom_math::bbox`.
//!
//! What this file proves:
//!
//! - Grid boxes: union, intersection, containment and overlap agree with per-axis brute force on
//!   random boxes and points, including touching boxes and boxes at ±(2^40 − 1); the laws hold
//!   (union and intersection are commutative and associative, intersection is `Some` exactly when
//!   the boxes overlap, the union holds both boxes, the intersection is held by both); the box of
//!   a set of points is the smallest that holds them all.
//! - Floating-point boxes: a box transformed by a general transform contains the exact rational
//!   image of every corner of the box (and so, the map being affine, of every point); a box rounded
//!   out to the grid contains the box exactly; grid boxes round-trip through `FBox3` unchanged; a
//!   bound that rounds outside the frame is refused with that bound, exactly, or saturated beyond
//!   `i128`.

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
use geom_math::{Axis, Box2, Box3, F3, FBox3, Transform};
use geom_num::NumError;
use geom_num::grid::{COORD_MAX, Coord, P2, P3};
use num_rational::BigRational;
use proptest::prelude::*;

fn coord() -> BoxedStrategy<i64> {
    prop_oneof![
        2 => -COORD_MAX..=COORD_MAX,
        1 => Just(COORD_MAX),
        1 => Just(-COORD_MAX),
        4 => -20i64..=20,
    ]
    .boxed()
}

fn point() -> BoxedStrategy<P3> {
    (coord(), coord(), coord())
        .prop_map(|(x, y, z)| P3::new(x, y, z).unwrap())
        .boxed()
}

fn grid_box() -> BoxedStrategy<Box3> {
    (point(), point())
        .prop_map(|(p, q)| Box3::from_points([p, q]).unwrap())
        .boxed()
}

fn axes(p: P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

fn brute_contains(b: &Box3, p: P3) -> bool {
    let (lo, hi, v) = (axes(b.min()), axes(b.max()), axes(p));
    (0..3).all(|i| lo[i] <= v[i] && v[i] <= hi[i])
}

fn brute_overlaps(a: &Box3, b: &Box3) -> bool {
    let (alo, ahi, blo, bhi) = (axes(a.min()), axes(a.max()), axes(b.min()), axes(b.max()));
    (0..3).all(|i| alo[i] <= bhi[i] && blo[i] <= ahi[i])
}

proptest! {
    #![proptest_config(config(4096))]

    #[test]
    fn grid_box_laws(a in grid_box(), b in grid_box(), c in grid_box(), p in point()) {
        prop_assert_eq!(a.contains(p), brute_contains(&a, p));
        prop_assert_eq!(a.overlaps(&b), brute_overlaps(&a, &b));
        prop_assert_eq!(a.overlaps(&b), a.intersection(&b).is_some());
        prop_assert_eq!(a.union(&b), b.union(&a));
        prop_assert_eq!(a.union(&b).union(&c), a.union(&b.union(&c)));
        prop_assert_eq!(a.intersection(&b), b.intersection(&a));
        let u = a.union(&b);
        prop_assert!(u.contains_box(&a) && u.contains_box(&b));
        if let Some(i) = a.intersection(&b) {
            prop_assert!(a.contains_box(&i) && b.contains_box(&i));
            let bc = b.intersection(&c);
            prop_assert_eq!(i.intersection(&c), bc.and_then(|x| a.intersection(&x)));
            prop_assert_eq!(a.contains(p) && b.contains(p), i.contains(p));
        } else {
            prop_assert!(!(a.contains(p) && b.contains(p)));
        }
        prop_assert_eq!(a.contains_box(&b), a.contains(b.min()) && a.contains(b.max()));
        let e = a.extent().unwrap();
        prop_assert!(e.x() >= 0 && e.y() >= 0 && e.z() >= 0);
    }

    #[test]
    fn box_of_points_is_smallest(pts in prop::collection::vec(point(), 1..12)) {
        let b = Box3::from_points(pts.iter().copied()).unwrap();
        prop_assert!(pts.iter().all(|&p| b.contains(p)));
        for i in 0..3 {
            let lo = pts.iter().map(|&p| axes(p)[i]).min().unwrap();
            let hi = pts.iter().map(|&p| axes(p)[i]).max().unwrap();
            prop_assert_eq!(axes(b.min())[i], lo);
            prop_assert_eq!(axes(b.max())[i], hi);
        }
        let b2 = Box2::from_points(pts.iter().map(|p| P2 { x: p.x, y: p.y })).unwrap();
        prop_assert_eq!((b2.min().x, b2.min().y), (b.min().x, b.min().y));
        prop_assert_eq!((b2.max().x, b2.max().y), (b.max().x, b.max().y));
    }

    #[test]
    fn float_boxes_enclose_exact_images(
        lo in prop::array::uniform3(-1.0e6f64..1.0e6),
        size in prop::array::uniform3(0.0f64..1.0e4),
        h in -2.0f64..2.0,
        ax in 0usize..3,
        s in prop::array::uniform3(0.1f64..10.0),
        t in prop::array::uniform3(-1.0e7f64..1.0e7),
    ) {
        let hi = [lo[0] + size[0], lo[1] + size[1], lo[2] + size[2]];
        let corner = |v: [f64; 3]| F3::new(v[0], v[1], v[2]).unwrap();
        let fb = FBox3::new(corner(lo), corner(hi)).unwrap();
        let rotate = Transform::rotation(Axis::ALL[ax], h).unwrap();
        let scale = Transform::scale(s[0], s[1], s[2]).unwrap();
        let tf = Transform::translation(F3::new(t[0], t[1], t[2]).unwrap())
            .compose(&scale.compose(&rotate).unwrap())
            .unwrap();
        let img = fb.transformed(&tf).unwrap();
        let m = tf.rows().map(|r| r.map(exact));
        let (ilo, ihi) = (img.lo().to_array().map(exact), img.hi().to_array().map(exact));
        for cx in [lo[0], hi[0]] {
            for cy in [lo[1], hi[1]] {
                for cz in [lo[2], hi[2]] {
                    let c = [exact(cx), exact(cy), exact(cz)];
                    for i in 0..3 {
                        let r = &m[i];
                        let v: BigRational = &r[0] * &c[0] + &r[1] * &c[1] + &r[2] * &c[2] + &r[3];
                        prop_assert!(ilo[i] <= v && v <= ihi[i], "axis {i}: {v} outside");
                    }
                }
            }
        }
        // Rounding out to the grid holds the box exactly, when it is in range.
        if let Ok(g) = img.to_grid_outward() {
            for i in 0..3 {
                prop_assert!(BigRational::from_integer(axes(g.min())[i].into()) <= ilo[i]);
                prop_assert!(BigRational::from_integer(axes(g.max())[i].into()) >= ihi[i]);
            }
        }
    }

    #[test]
    fn grid_boxes_round_trip_through_floats(b in grid_box(), c in grid_box()) {
        let fb = FBox3::from_box3(&b);
        prop_assert_eq!(fb.to_grid_outward().unwrap(), b);
        prop_assert_eq!(fb.overlaps(&FBox3::from_box3(&c)), b.overlaps(&c));
        prop_assert_eq!(fb.union(&FBox3::from_box3(&c)).to_grid_outward().unwrap(), b.union(&c));
        prop_assert_eq!(
            fb.intersection(&FBox3::from_box3(&c)).map(|x| x.to_grid_outward().unwrap()),
            b.intersection(&c)
        );
        prop_assert!(fb.contains(F3::from(b.min())) && fb.contains(F3::from(b.max())));
    }
}

#[test]
fn touching_and_extreme_boxes() {
    let p = |x, y, z| P3::new(x, y, z).unwrap();
    let a = Box3::new(p(0, 0, 0), p(10, 10, 10)).unwrap();
    let b = Box3::new(p(10, 0, 0), p(20, 10, 10)).unwrap();
    assert!(a.overlaps(&b));
    assert_eq!(
        a.intersection(&b).unwrap(),
        Box3::new(p(10, 0, 0), p(10, 10, 10)).unwrap()
    );
    let c = Box3::new(p(11, 0, 0), p(20, 10, 10)).unwrap();
    assert!(!a.overlaps(&c));
    assert_eq!(a.intersection(&c), None);
    assert_eq!(
        Box3::new(p(1, 0, 0), p(0, 0, 0)),
        Err(NumError::EmptyInterval)
    );
    let m = COORD_MAX;
    let all = Box3::new(p(-m, -m, -m), p(m, m, m)).unwrap();
    assert!(all.contains_box(&a));
    assert_eq!(all.extent().unwrap().to_array(), [2 * COORD_MAX; 3]);
    assert_eq!(Box3::from_points(std::iter::empty()), None);
    let flat = Box2::new(P2::new(0, 0).unwrap(), P2::new(5, 0).unwrap()).unwrap();
    assert!(flat.contains(P2::new(3, 0).unwrap()));
    assert!(!flat.contains(P2::new(3, 1).unwrap()));
    assert_eq!(flat.extent().unwrap().x(), 5);
    // A float box beyond the frame does not round to the grid.
    let far = FBox3::from_point(F3::new(COORD_MAX as f64 + 0.5, 0.0, 0.0).unwrap());
    let beyond = i128::from(COORD_MAX) + 1;
    assert_eq!(
        far.to_grid_outward(),
        Err(NumError::OutOfRange { value: beyond })
    );
    // Far beyond i64 the error still carries the rounded bound exactly, and beyond i128 it is
    // saturated with its sign, as geom-num documents `OutOfRange` (its message then says "at
    // least" or "at most").
    let huge = pow2f(100) + pow2f(48);
    let way_out = FBox3::from_point(F3::new(-huge, 0.0, 0.0).unwrap());
    assert_eq!(
        way_out.to_grid_outward(),
        Err(NumError::OutOfRange {
            value: -(1i128 << 100) - (1i128 << 48)
        })
    );
    for (v, want) in [(f64::MAX, i128::MAX), (-f64::MAX, i128::MIN)] {
        let edge = FBox3::from_point(F3::new(0.0, v, 0.0).unwrap());
        assert_eq!(
            edge.to_grid_outward(),
            Err(NumError::OutOfRange { value: want })
        );
    }
    let near = FBox3::from_point(F3::new(COORD_MAX as f64 - 0.5, -0.5, 0.0).unwrap());
    let g = near.to_grid_outward().unwrap();
    assert_eq!(
        (g.min().x, g.max().x),
        (Coord::new(COORD_MAX - 1).unwrap(), Coord::MAX)
    );
    assert_eq!((g.min().y.get(), g.max().y.get()), (-1, 0));
    assert_eq!(
        FBox3::new(F3::new(1.0, 0.0, 0.0).unwrap(), F3::ZERO),
        Err(NumError::EmptyInterval)
    );
}
