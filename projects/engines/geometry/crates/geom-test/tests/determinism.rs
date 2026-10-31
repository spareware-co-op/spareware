//! The determinism matrix for operation logs (piece G98; chapters 21 and 28.13).
//!
//! The board's done-when: "The replay corpus runs on every target, mode and thread count plus the
//! shipped-checker leg; golden work counts stored beside hashes, and a count change without a
//! version bump fails; memory-budget exhaustion identical on wasm32 and x86-64; a time stop writes
//! nothing."
//!
//! What this file proves:
//!
//! - The corpus's digest is the golden `geom-test/replay-corpus`. `golden run` carries that one
//!   digest through every target and build mode, and the release build as shipped runs the cheap
//!   checker, so it proves the checker changes nothing.
//! - Replays running side by side on 1, 2, 3 and 8 threads give that digest too, so nothing is
//!   shared between runs. No operation takes a pool yet; when one does, `Kernel` hands it one.
//! - Every family holds what it says: every primitive, moved and mirrored bodies, and a stop by
//!   each budget, whose error and hash are in the digest and so held across targets.
//! - A stored work count that moves fails, and `fixture diff` refuses it without a new version.
//! - A cancelled operation, a time stop, returns no result and writes nothing to a log.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::disallowed_methods,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};

use geom_format::operation::Operation;
use geom_make::{BoxInput, make_box};
use geom_num::Steps;
use geom_num::grid::P3;
use geom_op::{ErrorKind, OpContext, OpId, OpPath};
use geom_run::{Budget, Cancel};
use geom_test::fixture::calls::Call;
use geom_test::fixture::kernel::Kernel;
use geom_test::fixture::replay::{FAMILIES, digest, operation_of, seeded};
use geom_test::fixture::repo::{embedded, parse_all};
use geom_test::fixture::run::{Failure, Ran, Replayed, Verdict, replay, run};
use geom_test::fixture::{Expect, Fixture, Kind, Name, OpLog, Source, Speed, Spend, Status};
use geom_test::golden;

const EDITION: &str = "pre-edition";

fn stored() -> Vec<Fixture> {
    parse_all(&embedded()).unwrap()
}

#[test]
fn the_stored_fixtures_pass_and_the_box_is_the_box() {
    let fixtures = stored();
    assert!(fixtures.len() >= 3);
    for fixture in &fixtures {
        assert_eq!(
            run(fixture, EDITION, &mut Kernel::new()),
            Verdict::Pass,
            "{}",
            fixture.id
        );
    }
    let cube = fixtures.iter().find(|f| f.id == "make/0001-box").unwrap();
    let Expect::Ok { measures, .. } = cube.expect.values().next().unwrap() else {
        panic!()
    };
    assert_eq!(
        (
            measures.counts.faces,
            measures.counts.edges,
            measures.counts.vertices
        ),
        (6, 12, 8)
    );
    assert_eq!(measures.bounds, Some([[0, 0, 0], [6400, 3200, 1600]]));
    let stop = fixtures
        .iter()
        .find(|f| f.id == "budget/0001-logical-memory")
        .unwrap();
    assert!(matches!(
        stop.expect.values().next().unwrap(),
        Expect::Err {
            kind: Kind(ErrorKind::BudgetExceeded),
            ..
        }
    ));
}

#[test]
fn replay_corpus_golden() {
    let fixtures = stored();
    let checked = golden::check("geom-test/replay-corpus", || {
        (digest(&fixtures, &mut Kernel::new()).unwrap(), ())
    });
    checked.verdict().unwrap();
}

// WebAssembly has no threads (chapter 21).
#[cfg(not(target_family = "wasm"))]
#[test]
fn every_thread_count_gives_one_digest() {
    let fixtures = stored();
    let one = digest(&fixtures, &mut Kernel::new()).unwrap();
    for threads in golden::THREAD_COUNTS {
        std::thread::scope(|scope| {
            let runs: Vec<_> = (0..threads)
                .map(|_| scope.spawn(|| digest(&fixtures, &mut Kernel::new()).unwrap()))
                .collect();
            for run in runs {
                assert_eq!(run.join().unwrap(), one, "{threads} threads");
            }
        });
    }
}

