//! Tests of `geom_sketch::check` (piece G226): the checks after a commit.
//!
//! The board's done-when: "Exact re-check of root choices (`CommitFlip`), interval residual bounds
//! per constraint, the `ε_fit` ceiling; flips near degeneracy and bounds against `dashu` tested; a
//! 1,000-entity sketch checked in 2 ms." The benchmark is `benches/check.rs`.
//!
//! What this file proves: a sketch the commit holds exactly checks as exact; a dimension the commit
//! can only bound gets a bound that contains the exact residual, computed in `dashu` rationals,
//! and no more than a hair above it; a stored side that rounding carries onto the line fails with
//! `CommitFlip` naming the constraint, and one that survives passes; a residual past the ceiling
//! fails with `ExactnessNotAccepted`; a check changes nothing and is charged eight units of work for
//! each constraint; and a whole radius enters its bound as the point itself, widened only by the
//! arithmetic.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests, written as the definitions read; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;
use geom_num::{GridStep, P3};
use geom_run::{Budget, WorkMeter};
use geom_sketch::check::{Checked, Checker, Holds};
use geom_sketch::commit::commit;
use geom_sketch::model::{
    Amount, Constraint, ConstraintKind, Entity, EntityKind, Extent, ItemId, Side, Sketch,
    SketchError, Standing,
};
use geom_sketch::plane::{Lattice, PlaneFrame, SketchPlane};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
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
    let lattice = Lattice::new(P3::new(0, 0, 0).unwrap(), [1, 0, 0], [0, 1, 0]).unwrap();
    let mut s = Sketch::new(SketchPlane::new(PlaneFrame::Lattice(lattice), GridStep::Q0));
    for e in entities {
        s.add_entity(e).unwrap();
    }
    for c in constraints {
        s.add_constraint(c).unwrap();
    }
    s
}

fn ratio(n: i64, d: u64) -> RBig {
    RBig::from_parts(IBig::from(n), UBig::from(d))
}

fn length_of(value: f64) -> Vec<Constraint> {
    vec![rule(
        1,
        ConstraintKind::Length {
            line: lid(1),
            length: Amount::Value(value),
        },
    )]
}

#[test]
fn what_the_commit_holds_exactly_checks_as_exact() {
    let s = build(
        vec![point(1, 0.2, 0.1), point(2, 100.3, 0.4), line(1, 1, 2)],
        vec![rule(1, ConstraintKind::Horizontal { line: lid(1) })],
    );
    let c = commit(&s, &mut meter()).unwrap();
    let checker = Checker::new(&s).unwrap();
    let got = checker.check(&s, &c, Checker::fit(), &mut meter()).unwrap();
    assert_eq!(got.get(cid(1)), Some(Holds::Exact));
}

#[test]
fn a_bounded_dimension_gets_a_bound_that_contains_its_exact_residual() {
    // Two points that round to (0, 0) and (30, 40), a length of 49 stated: the residual is
    // (|PQ|^2 - d^2) / 2d = (2500 - 2401) / 98 = 99 / 98 steps, exactly.
    let s = build(
        vec![point(1, 0.3, 0.2), point(2, 30.2, 39.8), line(1, 1, 2)],
        length_of(49.0),
    );
    let c = commit(&s, &mut meter()).unwrap();
    let checker = Checker::new(&s).unwrap();
    let got = checker.check(&s, &c, Checker::fit(), &mut meter()).unwrap();
    let Some(Holds::Within { residual }) = got.get(cid(1)) else {
        panic!("{got:?}");
    };
    let bound = RBig::try_from(residual).unwrap();
    let exact = ratio(99, 98);
    assert!(bound >= exact, "{residual} below {exact}");
    // And the bound is tight: it exceeds the exact value by far less than a millionth of a step.
    let slack = RBig::try_from(1e-6).unwrap();
    assert!(&bound - &exact < slack, "{residual}");
}

#[test]
fn a_length_that_holds_exactly_after_rounding_has_a_bound_of_nearly_zero() {
    let s = build(
        vec![point(1, 0.3, 0.2), point(2, 30.2, 39.8), line(1, 1, 2)],
        length_of(50.0),
    );
    let c = commit(&s, &mut meter()).unwrap();
    let got = Checker::new(&s)
        .unwrap()
        .check(&s, &c, Checker::fit(), &mut meter())
        .unwrap();
    let Some(Holds::Within { residual }) = got.get(cid(1)) else {
        panic!("{got:?}");
    };
    assert!(residual < 1e-9, "{residual}");
}

