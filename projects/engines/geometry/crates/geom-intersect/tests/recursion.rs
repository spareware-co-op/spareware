//! The recursion bound of `geom-intersect`'s module headers, run at its bound (chapter 28.8; the
//! recursion check of piece G13): the charts of an offset nested `MAX_NESTING` deep, the deepest
//! `Offset::new` allows, read down to the base's crease.

#![allow(clippy::unwrap_used, reason = "tests: a failure stops the test")]

use geom_intersect::seam::charts;
use geom_nurbs::{Basis, Surface};
use geom_surface::offset::MAX_NESTING;
use geom_surface::{Offset, SurfaceKind};

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
