//! A circle lying in a plane whose domain holds only a part of it (issue 974, decision 32.69).
//! `curve_surface` answers it with one overlap by identity for each arc the domain holds, from
//! exact signs, where before the engine bisected round the whole circle to the resolution floor
//! and flagged it. Each test here pins one branch: a side through the circle, two sides at a
//! corner, an arc through the join of the circle's own parameter, a touch from inside and from
//! outside, a corner on the circle, four arcs, none, a frame turning the other way, a plane on
//! other axes, and the cases the signs cannot place, which stay the engine's. An arc's end on the
//! circle's own join is the domain's own end exactly, with no stretch past the join unless the
//! arc passes it, and a centre on a side or a corner of the domain gives exact quarter-turns.
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

use geom_curve::{Circle, CurveKind, Frame, ParamCurve};
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::quadrics::natural;
use geom_intersect::result::{Coincidence, Item};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, Plane, SurfaceKind};

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
const THREE_FIFTHS: f64 = 0.295_167_235_300_866_5;

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
fn arcs(curve: &CurveKind<3>, surface: &SurfaceKind, want: &[(f64, f64)]) -> Vec<[(f64, f64); 2]> {
    let got = overlaps(curve, surface).unwrap_or_else(|| panic!("not overlaps alone"));
    assert_eq!(got.len(), want.len(), "{got:?}");
    for ([t, _, _], (from, to)) in got.iter().zip(want) {
        assert!(
            t.0 <= *from && *from - t.0 <= CLOSE,
            "start {t:?} of {want:?}"
        );
        assert!(t.1 >= *to && t.1 - *to <= CLOSE, "end {t:?} of {want:?}");
    }
    got.iter().map(|[_, u, v]| [*u, *v]).collect()
}

/// The pair is the engine's: no overlap is claimed, and something is said.
fn engine(curve: &CurveKind<3>, surface: &SurfaceKind) {
    let mut meter = WorkMeter::new(Budget::of_work(50_000_000));
    let found = curve_surface(curve, surface, &mut meter).unwrap();
    assert_ne!(found.items(), [], "nothing said");
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
    arcs(
        &ring(),
        &plane(ORIGIN, X, Y, (-300.0, 17.0), SIDE),
        &[ROUND],
    );
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
        &plane(
            ORIGIN,
            [0.6, 0.8, 0.0],
            [-0.8, 0.6, 0.0],
            (-300.0, 13.6),
            SIDE,
        ),
    );
    // An arc of a circle, not a whole turn.
    let frame = Frame::new(X, Z).unwrap();
    let arc = CurveKind::Circle(Circle::new([12.0, 8.0, 0.0], 0.5, frame, (0.25, 1.75)).unwrap());
    engine(&arc, &plane(ORIGIN, X, Y, (-300.0, 12.0), SIDE));
}

/// The span in t of the pair's one overlap.
fn one(curve: &CurveKind<3>, surface: &SurfaceKind) -> (f64, f64) {
    let got = overlaps(curve, surface).unwrap_or_else(|| panic!("not overlaps alone"));
    let [[t, _, _]] = got[..] else {
        panic!("not one overlap: {got:?}")
    };
    t
}

/// The double `steps` doubles on from the positive `x` (back where `steps` is negative).
fn beside(x: f64, steps: i64) -> f64 {
    f64::from_bits(x.to_bits().wrapping_add_signed(steps))
}

#[test]
fn an_arc_ending_on_the_circles_join_ends_at_the_domains_own_end() {
    // A side through the centre along the frame's x̂ cuts the circle at its join and half a turn
    // on. The half on ŷ's side is t in [0, 1] and the other t in [1, 2]: one overlap each, and its
    // end on the join is the domain's end exactly, on every frame along the plane's axes. No
    // second overlap stands on the far side of the join.
    for x in [X, [-1.0, 0.0, 0.0], Y, [0.0, -2.0, 0.0]] {
        for n in [Z, [0.0, 0.0, -3.0]] {
            let y_hat = [n[1] * x[2] - n[2] * x[1], n[2] * x[0] - n[0] * x[2]];
            for first_half in [true, false] {
                let surface = if x[0] == 0.0 {
                    let right = (y_hat[0] > 0.0) == first_half;
                    let u = if right { (12.0, 300.0) } else { (-300.0, 12.0) };
                    plane(ORIGIN, X, Y, u, SIDE)
                } else {
                    let above = (y_hat[1] > 0.0) == first_half;
                    let v = if above { (8.0, 300.0) } else { (-300.0, 8.0) };
                    plane(ORIGIN, X, Y, SIDE, v)
                };
                let t = one(&circle([12.0, 8.0, 0.0], 5.0, x, n), &surface);
                if first_half {
                    assert_eq!(t.0, 0.0, "{x:?} {n:?}");
                    assert!(t.1 >= 1.0 && t.1 - 1.0 <= CLOSE, "{t:?} {x:?} {n:?}");
                } else {
                    assert!(t.0 <= 1.0 && 1.0 - t.0 <= CLOSE, "{t:?} {x:?} {n:?}");
                    assert_eq!(t.1, 2.0, "{x:?} {n:?}");
                }
            }
        }
    }
}

