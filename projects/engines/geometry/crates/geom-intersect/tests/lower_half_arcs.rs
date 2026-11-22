//! Arcs of a circle lying in a plane whose domain cuts it (decision 32.69), ending at points of
//! the lower half and of the upper half. The closed form places each end by its angle: a point of
//! the upper half as `turn_of` writes it, one of the lower half as two half-turns less the angle of
//! its mirror in the first axis (decision 32.80). These pin both halves: lower-half ends with x
//! surely negative, x above zero and x on the line through the centre, and upper-half ends on
//! each side. An end not mirrored, or left without its two half-turns, moves its arc.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{Circle, CurveKind, Frame};
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::result::{Coincidence, Item};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};

const X: [f64; 3] = [1.0, 0.0, 0.0];
const Y: [f64; 3] = [0.0, 1.0, 0.0];
const Z: [f64; 3] = [0.0, 0.0, 1.0];

/// The arcs of the unit circle about the origin in z = 0 that the plane z = 0 holds over the
/// domain `u` along x and `v` along y, as their spans in the circle's turn, in half-turns.
fn arcs(u: (f64, f64), v: (f64, f64)) -> Vec<(f64, f64)> {
    let circle = CurveKind::Circle(
        Circle::new([0.0; 3], 1.0, Frame::new(X, Z).unwrap(), (0.0, 2.0)).unwrap(),
    );
    let plane = SurfaceKind::Plane(Plane::new([0.0; 3], X, Y, (u, v)).unwrap());
    let mut meter = WorkMeter::new(Budget::of_work(50_000_000));
    let found = curve_surface(&circle, &plane, &mut meter).unwrap();
    let mut out = Vec::new();
    for item in found.items() {
        let Item::Overlap {
            at,
            proof: Coincidence::Identical,
            ..
        } = item
        else {
            panic!("not an arc on the plane: {item:?}");
        };
        let t = at.spans()[0];
        out.push((t.lo(), t.hi()));
    }
    out
}

/// `found` is one arc per span of `want`, each end within a step's reach of it.
fn spans(found: &[(f64, f64)], want: &[(f64, f64)]) {
    assert_eq!(found.len(), want.len(), "{found:?}");
    for (f, w) in found.iter().zip(want) {
        assert!(
            (f.0 - w.0).abs() < 1e-12 && (f.1 - w.1).abs() < 1e-12,
            "{found:?} against {want:?}"
        );
    }
}

/// The turn of (x, y), in half-turns, on [0, 2), in doubles.
fn turn(x: f64, y: f64) -> f64 {
    let t = geom_num::math::atan2(y, x).unwrap().get() / core::f64::consts::PI;
    if t < 0.0 { t + 2.0 } else { t }
}

/// x at most 0: the left half, from (0, 1) round to (0, −1), both ends on the line x = 0 through
/// the centre.
#[test]
fn an_arc_to_a_lower_half_end_on_the_line_through_the_centre_ends_in_place() {
    spans(&arcs((-5.0, 0.0), (-5.0, 5.0)), &[(0.5, 1.5)]);
}

/// x at least 0: the right half, from (0, −1) round to (0, 1), cut at the turn's start. Its first
/// end is of the lower half, on the line x = 0 through the centre.
#[test]
fn an_arc_from_a_lower_half_end_on_the_line_through_the_centre_starts_in_place() {
    spans(&arcs((0.0, 5.0), (-5.0, 5.0)), &[(0.0, 0.5), (1.5, 2.0)]);
}

/// y at least −0.6: from (0.8, −0.6), x above zero, round through the top to (−0.8, −0.6), x surely
/// negative. Both ends are of the lower half; the arc is cut at the turn's start.
#[test]
fn lower_half_ends_each_side_of_the_axis_are_placed() {
    let (right, left) = (turn(0.8, -0.6), turn(-0.8, -0.6));
    let found = arcs((-5.0, 5.0), (-0.6, 5.0));
    spans(&found, &[(0.0, left), (right, 2.0)]);
}

/// y at most 0.6: from (−0.8, 0.6), x surely negative, round through the bottom to (0.8, 0.6), x
/// above zero. Both ends are of the upper half; the arc is cut at the turn's start.
#[test]
fn upper_half_ends_each_side_of_the_axis_are_placed() {
    let (left, right) = (turn(-0.8, 0.6), turn(0.8, 0.6));
    let found = arcs((-5.0, 5.0), (-5.0, 0.6));
    spans(&found, &[(0.0, right), (left, 2.0)]);
}
