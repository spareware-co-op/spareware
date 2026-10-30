//! Budgets and cancellation: chapter 23.10's phase rule, the work meter, and the caller's `Cancel`.
//!
//! What this file proves:
//!
//! - The phase rule. A phase whose items use exactly what the meter has left passes and one unit
//!   less fails at the end of the phase, reporting the whole total. An item over R fails the phase
//!   at that item, reporting what a serial loop had used. An item's own error beats a total over
//!   R, and items above a failure count zero. R falls by each phase's total, and a stopped meter
//!   fails the next phase at once. All of this is identical on every executor and chunk size, for
//!   work and for logical bytes, at zero budgets and with charges of `u64::MAX`.
//! - The meter. It is a latch that keeps its first stop and adds nothing once stopped; it reads
//!   the cancellation at fixed points, every `POLL_EVERY` charges and when polled.
//! - Cancellation. A token cancelled before a phase gives `Cancelled` from every helper on every
//!   executor, except on empty input, where nothing runs. The source is asked at fixed points
//!   (before every chunk and every `POLL_EVERY` items). A cancellation during a phase gives
//!   `Cancelled`, never partial results, and a long item that charges stops soon after it.
//! - Errors print what happened, with remedies, and expose the function's error as their source.

#![allow(
    clippy::disallowed_methods,
    reason = "tests time how fast cancellation is seen; no kernel result depends on the clock"
)]
// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their cases and oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test cases and oracles; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use common::{
    Countdown, Fault, PHASE, Script, SplitMix, Tally, by_index, execs, phase_model, plan,
    run_scripted, same_everywhere,
};
use geom_run::{
    BatchBudget, Budget, BudgetKind, CancelToken, Exceeded, Exec, Interrupted, LogicalSize,
    POLL_EVERY, Phase, RunError, Stop, Stopped, Usage, WorkMeter,
};

/// The budget failure of a fresh meter with cap `cap`, in `kind`, having used `used`.
fn over(cap: Budget, kind: BudgetKind, used: Usage) -> RunError<Fault> {
    RunError::BudgetExceeded(Exceeded {
        budget: cap,
        kind,
        phase: PHASE,
        used,
    })
}

fn work(units: u128) -> Usage {
    Usage::new(units, 0)
}

#[test]
fn exactly_what_is_left_passes_and_one_unit_less_fails_at_the_end_of_the_phase() {
    let mut rng = SplitMix(11);
    let items: Vec<Script> = (0..500).map(|_| Script::ok(rng.below(10))).collect();
    let total: u64 = items
        .iter()
        .map(|item| item.work)
        .fold(0, u64::saturating_add);
    for size in [1, 2, 7, 64, 500, 1000] {
        let enough = same_everywhere(|e| run_scripted(e, &items, Budget::of_work(total), size));
        assert_eq!(enough.answer, Ok((0..500).collect()), "size {size}");
        assert_eq!((enough.used, enough.stopped), (work(total.into()), None));
        let cap = Budget::of_work(total - 1);
        let short = same_everywhere(|e| run_scripted(e, &items, cap, size));
        // Every item ran: the failure reports the whole total.
        let expected = over(cap, BudgetKind::Work, work(total.into()));
        assert_eq!(short.answer, Err(expected), "size {size}");
        assert_eq!(short.stopped, Some(BudgetKind::Work));
    }
}

#[test]
fn an_item_over_r_fails_the_phase_at_that_item() {
    // Ten items of one unit, then one of eleven: over R = 10 on its own.
    let mut items = vec![Script::ok(1); 10];
    items.push(Script::ok(11));
    items.extend([Script::ok(1); 50]);
    let cap = Budget::of_work(10);
    for size in [1, 3, 11, 64] {
        let got = same_everywhere(|e| run_scripted(e, &items, cap, size));
        assert_eq!(
            got.answer,
            Err(over(cap, BudgetKind::Work, work(21))),
            "size {size}"
        );
        assert_eq!((got.used, got.stopped), (work(21), Some(BudgetKind::Work)));
    }
}

