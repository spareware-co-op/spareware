//! The arithmetic of the turned chart: which parameters close, where their cuts are, how a box is
//! moved a turn, how two boxes at the two ends of a closing parameter are joined, and how a reading
//! of the turned chart is put back (chapter 12.10).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::contact::{
    Reading, TURN, closing, cut_of, cuts_of, cuts_with_readings, in_a_seam, moved, spans_of,
    unturned, wrapped,
};
use geom_intersect::general::{FlaggedPair, Settled};
use geom_intersect::result::{CurveContact, PointContact, Unresolved};
use geom_num::interval::Interval;
use geom_surface::{Cylinder, Plane, SurfaceKind};

fn span(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

fn plane() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 2.0), (0.0, 2.0)),
        )
        .unwrap(),
    )
}

fn cylinder() -> SurfaceKind {
    SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            1.0,
            Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            ((0.0, 2.0), (0.0, 4.0)),
        )
        .unwrap(),
    )
}

#[test]
fn only_a_whole_turn_closes() {
    assert_eq!(closing(&plane(), &plane()), [false; 4]);
    assert_eq!(closing(&cylinder(), &plane()), [true, false, false, false]);
    assert_eq!(closing(&plane(), &cylinder()), [false, false, true, false]);
}

#[test]
fn a_cut_is_the_domain_of_a_closing_parameter_and_nothing_else() {
    assert_eq!(cuts_of(&plane(), &plane()), [None; 4]);
    assert_eq!(
        cuts_of(&cylinder(), &plane()),
        [Some((0.0, 2.0)), None, None, None]
    );
    assert_eq!(
        cuts_of(&plane(), &cylinder()),
        [None, None, Some((0.0, 2.0)), None]
    );
}

#[test]
fn a_box_moved_a_turn_is_moved_by_two_and_no_more() {
    assert_eq!(TURN.to_bits(), 2.0_f64.to_bits());
    let up = moved(span(0.0, 0.5), TURN).unwrap();
    assert!(up.lo() <= 2.0 && up.lo() > 1.999);
    assert!(up.hi() >= 2.5 && up.hi() < 2.501);
    let down = moved(span(2.0, 2.0), -TURN).unwrap();
    assert!(down.lo() <= 0.0 && down.hi() >= 0.0);
    assert!(down.hi() < 0.001 && down.lo() > -0.001);
}

fn at(u2: (f64, f64)) -> [Interval; 4] {
    [
        span(0.5, 0.5),
        span(1.0, 1.0),
        span(u2.0, u2.1),
        span(1.0, 1.0),
    ]
}

#[test]
fn boxes_are_wrapped_only_when_a_closing_parameter_needs_it() {
    let closes = [false, false, true, false];
    // Touching as they are: nothing to move, so no wrapped box (the clusters met already).
    assert_eq!(wrapped(&at((0.0, 0.1)), &at((0.1, 0.2)), closes), None);
    // One at each end of the closing parameter: moved a turn, they touch.
    let there = wrapped(&at((0.0, 0.1)), &at((1.95, 2.0)), closes).unwrap();
    assert!(there[2].hi() >= -0.05 && there[2].hi() < 0.001);
    assert_eq!(there[0], span(0.5, 0.5));
    // Apart along a parameter that does not close: never joined.
    assert_eq!(wrapped(&at((0.0, 0.1)), &at((1.95, 2.0)), [false; 4]), None);
    let mut off = at((1.95, 2.0));
    off[1] = span(1.5, 1.5);
    assert_eq!(wrapped(&at((0.0, 0.1)), &off, closes), None);
}

#[test]
fn the_other_order_wraps_the_other_way() {
    let closes = [false, false, true, false];
    let there = wrapped(&at((1.95, 2.0)), &at((0.0, 0.1)), closes).unwrap();
    assert!(there[2].lo() <= 2.0 && there[2].lo() > 1.999);
    assert!(there[2].hi() >= 2.1 && there[2].hi() < 2.101);
}

