//! A line parallel to a plane whose stored vectors are not whole numbers (decision 32.85, issue
//! 995). Whether the line runs along the plane, and whether it lies in it, are the signs of the
//! normal against the direction and against the offset of the plane from the line, each a sum of
//! products of three stored doubles, decided exactly. Lying in it is one overlap by identity over
//! what the domain holds; parallel off it is nothing, proven; any other line is the one point it
//! was. Before, an enclosure of a step whose true value is zero always straddled zero, and the
//! engine ran the whole segment: a million units, or the budget.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly, and the names are a point's coordinates"
)]

use geom_curve::{CurveKind, Line};
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::result::{Coincidence, Intersection, Item, PointContact};
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};

/// Far past what a closed form spends, and far short of the engine's million units on these
/// pairs: a pair the closed form leaves stops here instead of running on.
const BUDGET: u64 = 100_000;

/// How far off a plane the moved lines stand, along x: 2^-30.
const HAIR: f64 = 1.0 / 1_073_741_824.0;

/// The plane through `c` on `a` and `b`, on a square of half-side 300.
fn plane(c: [f64; 3], a: [f64; 3], b: [f64; 3]) -> SurfaceKind {
    let side = (-300.0, 300.0);
    SurfaceKind::Plane(Plane::new(c, a, b, (side, side)).unwrap())
}

/// The segment from `a` to `b`.
fn segment(a: [f64; 3], b: [f64; 3]) -> CurveKind<3> {
    CurveKind::Line(Line::segment(a, b).unwrap().value)
}

/// The answer and the work it took.
fn meet(c: &CurveKind<3>, s: &SurfaceKind) -> (Intersection, u128) {
    let mut meter = WorkMeter::new(Budget::of_work(BUDGET));
    let found = curve_surface(c, s, &mut meter).unwrap();
    (found, meter.used().work)
}

/// Each span of the one item's box, as (lo, hi).
fn spans(item: &Item) -> Vec<(f64, f64)> {
    item.at().spans().iter().map(|s| (s.lo(), s.hi())).collect()
}

/// Whether `found` is one overlap by identity whose box holds the whole segment, t in [0, 1].
fn is_the_whole_overlap(found: &Intersection) -> bool {
    match found.items() {
        [
            item @ Item::Overlap {
                proof: Coincidence::Identical,
                ..
            },
        ] => spans(item)[0] == (0.0, 1.0),
        _ => false,
    }
}

/// The three planes of the piercing target's stored timeouts, and of one slow unit, each with a
/// vector holding a half, and a segment along y lying in it.
fn lying() -> [(CurveKind<3>, SurfaceKind); 5] {
    [
        (
            segment([0.0, 128.0, 0.0], [0.0, -100.0, 0.0]),
            plane([0.0; 3], [9.0, 0.0, -43.5], [0.0, 1.0, 0.0]),
        ),
        (
            segment([0.0, 12.0, 0.0], [0.0, 16.0, 0.0]),
            plane([0.0; 3], [16.0, 0.0, 48.5], [0.0, 169.0, 0.0]),
        ),
        (
            segment([0.0, 16.0, 0.0], [0.0, 0.0, 0.0]),
            plane([0.0; 3], [1.0, 0.0, 4.5], [0.0, 50.0, 0.0]),
        ),
        (
            segment([0.0, 164.0, 0.0], [0.0, 128.0, 0.0]),
            plane([0.0; 3], [1.0, 0.0, -0.5], [0.0, 2.0, 0.0]),
        ),
        // On the axis off the grid: the vectors' doubles are roundings of 0.8 and 0.6, and the
        // segment runs 256 times the first, so its normal's products cancel exactly.
        (
            segment([0.0, 0.0, 5.0], [256.0 * 0.8, 256.0 * -0.6, 5.0]),
            plane([0.0; 3], [0.8, -0.6, 0.0], [0.0, 0.0, 1.0]),
        ),
    ]
}

#[test]
fn a_line_lying_in_a_plane_off_the_grid_is_one_overlap() {
    for (line, plane) in lying() {
        let (found, _) = meet(&line, &plane);
        assert!(is_the_whole_overlap(&found), "{:?}", found.items());
    }
}

