//! A line parallel to a cylinder's axis, or to a cone's ruling (decision 32.107, issue 1112). Its
//! quadratic leads with A = 0 exactly, which no enclosure is, so before, `quadratic_with` refused it
//! and the engine ran along the line: 0.83 to 1.05 million units a call of the fuzz targets, for
//! flagged items or the budget's stop. Whether the line is parallel is now an exact sign in whole
//! numbers of the stored doubles, and then the quadratic is 2B·t + C: a cylinder's B is zero, and
//! C, the same at every point, is nothing or the line along a ruling; a cone's B apart from zero is
//! one crossing at t = −C/2B, and B zero is nothing or the line along the ruling, by C's enclosure
//! or its exact sign. A line a hair off parallel is still the engine's.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{CurveKind, Frame, Line};
use geom_intersect::IntersectError;
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::result::{Coincidence, Intersection, Item, PointContact};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, SurfaceKind};

/// Far past what a closed form spends, and far short of the engine's 0.4 million units and more on
/// these pairs: a pair the closed form leaves stops here instead of running on.
const BUDGET: u64 = 100_000;

/// The frame on an axis none of whose coordinates is zero, (1, 2, 2), three long.
const SLANT: ([f64; 3], [f64; 3]) = ([1.0, 0.0, 0.0], [1.0, 2.0, 2.0]);

/// A point off the grid's coarse steps, as the apex of the cones on [`SLANT`].
const APEX: [f64; 3] = [0.5, 0.25, 0.125];

/// Square to [`SLANT`]'s axis, three long: the turn of a quarter (u = 0.25) from the frame's x.
const ACROSS: [f64; 3] = [2.0, 1.0, -2.0];

/// The axis and [`ACROSS`] added, over three: a ruling of the cone of slope 1 on [`SLANT`].
const RULING: [f64; 3] = [1.0, 1.0, 0.0];

/// Square to both: (−2, 2, −1), the direction round the cone along that ruling, so a line along it
/// stays in the cone's tangent plane there.
const ROUND: [f64; 3] = [-2.0, 2.0, -1.0];

/// A frame by its direction in the plane and its axis.
fn frame((x, n): ([f64; 3], [f64; 3])) -> Frame<3> {
    Frame::new(x, n).unwrap()
}

/// The cylinder about `c` of radius `r`, a whole turn and 300 either way.
fn cylinder(c: [f64; 3], r: f64, f: Frame<3>) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new(c, r, f, ((0.0, 2.0), (-300.0, 300.0))).unwrap())
}

/// The cone about `c` of radius `r0` at v = 0 and slope `k`, a whole turn and 300 either way.
fn cone(c: [f64; 3], r0: f64, k: f64, f: Frame<3>) -> SurfaceKind {
    SurfaceKind::Cone(Cone::new(c, r0, k, f, ((0.0, 2.0), (-300.0, 300.0))).unwrap())
}

/// The line `o + t·d`, t in [−300, 300], as a plane's edge is.
fn line(o: [f64; 3], d: [f64; 3]) -> CurveKind<3> {
    CurveKind::Line(Line::new(o, d, (-300.0, 300.0)).unwrap())
}

/// The segment from `a` to `b`, t in [0, 1].
fn segment(a: [f64; 3], b: [f64; 3]) -> CurveKind<3> {
    CurveKind::Line(Line::segment(a, b).unwrap().value)
}

/// `a + k·b`.
fn plus(a: [f64; 3], k: f64, b: [f64; 3]) -> [f64; 3] {
    [a[0] + k * b[0], a[1] + k * b[1], a[2] + k * b[2]]
}

/// The answer and the work it took.
fn meet(c: &CurveKind<3>, s: &SurfaceKind) -> (Intersection, u128) {
    let mut meter = WorkMeter::new(Budget::of_work(BUDGET));
    let found = curve_surface(c, s, &mut meter).unwrap();
    (found, meter.used().work)
}

/// Whether `(lo, hi)` holds `x` and is within 10⁻⁹ of it at either end.
fn near((lo, hi): (f64, f64), x: f64) -> bool {
    lo <= x && x <= hi && hi - lo < 1e-9
}

