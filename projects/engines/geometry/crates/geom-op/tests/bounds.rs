//! Per-entity error bounds (G143, 13.2, 32.21): each rule on a history worked by hand.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "test cases; the rules are for kernel code"
)]

use geom_num::ladder::{FIT, PER_OPERATION};
use geom_num::{Exactness, GridStep, Steps};
use geom_op::bounds::{Bounds, Limits, Spent};
use geom_op::detail::{BoundDetail, OpErrorDetail};
use geom_op::{
    ErrorKind, History, NameId, NameSet, OpError, OpId, OpPath, Origin, PieceKey, Record, Remedy,
    Role, Side,
};

fn name(x: u8) -> NameId {
    NameId::from_bytes([x; 16])
}

fn set(xs: &[u8]) -> NameSet {
    NameSet::new(xs.iter().map(|x| name(*x))).unwrap()
}

fn step(x: u8) -> OpPath {
    OpPath::new(OpId::from_bytes([x; 16]))
}

fn record(output: u8, origin: Origin) -> Record {
    Record {
        output: name(output),
        origin,
    }
}

fn history(records: Vec<Record>) -> History {
    History::new(records, set(&[]), []).unwrap()
}

fn unchanged(output: u8) -> Record {
    record(output, Origin::Unchanged)
}

fn modified(output: u8, from: &[u8]) -> Record {
    record(output, Origin::Modified { from: set(from) })
}

fn generated(output: u8, from: &[u8]) -> Record {
    record(
        output,
        Origin::Generated {
            from: set(from),
            role: Role::new("fillet", []).unwrap(),
        },
    )
}

/// Bounds of face 1 at 3 steps and face 2 at 5, each from its own step.
fn start() -> Bounds {
    let made = history(vec![generated(1, &[]), generated(2, &[])]);
    let first = Bounds::new(GridStep::Q0)
        .after(&step(10), &made, &[(name(1), Steps(3))], Limits::DEFAULT)
        .unwrap();
    first
        .after(
            &step(11),
            &history(vec![unchanged(1), modified(2, &[2])]),
            &[(name(2), Steps(5))],
            Limits::DEFAULT,
        )
        .unwrap()
}

#[test]
fn an_entity_with_no_term_is_exact_and_not_stored() {
    let empty = Bounds::new(GridStep::Q1);
    assert_eq!(empty.step(), GridStep::Q1);
    assert_eq!(empty.of(name(1)), Steps(0));
    assert_eq!(empty.terms(name(1)), &[]);
    assert_eq!(empty.entities().count(), 0);
    assert_eq!(empty.largest(), Steps(0));
    assert_eq!(empty.class(), Exactness::Exact);
    // A term of zero is no term.
    let made = history(vec![generated(1, &[])]);
    let after = empty
        .after(&step(1), &made, &[(name(1), Steps(0))], Limits::DEFAULT)
        .unwrap();
    assert_eq!(after, empty);
}

#[test]
fn a_new_entity_has_its_own_term_alone() {
    let bounds = start();
    assert_eq!(bounds.of(name(1)), Steps(3));
    assert_eq!(bounds.of(name(2)), Steps(5));
    assert_eq!(bounds.entities().collect::<Vec<_>>(), [name(1), name(2)]);
    assert_eq!(
        bounds.terms(name(2)),
        &[Spent {
            step: step(11),
            bound: Steps(5)
        }]
    );
}

#[test]
fn the_body_has_the_largest_and_the_class_says_so() {
    let bounds = start();
    assert_eq!(bounds.largest(), Steps(5));
    assert_eq!(
        bounds.class(),
        Exactness::Certified {
            bound: Steps(5),
            step: GridStep::Q0
        }
    );
    // The largest is not the last by name.
    let swapped = start()
        .after(
            &step(12),
            &history(vec![modified(1, &[1]), unchanged(2)]),
            &[(name(1), Steps(16))],
            Limits::DEFAULT,
        )
        .unwrap();
    assert_eq!(swapped.largest(), Steps(19));
}

