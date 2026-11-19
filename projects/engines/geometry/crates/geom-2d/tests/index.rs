//! Tests of `geom_2d::index` (piece G198): the 2D index on `geom-bvh` and the snap query.
//!
//! What this file proves: the tree finds exactly the entities whose boxes meet the aperture; the
//! closest snap wins and a tie goes to the lower name; a crossing of two entities near the cursor
//! is found; nothing is found outside the aperture; and on random drawings from a printed seed
//! (`SPAREKIT_SEED` replaces it) the tree's answer is the scan's, so the index never misses.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    reason = "tests, written as the definitions read; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::num::NonZeroU64;

use geom_2d::carrier::{Carriers, Circle2, Curve2};
use geom_2d::entity::{Attr, Ends, Entity, Frac, Polyline, Shape, Spline};
use geom_2d::index::{IndexError, SnapIndex, bounds};
use geom_2d::point::{Place, Pt2};
use geom_2d::snap::{Drawing, SnapKind};
use geom_num::NumError;
use geom_num::arc::{Circle2 as GridCircle, Round};
use geom_num::carrier::{Carrier, Rational2};
use geom_num::conic::Ellipse2;
use geom_num::grid::{P2, Steps};
use geom_num::interval::Interval;
use geom_num::one_root::{Crossing2, Root};
use geom_op::NameId;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn name(n: u32) -> NameId {
    let mut b = [0_u8; 16];
    b[12..].copy_from_slice(&n.to_be_bytes());
    NameId::from_bytes(b)
}

fn segment(a: (i64, i64), b: (i64, i64)) -> Shape {
    Shape::Line {
        carrier: Carrier::through(p(a.0, a.1), p(b.0, b.1)).unwrap(),
        ends: Ends::Segment(Pt2::Grid(p(a.0, a.1)), Pt2::Grid(p(b.0, b.1))),
    }
}

fn drawing(shapes: Vec<Shape>) -> Drawing {
    let items = shapes
        .into_iter()
        .zip(0_u32..)
        .map(|(s, n)| (name(n), Entity::new(Attr(0), s)))
        .collect();
    Drawing::new(items, Vec::new()).unwrap()
}

fn build(d: &Drawing) -> SnapIndex<'_> {
    SnapIndex::build(d, &Exec::serial(), &mut meter()).unwrap()
}

#[test]
fn the_tree_finds_the_entities_whose_boxes_meet_the_aperture() {
    let d = drawing(vec![
        segment((0, 0), (10, 0)),
        segment((100, 100), (110, 100)),
        Shape::Point(p(3, 4)),
        Shape::Circle(Circle2::Grid(GridCircle::new(p(50, 50), 5).unwrap())),
    ]);
    let index = build(&d);
    let near = |x, y, r| index.candidates(p(x, y), Steps(r), &mut meter()).unwrap();
    assert_eq!(near(5, 2, 3), [name(0), name(2)]);
    assert_eq!(near(105, 100, 1), [name(1)]);
    assert_eq!(near(50, 56, 1), [name(3)]);
    assert_eq!(near(500, 500, 10), Vec::<NameId>::new());
}

