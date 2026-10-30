//! Tests of the budgets in the operation contract (piece G83), through the public interface.
//!
//! What this file proves:
//!
//! - A charge of `count` things passes exactly when a loop charging them one at a time would, and
//!   leaves the meter with exactly that loop's use and stop, for counts and units up to
//!   `u64::MAX` (a property test against the loop, with a fixed seed, and the extremes by hand).
//! - A reservation charges its logical bytes before it allocates, fails on the budget before any
//!   allocation, and turns a count the target cannot address or the machine refuses into
//!   `OutOfMemory`, never an abort.
//! - A context's meter counts against its budget and reads its cancellation; a stop becomes the
//!   error of the same meaning at the context's path, with the budget record.
//! - A run publishes its body's result only when nothing stopped it: a body's own error passes
//!   through, a stop the body ignored is reported, and a cancellation asked for before it returns
//!   wins over its result.
//! - A budget failure is the same error, field for field, on the serial executor and on pools of
//!   1 to 8 threads, at chunk sizes 1, 7 and 4,096 (the pools on native targets only; the serial
//!   case runs everywhere, WebAssembly included, against the same expected values).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test cases and oracles; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken case must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::num::NonZeroUsize;

use geom_op::{
    Charge, ErrorKind, OpContext, OpError, OpErrorDetail, OpId, OpPath, RUN, RunDetail, reserve,
};
use geom_run::{
    Budget, BudgetKind, CancelToken, ChunkPlan, Exceeded, Exec, LogicalSize, Phase, Pool, Stop,
    Usage, WorkMeter,
};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

const PHASE: Phase = Phase::new("budgets");
const FACE: Charge = Charge::new(PHASE, "input face", 3);

fn config(cases: u32) -> Config {
    Config {
        cases,
        rng_seed: RngSeed::Fixed(0x0000_6783),
        failure_persistence: None,
        ..Config::default()
    }
}

fn path() -> OpPath {
    OpPath::new(OpId::from_bytes([0x83; 16]))
}

/// A meter that has already used `used` work units of a `cap`.
fn meter_after(cap: u64, used: u64) -> WorkMeter<'static> {
    let mut meter = WorkMeter::new(Budget::of_work(cap));
    let _ = meter.charge(used);
    meter
}

/// What a loop charging `units` for each of `count` things leaves: the result, the use and the
/// stop.
fn by_loop(cap: u64, used: u64, units: u64, count: u64) -> (Result<(), Stop>, Usage, Option<Stop>) {
    let mut meter = meter_after(cap, used);
    let mut result = meter.check().map_err(|s| s.reason());
    for _ in 0..count {
        if result.is_err() {
            break;
        }
        result = meter.charge(units).map_err(|s| s.reason());
    }
    (result, meter.used(), meter.stopped())
}

fn by_charge(
    cap: u64,
    used: u64,
    units: u64,
    count: u64,
) -> (Result<(), Stop>, Usage, Option<Stop>) {
    let mut meter = meter_after(cap, used);
    let result = Charge::new(PHASE, "thing", units)
        .charge(&mut meter, count)
        .map_err(|s| s.reason());
    (result, meter.used(), meter.stopped())
}

proptest! {
    #![proptest_config(config(512))]

    #[test]
    fn a_charge_is_the_loop_that_charges_one_at_a_time(
        cap in prop_oneof![0_u64..64, 0_u64..10_000],
        used in prop_oneof![0_u64..8, 0_u64..20_000],
        units in prop_oneof![Just(0_u64), 1_u64..5, 1_u64..1_000],
        count in 0_u64..600,
    ) {
        prop_assert_eq!(by_charge(cap, used, units, count), by_loop(cap, used, units, count));
    }
}