#[test]
fn an_items_own_error_beats_a_total_over_r_and_items_above_count_zero() {
    // Chapter 23.10: no item is over R on its own, so the lowest-numbered failure of any kind is
    // item 2's own error, though the total had passed R at item 1.
    let items = [
        Script::ok(6),
        Script::ok(6),
        Script::fail(1),
        Script::ok(1000),
    ];
    let cap = Budget::of_work(10);
    for size in [1, 2, 3, 4] {
        let got = same_everywhere(|e| run_scripted(e, &items, cap, size));
        assert_eq!(
            got.answer,
            Err(RunError::Failed(Fault::Own(2))),
            "size {size}"
        );
        // What the loop used: items 0, 1 and 2, not item 3. It takes the operation over its
        // budget, so the meter stops and the next phase reports it.
        assert_eq!((got.used, got.stopped), (work(13), Some(BudgetKind::Work)));
    }

    // Item 10 fails; items above it count zero, even on executors that ran them.
    let mut items = vec![Script::ok(1); 64];
    items[10] = Script::fail(1);
    for size in [1, 4, 11, 16, 64, 100] {
        let got = same_everywhere(|e| run_scripted(e, &items, Budget::of_work(5), size));
        assert_eq!(
            got.answer,
            Err(RunError::Failed(Fault::Own(10))),
            "size {size}"
        );
        assert_eq!(got.used, work(11));
    }
}

#[test]
fn r_falls_by_each_phase_total_and_a_stopped_meter_fails_at_once() {
    const FIRST: Phase = Phase::new("first");
    const SECOND: Phase = Phase::new("second");
    const THIRD: Phase = Phase::new("third");
    for (name, exec) in execs() {
        let mut meter = WorkMeter::new(Budget::of_work(100));
        let thirty = [Script::ok(10); 3];
        let got = exec.try_map(&mut meter, FIRST, plan(2), &thirty, common::scripted);
        assert_eq!(got, Ok(vec![0, 1, 2]), "{name}");
        assert_eq!(meter.left(), Budget::of_work(70));

        // Each item of the next phase is capped at 70.
        let items = [Script::ok(70), Script::ok(71)];
        let got = exec.try_map(&mut meter, SECOND, plan(1), &items, common::scripted);
        let expected = Exceeded {
            budget: Budget::of_work(100),
            kind: BudgetKind::Work,
            phase: SECOND,
            used: work(30 + 70 + 71),
        };
        assert_eq!(got, Err(RunError::BudgetExceeded(expected)), "{name}");
        assert_eq!(meter.stopped(), Some(Stop::OverBudget(BudgetKind::Work)));

        // The meter has stopped: the next phase fails before any item runs.
        let ran = Tally::new();
        let got = exec.map(&mut meter, THIRD, plan(1), &[1, 2, 3], |_, x, _| {
            ran.add();
            Ok(*x)
        });
        let expected = Exceeded {
            phase: THIRD,
            ..expected
        };
        assert_eq!(got, Err(RunError::BudgetExceeded(expected)), "{name}");
        assert_eq!(ran.get(), 0);
    }
}

/// A kind of record with a fixed logical size of 48 bytes, whatever the target.
struct Record;

impl LogicalSize for Record {
    const LOGICAL_BYTES: u64 = 48;
}

