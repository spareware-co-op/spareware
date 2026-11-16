//! A pole that lies on the other surface's axis (chapter 12.10): a cone's apex at the centre of a
//! cylinder far wider than the cone. The apex is a whole line of the cone's domain, read as one
//! point, and its nearest points on the cylinder are a whole parallel, a continuum, at the
//! cylinder's radius: further than the apex's own spread, so the two are apart there. Nothing is
//! settled or flagged, in either order.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::general::subdivide;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, SurfaceKind};

const FLOOR: f64 = 1.0 / 128.0;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 23))
}

#[test]
fn a_cone_apex_on_a_wide_cylinders_axis_is_apart_from_it() {
    let frame = || Frame::standard().unwrap();
    // The apex at the origin, its radius v, for v up to 1.
    let cone = SurfaceKind::Cone(
        Cone::new([0.0; 3], 0.0, 1.0, frame(), ((0.0, 2.0), (0.0, 1.0))).unwrap(),
    );
    let can = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 4.0, frame(), ((0.0, 2.0), (-2.0, 2.0))).unwrap(),
    );
    for (a, b) in [(&cone, &can), (&can, &cone)] {
        let found = subdivide(a, b, FLOOR, &mut meter()).unwrap();
        assert!(found.settled.is_empty(), "{found:?}");
        assert!(found.flagged.is_empty(), "{found:?}");
    }
}