fn wall_and_point(y: f64, side: Side) -> Sketch {
    build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 1000.0, 300.0),
            point(3, 500.4, y),
            line(1, 1, 2),
        ],
        vec![rule(
            1,
            ConstraintKind::FromLine {
                point: pid(3),
                line: lid(1),
                distance: Amount::Value(0.01),
                side,
            },
        )],
    )
}

#[test]
fn a_stored_side_that_rounding_carries_onto_the_line_is_a_commit_flip() {
    // A wall from (0, 0) to (1000, 300) and a point 0.02 steps below it at x = 500.4. The point
    // rounds to (500, 150), which is exactly on the wall: the stored side (right) cannot be told.
    let s = wall_and_point(150.1, Side::Right);
    let c = commit(&s, &mut meter()).unwrap();
    let checker = Checker::new(&s).unwrap();
    assert_eq!(
        checker.check(&s, &c, Checker::fit(), &mut meter()),
        Err(SketchError::CommitFlip(cid(1)))
    );
}

#[test]
fn a_stored_side_that_survives_rounding_passes() {
    // The same wall and a point well below it (right of the way the wall runs).
    let s = wall_and_point(100.1, Side::Right);
    let c = commit(&s, &mut meter()).unwrap();
    let got = Checker::new(&s)
        .unwrap()
        .check(&s, &c, 1.0e9, &mut meter())
        .unwrap();
    assert!(
        matches!(got.get(cid(1)), Some(Holds::Within { .. })),
        "{got:?}"
    );
    // And the wrong side stored for it is a flip.
    let wrong = wall_and_point(100.1, Side::Left);
    let c = commit(&wrong, &mut meter()).unwrap();
    assert_eq!(
        Checker::new(&wrong)
            .unwrap()
            .check(&wrong, &c, 1.0e9, &mut meter()),
        Err(SketchError::CommitFlip(cid(1)))
    );
}

#[test]
fn a_residual_past_the_ceiling_is_refused_with_its_bound() {
    // A length of 500 stated of two points 50 apart: (250000 - 2500) / 1000 = 247.5 steps away,
    // past the 64-step ceiling.
    let s = build(
        vec![point(1, 0.0, 0.0), point(2, 30.0, 40.0), line(1, 1, 2)],
        length_of(500.0),
    );
    let c = commit(&s, &mut meter()).unwrap();
    let checker = Checker::new(&s).unwrap();
    let Err(SketchError::ExactnessNotAccepted { constraint, bound }) =
        checker.check(&s, &c, Checker::fit(), &mut meter())
    else {
        panic!("not refused");
    };
    assert_eq!(constraint, cid(1));
    assert!((247.5..247.6).contains(&bound), "{bound}");
    // A caller that accepts more gets the bound instead.
    let got = checker.check(&s, &c, 1.0e6, &mut meter()).unwrap();
    assert!(matches!(got.get(cid(1)), Some(Holds::Within { .. })));
}

#[test]
fn checking_changes_nothing_and_a_small_budget_refuses() {
    let s = build(
        vec![point(1, 0.3, 0.2), point(2, 30.2, 39.8), line(1, 1, 2)],
        length_of(50.0),
    );
    let c = commit(&s, &mut meter()).unwrap();
    let before = format!("{c:?}");
    let checker = Checker::new(&s).unwrap();
    let first = checker.check(&s, &c, Checker::fit(), &mut meter()).unwrap();
    let second = checker.check(&s, &c, Checker::fit(), &mut meter()).unwrap();
    assert_eq!(first, second);
    assert_eq!(format!("{c:?}"), before);
    let mut none = WorkMeter::new(Budget::of_work(1));
    assert_eq!(
        checker.check(&s, &c, Checker::fit(), &mut none),
        Err(SketchError::Stopped)
    );
}

