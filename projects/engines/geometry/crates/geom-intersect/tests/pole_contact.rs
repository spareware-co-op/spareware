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
use geom_intersect::result::PointContact;
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
        assert_eq!((v.lo(), v.hi()), (0.5, 0.5), "{at:?}");
    }
}

#[test]
fn a_sphere_hanging_from_a_plane_at_its_north_pole_touches_it_at_one_point() {
    one_point_at_the_north_pole(&hanging((-0.5, 0.5)));
    // A cap that stops short of the south pole: the north one is still read the same way.
    one_point_at_the_north_pole(&hanging((-0.25, 0.5)));
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

#[test]
fn a_plane_touching_only_a_nurbs_tip_is_one_singular_point_on_its_u_pole() {
    // z = −1 + (x − 1/4)/4 through the lower tip: shallower than every side of the cone there.
    let shallow = SurfaceKind::Plane(
        Plane::new(
            [0.25, 0.125, -1.0],
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
        assert_eq!((u.lo(), u.hi()), (0.0, 0.0), "{at:?}");
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
