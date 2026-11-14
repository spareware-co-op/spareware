//! The recursion bounds of `geom-intersect`'s module headers, run at their bounds (chapter 28.8;
//! the recursion check of piece G13).
//!
//! A tilted plane cuts an extrusion whose profile is a trimmed circle, so the map's `image` of the
//! trimmed curve asks for the image of its basis, the deepest the cycle goes, and the result is the
//! ellipse the map keeps (G167). The charts of an offset nested `MAX_NESTING` deep, the deepest
//! `Offset::new` allows, are read down to the base's crease (G173).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "tests: a failure stops the test"
)]

use geom_curve::{Basis as CurveBasis, Circle, CurveKind, Frame, Trimmed};
use geom_intersect::quadrics::natural;
use geom_intersect::result::Item;
use geom_intersect::seam::charts;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::offset::MAX_NESTING;
use geom_surface::{Extrusion, Offset, Plane, SurfaceKind};

/// A flat bicubic patch with a triple knot at u = 1/2, which the charts read as a crease, offset
/// `MAX_NESTING` times.
fn deepest() -> SurfaceKind {
    let bu = Basis::new(
        3,
        vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let bv = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut pts = Vec::new();
    for i in 0..7_u8 {
        for j in 0..4_u8 {
            pts.push([f64::from(i), f64::from(j), 0.0]);
        }
    }
    let mut k = SurfaceKind::Nurbs(Surface::new(bu, bv, &pts).unwrap());
    for _ in 0..MAX_NESTING {
        k = SurfaceKind::Offset(Offset::new(k, 0.05).unwrap());
    }
    k
}

#[test]
fn charts_at_their_bound() {
    let found = charts(&deepest());
    assert_eq!(found.creases, (vec![0.5], Vec::new()));
}

const WIDE: (f64, f64) = (-1000.0, 1000.0);

#[test]
fn the_image_of_a_trimmed_curve_runs_at_its_bound() {
    let up = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let arc = Circle::new([0.0; 3], 5.0, up, (0.0, 0.5)).unwrap();
    let profile = Trimmed::new(CurveBasis::Circle(arc), (0.0, 0.5)).unwrap();
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