#[test]
fn a_check_is_charged_eight_units_of_work_for_each_constraint() {
    // Two constraints, a length and a distance across: the check is charged 16 units before it
    // reads anything, so a budget of 16 covers it and one of 15 stops it.
    let s = build(
        vec![point(1, 0.3, 0.2), point(2, 30.2, 39.8), line(1, 1, 2)],
        vec![
            rule(
                1,
                ConstraintKind::Length {
                    line: lid(1),
                    length: Amount::Value(50.0),
                },
            ),
            rule(
                2,
                ConstraintKind::Across {
                    a: pid(1),
                    b: pid(2),
                    distance: Amount::Value(30.0),
                },
            ),
        ],
    );
    let c = commit(&s, &mut meter()).unwrap();
    let checker = Checker::new(&s).unwrap();
    let mut enough = WorkMeter::new(Budget::of_work(16));
    let got = checker.check(&s, &c, Checker::fit(), &mut enough).unwrap();
    assert_eq!(got.get(cid(2)), Some(Holds::Exact));
    let mut short = WorkMeter::new(Budget::of_work(15));
    assert_eq!(
        checker.check(&s, &c, Checker::fit(), &mut short),
        Err(SketchError::Stopped)
    );
}

fn circle(key: u64, centre: u64, radius: f64) -> Entity {
    Entity {
        id: ItemId::new(4, key),
        kind: EntityKind::Circle {
            centre: pid(centre),
            radius,
        },
        construction: false,
    }
}

#[test]
fn a_point_rounding_onto_a_circle_is_a_commit_flip() {
    // A circle of radius 50.4 (stored 50) about (200, 200) and a point 50.3 away, inside it. The
    // point rounds to (250, 200), exactly on the circle of radius 50: the stored inside is lost.
    let s = build(
        vec![
            point(1, 200.0, 200.0),
            point(2, 250.3, 200.0),
            circle(1, 1, 50.4),
        ],
        vec![rule(
            1,
            ConstraintKind::FromCircle {
                point: pid(2),
                circle: ItemId::new(4, 1),
                distance: Amount::Value(0.1),
                outside: false,
            },
        )],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(
        Checker::new(&s)
            .unwrap()
            .check(&s, &c, Checker::fit(), &mut meter()),
        Err(SketchError::CommitFlip(cid(1)))
    );
}

#[test]
fn two_circles_rounding_into_a_tangency_it_was_not_are_a_commit_flip() {
    // Two circles of radius 10 stored as touching from inside, whose centres round to (0, 0) and
    // (10, 10): the distance between them squared is 200, the sum of the radii squared, so neither
    // outside nor inside can be told.
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 10.2, 9.9),
            circle(1, 1, 10.0),
            circle(2, 2, 10.0),
        ],
        vec![rule(
            1,
            ConstraintKind::TangentCircles {
                a: ItemId::new(4, 1),
                b: ItemId::new(4, 2),
                outside: false,
            },
        )],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(
        Checker::new(&s)
            .unwrap()
            .check(&s, &c, Checker::fit(), &mut meter()),
        Err(SketchError::CommitFlip(cid(1)))
    );
}

fn kid(key: u64) -> ItemId {
    ItemId::new(4, key)
}

fn on_circle(key: u64, point: u64, circle: u64) -> Constraint {
    rule(
        key,
        ConstraintKind::OnCircle {
            point: pid(point),
            circle: kid(circle),
        },
    )
}

fn tangent_circles(outside: bool) -> Constraint {
    rule(
        1,
        ConstraintKind::TangentCircles {
            a: kid(1),
            b: kid(2),
            outside,
        },
    )
}

/// What the checks make of `s`, committed, with a ceiling no residual here reaches.
fn checked(s: &Sketch) -> Result<Checked, SketchError> {
    let c = commit(s, &mut meter()).unwrap();
    Checker::new(s).unwrap().check(s, &c, 1.0e9, &mut meter())
}

#[test]
fn circles_well_apart_or_well_nested_keep_their_stored_tangency() {
    // Outside: centres 30 apart, radii 10: 900 against 200. Inside: centres 2 apart, radii 10 and
    // 3: 4 against 109. Each choice is told as it was stored, and the other one is a flip.
    let apart = |outside: bool| {
        build(
            vec![
                point(1, 0.0, 0.0),
                point(2, 30.0, 0.0),
                circle(1, 1, 10.0),
                circle(2, 2, 10.0),
            ],
            vec![tangent_circles(outside)],
        )
    };
    let nested = |outside: bool| {
        build(
            vec![
                point(1, 0.0, 0.0),
                point(2, 2.0, 0.0),
                circle(1, 1, 10.0),
                circle(2, 2, 3.0),
            ],
            vec![tangent_circles(outside)],
        )
    };
    assert!(checked(&apart(true)).is_ok());
    assert_eq!(checked(&apart(false)), Err(SketchError::CommitFlip(cid(1))));
    assert!(checked(&nested(false)).is_ok());
    assert_eq!(checked(&nested(true)), Err(SketchError::CommitFlip(cid(1))));
}