#[test]
fn an_untouched_face_keeps_its_own_however_long_the_history() {
    // 200 cuts, each a new edge that spends its own term: the face they leave alone spends nothing.
    let mut bounds = start();
    for cut in 0..200_u8 {
        bounds = bounds
            .after(
                &step(cut),
                &history(vec![unchanged(1), unchanged(2), generated(3, &[])]),
                &[(name(3), Steps(1))],
                Limits::DEFAULT,
            )
            .unwrap();
        assert_eq!(bounds.of(name(1)), Steps(3));
        assert_eq!(bounds.of(name(3)), Steps(1));
    }
    assert_eq!(bounds.largest(), Steps(5));
}

#[test]
fn an_edge_filleted_after_a_loft_carries_both() {
    let bounds = start()
        .after(
            &step(12),
            &history(vec![modified(1, &[1]), unchanged(2)]),
            &[(name(1), Steps(4))],
            Limits::DEFAULT,
        )
        .unwrap();
    assert_eq!(bounds.of(name(1)), Steps(7));
    assert_eq!(bounds.of(name(2)), Steps(5));
    let spent: Vec<&Spent> = bounds.terms(name(1)).iter().collect();
    assert_eq!(spent.len(), 2);
    assert_eq!(spent[0].step, step(10));
    assert_eq!(spent[1].step, step(12));
}

#[test]
fn every_origin_sums_its_own_operands() {
    let cuts = PieceKey::ByTools {
        tools: set(&[9]),
        side: Side::Front,
    };
    let records = vec![
        // Generated from both, with a term of its own.
        generated(20, &[1, 2]),
        // Modified from both.
        modified(21, &[1, 2]),
        // Merged from both.
        record(22, Origin::Merged { of: set(&[1, 2]) }),
        // Split from one.
        record(
            23,
            Origin::Split {
                of: name(2),
                piece: cuts,
            },
        ),
        // Made from nothing, with no term.
        generated(24, &[]),
    ];
    let bounds = start()
        .after(
            &step(12),
            &history(records),
            &[(name(20), Steps(2))],
            Limits::DEFAULT,
        )
        .unwrap();
    assert_eq!(bounds.of(name(20)), Steps(10));
    assert_eq!(bounds.of(name(21)), Steps(8));
    assert_eq!(bounds.of(name(22)), Steps(8));
    assert_eq!(bounds.of(name(23)), Steps(5));
    assert_eq!(bounds.of(name(24)), Steps(0));
    // Inputs with no output go.
    assert_eq!(bounds.of(name(1)), Steps(0));
    assert_eq!(bounds.of(name(2)), Steps(0));
    assert_eq!(bounds.terms(name(1)), &[]);
}

#[test]
fn an_unchanged_entity_keeps_its_terms_as_they_are() {
    let bounds = start()
        .after(
            &step(12),
            &history(vec![unchanged(1), unchanged(2)]),
            &[],
            Limits::DEFAULT,
        )
        .unwrap();
    assert_eq!(bounds, start());
}

#[test]
fn the_order_of_reporting_changes_nothing() {
    let records = || vec![modified(1, &[1]), modified(2, &[2])];
    let one = start()
        .after(
            &step(12),
            &history(records()),
            &[(name(1), Steps(1)), (name(2), Steps(2))],
            Limits::DEFAULT,
        )
        .unwrap();
    let other = start()
        .after(
            &step(12),
            &history(records()),
            &[(name(2), Steps(2)), (name(1), Steps(1))],
            Limits::DEFAULT,
        )
        .unwrap();
    assert_eq!(one, other);
}

/// `error`'s remedy offering `needed` steps on Q0.
fn offers(error: &OpError, needed: Steps) -> bool {
    error.remedies().contains(&Remedy::Accept {
        class: Exactness::Certified {
            bound: needed,
            step: GridStep::Q0,
        },
    })
}