#[test]
fn a_charge_at_the_extremes_counts_exactly() {
    // The first thing over the cap is the last counted.
    let mut meter = meter_after(10, 0);
    let stop = FACE.charge(&mut meter, 5).unwrap_err();
    assert_eq!(stop.reason(), Stop::OverBudget(BudgetKind::Work));
    assert_eq!(meter.used().work, 12);
    // Exactly to the cap passes.
    let mut meter = meter_after(15, 0);
    FACE.charge(&mut meter, 5).unwrap();
    assert_eq!(meter.used().work, 15);
    assert_eq!(meter.stopped(), None);
    // A product past u64: the loop stops at the first unit past the cap, never saturating.
    let mut meter = meter_after(u64::MAX, 0);
    let huge = Charge::new(PHASE, "thing", u64::MAX);
    huge.charge(&mut meter, 1).unwrap();
    assert!(huge.charge(&mut meter, u64::MAX).is_err());
    assert_eq!(meter.used().work, 2 * u128::from(u64::MAX));
    let mut meter = meter_after(u64::MAX - 1, 0);
    assert!(
        Charge::new(PHASE, "thing", 2)
            .charge(&mut meter, u64::MAX)
            .is_err()
    );
    assert_eq!(meter.used().work, u128::from(u64::MAX) + 1);
    // Zero units never pass anything.
    let mut meter = meter_after(0, 0);
    Charge::new(PHASE, "thing", 0)
        .charge(&mut meter, u64::MAX)
        .unwrap();
    assert_eq!(meter.used().work, 0);
    // A stopped meter charges nothing more.
    let mut meter = meter_after(1, 2);
    assert!(FACE.charge(&mut meter, 0).is_err());
    assert_eq!(meter.used().work, 2);
    assert_eq!(FACE.phase, PHASE);
    assert_eq!(FACE.per, "input face");
    assert_eq!(FACE.units, 3);
}

#[derive(Debug)]
struct Edge;
impl LogicalSize for Edge {
    const LOGICAL_BYTES: u64 = 24;
}

/// One logical byte each, but a real page each: so a count the budget allows asks the machine
/// for more than any address space holds.
#[derive(Debug)]
struct Page {
    _bytes: [u8; 4096],
}
impl LogicalSize for Page {
    const LOGICAL_BYTES: u64 = 1;
}

#[test]
fn a_reservation_charges_its_logical_bytes_before_it_allocates() {
    let mut meter = WorkMeter::new(Budget::new(0, 240));
    let edges: Vec<Edge> = reserve(&mut meter, 10).unwrap();
    assert!(edges.capacity() >= 10);
    assert_eq!(meter.used(), Usage::new(0, 240));
    // One more edge passes the logical cap: refused before any allocation, deterministically.
    let stop = reserve::<Edge>(&mut meter, 1).unwrap_err();
    assert_eq!(stop.reason(), Stop::OverBudget(BudgetKind::LogicalBytes));
    assert_eq!(meter.used(), Usage::new(0, 264));
    // A stopped meter reserves nothing more.
    assert!(reserve::<Edge>(&mut meter, 0).is_err());
}

#[test]
fn an_allocation_the_target_cannot_make_is_out_of_memory_never_an_abort() {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    // 2^52 pages are 2^64 bytes: no target can address them, and the logical budget allows them.
    let stop = reserve::<Page>(&mut meter, 1 << 52).unwrap_err();
    assert_eq!(stop.reason(), Stop::OutOfMemory);
    assert_eq!(meter.stopped(), Some(Stop::OutOfMemory));
    assert_eq!(meter.used().logical_bytes, 1 << 52);
    let ctx = OpContext::new(path(), Budget::UNLIMITED);
    let error = ctx.stopped(&meter, stop, PHASE);
    assert_eq!(error.kind(), ErrorKind::OutOfMemory);
    assert!(!error.is_replayable());
}

#[test]
fn a_contexts_meter_counts_against_its_budget_and_reads_its_cancellation() {
    let token = CancelToken::new();
    let ctx = OpContext::new(path(), Budget::new(7, 9));
    let meter = ctx.meter();
    assert_eq!(meter.cap(), Budget::new(7, 9));
    assert!(meter.cancellation().is_none());
    let ctx = ctx.with_cancel(&token);
    let mut meter = ctx.meter();
    assert_eq!(meter.cap(), Budget::new(7, 9));
    assert!(meter.cancellation().is_some());
    meter.poll().unwrap();
    token.cancel();
    assert_eq!(meter.poll().unwrap_err().reason(), Stop::Cancelled);
}

