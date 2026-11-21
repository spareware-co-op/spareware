//! A circle lying in a plane whose domain has a side within a rounding of touching the circle
//! (decisions 32.80 and 32.108, issues 1002 and 1003). The unit circle about the origin in z = 0
//! against the plane z = 0, a side of the domain one to four steps inside the point where it would
//! touch: the side crosses the circle at two points about 3e-8 apart. The interval enclosure of
//! r² − b² reaches zero there, so each crossing was held as reaching the touch, and 32.80 left the
//! pair to the engine: one flagged region for 5,088 units. Now the square's exact sum, two steps
//! wide, holds each crossing to its place, and the closed form answers the arc the domain holds,
//! its stretches apart. A side as far outside holds the sliver between the two crossings, one arc
//! that now ends at them. On the touch itself, and where two crossings stand closer than a step
//! of their angle, the answer is the engine's one flagged region, as before. Away from a touch
//! every enclosure is the interval's, as it was.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{Circle, CurveKind, Frame};
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::result::{Coincidence, Item, Unresolved};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};

const X: [f64; 3] = [1.0, 0.0, 0.0];
const Y: [f64; 3] = [0.0, 1.0, 0.0];
const Z: [f64; 3] = [0.0, 0.0, 1.0];

/// The domain's other sides, far from the circle.
const FAR: (f64, f64) = (-5.0, 5.0);

/// The closed form's work on a side inside a near touch: the signs, the angles and a box of
/// each of the two stretches either side of the circle's join. Before, the engine took thousands
/// more (5,088 at the lowest point, decision 32.80).
const CLOSED: u128 = 11;

/// The same where the stretch left out holds the join, so the arc is one stretch.
const CLOSED_AT_JOIN: u128 = 7;

/// The engine's work where the closed form leaves the pair, as before 32.69.
const ENGINE: u128 = 5088;

/// What the closed form spends before it finds the stretches meet and leaves the pair.
const TRIED: u128 = 9;

/// How far an end's enclosure may stand from the true crossing, in half-turns. The interval's
/// enclosure of the square stood about 6e-9 off.
const CLOSE: f64 = 1e-14;

/// The unit circle about the origin in z = 0, from x towards y, against the plane z = 0 through
/// `origin` over the domain `u` along x and `v` along y: its items, and the work.
fn meet(origin: [f64; 3], u: (f64, f64), v: (f64, f64)) -> (Vec<Item>, u128) {
    let circle = CurveKind::Circle(
        Circle::new([0.0; 3], 1.0, Frame::new(X, Z).unwrap(), (0.0, 2.0)).unwrap(),
    );
    let plane = SurfaceKind::Plane(Plane::new(origin, X, Y, (u, v)).unwrap());
    let mut meter = WorkMeter::new(Budget::of_work(50_000_000));
    let found = curve_surface(&circle, &plane, &mut meter).unwrap();
    (found.items().to_vec(), meter.used().work)
}

