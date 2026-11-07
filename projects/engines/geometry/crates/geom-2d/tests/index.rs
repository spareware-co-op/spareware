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
    clippy::many_single_char_names,
    reason = "tests, written as the definitions read; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::num::NonZeroU64;

use geom_2d::carrier::{Carriers, Circle2};
use geom_2d::entity::{Attr, Ends, Entity, Shape};
use geom_2d::index::{SnapIndex, bounds};
use geom_2d::point::{Place, Pt2};
use geom_2d::snap::{Drawing, SnapKind};
use geom_num::arc::Circle2 as GridCircle;
use geom_num::carrier::Carrier;
use geom_num::grid::{P2, Steps};
use geom_num::one_root::Crossing2;
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
