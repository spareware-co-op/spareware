//! A circle lying in a plane whose domain holds only a part of it (issue 974, decision 32.69).
//! `curve_surface` answers it with one overlap by identity for each arc the domain holds, from
//! exact signs, where before the engine bisected round the whole circle to the resolution floor
//! and flagged it. Each test here pins one branch: a side through the circle, two sides at a
//! corner, an arc through the join of the circle's own parameter, a touch from inside and from
//! outside, a corner on the circle, four arcs, none, a frame turning the other way, a plane on
//! other axes, and the cases the signs cannot place, which stay the engine's.
//!
//! Which arcs there are is exact; where each ends is an enclosure, held here to [`CLOSE`] of the
//! true end.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{Circle, CurveKind, Frame};
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::result::{Coincidence, Item};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};

const ROUND: (f64, f64) = (0.0, 2.0);
const SIDE: (f64, f64) = (-300.0, 300.0);
const X: [f64; 3] = [1.0, 0.0, 0.0];
const Y: [f64; 3] = [0.0, 1.0, 0.0];
const Z: [f64; 3] = [0.0, 0.0, 1.0];
const ORIGIN: [f64; 3] = [0.0; 3];

/// The most work the closed form takes here: a box of the circle for each arc, and the signs.
const FEW: u128 = 400;

/// How far an arc's enclosure may reach past its true end, in half-turns.
const CLOSE: f64 = 1e-12;

/// The angle whose cosine is 3/5, in half-turns: where a line 3 from the centre of a circle of
/// radius 5 crosses it.
const THREE_FIFTHS: f64 = 0.295_167_235_300_866_53;

fn circle(center: [f64; 3], radius: f64, x: [f64; 3], n: [f64; 3]) -> CurveKind<3> {
    CurveKind::Circle(Circle::new(center, radius, Frame::new(x, n).unwrap(), ROUND).unwrap())
}

fn plane(c: [f64; 3], a: [f64; 3], b: [f64; 3], u: (f64, f64), v: (f64, f64)) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(c, a, b, (u, v)).unwrap())
}

/// The circle of radius 5 about (12, 8, 0) in the plane z = 0, from x towards y.
fn ring() -> CurveKind<3> {
    circle([12.0, 8.0, 0.0], 5.0, X, Z)
}

/// The spans in t, u and v of the pair's items, every one an overlap by identity, for a few
/// units; `None` if any item is anything else.
fn overlaps(curve: &CurveKind<3>, surface: &SurfaceKind) -> Option<Vec<[(f64, f64); 3]>> {
    let mut meter = WorkMeter::new(Budget::of_work(50_000_000));
    let found = curve_surface(curve, surface, &mut meter).unwrap();
    let mut out = Vec::new();
    for item in found.items() {
        let Item::Overlap {
            at,
            proof: Coincidence::Identical,
            ..
        } = item
        else {
            return None;
        };
        let spans = at.spans();
        out.push([0, 1, 2].map(|k| (spans[k].lo(), spans[k].hi())));
    }
    let work = meter.used().work;
    assert!(work <= FEW, "{work} units");
    Some(out)
}

/// The pair's answer is these arcs of the circle, in order: each enclosed, and no wider than
/// [`CLOSE`] past either end. The spans in u and v of each are returned.
fn arcs(
    curve: &CurveKind<3>,
    surface: &SurfaceKind,
    want: &[(f64, f64)],
) -> Vec<[(f64, f64); 2]> {
    let got = overlaps(curve, surface).unwrap_or_else(|| panic!("not overlaps alone"));
    assert_eq!(got.len(), want.len(), "{got:?}");
    for ([t, _, _], (from, to)) in got.iter().zip(want) {
        assert!(t.0 <= *from && *from - t.0 <= CLOSE, "start {t:?} of {want:?}");
        assert!(t.1 >= *to && t.1 - *to <= CLOSE, "end {t:?} of {want:?}");
    }
    got.iter().map(|[_, u, v]| [*u, *v]).collect()
}

/// The pair is the engine's: no overlap is claimed, and something is said.
fn engine(curve: &CurveKind<3>, surface: &SurfaceKind) {
    let mut meter = WorkMeter::new(Budget::of_work(50_000_000));
    let found = curve_surface(curve, surface, &mut meter).unwrap();
    assert!(!found.items().is_empty());
    assert!(
        found
            .items()
            .iter()
            .all(|i| !matches!(i, Item::Overlap { .. })),
        "an overlap: {:?}",
        found.items()
    );
}

#[test]
fn a_side_through_the_centre_holds_half_the_circle() {
    // x at most 12: from the top of the circle round its far side to the bottom.
    let held = arcs(
        &ring(),
        &plane(ORIGIN, X, Y, (-300.0, 12.0), SIDE),
        &[(0.5, 1.5)],
    );
    // The arc is at u in [7, 12], cut to the domain, and v in [3, 13].
    let [[u, v]] = held[..] else { panic!() };
    assert!(u.0 <= 7.0 && u.1 == 12.0, "{u:?}");
    assert!(v.0 <= 3.0 && v.1 >= 13.0, "{v:?}");
}

#[test]
fn an_arc_through_the_join_of_the_circles_parameter_is_two_overlaps() {
    // x at least 12: the half through the circle's start, written each side of the join.
    arcs(
        &ring(),
        &plane(ORIGIN, X, Y, (12.0, 300.0), SIDE),
        &[(0.0, 0.5), (1.5, 2.0)],
    );
}

