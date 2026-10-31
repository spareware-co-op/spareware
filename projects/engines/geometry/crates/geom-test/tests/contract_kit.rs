//! Every law of `geom_test::contract` can fail: each is broken here by a seeded operation.
//!
//! What this file proves: a small operation that charges its context's meter, keeps every law
//! (`a_good_operation_keeps_every_law`), and for each law an operation or a value made to break it
//! is refused, naming that law. The kit says nothing about the kernel until these pass.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

use std::cell::Cell;

use geom_num::{Exactness, GridStep, Steps};
use geom_op::{
    Bound, Class, ErrorKind, ErrorSpec, Family, MakeDetail, NameId, OpContext, OpError, OpSchema,
    Remedy, Value,
};
use geom_run::{Budget, Stopped, WorkMeter};
use geom_test::contract::{
    Done, EVERY_POINT_BELOW, Law, Subject, Trigger, accept_holds, detail_name,
    errors_tell_the_truth, exactness_holds, example_holds, no_tolerance_grows, range_holds,
    remedy_holds, transactional,
};
use geom_test::rng::SplitMix64;

fn rng() -> SplitMix64 {
    SplitMix64::new(geom_test::Seed(0x4731_3033))
}

const ZERO: ErrorSpec = ErrorSpec {
    detail: "ZeroLength",
    kind: ErrorKind::Degenerate,
    data: &[],
    doc: "A size of zero.",
};

const SCHEMA: OpSchema = OpSchema {
    name: "seeded.op",
    version: 1,
    family: Family::Make,
    doc: "A seeded operation.",
    inputs: &[],
    outputs: &[],
    roles: &[],
    errors: &[ZERO],
    diagnostics: &[],
    classes: &[Class::Exact],
    charges: &[],
    batch: false,
    example: "seeded.op { size: 3 } gives a solid of 6 faces and 8 vertices, Exact.",
};

const WRONG: OpSchema = OpSchema {
    errors: &[ErrorSpec {
        kind: ErrorKind::InvalidInput,
        ..ZERO
    }],
    ..SCHEMA
};

const SILENT: OpSchema = OpSchema {
    example: "seeded.op { size: 3 }",
    ..SCHEMA
};

fn certified(bound: u64) -> Exactness {
    Exactness::Certified {
        bound: Steps(bound),
        step: GridStep::Q0,
    }
}

/// Charges `units`, one at a time.
fn charge(meter: &mut WorkMeter<'_>, units: u64) -> Result<(), Stopped> {
    for _ in 0..units {
        meter.charge(1)?;
    }
    Ok(())
}

/// The seeded operation: `size` units of work, and an error at size 0.
fn run(c: &OpContext<'_>, size: u64) -> Result<Done<u64>, OpError> {
    if size == 0 {
        return Err(OpError::new(
            c.path().clone(),
            MakeDetail::ZeroLength { input: "size" },
        ));
    }
    c.run(|meter| {
        charge(meter, size).map_err(|s| c.stopped(meter, s, geom_op::RUN))?;
        Ok(Done {
            answer: size * 2,
            exactness: Exactness::Exact,
        })
    })
}

fn good(size: u64) -> Subject<'static, u64> {
    Subject::new(format!("seeded.op {size}"), move |c| run(c, size))
}

#[test]
fn a_good_operation_keeps_every_law() {
    let done = transactional(&good(12), &mut rng()).unwrap();
    assert_eq!(done.work, 12);
    assert!(!done.sampled);
    assert_eq!(
        errors_tell_the_truth(
            &SCHEMA,
            &[Trigger {
                detail: "ZeroLength",
                subject: good(0)
            }]
        ),
        Ok(())
    );
    example_holds(&SCHEMA, &[("face", 6), ("vertex", 8)], Exactness::Exact).unwrap();
}

#[test]
fn a_long_run_is_sampled_and_still_stopped_at_its_ends() {
    let done = transactional(&good(EVERY_POINT_BELOW * 3), &mut rng()).unwrap();
    assert_eq!(done.work, EVERY_POINT_BELOW * 3);
    assert!(done.sampled);
}