/// The start of the turned chart's second parameter u2 is 0.5; its first parameters do not close.
const STARTS: [Option<f64>; 4] = [None, None, Some(0.5), None];

#[test]
fn a_point_wholly_a_turn_past_the_start_moves_back_and_one_across_the_cut_stays() {
    let point = |u2: Interval| Reading::Point {
        at: [span(0.5, 0.5), span(1.0, 1.0), u2, span(1.0, 1.0)],
        gap: span(0.0, 0.0),
        contact: PointContact::IsolatedTangent,
    };
    let Reading::Point { at: back, .. } = unturned(point(span(2.6, 2.7)), &STARTS) else {
        panic!("a point stays a point");
    };
    assert!(back[2].lo() <= 0.6 && back[2].lo() > 0.59);
    assert!(back[2].hi() >= 0.7 && back[2].hi() < 0.71);
    // The other parameters keep their values: only the one with a start is moved.
    assert_eq!(back[0], span(0.5, 0.5));
    assert_eq!(back[3], span(1.0, 1.0));
    // Across the cut (it starts below the start plus a turn): left as it is.
    let Reading::Point { at: kept, .. } = unturned(point(span(2.0, 2.6)), &STARTS) else {
        panic!("a point stays a point");
    };
    assert_eq!(kept[2], span(2.0, 2.6));
    // Exactly at the start plus a turn is past it.
    let Reading::Point { at: edge, .. } = unturned(point(span(2.5, 2.5)), &STARTS) else {
        panic!("a point stays a point");
    };
    assert!(edge[2].hi() < 0.51 && edge[2].lo() > 0.49);
}

#[test]
fn a_patch_wholly_a_turn_past_the_start_moves_back_in_each_kind_of_reading() {
    let first = ((0.0, 1.0), (0.0, 1.0));
    let second = ((2.6, 2.9), (0.0, 1.0));
    let want = ((0.6_f64 - 1e-12, 0.9 + 1e-12), (0.0, 1.0));
    let near = |got: ((f64, f64), (f64, f64))| {
        (got.0.0 - want.0.0).abs() < 1e-9 && (got.0.1 - want.0.1).abs() < 1e-9 && got.1 == want.1
    };
    let undecided = Reading::Undecided {
        first,
        second,
        why: Unresolved::HigherOrderContact,
    };
    let Reading::Undecided {
        first: f,
        second: s,
        ..
    } = unturned(undecided, &STARTS)
    else {
        panic!("undecided stays undecided");
    };
    assert_eq!(f, first);
    assert!(near(s), "{s:?}");
    let curve = Reading::Curve {
        first,
        second,
        contact: CurveContact::Crossing,
    };
    let Reading::Curve {
        first: f,
        second: s,
        ..
    } = unturned(curve, &STARTS)
    else {
        panic!("a curve stays a curve");
    };
    assert_eq!(f, first);
    assert!(near(s), "{s:?}");
    // One that reaches across the cut stays.
    let across = Reading::Undecided {
        first,
        second: ((2.0, 2.9), (0.0, 1.0)),
        why: Unresolved::HigherOrderContact,
    };
    let Reading::Undecided { second: s, .. } = unturned(across, &STARTS) else {
        panic!("undecided stays undecided");
    };
    assert_eq!(s, ((2.0, 2.9), (0.0, 1.0)));
}

#[test]
fn the_ranges_of_a_pair_are_its_four_parameters_in_order() {
    assert_eq!(
        spans_of(((1.0, 2.0), (3.0, 4.0)), ((5.0, 6.0), (7.0, 8.0))),
        [(1.0, 2.0), (3.0, 4.0), (5.0, 6.0), (7.0, 8.0)]
    );
}

/// The turned chart of `STARTS` has the first chart's cut at 2.5 and its own at 1.5 and 3.5.
fn u2(a: f64, b: f64) -> [(f64, f64); 4] {
    [(1.5, 1.5), (0.0, 1.0), (a, b), (3.5, 3.5)]
}