#[test]
fn two_sides_at_a_corner_hold_the_arc_between_them() {
    // The corner (12, 8) is the centre: x at most 12 and y at most 8 is a quarter.
    arcs(
        &ring(),
        &plane(ORIGIN, X, Y, (-300.0, 12.0), (-300.0, 8.0)),
        &[(1.0, 1.5)],
    );
}

#[test]
fn a_touch_from_inside_cuts_nothing() {
    // x at most 17 touches the circle at (17, 8): all of it is held.
    arcs(&ring(), &plane(ORIGIN, X, Y, (-300.0, 17.0), SIDE), &[ROUND]);
    // y at least 3 touches it at (12, 3), inside the arc that x at most 15 leaves.
    arcs(
        &ring(),
        &plane(ORIGIN, X, Y, (-300.0, 15.0), (3.0, 300.0)),
        &[(THREE_FIFTHS, 2.0 - THREE_FIFTHS)],
    );
}

#[test]
fn a_touch_from_outside_is_the_engines() {
    // x at least 17 holds the one point (17, 8).
    engine(&ring(), &plane(ORIGIN, X, Y, (17.0, 300.0), SIDE));
}

#[test]
fn an_arc_may_end_at_a_corner_of_the_domain() {
    // The corner (15, 12) is on the circle, 3 and 4 from its centre. With x at most 15 and y at
    // least 12 the circle comes in at the corner and leaves through the side y = 12 at (9, 12).
    arcs(
        &ring(),
        &plane(ORIGIN, X, Y, (-300.0, 15.0), (12.0, 300.0)),
        &[(THREE_FIFTHS, 1.0 - THREE_FIFTHS)],
    );
}

#[test]
fn a_corner_touched_from_outside_is_the_engines() {
    // With x at most 15 and y at most 12 the circle is outside the domain on both hands of the
    // corner (15, 12): a lone point, beside the arc from (9, 12) round to (15, 4).
    engine(
        &ring(),
        &plane(ORIGIN, X, Y, (-300.0, 15.0), (-300.0, 12.0)),
    );
}

#[test]
fn a_circle_through_all_four_sides_is_four_arcs() {
    // Radius 5 about the origin against the square of half-side 4: its corners are outside the
    // circle and its sides inside, so the circle is held between (4, 3) and (3, 4) and so on.
    let about = circle(ORIGIN, 5.0, X, Z);
    let low = 0.5 - THREE_FIFTHS;
    arcs(
        &about,
        &plane(ORIGIN, X, Y, (-4.0, 4.0), (-4.0, 4.0)),
        &[
            (low, THREE_FIFTHS),
            (1.0 - THREE_FIFTHS, 1.0 - low),
            (1.0 + low, 1.0 + THREE_FIFTHS),
            (2.0 - THREE_FIFTHS, 2.0 - low),
        ],
    );
}

#[test]
fn a_circle_round_the_whole_domain_misses_it() {
    // The square of half-side 3 is inside the circle of radius 5: its corners are 4.25 out.
    let about = circle(ORIGIN, 5.0, X, Z);
    let held = overlaps(&about, &plane(ORIGIN, X, Y, (-3.0, 3.0), (-3.0, 3.0)));
    assert_eq!(held, Some(Vec::new()));
}

#[test]
fn a_frame_turning_the_other_way_runs_the_arc_backwards() {
    // The normal is -z, so the circle runs from x towards -y: the quarter x at most 12, y at most
    // 8 is its second quarter, not its third.
    let back = circle([12.0, 8.0, 0.0], 5.0, X, [0.0, 0.0, -1.0]);
    arcs(
        &back,
        &plane(ORIGIN, X, Y, (-300.0, 12.0), (-300.0, 8.0)),
        &[(0.5, 1.0)],
    );
}

#[test]
fn a_plane_on_other_axes_with_vectors_of_any_length_is_placed() {
    // The circle is in the plane x = 0, about (0, 12, 8), from y towards z.
    let side = circle([0.0, 12.0, 8.0], 5.0, Y, X);
    // u is y/2: u at most 6 is y at most 12.
    let held = arcs(
        &side,
        &plane(ORIGIN, [0.0, 2.0, 0.0], Z, (-150.0, 6.0), SIDE),
        &[(0.5, 1.5)],
    );
    let [[u, _]] = held[..] else { panic!() };
    assert!(u.0 <= 3.5 && u.1 == 6.0, "{u:?}");
    // u is -y/2: u at least -6 is y at most 12.
    arcs(
        &side,
        &plane(ORIGIN, [0.0, -2.0, 0.0], Z, (-6.0, 150.0), SIDE),
        &[(0.5, 1.5)],
    );
    // The plane's vectors the other way round: u is z, v is y.
    arcs(
        &side,
        &plane(ORIGIN, Z, Y, SIDE, (-300.0, 12.0)),
        &[(0.5, 1.5)],
    );
}

#[test]
fn what_the_signs_cannot_place_is_the_engines() {
    // Small circles, so the engine's run round each is short.
    let small = circle([12.0, 8.0, 0.0], 0.5, X, Z);
    // The plane's vectors off the coordinate axes: u = 16 is the line through the centre.
    engine(
        &small,
        &plane(ORIGIN, [0.6, 0.8, 0.0], [-0.8, 0.6, 0.0], (-300.0, 13.6), SIDE),
    );
    // An arc of a circle, not a whole turn.
    let frame = Frame::new(X, Z).unwrap();
    let arc = CurveKind::Circle(Circle::new([12.0, 8.0, 0.0], 0.5, frame, (0.25, 1.75)).unwrap());
    engine(&arc, &plane(ORIGIN, X, Y, (-300.0, 12.0), SIDE));
}
