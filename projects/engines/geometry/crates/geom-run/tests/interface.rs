//! Tests of promises `geom_run`'s public interface makes that the other files leave unchecked,
//! each through the interface alone (the mutation gate judges the crate by these files only).
//!
//! What this file proves:
//!
//! - A cancellation token reads as cancelled exactly after `cancel`, through its own method and
//!   through the `Cancel` trait.
//! - A meter charges up to its cap and stops on the first unit past it, recording what was used;
//!   `with_cancel` hands the meter its cancellation, which `cancellation` returns and its debug
//!   form shows; and a cancelled token stops a phase before any item runs.
//! - An executor shows whether it has a pool, returns that pool, and runs its chunks on it.
//! - An item's error prints what happened and exposes the function's own error as its source.
//! - The stable sort merges its sorted chunks to the end, and runs no merge round past the one
//!   that leaves one run: a cancellation asked during that last round does not cancel the sort.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "test counters; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::fmt;
use std::num::NonZeroUsize;
use std::sync::Mutex;

use geom_run::{
    Budget, BudgetKind, Cancel, CancelToken, ChunkPlan, Exceeded, Exec, ItemError, Phase, Pool,
    RunError, Stop, Stopped, Usage, WorkMeter,
};

const PHASE: Phase = Phase::new("interface");

fn plan(size: usize) -> ChunkPlan {
    ChunkPlan::new(NonZeroUsize::new(size).unwrap())
}

/// A pool that runs every task on the calling thread and counts the calls it gets.
struct Counting(Mutex<usize>);

impl Pool for Counting {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        *self.0.lock().unwrap() += 1;
        for task in tasks {
            task();
        }
    }
}

#[test]
fn a_token_is_cancelled_exactly_after_cancel() {
    let token = CancelToken::new();
    let source: &dyn Cancel = &token;
    assert!(!token.is_cancelled());
    assert!(!source.requested());
    token.cancel();
    assert!(token.is_cancelled());
    assert!(source.requested());
}

#[test]
fn a_meter_stops_on_the_first_unit_past_its_cap() {
    let cap = Budget::of_work(5);
    let mut meter = WorkMeter::new(cap);
    assert_eq!(meter.charge(2), Ok(()));
    assert_eq!(meter.charge(3), Ok(()));
    assert_eq!(meter.used(), Usage::new(5, 0));
    assert_eq!(meter.used().get(BudgetKind::Work), 5);
    let stopped = meter.charge(1).unwrap_err();
    assert_eq!(stopped.reason(), Stop::OverBudget(BudgetKind::Work));
    assert_eq!(meter.used().get(BudgetKind::Work), 6);
    assert_eq!(meter.stopped(), Some(Stop::OverBudget(BudgetKind::Work)));
    assert_eq!(
        meter.error::<Stopped>(stopped, PHASE),
        RunError::BudgetExceeded(Exceeded {
            budget: cap,
            kind: BudgetKind::Work,
            phase: PHASE,
            used: Usage::new(6, 0),
        })
    );
}

#[test]
fn a_meter_holds_the_cancellation_it_is_given() {
    let cap = Budget::of_work(7);
    let token = CancelToken::new();
    let plain = WorkMeter::new(cap);
    assert!(plain.cancellation().is_none());
    assert_eq!(
        format!("{plain:?}"),
        format!(
            "WorkMeter {{ cap: {cap:?}, used: {:?}, cancellable: false, stop: None }}",
            Usage::ZERO
        )
    );
    let meter = WorkMeter::new(cap).with_cancel(&token);
    assert!(meter.cancellation().is_some_and(|c| !c.requested()));
    token.cancel();
    assert!(meter.cancellation().is_some_and(Cancel::requested));
    assert_eq!(
        format!("{meter:?}"),
        format!(
            "WorkMeter {{ cap: {cap:?}, used: {:?}, cancellable: true, stop: None }}",
            Usage::ZERO
        )
    );
}