#[test]
fn logical_memory_is_budgeted_like_work() {
    let cap = Budget::new(1_000, 100);
    // Item 2 reserves 101 bytes: over R on its own.
    let items = [
        Script::bytes(1, 50),
        Script::bytes(1, 0),
        Script::bytes(1, 101),
    ];
    let got = same_everywhere(|e| run_scripted(e, &items, cap, 1));
    let used = Usage::new(3, 151);
    assert_eq!(got.answer, Err(over(cap, BudgetKind::LogicalBytes, used)));
    // A total over R in bytes fails at the end of the phase.
    let items = [Script::bytes(1, 60), Script::bytes(1, 60)];
    let got = same_everywhere(|e| run_scripted(e, &items, cap, 1));
    let used = Usage::new(2, 120);
    assert_eq!(got.answer, Err(over(cap, BudgetKind::LogicalBytes, used)));
    // Work is reported before bytes when both pass at the end of the phase.
    let items = [Script::bytes(600, 60), Script::bytes(600, 60)];
    let got = same_everywhere(|e| run_scripted(e, &items, cap, 2));
    let used = Usage::new(1200, 120);
    assert_eq!(got.answer, Err(over(cap, BudgetKind::Work, used)));

    // Fixed sizes per kind, exact even past `u64::MAX`.
    let mut meter = WorkMeter::new(Budget::new(0, 480));
    assert_eq!(meter.reserve_for::<Record>(10), Ok(()));
    assert_eq!(meter.used(), Usage::new(0, 480));
    assert!(meter.reserve_for::<Record>(1).is_err());
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    assert!(meter.reserve_for::<Record>(u64::MAX).is_err());
    assert_eq!(meter.used().logical_bytes, u128::from(u64::MAX) * 48);
}

#[test]
fn budget_failures_do_not_depend_on_the_chunk_size() {
    let mut rng = SplitMix(12);
    let items: Vec<Script> = (0..300)
        .map(|_| Script {
            work: rng.below(5),
            bytes: rng.below(2),
            fails: rng.below(150) == 0,
        })
        .collect();
    let total: u64 = items
        .iter()
        .map(|item| item.work)
        .fold(0, u64::saturating_add);
    for work_cap in [0, 1, 2, 4, 5, total / 3, total - 1, total, total + 1] {
        for bytes_cap in [0, 1, 100, u64::MAX] {
            let cap = Budget::new(work_cap, bytes_cap);
            let expected = phase_model(&items, cap, PHASE);
            for size in [1, 2, 7, 64, 299, 300, 305] {
                let got = same_everywhere(|e| run_scripted(e, &items, cap, size));
                assert_eq!(got, expected, "{cap:?}, size {size}");
            }
        }
    }
}

#[test]
fn zero_budgets_and_zero_charges() {
    let free = [Script::ok(0); 50];
    let mut one = free;
    one[30] = Script::ok(1);
    for size in [1, 8, 50] {
        let got = same_everywhere(|e| run_scripted(e, &free, Budget::new(0, 0), size));
        assert_eq!(got.answer, Ok((0..50).collect()));
        let cap = Budget::new(0, 0);
        let got = same_everywhere(|e| run_scripted(e, &one, cap, size));
        assert_eq!(got.answer, Err(over(cap, BudgetKind::Work, work(1))));
    }
}

#[test]
fn huge_charges_are_counted_exactly() {
    let max = u64::MAX;
    let items = [Script::ok(max); 3];
    for size in [1, 2, 3] {
        // No item is over R = `u64::MAX` on its own; the total is, at the end of the phase.
        for cap in [Budget::of_work(max), Budget::UNLIMITED] {
            let got = same_everywhere(|e| run_scripted(e, &items, cap, size));
            let used = work(3 * u128::from(max));
            assert_eq!(
                got.answer,
                Err(over(cap, BudgetKind::Work, used)),
                "size {size}"
            );
        }
        let got = same_everywhere(|e| run_scripted(e, &items[..1], Budget::of_work(max), size));
        assert_eq!(got.answer, Ok(vec![0]));
    }
    // One meter charged `u64::MAX` twice: the second charge passes the cap, exactly.
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    assert_eq!(meter.charge(max), Ok(()));
    assert!(meter.charge(1).is_err());
    assert_eq!(meter.used(), work(u128::from(max) + 1));
}

