//! Tests of `geom_sketch::commit` (piece G225): the commit plan and exact incidences.
//!
//! The board's done-when: "The exact-relation graph, masters rounded once in local integer space,
//! integer carriers, depth-one points and rational-centre arcs, cycles broken with the broken
//! relation reported; T-junctions on sloping lines leave no slivers; no value rounded twice; a
//! 1,000-entity commit in 2 ms." The benchmark is `benches/commit.rs`.
//!
//! What this file proves, each on the committed values with `geom-num`'s exact predicates rather
//! than on how they were made: a rectangle commits to grid points and axis carriers; two grid points
//! that a horizontal line joins share one rounded row; a T-junction on a sloping wall is a point on
//! the wall's carrier, and the two pieces of the wall keep one carrier; the corners of a
//! parallelogram lie on both their lines; an arc's endpoints lie on its own circle and a tangent line
//! meets it at a right angle; a relation that the stored geometry cannot hold is marked bounded and
//! reported, the later one giving way; and the same sketch commits to the same bytes every time.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests, written as the definitions read; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_num::carrier::Carrier;
use geom_num::{GridStep, P3, Sign};
use geom_run::{Budget, WorkMeter};
use geom_sketch::commit::{Mark, Stored, Why, commit};
use geom_sketch::model::{
    Amount, Constraint, ConstraintKind, Entity, EntityKind, Extent, ItemId, Sketch, Standing,
};
use geom_sketch::plane::{Lattice, PlaneFrame, SketchPlane};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn sketch() -> Sketch {
    let lattice = Lattice::new(P3::new(0, 0, 0).unwrap(), [1, 0, 0], [0, 1, 0]).unwrap();
    Sketch::new(SketchPlane::new(PlaneFrame::Lattice(lattice), GridStep::Q0))
}

fn pid(key: u64) -> ItemId {
    ItemId::new(1, key)
}

fn lid(key: u64) -> ItemId {
    ItemId::new(2, key)
}

fn cid(key: u64) -> ItemId {
    ItemId::new(3, key)
}

fn point(key: u64, x: f64, y: f64) -> Entity {
    Entity {
        id: pid(key),
        kind: EntityKind::Point { x, y },
        construction: false,
    }
}

fn line(key: u64, a: u64, b: u64) -> Entity {
    Entity {
        id: lid(key),
        kind: EntityKind::Line {
            start: pid(a),
            end: pid(b),
            extent: Extent::Segment,
        },
        construction: false,
    }
}

fn rule(key: u64, kind: ConstraintKind) -> Constraint {
    Constraint {
        id: cid(key),
        kind,
        standing: Standing::Driving,
        priority: 0,
    }
}

fn build(entities: Vec<Entity>, constraints: Vec<Constraint>) -> Sketch {
    let mut s = sketch();
    for e in entities {
        s.add_entity(e).unwrap();
    }
    for c in constraints {
        s.add_constraint(c).unwrap();
    }
    s
}

fn grid(s: &Stored) -> (i64, i64) {
    match s {
        Stored::Grid(p) => (p.x.get(), p.y.get()),
        Stored::Meet(m) => panic!("expected a grid point, found {m:?}"),
    }
}