#[test]
fn a_line_a_hair_off_a_plane_off_the_grid_meets_nothing() {
    for (line, plane) in lying() {
        let CurveKind::Line(l) = &line else {
            panic!("a segment is a line");
        };
        let [x, y, z] = l.origin();
        let [p, q, r] = l.direction();
        // Moved off the plane along x by 2^-30, and still parallel: a tolerance would call it in
        // the plane, and it is not.
        let off = x + HAIR;
        let moved = CurveKind::Line(
            Line::segment([off, y, z], [off + p, y + q, z + r])
                .unwrap()
                .value,
        );
        let (found, _) = meet(&moved, &plane);
        assert!(found.items().is_empty(), "{:?}", found.items());
    }
}

#[test]
fn a_parallel_line_well_off_a_plane_off_the_grid_meets_nothing() {
    // The piercing target's three slow units: 0.48, 8.3 and 3.97 off.
    let pairs = [
        (
            segment([0.0, 148.0, 0.0], [0.0, -100.0, 0.0]),
            plane([-4.0, -4.0, -4.0], [8.0, 0.0, 9.5], [0.0, 1.0, 0.0]),
        ),
        (
            segment([0.0, 12.0, 0.0], [0.0, 16.0, 0.0]),
            plane([16.0, -4.0, 0.0], [80.0, 0.0, 48.5], [0.0, 169.0, 0.0]),
        ),
        (
            segment([0.0, 0.0, 0.0], [0.0, 252.0, 0.0]),
            plane([0.0, 0.0, 4.0], [214.0, 0.0, 24.5], [0.0, 1.0, 0.0]),
        ),
    ];
    for (line, plane) in pairs {
        let (found, _) = meet(&line, &plane);
        assert!(found.items().is_empty(), "{:?}", found.items());
    }
}

#[test]
fn a_line_across_a_plane_off_the_grid_is_one_point() {
    let p = plane([0.0; 3], [9.0, 0.0, -43.5], [0.0, 1.0, 0.0]);
    // Through the plane at (0, 3, 0), half way along.
    let (found, _) = meet(&segment([-5.0, 3.0, 0.0], [5.0, 3.0, 0.0]), &p);
    match found.items() {
        [
            item @ Item::Point {
                contact: PointContact::Crossing,
                ..
            },
        ] => {
            let t = spans(item)[0];
            assert!(t.0 <= 0.5 && 0.5 <= t.1, "{t:?}");
        }
        other => panic!("{other:?}"),
    }
    // Tilted from lying in it by a hair at its far end: no longer parallel, so it meets the plane
    // once, at its first end, which lies in it.
    let (found, _) = meet(
        &segment([0.0, 128.0, 0.0], [0.0, -100.0, 1.0_f64.next_down() - 1.0]),
        &p,
    );
    match found.items() {
        [item @ Item::Point { .. }] => assert!(spans(item)[0].0 <= 0.0, "{item:?}"),
        other => panic!("{other:?}"),
    }
}

/// A plane on vectors none of whose coordinates is zero, and a point of it, so that every product
/// of the two signs counts: c + a − b.
const C: [f64; 3] = [1.5, -2.5, 0.25];
const A: [f64; 3] = [1.5, 2.25, -0.75];
const B: [f64; 3] = [0.5, -1.25, 2.5];
const ON: [f64; 3] = [2.5, 1.0, -3.0];
/// 2a + 3b, along the plane.
const ALONG: [f64; 3] = [4.5, 0.75, 6.0];

/// `p + s·q`, exact for the numbers here.
fn step(p: [f64; 3], s: f64, q: [f64; 3]) -> [f64; 3] {
    let ([x, y, z], [e, f, g]) = (p, q);
    [x + s * e, y + s * f, z + s * g]
}