#[test]
fn every_family_holds_what_it_says() {
    assert!(seeded("no-such").unwrap().is_none());
    let mut ran = BTreeSet::new();
    for family in FAMILIES {
        for log in seeded(family).unwrap().unwrap() {
            let got = replay(&mut Kernel::new(), &log);
            if family == "budget" {
                assert!(
                    matches!(
                        got,
                        Replayed::Failed {
                            kind: ErrorKind::BudgetExceeded,
                            ..
                        }
                    ),
                    "{got:?}"
                );
            } else {
                assert!(matches!(got, Replayed::Done { .. }), "{family}: {got:?}");
            }
            ran.extend(log.operations.iter().map(|o| o.name.clone()));
        }
    }
    for name in [
        "make.box",
        "make.cone",
        "make.cylinder",
        "make.prism",
        "make.sphere",
        "make.torus",
        "transform.mirror",
        "transform.move",
        "transform.rotate",
    ] {
        assert!(ran.contains(name), "{name}");
    }
}

#[test]
fn a_seeded_log_round_trips_and_repeats() {
    for family in FAMILIES {
        for log in seeded(family).unwrap().unwrap() {
            assert_eq!(OpLog::decode(&log.encode().unwrap()).unwrap(), log);
            assert_eq!(
                replay(&mut Kernel::new(), &log),
                replay(&mut Kernel::new(), &log)
            );
        }
    }
}

/// A fixture holding what the kernel gives for `log`.
fn fixture_of(log: OpLog) -> Fixture {
    let expect = match replay(&mut Kernel::new(), &log) {
        Replayed::Done {
            hash,
            measures,
            work,
        } => Expect::Ok {
            hash: Some(hash),
            work: Some(work),
            measures,
        },
        Replayed::Failed {
            kind,
            entities,
            work,
            ..
        } => Expect::Err {
            kind: Kind(kind),
            entities: entities.into_iter().map(Name).collect(),
            work: Some(work),
        },
        Replayed::Broken(why) => panic!("{why}"),
    };
    Fixture {
        id: "replay/0001-a".to_owned(),
        source: Source::ClosedForm {
            derivation: "d".to_owned(),
        },
        fixed_by: None,
        speed: Speed::Fast,
        expect: [((EDITION.to_owned(), log.versions()), expect)].into(),
        log,
        status: Status::Pass,
        history: Vec::new(),
    }
}

#[test]
fn a_work_count_that_moves_fails() {
    let log = seeded("make").unwrap().unwrap().remove(0);
    let good = fixture_of(log);
    assert_eq!(run(&good, EDITION, &mut Kernel::new()), Verdict::Pass);
    let mut moved = good.clone();
    let key = moved.expect.keys().next().unwrap().clone();
    let Some(Expect::Ok { work, .. }) = moved.expect.get_mut(&key) else {
        panic!()
    };
    let spent = work.as_mut().unwrap();
    spent.logical_bytes += 1;
    assert!(matches!(
        run(&moved, EDITION, &mut Kernel::new()),
        Verdict::Fail(Failure::Work { .. })
    ));
}

#[test]
fn an_exhausted_budget_is_a_stored_error() {
    for log in seeded("budget").unwrap().unwrap() {
        let f = fixture_of(log);
        assert!(matches!(
            f.expect.values().next().unwrap(),
            Expect::Err {
                kind: Kind(ErrorKind::BudgetExceeded),
                ..
            }
        ));
        assert_eq!(run(&f, EDITION, &mut Kernel::new()), Verdict::Pass);
    }
}

/// A caller's stop already requested: a time budget as the kernel sees one.
struct Stop(AtomicBool);

impl Cancel for Stop {
    fn requested(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[test]
fn a_time_stop_writes_nothing() {
    let stop = Stop(AtomicBool::new(true));
    let path = OpPath::new(OpId::from_bytes([1; 16]));
    let ctx = OpContext::new(path, Budget::new(1 << 40, 1 << 40)).with_cancel(&stop);
    let input = BoxInput {
        min: P3::new(0, 0, 0).unwrap(),
        size: [Steps(10), Steps(20), Steps(30)],
    };
    let error = make_box(&ctx, &input).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Cancelled);
    let operation: Operation =
        operation_of(1, &Call::Box(input), Budget::new(1 << 40, 1 << 40)).unwrap();
    let mut log = OpLog::new(Vec::new(), Vec::new());
    let none = Spend::default();
    // Neither stop is ever written, and nothing else is lost from the log.
    assert!(!log.record(&operation, &Ran::Failed(error, none)));
    let memory = geom_op::OpError::new(operation.op.clone(), geom_op::RunDetail::OutOfMemory);
    assert!(!log.record(&operation, &Ran::Failed(memory, none)));
    assert_eq!(log.operations.len(), 0);
    // Anything that ran, or failed in a way that replays, is.
    let typed = geom_op::OpError::new(operation.op.clone(), geom_num::NumError::Degenerate);
    assert!(log.record(&operation, &Ran::Failed(typed, none)));
    assert_eq!(log.operations, std::slice::from_ref(&operation));
    assert!(!log.record(&operation, &Ran::Refused("no".to_owned())));
}