#[test]
fn a_stop_becomes_the_error_of_the_same_meaning_at_the_contexts_path() {
    let at = path().step("cut").unwrap();
    let ctx = OpContext::new(at.clone(), Budget::of_work(4));
    let mut meter = ctx.meter();
    let stop = FACE.charge(&mut meter, 2).unwrap_err();
    let error = ctx.stopped(&meter, stop, PHASE);
    assert_eq!(error.op(), &at);
    assert_eq!(
        error.detail(),
        &OpErrorDetail::Run(RunDetail::BudgetExceeded(Exceeded {
            budget: Budget::of_work(4),
            kind: BudgetKind::Work,
            phase: PHASE,
            used: Usage::new(6, 0),
        }))
    );
    assert!(error.is_replayable());
    // Cancellation keeps its meaning and is never replayable.
    let token = CancelToken::new();
    let ctx = OpContext::new(at, Budget::UNLIMITED).with_cancel(&token);
    let mut meter = ctx.meter();
    token.cancel();
    let stop = meter.poll().unwrap_err();
    let error = ctx.stopped(&meter, stop, PHASE);
    assert_eq!(error.kind(), ErrorKind::Cancelled);
    assert!(!error.is_replayable());
}

#[test]
fn a_run_publishes_only_what_nothing_stopped() {
    let ctx = OpContext::new(path(), Budget::of_work(10));
    // A body that stays inside its budget publishes its result.
    let value = ctx
        .run(|meter| {
            FACE.charge(meter, 3)
                .map_err(|s| ctx.stopped(meter, s, PHASE))?;
            Ok(meter.used().work)
        })
        .unwrap();
    assert_eq!(value, 9);
    // The body's own error passes through untouched.
    let own = OpError::new(path(), RunDetail::Internal { what: "own" });
    assert_eq!(ctx.run(|_| Err::<(), _>(own.clone())).unwrap_err(), own);
    // A body that ignores a stop publishes nothing: the stop is reported, charged to RUN.
    let error = ctx
        .run(|meter| {
            let _ignored = FACE.charge(meter, 4);
            Ok(())
        })
        .unwrap_err();
    assert_eq!(
        error.detail(),
        &OpErrorDetail::Run(RunDetail::BudgetExceeded(Exceeded {
            budget: Budget::of_work(10),
            kind: BudgetKind::Work,
            phase: RUN,
            used: Usage::new(12, 0),
        }))
    );
    assert_eq!(RUN, Phase::new("operation"));
}

#[test]
fn a_cancellation_asked_for_before_a_run_returns_wins_over_its_result() {
    let token = CancelToken::new();
    let ctx = OpContext::new(path(), Budget::UNLIMITED).with_cancel(&token);
    let error = ctx
        .run(|_| {
            token.cancel();
            Ok(1)
        })
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Cancelled);
    assert_eq!(error.op(), &path());
    // Cancelled before it starts: the body still returns, and nothing is published.
    let error = ctx.run(|_| Ok(2)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Cancelled);
}

#[test]
fn a_stop_without_its_meter_keeps_its_meaning_or_is_a_defect() {
    let mut cancelled = WorkMeter::new(Budget::UNLIMITED);
    let token = CancelToken::new();
    token.cancel();
    let mut cancelled_meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    let stop = cancelled_meter.poll().unwrap_err();
    assert_eq!(
        OpErrorDetail::from(stop),
        OpErrorDetail::Run(RunDetail::Cancelled)
    );
    let stop = cancelled.out_of_memory();
    assert_eq!(
        OpErrorDetail::from(stop),
        OpErrorDetail::Run(RunDetail::OutOfMemory)
    );
    let mut over = WorkMeter::new(Budget::of_work(0));
    let stop = over.charge(1).unwrap_err();
    let detail = OpErrorDetail::from(stop);
    assert_eq!(detail.kind(), ErrorKind::Internal);
    assert_eq!(
        detail.to_string(),
        "internal error in the parallel helper (a budget stop lost its record; report it with \
         OpContext::stopped); nothing was published, please report it"
    );
}