#[test]
fn every_product_of_a_general_plane_counts() {
    let p = plane(C, A, B);
    // In it, from either end.
    for (from, to) in [(ON, step(ON, 1.0, ALONG)), (step(ON, 1.0, ALONG), ON)] {
        let (found, _) = meet(&segment(from, to), &p);
        assert!(is_the_whole_overlap(&found), "{:?}", found.items());
    }
    // Parallel, an eighth off it on either side (the normal's z is −3).
    for z in [0.125, -0.125] {
        let from = step(ON, z, [0.0, 0.0, 1.0]);
        let (found, _) = meet(&segment(from, step(from, 1.0, ALONG)), &p);
        assert!(found.items().is_empty(), "{z}: {:?}", found.items());
    }
    // Across it, either way, at its middle.
    let (high, low) = (
        step(ON, 1.0, [0.0, 0.0, 1.0]),
        step(ON, -1.0, [0.0, 0.0, 1.0]),
    );
    for (from, to) in [(high, low), (low, high)] {
        let (found, _) = meet(&segment(from, to), &p);
        match found.items() {
            [
                item @ Item::Point {
                    contact: PointContact::Crossing,
                    ..
                },
            ] => {
                let t = spans(item)[0];
                assert!(t.0 <= 0.5 && 0.5 <= t.1, "{t:?}");
            }
            other => panic!("{other:?}"),
        }
    }
}

/// An enclosure of `(p − q) · (a × b)` from the stored doubles, as the closed form takes it.
fn enclosure(p: [f64; 3], q: [f64; 3], a: [f64; 3], b: [f64; 3]) -> Interval {
    let lift = |v: [f64; 3]| v.map(|x| Interval::point(x).unwrap());
    let ([p0, p1, p2], [q0, q1, q2]) = (lift(p), lift(q));
    let w = [
        p0.sub(q0).unwrap(),
        p1.sub(q1).unwrap(),
        p2.sub(q2).unwrap(),
    ];
    let ([a0, a1, a2], [b0, b1, b2]) = (lift(a), lift(b));
    let n = [
        a1.mul(b2).unwrap().sub(a2.mul(b1).unwrap()).unwrap(),
        a2.mul(b0).unwrap().sub(a0.mul(b2).unwrap()).unwrap(),
        a0.mul(b1).unwrap().sub(a1.mul(b0).unwrap()).unwrap(),
    ];
    let dot = |x: [Interval; 3]| {
        let t = x[0]
            .mul(n[0])
            .unwrap()
            .add(x[1].mul(n[1]).unwrap())
            .unwrap();
        t.add(x[2].mul(n[2]).unwrap()).unwrap()
    };
    dot(w)
}

#[test]
fn a_line_across_a_plane_off_the_grid_is_told_by_its_enclosure_alone() {
    // The step's enclosure excludes zero, so the line crosses the plane: one point, from the
    // closed form's division, at its cost (4 units) and not the engine's.
    let p = plane(C, A, B);
    let (high, low) = (
        step(ON, 1.0, [0.0, 0.0, 1.0]),
        step(ON, -1.0, [0.0, 0.0, 1.0]),
    );
    let along = enclosure(step(high, -1.0, low), [0.0; 3], A, B);
    assert!(!along.contains(0.0), "{along:?}");
    let (found, work) = meet(&segment(high, low), &p);
    assert!(
        matches!(found.items(), [Item::Point { .. }]),
        "{:?}",
        found.items()
    );
    assert!(work <= 4, "{work}");
}

#[test]
fn a_pair_whose_enclosures_hold_zero_is_told_by_the_exact_signs() {
    // Lying in the plane: the step's and the offset's true values are zero, and their enclosures
    // straddle it. Only the exact signs say parallel and in it.
    for (line, plane) in lying() {
        let (CurveKind::Line(l), SurfaceKind::Plane(s)) = (&line, &plane) else {
            panic!("a segment and a plane");
        };
        let (c, a, b) = s.vectors();
        let along = enclosure(l.direction(), [0.0; 3], a, b);
        let side = enclosure(c, l.origin(), a, b);
        assert!(
            along.contains(0.0) && side.contains(0.0),
            "{along:?} {side:?}"
        );
        let (found, work) = meet(&line, &plane);
        assert!(is_the_whole_overlap(&found), "{:?}", found.items());
        assert!(work <= 1, "{work}");
    }
    // Parallel to the general plane and off it by one unit in the last place of a coordinate of
    // the origin: the offset is 3 · 2^-51 off zero, and its enclosure straddles zero. The exact
    // sum says off it: nothing, proven, where an enclosure alone would leave it to the engine.
    let p = plane(C, A, B);
    let [x, y, z] = ON;
    let from = [x, y, z.next_up()];
    let to = step(from, 1.0, ALONG);
    assert_eq!(
        step(to, -1.0, from),
        ALONG,
        "the direction is stored exactly"
    );
    let along = enclosure(ALONG, [0.0; 3], A, B);
    let side = enclosure(C, from, A, B);
    assert!(
        along.contains(0.0) && side.contains(0.0),
        "{along:?} {side:?}"
    );
    let (found, work) = meet(&segment(from, to), &p);
    assert!(found.items().is_empty(), "{:?}", found.items());
    assert!(work <= 1, "{work}");
}

