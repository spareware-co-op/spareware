//! Two tangent points of one pair, one on the seam of a cylinder and one on the cut of the chart
//! turned half a turn (chapter 12.10; `architecture/decisions/G173-contacts-on-a-seam.md`).

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
use geom_surface::{Cylinder, SurfaceKind, Torus};

const UNIT: f64 = 1024.0;
const FLOOR: f64 = 8.0;
const CEILING: f64 = 10.24;

fn cylinder() -> SurfaceKind {
    SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            3.0 * UNIT,
            Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            ((0.0, 2.0), (-4.0 * UNIT, 4.0 * UNIT)),
        )
        .unwrap(),
    )
}

/// The outer half of a torus of 2 and 1 units about the y axis. Its rim, the circle of 3 units in
/// the plane y = 0, touches the cylinder from inside at (3, 0, 0) and (-3, 0, 0).
fn rim() -> SurfaceKind {
    SurfaceKind::Torus(
        Torus::new(
            [0.0; 3],
            2.0 * UNIT,
            UNIT,
            Frame::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0]).unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    )
}

fn readings(a: &SurfaceKind, b: &SurfaceKind) -> Vec<Reading> {
    let found = subdivide(a, b, FLOOR, &mut WorkMeter::new(Budget::of_work(1 << 24))).unwrap();
    analyse(
        a,
        b,
        &found,
        FLOOR,
        CEILING,
        &mut WorkMeter::new(Budget::of_work(1 << 24)),
    )
    .unwrap()
}

/// Whether `x` encloses a point within a hundredth of `at`.
fn near(x: geom_num::interval::Interval, at: f64) -> bool {
    x.lo() <= at + 0.01 && at - 0.01 <= x.hi() && x.hi() - x.lo() < 0.01
}

/// The rim touches the cylinder at (3, 0, 0), on the cylinder's seam (u = 0 = 2), and at
/// (-3, 0, 0), at u = 1, which is the cut of the cylinder's chart turned half a turn. The first is
/// read in the turned chart and the second in the first, where neither is cut: two tangent points,
/// whichever surface is first, neither read twice nor lost.
#[test]
fn a_rim_touching_a_cylinder_on_its_seam_and_on_the_turned_cut_has_two_tangent_points() {
    let (cylinder, rim) = (cylinder(), rim());
    for (found, cylinder_first) in [
        (readings(&cylinder, &rim), true),
        (readings(&rim, &cylinder), false),
    ] {
        assert_eq!(found.len(), 2, "{found:#?}");
        let mut seen = [false; 2];
        for reading in &found {
            let Reading::Point { at, contact, .. } = reading else {
                panic!("{found:#?}");
            };
            assert_eq!(*contact, PointContact::IsolatedTangent, "{at:?}");
            let u = if cylinder_first { at[0] } else { at[2] };
            if near(u, 2.0) {
                seen[0] = true;
            } else if near(u, 1.0) {
                seen[1] = true;
            }
        }
        assert_eq!(seen, [true, true], "{found:#?}");
    }
}