#[test]
fn a_range_holds_the_first_cut_when_it_spans_a_turn_past_the_start() {
    assert_eq!(cut_of(&u2(2.4, 2.6), &STARTS), (true, false));
    assert_eq!(cut_of(&u2(2.5, 2.7), &STARTS), (true, false));
    assert_eq!(cut_of(&u2(2.3, 2.5), &STARTS), (true, false));
    assert_eq!(cut_of(&u2(2.6, 2.9), &STARTS), (false, false));
    assert_eq!(cut_of(&u2(2.0, 2.4), &STARTS), (false, false));
}

#[test]
fn a_range_touching_the_turned_charts_own_cut_is_marked() {
    // Its start (the half-way line, a turn from the first chart's start plus a half) ...
    assert_eq!(cut_of(&u2(1.5, 2.6), &STARTS), (true, true));
    // ... and its end, a turn on.
    assert_eq!(cut_of(&u2(2.4, 3.5), &STARTS), (true, true));
    // Near them is not on them; a parameter that does not close has no cut.
    assert_eq!(cut_of(&u2(1.6, 2.6), &STARTS), (true, false));
    assert_eq!(cut_of(&u2(2.4, 3.4), &STARTS), (true, false));
    assert_eq!(cut_of(&u2(2.4, 2.6), &[None; 4]), (false, false));
}

#[test]
fn only_the_cuts_a_reading_left_undecided_lies_on_are_kept() {
    let both = [Some((0.0, 2.0)), Some((0.0, 2.0)), None, None];
    let undecided = |first: ((f64, f64), (f64, f64))| Reading::Undecided {
        first,
        second: ((0.0, 1.0), (0.0, 1.0)),
        why: Unresolved::HigherOrderContact,
    };
    // On the end of u only.
    let at_u = [undecided(((1.5, 2.0), (0.3, 0.4)))];
    assert_eq!(
        cuts_with_readings(both, &at_u),
        [Some((0.0, 2.0)), None, None, None]
    );
    // On the start of v only.
    let at_v = [undecided(((0.3, 0.4), (0.0, 0.5)))];
    assert_eq!(
        cuts_with_readings(both, &at_v),
        [None, Some((0.0, 2.0)), None, None]
    );
    // On neither: no cut is turned.
    let inside = [undecided(((0.3, 0.4), (0.3, 0.4)))];
    assert_eq!(cuts_with_readings(both, &inside), [None; 4]);
    // A tangent curve counts, a decided point does not.
    let curve = [Reading::Curve {
        first: ((1.5, 2.0), (0.3, 0.4)),
        second: ((0.0, 1.0), (0.0, 1.0)),
        contact: CurveContact::Crossing,
    }];
    assert_eq!(
        cuts_with_readings(both, &curve),
        [Some((0.0, 2.0)), None, None, None]
    );
    let point = [Reading::Point {
        at: [
            span(2.0, 2.0),
            span(0.5, 0.5),
            span(0.5, 0.5),
            span(0.5, 0.5),
        ],
        gap: span(0.0, 0.0),
        contact: PointContact::IsolatedTangent,
    }];
    assert_eq!(cuts_with_readings(both, &point), [None; 4]);
}

#[test]
fn a_settled_pair_is_in_a_seam_when_it_touches_a_flagged_pair_in_both_patches() {
    let near = ((0.0, 1.0), (0.0, 1.0));
    let touching = ((0.5, 2.0), (0.5, 2.0));
    let apart = ((5.0, 6.0), (5.0, 6.0));
    let seam = [FlaggedPair {
        first: near,
        second: near,
        why: Unresolved::HigherOrderContact,
    }];
    let pair = |first, second| Settled {
        first,
        second,
        starts: Vec::new(),
    };
    assert!(in_a_seam(&seam, &pair(touching, touching)));
    assert!(!in_a_seam(&seam, &pair(touching, apart)));
    assert!(!in_a_seam(&seam, &pair(apart, touching)));
    assert!(!in_a_seam(&seam, &pair(apart, apart)));
    assert!(!in_a_seam(&[], &pair(touching, touching)));
}
