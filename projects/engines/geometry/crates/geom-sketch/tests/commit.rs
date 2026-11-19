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
//! Beyond those, each stored carrier, meeting, circle and mark is pinned to the value the rules
//! give: class normals turned by quarter turns, the normals level and plumb lines set, a first line
//! that rounds to one point, mirrored points, shared corners and centres, tangent arcs, relations
//! whose fixed ends miss their carriers, the grid's edge, and the work a commit charges.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests, written as the definitions read; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_num::arc::Circle2;
use geom_num::carrier::{Carrier, Rational2};
use geom_num::{GridStep, NumError, P2, P3, Sign};
use geom_run::{Budget, WorkMeter};
use geom_sketch::commit::{Broken, Committed, Mark, Stored, Why, commit};
use geom_sketch::model::{
    Amount, Constraint, ConstraintKind, Entity, EntityKind, Extent, ItemId, Sketch, SketchError,
    Standing,
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

/// The circle or arc `key`'s id.
fn kid(key: u64) -> ItemId {
    ItemId::new(4, key)
}

fn round(key: u64, centre: u64, radius: f64) -> Entity {
    Entity {
        id: kid(key),
        kind: EntityKind::Circle {
            centre: pid(centre),
            radius,
        },
        construction: false,
    }
}

fn arc(key: u64, centre: u64, radius: f64, start: u64, end: u64) -> Entity {
    Entity {
        id: kid(key),
        kind: EntityKind::Arc {
            centre: pid(centre),
            radius,
            start: pid(start),
            end: pid(end),
        },
        construction: false,
    }
}

fn fix(key: u64, point: u64, slot: u8, value: f64) -> Constraint {
    rule(
        key,
        ConstraintKind::Fix {
            entity: pid(point),
            slot,
            value,
        },
    )
}

fn carrier(a: i128, b: i128, c: i128) -> Carrier {
    Carrier::new(a, b, c).unwrap()
}

/// The point (x / w, y / w).
fn at(x: i128, y: i128, w: i128) -> Rational2 {
    Rational2::new(x, y, w).unwrap()
}

/// Asserts that the line `key` is stored as exactly the carrier a x + b y = c, coefficients and all.
fn assert_line(c: &Committed, key: u64, a: i128, b: i128, offset: i128) {
    assert_eq!(c.line(lid(key)), Some(&carrier(a, b, offset)), "line {key}");
}

/// Asserts that the point `key` is stored as the meeting of carriers at (x / w, y / w).
fn assert_meet(c: &Committed, key: u64, x: i128, y: i128, w: i128) {
    match c.point(pid(key)).unwrap() {
        Stored::Meet(m) => assert!(m.same(&at(x, y, w)).unwrap(), "point {key}: {m:?}"),
        Stored::Grid(p) => panic!("point {key}: expected a meeting, found {p:?}"),
    }
}

/// Asserts that every constraint `1..=count` is marked exact and nothing gave way.
fn assert_all_exact(c: &Committed, count: u64) {
    for k in 1..=count {
        assert_eq!(
            c.mark(cid(k)),
            Some(Mark::Exact),
            "rule {k}: {:?}",
            c.broken
        );
    }
    assert_eq!(c.broken, []);
}

fn disagrees(keys: &[u64]) -> Vec<Broken> {
    keys.iter()
        .map(|&k| Broken {
            constraint: cid(k),
            why: Why::Disagrees,
        })
        .collect()
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

#[test]
fn a_commit_charges_its_entities_four_times_its_constraints_each_points_lines_and_each_check() {
    // Three entities and one constraint: 3 + 4; each of the two points has one line through it: 2;
    // the one constraint is checked: 1. Ten units in all, and not one more.
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
    assert!(commit(&s, &mut WorkMeter::new(Budget::of_work(10))).is_ok());
    assert!(commit(&s, &mut WorkMeter::new(Budget::of_work(9))).is_err());
}

#[test]
fn a_radius_of_the_largest_grid_coordinate_commits_and_one_past_the_grid_is_refused() {
    let largest = build(
        vec![point(1, 0.0, 0.0), round(1, 1, 1_099_511_627_775.0)],
        vec![],
    );
    let c = commit(&largest, &mut meter()).unwrap();
    assert_eq!(c.circles[0].1.radius(), 1_099_511_627_775);
    // Past 2^40 - 1 steps the radius is not a grid value, though a circle could hold it.
    let past = build(
        vec![point(1, 0.0, 0.0), round(1, 1, 1_500_000_000_000.0)],
        vec![],
    );
    assert!(
        matches!(
            commit(&past, &mut meter()),
            Err(SketchError::Numeric(NumError::OutOfRange { .. }))
        ),
        "{:?}",
        commit(&past, &mut meter())
    );
}

#[test]
fn a_carrier_through_two_grid_points_is_stored_in_lowest_terms() {
    // From (10, 0) to (40, 40): a = 0 - 40, b = 40 - 10, c = -40 * 10, all divided by 10.
    let s = build(
        vec![point(1, 10.0, 0.0), point(2, 40.0, 40.0), line(1, 1, 2)],
        vec![],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_line(&c, 1, -4, 3, -40);
}

#[test]
fn a_chain_of_distances_rounds_its_first_point_once_and_places_the_rest_from_it() {
    // x2 = x1 + 10 and x1 = x3 + 20: one coordinate group, rounded once from its first point's
    // solved 0.4, so the second point is at 10 although its own solved 10.6 would round to 11.
    let s = build(
        vec![
            point(1, 0.4, 0.0),
            point(2, 10.6, 0.0),
            point(3, -20.3, 0.0),
        ],
        vec![
            rule(
                1,
                ConstraintKind::Across {
                    a: pid(1),
                    b: pid(2),
                    distance: Amount::Value(10.0),
                },
            ),
            rule(
                2,
                ConstraintKind::Across {
                    a: pid(3),
                    b: pid(1),
                    distance: Amount::Value(20.0),
                },
            ),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(grid(c.point(pid(1)).unwrap()), (0, 0));
    assert_eq!(grid(c.point(pid(2)).unwrap()), (10, 0));
    assert_eq!(grid(c.point(pid(3)).unwrap()), (-20, 0));
    assert_all_exact(&c, 2);
}

#[test]
fn a_distance_too_large_to_be_a_whole_figure_gives_way_instead_of_failing() {
    // 10^15 steps is past the figures the commit takes as whole; the relation is not built, and it
    // is reported as not held.
    let s = build(
        vec![point(1, 0.0, 0.0), point(2, 10.0, 0.0)],
        vec![rule(
            1,
            ConstraintKind::Across {
                a: pid(1),
                b: pid(2),
                distance: Amount::Value(1.0e15),
            },
        )],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(grid(c.point(pid(2)).unwrap()), (10, 0));
    assert_eq!(c.mark(cid(1)), Some(Mark::Bounded));
    assert_eq!(c.broken, disagrees(&[1]));
}

#[test]
fn lines_made_parallel_and_perpendicular_share_one_normal_turned_by_each_ones_quarter_turns() {
    // Two classes of lines whose rounded ends are not quite parallel or square: each line takes the
    // class's normal, from its first line, turned once for each quarter turn between them, through
    // its first end, and its other end is moved onto it. The second class chains three quarter
    // turns: lines 5 and 7 are square to line 4, and line 6 parallel to it.
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 4.0, 1.0),
            point(3, 10.0, 10.0),
            point(4, 18.0, 13.0),
            point(5, 30.0, 0.0),
            point(6, 31.0, -3.0),
            point(7, 0.0, 100.0),
            point(8, 4.0, 101.0),
            point(9, 50.0, 100.0),
            point(10, 51.0, 97.0),
            point(11, 100.0, 100.0),
            point(12, 108.0, 103.0),
            point(13, 150.0, 100.0),
            point(14, 151.0, 97.0),
            line(1, 1, 2),
            line(2, 3, 4),
            line(3, 5, 6),
            line(4, 7, 8),
            line(5, 9, 10),
            line(6, 11, 12),
            line(7, 13, 14),
        ],
        vec![
            rule(
                1,
                ConstraintKind::Perpendicular {
                    a: lid(2),
                    b: lid(3),
                },
            ),
            rule(
                2,
                ConstraintKind::Parallel {
                    a: lid(1),
                    b: lid(2),
                },
            ),
            rule(
                3,
                ConstraintKind::Perpendicular {
                    a: lid(5),
                    b: lid(6),
                },
            ),
            rule(
                4,
                ConstraintKind::Perpendicular {
                    a: lid(6),
                    b: lid(7),
                },
            ),
            rule(
                5,
                ConstraintKind::Perpendicular {
                    a: lid(4),
                    b: lid(5),
                },
            ),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    // The first class: line 1 from (0, 0) to (4, 1) gives the normal (-1, 4).
    assert_line(&c, 1, -1, 4, 0);
    assert_line(&c, 2, -1, 4, 30);
    assert_line(&c, 3, -4, -1, -120);
    assert_eq!(grid(c.point(pid(2)).unwrap()), (4, 1));
    assert_meet(&c, 4, 18, 12, 1);
    assert_meet(&c, 6, 123, -12, 4);
    // The second class: line 4 from (0, 100) to (4, 101), the same normal.
    assert_line(&c, 4, -1, 4, 400);
    assert_line(&c, 5, -4, -1, -300);
    assert_line(&c, 6, -1, 4, 300);
    assert_line(&c, 7, -4, -1, -700);
    assert_meet(&c, 10, 203, 388, 4);
    assert_meet(&c, 12, 108, 102, 1);
    assert_meet(&c, 14, 603, 388, 4);
    assert_all_exact(&c, 5);
}

#[test]
fn a_class_whose_first_line_rounds_to_one_point_takes_its_normal_from_the_solved_direction() {
    // Line 1 runs from (10, 10) to (10.375, 10.125): both ends round to (10, 10). Its solved
    // direction (0.375, 0.125), the longer part scaled to 2^40 and each rounded once, is the normal
    // (-2^40 / 3 rounded, 2^40) = (-366,503,875,925, 1,099,511,627,776), already in lowest terms.
    let (a, b) = (-366_503_875_925_i128, 1_099_511_627_776_i128);
    let s = build(
        vec![
            point(1, 10.0, 10.0),
            point(2, 10.375, 10.125),
            point(3, 0.0, 50.0),
            point(4, 300.2, 150.3),
            line(1, 1, 2),
            line(2, 3, 4),
        ],
        vec![rule(
            1,
            ConstraintKind::Parallel {
                a: lid(1),
                b: lid(2),
            },
        )],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_line(&c, 1, a, b, 10 * a + 10 * b);
    assert_line(&c, 2, a, b, 50 * b);
    assert_eq!(grid(c.point(pid(2)).unwrap()), (10, 10));
    // (300, 150) misses line 2 by 100 / b; it slides along x = 300 onto it.
    let Stored::Meet(m) = *c.point(pid(4)).unwrap() else {
        panic!("{:?}", c.point(pid(4)));
    };
    assert_eq!(carrier(a, b, 50 * b).side(&m).unwrap(), Sign::Zero);
    assert_eq!(carrier(1, 0, 300).side(&m).unwrap(), Sign::Zero);
    assert_all_exact(&c, 1);
}

#[test]
fn a_first_line_whose_solved_ends_are_too_far_apart_to_measure_is_not_finite() {
    // Both ends of line 1 coincide with (0, 0), but were solved at x = 1e308 and x = -1e308: the
    // direction between them overflows, and the commit says so.
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 1.0e308, 0.0),
            point(3, -1.0e308, 0.0),
            point(4, 0.0, 10.0),
            point(5, 10.0, 20.0),
            line(1, 2, 3),
            line(2, 4, 5),
        ],
        vec![
            rule(
                1,
                ConstraintKind::Coincident {
                    a: pid(1),
                    b: pid(2),
                },
            ),
            rule(
                2,
                ConstraintKind::Coincident {
                    a: pid(1),
                    b: pid(3),
                },
            ),
            rule(
                3,
                ConstraintKind::Parallel {
                    a: lid(1),
                    b: lid(2),
                },
            ),
        ],
    );
    assert_eq!(commit(&s, &mut meter()).err(), Some(SketchError::NotFinite));
}

#[test]
fn whole_quarter_turns_and_headings_commit_to_square_and_level_carriers() {
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 4.0, 1.0),
            point(3, 20.0, 0.0),
            point(4, 21.0, -3.0),
            point(5, 0.0, 50.0),
            point(6, 4.0, 51.0),
            point(7, 20.0, 50.0),
            point(8, 28.0, 53.0),
            point(9, 5.0, 100.0),
            point(10, 5.6, 110.2),
            point(11, 50.0, 200.0),
            point(12, 0.0, 200.6),
            point(13, 300.0, 0.0),
            point(14, 300.6, 40.0),
            line(1, 1, 2),
            line(2, 3, 4),
            line(3, 5, 6),
            line(4, 7, 8),
            line(5, 9, 10),
            line(6, 11, 12),
            line(7, 13, 14),
        ],
        vec![
            // Angles are in half turns: 0.5 is a quarter turn, 1 a half turn.
            rule(
                1,
                ConstraintKind::Turned {
                    a: lid(1),
                    b: lid(2),
                    angle: Amount::Value(0.5),
                },
            ),
            rule(
                2,
                ConstraintKind::Turned {
                    a: lid(3),
                    b: lid(4),
                    angle: Amount::Value(1.0),
                },
            ),
            rule(
                3,
                ConstraintKind::Heading {
                    line: lid(5),
                    angle: Amount::Value(0.5),
                },
            ),
            rule(
                4,
                ConstraintKind::Heading {
                    line: lid(6),
                    angle: Amount::Value(1.0),
                },
            ),
            rule(
                5,
                ConstraintKind::Heading {
                    line: lid(7),
                    angle: Amount::Value(-0.5),
                },
            ),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    // A quarter turn: line 2 square to line 1, through (20, 0); its far end slides onto it.
    assert_line(&c, 1, -1, 4, 0);
    assert_line(&c, 2, -4, -1, -80);
    assert_meet(&c, 4, 83, -12, 4);
    // A half turn: line 4 parallel to line 3.
    assert_line(&c, 3, -1, 4, 200);
    assert_line(&c, 4, -1, 4, 180);
    assert_meet(&c, 8, 28, 52, 1);
    // Headings of a quarter turn either way are plumb, a half turn level; each carrier is the
    // axis's own (1, 0) or (0, 1), whichever way the line was drawn, and the far end shares the
    // first end's column or row.
    assert_line(&c, 5, 1, 0, 5);
    assert_eq!(grid(c.point(pid(10)).unwrap()), (5, 110));
    assert_line(&c, 6, 0, 1, 200);
    assert_eq!(grid(c.point(pid(12)).unwrap()), (0, 200));
    assert_line(&c, 7, 1, 0, 300);
    assert_eq!(grid(c.point(pid(14)).unwrap()), (300, 40));
    assert_all_exact(&c, 5);
}

#[test]
fn a_level_or_plumb_line_in_a_class_sets_the_normal_of_every_line_square_to_it() {
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 0.6, 50.0),
            point(3, 10.0, 60.0),
            point(4, 40.0, 60.6),
            point(5, 100.0, 0.0),
            point(6, 150.0, 0.6),
            point(7, 200.0, 10.0),
            point(8, 200.6, 40.0),
            line(1, 1, 2),
            line(2, 3, 4),
            line(3, 5, 6),
            line(4, 7, 8),
        ],
        vec![
            rule(
                1,
                ConstraintKind::Perpendicular {
                    a: lid(1),
                    b: lid(2),
                },
            ),
            rule(2, ConstraintKind::Horizontal { line: lid(2) }),
            rule(
                3,
                ConstraintKind::Perpendicular {
                    a: lid(3),
                    b: lid(4),
                },
            ),
            rule(4, ConstraintKind::Vertical { line: lid(4) }),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    // Square to a level line: plumb, through (0, 0); its far end (1, 50) slides onto x = 0.
    assert_line(&c, 1, 1, 0, 0);
    assert_line(&c, 2, 0, 1, 60);
    assert_meet(&c, 2, 0, 50, 1);
    assert_eq!(grid(c.point(pid(4)).unwrap()), (40, 60));
    // Square to a plumb line: level, through (100, 0); (150, 1) slides onto y = 0.
    assert_line(&c, 3, 0, -1, 0);
    assert_line(&c, 4, 1, 0, 200);
    assert_meet(&c, 6, 150, 0, 1);
    assert_eq!(grid(c.point(pid(8)).unwrap()), (200, 40));
    assert_all_exact(&c, 4);
}

#[test]
fn circles_keep_their_shared_centres_their_whole_radii_and_their_defining_points() {
    let s = build(
        vec![
            point(1, 200.4, 200.4),
            point(2, 200.6, 199.6),
            point(3, 230.2, 240.3),
            point(4, 230.0, 200.2),
            point(5, 400.0, 0.0),
            point(6, 420.4, 15.2),
            point(7, 600.0, 0.0),
            round(1, 1, 50.4),
            round(2, 2, 30.2),
            round(3, 5, 25.0),
            round(4, 7, 49.6),
        ],
        vec![
            rule(
                1,
                ConstraintKind::Concentric {
                    a: kid(1),
                    b: kid(2),
                },
            ),
            rule(
                2,
                ConstraintKind::Radius {
                    circle: kid(1),
                    radius: Amount::Value(50.0),
                },
            ),
            rule(
                3,
                ConstraintKind::Diameter {
                    circle: kid(2),
                    diameter: Amount::Value(60.0),
                },
            ),
            rule(
                4,
                ConstraintKind::OnCircle {
                    point: pid(3),
                    circle: kid(1),
                },
            ),
            rule(
                5,
                ConstraintKind::OnCircle {
                    point: pid(4),
                    circle: kid(2),
                },
            ),
            rule(
                6,
                ConstraintKind::OnCircle {
                    point: pid(6),
                    circle: kid(3),
                },
            ),
            rule(
                7,
                ConstraintKind::EqualRadius {
                    a: kid(1),
                    b: kid(4),
                },
            ),
            rule(
                8,
                ConstraintKind::EqualRadius {
                    a: kid(1),
                    b: kid(2),
                },
            ),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    // The concentric pair shares the first centre's rounding, (200, 200), though the second was
    // solved nearer (201, 200). A stated whole radius or diameter keeps the circle a circle even
    // with a point on it; the circle with only a point on it is stored by that point.
    let centre = |x: i64, y: i64| P2::new(x, y).unwrap();
    assert_eq!(
        c.circles,
        [
            (kid(1), Circle2::new(centre(200, 200), 50).unwrap()),
            (kid(2), Circle2::new(centre(200, 200), 30).unwrap()),
            (kid(4), Circle2::new(centre(600, 0), 50).unwrap()),
        ]
    );
    assert_eq!(c.rounds.len(), 1);
    assert_eq!(c.rounds[0].0, kid(3));
    assert_eq!(c.rounds[0].1.on(), centre(420, 15));
    assert!(c.rounds[0].1.centre().same(&at(400, 0, 1)).unwrap());
    // Radius 50 and diameter 60 hold on the stored radii; (230, 240) is on the circle of radius
    // 50 and (230, 200) on the one of radius 30; circles 1 and 4 both round to 50, circles 1 and 2
    // do not match.
    for k in 1..=7 {
        assert_eq!(c.mark(cid(k)), Some(Mark::Exact), "rule {k}");
    }
    assert_eq!(c.mark(cid(8)), Some(Mark::Bounded));
    assert_eq!(c.broken, disagrees(&[8]));
}

#[test]
fn an_arc_centre_is_on_the_bisector_at_the_solved_centres_row_and_a_point_is_on_its_own_arc() {
    // The bisector of (400, 0) and (300, 100) runs at 45 degrees; the centre, solved at (302.4,
    // 0.4), is taken where it crosses the row y = 0, so it is (300, 0).
    let s = build(
        vec![
            point(1, 302.4, 0.4),
            point(2, 400.0, 0.0),
            point(3, 300.0, 100.0),
            point(4, 600.0, 0.0),
            point(5, 700.0, 0.0),
            point(6, 600.0, 100.0),
            point(7, 680.0, 60.0),
            arc(1, 1, 100.0, 2, 3),
            arc(2, 4, 100.0, 5, 6),
        ],
        vec![rule(
            1,
            ConstraintKind::OnCircle {
                point: pid(7),
                circle: kid(2),
            },
        )],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert!(c.arcs[0].1.round().centre().same(&at(300, 0, 1)).unwrap());
    assert!(c.arcs[1].1.round().centre().same(&at(600, 0, 1)).unwrap());
    // (680, 60) is 100 from (600, 0), on the second arc's circle.
    assert_all_exact(&c, 1);
}

#[test]
fn a_line_ending_at_a_circles_centre_or_an_arcs_end_passes_through_that_point() {
    // A circle's centre and an arc's ends stay where they were rounded: a line in a class that ends
    // at one passes through it, and its other end slides onto it instead.
    let s = build(
        vec![
            point(1, 100.0, 0.0),
            point(2, 104.0, 1.0),
            point(3, 0.0, 0.0),
            point(4, 10.0, 10.0),
            point(5, 500.0, 0.0),
            point(6, 600.0, 0.0),
            point(7, 500.0, 100.0),
            point(8, 610.0, 10.0),
            line(1, 1, 2),
            line(2, 4, 3),
            line(3, 8, 6),
            round(1, 3, 5.0),
            arc(2, 5, 100.0, 6, 7),
        ],
        vec![
            rule(
                1,
                ConstraintKind::Parallel {
                    a: lid(1),
                    b: lid(2),
                },
            ),
            rule(
                2,
                ConstraintKind::Parallel {
                    a: lid(1),
                    b: lid(3),
                },
            ),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_line(&c, 1, -1, 4, -100);
    assert_line(&c, 2, -1, 4, 0);
    assert_line(&c, 3, -1, 4, -600);
    assert_eq!(grid(c.point(pid(3)).unwrap()), (0, 0));
    assert_eq!(grid(c.point(pid(6)).unwrap()), (600, 0));
    assert_meet(&c, 4, 20, 5, 2);
    assert_meet(&c, 8, 1220, 5, 2);
    assert_all_exact(&c, 2);
}

#[test]
fn a_point_mirrored_about_a_level_or_plumb_line_is_mirrored_not_rounded_again() {
    // As the test above, but each partner's own rounding would miss the mirror by a step.
    let s = build(
        vec![
            point(1, 0.0, 100.3),
            point(2, 500.0, 100.4),
            point(3, 100.2, 40.4),
            point(4, 100.1, 159.4),
            line(1, 1, 2),
            point(5, 300.2, 0.0),
            point(6, 299.8, 500.0),
            point(7, 220.4, 80.0),
            point(8, 379.4, 80.2),
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
    assert_eq!(grid(c.point(pid(4)).unwrap()), (100, 160));
    assert_eq!(grid(c.point(pid(8)).unwrap()), (380, 80));
    assert_all_exact(&c, 4);
}

#[test]
fn a_mirrored_point_whose_coordinate_another_line_shares_keeps_it_and_the_mirror_gives_way() {
    // Point 4's row is shared with point 9 by a level line, and point 8's column with point 10 by
    // a plumb one: the shared value is the group's own rounding, and the symmetry is not held.
    let s = build(
        vec![
            point(1, 0.0, 100.3),
            point(2, 500.0, 100.4),
            point(3, 100.2, 40.4),
            point(4, 100.1, 159.4),
            point(5, 300.2, 0.0),
            point(6, 299.8, 500.0),
            point(7, 220.4, 80.0),
            point(8, 379.4, 80.2),
            point(9, 150.0, 158.6),
            point(10, 378.6, 150.0),
            line(1, 1, 2),
            line(2, 5, 6),
            line(3, 4, 9),
            line(4, 8, 10),
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
            rule(5, ConstraintKind::Horizontal { line: lid(3) }),
            rule(6, ConstraintKind::Vertical { line: lid(4) }),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(grid(c.point(pid(4)).unwrap()), (100, 159));
    assert_eq!(grid(c.point(pid(9)).unwrap()), (150, 159));
    assert_eq!(grid(c.point(pid(8)).unwrap()), (379, 80));
    assert_eq!(grid(c.point(pid(10)).unwrap()), (379, 150));
    assert_eq!(c.mark(cid(3)), Some(Mark::Bounded));
    assert_eq!(c.mark(cid(4)), Some(Mark::Bounded));
    assert_eq!(c.broken, disagrees(&[3, 4]));
}

#[test]
fn points_symmetric_about_a_sloping_line_are_rounded_each_on_its_own() {
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 100.0, 37.0),
            point(3, 10.2, 50.4),
            point(4, 80.4, 20.6),
            line(1, 1, 2),
        ],
        vec![rule(
            1,
            ConstraintKind::Symmetric {
                a: pid(3),
                b: pid(4),
                line: lid(1),
            },
        )],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(grid(c.point(pid(3)).unwrap()), (10, 50));
    assert_eq!(grid(c.point(pid(4)).unwrap()), (80, 21));
    assert_eq!(c.mark(cid(1)), Some(Mark::Bounded));
    assert_eq!(c.broken, []);
}

#[test]
fn a_line_in_a_class_passes_through_its_end_that_another_line_shares() {
    // Line 2's second end is also line 3's first: the carrier passes through it, so the corner is
    // a grid point on both lines, and line 2's free first end slides onto the carrier.
    let s = build(
        vec![
            point(1, 10.0, 10.0),
            point(2, 20.0, 10.0),
            point(3, 40.0, 30.0),
            point(4, 100.0, 0.0),
            point(5, 104.0, 1.0),
            line(1, 4, 5),
            line(2, 1, 2),
            line(3, 2, 3),
        ],
        vec![rule(
            1,
            ConstraintKind::Parallel {
                a: lid(1),
                b: lid(2),
            },
        )],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_line(&c, 1, -1, 4, -100);
    assert_line(&c, 2, -1, 4, 20);
    assert_line(&c, 3, -1, 1, -10);
    assert_eq!(grid(c.point(pid(2)).unwrap()), (20, 10));
    assert_meet(&c, 1, 20, 15, 2);
    assert_all_exact(&c, 1);
}

#[test]
fn a_point_sliding_on_a_steep_line_keeps_its_rounded_row() {
    // The line from (0, 0) to (10, 100) is -10 x + y = 0; the point solved at (5.3, 51.6) keeps the
    // row y = 52 and slides along it to x = 5.2.
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 10.0, 100.0),
            point(3, 5.3, 51.6),
            line(1, 1, 2),
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
    assert_line(&c, 1, -10, 1, 0);
    assert_meet(&c, 3, 26, 260, 5);
    assert_all_exact(&c, 1);
}

#[test]
fn a_point_coincident_with_a_circles_centre_is_stored_as_that_centre() {
    // Point 2 coincides with the circle's centre, which stays a grid point, and is put on a line
    // that misses it: it takes the centre's form, and the later relation, on the line, gives way.
    let s = build(
        vec![
            point(1, 50.2, 30.4),
            point(2, 50.4, 29.8),
            point(3, 0.0, 0.0),
            point(4, 100.0, 61.0),
            line(1, 3, 4),
            round(1, 1, 10.0),
        ],
        vec![
            rule(
                1,
                ConstraintKind::Coincident {
                    a: pid(1),
                    b: pid(2),
                },
            ),
            rule(
                2,
                ConstraintKind::OnLine {
                    point: pid(2),
                    line: lid(1),
                },
            ),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(grid(c.point(pid(1)).unwrap()), (50, 30));
    assert_eq!(grid(c.point(pid(2)).unwrap()), (50, 30));
    assert_eq!(c.mark(cid(1)), Some(Mark::Exact));
    assert_eq!(c.mark(cid(2)), Some(Mark::Bounded));
    assert_eq!(c.broken, disagrees(&[2]));
}

#[test]
fn an_arcs_centre_is_on_its_tangent_lines_normal_and_a_line_that_is_not_tangent_gives_way() {
    // An arc of radius 500 about (0, 0) from (500, 0) to (300, 400), its centre solved at (2.6,
    // 1.4). The line from its end (300, 400) to (700, 100) is square to the radius there: the
    // centre is where that line's normal at the end meets the chord's bisector, (0, 0). The line
    // from its start (500, 0) to (600, 100) is not tangent: its normal misses the centre.
    let s = build(
        vec![
            point(1, 2.6, 1.4),
            point(2, 500.0, 0.0),
            point(3, 300.0, 400.0),
            point(4, 700.0, 100.0),
            point(5, 600.0, 100.0),
            line(1, 3, 4),
            line(2, 2, 5),
            arc(1, 1, 500.0, 2, 3),
        ],
        vec![
            rule(
                1,
                ConstraintKind::TangentAtEnd {
                    line: lid(1),
                    arc: kid(1),
                    point: pid(3),
                },
            ),
            rule(
                2,
                ConstraintKind::TangentAtEnd {
                    line: lid(2),
                    arc: kid(1),
                    point: pid(2),
                },
            ),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_line(&c, 1, 3, 4, 2500);
    assert_line(&c, 2, -1, 1, -500);
    assert!(c.arcs[0].1.round().centre().same(&at(0, 0, 1)).unwrap());
    assert_eq!(c.mark(cid(1)), Some(Mark::Exact));
    assert_eq!(c.mark(cid(2)), Some(Mark::Bounded));
    assert_eq!(c.broken, disagrees(&[2]));
}

#[test]
fn a_relation_whose_line_has_a_fixed_end_off_its_carrier_gives_way() {
    // In each group the first line's ends are both fixed, the second end a few steps off the
    // carrier the class's level or plumb line sets: the relation itself holds on the carriers, but
    // a carrier that misses its own end is not held. The last line is plumb with its ends fixed
    // three columns apart.
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 100.0, 7.0),
            point(3, 200.0, 0.3),
            point(4, 300.0, -0.2),
            point(5, 0.0, 50.0),
            point(6, 100.0, 57.0),
            point(7, 200.0, 50.3),
            point(8, 300.0, 49.8),
            point(9, 0.0, 100.0),
            point(10, 100.0, 107.0),
            point(11, 200.3, 100.0),
            point(12, 199.8, 200.0),
            point(13, 0.0, 300.0),
            point(14, 100.0, 307.0),
            point(15, 200.3, 300.0),
            point(16, 199.8, 400.0),
            point(17, 400.0, 0.0),
            point(18, 403.0, 50.0),
            line(1, 1, 2),
            line(2, 3, 4),
            line(3, 5, 6),
            line(4, 7, 8),
            line(5, 9, 10),
            line(6, 11, 12),
            line(7, 13, 14),
            line(8, 15, 16),
            line(9, 17, 18),
        ],
        vec![
            fix(1, 1, 0, 0.0),
            fix(2, 1, 1, 0.0),
            fix(3, 2, 0, 100.0),
            fix(4, 2, 1, 7.0),
            fix(5, 5, 0, 0.0),
            fix(6, 5, 1, 50.0),
            fix(7, 6, 0, 100.0),
            fix(8, 6, 1, 57.0),
            fix(9, 9, 0, 0.0),
            fix(10, 9, 1, 100.0),
            fix(11, 10, 0, 100.0),
            fix(12, 10, 1, 107.0),
            fix(13, 13, 0, 0.0),
            fix(14, 13, 1, 300.0),
            fix(15, 14, 0, 100.0),
            fix(16, 14, 1, 307.0),
            fix(17, 17, 0, 400.0),
            fix(18, 18, 0, 403.0),
            rule(19, ConstraintKind::Horizontal { line: lid(2) }),
            rule(
                20,
                ConstraintKind::Collinear {
                    a: lid(1),
                    b: lid(2),
                },
            ),
            rule(21, ConstraintKind::Horizontal { line: lid(4) }),
            rule(
                22,
                ConstraintKind::Parallel {
                    a: lid(3),
                    b: lid(4),
                },
            ),
            rule(23, ConstraintKind::Vertical { line: lid(6) }),
            rule(
                24,
                ConstraintKind::Perpendicular {
                    a: lid(5),
                    b: lid(6),
                },
            ),
            rule(25, ConstraintKind::Vertical { line: lid(8) }),
            rule(
                26,
                ConstraintKind::Turned {
                    a: lid(7),
                    b: lid(8),
                    angle: Amount::Value(0.5),
                },
            ),
            rule(27, ConstraintKind::Vertical { line: lid(9) }),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_line(&c, 1, 0, 1, 0);
    assert_line(&c, 2, 0, 1, 0);
    assert_line(&c, 3, 0, 1, 50);
    assert_line(&c, 4, 0, 1, 50);
    assert_line(&c, 5, 0, -1, -100);
    assert_line(&c, 6, 1, 0, 200);
    assert_line(&c, 7, 0, -1, -300);
    assert_line(&c, 8, 1, 0, 200);
    assert_line(&c, 9, 1, 0, 400);
    assert_eq!(grid(c.point(pid(2)).unwrap()), (100, 7));
    assert_eq!(grid(c.point(pid(18)).unwrap()), (403, 50));
    for k in [1..=19, 21..=21, 23..=23, 25..=25].into_iter().flatten() {
        assert_eq!(c.mark(cid(k)), Some(Mark::Exact), "rule {k}");
    }
    assert_eq!(c.broken, disagrees(&[20, 22, 24, 26, 27]));
}
