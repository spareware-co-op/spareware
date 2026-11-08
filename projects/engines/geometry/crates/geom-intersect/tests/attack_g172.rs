//! Attacks on piece G172's claims: an item's bound is 12.2's `ε_out`, rounded up; past the ceiling it
//! is flagged and never a wider Certified item; and the bound holds for shapes really moved.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_intersect::IntersectError;
use geom_intersect::bound::{Terms, settle};
use geom_intersect::result::{Item, ParamBox, PointContact, Unresolved};
use geom_math::Box3;
use geom_num::Exactness;
use geom_num::grid::{GridStep, P3, Steps};
use geom_num::interval::Interval;

const CEILING: Steps = Steps(100);

fn p3(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn terms(a: u64, b: u64, sine: f64, curvature: f64) -> Terms {
    Terms {
        operands: [Steps(a), Steps(b)],
        sine,
        curvature,
        step: GridStep::Q0,
        xyz: Box3::new(p3(0, 0, 0), p3(1, 1, 1)).unwrap(),
    }
}

fn point(class: Exactness) -> Item {
    Item::Point {
        at: ParamBox::new(&[Interval::point(0.5).unwrap(); 2]).unwrap(),
        xyz: Box3::new(p3(0, 0, 0), p3(1, 1, 1)).unwrap(),
        contact: PointContact::Crossing,
        class,
    }
}

fn certified(bound: u64) -> Exactness {
    Exactness::Certified {
        bound: Steps(bound),
        step: GridStep::Q0,
    }
}

fn class(item: &Item) -> Exactness {
    match item {
        Item::Point { class, .. } => *class,
        other => panic!("{other:?}"),
    }
}

fn needed(item: &Item) -> Steps {
    match item {
        Item::Flagged {
            why: Unresolved::BoundExceeded { needed },
            ..
        } => *needed,
        other => panic!("{other:?}"),
    }
}

#[test]
fn exact_data_stays_exact() {
    let item = settle(point(Exactness::Exact), &terms(0, 0, 0.5, 1.0), CEILING).unwrap();
    assert_eq!(class(&item), Exactness::Exact);
    // At no angle at all, because nothing has moved.
    let item = settle(point(Exactness::Exact), &terms(0, 0, 0.0, 1.0), CEILING).unwrap();
    assert_eq!(class(&item), Exactness::Exact);
}

#[test]
fn a_bound_of_nothing_keeps_a_certified_items_own() {
    let item = settle(point(certified(7)), &terms(0, 0, 0.5, 0.0), CEILING).unwrap();
    assert_eq!(class(&item), certified(7));
}

#[test]
fn an_operands_bound_makes_an_exact_item_certified() {
    for (a, b) in [(3, 0), (0, 3)] {
        let item = settle(point(Exactness::Exact), &terms(a, b, 0.5, 0.0), CEILING).unwrap();
        // (3 + 0) / 0.5 is 6, and the interval's rounding takes it to the next step up.
        assert_eq!(class(&item), certified(7));
    }
}

#[test]
fn an_items_own_error_adds_to_what_the_operands_move() {
    let item = settle(point(certified(5)), &terms(2, 3, 0.25, 0.0), CEILING).unwrap();
    assert_eq!(class(&item), certified(26));
}

/// With curvature the angle closes up over the distance moved: e/(s − 4κe·2/s).
#[test]
fn curvature_adds_the_second_order_rest() {
    let (e, s, kappa) = (4.0, 0.5, 5e-3);
    let item = settle(point(Exactness::Exact), &terms(2, 2, s, kappa), CEILING).unwrap();
    let exact = e / (s - 2.0 * kappa * (2.0 * e / s));
    let Exactness::Certified { bound, .. } = class(&item) else {
        panic!("{item:?}");
    };
    assert_eq!(bound.get() as f64, exact.ceil());
    assert!(exact > e / s);
}

/// The rate at which the angle closes is what stops the form: at s − 2κ·r₀ = s/2, it gives up.
#[test]
fn curvature_that_can_undo_the_angle_has_no_bound() {
    let (e, s) = (4.0, 0.5);
    let edge = s * s / (8.0 * e);
    let item = settle(
        point(Exactness::Exact),
        &terms(2, 2, s, edge * 1.001),
        CEILING,
    )
    .unwrap();
    assert_eq!(needed(&item), Steps(u64::MAX));
    let item = settle(
        point(Exactness::Exact),
        &terms(2, 2, s, edge * 0.999),
        CEILING,
    )
    .unwrap();
    assert!(matches!(class(&item), Exactness::Certified { .. }));
}

#[test]
fn no_angle_carries_no_bound() {
    let item = settle(point(Exactness::Exact), &terms(1, 0, 0.0, 0.0), CEILING).unwrap();
    assert_eq!(needed(&item), Steps(u64::MAX));
}

#[test]
fn past_the_ceiling_an_item_is_flagged_with_the_bound_it_needed() {
    let at = ParamBox::new(&[Interval::point(0.5).unwrap(); 2]).unwrap();
    let item = settle(point(certified(95)), &terms(1, 2, 0.5, 0.0), CEILING).unwrap();
    assert_eq!(needed(&item), Steps(102));
    let Item::Flagged { at: kept, xyz, .. } = item else {
        panic!("{item:?}");
    };
    assert_eq!(kept, at);
    assert_eq!(xyz, terms(0, 0, 1.0, 0.0).xyz);
}

/// The ceiling is the most an item may carry, so a bound of exactly it stands.
#[test]
fn a_bound_of_the_ceiling_itself_stands() {
    let item = settle(point(certified(100)), &terms(0, 0, 0.5, 0.0), CEILING).unwrap();
    assert_eq!(class(&item), certified(100));
    let item = settle(point(certified(101)), &terms(0, 0, 0.5, 0.0), CEILING).unwrap();
    assert_eq!(needed(&item), Steps(101));
}

#[test]
fn a_flagged_item_stays_as_it_was() {
    let flagged = Item::Flagged {
        at: ParamBox::new(&[Interval::point(0.5).unwrap(); 2]).unwrap(),
        xyz: Box3::new(p3(0, 0, 0), p3(1, 1, 1)).unwrap(),
        why: Unresolved::HigherOrderContact,
    };
    let item = settle(flagged.clone(), &terms(1, 1, 0.5, 0.0), CEILING).unwrap();
    assert_eq!(item, flagged);
}

#[test]
fn an_approximate_item_grows_and_stays_approximate() {
    let approximate = |deviation| Exactness::Approximate {
        deviation: Steps(deviation),
        step: GridStep::Q0,
    };
    // Past the ceiling too: a deviation proves nothing, so there is nothing for the ceiling to judge.
    let item = settle(point(approximate(500)), &terms(1, 2, 0.5, 0.0), CEILING).unwrap();
    assert_eq!(class(&item), approximate(507));
}

#[test]
fn a_certified_item_keeps_its_own_grid_step() {
    let class_at = |step| Exactness::Certified {
        bound: Steps(1),
        step,
    };
    let item = settle(
        point(class_at(GridStep::Q1)),
        &terms(1, 0, 1.0, 0.0),
        CEILING,
    )
    .unwrap();
    assert_eq!(
        class(&item),
        Exactness::Certified {
            bound: Steps(3),
            step: GridStep::Q1
        }
    );
}

#[test]
fn a_sine_or_curvature_out_of_range_is_refused() {
    for (s, k) in [
        (f64::NAN, 0.0),
        (-0.5, 0.0),
        (1.5, 0.0),
        (0.5, f64::NAN),
        (0.5, -1.0),
        (0.5, f64::INFINITY),
    ] {
        assert_eq!(
            settle(point(Exactness::Exact), &terms(1, 1, s, k), CEILING),
            Err(IntersectError::InvalidInput)
        );
    }
}

/// The law itself: two circles of radius 1024 steps crossing at an angle, each moved outward or in by
/// its bound, cross within the bound of where they crossed.
#[test]
fn circles_really_moved_cross_within_the_bound() {
    let crossing = |r0: f64, r1: f64, d: f64| {
        // Centres d apart on the x axis.
        let x = (d * d + r0 * r0 - r1 * r1) / (2.0 * d);
        (x, (r0 * r0 - x * x).sqrt())
    };
    let r = 1024.0;
    let mut held = 0;
    for d in [128.0, 512.0, 1024.0, 1536.0, 1900.0] {
        let (x, y) = crossing(r, r, d);
        // The normals at the crossing point at the two centres.
        let (ax, ay) = (x / r, y / r);
        let (bx, by) = ((x - d) / r, y / r);
        let s = (ax * by - ay * bx).abs();
        for (a, b) in [(1u64, 1u64), (3, 1), (1, 3), (4, 4)] {
            let t = terms(a, b, s, 1.0 / r);
            let item = settle(point(Exactness::Exact), &t, Steps(u64::MAX - 1)).unwrap();
            // Near tangency the form has no bound to give, and the item says so.
            let Item::Point {
                class: Exactness::Certified { bound, .. },
                ..
            } = item
            else {
                continue;
            };
            held += 1;
            for (u, v) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                let (a, b) = (a as f64 * u, b as f64 * v);
                let (p, q) = crossing(r + a, r + b, d);
                let moved = ((p - x) * (p - x) + (q - y) * (q - y)).sqrt();
                assert!(moved <= bound.get() as f64, "{moved} past {bound:?} at {d}");
            }
        }
    }
    assert!(held >= 12, "{held}");
}