#[test]
fn the_meter_is_a_latch_that_keeps_its_first_stop() {
    let mut meter = WorkMeter::new(Budget::new(5, 5));
    assert_eq!(meter.charge(5), Ok(()));
    let stopped = meter.charge(1).unwrap_err();
    assert_eq!(stopped.reason(), Stop::OverBudget(BudgetKind::Work));
    // Nothing more is added, and the first reason stays.
    assert_eq!(meter.reserve(1), Err(stopped));
    assert_eq!(meter.charge(0), Err(stopped));
    assert_eq!(meter.out_of_memory(), stopped);
    assert_eq!(meter.poll(), Err(stopped));
    assert_eq!(meter.check(), Err(stopped));
    assert_eq!(meter.used(), work(6));
    assert_eq!(meter.left(), Budget::new(0, 5));
    let error: RunError = meter.error(stopped, PHASE);
    let expected = Exceeded {
        budget: Budget::new(5, 5),
        kind: BudgetKind::Work,
        phase: PHASE,
        used: work(6),
    };
    assert_eq!(error, RunError::BudgetExceeded(expected));
    assert_eq!(meter.stop_error::<Stopped>(PHASE), Some(error));

    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    assert_eq!(meter.stop_error::<Stopped>(PHASE), None);
    let oom = meter.out_of_memory();
    assert_eq!(oom.reason(), Stop::OutOfMemory);
    assert_eq!(meter.error::<Stopped>(oom, PHASE), RunError::OutOfMemory);
    assert!(meter.charge(0).is_err());
}

#[test]
fn the_meter_reads_the_cancellation_every_poll_every_charges() {
    let source = Countdown::new(2);
    let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&source);
    let mut charges = 0;
    // Capped, so a meter that never reads its cancellation fails the count below instead of
    // charging for ever.
    while charges < 4 * POLL_EVERY && meter.charge(1).is_ok() {
        charges += 1;
    }
    // Questions at charges 64 and 128 say "not yet"; the third, at charge 192, says stop. The
    // stopping charge was counted before the read.
    assert_eq!(charges, 3 * POLL_EVERY - 1);
    assert_eq!(source.asked(), 3);
    assert_eq!(meter.stopped(), Some(Stop::Cancelled));

    // `poll` reads at once; `check` never reads.
    let source = Countdown::new(1);
    let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&source);
    assert_eq!(meter.check(), Ok(()));
    assert_eq!(meter.poll(), Ok(()));
    assert!(meter.poll().is_err());
    assert_eq!(source.asked(), 2);
}

#[test]
fn a_token_cancelled_before_the_phase_gives_cancelled_from_every_helper() {
    let token = CancelToken::new();
    token.cancel();
    let items: Vec<u32> = (0..100).collect();
    let id = |_: usize, x: &u32, _: &mut WorkMeter<'_>| Ok::<_, Stopped>(*x);
    let yes = |_: usize, _: &u32, _: &mut WorkMeter<'_>| Ok::<_, Stopped>(true);
    for (name, exec) in execs() {
        for size in [1, 7, 100, 1000] {
            let p = plan(size);
            let fresh = || WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
            let mut meter = fresh();
            assert_eq!(
                exec.map(&mut meter, PHASE, p, &items, id),
                Err(RunError::Cancelled)
            );
            assert_eq!(meter.stopped(), Some(Stop::Cancelled), "{name}");
            let got = exec.find_first(&mut fresh(), PHASE, p, &items, yes);
            assert_eq!(got, Err(RunError::Cancelled), "{name}");
            let got = exec.filter_ordered(&mut fresh(), PHASE, p, &items, yes);
            assert_eq!(got, Err(RunError::Cancelled), "{name}");
            let counts = [1_usize; 100];
            let got = exec.scan_exclusive::<()>(&mut fresh(), PHASE, p, &counts);
            assert_eq!(got, Err(RunError::Cancelled), "{name}");
            let got = exec.sort_stable::<_, _, (), _>(&mut fresh(), PHASE, p, &items, |x| *x);
            assert_eq!(got, Err(RunError::Cancelled), "{name}");
            let got = exec.count_then_fill(
                &mut fresh(),
                PHASE,
                p,
                &items,
                |_, _, _| Ok::<_, Stopped>(1),
                |_, x, out: &mut [u32], _, _| {
                    out.fill(*x);
                    Ok(())
                },
            );
            assert_eq!(got, Err(RunError::Cancelled), "{name}");
            let batch = BatchBudget::new(Budget::UNLIMITED);
            let got = exec.map_each(batch, Some(&token), PHASE, p, &items, by_index, id);
            assert_eq!(got, Err(Interrupted::Cancelled), "{name}");
        }
    }
}