#[test]
fn a_sum_past_the_ceiling_is_refused_and_names_the_steps() {
    let limits = Limits {
        own: PER_OPERATION,
        total: Steps(9),
    };
    let records = || history(vec![modified(1, &[1]), modified(2, &[2])]);
    // Exactly on the ceiling passes.
    let held = start()
        .after(&step(12), &records(), &[(name(1), Steps(6))], limits)
        .unwrap();
    assert_eq!(held.of(name(1)), Steps(9));
    // A step past it does not.
    let error = start()
        .after(&step(12), &records(), &[(name(1), Steps(7))], limits)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::ExactnessNotAccepted);
    assert_eq!(
        *error.detail(),
        OpErrorDetail::Bound(BoundDetail::OverCeiling {
            needed: Steps(10),
            ceiling: Steps(9),
            step: GridStep::Q0,
        })
    );
    assert_eq!(error.op(), &step(12));
    assert_eq!(error.entities(), &[name(1)]);
    assert!(offers(&error, Steps(10)), "{:?}", error.remedies());
    let words = error.message();
    assert!(words.contains(&format!("{} spent 3", step(10))), "{words}");
    assert!(words.contains(&format!("{} spent 7", step(12))), "{words}");
    // With no own term at all, inherited bounds alone can pass it: a tighter caller.
    let tight = Limits {
        own: PER_OPERATION,
        total: Steps(4),
    };
    let error = start()
        .after(&step(12), &records(), &[], tight)
        .unwrap_err();
    assert_eq!(error.entities(), &[name(2)]);
}

#[test]
fn the_ceiling_is_the_fit_bound_unless_the_caller_accepts_more() {
    assert_eq!(Limits::DEFAULT.total, FIT);
    assert_eq!(Limits::DEFAULT.own, PER_OPERATION);
    // Four steps at the full ε_step fill ε_fit exactly; a fifth does not fit.
    let mut bounds = Bounds::new(GridStep::Q0)
        .after(
            &step(0),
            &history(vec![generated(1, &[])]),
            &[(name(1), PER_OPERATION)],
            Limits::DEFAULT,
        )
        .unwrap();
    let again = history(vec![modified(1, &[1])]);
    for i in 1..4 {
        bounds = bounds
            .after(
                &step(i),
                &again,
                &[(name(1), PER_OPERATION)],
                Limits::DEFAULT,
            )
            .unwrap();
    }
    assert_eq!(bounds.of(name(1)), FIT);
    assert_eq!(bounds.terms(name(1)).len(), 4);
    let error = bounds
        .after(&step(4), &again, &[(name(1), Steps(1))], Limits::DEFAULT)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::ExactnessNotAccepted);
    // The caller takes the larger bound, and it goes through.
    let accepted = Limits {
        own: PER_OPERATION,
        total: Steps(65),
    };
    let more = bounds
        .after(&step(4), &again, &[(name(1), Steps(1))], accepted)
        .unwrap();
    assert_eq!(more.of(name(1)), Steps(65));
}

#[test]
fn an_own_term_past_a_step_is_refused() {
    let records = || history(vec![modified(1, &[1]), modified(2, &[2])]);
    let limit = PER_OPERATION;
    let held = start()
        .after(&step(12), &records(), &[(name(1), limit)], Limits::DEFAULT)
        .unwrap();
    assert_eq!(held.of(name(1)), Steps(3 + limit.0));
    let over = Steps(limit.0 + 1);
    let error = start()
        .after(&step(12), &records(), &[(name(2), over)], Limits::DEFAULT)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::ExactnessNotAccepted);
    assert_eq!(
        *error.detail(),
        OpErrorDetail::Bound(BoundDetail::OverStep {
            own: over,
            limit,
            step: GridStep::Q0,
        })
    );
    assert_eq!(error.entities(), &[name(2)]);
    assert!(offers(&error, over), "{:?}", error.remedies());
}