#[test]
fn a_join_against_the_frames_x_is_placed_the_same_way() {
    // The domain (1, 3) puts the join at (7, 8), against x̂. The lower half is t in [1, 2] and the
    // upper half t in [2, 3].
    let frame = Frame::new(X, Z).unwrap();
    let ring = CurveKind::Circle(Circle::new([12.0, 8.0, 0.0], 5.0, frame, (1.0, 3.0)).unwrap());
    let t = one(&ring, &plane(ORIGIN, X, Y, SIDE, (-300.0, 8.0)));
    assert_eq!(t.0, 1.0);
    assert!(t.1 >= 2.0 && t.1 - 2.0 <= CLOSE, "{t:?}");
    let t = one(&ring, &plane(ORIGIN, X, Y, SIDE, (8.0, 300.0)));
    assert!(t.0 <= 2.0 && 2.0 - t.0 <= CLOSE, "{t:?}");
    assert_eq!(t.1, 3.0);
}

#[test]
fn an_arc_has_a_stretch_past_the_join_only_where_it_passes_it() {
    // One double under 8: the upper half and a little more, starting just before the join and
    // passing it. Two overlaps, each ending on the join exactly.
    let got = overlaps(
        &ring(),
        &plane(ORIGIN, X, Y, SIDE, (beside(8.0, -1), 300.0)),
    )
    .unwrap();
    let [[low, _, _], [high, _, _]] = got[..] else {
        panic!("not two overlaps: {got:?}")
    };
    assert_eq!(low.0, 0.0);
    assert!(low.1 >= 1.0 && low.1 - 1.0 <= CLOSE, "{low:?}");
    assert!(high.0 < 2.0 && 2.0 - high.0 <= CLOSE, "{high:?}");
    assert_eq!(high.1, 2.0);
    // One double over 8: the lower half and a little more, ending just past the join.
    let got = overlaps(
        &ring(),
        &plane(ORIGIN, X, Y, SIDE, (-300.0, beside(8.0, 1))),
    )
    .unwrap();
    let [[low, _, _], [high, _, _]] = got[..] else {
        panic!("not two overlaps: {got:?}")
    };
    assert_eq!(low.0, 0.0);
    assert!(low.1 > 0.0 && low.1 <= CLOSE, "{low:?}");
    assert!(high.0 <= 1.0 && 1.0 - high.0 <= CLOSE, "{high:?}");
    assert_eq!(high.1, 2.0);
    // One double under 8, the lower side: the arc ends just before the join. One overlap.
    let t = one(
        &ring(),
        &plane(ORIGIN, X, Y, SIDE, (-300.0, beside(8.0, -1))),
    );
    assert!(t.0 <= 1.0 && 1.0 - t.0 <= CLOSE, "{t:?}");
    assert_eq!(t.1, 2.0);
    // One double over 8, the upper side: the arc starts just past the join. One overlap.
    let t = one(&ring(), &plane(ORIGIN, X, Y, SIDE, (beside(8.0, 1), 300.0)));
    assert_eq!(t.0, 0.0);
    assert!(t.1 >= 1.0 - CLOSE && t.1 - 1.0 <= CLOSE, "{t:?}");
}

#[test]
fn a_frame_off_the_planes_axes_is_placed_away_from_the_join_and_the_engines_on_it() {
    // x̂ = (3/5, 4/5): the join of the circle of radius 5/8 is at (12.375, 8.5). The side through
    // the centre cuts the circle far from the join: an arc, as on any frame.
    let tilted = circle([12.0, 8.0, 0.0], 0.625, [3.0, 4.0, 0.0], Z);
    arcs(
        &tilted,
        &plane(ORIGIN, X, Y, (-300.0, 12.0), SIDE),
        &[(0.5 - THREE_FIFTHS, 1.5 - THREE_FIFTHS)],
    );
    // The side through the join: no exact sign places the arc's start against it.
    engine(&tilted, &plane(ORIGIN, X, Y, (-300.0, 12.375), SIDE));
    // A join that is off x̂'s line (the domain starts at a half): the same.
    let frame = Frame::new(X, Z).unwrap();
    let ring = CurveKind::Circle(Circle::new([12.0, 8.0, 0.0], 0.5, frame, (0.5, 2.5)).unwrap());
    engine(&ring, &plane(ORIGIN, X, Y, (-300.0, 12.0), SIDE));
}