#[test]
fn a_cancelled_token_stops_a_phase_before_any_item_runs() {
    let token = CancelToken::new();
    token.cancel();
    let items = [1u32, 2, 3];
    let ran = Mutex::new(0usize);
    let pool = Counting(Mutex::new(0));
    for exec in [Exec::serial(), Exec::with_pool(&pool)] {
        let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
        let got = exec.map(&mut meter, PHASE, plan(1), &items, |_, x, _| {
            *ran.lock().unwrap() += 1;
            Ok::<u32, Stopped>(*x)
        });
        assert_eq!(got, Err(RunError::Cancelled), "{exec:?}");
    }
    assert_eq!(*ran.lock().unwrap(), 0);
}

#[test]
fn an_executor_shows_returns_and_uses_its_pool() {
    let pool = Counting(Mutex::new(0));
    assert_eq!(format!("{:?}", Exec::serial()), "Exec { pool: false }");
    assert!(Exec::serial().pool().is_none());
    let exec = Exec::with_pool(&pool);
    assert_eq!(format!("{exec:?}"), "Exec { pool: true }");
    let given = exec.pool().unwrap();
    let ptr = |p: &dyn Pool| std::ptr::from_ref(p).cast::<()>();
    assert_eq!(ptr(given), ptr(&pool));
    let items: Vec<u32> = (0..10).collect();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let got = exec.map(&mut meter, PHASE, plan(3), &items, |_, x, _| {
        Ok::<u32, Stopped>(x * 2)
    });
    assert_eq!(got, Ok(items.iter().map(|x| x * 2).collect()));
    assert!(
        *pool.0.lock().unwrap() > 0,
        "the pool was never asked to run a chunk"
    );
}

/// An error type of the function run, with a name to print.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bad;

impl fmt::Display for Bad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bad input")
    }
}

impl std::error::Error for Bad {}

#[test]
fn an_item_error_prints_what_happened_and_exposes_its_source() {
    use std::error::Error;

    let exceeded = Exceeded {
        budget: Budget::of_work(3),
        kind: BudgetKind::Work,
        phase: PHASE,
        used: Usage::new(4, 0),
    };
    let failed: ItemError<Bad> = ItemError::Failed(Bad);
    let over: ItemError<Bad> = ItemError::BudgetExceeded(exceeded);
    let total: ItemError<Bad> = ItemError::BatchTotal(exceeded);
    assert_eq!(failed.to_string(), "bad input");
    assert_eq!(over.to_string(), exceeded.to_string());
    assert_eq!(
        total.to_string(),
        format!("the batch's total ran out at or before this item: {exceeded}")
    );
    let source = failed.source().unwrap();
    assert_eq!(source.to_string(), "bad input");
    assert!(source.downcast_ref::<Bad>().is_some());
    assert!(over.source().is_none());
    assert!(total.source().is_none());
}

/// A stable sort of `items` by value in chunks of `size` on the serial executor, whose key
/// function counts its calls and cancels the meter's token at call `cancel_at`. Returns the order
/// and the number of calls.
fn counted_sort(
    items: &[u32],
    size: usize,
    cancel_at: Option<usize>,
) -> (Result<Vec<usize>, RunError>, usize) {
    let token = CancelToken::new();
    let calls = Mutex::new(0usize);
    let key = |x: &u32| {
        let mut n = calls.lock().unwrap();
        *n += 1;
        if Some(*n) == cancel_at {
            token.cancel();
        }
        *x
    };
    let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    let got = Exec::serial().sort_stable(&mut meter, PHASE, plan(size), items, key);
    let n = *calls.lock().unwrap();
    (got, n)
}

#[test]
fn the_sort_merges_its_chunks_to_the_end_and_no_further() {
    // Four items in two chunks of two: each chunk is sorted, then one merge round leaves one run.
    let items = [3u32, 2, 1, 0];
    let (got, calls) = counted_sort(&items, 2, None);
    assert_eq!(got, Ok(vec![3, 2, 1, 0]));
    assert!(calls < 100, "{calls} key calls for four items");
    // Ask to stop during the last comparison, which belongs to the last merge round: that round
    // checked the cancellation before it began, and no round follows it, so the sort finishes.
    let (got, again) = counted_sort(&items, 2, Some(calls));
    assert_eq!(again, calls);
    assert_eq!(got, Ok(vec![3, 2, 1, 0]));
    // One chunk wider than the input: sorted in its chunk, with no merge round at all.
    let (got, _) = counted_sort(&items, 8, None);
    assert_eq!(got, Ok(vec![3, 2, 1, 0]));
}