#[test]
fn circles_rounding_onto_the_threshold_flip_from_outside_too() {
    // The centres round to (0, 0) and (10, 10): 200 against 10^2 + 10^2. Neither choice can be
    // told, so a stored outside fails as a stored inside does.
    let s = build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 10.2, 9.9),
            circle(1, 1, 10.0),
            circle(2, 2, 10.0),
        ],
        vec![tangent_circles(true)],
    );
    assert_eq!(checked(&s), Err(SketchError::CommitFlip(cid(1))));
}

#[test]
fn circles_through_defining_points_are_told_by_their_exact_radii() {
    // Each circle passes through a defining point, so the commit stores it by that point: radius
    // squared 6^2 + 8^2 = 100 each, centres rounding to (0, 0) and (10, 10), 200 apart squared.
    let at = |further: f64| {
        build(
            vec![
                point(1, 0.0, 0.0),
                point(2, 10.2 + further, 9.9),
                point(3, 6.1, 7.9),
                point(4, 16.1 + further, 17.9),
                circle(1, 1, 10.3),
                circle(2, 2, 10.3),
            ],
            vec![
                tangent_circles(true),
                on_circle(2, 3, 1),
                on_circle(3, 4, 2),
            ],
        )
    };
    let s = at(0.0);
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!((c.circles.len(), c.rounds.len()), (0, 2), "{c:?}");
    assert_eq!(checked(&s), Err(SketchError::CommitFlip(cid(1))));
    // Moved 20 steps further apart, outside is told.
    let far = at(20.0);
    let c = commit(&far, &mut meter()).unwrap();
    assert_eq!(c.rounds.len(), 2, "{c:?}");
    assert!(checked(&far).is_ok());
}

/// A wall from (0, 0) to (1000, 300) and a second line parallel to it, starting at (500.4, y).
fn two_walls(y: f64, side: Side) -> Sketch {
    build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 1000.0, 300.0),
            point(3, 500.4, y),
            point(4, 1500.4, y + 300.0),
            line(1, 1, 2),
            line(2, 3, 4),
        ],
        vec![rule(
            1,
            ConstraintKind::LinesApart {
                a: lid(1),
                b: lid(2),
                distance: Amount::Value(0.01),
                side,
            },
        )],
    )
}

#[test]
fn a_stored_side_between_lines_is_decided_again() {
    // The second line's start rounds onto the wall: the side cannot be told.
    assert_eq!(
        checked(&two_walls(150.1, Side::Right)),
        Err(SketchError::CommitFlip(cid(1)))
    );
    // Well below the wall: right is told, and left is a flip.
    assert!(checked(&two_walls(100.1, Side::Right)).is_ok());
    assert_eq!(
        checked(&two_walls(100.1, Side::Left)),
        Err(SketchError::CommitFlip(cid(1)))
    );
}

/// The same wall and a circle of radius 10 about (500.4, y), tangent to it on `side`.
fn wall_and_circle(y: f64, side: Side) -> Sketch {
    build(
        vec![
            point(1, 0.0, 0.0),
            point(2, 1000.0, 300.0),
            point(3, 500.4, y),
            line(1, 1, 2),
            circle(1, 3, 10.0),
        ],
        vec![rule(
            1,
            ConstraintKind::TangentLine {
                line: lid(1),
                circle: kid(1),
                side,
            },
        )],
    )
}

#[test]
fn a_stored_side_of_a_tangent_circle_is_decided_again() {
    assert_eq!(
        checked(&wall_and_circle(150.1, Side::Right)),
        Err(SketchError::CommitFlip(cid(1)))
    );
    assert!(checked(&wall_and_circle(100.1, Side::Right)).is_ok());
    assert_eq!(
        checked(&wall_and_circle(100.1, Side::Left)),
        Err(SketchError::CommitFlip(cid(1)))
    );
}