/// The spans in t of `items`, each an overlap by identity, in order round the turn.
fn stretches(items: &[Item]) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = items
        .iter()
        .map(|item| {
            let Item::Overlap {
                at,
                proof: Coincidence::Identical,
                ..
            } = item
            else {
                panic!("not an arc on the plane: {items:?}");
            };
            let t = at.spans()[0];
            (t.lo(), t.hi())
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// The one stretch of the turn that `spans` leave uncovered, from where one ends to where the next
/// starts, running past 2 where it passes the join.
fn gap(spans: &[(f64, f64)]) -> (f64, f64) {
    let mut gaps = Vec::new();
    for (k, &(_, end)) in spans.iter().enumerate() {
        let next = spans.get(k + 1).map_or(spans[0].0 + 2.0, |s| s.0);
        if next > end {
            gaps.push((end, next));
        }
    }
    assert_eq!(gaps.len(), 1, "{spans:?}");
    gaps[0]
}

/// The one arc that `spans` cover, running past 2 where it passes the join.
fn arc(spans: &[(f64, f64)]) -> (f64, f64) {
    match spans {
        [one] => *one,
        [(0.0, upto), (from, 2.0)] => (*from, upto + 2.0),
        _ => panic!("not one arc: {spans:?}"),
    }
}

/// Half the angle between the two crossings of the line at `c` from the centre, in half-turns: the
/// turn of the half-chord √(1 − c²) seen from the centre over |c|.
fn half_angle(c: f64) -> f64 {
    let a = c.abs();
    let chord = ((1.0 - a) * (1.0 + a)).sqrt();
    geom_num::math::atan2(chord, a).unwrap().get() / core::f64::consts::PI
}

/// `v` moved `k` steps towards zero.
fn inward(mut v: f64, k: usize) -> f64 {
    for _ in 0..k {
        v = if v < 0.0 { v.next_up() } else { v.next_down() };
    }
    v
}

/// A side of the domain at `end`, a touching point's coordinate moved inward: the side's domain,
/// holding the circle's centre (`inside`) or not, and the turn of the point where it would touch.
fn side(axis: usize, touch: f64, end: f64, inside: bool) -> ((f64, f64), (f64, f64), f64) {
    let held = if (touch < 0.0) == inside {
        (end, 5.0)
    } else {
        (-5.0, end)
    };
    let turn = match (axis, touch < 0.0) {
        (0, false) => 2.0,
        (0, true) => 1.0,
        (_, false) => 0.5,
        (_, true) => 1.5,
    };
    if axis == 0 {
        (held, FAR, turn)
    } else {
        (FAR, held, turn)
    }
}

/// Each side one to four steps inside touching the circle: the side crosses it twice, the closed
/// form answers the arc the domain holds, and the stretch it leaves out runs between the two true
/// crossings, which the enclosures tell apart. Before, each was the engine's flagged region.
#[test]
fn a_side_a_step_inside_touching_crosses_the_circle_twice() {
    for axis in [0, 1] {
        for touch in [-1.0, 1.0] {
            for k in 1..=4 {
                let end = inward(touch, k);
                let (u, v, turn) = side(axis, touch, end, true);
                let (items, work) = meet([0.0; 3], u, v);
                let case = format!("side {axis} at {end:e}");
                let (from, to) = gap(&stretches(&items));
                let half = half_angle(end);
                assert!((from - (turn - half)).abs() <= CLOSE, "{from} {case}");
                assert!((to - (turn + half)).abs() <= CLOSE, "{to} {case}");
                let closed = if turn == 2.0 { CLOSED_AT_JOIN } else { CLOSED };
                assert_eq!(work, closed, "{case}");
            }
        }
    }
}

/// Each side one to four steps outside touching the circle: the domain holds the sliver between
/// the two crossings, one arc whose ends are the crossings' own. Before, the closed form's arc
/// reached about 6e-9 past each.
#[test]
fn the_sliver_beyond_a_side_a_step_outside_touching_ends_at_its_crossings() {
    for axis in [0, 1] {
        for touch in [-1.0, 1.0] {
            for k in 1..=4 {
                let end = inward(touch, k);
                let (u, v, turn) = side(axis, touch, end, false);
                let (items, _) = meet([0.0; 3], u, v);
                let case = format!("side {axis} at {end:e}");
                let (from, to) = arc(&stretches(&items));
                let half = half_angle(end);
                assert!((from - (turn - half)).abs() <= CLOSE, "{from} {case}");
                assert!((to - (turn + half)).abs() <= CLOSE, "{to} {case}");
            }
        }
    }
}

/// A side on the lowest point touches the circle: holding the centre it holds the whole circle,
/// and from the other side the lone point, which is the engine's one flagged region.
#[test]
fn a_side_on_the_touch_holds_the_whole_circle_or_one_flagged_region() {
    let (items, _) = meet([0.0; 3], FAR, (-1.0, 5.0));
    assert_eq!(stretches(&items), [(0.0, 2.0)]);
    let (items, work) = meet([0.0; 3], FAR, (-5.0, -1.0));
    let [
        Item::Flagged {
            at,
            why: Unresolved::HigherOrderContact,
            ..
        },
    ] = &items[..]
    else {
        panic!("not one flagged region: {items:?}");
    };
    assert!(at.spans()[0].contains(1.5), "{items:?}");
    assert_eq!(work, 1056);
}

/// The side at y = −1 + 5e-33 crosses the circle at x = ±1e-16, whose angles stand within a step
/// of 1.5 half-turns: the two crossings are apart, but no enclosure of their angles tells them
/// apart, so the stretches would meet and the pair stays the engine's one flagged region.
#[test]
fn crossings_closer_than_a_step_of_their_angle_are_one_flagged_region() {
    let (items, work) = meet([0.0, 5e-33, 0.0], FAR, (-1.0, 5.0));
    let [
        Item::Flagged {
            at,
            why: Unresolved::HigherOrderContact,
            ..
        },
    ] = &items[..]
    else {
        panic!("not one flagged region: {items:?}");
    };
    assert!(at.spans()[0].contains(1.5), "{items:?}");
    assert_eq!(work, ENGINE + TRIED);
}

/// A side far from touching: the interval's enclosure of the square is kept, and so is every bit
/// of the arc's ends (the square's exact sum would give 1.3564337068712955 and
/// 1.6435662931287054).
#[test]
fn a_side_far_from_touching_keeps_its_interval_enclosure() {
    let (items, work) = meet([0.0; 3], FAR, (-0.9, 5.0));
    assert_eq!(
        stretches(&items),
        [
            (0.0, 1.356_433_706_871_296_4),
            (1.643_566_293_128_705_2, 2.0)
        ]
    );
    assert_eq!(work, CLOSED);
}
