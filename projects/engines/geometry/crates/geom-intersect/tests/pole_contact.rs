//! A plane touching a sphere at its north pole (v = 1/2), the other pole to `attack_g173`'s south
//! one: in the sphere's own chart the pole has no tangent plane, so the contact is read in a chart
//! turned to put that pole on the equator (chapter 12.10), and the reading is put back at the pole.
//! Whole spheres and a cap that ends short of the south pole both read one tangent point there.
//! And a NURBS whose first row in u is one point, its tip, resting on a plane that meets it there
//! alone: a lone pole on the u side, read as one singular point over the whole line u = 0.

#![allow(
    clippy::unwrap_used,
    clippy::many_single_char_names,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::contact::{Reading, analyse};
use geom_intersect::general::subdivide;
use geom_intersect::result::{PointContact, Unresolved};
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, Sphere, SurfaceKind};

const TOUCH: f64 = 1.0 / 16.0;
const CEILING: f64 = 1.0 / 16.0;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

/// The sphere of radius 5 hanging below the plane z = 0, its north pole on it.
fn hanging(v: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(
            [0.0625, 0.03125, -5.0],
            5.0,
            Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            ((0.0, 2.0), v),
        )
        .unwrap(),
    )
}

fn ceiling() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0625, 0.03125, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-9.0, 9.0), (-9.0, 9.0)),
        )
        .unwrap(),
    )
}

fn one_point_at_the_north_pole(ball: &SurfaceKind) {
    one_point_at_a_pole(ball, 0.5);
}

/// The plane touches `ball` at one tangent point, its top, which is the pole at v = `pole`.
fn one_point_at_a_pole(ball: &SurfaceKind, pole: f64) {
    let plane = ceiling();
    for (one, two, swapped) in [(ball, &plane, false), (&plane, ball, true)] {
        let found = subdivide(one, two, TOUCH, &mut meter()).unwrap();
        assert!(found.settled.is_empty(), "{found:?}");
        let read = analyse(one, two, &found, TOUCH, CEILING, &mut meter()).unwrap();
        let [Reading::Point { at, gap, contact }] = read.as_slice() else {
            panic!("{read:?}");
        };
        assert_eq!(*contact, PointContact::IsolatedTangent, "{read:?}");
        assert!(gap.contains(0.0), "{gap:?}");
        let v = if swapped { at[3] } else { at[1] };
        assert_eq!((v.lo(), v.hi()), (pole, pole), "{at:?}");
    }
}

#[test]
fn a_sphere_hanging_from_a_plane_at_its_north_pole_touches_it_at_one_point() {
    one_point_at_the_north_pole(&hanging((-0.5, 0.5)));
    // A cap that stops short of the south pole: the north one is still read the same way.
    one_point_at_the_north_pole(&hanging((-0.25, 0.5)));
}

#[test]
fn a_sphere_flipped_along_its_axis_is_left_undecided_at_its_top_pole() {
    // The axial sign −1 puts v = −1/2 at the top, and the sphere's normal points in: the turned
    // chart, whose normal points out, is not its own, so the contact there is not read as a
    // tangent point. It is left undecided at the pole, never passed as a point.
    let SurfaceKind::Sphere(ball) = hanging((-0.5, 0.5)) else {
        panic!();
    };
    let flipped = SurfaceKind::Sphere(ball.flipped_axially().unwrap());
    let plane = ceiling();
    for (one, two, swapped) in [(&flipped, &plane, false), (&plane, &flipped, true)] {
        let found = subdivide(one, two, TOUCH, &mut meter()).unwrap();
        let read = analyse(one, two, &found, TOUCH, CEILING, &mut meter()).unwrap();
        let [Reading::Undecided { first, second, why }] = read.as_slice() else {
            panic!("{read:?}");
        };
        assert_eq!(*why, Unresolved::HigherOrderContact, "{read:?}");
        let ball = if swapped { second } else { first };
        assert_eq!(ball.1.0, -0.5, "{read:?}");
    }
}

/// A double cone of degree 1 in u: its rows in u are the tip (1/4, 1/8, −1), a triangle at z = 0
/// about the line x = 1/4, y = 1/8, and the tip (1/4, 1/8, 1); v runs round the triangle.
fn spindle() -> SurfaceKind {
    let linear = || Basis::new(1, vec![0.0, 0.0, 1.0, 2.0, 2.0]).unwrap();
    let (x, y) = (0.25, 0.125);
    let ring = [[1.0, 0.0], [-0.5, 0.875], [-0.5, -0.875]];
    let mut net = Vec::new();
    for z in [-1.0, 0.0, 1.0] {
        for [a, b] in ring {
            net.push(if z == 0.0 {
                [x + a, y + b, z]
            } else {
                [x, y, z]
            });
        }
    }
    SurfaceKind::Nurbs(Surface::new(linear(), linear(), &net).unwrap())
}

/// A plane through the spindle's tip at `z` = ±1, shallower than every side of the cone there
/// (z = ±1 + (x − 1/4)/4), meets it at that tip alone: one singular point over the pole line
/// u = `pole`, read from either surface.
fn one_point_at_a_tip(z: f64, pole: f64) {
    let shallow = SurfaceKind::Plane(
        Plane::new(
            [0.25, 0.125, z],
            [1.0, 0.0, 0.25],
            [0.0, 1.0, 0.0],
            ((-4.0, 4.0), (-4.0, 4.0)),
        )
        .unwrap(),
    );
    let tip = spindle();
    for (one, two, swapped) in [(&tip, &shallow, false), (&shallow, &tip, true)] {
        let found = subdivide(one, two, TOUCH, &mut meter()).unwrap();
        assert!(found.settled.is_empty(), "{found:?}");
        let read = analyse(one, two, &found, TOUCH, CEILING, &mut meter()).unwrap();
        let [Reading::Point { at, gap, contact }] = read.as_slice() else {
            panic!("{read:?}");
        };
        assert_eq!(*contact, PointContact::Singular { branches: 0 }, "{read:?}");
        // The plane through the tip: the gap is zero but for the projection's rounding.
        assert!(gap.lo() <= 1e-12 && gap.hi() <= TOUCH, "{gap:?}");
        let (u, v) = if swapped {
            (at[2], at[3])
        } else {
            (at[0], at[1])
        };
        assert_eq!((u.lo(), u.hi()), (pole, pole), "{at:?}");
        assert_eq!((v.lo(), v.hi()), (0.0, 2.0), "{at:?}");
        // The plane's own parameters are the tip's (x, y) less its origin's: (0, 0).
        let (s, r) = if swapped {
            (at[0], at[1])
        } else {
            (at[2], at[3])
        };
        assert!(s.contains(0.0) && r.contains(0.0), "{at:?}");
    }
}

#[test]
fn a_plane_touching_only_a_nurbs_tip_is_one_singular_point_on_its_u_pole() {
    // The lower tip is the pole at the start of u, the upper one at its end.
    one_point_at_a_tip(-1.0, 0.0);
    one_point_at_a_tip(1.0, 2.0);
}