/// The one overlap of `found`, by identity over all of the segment (t in [0, 1]) at the turn `u`,
/// from `v` to `w` (the lower first).
fn along_all_of_it(found: &Intersection, u: f64, (v, w): (f64, f64)) {
    match found.items() {
        [
            Item::Overlap {
                at,
                proof: Coincidence::Identical,
                ..
            },
        ] => {
            let [t, turn, height, ..] = at.spans() else {
                panic!("{at:?}");
            };
            assert_eq!((t.lo(), t.hi()), (0.0, 1.0), "{at:?}");
            assert!(near((turn.lo(), turn.hi()), u), "{at:?} against u = {u}");
            let (lo, hi) = (height.lo(), height.hi());
            assert!(lo <= v && v - lo < 1e-9, "{at:?} against v from {v}");
            assert!(w <= hi && hi - w < 1e-9, "{at:?} against v to {w}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_line_parallel_to_a_cylinder_s_axis_is_nothing_or_its_ruling() {
    // The issue's natural input (`ee00dfdcdc3d06`): the plane's side y = −300 along the axis x of
    // the cylinder through (0, −132, −144) of radius 221, which it misses by 221.27 − 221; C is 119.
    // Before, the engine ran along it: 908,956 units a call of the target, and the issue measured
    // this side at 99.5% of its instructions.
    let (found, work) = meet(
        &line([0.0, -300.0, 0.0], [1.0, 0.0, 0.0]),
        &cylinder(
            [0.0, -132.0, -144.0],
            221.0,
            frame(([0.0, 1.0, 0.0], [1.0, 0.0, 0.0])),
        ),
    );
    assert!(found.items().is_empty(), "{:?}", found.items());
    assert_eq!(work, 0);
    // On the slant axis off the grid, a segment along the axis 192 from it, a quarter turn round:
    // on the cylinder of radius 192 it lies on the ruling; a step of the radius either way and C's
    // enclosure still holds zero, and its exact sign says it is nothing.
    let start = plus(APEX, 64.0, ACROSS);
    let side = segment(start, plus(start, 1.0, SLANT.1));
    let (found, work) = meet(&side, &cylinder(APEX, 192.0, frame(SLANT)));
    along_all_of_it(&found, 0.25, (0.0, 3.0));
    assert_eq!(work, 1);
    for r in [192f64.next_up(), 192f64.next_down()] {
        let (found, work) = meet(&side, &cylinder(APEX, r, frame(SLANT)));
        assert!(found.items().is_empty(), "{r}: {:?}", found.items());
        assert_eq!(work, 0);
    }
}

#[test]
fn a_segment_along_a_cone_s_ruling_is_one_overlap() {
    // The issue's five piercing inputs, each a segment along a ruling of a cone about the z axis:
    // `f60001ff000201ff00ff030f00180026` on the cone of start radius 16 and slope 0.5;
    // `00ffff00fffffffffeffff03f8000000`, `00ffff20fffffffffeffff03f8000000` and
    // `00ffff00fffffe03fefffd03f80043` on cones of slope zero, whose rulings run along the axis;
    // `000000010001ff0f00010001` from (0, 0, 4) to the apex at (0, 4, −4). Before, 826,784 to
    // 1,048,577 units a call of the target (flagged items, or the budget's stop).
    let issue: [(CurveKind<3>, SurfaceKind, f64, (f64, f64)); 5] = [
        (
            segment([0.0, 4.0, -4.0], [0.0, 8.0, 4.0]),
            cone([0.0, -4.0, 12.0], 16.0, 0.5, Frame::standard().unwrap()),
            0.5,
            (-16.0, -8.0),
        ),
        (
            segment([-4.0, -4.0, 0.0], [-4.0, -4.0, -4.0]),
            cone([-8.0, -4.0, -4.0], 4.0, 0.0, Frame::standard().unwrap()),
            0.0,
            (0.0, 4.0),
        ),
        (
            segment([-4.0, -4.0, 128.0], [-4.0, -4.0, -4.0]),
            cone([-8.0, -4.0, -4.0], 4.0, 0.0, Frame::standard().unwrap()),
            0.0,
            (0.0, 132.0),
        ),
        (
            segment([-4.0, -4.0, 0.0], [-4.0, -4.0, -8.0]),
            cone([-8.0, -4.0, -12.0], 4.0, 0.0, Frame::standard().unwrap()),
            0.0,
            (4.0, 12.0),
        ),
        (
            segment([0.0, 0.0, 4.0], [0.0, 4.0, -4.0]),
            cone([0.0, 4.0, 0.0], 2.0, 0.5, Frame::standard().unwrap()),
            1.5,
            (-4.0, 4.0),
        ),
    ];
    // On the slant axis, the cone of slope 1 with its apex at APEX, and the ruling a quarter turn
    // round: a segment from 4 to 8 along it, from the apex up and down (where the origin's ρ is zero,
    // so its turn is read from the direction), on the lower nappe (ρ below zero), and the same cone
    // built of start radius 3 about the point 3 up its axis, so v there is 3 less.
    let apex = cone(APEX, 0.0, 1.0, frame(SLANT));
    let started = cone(plus(APEX, 1.0, SLANT.1), 3.0, 1.0, frame(SLANT));
    let on = |a: f64, b: f64| segment(plus(APEX, a, RULING), plus(APEX, b, RULING));
    let slant: [(CurveKind<3>, SurfaceKind, f64, (f64, f64)); 5] = [
        (on(4.0, 8.0), apex.clone(), 0.25, (4.0, 8.0)),
        (on(0.0, 4.0), apex.clone(), 0.25, (0.0, 4.0)),
        (on(0.0, -4.0), apex.clone(), 0.25, (-4.0, 0.0)),
        (on(-4.0, -8.0), apex, 0.25, (-8.0, -4.0)),
        (on(4.0, 8.0), started, 0.25, (1.0, 5.0)),
    ];
    for (edge, surface, u, v) in issue.into_iter().chain(slant) {
        let (found, work) = meet(&edge, &surface);
        along_all_of_it(&found, u, v);
        assert_eq!(work, 1);
    }
}

#[test]
fn a_line_parallel_to_a_ruling_off_the_cone_is_nothing_or_one_crossing() {
    // In the cone's tangent plane along the ruling, 3·2⁻³⁰ beside it or a quarter: B is zero, and
    // C is about 7.8e-18, inside its enclosure and decided by its exact sign, or 0.5625, by the
    // enclosure. The same 3·2⁻³⁰ beside the cone of start radius 3 that is the same cone.
    let apex = cone(APEX, 0.0, 1.0, frame(SLANT));
    let started = cone(plus(APEX, 1.0, SLANT.1), 3.0, 1.0, frame(SLANT));
    let beside = |k: f64| {
        let start = plus(plus(APEX, 4.0, RULING), k, ROUND);
        segment(start, plus(start, 4.0, RULING))
    };
    let hair = 1.0 / 1_073_741_824.0;
    for (edge, surface) in [
        (beside(hair), apex.clone()),
        (beside(0.25), apex.clone()),
        (beside(hair), started),
    ] {
        let (found, work) = meet(&edge, &surface);
        assert!(found.items().is_empty(), "{:?}", found.items());
        assert_eq!(work, 0);
    }
    // Parallel to the ruling, three across the axis from it: it crosses the cone once, on the
    // lower nappe, at t = −1.5, half a turn from the ruling (u = 1.25), v = −1.5.
    let (found, work) = meet(&line(plus(APEX, 1.0, ACROSS), RULING), &apex);
    match found.items() {
        [
            Item::Point {
                at,
                contact: PointContact::Crossing,
                ..
            },
        ] => {
            let [t, u, v, ..] = at.spans() else {
                panic!("{at:?}");
            };
            assert!(near((t.lo(), t.hi()), -1.5), "{at:?}");
            assert!(near((u.lo(), u.hi()), 1.25), "{at:?}");
            assert!(near((v.lo(), v.hi()), -1.5), "{at:?}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(work, 4);
}

#[test]
fn a_line_a_hair_off_parallel_is_still_the_engines() {
    // Through a point of the ruling, along it turned 2⁻⁵⁴ round the surface, so it lies in the
    // tangent plane and touches there: A is about 10⁻³¹ and its enclosure holds zero, but the line
    // is not parallel, so neither the cylinder's nor the cone's closed form of 32.107 takes it, and
    // it is no overlap. The engine flags it or stops on the budget, as before.
    let tilt = f64::EPSILON / 4.0;
    let pairs = [
        (
            line(
                plus(APEX, 64.0, ACROSS),
                plus([0.25, 0.5, 0.5], tilt, ROUND),
            ),
            cylinder(APEX, 192.0, frame(SLANT)),
        ),
        (
            line(
                plus(APEX, 4.0, RULING),
                plus([0.25, 0.25, 0.0], tilt, ROUND),
            ),
            cone(APEX, 0.0, 1.0, frame(SLANT)),
        ),
    ];
    for (edge, surface) in pairs {
        let mut meter = WorkMeter::new(Budget::of_work(1_000));
        match curve_surface(&edge, &surface, &mut meter) {
            Err(IntersectError::BudgetExceeded) => {}
            Ok(found) => assert!(
                found
                    .items()
                    .iter()
                    .all(|i| matches!(i, Item::Flagged { .. })),
                "{:?}",
                found.items()
            ),
            Err(other) => panic!("{other:?}"),
        }
    }
}