#[test]
fn empty_input_runs_nothing_so_nothing_can_stop_it() {
    let token = CancelToken::new();
    token.cancel();
    let items: [u32; 0] = [];
    for (name, exec) in execs() {
        let mut meter = WorkMeter::new(Budget::new(0, 0)).with_cancel(&token);
        let got = exec.map(&mut meter, PHASE, plan(4), &items, |_, x, _| Ok(*x));
        assert_eq!(got, Ok(vec![]), "{name}");
        assert_eq!(meter.stopped(), None);
    }
}

#[test]
fn cancellation_is_asked_at_fixed_points() {
    let items: Vec<u32> = (0..1000).collect();
    for size in [1, 7, 63, 64, 65, 200, 1000, 5000] {
        // Before every chunk and every POLL_EVERY items inside it.
        let mut expected = 0_usize;
        for chunk in items.chunks(size) {
            expected += chunk.len().div_ceil(POLL_EVERY);
        }
        for (name, exec) in execs() {
            let never = Countdown::new(usize::MAX);
            let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&never);
            let got = exec.map(&mut meter, PHASE, plan(size), &items, |_, x, _| Ok(*x));
            assert_eq!(got, Ok(items.clone()));
            // Also on the broken pools: a chunk run twice is computed once, and a chunk a pool
            // skipped is computed on the calling thread.
            assert_eq!(never.asked(), expected, "size {size}, {name}");
        }
    }
}

#[test]
fn cancellation_during_a_phase_never_returns_partial_results() {
    let items: Vec<u64> = (0..2000).collect();
    for allowed in [0, 1, 5, 99] {
        for (name, exec) in execs() {
            // The source says "stop" from its (allowed + 1)th question on. With 200 chunks, some
            // chunk the answer needs is always refused.
            let countdown = Countdown::new(allowed);
            let ran = Tally::new();
            let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&countdown);
            let got = exec.map(&mut meter, PHASE, plan(10), &items, |_, x, _| {
                ran.add();
                Ok(*x)
            });
            assert_eq!(got, Err(RunError::Cancelled), "{allowed}, {name}");
            let ran = ran.get();
            if name == "serial" {
                // Exactly the chunks that were allowed to start.
                assert_eq!(ran, allowed * 10);
            }
        }
    }
}

#[test]
fn cancelling_from_inside_an_item() {
    let items: Vec<u64> = (0..5000).collect();
    for (name, exec) in execs() {
        let token = CancelToken::new();
        let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
        let got = exec.map(&mut meter, PHASE, plan(5), &items, |i, x, _| {
            if i == 12 {
                token.cancel();
            }
            Ok(x + 1)
        });
        match got {
            Err(RunError::Cancelled) => {}
            // On threads, every remaining chunk may already have started before the token was
            // set; then the answer is complete and correct. Anything partial is a failure.
            Ok(values) => {
                assert_ne!(name, "serial");
                assert_eq!(values, (1..=5000).collect::<Vec<_>>());
            }
            Err(other) => panic!("unexpected {other:?}"),
        }
        assert!(token.is_cancelled());
    }
}

