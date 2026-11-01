//! Tests of `geom_sketch::plane` (piece G216): sketch planes, their maps and their metric.
//!
//! The board's done-when: "Lattice and rotated frames, Gram metric, maps with stated class; exact
//! round trips on axis-aligned and rational-pitch planes."
//!
//! Seeds: each test prints `seed: <hex>` and replays with `SPAREKIT_SEED`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::many_single_char_names,
    reason = "tests, written as the formulas read; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_num::{Exactness, GridStep, P3, Steps};
use geom_sketch::plane::{
    LATTICE_CELL, Lattice, Placed, PlaneError, PlaneFrame, Rotated, SketchPlane,
};

/// The grid's largest coordinate.
const FAR: i64 = (1 << 40) - 1;

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// A fixed linear congruential sequence: nothing here depends on which numbers come out.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 20
    }
    fn within(&mut self, reach: i64) -> i64 {
        (self.next() % (2 * reach as u64 + 1)) as i64 - reach
    }
}

#[test]
fn an_axis_aligned_plane_is_the_grid_itself() {
    let origin = p(7, -3, 11);
    let plan = Lattice::new(origin, [1, 0, 0], [0, 1, 0]).unwrap();
    assert_eq!(plan.place(0, 0).unwrap(), origin);
    assert_eq!(plan.place(5, -2).unwrap(), p(12, -5, 11));
    assert_eq!(plan.locate(p(12, -5, 11)).unwrap(), (5, -2));
    assert_eq!(plan.gram(), [1, 0, 1]);
    assert_eq!(plan.normal(), [0, 0, 1]);
    assert_eq!(plan.length_squared(3, 4).unwrap(), 25);
    // Each axis-aligned plane, at the very edge of the range.
    for (u, v) in [
        ([1, 0, 0], [0, 1, 0]),
        ([0, 1, 0], [0, 0, 1]),
        ([0, 0, 1], [1, 0, 0]),
    ] {
        let frame = Lattice::new(p(0, 0, 0), u, v).unwrap();
        for (s, t) in [(FAR, FAR), (-FAR, FAR), (FAR, -FAR), (-FAR, -FAR)] {
            let point = frame.place(s, t).unwrap();
            assert_eq!(frame.locate(point).unwrap(), (s, t));
        }
        assert_eq!(frame.place(FAR + 1, 0), Err(PlaneError::OutOfRange));
    }
}

#[test]
fn a_roof_measures_its_true_length() {
    // A 5-in-12 roof: y runs 12 for every 5 z rises; the rafter's unit is 13 steps long.
    let roof = Lattice::of_normal(p(0, 0, 0), [0, -5, 12]).unwrap();
    assert_eq!(roof.gram(), [1, 0, 169]);
    assert_eq!(roof.normal(), [0, -5, 12]);
    assert_eq!(roof.place(0, 1).unwrap(), p(0, 12, 5));
    assert_eq!(roof.length_squared(0, 7).unwrap(), 49 * 169);
    assert_eq!(roof.length_squared(3, 4).unwrap(), 9 + 16 * 169);
    assert_eq!(roof.inner((1, 1), (1, -1)).unwrap(), 1 - 169);
    assert_eq!(roof.inner((2, 3), (5, 7)).unwrap(), 10 + 21 * 169);
    // Skew bases measure through the off-diagonal.
    let skew = Lattice::new(p(0, 0, 0), [1, 0, 0], [1, 1, 0]).unwrap();
    assert_eq!(skew.gram(), [1, 1, 2]);
    assert_eq!(skew.inner((1, 2), (3, 4)).unwrap(), 3 + (4 + 6) + 8 * 2);
}