#[test]
fn a_parallel_line_an_ulp_below_a_plane_whose_enclosures_hold_zero_meets_nothing() {
    // The other side of the one above: the origin's z one unit in the last place lower, so the
    // offset is −3 · 2^-51, a negative exact sum, and both enclosures still straddle zero. Every
    // other negative sign in these tests is far enough off zero for its enclosure to tell it, so
    // only this pair asks the exact sum for a negative sign. Read as zero, it would put the line
    // in the plane: the whole overlap, where the answer is nothing.
    let p = plane(C, A, B);
    let [x, y, z] = ON;
    let from = [x, y, z.next_down()];
    let to = step(from, 1.0, ALONG);
    assert_eq!(
        step(to, -1.0, from),
        ALONG,
        "the direction is stored exactly"
    );
    let along = enclosure(ALONG, [0.0; 3], A, B);
    let side = enclosure(C, from, A, B);
    assert!(
        along.contains(0.0) && side.contains(0.0),
        "{along:?} {side:?}"
    );
    let (found, work) = meet(&segment(from, to), &p);
    assert!(found.items().is_empty(), "{:?}", found.items());
    assert!(work <= 1, "{work}");
}

/// The plane z = 0 over the square (0, 10) by (0, 10), u along x and v along y.
fn square() -> SurfaceKind {
    let side = (0.0, 10.0);
    SurfaceKind::Plane(
        Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], (side, side)).unwrap(),
    )
}

/// The segment `o + t·d`, t in [0, 1], its `d` stored as given.
fn stored(o: [f64; 3], d: [f64; 3]) -> CurveKind<3> {
    CurveKind::Line(Line::new(o, d, (0.0, 1.0)).unwrap())
}

/// `x` more `n` steps of the doubles.
fn up(x: f64, n: u32) -> f64 {
    (0..n).fold(x, |y, _| y.next_up())
}

/// `x` less `n` steps of the doubles.
fn down(x: f64, n: u32) -> f64 {
    (0..n).fold(x, |y, _| y.next_down())
}

/// Whether `span` holds `(lo, hi)`, each within 10⁻⁹ outside.
fn holds(span: (f64, f64), (lo, hi): (f64, f64)) -> bool {
    span.0 <= lo && lo - span.0 < 1e-9 && hi <= span.1 && span.1 - hi < 1e-9
}

/// Whether `found` is one overlap by identity over the part of the segment `t`, at `u` and `v`.
fn is_the_part(found: &Intersection, t: (f64, f64), u: (f64, f64), v: (f64, f64)) -> bool {
    match found.items() {
        [
            item @ Item::Overlap {
                proof: Coincidence::Identical,
                ..
            },
        ] => {
            let spans = spans(item);
            holds(spans[0], t) && holds(spans[1], u) && holds(spans[2], v)
        }
        _ => false,
    }
}