/// A pool of `n` threads: tasks dealt round-robin, run in scoped threads.
#[cfg(not(target_family = "wasm"))]
struct Threads(usize);

#[cfg(not(target_family = "wasm"))]
impl Pool for Threads {
    #[allow(clippy::disallowed_methods, reason = "a test pool starts threads")]
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        let mut lanes: Vec<Vec<&mut &mut (dyn FnMut() + Send)>> =
            (0..self.0).map(|_| Vec::new()).collect();
        for (i, task) in tasks.iter_mut().enumerate() {
            lanes[i % self.0].push(task);
        }
        std::thread::scope(|scope| {
            for lane in lanes {
                scope.spawn(move || {
                    for task in lane {
                        task();
                    }
                });
            }
        });
    }
}

const SQUARE: Phase = Phase::new("square");
const ITEM: Charge = Charge::new(SQUARE, "item", 2);

/// An operation that squares 1,000 numbers, charging two units each, then one face per item
/// over 100.
fn squares(ctx: &OpContext<'_>, exec: Exec<'_>, size: usize) -> Result<u64, OpError> {
    let items: Vec<u64> = (0..1_000).collect();
    let plan = ChunkPlan::new(NonZeroUsize::new(size).unwrap());
    ctx.run(|meter| {
        let squares = exec
            .map(meter, SQUARE, plan, &items, |_, x, m| {
                ITEM.charge(m, 1)?;
                Ok(x * x)
            })
            .map_err(|e| OpError::new(ctx.path().clone(), e))?;
        let big = squares.iter().filter(|&&s| s > 100).count() as u64;
        FACE.charge(meter, big)
            .map_err(|s| ctx.stopped(meter, s, PHASE))?;
        Ok(squares.iter().sum())
    })
}

/// Budgets and the error each gives, the same on every target.
fn expected() -> Vec<(Budget, Result<u64, OpErrorDetail>)> {
    let exceeded = |budget: Budget, kind, phase, used| {
        Err(OpErrorDetail::Run(RunDetail::BudgetExceeded(Exceeded {
            budget,
            kind,
            phase,
            used,
        })))
    };
    vec![
        (Budget::of_work(4_967), Ok(332_833_500)),
        (
            Budget::of_work(4_966),
            exceeded(
                Budget::of_work(4_966),
                BudgetKind::Work,
                PHASE,
                Usage::new(4_967, 0),
            ),
        ),
        (
            Budget::of_work(1_999),
            exceeded(
                Budget::of_work(1_999),
                BudgetKind::Work,
                SQUARE,
                Usage::new(2_000, 0),
            ),
        ),
        (
            Budget::of_work(0),
            exceeded(
                Budget::of_work(0),
                BudgetKind::Work,
                SQUARE,
                Usage::new(2, 0),
            ),
        ),
    ]
}

fn check(exec: Exec<'_>, size: usize) {
    for (budget, want) in expected() {
        let ctx = OpContext::new(path(), budget);
        let got = squares(&ctx, exec, size).map_err(|e| *e.detail());
        assert_eq!(got, want, "budget {budget:?}, chunk size {size}");
    }
}

#[test]
fn a_budget_failure_is_the_same_on_the_serial_executor_at_every_chunk_size() {
    for size in [1, 7, 4_096] {
        check(Exec::serial(), size);
    }
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn a_budget_failure_is_the_same_on_every_pool() {
    for threads in [1, 2, 3, 8] {
        let pool = Threads(threads);
        for size in [1, 7, 4_096] {
            check(Exec::with_pool(&pool), size);
        }
    }
}