#[test]
fn an_operation_that_ignores_a_stop_is_caught() {
    // It never reads its context, so nothing can stop it.
    let deaf = Subject::new("deaf", |_| {
        Ok(Done {
            answer: 1u64,
            exactness: Exactness::Exact,
        })
    });
    let reads = transactional(&deaf, &mut rng()).unwrap();
    // No read and no budget: nothing to inject, and nothing published past a stop.
    assert_eq!((reads.reads, reads.work), (0, 0));
    // One that counts its budget itself and answers anyway publishes past it.
    let stubborn = Subject::new("stubborn", |c| {
        let spent = run(c, 5).map_or(10, |d| d.answer);
        Ok(Done {
            answer: spent,
            exactness: Exactness::Exact,
        })
    });
    let e = transactional(&stubborn, &mut rng()).unwrap_err();
    assert_eq!(e.law, Law::Transactional);
}

#[test]
fn a_budget_that_changes_the_answer_is_caught() {
    // It reads its own meter and answers either way, so a budget changes the answer, not stops it.
    let shifty = Subject::new("shifty", |c| {
        let tight = c.meter().charge(100).is_err();
        Ok(Done {
            answer: tight,
            exactness: Exactness::Exact,
        })
    });
    assert_eq!(
        transactional(&shifty, &mut rng()).unwrap_err().law,
        Law::Transactional
    );
}

#[test]
fn a_changed_input_is_caught() {
    let input = Cell::new(0u64);
    let writes = Subject::new("writes", |c| {
        input.set(input.get() + 1);
        run(c, 3)
    })
    .watching(|| input.get() == 0);
    assert_eq!(
        transactional(&writes, &mut rng()).unwrap_err().law,
        Law::Transactional
    );
}

#[test]
fn two_answers_are_caught() {
    let count = Cell::new(0u64);
    let moody = Subject::new("moody", |_| {
        count.set(count.get() + 1);
        Ok(Done {
            answer: count.get(),
            exactness: Exactness::Exact,
        })
    });
    let e = transactional(&moody, &mut rng()).unwrap_err();
    assert!(e.why.contains("differently"), "{e}");
}

#[test]
fn a_listed_error_nothing_triggers_is_caught() {
    let none: [Trigger<'_, u64>; 0] = [];
    let e = errors_tell_the_truth(&SCHEMA, &none).unwrap_err();
    assert_eq!(e.law, Law::ErrorsTellTheTruth);
    assert!(e.why.contains("ZeroLength"), "{e}");
}

#[test]
fn an_error_the_schema_does_not_list_is_caught() {
    let unlisted = Subject::new("unlisted", |c: &OpContext<'_>| {
        Err::<Done<u64>, _>(OpError::new(c.path().clone(), MakeDetail::BaseNotLevel))
    });
    let e = errors_tell_the_truth(
        &SCHEMA,
        &[
            Trigger {
                detail: "ZeroLength",
                subject: good(0),
            },
            Trigger {
                detail: "ZeroLength",
                subject: unlisted,
            },
        ],
    )
    .unwrap_err();
    assert!(e.why.contains("does not list"), "{e}");
}

#[test]
fn a_trigger_that_succeeds_is_caught_and_so_is_a_wrong_kind() {
    let e = errors_tell_the_truth(
        &SCHEMA,
        &[Trigger {
            detail: "ZeroLength",
            subject: good(3),
        }],
    )
    .unwrap_err();
    assert!(e.why.contains("succeeded"), "{e}");
    let e = errors_tell_the_truth(
        &WRONG,
        &[Trigger {
            detail: "ZeroLength",
            subject: good(0),
        }],
    )
    .unwrap_err();
    assert!(e.why.contains("schema says"), "{e}");
}

#[test]
fn an_error_is_named_as_the_schema_names_it() {
    let c = geom_op::OpContext::new(
        geom_op::OpPath::new(geom_op::OpId::from_bytes([1; 16])),
        Budget::UNLIMITED,
    );
    assert_eq!(detail_name(&run(&c, 0).unwrap_err()), "ZeroLength");
}

/// The seeded operation with a remedy: it needs `size` at most `limit`.
fn limited(size: u64, limit: u64, offered: u64) -> Subject<'static, u64> {
    Subject::new("limited", move |c: &OpContext<'_>| {
        if size > limit {
            return Err(
                OpError::new(c.path().clone(), MakeDetail::BaseNotLevel).with_remedy(
                    Remedy::ChangeInput {
                        input: "size",
                        to: Some((Bound::AtMost, Value::Count(u128::from(offered)))),
                    },
                ),
            );
        }
        Ok(Done {
            answer: size,
            exactness: Exactness::Exact,
        })
    })
}

