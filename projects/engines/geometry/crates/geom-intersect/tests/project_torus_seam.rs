//! A point whose nearest point on a torus lies on one of its seams (chapter 12.10): the outer
//! equator is the seam of v (v = 0 and v = 2 are one circle), and the meridian at u = 0 the seam of
//! u. The nearest point is one point, read from either end of the closing parameter, never the
//! whole turn between them, and never a continuum.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::{Projection, project_surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{SurfaceKind, Torus};

const FLOOR: f64 = 1e-6;
const TURN: f64 = 2.0;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 22))
}

/// The torus of radii 6 and 2 about the z axis, both parameters a whole turn.
fn ring() -> SurfaceKind {
    SurfaceKind::Torus(
        Torus::new(
            [0.0; 3],
            6.0,
            2.0,
            Frame::standard().unwrap(),
            ((0.0, TURN), (0.0, TURN)),
        )
        .unwrap(),
    )
}

/// The nearest point of `p` on the ring is `near`, at `d`, every reading of it on a seam of
/// parameter `k` at one end or the other.
fn one_foot(p: [f64; 3], near: [f64; 3], d: f64, k: usize) {
    let found = project_surface(&ring(), &p, FLOOR, &mut meter()).unwrap();
    let Projection::Points(points) = &found else {
        panic!("{p:?}: {found:?}");
    };
    assert!(!points.is_empty(), "{found:?}");
    for n in points {
        assert!(n.distance.contains(d), "{n:?}");
        for (x, want) in n.point.iter().zip(near) {
            assert!(x.lo() - FLOOR <= want && want <= x.hi() + FLOOR, "{n:?}");
        }
        let t = n.at[k];
        assert!(t.hi() <= FLOOR || t.lo() >= TURN - FLOOR, "{n:?}");
        assert!(t.hi() - t.lo() <= FLOOR, "{n:?}");
    }
}

#[test]
fn a_point_beside_the_outer_equator_has_one_foot_on_the_v_seam() {
    // Off the u seam, at u = 1/2: the point (0, 9, 0) is 1 from (0, 8, 0).
    one_foot([0.0, 9.0, 0.0], [0.0, 8.0, 0.0], 1.0, 1);
}

#[test]
fn a_point_beside_the_first_meridian_has_one_foot_on_the_u_seam() {
    // Off the v seam, at the top of the tube: (6, 0, 3) is 1 from (6, 0, 2).
    one_foot([6.0, 0.0, 3.0], [6.0, 0.0, 2.0], 1.0, 0);
}
