//! Tests of `geom_sketch::check` (piece G226): the checks after a commit.
//!
//! The board's done-when: "Exact re-check of root choices (CommitFlip), interval residual bounds
//! per constraint, the ε_fit ceiling; flips near degeneracy and bounds against dashu tested; a
//! 1,000-entity sketch checked in 2 ms." The benchmark is `benches/check.rs`.
//!
//! What this file proves: a sketch the commit holds exactly checks as exact; a dimension the commit
//! can only bound gets a bound that contains the exact residual, computed in `dashu` rationals,
//! and no more than a hair above it; a stored side that rounding carries onto the line fails with
//! `CommitFlip` naming the constraint, and one that survives passes; a residual past the ceiling
//! fails with `ExactnessNotAccepted`; and a check changes nothing.

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
use geom_sketch::check::{Checker, Holds};
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
