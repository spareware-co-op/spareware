//! Attacks on piece G169's claims: loops found however small, start points on every branch against
//! known curves, a closed loop in a lens the chapter's Gauss-map test would miss, tangent contacts
//! flagged, and pairs apart culled.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::IntersectError;
use geom_intersect::general::{Subdivided, subdivide};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, ParamSurface, Plane, Sphere, SurfaceKind};

const FLOOR: f64 = 1e-3;
const TURN: f64 = 2.0;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

fn frame() -> Frame<3> {
    Frame::standard().unwrap()
}

fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// A biquadratic patch of z = h ± c(x² + y²) over [−r, r]², exact in Bézier form.
fn bowl(r: f64, h: f64, c: f64) -> SurfaceKind {
    let basis = || Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for (i, x) in [-r, 0.0, r].into_iter().enumerate() {
        for (j, y) in [-r, 0.0, r].into_iter().enumerate() {
            // The middle control points of a parabola stand at −(end value) from the ends' mean.
            let w = |k: usize, t: f64| if k == 1 { -t * t } else { t * t };
            net.push([x, y, h + c * (w(i, r) + w(j, r))]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap())
}

/// Every start point lies on both surfaces, and pairs are even.
fn all_on_both(first: &SurfaceKind, second: &SurfaceKind, found: &Subdivided) {
    let sides = (Limit::FromRight, Limit::FromRight);
    for pair in &found.settled {
        assert_eq!(pair.starts.len() % 2, 0, "{pair:?}");
        for [a, b, c, d] in &pair.starts {
            let p = first.point(mid(*a), mid(*b), sides);
            let q = second.point(mid(*c), mid(*d), sides);
            if let (Ok(p), Ok(q)) = (p, q) {
                let gap = (p[0] - q[0]).hypot(p[1] - q[1]).hypot(p[2] - q[2]);
                assert!(gap < 4.0 * FLOOR, "{gap}: {pair:?}");
            }
        }
    }
}

#[test]
fn a_sphere_through_a_plane_patch_has_start_points_on_its_circle() {
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-9.0, 9.0), (-9.0, 9.0)),
        )
        .unwrap(),
    );
    let sphere = SurfaceKind::Sphere(
        Sphere::new([1.0, 2.0, 3.0], 5.0, frame(), ((0.1, 1.9), (-0.45, 0.45))).unwrap(),
    );
    let found = subdivide(&plane, &sphere, FLOOR, &mut meter()).unwrap();
    assert!(!found.settled.is_empty(), "{found:?}");
    assert!(found.flagged.is_empty(), "{:?}", found.flagged);
    all_on_both(&plane, &sphere, &found);
    for pair in &found.settled {
        for [u, v, ..] in &pair.starts {
            let r = (mid(*u) - 1.0).hypot(mid(*v) - 2.0);
            assert!((r - 4.0).abs() < 4.0 * FLOOR, "{r}");
        }
    }
}

#[test]
fn two_crossing_cylinders_have_start_points_on_both() {
    let a = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], 2.0, frame(), ((0.1, 1.9), (-6.0, 6.0))).unwrap(),
    );
    let b = SurfaceKind::Cylinder(
        Cylinder::new(
            [0.5, 0.0, 0.0],
            1.0,
            Frame::new([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]).unwrap(),
            ((0.1, 1.9), (-6.0, 6.0)),
        )
        .unwrap(),
    );
    let found = subdivide(&a, &b, FLOOR, &mut meter()).unwrap();
    assert!(!found.settled.is_empty(), "{found:?}");
    all_on_both(&a, &b, &found);
}

/// The lens of z = x² + y² − 1 and z = 1 − x² − y²: a closed circle of radius 1. Its outward normals
/// are separated by a plane, which chapter 12.8 took to prove it had no loop: the search must not.
#[test]
fn a_lens_is_found_though_a_plane_separates_its_normals() {
    let low = bowl(2.0, -1.0, 1.0);
    let high = bowl(2.0, 1.0, -1.0);
    let found = subdivide(&low, &high, FLOOR, &mut meter()).unwrap();
    assert!(!found.settled.is_empty(), "{found:?}");
    all_on_both(&low, &high, &found);
    let mut most: f64 = 0.0;
    for pair in &found.settled {
        for [u, v, ..] in &pair.starts {
            let (x, y) = (4.0 * mid(*u) - 2.0, 4.0 * mid(*v) - 2.0);
            let r = x.hypot(y);
            assert!((r - 1.0).abs() < 4.0 * FLOOR, "{r}");
            most = most.max(r);
        }
    }
    assert!(most > 0.9, "{most}");
}

/// A loop far smaller than any sampling would see: radius 0.01 on a patch 4 wide.
#[test]
fn a_tiny_loop_is_found() {
    let low = bowl(2.0, -1e-4, 1.0);
    let high = bowl(2.0, 1e-4, -1.0);
    let found = subdivide(&low, &high, 1e-5, &mut meter()).unwrap();
    assert!(!found.settled.is_empty(), "{found:?}");
    for pair in &found.settled {
        for [u, v, ..] in &pair.starts {
            let (x, y) = (4.0 * mid(*u) - 2.0, 4.0 * mid(*v) - 2.0);
            assert!((x.hypot(y) - 0.01).abs() < 4e-5, "{x} {y}");
        }
    }
}

#[test]
fn patches_apart_have_nothing() {
    let low = bowl(2.0, -1.0, 0.1);
    let high = bowl(2.0, 1.0, 0.1);
    let found = subdivide(&low, &high, FLOOR, &mut meter()).unwrap();
    assert!(found.settled.is_empty(), "{found:?}");
    assert!(found.flagged.is_empty(), "{:?}", found.flagged);
}

/// Touching at one point: nothing can decide it, and it is flagged, not lost.
#[test]
fn a_tangent_contact_is_flagged() {
    let low = bowl(2.0, 0.0, 1.0);
    let high = bowl(2.0, 0.0, -1.0);
    let found = subdivide(&low, &high, 1e-2, &mut meter()).unwrap();
    assert!(!found.flagged.is_empty(), "{found:?}");
}

#[test]
fn a_floor_that_is_not_positive_is_refused() {
    let low = bowl(2.0, -1.0, 1.0);
    for floor in [0.0, -1.0, f64::NAN] {
        assert_eq!(
            subdivide(&low, &low, floor, &mut meter()),
            Err(IntersectError::InvalidInput)
        );
    }
}

#[test]
fn a_budget_stops_the_search() {
    let low = bowl(2.0, -1.0, 1.0);
    let high = bowl(2.0, 1.0, -1.0);
    let tight = subdivide(&low, &high, FLOOR, &mut WorkMeter::new(Budget::of_work(10)));
    assert_eq!(tight, Err(IntersectError::BudgetExceeded));
}

#[test]
fn the_search_is_deterministic() {
    let low = bowl(2.0, -1.0, 1.0);
    let high = bowl(2.0, 1.0, -1.0);
    let once = subdivide(&low, &high, FLOOR, &mut meter()).unwrap();
    let twice = subdivide(&low, &high, FLOOR, &mut meter()).unwrap();
    assert_eq!(once, twice);
    let domain = ((0.0, TURN), (-0.5, 0.5));
    assert_eq!(domain.0.1, TURN);
}
