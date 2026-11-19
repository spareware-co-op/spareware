//! A line handed to the closed forms on the grid must be the stored line (issue #1058). The closed
//! forms of a line with a circle, a sphere and a cylinder take the line as two grid points, its
//! origin `o` and `o + d`. That second point was summed in doubles and then checked to be whole,
//! but near 2^39 a double holds only 2^-13, so a direction's fraction under half of that rounds
//! away and the sum lands on the grid: the closed form was handed a line of another direction from
//! the stored one, and answered about that line.
//!
//! Every case here is the line from (0, 2^39) along (1, 2^-20), against a round shape of radius
//! 2^20 centred at (0, 2^39 + 2^20): the circle, or the sphere or the cylinder about z through it,
//! with the line in z = 0. The line passes through the shape's lowest point, (0, 2^39), on a slope
//! of 2^-20, so it crosses twice: at t = 0, and at t = 2 / (1 + 2^-40), just short of 2 (worked
//! by hand: put o + t·d into |p − c|² = r², and the t² and t terms are (1 + 2^-40)·t² and
//! −2·2^-20·2^20·t, the constant 2^40 − 2^40). The line rounded onto the grid is y = 2^39, which
//! only touches the shape there, so the closed form said one tangent point, at the middle of the
//! true pair, a point that is on neither.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_intersect::curve::curves;
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::result::{Intersection, Item, PointContact};
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, Sphere, SurfaceKind};

/// 2^39: where a double holds a coordinate only to 2^-13.
const FAR: f64 = 549_755_813_888.0;

/// 2^20, the radius.
const R: f64 = 1_048_576.0;

/// 2^-20, the direction's fraction, under half of 2^-13 so `FAR + TILT` rounds to `FAR`.
const TILT: f64 = 1.0 / R;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(16_000_000))
}

/// Each point's enclosure of its first parameter and its contact, sorted by the enclosure's start.
fn points(r: &Intersection) -> Vec<(Interval, PointContact)> {
    let mut out: Vec<(Interval, PointContact)> = r
        .items()
        .iter()
        .map(|i| match i {
            Item::Point { at, contact, .. } => (at.spans()[0], *contact),
            other => panic!("not a point: {other:?}"),
        })
        .collect();
    out.sort_by(|a, b| a.0.lo().total_cmp(&b.0.lo()));
    out
}

/// Two crossings, apart, enclosing t = 0 and t = 2 / (1 + 2^-40), as the stored line meets the
/// shape. The enclosures are up to about 2^-10 wide (the discriminant 2^-80 is enclosed against
/// terms near 1), so the second root's double, within 2^-51 of it, stands well inside its own.
fn crosses_twice(r: &Intersection) {
    let found = points(r);
    assert_eq!(found.len(), 2, "{found:?}");
    let [(first, a), (second, b)] = [found[0], found[1]];
    assert_eq!([a, b], [PointContact::Crossing; 2], "{found:?}");
    assert!(first.hi() < second.lo(), "{found:?}");
    assert!(first.contains(0.0), "{found:?}");
    assert!(second.contains(2.0 / (1.0 + TILT * TILT)), "{found:?}");
}

fn line_2d() -> CurveKind<2> {
    CurveKind::Line(Line::infinite([0.0, FAR], [1.0, TILT]).unwrap())
}

fn line_3d() -> CurveKind<3> {
    CurveKind::Line(Line::infinite([0.0, FAR, 0.0], [1.0, TILT, 0.0]).unwrap())
}

#[test]
fn a_line_whose_direction_rounds_onto_the_grid_crosses_a_circle_twice() {
    let circle =
        CurveKind::Circle(Circle::full([0.0, FAR + R], R, Frame::standard().unwrap()).unwrap());
    crosses_twice(&curves(&line_2d(), &circle, &mut meter()).unwrap());
    crosses_twice(
        &curves(&circle, &line_2d(), &mut meter())
            .unwrap()
            .swapped(1),
    );
}

#[test]
fn a_line_whose_direction_rounds_onto_the_grid_crosses_a_sphere_twice() {
    let sphere = SurfaceKind::Sphere(
        Sphere::new(
            [0.0, FAR + R, 0.0],
            R,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    );
    crosses_twice(&curve_surface(&line_3d(), &sphere, &mut meter()).unwrap());
}

#[test]
fn a_line_whose_direction_rounds_onto_the_grid_crosses_a_cylinder_twice() {
    let cylinder = SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0, FAR + R, 0.0],
            R,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-1.0, 1.0)),
        )
        .unwrap(),
    );
    crosses_twice(&curve_surface(&line_3d(), &cylinder, &mut meter()).unwrap());
}