#[test]
fn a_sum_that_would_wrap_is_past_any_ceiling() {
    let huge = Limits {
        own: Steps(u64::MAX),
        total: Steps(u64::MAX - 1),
    };
    let made = history(vec![generated(1, &[]), generated(2, &[])]);
    let bounds = Bounds::new(GridStep::Q0)
        .after(
            &step(0),
            &made,
            &[(name(1), Steps(u64::MAX - 1)), (name(2), Steps(9))],
            huge,
        )
        .unwrap();
    let error = bounds
        .after(&step(1), &history(vec![modified(3, &[1, 2])]), &[], huge)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::ExactnessNotAccepted);
}

#[test]
fn a_term_astray_is_a_defect() {
    let astray = OpErrorDetail::Bound(BoundDetail::OwnTermAstray);
    let records = || history(vec![unchanged(1), modified(2, &[2])]);
    // On an entity that passed through unchanged.
    let e = start()
        .after(
            &step(12),
            &records(),
            &[(name(1), Steps(1))],
            Limits::DEFAULT,
        )
        .unwrap_err();
    assert_eq!(*e.detail(), astray);
    assert_eq!(e.kind(), ErrorKind::Internal);
    assert_eq!(e.entities(), &[name(1)]);
    // On an entity that is not an output.
    let e = start()
        .after(
            &step(12),
            &records(),
            &[(name(9), Steps(1))],
            Limits::DEFAULT,
        )
        .unwrap_err();
    assert_eq!(*e.detail(), astray);
    assert_eq!(e.entities(), &[name(9)]);
    // Twice on one output.
    let e = start()
        .after(
            &step(12),
            &records(),
            &[(name(2), Steps(1)), (name(2), Steps(2))],
            Limits::DEFAULT,
        )
        .unwrap_err();
    assert_eq!(*e.detail(), astray);
    assert_eq!(e.entities(), &[name(2)]);
}

#[test]
fn operands_join_and_a_copy_keeps_the_larger() {
    let a = start();
    let made = history(vec![generated(2, &[]), generated(3, &[])]);
    let b = Bounds::new(GridStep::Q0)
        .after(
            &step(20),
            &made,
            &[(name(2), Steps(9)), (name(3), Steps(1))],
            Limits::DEFAULT,
        )
        .unwrap();
    let both = a.join(&step(30), &b).unwrap();
    assert_eq!(both.of(name(1)), Steps(3));
    assert_eq!(both.of(name(2)), Steps(9));
    assert_eq!(both.of(name(3)), Steps(1));
    assert_eq!(
        both.entities().collect::<Vec<_>>(),
        [name(1), name(2), name(3)]
    );
    assert_eq!(both.terms(name(2)).len(), 1);
    assert_eq!(both.terms(name(2))[0].step, step(20));
    // The other way round: the same.
    assert_eq!(b.join(&step(30), &a).unwrap(), both);
    // A smaller copy changes nothing, and neither does a like one.
    assert_eq!(a.join(&step(30), &a).unwrap(), a);
    let other = Bounds::new(GridStep::Q1);
    let e = a.join(&step(30), &other).unwrap_err();
    assert_eq!(*e.detail(), OpErrorDetail::Bound(BoundDetail::MixedSteps));
    assert_eq!(e.kind(), ErrorKind::InvalidInput);
    assert_eq!(e.op(), &step(30));
}

#[test]
fn every_detail_says_what_it_is() {
    let cases = [
        (
            BoundDetail::OverStep {
                own: Steps(17),
                limit: Steps(16),
                step: GridStep::Q0,
            },
            "17",
        ),
        (
            BoundDetail::OverCeiling {
                needed: Steps(70),
                ceiling: Steps(64),
                step: GridStep::Q0,
            },
            "70",
        ),
        (BoundDetail::OwnTermAstray, "term"),
        (BoundDetail::MixedSteps, "grid step"),
    ];
    for (detail, word) in cases {
        let words = detail.to_string();
        assert!(words.contains(word), "{words}");
        assert!(
            OpErrorDetail::from(detail).to_string().contains(word),
            "{detail:?}"
        );
    }
    let internal = OpErrorDetail::from(BoundDetail::OwnTermAstray);
    assert!(
        internal
            .remedies()
            .iter()
            .all(|r| !matches!(r, Remedy::Accept { .. }))
    );
}