#[test]
#[cfg_attr(
    target_os = "wasi",
    ignore = "needs a second thread to cancel while the item runs; wasm32-wasip1 has no threads"
)]
fn a_long_item_that_charges_stops_soon_after_a_cancellation() {
    let token = CancelToken::new();
    let items = [()];
    let started = AtomicBool::new(false);
    // Neither side waits for ever: a helper that never starts the item, or a meter that never sees
    // the cancellation, fails the test after this long instead of hanging it.
    let deadline = Instant::now() + Duration::from_secs(10);
    let (got, latency) = std::thread::scope(|scope| {
        let canceller = scope.spawn(|| {
            while !started.load(Ordering::SeqCst) && Instant::now() < deadline {
                std::thread::yield_now();
            }
            std::thread::sleep(Duration::from_millis(20));
            token.cancel();
            Instant::now()
        });
        let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
        let got: Result<Vec<()>, RunError> =
            Exec::serial().map(&mut meter, PHASE, plan(1), &items, |_, (), meter| {
                started.store(true, Ordering::SeqCst);
                // Endless work, charged one unit at a time: only the meter can stop it (or the
                // deadline, as out of memory, which the assertion below refuses).
                let mut charges = 0u64;
                loop {
                    meter.charge(1)?;
                    charges += 1;
                    if charges.is_multiple_of(4096) && Instant::now() > deadline {
                        return Err(meter.out_of_memory());
                    }
                }
            });
        let returned = Instant::now();
        let cancelled_at = canceller.join().unwrap();
        (got, returned.saturating_duration_since(cancelled_at))
    });
    assert_eq!(got, Err(RunError::Cancelled));
    // A generous bound for a loaded test machine; the benchmark measures the real latency.
    assert!(
        latency < Duration::from_secs(1),
        "stopped {latency:?} after the cancellation"
    );
}

#[test]
fn cancel_token_basics() {
    let token = CancelToken::default();
    assert!(!token.is_cancelled());
    token.cancel();
    token.cancel();
    assert!(token.is_cancelled());
    assert!(!CancelToken::new().is_cancelled());
}

#[derive(Debug, PartialEq)]
struct Broken;

impl std::fmt::Display for Broken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("broken edge")
    }
}

impl std::error::Error for Broken {}

#[test]
fn errors_say_what_happened() {
    let cancelled: RunError<Broken> = RunError::Cancelled;
    assert_eq!(
        cancelled.to_string(),
        "cancelled by the caller; nothing was published"
    );
    let oom: RunError<Broken> = RunError::OutOfMemory;
    let text = "the machine refused a large allocation; nothing was published";
    assert_eq!(oom.to_string(), text);
    let exceeded = Exceeded {
        budget: Budget::new(10, 99),
        kind: BudgetKind::Work,
        phase: Phase::new("blend faces"),
        used: Usage::new(12, 3),
    };
    let budget: RunError<Broken> = RunError::BudgetExceeded(exceeded);
    let text = "phase `blend faces` passed its work budget of 10 with 12 used; raise the budget, \
                split the call, or ask for a preview";
    assert_eq!(budget.to_string(), text);
    let failed = RunError::Failed(Broken);
    assert_eq!(failed.to_string(), "broken edge");
    let source = std::error::Error::source(&failed).map(ToString::to_string);
    assert_eq!(source.as_deref(), Some("broken edge"));
    assert!(std::error::Error::source(&budget).is_none());
    let internal: RunError<Broken> = RunError::Internal("an example");
    assert!(internal.to_string().contains("an example"));
    assert_eq!(
        RunError::<Broken>::from(Interrupted::Cancelled),
        RunError::Cancelled
    );
    assert_eq!(
        RunError::<Broken>::from(Interrupted::OutOfMemory),
        RunError::OutOfMemory
    );
    assert_eq!(
        RunError::<Broken>::from(Interrupted::Internal("an example")),
        RunError::Internal("an example")
    );
    let internal = Interrupted::Internal("an example").to_string();
    assert!(internal.contains("an example") && internal.contains("please report it"));
    let stopped = WorkMeter::new(Budget::new(0, 0)).charge(1).unwrap_err();
    assert_eq!(
        stopped.to_string(),
        "the work meter stopped: over its work budget"
    );
}