/// The residual bound of constraint 1 in `s`.
fn bound_of(s: &Sketch) -> f64 {
    let got = checked(s).unwrap();
    let Some(Holds::Within { residual }) = got.get(cid(1)) else {
        panic!("{got:?}");
    };
    residual
}

fn radius(of: u64, value: f64) -> Constraint {
    rule(
        1,
        ConstraintKind::Radius {
            circle: kid(of),
            radius: Amount::Value(value),
        },
    )
}

#[test]
fn a_radius_is_bounded_on_the_committed_circle_not_the_solved_one() {
    // A circle drawn at radius 50.4 and stated so: the commit stores a whole radius of 50, so the
    // stated radius is 0.4 steps away on what was stored.
    let whole = build(
        vec![point(1, 200.0, 200.0), circle(1, 1, 50.4)],
        vec![radius(1, 50.4)],
    );
    let r = bound_of(&whole);
    assert!((0.39..0.41).contains(&r), "{r}");
    // Through a defining point (230, 240): radius 50 exactly, stored by the point.
    let round = build(
        vec![
            point(1, 200.0, 200.0),
            point(2, 230.2, 240.1),
            circle(1, 1, 50.4),
        ],
        vec![radius(1, 50.4), on_circle(2, 2, 1)],
    );
    let c = commit(&round, &mut meter()).unwrap();
    assert_eq!(c.rounds.len(), 1, "{c:?}");
    let r = bound_of(&round);
    assert!((0.39..0.41).contains(&r), "{r}");
}

#[test]
fn a_whole_radius_enters_the_bound_as_the_point_itself() {
    // The circle drawn at 50.4 and stated so is stored at the whole radius 50. The rule's residual
    // is r - 0 - 1 * 50.4, and on what was stored it is exactly 50.4 - 50, a double (the two are
    // within a factor of two, so their difference is exact). A whole number a double holds is held
    // as the point itself, so the bound is that value widened by the arithmetic alone: one step of
    // 50 (2^-47) for r - 0, one for 1 * 50.4, and one step of the result (2^-54) for the last
    // subtraction, each exact here. A radius held a step wide each way would add a third step of 50.
    let s = build(
        vec![point(1, 200.0, 200.0), circle(1, 1, 50.4)],
        vec![radius(1, 50.4)],
    );
    let exact = 50.4_f64 - 50.0;
    let step = 2.0_f64.powi(-47);
    let widened = exact + 2.0 * step + 2.0_f64.powi(-54);
    let r = bound_of(&s);
    assert_eq!(r.to_bits(), widened.to_bits(), "{r} against {widened}");
}

#[test]
fn an_arcs_radius_is_bounded_on_the_committed_arc() {
    // An arc about (200, 200) drawn at radius 100.3, its ends rounding to (300, 200) and
    // (200, 300): the stored arc's radius is 100, 0.3 steps from the stated one.
    let s = build(
        vec![
            point(1, 200.0, 200.0),
            point(2, 300.3, 200.0),
            point(3, 200.0, 300.3),
            Entity {
                id: kid(1),
                kind: EntityKind::Arc {
                    centre: pid(1),
                    radius: 100.3,
                    start: pid(2),
                    end: pid(3),
                },
                construction: false,
            },
        ],
        vec![radius(1, 100.3)],
    );
    let c = commit(&s, &mut meter()).unwrap();
    assert_eq!(c.arcs.len(), 1, "{c:?}");
    let r = bound_of(&s);
    assert!((0.29..0.31).contains(&r), "{r}");
}

#[test]
fn a_bound_equal_to_the_ceiling_is_accepted() {
    let s = build(
        vec![point(1, 0.0, 0.0), point(2, 30.0, 40.0), line(1, 1, 2)],
        length_of(500.0),
    );
    let c = commit(&s, &mut meter()).unwrap();
    let checker = Checker::new(&s).unwrap();
    let bound = bound_of(&s);
    let got = checker.check(&s, &c, bound, &mut meter()).unwrap();
    assert_eq!(got.get(cid(1)), Some(Holds::Within { residual: bound }));
    assert!(matches!(
        checker.check(&s, &c, bound.next_down(), &mut meter()),
        Err(SketchError::ExactnessNotAccepted { .. })
    ));
}
