//! A circle lying in a plane whose domain's lower side stands one to four steps above the circle's
//! lowest point (decision 32.80, issue 1002). The side crosses the circle at two points about
//! 3e-8 apart, but the enclosure of each crossing reaches the lowest point, so the closed form of
//! 32.69 cannot tell them apart: the arc between them would end in two stretches that meet, which
//! `Intersection::new` refuses as `Internal`. The pair is the engine's, and its answer is the one it
//! gave before 32.69: one flagged region, of higher-order contact.

#![allow(
    clippy::unwrap_used,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{Circle, CurveKind, Frame};
use geom_intersect::curve_surface::curve_surface;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};

const X: [f64; 3] = [1.0, 0.0, 0.0];
const Y: [f64; 3] = [0.0, 1.0, 0.0];
const Z: [f64; 3] = [0.0, 0.0, 1.0];

/// The engine's work on each of the four pairs, as before 32.69.
const ENGINE: u128 = 5088;

/// What the closed form spends before it finds the stretches meet and leaves the pair.
const TRIED: u128 = 9;

/// What the second order test spends more since decision 32.104: the run is the whole turn, and
/// grown past the window's end it is read modulo the turn in two stretches, so the circle's first
/// and second derivatives are each bounded once more, at a unit a coordinate.
const ACROSS: u128 = 6;

#[test]
fn a_side_a_step_above_the_lowest_point_is_the_engines() {
    let mut c = -1.0_f64;
    for _ in 0..4 {
        c = c.next_up();
        let circle = CurveKind::Circle(
            Circle::new([0.0; 3], 1.0, Frame::new(X, Z).unwrap(), (0.0, 2.0)).unwrap(),
        );
        let plane =
            SurfaceKind::Plane(Plane::new([0.0; 3], X, Y, ((-5.0, 5.0), (c, 5.0))).unwrap());
        let mut meter = WorkMeter::new(Budget::of_work(50_000_000));
        let found = curve_surface(&circle, &plane, &mut meter);
        let want = format!(
            "[Flagged {{ at: ParamBox {{ spans: [Interval {{ lo: 0.0, hi: 2.0 }}, \
             Interval {{ lo: -1.25, hi: 1.25 }}, Interval {{ lo: {c:?}, hi: 1.25 }}, \
             Interval {{ lo: 0.0, hi: 0.0 }}], unknowns: 3 }}, xyz: Box3 {{ min: P3 {{ x: Coord(-1), \
             y: Coord(-2), z: Coord(0) }}, max: P3 {{ x: Coord(1), y: Coord(2), z: Coord(0) }} }}, \
             why: HigherOrderContact }}]"
        );
        let found = found.unwrap();
        assert_eq!(format!("{:?}", found.items()), want, "side at {c:e}");
        assert_eq!(meter.used().work, ENGINE + TRIED + ACROSS, "side at {c:e}");
    }
}