#[test]
fn a_rectangle_commits_to_grid_points_and_axis_carriers() {
    // Solved corners a little off the grid; the rules say which sides are level and plumb.
    let s = build(
        vec![
            point(1, 10.2, 20.4),
            point(2, 110.1, 19.9),
            point(3, 109.8, 80.2),
            point(4, 10.4, 79.6),
            line(1, 1, 2),
            line(2, 2, 3),
            line(3, 3, 4),
            line(4, 4, 1),
        ],
        vec![
            rule(1, ConstraintKind::Horizontal { line: lid(1) }),
            rule(2, ConstraintKind::Vertical { line: lid(2) }),
            rule(3, ConstraintKind::Horizontal { line: lid(3) }),
            rule(4, ConstraintKind::Vertical { line: lid(4) }),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    // The rows and columns are each rounded once, from the first point that names them.
    assert_eq!(grid(c.point(pid(1)).unwrap()), (10, 20));
    assert_eq!(grid(c.point(pid(2)).unwrap()), (110, 20));
    assert_eq!(grid(c.point(pid(3)).unwrap()), (110, 80));
    assert_eq!(grid(c.point(pid(4)).unwrap()), (10, 80));
    for k in 1..=4 {
        assert_eq!(c.mark(cid(k)), Some(Mark::Exact), "rule {k}");
    }
    assert_eq!(c.broken, []);
    // A level side is the carrier y = row; a plumb one, x = column.
    let ([a, b], _) = c.line(lid(1)).unwrap().coefficients();
    assert_eq!(a, 0);
    assert_ne!(b, 0);
    let ([a, b], _) = c.line(lid(2)).unwrap().coefficients();
    assert_ne!(a, 0);
    assert_eq!(b, 0);
}

#[test]
fn coincident_points_are_one_stored_point() {
    let s = build(
        vec![
            point(1, 5.4, 5.4),
            point(2, 5.2, 5.6),
            point(3, 40.0, 5.0),
            line(1, 1, 3),
            line(2, 2, 3),
        ],
        vec![rule(
            1,
            ConstraintKind::Coincident {
                a: pid(1),
                b: pid(2),
            },
        )],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(
        grid(c.point(pid(1)).unwrap()),
        grid(c.point(pid(2)).unwrap())
    );
    assert_eq!(c.mark(cid(1)), Some(Mark::Exact));
}

#[test]
fn a_t_junction_on_a_sloping_wall_is_on_the_wall_and_leaves_no_sliver() {
    // A wall rising 333 in 1000 and a cross wall that ends on it at T, 500.4 along.
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 1000.0, 333.0),
            point(3, 500.4, 166.7),
            point(4, 500.0, 900.0),
            line(1, 1, 2),
            line(2, 4, 3),
        ],
        vec![rule(
            1,
            ConstraintKind::OnLine {
                point: pid(3),
                line: lid(1),
            },
        )],
    );
    let c = commit(&s, &mut meter()).unwrap();
    // T is not a grid point: rounded to the grid it would leave a gap on the wall. It is the meeting
    // of the wall's carrier and the cross wall's, so it is on both exactly.
    let t = c.point(pid(3)).unwrap();
    assert!(matches!(t, Stored::Meet(_)), "{t:?}");
    for k in 1..=2 {
        assert_eq!(
            c.line(lid(k)).unwrap().side(&t.rational()).unwrap(),
            Sign::Zero,
            "line {k}"
        );
    }
    // The wall's ends are grid points on its carrier.
    assert_eq!(grid(c.point(pid(1)).unwrap()), (0, 0));
    assert_eq!(grid(c.point(pid(2)).unwrap()), (1000, 333));
    assert_eq!(c.mark(cid(1)), Some(Mark::Exact));
    assert!(c.broken.is_empty(), "{:?}", c.broken);
}

#[test]
fn a_sloping_wall_cut_in_two_keeps_one_carrier_and_a_point_on_both_pieces() {
    // The same wall as two pieces meeting at T, collinear, and the cross wall ending at T.
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 500.4, 166.7),
            point(3, 1000.0, 333.0),
            point(4, 500.0, 900.0),
            line(1, 1, 2),
            line(2, 2, 3),
            line(3, 4, 2),
        ],
        vec![rule(
            1,
            ConstraintKind::Collinear {
                a: lid(1),
                b: lid(2),
            },
        )],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert!(
        c.line(lid(1))
            .unwrap()
            .same(c.line(lid(2)).unwrap())
            .unwrap()
    );
    let tee = c.point(pid(2)).unwrap().rational();
    for k in 1..=3 {
        assert_eq!(
            c.line(lid(k)).unwrap().side(&tee).unwrap(),
            Sign::Zero,
            "line {k}"
        );
    }
    // Every end of both pieces lies on their carrier.
    for p in [1, 2, 3] {
        let end = c.point(pid(p)).unwrap().rational();
        assert_eq!(
            c.line(lid(1)).unwrap().side(&end).unwrap(),
            Sign::Zero,
            "end {p}"
        );
    }
    assert_eq!(c.mark(cid(1)), Some(Mark::Exact));
    assert!(c.broken.is_empty(), "{:?}", c.broken);
}