/// The domains of the curves `natural` finds where the cylinder of radius 5 about the y axis
/// through `(x, 300, z)`, from y = 0 to y = 600, meets the plane y = 0 over 300 each way in z
/// and in x; `None` where the pair is the engine's.
fn end_circle_domains(x: f64, z: f64) -> Option<Vec<(f64, f64)>> {
    let frame = Frame::new(Z, Y).unwrap();
    let tube = Cylinder::new([x, 300.0, z], 5.0, frame, (ROUND, SIDE)).unwrap();
    let flat = plane(ORIGIN, Z, X, SIDE, SIDE);
    let mut meter = WorkMeter::new(Budget::of_work(1 << 22));
    let found = natural(&SurfaceKind::Cylinder(tube), &flat, &mut meter).unwrap()?;
    let mut out = Vec::new();
    for item in found.items() {
        let Item::Curve { curve, .. } = item else {
            panic!("not a curve: {item:?}")
        };
        out.push(curve.domain());
    }
    Some(out)
}

#[test]
fn a_centre_on_a_side_of_the_domain_gives_the_half_circle_between_exact_quarter_turns() {
    // The end circle's centre (-300, 0, 0) is on the side x = -300, and the arc the plane holds
    // starts on the circle's own join. One overlap, from 0 exactly.
    let end = circle([-300.0, 0.0, 0.0], 5.0, Z, Y);
    let t = one(&end, &plane(ORIGIN, Z, X, SIDE, SIDE));
    assert_eq!(t.0, 0.0);
    assert!(t.1 >= 1.0 && t.1 - 1.0 <= CLOSE, "{t:?}");
    // The curve of the two surfaces is the cylinder's end circle, an edge of its domain: the box
    // of the middle of each stretch the plane's edge cuts reaches that edge, so the pair is the
    // engine's (issue 1039, decision 32.94). Before, the half circle (1.5, 2.5) was kept on it.
    assert_eq!(end_circle_domains(-300.0, 0.0), None);
}

#[test]
fn a_centre_on_a_corner_of_the_domain_gives_the_quarter_circle_between_exact_quarter_turns() {
    // The centre (-300, 0, -300) is the corner: the quarter from the join to a quarter turn on.
    let end = circle([-300.0, 0.0, -300.0], 5.0, Z, Y);
    let t = one(&end, &plane(ORIGIN, Z, X, SIDE, SIDE));
    assert_eq!(t.0, 0.0);
    assert!(t.1 >= 0.5 && t.1 - 0.5 <= CLOSE, "{t:?}");
    // The engine's, as on a side (issue 1039); before, the quarter (1.5, 2.0) was kept.
    assert_eq!(end_circle_domains(-300.0, -300.0), None);
}

#[test]
fn a_number_whose_products_would_not_be_exact_is_the_engines() {
    // The plane's centre is 1e-80 off the origin along x, either way: smaller than the numbers
    // whose products are exact. A small circle, so the engine's run round it is short.
    let small = circle([12.0, 8.0, 0.0], 0.5, X, Z);
    for tiny in [1e-80, -1e-80] {
        engine(&small, &plane([tiny, 0.0, 0.0], X, Y, (-300.0, 12.0), SIDE));
    }
}

#[test]
fn a_plane_on_each_pair_of_axes_either_way_round_is_placed() {
    // u is y and v is x, in the plane z = 0: v at most 12 is the half round the far side.
    arcs(
        &ring(),
        &plane(ORIGIN, Y, X, SIDE, (-300.0, 12.0)),
        &[(0.5, 1.5)],
    );
    // u is x and v is z, in the plane y = 0: the circle about (12, 0, 8), from x.
    let level = circle([12.0, 0.0, 8.0], 5.0, X, Y);
    arcs(
        &level,
        &plane(ORIGIN, X, Z, (-300.0, 12.0), SIDE),
        &[(0.5, 1.5)],
    );
}

