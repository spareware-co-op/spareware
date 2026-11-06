//! The recursion bound of `geom-intersect`'s module header, run at its bound (chapter 28.8; the
//! recursion check of piece G13): a tilted plane cuts an extrusion whose profile is a trimmed
//! circle, so the map's `image` of the trimmed curve asks for the image of its basis, the deepest
//! the cycle goes, and the result is the ellipse the map keeps.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "tests: a failure stops the test"
)]

use geom_curve::{Basis, Circle, CurveKind, Frame, Trimmed};
use geom_intersect::quadrics::natural;
use geom_intersect::result::Item;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Extrusion, Plane, SurfaceKind};

const WIDE: (f64, f64) = (-1000.0, 1000.0);

#[test]
fn the_image_of_a_trimmed_curve_runs_at_its_bound() {
    let up = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let arc = Circle::new([0.0; 3], 5.0, up, (0.0, 0.5)).unwrap();
    let profile = Trimmed::new(Basis::Circle(arc), (0.0, 0.5)).unwrap();
    let wall =
        SurfaceKind::Extrusion(Extrusion::new(profile, [0.0, 0.0, 1.0], (0.0, 10.0)).unwrap());
    let tilted = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, 5.0],
            [1.0, 0.0, 0.25],
            [0.0, 1.0, 0.0],
            (WIDE, WIDE),
        )
        .unwrap(),
    );
    let meter = &mut WorkMeter::new(Budget::UNLIMITED);
    let meeting = natural(&tilted, &wall, meter).unwrap().unwrap();
    let curves: Vec<&CurveKind<3>> = meeting
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Curve { curve, .. } => Some(&**curve),
            _ => None,
        })
        .collect();
    assert_eq!(curves.len(), 1);
    assert!(
        matches!(curves[0], CurveKind::Ellipse(_)),
        "{:?}",
        curves[0]
    );
}