#[test]
fn the_corners_of_a_parallelogram_lie_on_both_their_lines() {
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 400.3, 100.2),
            point(3, 530.1, 310.4),
            point(4, 129.9, 210.1),
            line(1, 1, 2),
            line(2, 2, 3),
            line(3, 4, 3),
            line(4, 1, 4),
        ],
        vec![
            rule(
                1,
                ConstraintKind::Parallel {
                    a: lid(1),
                    b: lid(3),
                },
            ),
            rule(
                2,
                ConstraintKind::Parallel {
                    a: lid(2),
                    b: lid(4),
                },
            ),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(c.mark(cid(1)), Some(Mark::Exact));
    assert_eq!(c.mark(cid(2)), Some(Mark::Exact));
    assert!(c.broken.is_empty(), "{:?}", c.broken);
    for (l, ends) in [(1, [1, 2]), (2, [2, 3]), (3, [4, 3]), (4, [1, 4])] {
        for p in ends {
            let at = c.point(pid(p)).unwrap().rational();
            assert_eq!(
                c.line(lid(l)).unwrap().side(&at).unwrap(),
                Sign::Zero,
                "point {p} on line {l}"
            );
        }
    }
}

#[test]
fn perpendicular_lines_commit_to_perpendicular_carriers() {
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 300.0, 100.0),
            point(3, 270.2, 190.3),
            line(1, 1, 2),
            line(2, 2, 3),
        ],
        vec![rule(
            1,
            ConstraintKind::Perpendicular {
                a: lid(1),
                b: lid(2),
            },
        )],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(c.mark(cid(1)), Some(Mark::Exact));
    let ([a1, b1], _) = c.line(lid(1)).unwrap().coefficients();
    let ([a2, b2], _) = c.line(lid(2)).unwrap().coefficients();
    assert_eq!(a1 * a2 + b1 * b2, 0);
    // The shared corner is on both lines.
    let corner = c.point(pid(2)).unwrap().rational();
    assert_eq!(c.line(lid(1)).unwrap().side(&corner).unwrap(), Sign::Zero);
    assert_eq!(c.line(lid(2)).unwrap().side(&corner).unwrap(), Sign::Zero);
}

#[test]
fn a_circle_has_a_grid_centre_and_a_whole_radius_and_an_arc_lies_on_its_own_circle() {
    let circle = Entity {
        id: ItemId::new(4, 1),
        kind: EntityKind::Circle {
            centre: pid(1),
            radius: 50.4,
        },
        construction: false,
    };
    let arc = Entity {
        id: ItemId::new(4, 2),
        kind: EntityKind::Arc {
            centre: pid(4),
            radius: 100.0,
            start: pid(2),
            end: pid(3),
        },
        construction: false,
    };
    let s = build(
        vec![
            point(1, 200.0, 200.0),
            point(2, 300.0, 200.0),
            point(3, 200.0, 300.0),
            point(4, 200.0, 200.0),
            circle,
            arc,
        ],
        vec![
            rule(
                1,
                ConstraintKind::Radius {
                    circle: ItemId::new(4, 1),
                    radius: Amount::Value(50.0),
                },
            ),
            rule(
                2,
                ConstraintKind::Concentric {
                    a: ItemId::new(4, 1),
                    b: ItemId::new(4, 2),
                },
            ),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    let (_, circ) = &c.circles[0];
    assert_eq!(circ.radius(), 50);
    assert_eq!((circ.centre().x.get(), circ.centre().y.get()), (200, 200));
    // The arc's endpoints are on its circle exactly (`Arc2::new` refuses otherwise), and its centre
    // is the circle's centre: the centres coincide.
    let (_, a) = &c.arcs[0];
    assert!(a.holds(&a.round().centre()).is_ok());
    assert_eq!(a.round().side_of_grid(a.end()).unwrap(), Sign::Zero);
    // The radius is a whole 50 and the stated one is 50: exact. The centres are shared: exact.
    assert_eq!(c.mark(cid(1)), Some(Mark::Exact));
    assert_eq!(c.mark(cid(2)), Some(Mark::Exact));
}

#[test]
fn a_line_tangent_at_an_arcs_end_meets_it_at_a_right_angle() {
    // An arc from (100, 0) to (0, 100) about the origin, and a line leaving (100, 0) straight up.
    let arc = Entity {
        id: ItemId::new(4, 1),
        kind: EntityKind::Arc {
            centre: pid(1),
            radius: 100.0,
            start: pid(2),
            end: pid(3),
        },
        construction: false,
    };
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 100.0, 0.0),
            point(3, 0.0, 100.0),
            point(4, 100.0, 80.0),
            line(1, 2, 4),
            arc,
        ],
        vec![rule(
            1,
            ConstraintKind::TangentAtEnd {
                line: lid(1),
                arc: ItemId::new(4, 1),
                point: pid(2),
            },
        )],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(c.mark(cid(1)), Some(Mark::Exact), "{:?}", c.broken);
}

