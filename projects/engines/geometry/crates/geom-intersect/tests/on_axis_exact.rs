//! A surface's axis handed to the closed forms on the grid must be the stored axis (issue #1063).
//! The closed forms of a line with a cylinder and a torus take the axis as two grid points, the
//! centre `c` and `c + n`, n the frame's stored normal. That second point was summed in doubles and
//! then checked to be whole, but near 2^39 a double holds only 2^-13, so a normal's fraction under
//! half of that rounds away and the sum lands on the grid: the closed form was handed an upright
//! axis for a tilted surface, and answered about that other surface.
//!
//! Every case here is centred at (2^39, 0, 0) with the stored normal n = (2^-15, 0, 1), so
//! `c + n` rounds to (2^39, 0, 1). The frame's x is (1, 0, 0), so x̂ = (1, 0, -2^-15)·a and
//! n̂ = (2^-15, 0, 1)·a, a = 1/√(1 + 2^-30). Both lines lie in y = 0, the plane of the axis and
//! x̂, on the far side of the axis from x̂ (u = 1, off the seam), and run over t in [-64, 64].

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{CurveKind, Frame, Line};
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::result::{Intersection, Item, PointContact};
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, SurfaceKind, Torus};

/// 2^39: where a double holds a coordinate only to 2^-13.
const FAR: f64 = 549_755_813_888.0;

/// 2^20, the cylinder's radius and the torus's major radius.
const R: f64 = 1_048_576.0;

/// 2^-15, the normal's tilt, under half of 2^-13 so `FAR + TILT` rounds to `FAR`.
const TILT: f64 = 1.0 / 32_768.0;

/// R·TILT = 2^5: how far the tilt moves the far side of the axis along it.
const LIFT: f64 = 32.0;

/// 1/√(1 + TILT²): the stored normal's length is 1/a.
fn a() -> f64 {
    1.0 / (1.0 + TILT * TILT).sqrt()
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(16_000_000))
}

fn frame() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [TILT, 0.0, 1.0]).unwrap()
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

/// The line up the upright cylinder's ruling at u = 1, (2^39 − 2^20, 0, t), crosses the tilted
/// cylinder once in t's span: at P − c = (−R, 0, t) the axial part is a·(t − LIFT) and the distance
/// from the axis squared R² + t² − a²·(t − LIFT)², which is R² where t = ±a·(t − LIFT), so at
/// t = LIFT·a/(1 + a), just short of 16 (worked by hand), and at −LIFT·a/(1 − a), about −2^36,
/// far outside. The upright axis held the line along a ruling, an overlap over the whole span.
#[test]
fn a_line_up_a_ruling_of_a_cylinder_whose_axis_rounds_upright_crosses_it_once() {
    let cylinder = SurfaceKind::Cylinder(
        Cylinder::new([FAR, 0.0, 0.0], R, frame(), ((0.0, 2.0), (-64.0, 64.0))).unwrap(),
    );
    let line =
        CurveKind::Line(Line::new([FAR - R, 0.0, 0.0], [0.0, 0.0, 1.0], (-64.0, 64.0)).unwrap());
    let found = points(&curve_surface(&line, &cylinder, &mut meter()).unwrap());
    assert_eq!(found.len(), 1, "{found:?}");
    let (at, contact) = found[0];
    assert_eq!(contact, PointContact::Crossing, "{found:?}");
    assert!(at.contains(LIFT * a() / (1.0 + a())), "{found:?}");
}

/// The line (2^39 − 2^20 + t, 0, 40) crosses the tilted torus twice: the plane y = 0 holds the
/// axis, so it cuts the torus in two circles of radius 16, about c ∓ R·x̂. The far one's centre is
/// (2^39 − R·a, 0, LIFT·a), so the line meets it where (t − R·(1 − a))² + (40 − LIFT·a)² = 16²:
/// t = R·(1 − a) ± √(16² − (40 − LIFT·a)²), about ±13.86 (worked by hand). The near circle's
/// centre is at z = −LIFT·a, 72 below the line. About the upright axis both centres are at z = 0,
/// 40 from the line, so the closed form found no root, and the torus was left unmet.
#[test]
fn a_line_across_a_torus_whose_axis_rounds_upright_crosses_it_twice() {
    let torus = SurfaceKind::Torus(
        Torus::new([FAR, 0.0, 0.0], R, 16.0, frame(), ((0.0, 2.0), (0.0, 2.0))).unwrap(),
    );
    let line =
        CurveKind::Line(Line::new([FAR - R, 0.0, 40.0], [1.0, 0.0, 0.0], (-64.0, 64.0)).unwrap());
    let found = points(&curve_surface(&line, &torus, &mut meter()).unwrap());
    assert_eq!(found.len(), 2, "{found:?}");
    let [(first, p), (second, q)] = [found[0], found[1]];
    assert_eq!([p, q], [PointContact::Crossing; 2], "{found:?}");
    assert!(first.hi() < second.lo(), "{found:?}");
    let shift = R * (1.0 - a());
    let above = 40.0 - LIFT * a();
    let half = (256.0 - above * above).sqrt();
    assert!(first.contains(shift - half), "{found:?}");
    assert!(second.contains(shift + half), "{found:?}");
}