#[test]
fn every_small_pitch_round_trips_exactly() {
    let mut rng = Lcg(0x216);
    let pitches = [
        [0, 1, 1],
        [0, -1, 1],
        [1, 1, 0],
        [1, 1, 1],
        [1, 2, 3],
        [0, 3, 4],
        [0, -5, 12],
        [3, 4, 12],
        [2, -3, 6],
        [-3, 4, 0],
        [1, -1, 2],
        [6, 7, 9],
        [0, 0, 1],
        [0, 0, -1],
        [0, 2, 0],
        [4, 0, 0],
        [2, 4, 6],
    ];
    for normal in pitches {
        let origin = p(
            rng.within(1 << 30),
            rng.within(1 << 30),
            rng.within(1 << 30),
        );
        let frame = Lattice::of_normal(origin, normal).unwrap();
        // Primitive, the given way round, and short: a reduced basis's product is near the area.
        let g = gcd(gcd(normal[0], normal[1]), normal[2]);
        assert_eq!(
            frame.normal(),
            normal.map(|x| i128::from(x / g)),
            "{normal:?}"
        );
        let [a, b, c] = frame.gram();
        assert!(2 * b.abs() <= a && a <= c, "{normal:?}: {:?}", frame.gram());
        for _ in 0..2000 {
            let (s, t) = (rng.within(1 << 24), rng.within(1 << 24));
            let point = frame.place(s, t).unwrap();
            assert_eq!(frame.locate(point).unwrap(), (s, t), "{normal:?}");
            let d = [
                i128::from(point.x.get() - origin.x.get()),
                i128::from(point.y.get() - origin.y.get()),
                i128::from(point.z.get() - origin.z.get()),
            ];
            assert_eq!(
                d[0] * i128::from(normal[0])
                    + d[1] * i128::from(normal[1])
                    + d[2] * i128::from(normal[2]),
                0
            );
            assert_eq!(
                frame.length_squared(s, t).unwrap(),
                d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
            );
        }
    }
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

#[test]
fn every_grid_point_of_the_plane_is_a_lattice_point() {
    // Walk the box: each grid point on the plane through the origin must locate and come back.
    for normal in [[1, 2, 3], [0, 3, 4], [2, -3, 6], [1, 1, 1]] {
        let frame = Lattice::of_normal(p(0, 0, 0), normal).unwrap();
        let mut found = 0;
        for x in -20_i64..=20 {
            for y in -20_i64..=20 {
                for z in -20_i64..=20 {
                    let on = x * normal[0] + y * normal[1] + z * normal[2] == 0;
                    let point = p(x, y, z);
                    match frame.locate(point) {
                        Ok((s, t)) => {
                            assert!(on);
                            assert_eq!(frame.place(s, t).unwrap(), point);
                            found += 1;
                        }
                        Err(e) => {
                            assert!(!on);
                            assert_eq!(e, PlaneError::OffPlane);
                        }
                    }
                }
            }
        }
        assert!(found > 40, "{normal:?}: {found}");
    }
}

#[test]
fn one_plane_has_one_frame() {
    let a = Lattice::of_normal(p(1, 2, 3), [2, -3, 6]).unwrap();
    let b = Lattice::of_normal(p(1, 2, 3), [4, -6, 12]).unwrap();
    assert_eq!(a, b);
    // The reverse normal gives the same u, and the reverse v.
    let c = Lattice::of_normal(p(1, 2, 3), [-2, 3, -6]).unwrap();
    assert_eq!(c.normal(), [-2, 3, -6]);
    assert_eq!(c.place(1, 0).unwrap(), a.place(1, 0).unwrap());
    assert_eq!(c.place(0, 1).unwrap(), a.place(0, -1).unwrap());
}

#[test]
fn what_is_not_a_frame_is_refused() {
    let o = p(0, 0, 0);
    assert_eq!(
        Lattice::new(o, [1, 2, 3], [2, 4, 6]),
        Err(PlaneError::Degenerate)
    );
    assert_eq!(
        Lattice::new(o, [0, 0, 0], [1, 0, 0]),
        Err(PlaneError::Degenerate)
    );
    assert_eq!(
        Lattice::of_normal(o, [0, 0, 0]),
        Err(PlaneError::Degenerate)
    );
    // Every second grid point: the cross product is 4 times a primitive one.
    assert_eq!(
        Lattice::new(o, [2, 0, 0], [0, 2, 0]),
        Err(PlaneError::NotPrimitive)
    );
    assert_eq!(
        Lattice::new(o, [1, 0, 0], [0, 2, 0]),
        Err(PlaneError::NotPrimitive)
    );
    // The cell's diagonal: 16 steps is in, and past it is out.
    assert!(Lattice::new(o, [1, 0, 0], [0, 15, 0]).is_err());
    let in_cell = Lattice::new(o, [1, 0, 0], [15, 1, 0]);
    assert_eq!(
        in_cell,
        Err(PlaneError::CellTooLarge {
            diagonal_squared: 1 + 226 + 30
        })
    );
    let exactly = Lattice::of_normal(o, [0, -4, 15]).unwrap();
    assert_eq!(exactly.gram(), [1, 0, 241]);
    assert_eq!(
        Lattice::of_normal(o, [0, -12, 11]),
        Err(PlaneError::CellTooLarge {
            diagonal_squared: 1 + 265
        })
    );
    assert_eq!(LATTICE_CELL, Steps(16));
    let plan = Lattice::of_normal(o, [0, 0, 1]).unwrap();
    assert_eq!(plan.locate(p(1, 1, 1)), Err(PlaneError::OffPlane));
    // A sum past the integers' range is refused, not wrapped.
    let huge = Lattice::new(p(FAR, FAR, FAR), [1, 0, 0], [0, 1, 0]).unwrap();
    assert_eq!(huge.place(1, 0), Err(PlaneError::OutOfRange));
    assert_eq!(plan.place(i64::MAX, i64::MAX), Err(PlaneError::OutOfRange));
    assert!(plan.length_squared(i64::MAX, 0).is_ok());
    let tall = Lattice::of_normal(o, [0, -1, 1]).unwrap();
    assert_eq!(
        tall.inner((i64::MAX, i64::MAX), (i64::MAX, i64::MAX)),
        Err(PlaneError::OutOfRange)
    );
    assert_eq!(
        Lattice::new(o, [i64::MAX, 0, 0], [0, i64::MAX, 1]),
        Err(PlaneError::NotPrimitive)
    );
}

#[test]
fn errors_say_what_went_wrong() {
    assert_eq!(
        PlaneError::Degenerate.to_string(),
        "the vectors do not span a plane"
    );
    assert_eq!(
        PlaneError::NotPrimitive.to_string(),
        "the vectors do not reach every grid point of the plane"
    );
    assert_eq!(
        PlaneError::CellTooLarge {
            diagonal_squared: 300
        }
        .to_string(),
        "the lattice cell is too large"
    );
    assert_eq!(
        PlaneError::OffPlane.to_string(),
        "the point is not on the plane"
    );
    assert_eq!(
        PlaneError::NotOrthonormal.to_string(),
        "the axes are not orthonormal"
    );
    assert_eq!(
        PlaneError::OutOfRange.to_string(),
        "a value leaves the grid's range"
    );
}

/// A rotated frame's axes by a turn of `a` radians about z, then a tilt of `b` about the new x.
fn axes(a: f64, b: f64) -> ([f64; 3], [f64; 3]) {
    let (sa, ca) = a.sin_cos();
    let (sb, cb) = b.sin_cos();
    ([ca, sa, 0.0], [-sa * cb, ca * cb, sb])
}

#[test]
fn a_rotated_frame_places_within_a_step() {
    let mut rng = Lcg(0x217);
    let (x, y) = axes(37.0_f64.to_radians(), 11.0_f64.to_radians());
    let origin = p(1000, -2000, 3000);
    let frame = Rotated::new(origin, x, y).unwrap();
    for _ in 0..5000 {
        let (s, t) = (rng.within(1 << 38), rng.within(1 << 38));
        let point = frame.place(s, t).unwrap();
        // Coordinates in exact rationals of doubles, the axes taken as given.
        let exact = |o: i64, a: f64, b: f64| {
            let (s, t) = (s as f64, t as f64);
            o as f64 + s * a + t * b
        };
        let reach = [
            point.x.get() as f64 - exact(1000, x[0], y[0]),
            point.y.get() as f64 - exact(-2000, x[1], y[1]),
            point.z.get() as f64 - exact(3000, x[2], y[2]),
        ];
        let size = reach.iter().map(|v| v * v).fold(0.0, |a, b| a + b).sqrt();
        assert!(size <= 3.0_f64.sqrt() / 2.0 + 1e-3, "{size}");
        let (back_s, back_t, off) = frame.project(point);
        assert!((back_s - s as f64).abs() <= 1.0 + 1e-3, "{back_s} {s}");
        assert!((back_t - t as f64).abs() <= 1.0 + 1e-3, "{back_t} {t}");
        assert!(off <= 1.0 + 1e-3, "{off}");
    }
    assert_eq!(frame.place(1 << 42, 0), Err(PlaneError::OutOfRange));
    let (near, _, off) = frame.project(origin);
    assert_eq!(near, 0.0);
    assert_eq!(off, 0.0);
}

#[test]
fn a_rotated_frame_is_orthonormal_or_refused() {
    let o = p(0, 0, 0);
    let (x, y) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    assert!(Rotated::new(o, x, y).is_ok());
    assert_eq!(
        Rotated::new(o, [1.0 + 1e-10, 0.0, 0.0], y),
        Err(PlaneError::NotOrthonormal)
    );
    assert_eq!(
        Rotated::new(o, x, [0.0, 1.0 - 1e-10, 0.0]),
        Err(PlaneError::NotOrthonormal)
    );
    assert_eq!(
        Rotated::new(o, x, [1e-10, 1.0, 0.0]),
        Err(PlaneError::NotOrthonormal)
    );
    assert_eq!(
        Rotated::new(o, [f64::NAN, 0.0, 0.0], y),
        Err(PlaneError::NotOrthonormal)
    );
    assert_eq!(
        Rotated::new(o, x, [f64::INFINITY, 0.0, 0.0]),
        Err(PlaneError::NotOrthonormal)
    );
    assert!(Rotated::new(o, [1.0 + 1e-13, 0.0, 0.0], [1e-13, 1.0, 0.0]).is_ok());
}

#[test]
fn a_placed_point_states_its_class() {
    let lattice = SketchPlane::new(
        PlaneFrame::Lattice(Lattice::of_normal(p(0, 0, 0), [0, -5, 12]).unwrap()),
        GridStep::Q0,
    );
    assert!(matches!(lattice.frame(), PlaneFrame::Lattice(_)));
    assert_eq!(
        lattice.place(2, 1).unwrap(),
        Placed {
            point: p(2, 12, 5),
            class: Exactness::Exact
        }
    );
    let rotated = SketchPlane::new(
        PlaneFrame::Rotated(Rotated::new(p(0, 0, 0), [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap()),
        GridStep::Q1,
    );
    assert!(matches!(rotated.frame(), PlaneFrame::Rotated(_)));
    assert_eq!(
        rotated.place(4, -6).unwrap(),
        Placed {
            point: p(4, -6, 0),
            class: Exactness::Certified {
                bound: Steps(1),
                step: GridStep::Q1
            }
        }
    );
    assert_eq!(lattice.place(i64::MAX, 0), Err(PlaneError::OutOfRange));
    assert_eq!(rotated.place(i64::MAX, 0), Err(PlaneError::OutOfRange));
}

#[test]
fn a_rotated_frame_measures_its_height_off_the_plane() {
    // No part of either axis is zero, so every term of the normal counts: (3, -6, 6) is 9 along it.
    let third = 1.0 / 3.0;
    let frame = Rotated::new(
        p(0, 0, 0),
        [2.0 * third, 2.0 * third, third],
        [-2.0 * third, third, 2.0 * third],
    )
    .unwrap();
    let (s, t, height) = frame.project(p(3, -6, 6));
    assert!(s.abs() < 1e-9 && t.abs() < 1e-9);
    assert!((height - 9.0).abs() < 1e-9, "{height}");
}