#[test]
fn a_segment_in_a_plane_past_its_domain_is_the_part_on_it() {
    // Issue 1159: a segment lying in a plane that runs past an end of its domain lies on it only
    // where its u and its v are both in the domain, and the overlap is that part, its t cut there
    // and its u and v the part's. Before, t was always the whole [0, 1]. The issue's two, the
    // first run backwards too; one whose v is cut by its u's end; one along the domain's edge
    // u = 0, whose u does not change and so cuts nothing; and one on the general plane, its
    // vectors neither whole nor square to each other, over (0, 1) by (0, 1), along a from u = −1
    // to 1 at v = 1/2.
    let general = SurfaceKind::Plane(Plane::new(C, A, B, ((0.0, 1.0), (0.0, 1.0))).unwrap());
    let cases = [
        (
            segment([-5.0, 5.0, 0.0], [5.0, 5.0, 0.0]),
            square(),
            (0.5, 1.0),
            (0.0, 5.0),
            (5.0, 5.0),
        ),
        (
            segment([5.0, 5.0, 0.0], [-5.0, 5.0, 0.0]),
            square(),
            (0.0, 0.5),
            (0.0, 5.0),
            (5.0, 5.0),
        ),
        (
            segment([-5.0, -5.0, 0.0], [15.0, 15.0, 0.0]),
            square(),
            (0.25, 0.75),
            (0.0, 10.0),
            (0.0, 10.0),
        ),
        (
            segment([-5.0, -5.0, 0.0], [15.0, 5.0, 0.0]),
            square(),
            (0.5, 0.75),
            (5.0, 10.0),
            (0.0, 2.5),
        ),
        (
            segment([0.0, -5.0, 0.0], [0.0, 5.0, 0.0]),
            square(),
            (0.5, 1.0),
            (0.0, 0.0),
            (0.0, 5.0),
        ),
        (
            segment(
                step(step(C, -1.0, A), 0.5, B),
                step(step(C, 1.0, A), 0.5, B),
            ),
            general,
            (0.5, 1.0),
            (0.0, 1.0),
            (0.5, 0.5),
        ),
    ];
    for (line, plane, t, u, v) in cases {
        let (found, work) = meet(&line, &plane);
        assert!(is_the_part(&found, t, u, v), "{:?}", found.items());
        assert_eq!(work, 1);
    }
}

#[test]
fn a_segment_in_a_plane_past_a_corner_of_its_domain_is_nothing() {
    // Issue 1159: a segment whose u and v each run onto the domain, but never both at once, passes
    // a corner outside it and meets nothing. Before, u and v were cut on their own and the answer
    // was an overlap over the whole segment. One past each corner, a unit off it, and one wholly
    // outside, 25 off.
    let cases = [
        segment([-6.0, 5.0, 0.0], [4.0, 15.0, 0.0]),
        segment([5.0, -6.0, 0.0], [15.0, 4.0, 0.0]),
        segment([-6.0, 5.0, 0.0], [4.0, -5.0, 0.0]),
        segment([5.0, 16.0, 0.0], [15.0, 6.0, 0.0]),
        segment([-20.0, 5.0, 0.0], [5.0, 30.0, 0.0]),
    ];
    for line in cases {
        let (found, work) = meet(&line, &square());
        assert!(found.items().is_empty(), "{:?}", found.items());
        assert_eq!(work, 0);
    }
}

#[test]
fn a_segment_in_a_plane_a_hair_past_a_corner_of_its_domain_is_nothing() {
    // Issue 1159: every bound of an enclosure is strictly past its exact value, so a segment that
    // passes a corner of the domain outside it by less than its rounding can still have each of
    // its u and v land on the domain, and its t cut by the one to meet, or cross, its t cut by the
    // other. It is not on the bounded plane: nothing. Past (10, 10) by 37 steps above v = 16,
    // where the two cuts meet; past (0, 10) by 43 steps above v = 9, where they meet too; past
    // (10, 10) by 38 steps, where they cross; and past (0, 10) by 24 steps above v = 9, where they
    // do not meet but the part's u or v is past an end.
    let cases = [
        stored([0.0, up(16.0, 37), 0.0], [20.0, -12.0, 0.0]),
        stored([-9.0, up(9.0, 43), 0.0], [18.0, 2.0, 0.0]),
        stored([0.0, up(16.0, 38), 0.0], [20.0, -12.0, 0.0]),
        stored([-1.0, up(9.0, 24), 0.0], [2.0, 2.0, 0.0]),
    ];
    for line in cases {
        let (found, work) = meet(&line, &square());
        assert!(found.items().is_empty(), "{:?}", found.items());
        assert_eq!(work, 0);
    }
    // A hair short of the corner instead, on the domain by a step: the part there, however short.
    let (found, _) = meet(
        &stored([0.0, down(16.0, 1), 0.0], [20.0, -12.0, 0.0]),
        &square(),
    );
    assert!(
        matches!(found.items(), [Item::Overlap { .. }]),
        "{:?}",
        found.items()
    );
}
