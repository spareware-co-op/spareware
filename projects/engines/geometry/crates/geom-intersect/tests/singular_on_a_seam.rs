//! A singular point that sits on a seam is one point, read once. Two cylinders of one radius with
//! perpendicular axes that meet (Steinmetz's solid, the SSI corpus' `c4_steinmetz`) cross in two
//! ellipses, which meet at two singular points of four half-branches each. One of them lies on the
//! second cylinder's seam (u = 0 and u = 2 are one line), so subdivision flagged it from both
//! sides, and each side's cluster saw only two of the four half-branches. Run at the corpus' scale
//! (8192 steps to the unit, floor 8, ceiling 8.192).

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
use geom_surface::{Cylinder, SurfaceKind};

const UNIT: f64 = 8192.0;
const FLOOR: f64 = 8.0;
const CEILING: f64 = 8.192;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

/// The cylinder about the z axis, from z = -2 to 2, of radius 1.
fn upright() -> SurfaceKind {
    SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0, 0.0, -2.0 * UNIT],
            UNIT,
            Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            ((0.0, 2.0), (0.0, 4.0 * UNIT)),
        )
        .unwrap(),
    )
}

/// The cylinder about the x axis, from x = -2 to 2, of radius 1: its seam is the line at y = 1,
/// where the first cylinder is met at (0, 1, 0).
fn sideways() -> SurfaceKind {
    SurfaceKind::Cylinder(
        Cylinder::new(
            [-2.0 * UNIT, 0.0, 0.0],
            UNIT,
            Frame::new([0.0, 1.0, 0.0], [1.0, 0.0, 0.0]).unwrap(),
            ((0.0, 2.0), (0.0, 4.0 * UNIT)),
        )
        .unwrap(),
    )
}

/// The branch counts of the singular points `analyse` reads for the two cylinders, in this order.
fn singular_points(a: &SurfaceKind, b: &SurfaceKind) -> Vec<u16> {
    let found = subdivide(a, b, FLOOR, &mut meter()).unwrap();
    analyse(a, b, &found, FLOOR, CEILING, &mut meter())
        .unwrap()
        .into_iter()
        .filter_map(|r| match r {
            Reading::Point {
                contact: PointContact::Singular { branches },
                ..
            } => Some(branches),
            _ => None,
        })
        .collect()
}

#[test]
fn two_cylinders_meet_at_two_singular_points_of_four_half_branches() {
    let (a, b) = (upright(), sideways());
    assert_eq!(singular_points(&a, &b), [4, 4]);
    assert_eq!(singular_points(&b, &a), [4, 4]);
}