/// The pair's answer is overlaps alone, with these spans in t, lowest first, each end within
/// [`CLOSE`].
fn spans_are(curve: &CurveKind<3>, surface: &SurfaceKind, want: &[(f64, f64)], case: &str) {
    let mut got: Vec<(f64, f64)> = overlaps(curve, surface)
        .unwrap_or_else(|| panic!("not overlaps alone: {case}"))
        .iter()
        .map(|[t, _, _]| *t)
        .collect();
    got.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(got.len(), want.len(), "{got:?}: {case}");
    for (t, w) in got.iter().zip(want) {
        assert!(
            (t.0 - w.0).abs() <= CLOSE && (t.1 - w.1).abs() <= CLOSE,
            "{got:?} for {want:?}: {case}"
        );
    }
}

/// A side one double off the line through the centre along the frame's x̂ (`step` doubles on from
/// it), keeping what is over it or under it, against the circle of that frame whose domain
/// starts at `lo`. The side cuts the circle beside its join and half a turn on. Where it keeps
/// the line, the arc is a half-turn and a sliver more at each end, through the join at one of
/// them: two overlaps. Where it does not, the arc is a sliver short at each end: one overlap.
fn beside_the_join(lo: f64, x: [f64; 3], n: [f64; 3], over: bool, step: i64) {
    let frame = Frame::new(x, n).unwrap();
    let round = (lo, lo + 2.0);
    let ring = CurveKind::Circle(Circle::new([12.0, 8.0, 0.0], 5.0, frame, round).unwrap());
    // ŷ = n̂ × x̂, in x and y.
    let y_hat = [-n[2] * x[1], n[2] * x[0]];
    let (surface, towards) = if x[0] == 0.0 {
        let at = beside(12.0, step);
        let u = if over { (at, 300.0) } else { (-300.0, at) };
        (plane(ORIGIN, X, Y, u, SIDE), y_hat[0])
    } else {
        let at = beside(8.0, step);
        let v = if over { (at, 300.0) } else { (-300.0, at) };
        (plane(ORIGIN, X, Y, SIDE, v), y_hat[1])
    };
    // The half on ŷ's side is the first half-turn from x̂: the first half of a domain that
    // starts at 0, the second of one that starts at 1.
    let first_half = ((towards > 0.0) == over) == (lo == 0.0);
    let keeps_the_line = over == (step < 0);
    let want = match (first_half, keeps_the_line) {
        (true, true) => [(lo, lo + 1.0), (lo + 2.0, lo + 2.0)].to_vec(),
        (true, false) => [(lo, lo + 1.0)].to_vec(),
        (false, true) => [(lo, lo), (lo + 1.0, lo + 2.0)].to_vec(),
        (false, false) => [(lo + 1.0, lo + 2.0)].to_vec(),
    };
    let case = format!("from {lo}, x {x:?}, n {n:?}, over {over}, step {step}");
    spans_are(&ring, &surface, &want, &case);
}

#[test]
fn a_side_a_double_off_the_join_is_placed_on_every_frame_along_the_planes_axes() {
    // Every frame along the plane's axes, turning either way, with the join along x̂ (a domain
    // from 0) and against it (from 1); each side of the line, a double either way off it.
    for lo in [0.0, 1.0] {
        for x in [X, [-1.0, 0.0, 0.0], Y, [0.0, -1.0, 0.0]] {
            for n in [Z, [0.0, 0.0, -1.0]] {
                for over in [true, false] {
                    for step in [-1, 1] {
                        beside_the_join(lo, x, n, over, step);
                    }
                }
            }
        }
    }
}

#[test]
fn an_end_whose_enclosure_reaches_a_join_that_no_sign_places_is_the_engines() {
    // A small circle, so the engine's run round it is short. The side x = 12 through its centre
    // cuts it at a quarter turn and at three quarters.
    let frame = Frame::new(X, Z).unwrap();
    let on = |domain: (f64, f64)| {
        CurveKind::Circle(Circle::new([12.0, 8.0, 0.0], 0.5, frame, domain).unwrap())
    };
    let (far, near) = (
        plane(ORIGIN, X, Y, (-300.0, 12.0), SIDE),
        plane(ORIGIN, X, Y, (12.0, 300.0), SIDE),
    );
    // With the join well clear, the far half is one overlap that ends at the upper end of the
    // enclosure of three quarters, and the near half one that starts at its lower end.
    let upper = one(&on((0.25, 2.25)), &far).1;
    let lower = one(&on((0.75, 2.75)), &near).0;
    assert!(
        lower < 1.5 && 1.5 < upper && upper - lower <= CLOSE,
        "{lower} {upper}"
    );
    // A whole turn that ends on either end of that enclosure has its join off x̂'s line, where no
    // exact sign places the far half's end against it, and the enclosure reaches the join: it
    // does not say which side of the join the end is on.
    engine(&on((upper - 2.0, upper)), &far);
    engine(&on((lower - 2.0, lower)), &far);
}