#[test]
fn the_closest_snap_wins_and_a_tie_goes_to_the_lower_name() {
    // Three segments meeting at (10, 0) from different sides, named 0 to 2, and a point at (13, 0).
    let d = drawing(vec![
        segment((0, 0), (10, 0)),
        segment((10, 0), (10, 10)),
        segment((20, 0), (10, 0)),
        Shape::Point(p(13, 0)),
    ]);
    let index = build(&d);
    let mut c = Carriers::new();
    let kinds = [SnapKind::End, SnapKind::Start, SnapKind::Node];
    // At (12, 0): the segments' shared end is 2 away, the point 1: the point wins.
    let found = index
        .snap(p(12, 0), Steps(5), &kinds, &mut c, &mut meter())
        .unwrap()
        .unwrap();
    assert_eq!((found.kind, found.name), (SnapKind::Node, name(3)));
    // At (10, 1): three ends coincide at distance 1; the lowest name takes it.
    let found = index
        .snap(p(10, 1), Steps(5), &kinds, &mut c, &mut meter())
        .unwrap()
        .unwrap();
    assert_eq!(found.name, name(0));
    // Outside the aperture there is nothing.
    assert!(
        index
            .snap(p(10, 30), Steps(5), &kinds, &mut c, &mut meter())
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_crossing_of_two_entities_near_the_cursor_is_found() {
    let d = drawing(vec![segment((0, 0), (10, 10)), segment((0, 10), (10, 0))]);
    let index = build(&d);
    let mut c = Carriers::new();
    let found = index
        .snap(
            p(6, 4),
            Steps(3),
            &[SnapKind::Intersection],
            &mut c,
            &mut meter(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        (found.kind, found.name, found.with),
        (SnapKind::Intersection, name(0), Some(name(1)))
    );
    let Place::OneRoot(z) = found.point.place(&c).unwrap() else {
        panic!("not a one-root point");
    };
    assert!(
        z.same(&Crossing2::of_point(&geom_num::carrier::Rational2::new(5, 5, 1).unwrap()).unwrap())
            .unwrap()
    );
}

#[test]
fn a_boxes_hold_what_a_snap_can_be() {
    let c = Carriers::new();
    let b = bounds(&Entity::new(Attr(0), segment((3, 9), (-4, 2))), &c)
        .unwrap()
        .unwrap();
    assert_eq!(b, ([-4, 2], [3, 9]));
    let b = bounds(
        &Entity::new(
            Attr(0),
            Shape::Circle(Circle2::Grid(GridCircle::new(p(10, 10), 5).unwrap())),
        ),
        &c,
    )
    .unwrap()
    .unwrap();
    assert!(b.0[0] <= 5 && b.1[0] >= 15 && b.0[1] <= 5 && b.1[1] >= 15);
}

/// The frame's limit on a coordinate in steps, as the index's boxes hold.
const FRAME: i64 = (1 << 40) - 1;

/// The whole frame, the box of a line without ends.
const WHOLE: ([i64; 2], [i64; 2]) = ([-FRAME, -FRAME], [FRAME, FRAME]);

fn box_of(shape: Shape, c: &Carriers) -> Option<([i64; 2], [i64; 2])> {
    bounds(&Entity::new(Attr(0), shape), c).unwrap()
}

/// The floor of `v` clamped to the frame, by the standard library: the oracle for a box corner.
fn floor_in_frame(v: f64) -> i64 {
    let f = FRAME as f64;
    let c = if v < -f {
        -f
    } else if v > f {
        f
    } else {
        v
    };
    c.floor() as i64
}

/// The box the index promises for a point read through its enclosure: the floors of the low
/// ends, and one more than the floors of the high ends, within the frame.
fn enclosed(point: &Pt2, c: &Carriers) -> ([i64; 2], [i64; 2]) {
    let Place::OneRoot(z) = point.place(c).unwrap() else {
        panic!("not a one-root point");
    };
    let [x, y] = z.enclosure();
    let up = |v: f64| (floor_in_frame(v) + 1).min(FRAME);
    (
        [floor_in_frame(x.lo()), floor_in_frame(y.lo())],
        [up(x.hi()), up(y.hi())],
    )
}

fn joined(a: ([i64; 2], [i64; 2]), b: ([i64; 2], [i64; 2])) -> ([i64; 2], [i64; 2]) {
    (
        [a.0[0].min(b.0[0]), a.0[1].min(b.0[1])],
        [a.1[0].max(b.1[0]), a.1[1].max(b.1[1])],
    )
}

fn ends(a: Pt2, b: Pt2) -> Shape {
    Shape::Line {
        carrier: Carrier::new(0, 1, 0).unwrap(),
        ends: Ends::Segment(a, b),
    }
}

#[test]
fn a_line_without_ends_and_a_ray_get_the_whole_frame() {
    let c = Carriers::new();
    let infinite = Shape::Line {
        carrier: Carrier::new(1, 2, 3).unwrap(),
        ends: Ends::Infinite,
    };
    assert_eq!(box_of(infinite, &c), Some(WHOLE));
    let ray = Shape::Line {
        carrier: Carrier::new(1, 0, 0).unwrap(),
        ends: Ends::Ray(Pt2::Grid(p(0, 0))),
    };
    assert_eq!(box_of(ray, &c), Some(WHOLE));
}

#[test]
fn a_crossing_end_is_boxed_by_its_enclosure_rounded_outward() {
    let mut c = Carriers::new();
    // 3x = 50 and 3y = -70 cross at (50/3, -70/3); x = 4 and y = -3 at a grid point.
    let a = c
        .push(Curve2::Line(Carrier::new(3, 0, 50).unwrap()))
        .unwrap();
    let b = c
        .push(Curve2::Line(Carrier::new(0, 3, -70).unwrap()))
        .unwrap();
    let e = c
        .push(Curve2::Line(Carrier::new(1, 0, 4).unwrap()))
        .unwrap();
    let f = c
        .push(Curve2::Line(Carrier::new(0, 1, -3).unwrap()))
        .unwrap();
    let thirds = Pt2::Crossing {
        a,
        b,
        root: Root::First,
    };
    let whole = Pt2::Crossing {
        a: e,
        b: f,
        root: Root::First,
    };
    // The oracle's own reading of the two enclosures.
    let t = enclosed(&thirds, &c);
    assert_eq!(t, ([16, -24], [17, -23]));
    let w = enclosed(&whole, &c);
    assert!(w.0[0] <= 4 && w.1[0] > 4 && w.0[1] <= -3 && w.1[1] > -3);
    assert_eq!(box_of(ends(thirds, whole), &c), Some(joined(t, w)));
    assert_eq!(box_of(ends(whole, whole), &c), Some(w));
    // A grid point as a crossing of a line with a vertical line at a negative whole number.
    let g = c
        .push(Curve2::Line(Carrier::new(1, 0, -9).unwrap()))
        .unwrap();
    let h = c
        .push(Curve2::Line(Carrier::new(1, 1, -2).unwrap()))
        .unwrap();
    let negative = Pt2::Crossing {
        a: g,
        b: h,
        root: Root::First,
    };
    let n = enclosed(&negative, &c);
    assert_eq!(box_of(ends(negative, negative), &c), Some(n));
}

#[test]
fn a_crossing_past_the_frame_is_clamped_to_its_edge() {
    let mut c = Carriers::new();
    // y = x / m and y = -1 + x / (m + 1) cross at x = -m (m + 1), about -2^42, past the frame;
    // y = 1 + x / (m + 1) crosses the first at x = m (m + 1), past it on the other side.
    let m = 1_i64 << 21;
    let a = c
        .push(Curve2::Line(Carrier::through(p(0, 0), p(m, 1)).unwrap()))
        .unwrap();
    let below_line = c
        .push(Curve2::Line(
            Carrier::through(p(0, -1), p(m + 1, 0)).unwrap(),
        ))
        .unwrap();
    let above_line = c
        .push(Curve2::Line(
            Carrier::through(p(0, 1), p(m + 1, 2)).unwrap(),
        ))
        .unwrap();
    let west = Pt2::Crossing {
        a,
        b: below_line,
        root: Root::First,
    };
    let east = Pt2::Crossing {
        a,
        b: above_line,
        root: Root::First,
    };
    let Place::OneRoot(z) = west.place(&c).unwrap() else {
        panic!("not a one-root point");
    };
    assert!(z.enclosure()[0].hi() < -(FRAME as f64));
    let Place::OneRoot(z) = east.place(&c).unwrap() else {
        panic!("not a one-root point");
    };
    assert!(z.enclosure()[0].lo() > FRAME as f64);
    // West of the frame: the low corner is the frame's edge, and the grid end holds the high one.
    let b = box_of(ends(west, Pt2::Grid(p(0, 0))), &c).unwrap();
    assert_eq!(b, joined(enclosed(&west, &c), ([0, 0], [0, 0])));
    assert_eq!(b.0[0], -FRAME);
    assert_eq!(b.1[0], 0);
    // East of it, both ends there: the box is pinned to the frame's edge.
    let b = box_of(ends(east, east), &c).unwrap();
    assert_eq!(b, enclosed(&east, &c));
    assert_eq!((b.0[0], b.1[0]), (FRAME, FRAME));
}

#[test]
fn a_rational_end_is_boxed_by_its_floor_and_ceiling() {
    let c = Carriers::new();
    // (7/2, -5/2) and (3, 4/3): floors (3, -3) and (3, 1), ceilings (4, -2) and (3, 2).
    let a = Pt2::Rational(Rational2::new(7, -5, 2).unwrap());
    let b = Pt2::Rational(Rational2::new(9, 4, 3).unwrap());
    assert_eq!(box_of(ends(a, b), &c), Some(([3, -3], [4, 2])));
}

#[test]
fn an_end_where_an_ellipse_meets_a_circle_gets_the_whole_frame() {
    let mut c = Carriers::new();
    let e = c
        .push(Curve2::Ellipse(
            Ellipse2::new(p(0, 0), [10, 0], 1, 2).unwrap(),
        ))
        .unwrap();
    let r = c
        .push(Curve2::Circle(Circle2::Grid(
            GridCircle::new(p(0, 0), 7).unwrap(),
        )))
        .unwrap();
    let root = Pt2::ConicRoot { a: e, b: r, k: 0 };
    assert!(matches!(root.place(&c).unwrap(), Place::Conic { .. }));
    assert_eq!(box_of(ends(root, Pt2::Grid(p(1, 1))), &c), Some(WHOLE));
}

#[test]
fn a_circle_about_a_rational_centre_is_boxed_about_the_nearest_grid_point() {
    let c = Carriers::new();
    // About (7/4, 0) through (5, 0): radius 13/4, squared 169/16, rounded up to 11, whose root
    // rounded up is 4, and one more for the centre off the grid; the centre rounds to (2, 0).
    let round = Round::new(p(5, 0), Rational2::new(7, 0, 4).unwrap());
    assert_eq!(
        box_of(Shape::Circle(Circle2::Round(round)), &c),
        Some(([-3, -5], [7, 5]))
    );
}

#[test]
fn a_splines_box_is_its_control_points_box() {
    let c = Carriers::new();
    let w = |n| Frac::whole(n).unwrap();
    // A straight spline standing upright has one x for every control point.
    let upright = Spline::new(
        1,
        vec![p(3, 1), p(3, 9)],
        None,
        vec![w(0), w(0), w(1), w(1)],
        None,
    )
    .unwrap();
    assert_eq!(box_of(Shape::Spline(upright), &c), Some(([3, 1], [3, 9])));
    let leaning = Spline::new(
        2,
        vec![p(0, 0), p(8, -2), p(4, 5)],
        None,
        vec![w(0), w(0), w(0), w(1), w(1), w(1)],
        None,
    )
    .unwrap();
    assert_eq!(box_of(Shape::Spline(leaning), &c), Some(([0, -2], [8, 5])));
}

#[test]
fn an_index_error_reads_as_its_cause() {
    let e = IndexError::Num(NumError::Degenerate);
    assert_eq!(e.to_string(), NumError::Degenerate.to_string());
    assert_ne!(e.to_string(), "");
}

#[test]
fn carriers_dropped_past_a_mark_are_gone_and_those_before_it_stay() {
    let mut c = Carriers::new();
    let at: Vec<_> = (1..=3)
        .map(|k| {
            c.push(Curve2::Line(Carrier::new(1, 0, k).unwrap()))
                .unwrap()
        })
        .collect();
    c.truncate(1);
    assert_eq!(c.len(), 1);
    assert!(c.get(at[0]).is_some());
    assert!(c.get(at[1]).is_none() && c.get(at[2]).is_none());
}

#[test]
fn a_circles_quadrants_and_a_polylines_vertices_and_middles_are_snapped() {
    let poly = Polyline::new(
        vec![p(100, 0), p(110, 0), p(110, 10)],
        vec![Frac::whole(0).unwrap(); 2],
        None,
        false,
    )
    .unwrap();
    let d = drawing(vec![
        Shape::Circle(Circle2::Grid(GridCircle::new(p(0, 0), 10).unwrap())),
        Shape::Polyline(poly),
    ]);
    let index = build(&d);
    let mut c = Carriers::new();
    let mut snap = |x, y, kind| {
        index
            .snap(p(x, y), Steps(3), &[kind], &mut c, &mut meter())
            .unwrap()
            .map(|f| (f.name, f.sub, f.pick))
    };
    // The north quadrant, pick 1.
    assert_eq!(snap(1, 9, SnapKind::Quadrant), Some((name(0), None, 1)));
    // The polyline's middle vertex, as its part 1.
    assert_eq!(snap(109, 1, SnapKind::End), Some((name(1), Some(1), 0)));
    // The middle of its second segment, (110, 5), as its part 1.
    assert_eq!(snap(109, 5, SnapKind::Mid), Some((name(1), Some(1), 0)));
}

#[test]
fn of_two_quadrants_at_one_distance_the_first_is_kept() {
    // From (4, 4), the east and north quadrants of a circle of radius 5, (5, 0) and (0, 5), are
    // both 17 away squared, in the same enclosure: the first made, pick 0, is kept.
    let d = drawing(vec![Shape::Circle(Circle2::Grid(
        GridCircle::new(p(0, 0), 5).unwrap(),
    ))]);
    let index = build(&d);
    let found = index
        .snap(
            p(4, 4),
            Steps(5),
            &[SnapKind::Quadrant],
            &mut Carriers::new(),
            &mut meter(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(found.pick, 0);
}

#[test]
fn enclosures_that_only_touch_are_a_tie_and_go_to_the_lower_name() {
    // At 2^26 steps the squared distances are near 2^52, where the enclosures are whole numbers a
    // few units wide: (2^26, 4) is 20 nearer than (2^26, 6), whose enclosure starts exactly where
    // the other's ends. Neither is wholly below the other, so the lower name wins.
    let x = 1_i64 << 26;
    let d = drawing(vec![Shape::Point(p(x, 6)), Shape::Point(p(x, 4))]);
    let index = build(&d);
    let mut c = Carriers::new();
    let alone = |y| {
        let one = drawing(vec![Shape::Point(p(x, y))]);
        build(&one)
            .snap(
                p(0, 0),
                Steps(2 * x as u64),
                &[SnapKind::Node],
                &mut Carriers::new(),
                &mut meter(),
            )
            .unwrap()
            .unwrap()
            .distance_squared
    };
    assert_eq!(alone(4)[1], alone(6)[0]);
    let found = index
        .snap(
            p(0, 0),
            Steps(2 * x as u64),
            &[SnapKind::Node],
            &mut c,
            &mut meter(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(found.name, name(0));
}

#[test]
fn a_snap_whose_enclosure_starts_at_the_apertures_edge_is_inside() {
    // (2^26, 3) from the origin: its squared distance's enclosure starts exactly where the
    // aperture of 2^26 steps, squared and enclosed, ends.
    let x = 1_i64 << 26;
    let d = drawing(vec![Shape::Point(p(x, 3))]);
    let found = build(&d)
        .snap(
            p(0, 0),
            Steps(x as u64),
            &[SnapKind::Node],
            &mut Carriers::new(),
            &mut meter(),
        )
        .unwrap()
        .unwrap();
    let limit = Interval::point(x as f64).unwrap().square().unwrap();
    assert_eq!(found.distance_squared[0], limit.hi());
}

#[test]
fn curves_whose_boxes_only_touch_are_still_crossed() {
    // Each pair touches on one side of its boxes, in both name orders, and crosses there.
    for (a, b, at) in [
        (segment((0, 0), (4, 0)), segment((4, -3), (4, 3)), (4, 0)),
        (segment((4, -3), (4, 3)), segment((0, 0), (4, 0)), (4, 0)),
        (segment((0, 0), (0, 4)), segment((-3, 4), (3, 4)), (0, 4)),
        (segment((-3, 4), (3, 4)), segment((0, 0), (0, 4)), (0, 4)),
    ] {
        let d = drawing(vec![a, b]);
        let index = build(&d);
        let mut c = Carriers::new();
        let found = index
            .snap(
                p(at.0 + 1, at.1 + 1),
                Steps(2),
                &[SnapKind::Intersection],
                &mut c,
                &mut meter(),
            )
            .unwrap();
        let found = found.expect("the crossing where the boxes touch");
        assert_eq!((found.name, found.with), (name(0), Some(name(1))));
        assert!(
            found
                .point
                .place(&c)
                .map(|place| match place {
                    Place::OneRoot(z) => z
                        .same(
                            &Crossing2::of_point(
                                &geom_num::carrier::Rational2::new(
                                    i128::from(at.0),
                                    i128::from(at.1),
                                    1
                                )
                                .unwrap()
                            )
                            .unwrap()
                        )
                        .unwrap(),
                    Place::Conic { .. } => false,
                })
                .unwrap()
        );
    }
}

#[test]
fn the_first_crossing_of_a_segment_and_a_circle_is_the_west_one() {
    let d = drawing(vec![
        segment((-10, 0), (10, 0)),
        Shape::Circle(Circle2::Grid(GridCircle::new(p(0, 0), 5).unwrap())),
    ]);
    let index = build(&d);
    let pick = |x| {
        index
            .snap(
                p(x, 1),
                Steps(2),
                &[SnapKind::Intersection],
                &mut Carriers::new(),
                &mut meter(),
            )
            .unwrap()
            .unwrap()
            .pick
    };
    assert_eq!((pick(-5), pick(5)), (0, 1));
}

fn below(rng: &mut SplitMix64, n: u64) -> i64 {
    rng.below(NonZeroU64::new(n).unwrap()) as i64
}

#[test]
fn on_random_drawings_the_tree_gives_the_scans_answer() {
    let seed = resolve("geom-2d snap index", Seed(0x198)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..4 {
        let mut shapes = Vec::new();
        for _ in 0..30 {
            let x = below(&mut rng, 400);
            let y = below(&mut rng, 400);
            shapes.push(match below(&mut rng, 4) {
                0 => Shape::Point(p(x, y)),
                1 => Shape::Circle(Circle2::Grid(
                    GridCircle::new(p(x, y), i128::from(1 + below(&mut rng, 30))).unwrap(),
                )),
                _ => {
                    let (dx, dy) = (below(&mut rng, 80) - 40, below(&mut rng, 80) - 40);
                    if dx == 0 && dy == 0 {
                        Shape::Point(p(x, y))
                    } else {
                        segment((x, y), (x + dx, y + dy))
                    }
                }
            });
        }
        let d = drawing(shapes);
        let index = build(&d);
        let kinds = [
            SnapKind::End,
            SnapKind::Start,
            SnapKind::Mid,
            SnapKind::Center,
            SnapKind::Node,
            SnapKind::Quadrant,
            SnapKind::Intersection,
        ];
        for _ in 0..12 {
            let cursor = p(below(&mut rng, 400), below(&mut rng, 400));
            let aperture = Steps(1 + below(&mut rng, 25) as u64);
            let tree = index
                .snap(cursor, aperture, &kinds, &mut Carriers::new(), &mut meter())
                .unwrap();
            let scan = index
                .scan(cursor, aperture, &kinds, &mut Carriers::new())
                .unwrap();
            let key = |f: &Option<geom_2d::index::Found>| {
                f.as_ref()
                    .map(|f| (f.kind, f.name, f.with, f.sub, f.pick, f.distance_squared))
            };
            assert_eq!(
                key(&tree),
                key(&scan),
                "cursor {cursor:?} aperture {aperture:?}"
            );
        }
    }
}
