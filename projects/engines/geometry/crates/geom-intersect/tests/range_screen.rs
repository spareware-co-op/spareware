//! natural's range check screens a kind of fixed terms by its speed's ceiling before it takes the
//! extents of its first partials (chapter 12.1's range, 12.7's closed forms). Where twice the
//! ceiling holds a parameter's doubles within two steps the extents are never taken; where it does
//! not, they decide, exactly as without the screen. The work the check charges shows which: an
//! operand's own box, and its extents' work only when the screen could not decide.
//!
//! Each case is the plane z = 0 written as `x = speed·(u − start)`, its box x from 0 to `speed`
//! wherever its u starts. Met by a ball in the middle of it, it gives the circle or is refused;
//! met by a plane outside the frame, the call stops at that plane's check, so its work is the two
//! checks' alone.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::IntersectError;
use geom_intersect::quadrics::natural;
use geom_nurbs::Direction;
use geom_run::{Budget, WorkMeter};
use geom_surface::{ParamSurface, Plane, Sphere, SurfaceKind};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The plane z = 0 at x = `speed`·(u − `start`), u over one unit from `start`, v along y.
fn written(start: f64, speed: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [-(start * speed), 0.0, 0.0],
            [speed, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((start, start + 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    )
}

/// A ball in the middle of the plane's box, cut by it in a circle.
fn ball(speed: f64) -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(
            [speed / 2.0, 0.5, 0.0],
            0.25,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    )
}

/// A plane 2^41 steps out, past the frame's 2^40, its parameters near zero.
fn outside() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [2_199_023_255_552.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    )
}

/// How many of the circle's items natural gives for the plane and its ball, the same in both orders.
fn met(start: f64, speed: f64) -> Result<usize, IntersectError> {
    let (p, b) = (written(start, speed), ball(speed));
    let found = natural(&p, &b, &mut meter()).map(|r| r.map_or(0, |r| r.items().len()));
    let turned = natural(&b, &p, &mut meter()).map(|r| r.map_or(0, |r| r.items().len()));
    assert_eq!(found, turned, "from {start} at {speed}");
    found
}

/// The work of the range check on the plane and then on `outside`, which refuses the call.
fn checked(start: f64, speed: f64) -> u128 {
    let mut m = meter();
    let r = natural(&written(start, speed), &outside(), &mut m).map(|r| r.is_some());
    assert_eq!(
        r,
        Err(IntersectError::FrameRange),
        "from {start} at {speed}"
    );
    m.used().work
}

/// The work of a surface's box over its domain, and of the extents of its first partial along u.
fn parts(s: &SurfaceKind) -> (u128, u128) {
    let mut boxed = meter();
    s.bound(s.domain(), &mut boxed).unwrap();
    let mut along = meter();
    s.derivative_bound(s.domain(), Direction::U, &mut along)
        .unwrap();
    (boxed.used().work, along.used().work)
}

#[test]
fn where_twice_the_ceiling_holds_a_step_no_extent_is_taken() {
    let (far, _) = parts(&outside());
    // 2^52 out a double's spacing is 1, so a speed of 1 puts twice the ceiling at two steps
    // exactly; 2^41 out the spacing is 2^-11, and twice the ceiling of a speed below 2048 is under
    // two steps; near zero both are far under. The check charges each operand's box and nothing
    // more.
    let below = 2048.0 - 1.0 / 2_199_023_255_552.0;
    for (start, speed) in [
        (0.0, 1.0),
        (4_503_599_627_370_496.0, 1.0),
        (0.0, below),
        (2_199_023_255_552.0, below),
    ] {
        assert_eq!(met(start, speed), Ok(1), "from {start} at {speed}");
        let (boxed, _) = parts(&written(start, speed));
        assert_eq!(
            checked(start, speed),
            boxed + far,
            "from {start} at {speed}"
        );
    }
}

#[test]
fn where_it_does_not_the_extents_decide() {
    let (far, _) = parts(&outside());
    let below = 2048.0 - 1.0 / 2_199_023_255_552.0;
    // 2^42 out the spacing is 2^-10: twice the ceiling is four steps, so the extents are taken
    // along u, and their bound, two steps exactly, fits.
    let start = 4_398_046_511_104.0;
    assert_eq!(met(start, below), Ok(1));
    let (boxed, along) = parts(&written(start, below));
    assert!(along > 0);
    assert_eq!(checked(start, below), boxed + along + far);
    // 2^43 out their bound is four steps, and the operand is refused.
    assert_eq!(
        met(8_796_093_022_208.0, below),
        Err(IntersectError::FrameRange)
    );
}
