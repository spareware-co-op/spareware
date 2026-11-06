//! A tangent point at the edge of a surface's parameters is never an overlap. A sphere whose pole
//! touches a torus' outer equator (the corner of the torus' parameter domain) is one tangent point:
//! at a pole the surface has no second fundamental form to compare, and an unreadable pair says
//! nothing of how the surfaces bend. Run at the scale of the SSI corpus' `c3_sphere_torus_outer`
//! (1024 steps to the unit, floor 8, ceiling 10.24).

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
use geom_run::{Budget, WorkMeter};
use geom_surface::{Sphere, SurfaceKind, Torus};

const UNIT: f64 = 1024.0;
const FLOOR: f64 = 8.0;
const CEILING: f64 = 10.24;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

/// The torus of major radius 3 and minor radius 1 about the z axis, over its whole domain: its
/// outer equator at x = 4 is the corner of the domain.
fn torus() -> SurfaceKind {
    SurfaceKind::Torus(
        Torus::new(
            [0.0, 0.0, 0.0],
            3.0 * UNIT,
            UNIT,
            Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            ((0.0, 2.0), (0.0, 2.0)),
        )
        .unwrap(),
    )
}

/// The unit sphere centred at x = `x` whose axis, and so whose south pole, is along x.
fn sphere(x: f64) -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(
            [x * UNIT, 0.0, 0.0],
            UNIT,
            Frame::new([0.0, 1.0, 0.0], [1.0, 0.0, 0.0]).unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    )
}

/// What `analyse` says of the torus and the sphere centred at `x`, both orders.
fn readings(x: f64) -> [Vec<Reading>; 2] {
    let (a, b) = (torus(), sphere(x));
    let one = {
        let found = subdivide(&a, &b, FLOOR, &mut meter()).unwrap();
        analyse(&a, &b, &found, FLOOR, CEILING, &mut meter()).unwrap()
    };
    let other = {
        let found = subdivide(&b, &a, FLOOR, &mut meter()).unwrap();
        analyse(&b, &a, &found, FLOOR, CEILING, &mut meter()).unwrap()
    };
    [one, other]
}

/// A claim that the surfaces coincide over patches, or touch along a curve.
fn claims_a_region(r: &Reading) -> bool {
    matches!(r, Reading::Overlap { .. } | Reading::Curve { .. })
}

#[test]
fn a_sphere_pole_touching_a_torus_corner_is_not_an_overlap() {
    for found in readings(5.0) {
        assert!(!found.iter().any(claims_a_region), "{found:#?}");
    }
}

#[test]
fn a_sphere_pole_a_little_apart_from_the_torus_corner_is_not_an_overlap() {
    // 0.05 of a unit (51 steps) clear of the torus: no contact to read at all.
    for found in readings(5.05) {
        assert!(!found.iter().any(claims_a_region), "{found:#?}");
    }
}

#[test]
fn a_sphere_pole_a_little_into_the_torus_corner_is_not_an_overlap() {
    // 0.05 of a unit (51 steps) across the torus: a small circle, not a region of coincidence.
    for found in readings(4.95) {
        assert!(!found.iter().any(claims_a_region), "{found:#?}");
    }
}