#[test]
fn a_relation_the_stored_geometry_cannot_hold_is_bounded_and_reported() {
    // A level line whose ends are fixed to different rows: the second Fix of the pair cannot hold
    // with the first, and the horizontal cannot hold either. The earlier constraints win.
    let s = build(
        vec![point(1, 0.0, 10.0), point(2, 100.0, 40.0), line(1, 1, 2)],
        vec![
            rule(
                1,
                ConstraintKind::Fix {
                    entity: pid(1),
                    slot: 1,
                    value: 10.0,
                },
            ),
            rule(
                2,
                ConstraintKind::Fix {
                    entity: pid(2),
                    slot: 1,
                    value: 40.0,
                },
            ),
            rule(3, ConstraintKind::Horizontal { line: lid(1) }),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(c.mark(cid(1)), Some(Mark::Exact));
    assert_eq!(c.mark(cid(2)), Some(Mark::Exact));
    assert_eq!(c.mark(cid(3)), Some(Mark::Bounded));
    assert_eq!(c.broken.len(), 1, "{:?}", c.broken);
    assert_eq!(c.broken[0].constraint, cid(3));
    assert_eq!(c.broken[0].why, Why::Disagrees);
}

#[test]
fn a_dimension_that_is_not_exact_is_bounded_without_being_broken() {
    let s = build(
        vec![point(1, 0.0, 0.0), point(2, 30.0, 40.0), line(1, 1, 2)],
        vec![rule(
            1,
            ConstraintKind::Length {
                line: lid(1),
                length: Amount::Value(50.0),
            },
        )],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(c.mark(cid(1)), Some(Mark::Bounded));
    assert_eq!(c.broken, []);
}

#[test]
fn the_same_sketch_commits_to_the_same_values() {
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 400.3, 100.2),
            point(3, 530.1, 310.4),
            point(4, 129.9, 210.1),
            line(1, 1, 2),
            line(2, 2, 3),
            line(3, 4, 3),
            line(4, 1, 4),
        ],
        vec![
            rule(
                1,
                ConstraintKind::Parallel {
                    a: lid(1),
                    b: lid(3),
                },
            ),
            rule(
                2,
                ConstraintKind::Parallel {
                    a: lid(2),
                    b: lid(4),
                },
            ),
        ],
    );
    let first = format!("{:?}", commit(&s, &mut meter()).unwrap());
    for _ in 0..3 {
        assert_eq!(
            format!("{:?}", commit(&s.clone(), &mut meter()).unwrap()),
            first
        );
    }
}

#[test]
fn a_degenerate_line_and_a_small_budget_are_errors() {
    let s = build(
        vec![point(1, 3.0, 3.0), point(2, 3.2, 3.1), line(1, 1, 2)],
        vec![],
    );
    assert!(commit(&s, &mut meter()).is_err());
    let ok = build(
        vec![point(1, 0.0, 0.0), point(2, 30.0, 40.0), line(1, 1, 2)],
        vec![],
    );
    let mut none = WorkMeter::new(Budget::of_work(1));
    assert!(commit(&ok, &mut none).is_err());
    assert!(commit(&ok, &mut meter()).is_ok());
}

