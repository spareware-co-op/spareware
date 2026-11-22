//! The contact rule at `ε_fit` (G242, chapter 11.4): an enclosure that excludes zero decides, one that
//! holds zero and lies strictly within `ε_fit` touches, anything else is a near contact with its
//! evidence and remedies; and a decision is refined to its limit before it is read.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_bool::carrier::{Bound, CarrierIx, SortedVec};
use geom_bool::rule::{
    CoincidenceChain, Contact, ContactError, EPS_FIT, NearContact, REFINEMENTS, Remedy, Verdict,
    decide, judge,
};
use geom_num::interval::Interval;
use geom_num::{P3, Sign, Steps};
use geom_poly::plane::PlaneIx;

fn iv(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

#[test]
fn an_enclosure_that_excludes_zero_is_decided_with_its_sign() {
    assert_eq!(judge(iv(0.5, 100.0)), Verdict::Decided(Sign::Positive));
    assert_eq!(judge(iv(-100.0, -0.5)), Verdict::Decided(Sign::Negative));
    assert_eq!(
        judge(iv(f64::MIN_POSITIVE, 1.0e30)),
        Verdict::Decided(Sign::Positive)
    );
}

#[test]
fn an_enclosure_holding_zero_touches_only_strictly_within_the_contact_radius() {
    assert_eq!(EPS_FIT, Steps(64));
    assert_eq!(judge(iv(0.0, 0.0)), Verdict::Touch);
    assert_eq!(judge(iv(-1.0, 1.0)), Verdict::Touch);
    assert_eq!(judge(iv(0.0, 63.999_999)), Verdict::Touch);
    assert_eq!(judge(iv(-63.999_999, 0.5)), Verdict::Touch);
    // At exactly `ε_fit` it is near, not touching.
    assert_eq!(judge(iv(0.0, 64.0)), Verdict::Near);
    assert_eq!(judge(iv(-64.0, 1.0)), Verdict::Near);
    assert_eq!(judge(iv(-1.0e6, 1.0e6)), Verdict::Near);
}

#[test]
fn a_decision_is_refined_to_its_limit_and_apart_wins_whenever_it_can_be_proven() {
    // The enclosure shrinks with refinement and only the last one excludes zero: apart.
    let mut calls = 0;
    let result = decide(|level| {
        calls += 1;
        Ok(if level < REFINEMENTS {
            iv(-2.0, 2.0)
        } else {
            iv(0.001, 2.0)
        })
    })
    .unwrap();
    assert_eq!(result, Contact::Apart(Sign::Positive));
    assert_eq!(calls, REFINEMENTS + 1);
    // One that excludes zero early stops early.
    let mut calls = 0;
    let result = decide(|level| {
        calls += 1;
        Ok(if level < 3 {
            iv(-2.0, 2.0)
        } else {
            iv(-9.0, -1.0)
        })
    })
    .unwrap();
    assert_eq!(result, Contact::Apart(Sign::Negative));
    assert_eq!(calls, 4);
}

#[test]
fn an_enclosure_that_still_holds_zero_at_the_limit_touches_or_fails_with_evidence() {
    let mut calls = 0;
    let touch = decide(|_| {
        calls += 1;
        Ok(iv(-3.0, 3.0))
    })
    .unwrap();
    assert_eq!(touch, Contact::Touch(iv(-3.0, 3.0)));
    assert_eq!(calls, REFINEMENTS + 1);
    let near = decide(|_| Ok(iv(-70.0, 70.0))).unwrap_err();
    let ContactError::NearContact(evidence) = near else {
        panic!("{near:?}");
    };
    assert_eq!(evidence.enclosure, iv(-70.0, 70.0));
    assert_eq!(evidence.refinement, REFINEMENTS);
    // The finest enclosure is the evidence, not the first.
    let narrowing = decide(|level| {
        Ok(iv(
            -1000.0 / f64::from(level + 1),
            1000.0 / f64::from(level + 1),
        ))
    });
    let Err(ContactError::NearContact(evidence)) = narrowing else {
        panic!("{narrowing:?}");
    };
    assert!(evidence.enclosure.hi() < 120.0 && evidence.enclosure.hi() > 100.0);
}

#[test]
fn an_error_from_a_measure_is_passed_on() {
    let stopped = decide(|level| {
        if level == 2 {
            Err(ContactError::Num(geom_num::NumError::BudgetExceeded))
        } else {
            Ok(iv(-1.0, 1.0))
        }
    });
    assert_eq!(
        stopped,
        Err(ContactError::Num(geom_num::NumError::BudgetExceeded))
    );
}

#[test]
fn the_remedies_name_the_width_to_move_or_close() {
    let near = NearContact {
        enclosure: iv(-70.5, 12.0),
        refinement: REFINEMENTS,
    };
    assert_eq!(
        near.remedies(),
        [
            Remedy::MoveApart(Bound(Steps(71))),
            Remedy::CloseNearContacts(Bound(Steps(71))),
            Remedy::MeshPath
        ]
    );
    let chain = CoincidenceChain {
        carrier: CarrierIx::Plane(PlaneIx::default()),
        distance: iv(2.0, 9.25),
        remaining: Bound(Steps(4)),
    };
    assert_eq!(
        chain.remedies(),
        [
            Remedy::CloseNearContacts(Bound(Steps(10))),
            Remedy::MoveApart(Bound(Steps(10)))
        ]
    );
}

#[test]
fn bounds_round_up_and_refuse_what_is_not_a_distance() {
    assert_eq!(Bound::of(0.0), Some(Bound(Steps(0))));
    assert_eq!(Bound::of(0.000_001), Some(Bound(Steps(1))));
    assert_eq!(Bound::of(3.0), Some(Bound(Steps(3))));
    assert_eq!(Bound::of(3.000_000_1), Some(Bound(Steps(4))));
    assert_eq!(Bound::of(-0.5), None);
    assert_eq!(Bound::of(f64::NAN), None);
    assert_eq!(Bound::of(f64::INFINITY), None);
    assert_eq!(Bound::of(1.0e19), None);
}

#[test]
fn carrier_sets_have_one_order_whatever_order_they_were_found_in() {
    use geom_bool::carrier::PointGeom;
    let a = CarrierIx::Plane(PlaneIx::default());
    let set = SortedVec::from_items([a, a, a]);
    assert_eq!(set.len(), 1);
    assert!(set.contains(&a));
    let p = P3::new(1, 2, 3).unwrap();
    let grid = PointGeom::Grid(p);
    assert!(grid.is_exact());
    assert!(grid.carriers().is_empty());
    let certified = PointGeom::Certified {
        at: p,
        on: set.clone(),
        within: Bound(Steps(1)),
    };
    assert!(!certified.is_exact());
    assert_eq!(certified.carriers(), set);
}
