//! A tangent point that lies on a seam is one point, decided in the chart that does not cut it
//! (chapter 12.10; `architecture/decisions/G173-contacts-on-a-seam.md`). Two unit spheres touching
//! from outside at (1, 0, 0), which is on the first sphere's seam (u = 0 and u = 2), flagged from
//! both sides by subdivision: each side alone is a band of about a hundred steps, undecided. Run at
//! the scale of the SSI corpus' `c3_spheres_external` (1024 steps to the unit, floor 8, ceiling
//! 10.24).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::contact::{Reading, analyse};
use geom_intersect::general::subdivide;
use geom_intersect::result::PointContact;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Sphere, SurfaceKind};

const UNIT: f64 = 1024.0;
const FLOOR: f64 = 8.0;
const CEILING: f64 = 10.24;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

/// The unit sphere centred at x = `x`, its seam on the side facing the other sphere.
fn sphere(x: f64) -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(
            [x * UNIT, 0.0, 0.0],
            UNIT,
            Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    )
}

fn readings(a: &SurfaceKind, b: &SurfaceKind) -> Vec<Reading> {
    let found = subdivide(a, b, FLOOR, &mut meter()).unwrap();
    analyse(a, b, &found, FLOOR, CEILING, &mut meter()).unwrap()
}

/// Whether `x` encloses a point within a hundredth of `at`.
fn near(x: geom_num::interval::Interval, at: f64) -> bool {
    x.lo() <= at + 0.01 && at - 0.01 <= x.hi() && x.hi() - x.lo() < 0.01
}

#[test]
fn two_spheres_touching_on_a_seam_have_one_tangent_point() {
    let (a, b) = (sphere(0.0), sphere(2.0));
    // The point (1, 0, 0) is at u = 0 = 2 of the sphere at the origin (its seam) and at u = 1 of the
    // other, and on both equators. Whichever sphere is first, the box holds the seam's parameter
    // at its end, 2 (or, a hair past it, as the box of the chart cut elsewhere), never moved back to
    // the start: the part past the end is the same points.
    for (found, seam_first) in [(readings(&a, &b), true), (readings(&b, &a), false)] {
        let [Reading::Point { at, gap, contact }] = found.as_slice() else {
            panic!("{found:#?}");
        };
        assert_eq!(*contact, PointContact::IsolatedTangent);
        assert!(gap.lo() <= 0.0 && 0.0 <= gap.hi(), "{gap:?}");
        let [u1, v1, u2, v2] = *at;
        let (on_seam, other) = if seam_first { (u1, u2) } else { (u2, u1) };
        assert!(near(on_seam, 2.0), "{at:?}");
        assert!(near(other, 1.0), "{at:?}");
        assert!(near(v1, 0.0) && near(v2, 0.0), "{at:?}");
    }
}