#[test]
fn a_carrier_through_two_stored_points_is_the_line_between_them() {
    let s = build(
        vec![point(1, 0.0, 0.0), point(2, 30.0, 40.0), line(1, 1, 2)],
        vec![],
    );
    let c = commit(&s, &mut meter()).unwrap();
    let want = Carrier::new(-4, 3, 0).unwrap();
    assert!(c.line(lid(1)).unwrap().same(&want).unwrap());
}

#[test]
fn points_mirrored_about_a_level_or_plumb_line_are_mirrored_exactly() {
    // A level line at row 100 and two points that must be mirrors about it; and a plumb one at
    // column 300 with two more. The partner's mirrored coordinate is computed, not rounded again.
    let s = build(
        vec![
            point(1, 0.0, 100.3),
            point(2, 500.0, 100.4),
            point(3, 100.2, 40.4),
            point(4, 100.1, 159.7),
            line(1, 1, 2),
            point(5, 300.2, 0.0),
            point(6, 299.8, 500.0),
            point(7, 220.4, 80.0),
            point(8, 379.7, 80.2),
            line(2, 5, 6),
        ],
        vec![
            rule(1, ConstraintKind::Horizontal { line: lid(1) }),
            rule(2, ConstraintKind::Vertical { line: lid(2) }),
            rule(
                3,
                ConstraintKind::Symmetric {
                    a: pid(3),
                    b: pid(4),
                    line: lid(1),
                },
            ),
            rule(
                4,
                ConstraintKind::Symmetric {
                    a: pid(7),
                    b: pid(8),
                    line: lid(2),
                },
            ),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(c.mark(cid(3)), Some(Mark::Exact), "{:?}", c.broken);
    assert_eq!(c.mark(cid(4)), Some(Mark::Exact), "{:?}", c.broken);
    let (a, b) = (
        grid(c.point(pid(3)).unwrap()),
        grid(c.point(pid(4)).unwrap()),
    );
    assert_eq!((a.0, a.1), (100, 40));
    assert_eq!((b.0, b.1), (100, 160));
    let (a, b) = (
        grid(c.point(pid(7)).unwrap()),
        grid(c.point(pid(8)).unwrap()),
    );
    assert_eq!((a.0, a.1), (220, 80));
    assert_eq!((b.0, b.1), (380, 80));
    assert_eq!(c.broken, []);
}

#[test]
fn a_point_on_a_circle_is_exact_at_its_defining_point_and_bounded_elsewhere() {
    let circle = Entity {
        id: ItemId::new(4, 1),
        kind: EntityKind::Circle {
            centre: pid(1),
            radius: 50.4,
        },
        construction: false,
    };
    let s = build(
        vec![
            point(1, 200.0, 200.0),
            point(2, 230.2, 240.1),
            point(3, 250.1, 200.2),
            point(4, 240.0, 210.0),
            circle,
        ],
        vec![
            rule(
                1,
                ConstraintKind::OnCircle {
                    point: pid(2),
                    circle: ItemId::new(4, 1),
                },
            ),
            rule(
                2,
                ConstraintKind::OnCircle {
                    point: pid(3),
                    circle: ItemId::new(4, 1),
                },
            ),
            rule(
                3,
                ConstraintKind::OnCircle {
                    point: pid(4),
                    circle: ItemId::new(4, 1),
                },
            ),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    // The circle is stored by its defining point (230, 240): radius squared 30^2 + 40^2.
    assert_eq!(c.circles.len(), 0);
    assert_eq!(c.rounds.len(), 1);
    assert_eq!(c.mark(cid(1)), Some(Mark::Exact));
    // (250, 200) is 50 away, exactly on it by luck; (240, 210) is not, and is bounded, not broken.
    assert_eq!(c.mark(cid(2)), Some(Mark::Exact));
    assert_eq!(c.mark(cid(3)), Some(Mark::Bounded));
    assert_eq!(c.broken, []);
}