#[test]
fn a_remedy_is_applied_at_its_number_and_one_step_past_it() {
    assert_eq!(remedy_holds("size", 9, &|n| limited(n, 4, 4)), Ok(4));
    // Too loose: the number offered fails.
    let e = remedy_holds("size", 9, &|n| limited(n, 4, 5)).unwrap_err();
    assert_eq!(e.law, Law::RemedyHolds);
    // Too tight: one step past the number still works.
    let e = remedy_holds("size", 9, &|n| limited(n, 4, 3)).unwrap_err();
    assert!(e.why.contains("one step past"), "{e}");
    // A remedy for another input is no remedy for this one.
    let e = remedy_holds("height", 9, &|n| limited(n, 4, 4)).unwrap_err();
    assert!(e.why.contains("no remedy"), "{e}");
}

/// A result of bound 4, accepted only as that or looser.
fn accepting(class: Option<Exactness>, offered: u64) -> Subject<'static, u64> {
    Subject::new("accepting", move |c: &OpContext<'_>| {
        let enough = matches!(
            class,
            Some(Exactness::Certified { bound, .. }) if bound.0 >= 4
        );
        if enough {
            return Ok(Done {
                answer: 1,
                exactness: certified(4),
            });
        }
        Err(OpError::new(
            c.path().clone(),
            MakeDetail::NotAccepted {
                would: certified(offered),
            },
        ))
    })
}

#[test]
fn an_offered_class_is_applied_and_one_step_tighter_refused() {
    assert_eq!(accept_holds(&|class| accepting(class, 4)), Ok(certified(4)));
    let e = accept_holds(&|class| accepting(class, 5)).unwrap_err();
    assert!(e.why.contains("tighter"), "{e}");
    let e = accept_holds(&|class| accepting(class, 3)).unwrap_err();
    assert!(e.why.contains("refused the class"), "{e}");
}

#[test]
fn exactness_never_rises_and_the_bounds_add() {
    let two = [certified(1), certified(2)];
    exactness_holds("sum", &two, certified(3)).unwrap();
    exactness_holds("looser", &two, certified(9)).unwrap();
    exactness_holds("exact", &[Exactness::Exact], Exactness::Exact).unwrap();
    for result in [Exactness::Exact, certified(2)] {
        let e = exactness_holds("risen", &two, result).unwrap_err();
        assert_eq!(e.law, Law::ExactnessIsNotLost, "{result:?}");
    }
}

#[test]
fn a_tolerance_that_rises_is_caught_and_a_new_entity_is_not() {
    let (a, b) = (NameId::from_bytes([1; 16]), NameId::from_bytes([2; 16]));
    no_tolerance_grows("same", &[(a, Steps(3))], &[(a, Steps(3))]).unwrap();
    no_tolerance_grows("lower", &[(a, Steps(3))], &[(a, Steps(2))]).unwrap();
    no_tolerance_grows("new", &[(a, Steps(3))], &[(b, Steps(9))]).unwrap();
    let e = no_tolerance_grows("risen", &[(a, Steps(3))], &[(a, Steps(4))]).unwrap_err();
    assert_eq!(e.law, Law::NoToleranceGrows);
}

#[test]
fn the_frames_edge_passes_and_a_step_past_it_is_typed() {
    let far = |n: i64| {
        Subject::new(format!("reach {n}"), move |c: &OpContext<'_>| {
            let p = geom_num::P3::new(n, 0, 0).map_err(|e| OpError::new(c.path().clone(), e))?;
            Ok(Done {
                answer: p,
                exactness: Exactness::Exact,
            })
        })
    };
    let edge = (1i64 << 40) - 1;
    range_holds(&far(edge), &far(edge + 1)).unwrap();
    // A refusal inside the frame, and a result published past it.
    assert_eq!(
        range_holds(&far(edge + 1), &far(edge + 1)).unwrap_err().law,
        Law::RangeHolds
    );
    assert_eq!(
        range_holds(&far(edge), &far(edge)).unwrap_err().law,
        Law::RangeHolds
    );
}

#[test]
fn an_example_is_held_to_what_the_call_gave() {
    example_holds(&SCHEMA, &[("face", 6)], Exactness::Exact).unwrap();
    let e = example_holds(&SCHEMA, &[("face", 7)], Exactness::Exact).unwrap_err();
    assert_eq!(e.law, Law::ExampleRuns);
    let e = example_holds(&SCHEMA, &[("vertex", 9)], Exactness::Exact).unwrap_err();
    assert!(e.why.contains("vertex"), "{e}");
    assert!(example_holds(&SCHEMA, &[], certified(1)).is_err());
    assert!(example_holds(&SILENT, &[], Exactness::Exact).is_err());
}
