//! Adversarial tests for piece G04, `geom-run`.
//!
//! Every test here tries to make the helper return something a plain serial loop would not, panic
//! where the crate promises no panics, or disagree with architecture chapters 23.4 and 23.10. The
//! oracles are written independently of the crate (no `common` helpers): plain loops over the
//! items, with `u128` running totals. The thread pools are written here too.
//!
//! Sections:
//!
//! 1. Extremes that must not panic ("No `unsafe`, no panics", lib.rs; 23.10 "Large allocations
//!    ... reserve fallibly").
//! 2. The phase rule of chapter 23.10 and the chunk-boundary rule of chapter 23.4.
//! 3. Exact oracles: exhaustive small cases and long random cases, every executor and chunk size.
//! 4. Forced scheduling: a slow low chunk while higher chunks finish, fail or pass the budget.
//! 5. Cancellation sources that are not monotone, nested runs, combining, plans at the extremes.
//! 6. Documented claims about panics and threads.
//!
//! Fixer's notes (the crate was rebuilt to chapters 23.4, 23.10 and 27 after this file found its
//! failures; the tests were ported to the new interface with their intent kept):
//!
//! - Threads now come from a caller-supplied `Pool` (23.4: kernel crates start no threads), so
//!   `Executor::Threads(n)` became `Exec::with_pool(&Threads(n))`, with `Threads` written below.
//! - Budgets follow chapter 23.10's phase rule, which this file's own section 2 asserts. The
//!   oracle, which stopped at the first item where the running total passed the budget, now
//!   follows that rule: each item is capped at R, the lowest-numbered failure of any kind wins,
//!   and a total over R fails only at the end of the phase. The two "huge chunk with a small
//!   budget" cases expected `needed: 11` from one-unit items, which the same rule forbids (the
//!   phase must run every item before a total over R can fail it); they now make item 10 cost 11
//!   units, over R on its own, which stops the huge chunk at item 10 just as early.
//! - `map_chunks` and `fold_chunks` are gone (23.4: "Chunk boundaries never matter", no
//!   reduce), so the chunk-function checks became checks of the per-item helpers, and the fold
//!   test became a test that combining is the caller's, on the calling thread.
//! - Counters are behind locks, not atomics: chapter 23.4 forbids atomic counters in kernel
//!   crates, and this crate's tests keep the same rule (`kernel_rules.rs` checks both).

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

use std::num::NonZeroUsize;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::thread;

use geom_run::{
    Budget, BudgetKind, Cancel, CancelToken, Chunk, ChunkPlan, Exceeded, Exec, Phase, Pool,
    RunError, Stopped, Usage, WorkMeter,
};

// ---------------------------------------------------------------------------------------------
// Helpers, pools and oracles
// ---------------------------------------------------------------------------------------------

/// `usize::MAX` zero-sized items: a real slice that costs no memory.
static HUGE: [(); usize::MAX] = [(); usize::MAX];

const PHASE: Phase = Phase::new("attack");

fn plan(size: usize) -> ChunkPlan {
    ChunkPlan::new(NonZeroUsize::new(size).unwrap())
}

/// `n` threads: the calling thread and up to `n − 1` scoped threads, each taking the next task.
/// A task's panic is passed on with its own payload. Where a thread cannot be started
/// (wasm32-wasip1 has no threads), the threads already started and the calling thread run every
/// task.
struct Threads(usize);

impl Pool for Threads {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        let helpers = self.0.min(tasks.len()).saturating_sub(1);
        let queue = Mutex::new(tasks.iter_mut());
        let work = || {
            loop {
                let next = queue.lock().unwrap().next();
                match next {
                    Some(task) => task(),
                    None => return,
                }
            }
        };
        let panicked = thread::scope(|scope| {
            // `spawn_scoped` reports a thread it cannot start, where `Scope::spawn` would panic.
            let handles: Vec<_> = (0..helpers)
                .map_while(|_| thread::Builder::new().spawn_scoped(scope, work).ok())
                .collect();
            let mut first = panic::catch_unwind(AssertUnwindSafe(&work)).err();
            for handle in handles {
                if let Err(payload) = handle.join() {
                    first.get_or_insert(payload);
                }
            }
            first
        });
        if let Some(payload) = panicked {
            panic::resume_unwind(payload);
        }
    }
}

/// Every task on the calling thread, in order.
struct Inline;

impl Pool for Inline {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        for task in tasks.iter_mut() {
            task();
        }
    }
}

static T0: Threads = Threads(0);
static T1: Threads = Threads(1);
static T2: Threads = Threads(2);
static T3: Threads = Threads(3);
static T4: Threads = Threads(4);
static T5: Threads = Threads(5);
static T8: Threads = Threads(8);
static T16: Threads = Threads(16);
static T512: Threads = Threads(512);
static T_MAX: Threads = Threads(usize::MAX);

/// Serial, then several thread counts.
fn executors() -> Vec<Exec<'static>> {
    vec![
        Exec::serial(),
        Exec::with_pool(&T2),
        Exec::with_pool(&T3),
        Exec::with_pool(&T8),
    ]
}

/// More thread counts, for the slower but larger cases.
fn all_executors() -> Vec<Exec<'static>> {
    vec![
        Exec::serial(),
        Exec::with_pool(&T0),
        Exec::with_pool(&T1),
        Exec::with_pool(&T2),
        Exec::with_pool(&T3),
        Exec::with_pool(&T4),
        Exec::with_pool(&T5),
        Exec::with_pool(&T8),
        Exec::with_pool(&T16),
    ]
}

/// Run `f`, turning a panic into `Err(its message)`.
fn no_panic<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(|payload| message(&*payload))
}

fn message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// One item of a test problem: a value charged `work` units, or a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
    Ok(u64),
    Err,
}

/// The phase-rule failure for a fresh meter with a work cap of `cap`, having used `used` units.
fn exceeded(cap: u64, used: u128) -> RunError<usize> {
    RunError::BudgetExceeded(Exceeded {
        budget: Budget::of_work(cap),
        kind: BudgetKind::Work,
        phase: PHASE,
        used: Usage::new(used, 0),
    })
}

/// Chapter 23.10's phase rule for per-item work, as a plain loop with a fresh meter of work cap
/// R = `cap`: values are item indices, failures name the item. Each item is capped at R; the
/// lowest-numbered failure of any kind (an item's error, or an item over R) wins, reporting what
/// the loop had used; otherwise a total over R fails at the end of the phase.
fn oracle_items(items: &[Item], cap: u64) -> Result<Vec<usize>, RunError<usize>> {
    let mut spent: u128 = 0;
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        match *item {
            Item::Err => return Err(RunError::Failed(i)),
            Item::Ok(work) => {
                spent += u128::from(work);
                if work > cap {
                    return Err(exceeded(cap, spent));
                }
                out.push(i);
            }
        }
    }
    if spent > u128::from(cap) {
        return Err(exceeded(cap, spent));
    }
    Ok(out)
}

fn item_fn(i: usize, item: &Item, meter: &mut WorkMeter<'_>) -> Result<usize, usize> {
    match *item {
        Item::Ok(work) => {
            meter.charge(work).map_err(|_| i)?;
            Ok(i)
        }
        Item::Err => Err(i),
    }
}

/// The per-item phase on `exec`, with a fresh meter of work cap `cap` (`None`: `u64::MAX`, the
/// largest cap there is; a total above it still fails).
fn run_items(
    exec: Exec<'_>,
    cap: Option<u64>,
    size: usize,
    items: &[Item],
) -> Result<Vec<usize>, RunError<usize>> {
    let mut meter = WorkMeter::new(Budget::of_work(cap.unwrap_or(u64::MAX)));
    exec.try_map(&mut meter, PHASE, plan(size), items, item_fn)
}

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), fixed seed: the same cases on every run.
struct SplitMix(u64);

impl SplitMix {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

fn spin(rounds: u64) -> u64 {
    let mut x: u64 = 0x1234_5678;
    for _ in 0..rounds {
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        x ^= x >> 29;
    }
    std::hint::black_box(x)
}

/// Item 10 costs 11 units, every other item 1: with R = 10, item 10 is over R on its own.
#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "the helper hands every function its item by reference"
)]
fn eleventh_over(i: usize, _: &(), meter: &mut WorkMeter<'_>) -> Result<usize, Stopped> {
    meter.charge(if i == 10 { 11 } else { 1 })?;
    Ok(i)
}

/// Fails at item 3; returns non-zero-sized values, so reserving room for a whole chunk would fail.
#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "the helper hands every function its item by reference"
)]
fn fail_at_three(i: usize, _: &(), _: &mut WorkMeter<'_>) -> Result<usize, usize> {
    if i == 3 { Err(i) } else { Ok(i) }
}

// ---------------------------------------------------------------------------------------------
// 1. Extremes that must not panic
// ---------------------------------------------------------------------------------------------

/// A chunk far larger than memory: item 10 is over the budget on its own, so the phase fails at
/// item 10 with what the loop used, 10 + 11 units. Reserving room for the whole chunk's values
/// (one `usize` per item) before the first item runs would fail or panic instead.
#[test]
fn serial_huge_chunk_with_small_budget_does_not_panic() {
    let got = no_panic(|| {
        let mut meter = WorkMeter::new(Budget::of_work(10));
        let plan = ChunkPlan::new(NonZeroUsize::MAX);
        Exec::serial().map(&mut meter, PHASE, plan, &HUGE[..], eleventh_over)
    });
    let expected = RunError::BudgetExceeded(Exceeded {
        budget: Budget::of_work(10),
        kind: BudgetKind::Work,
        phase: PHASE,
        used: Usage::new(21, 0),
    });
    assert_eq!(got, Ok(Err(expected)));
}

/// The same chunk, stopped by a failure at item 3 instead of a budget.
#[test]
fn serial_huge_chunk_with_early_failure_does_not_panic() {
    let got = no_panic(|| {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let plan = ChunkPlan::new(NonZeroUsize::MAX);
        Exec::serial().try_map(&mut meter, PHASE, plan, &HUGE[..], fail_at_three)
    });
    assert_eq!(got, Ok(Err(RunError::Failed(3))));
}

/// The serial executor answers these quickly; every pool must give the same answer, without
/// allocating per chunk of the whole problem or per requested thread.
#[test]
fn threaded_many_chunks_with_early_stop_does_not_panic() {
    let token = CancelToken::new();
    token.cancel();
    let over = RunError::BudgetExceeded(Exceeded {
        budget: Budget::of_work(10),
        kind: BudgetKind::Work,
        phase: PHASE,
        used: Usage::new(21, 0),
    });
    let mut wrong = Vec::new();
    for (threads, pool) in [(2, &T2), (8, &T8), (usize::MAX, &T_MAX)] {
        let exec = Exec::with_pool(pool);
        let got = no_panic(|| {
            let mut meter = WorkMeter::new(Budget::of_work(10));
            exec.map(&mut meter, PHASE, plan(1), &HUGE[..], eleventh_over)
        });
        if got != Ok(Err(over)) {
            wrong.push(format!("budget, Threads({threads}): {got:?}"));
        }
        let failed = no_panic(|| {
            let mut meter = WorkMeter::new(Budget::UNLIMITED);
            exec.try_map(&mut meter, PHASE, plan(1), &HUGE[..], fail_at_three)
        });
        if failed != Ok(Err(RunError::Failed(3))) {
            wrong.push(format!("failure, Threads({threads}): {failed:?}"));
        }
        let cancelled = no_panic(|| {
            let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
            exec.map(&mut meter, PHASE, plan(1), &HUGE[..], |_, (), _| Ok(()))
        });
        if cancelled != Ok(Err(RunError::Cancelled)) {
            wrong.push(format!("cancel, Threads({threads}): {cancelled:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// The serial references for the test above, so a failure there is clearly the pool path.
#[test]
fn serial_many_chunks_with_early_stop_is_fine() {
    let token = CancelToken::new();
    token.cancel();
    let mut meter = WorkMeter::new(Budget::of_work(10));
    let over = Exec::serial().map(&mut meter, PHASE, plan(1), &HUGE[..], eleventh_over);
    assert_eq!(
        over,
        Err(RunError::BudgetExceeded(Exceeded {
            budget: Budget::of_work(10),
            kind: BudgetKind::Work,
            phase: PHASE,
            used: Usage::new(21, 0),
        }))
    );
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let failed = Exec::serial().try_map(&mut meter, PHASE, plan(1), &HUGE[..], fail_at_three);
    assert_eq!(failed, Err(RunError::Failed(3)));
    let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    let cancelled = Exec::serial().map(&mut meter, PHASE, plan(1), &HUGE[..], |_, (), _| Ok(()));
    assert_eq!(cancelled, Err(RunError::Cancelled));
}

/// Huge thread counts on small problems: no overflow, same answer.
#[test]
fn absurd_thread_counts_on_small_problems() {
    static POOLS: [Threads; 4] = [
        Threads(usize::MAX),
        Threads(usize::MAX - 1),
        Threads(usize::MAX / 2),
        Threads(257),
    ];
    let items: Vec<Item> = (0..300).map(|i| Item::Ok(i % 5)).collect();
    let expected = oracle_items(&items, 400);
    for pool in &POOLS {
        let got = run_items(Exec::with_pool(pool), Some(400), 1, &items);
        assert_eq!(got, expected, "Threads({})", pool.0);
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Chapter rules
// ---------------------------------------------------------------------------------------------

/// Chapter 23.10, "Checked at fixed phase boundaries": "In a phase with remaining budget R, each
/// item runs with a cap of R. The phase fails with the lowest-numbered failure of any kind: an
/// item's own error, or an item over R. Otherwise, if the items' total exceeds R, it fails with
/// `BudgetExceeded` at the end of the phase." No item here is over R = 10 on its own, so the
/// lowest-numbered failure of any kind is item 2's own error.
#[test]
fn chapter_23_10_item_error_beats_a_total_over_r() {
    let items = [Item::Ok(6), Item::Ok(6), Item::Err];
    for executor in executors() {
        for size in [1, 2, 3] {
            let got = run_items(executor, Some(10), size, &items);
            assert_eq!(got, Err(RunError::Failed(2)), "{executor:?}, size {size}");
        }
    }
}

/// Chapter 23.10: with no failure, a total over R fails "at the end of the phase", and "the count
/// reported is what a serial loop would see": every item ran, so the whole total, 18.
#[test]
fn chapter_23_10_end_of_phase_overrun_reports_the_whole_total() {
    let items = [Item::Ok(6), Item::Ok(6), Item::Ok(6)];
    for executor in executors() {
        let got = run_items(executor, Some(10), 1, &items);
        assert_eq!(got, Err(exceeded(10, 18)), "{executor:?}");
    }
}

/// Chapter 23.4, "Chunk boundaries never matter": integer charges are order-free, so chunk sizes
/// "can be retuned for speed without a new kernel edition"; chapter 23.10: budget failures "may be
/// stored" as model state. The same items charged the same work give the same `BudgetExceeded`
/// for every chunk size. (Ported from `map_chunks`, which no longer exists: no helper lets a
/// result depend on where chunks start and end.)
#[test]
fn chapter_23_4_budget_failures_do_not_depend_on_chunk_size() {
    let items = [Item::Ok(6); 4];
    for executor in executors() {
        let reference = run_items(executor, Some(10), 1, &items);
        assert_eq!(reference, Err(exceeded(10, 24)));
        for size in [2, 3, 4] {
            let got = run_items(executor, Some(10), size, &items);
            assert_eq!(got, reference, "{executor:?}, size {size} vs size 1");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Exact oracles
// ---------------------------------------------------------------------------------------------

/// Every sequence of up to 4 items drawn from {0, 1, `u64::MAX`, failure}, every budget edge,
/// chunk sizes 1 to 3, every executor: the phase equals its serial oracle (so it is also
/// independent of the chunk size).
#[test]
fn exhaustive_small_cases_match_the_oracles() {
    let kinds = [Item::Ok(0), Item::Ok(1), Item::Ok(u64::MAX), Item::Err];
    let budgets = [
        None,
        Some(0),
        Some(1),
        Some(2),
        Some(u64::MAX - 1),
        Some(u64::MAX),
    ];
    let mut cases = 0usize;
    for len in 0..=4u32 {
        let total = kinds.len().pow(len);
        for code in 0..total {
            let mut rest = code;
            let items: Vec<Item> = (0..len)
                .map(|_| {
                    let kind = kinds[rest % kinds.len()];
                    rest /= kinds.len();
                    kind
                })
                .collect();
            for budget in budgets {
                let expected = oracle_items(&items, budget.unwrap_or(u64::MAX));
                for size in 1..=3 {
                    for executor in executors() {
                        let got = run_items(executor, budget, size, &items);
                        assert_eq!(got, expected, "{items:?} {budget:?} {size} {executor:?}");
                        cases += 1;
                    }
                }
            }
        }
    }
    // 341 sequences, 6 budgets, 3 chunk sizes, every executor.
    assert_eq!(cases, 341 * budgets.len() * 3 * executors().len());
}

/// Long random problems with failures, budgets at and around the total, and uneven cost (every
/// 97th item is slow, so chunks finish out of order).
#[test]
fn long_random_cases_match_the_oracles() {
    let mut rng = SplitMix(0x0004_6704);
    for case in 0..60 {
        let len = usize::try_from(rng.below(3000)).unwrap();
        let fail_one_in = [0, 3000, 300, 30][usize::try_from(rng.below(4)).unwrap()];
        let items: Vec<Item> = (0..len)
            .map(|_| {
                if fail_one_in > 0 && rng.below(fail_one_in) == 0 {
                    Item::Err
                } else {
                    Item::Ok(rng.below(50))
                }
            })
            .collect();
        let total: u64 = items
            .iter()
            .map(|item| match item {
                Item::Ok(w) => *w,
                Item::Err => 0,
            })
            .fold(0, u64::saturating_add);
        let budget = match rng.below(6) {
            0 => None,
            1 => Some(total),
            2 => Some(total.saturating_sub(1)),
            3 => Some(rng.below(total.max(1))),
            4 => Some(rng.below(60)),
            _ => Some(0),
        };
        let slow = |i: usize, item: &Item, meter: &mut WorkMeter<'_>| {
            if i.is_multiple_of(97) {
                spin(20_000);
            }
            item_fn(i, item, meter)
        };
        let expected = oracle_items(&items, budget.unwrap_or(u64::MAX));
        for size in [1, 7, 64, 4096] {
            for executor in all_executors() {
                let mut meter = WorkMeter::new(Budget::of_work(budget.unwrap_or(u64::MAX)));
                let got = executor.try_map(&mut meter, PHASE, plan(size), &items, slow);
                assert_eq!(got, expected, "case {case}, size {size}, {executor:?}");
            }
        }
    }
}

/// `f` sees the global item index and the item itself, exactly once per item, whatever the chunk
/// size; and no function ever sees a chunk.
#[test]
fn functions_see_the_right_indices_and_items() {
    let items: Vec<u64> = (0..1001).map(|i| i * 3 + 1).collect();
    for executor in all_executors() {
        for size in [1, 2, 7, 1000, 1001, 5000] {
            let calls = Mutex::new(0_usize);
            let mut meter = WorkMeter::new(Budget::UNLIMITED);
            let got = executor.map(&mut meter, PHASE, plan(size), &items, |i, x, _| {
                *calls.lock().unwrap() += 1;
                assert_eq!(*x, items[i]);
                Ok((i, *x))
            });
            let expected: Vec<(usize, u64)> = items.iter().copied().enumerate().collect();
            assert_eq!(got, Ok(expected), "{executor:?}, size {size}");
            let calls = calls.into_inner().unwrap();
            assert_eq!(calls, items.len(), "{executor:?}, size {size}");
        }
    }
}

/// Budgets near `u64::MAX` with several `u64::MAX` items inside one chunk: totals must not wrap.
#[test]
fn saturating_totals_inside_chunks() {
    let max = u64::MAX;
    let items = [
        Item::Ok(max),
        Item::Ok(0),
        Item::Ok(max),
        Item::Ok(max),
        Item::Ok(1),
        Item::Ok(0),
    ];
    for budget in [None, Some(max), Some(max - 1), Some(0)] {
        let expected = oracle_items(&items, budget.unwrap_or(max));
        for size in 1..=6 {
            for executor in all_executors() {
                assert_eq!(run_items(executor, budget, size, &items), expected);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 4. Forced scheduling
// ---------------------------------------------------------------------------------------------

/// Chunk 0 waits until a later item has started, then a little more, so higher chunks finish
/// (failing, going over R or passing the budget) while chunk 0 is still running. The answer must
/// still be the serial one.
#[test]
fn slow_first_chunk_while_higher_chunks_finish() {
    use std::time::{Duration, Instant};

    let max = u64::MAX;
    let tail = |items: &[Item]| {
        let mut v = items.to_vec();
        v.extend(std::iter::repeat_n(Item::Ok(1), 60));
        v
    };
    let cases: Vec<(Vec<Item>, Option<u64>)> = vec![
        (tail(&[Item::Err, Item::Ok(1), Item::Ok(max)]), Some(10)),
        (tail(&[Item::Ok(20), Item::Ok(1), Item::Ok(max)]), Some(10)),
        (tail(&[Item::Ok(5), Item::Ok(3), Item::Ok(100)]), Some(10)),
        (tail(&[Item::Ok(0), Item::Err, Item::Ok(max)]), Some(10)),
        (
            tail(&[Item::Ok(4), Item::Ok(3), Item::Ok(3), Item::Ok(1)]),
            Some(10),
        ),
        (
            tail(&[Item::Ok(0), Item::Ok(0), Item::Ok(0), Item::Err]),
            None,
        ),
        (
            tail(&[Item::Err, Item::Ok(0), Item::Ok(0), Item::Err]),
            None,
        ),
        (
            tail(&[Item::Ok(10), Item::Ok(0), Item::Ok(0), Item::Err]),
            Some(10),
        ),
        (tail(&[Item::Ok(11), Item::Ok(0), Item::Err]), Some(10)),
    ];
    for (items, budget) in cases {
        let expected = oracle_items(&items, budget.unwrap_or(max));
        for (threads, pool) in [(2, &T2), (3, &T3), (4, &T4)] {
            for size in [1, 2] {
                for _ in 0..3 {
                    let later_started = AtomicBool::new(false);
                    let f = |i: usize, item: &Item, meter: &mut WorkMeter<'_>| {
                        if i == 0 {
                            let start = Instant::now();
                            while !later_started.load(SeqCst)
                                && start.elapsed() < Duration::from_millis(200)
                            {
                                std::thread::yield_now();
                            }
                            std::thread::sleep(Duration::from_millis(3));
                        } else if i >= 2 {
                            later_started.store(true, SeqCst);
                        }
                        item_fn(i, item, meter)
                    };
                    let mut meter = WorkMeter::new(Budget::of_work(budget.unwrap_or(max)));
                    let exec = Exec::with_pool(pool);
                    let got = exec.try_map(&mut meter, PHASE, plan(size), &items, f);
                    assert_eq!(
                        got,
                        expected,
                        "{:?}, {budget:?}, Threads({threads}), size {size}",
                        &items[..4]
                    );
                }
            }
        }
    }
}

/// Ordered errors (chapter 23.4): lower-numbered items still run when a higher one fails first.
#[test]
fn lower_items_still_run_when_a_higher_failure_finishes_first() {
    use std::time::Duration;

    let items: Vec<u32> = (0..64).collect();
    for (threads, pool) in [(2, &T2), (4, &T4), (8, &T8)] {
        let low_ran = AtomicBool::new(false);
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let got = Exec::with_pool(pool).try_map(&mut meter, PHASE, plan(1), &items, |i, x, _| {
            if i == 0 {
                std::thread::sleep(Duration::from_millis(20));
                low_ran.store(true, SeqCst);
                Ok(*x)
            } else if i == 5 {
                Err("five")
            } else {
                Ok(*x)
            }
        });
        assert_eq!(got, Err(RunError::Failed("five")));
        assert!(low_ran.load(SeqCst), "item 0 never ran, Threads({threads})");
    }
}

// ---------------------------------------------------------------------------------------------
// 5. Cancellation, nesting, combining, plans
// ---------------------------------------------------------------------------------------------

/// A source that says "stop" exactly once, on its `on`th question, and "go" before and after.
/// cancel.rs: "The helper does not rely on that: whenever any question gets `true`, the whole
/// phase stops with `Cancelled`".
struct Blip {
    asked: Mutex<usize>,
    on: usize,
}

impl Cancel for Blip {
    fn requested(&self) -> bool {
        let mut asked = self.asked.lock().unwrap();
        *asked += 1;
        *asked == self.on
    }
}

#[test]
fn a_one_off_cancel_request_still_cancels() {
    let items: Vec<u32> = (0..200).collect();
    for on in [1, 2, 3, 50, 199] {
        for executor in all_executors() {
            let blip = Blip {
                asked: Mutex::new(0),
                on,
            };
            let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&blip);
            let got = executor.map(&mut meter, PHASE, plan(1), &items, |_, x, _| Ok(*x));
            assert_eq!(got, Err(RunError::Cancelled), "on {on}, {executor:?}");
        }
    }
}

/// Cancellation before the run, with a failure in chunk 0: the source is asked before chunk 0's
/// first item runs, so the answer is `Cancelled`, never the failure.
#[test]
fn cancel_before_the_run_wins_over_a_failure_in_chunk_zero() {
    let token = CancelToken::new();
    token.cancel();
    let items = [Item::Err, Item::Ok(1)];
    for executor in all_executors() {
        let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
        let got = executor.try_map(&mut meter, PHASE, plan(1), &items, item_fn);
        assert_eq!(got, Err(RunError::Cancelled), "{executor:?}");
    }
}

/// Runs nested in items, on pools nested in pools, give the serial answer, budgets included.
#[test]
fn nested_runs_match_serial() {
    const INNER: Phase = Phase::new("inner");
    let items: Vec<u64> = (0..500).collect();
    let nested = |outer: Exec<'static>, inner: Exec<'static>, cap: u64| {
        let mut meter = WorkMeter::new(Budget::of_work(cap));
        let answer = outer.try_map(&mut meter, PHASE, plan(37), &items, |i, x, meter| {
            meter.charge(1).map_err(|_| None)?;
            let sub: Vec<u64> = (0..(x % 40)).collect();
            let values = inner
                .map(meter, INNER, plan(3), &sub, |j, y, meter| {
                    meter.charge(1)?;
                    Ok(y * 2 + u64::try_from(j).unwrap())
                })
                .map_err(Some)?;
            Ok::<_, Option<RunError>>((i, values))
        });
        (answer, meter.used())
    };
    // A cap the phase passes, comparing values, and one whose total it passes at the end.
    for cap in [20_000, 10_000] {
        let reference = nested(Exec::serial(), Exec::serial(), cap);
        assert_eq!(reference.0.is_ok(), cap == 20_000);
        for outer in executors() {
            for inner in executors() {
                assert_eq!(
                    nested(outer, inner, cap),
                    reference,
                    "{outer:?} / {inner:?}"
                );
            }
        }
    }
}

/// Combining is the caller's (chapter 23.4: the helper offers no reduce, so no combine runs in
/// the thread or chunk order). The values come back in item order to the calling thread, where a
/// serial combine, even with an accumulator that is not `Send`, sees them in order once each, the
/// same on every executor and chunk size; a failed phase returns no values to combine.
#[test]
fn combining_is_left_to_the_caller_on_the_calling_thread() {
    use std::cell::RefCell;
    use std::rc::Rc;

    let items: Vec<u32> = (0..100).collect();
    for executor in all_executors() {
        for size in [1, 7, 100] {
            let log: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
            let mut meter = WorkMeter::new(Budget::UNLIMITED);
            let got = executor.map(&mut meter, PHASE, plan(size), &items, |i, _, _| Ok(i));
            for index in got.unwrap() {
                log.borrow_mut().push(index);
            }
            let expected: Vec<usize> = (0..items.len()).collect();
            assert_eq!(*log.borrow(), expected, "{executor:?}, size {size}");

            let mut meter = WorkMeter::new(Budget::of_work(0));
            let got = executor.map(&mut meter, PHASE, plan(size), &items, |i, _, meter| {
                meter.charge(1)?;
                Ok(i)
            });
            let over = RunError::BudgetExceeded(Exceeded {
                budget: Budget::of_work(0),
                kind: BudgetKind::Work,
                phase: PHASE,
                used: Usage::new(1, 0),
            });
            assert_eq!(got, Err(over));
        }
    }
}

/// Plans against a `u128` oracle at and around the extremes of `usize`.
#[test]
fn plans_match_a_wide_oracle_at_the_extremes() {
    fn oracle(len: usize, size: usize, index: usize) -> Option<(usize, usize)> {
        let start = index as u128 * size as u128;
        if start >= len as u128 {
            return None;
        }
        let end = (start + size as u128).min(len as u128);
        Some((
            usize::try_from(start).unwrap(),
            usize::try_from(end).unwrap(),
        ))
    }
    let max = usize::MAX;
    let mut values = vec![
        0,
        1,
        2,
        3,
        7,
        max / 2 - 1,
        max / 2,
        max / 2 + 1,
        max - 2,
        max - 1,
        max,
    ];
    let mut rng = SplitMix(0x0915);
    for _ in 0..40 {
        values.push(usize::try_from(rng.next_u64() >> rng.below(64)).unwrap_or(max));
    }
    for &len in &values {
        for &size in values.iter().filter(|&&s| s > 0) {
            let p = plan(size);
            let count = p.count(len);
            assert_eq!(count as u128, (len as u128).div_ceil(size as u128));
            let mut indices = values.clone();
            indices.extend([count.saturating_sub(1), count, count.saturating_add(1)]);
            for index in indices {
                let got = p.chunk(len, index).map(|c| (c.start(), c.end()));
                assert_eq!(
                    got,
                    oracle(len, size, index),
                    "len {len} size {size} i {index}"
                );
                if let Some(c) = p.chunk(len, index) {
                    assert_eq!(c.index(), index);
                    assert!(!c.is_empty());
                    assert_eq!(c.len(), c.end() - c.start());
                }
            }
            let mut chunks = p.chunks(len);
            assert_eq!(chunks.len(), count);
            if count > 0 {
                let first = chunks.next().unwrap();
                assert_eq!(first.start(), 0);
                assert_eq!(chunks.len(), count - 1);
            }
        }
    }
}

/// The iterator stays exhausted and keeps an exact length at the very end of `usize`.
#[test]
fn chunk_iterator_at_the_end_of_usize() {
    let p = plan(1);
    let len = usize::MAX;
    let last = p.chunk(len, usize::MAX - 1).unwrap();
    assert_eq!((last.start(), last.end()), (usize::MAX - 1, usize::MAX));
    assert_eq!(p.chunk(len, usize::MAX), None);
    let p = plan(usize::MAX);
    let mut chunks = p.chunks(usize::MAX);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks.next().map(Chunk::range), Some(0..usize::MAX));
    assert_eq!(chunks.next(), None);
    assert_eq!(chunks.next(), None);
    assert_eq!(chunks.len(), 0);
}

/// The public types can cross threads as a helper's callers need.
#[test]
fn public_types_are_thread_safe() {
    fn send_sync<T: Send + Sync>() {}
    fn copy<T: Copy>() {}
    send_sync::<Exec<'static>>();
    send_sync::<WorkMeter<'static>>();
    send_sync::<ChunkPlan>();
    send_sync::<Chunk>();
    send_sync::<CancelToken>();
    send_sync::<Budget>();
    send_sync::<RunError<String>>();
    send_sync::<geom_run::Chunks>();
    copy::<Exec<'static>>();
    copy::<Budget>();
    copy::<Usage>();
    copy::<Phase>();
    copy::<Exceeded>();
    copy::<RunError<u8>>();
}

// ---------------------------------------------------------------------------------------------
// 6. Documented claims about panics and threads
// ---------------------------------------------------------------------------------------------

/// exec.rs: a panic "run by a pool reaches the caller as the pool passes it on": the helper
/// neither catches nor rewraps it. This pool passes on the task's own payload, so the caller must
/// see "boom at item ...". Here only a helper thread panics.
#[test]
#[cfg_attr(
    any(target_os = "wasi", not(panic = "unwind")),
    ignore = "needs a helper thread and a panic that unwinds; wasm32-wasip1 has neither"
)]
fn a_helper_thread_panic_reaches_the_caller_with_its_own_payload() {
    use std::time::{Duration, Instant};

    let items: Vec<u64> = (0..64).collect();
    let main = std::thread::current().id();
    let helper_panicked = AtomicBool::new(false);
    let got = no_panic(|| {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        Exec::with_pool(&T2).map(&mut meter, PHASE, plan(1), &items, |i, x, _| {
            if std::thread::current().id() == main {
                let start = Instant::now();
                while !helper_panicked.load(SeqCst) && start.elapsed() < Duration::from_secs(2) {
                    std::thread::yield_now();
                }
                Ok(*x)
            } else {
                helper_panicked.store(true, SeqCst);
                panic!("boom at item {i}");
            }
        })
    });
    match got {
        Err(text) => assert!(text.starts_with("boom at item"), "payload was {text:?}"),
        Ok(answer) => panic!("no panic reached the caller: {answer:?}"),
    }
    // On the serial executor the panic unwinds through the helper as through a loop.
    let got = no_panic(|| {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        Exec::serial().map(&mut meter, PHASE, plan(1), &items, |i, x, _| {
            assert!(i != 7, "boom at item {i}");
            Ok(*x)
        })
    });
    assert_eq!(got.map(|_| ()), Err("boom at item 7".to_string()));
}

/// The helper starts no thread: with no pool, or a pool that runs tasks inline, every function
/// runs on the calling thread; with a pool of `n` threads, at most `n` functions run at once, and
/// never more than there are chunks. (Peak concurrency, not distinct thread ids: this test pool
/// starts fresh threads for every wave, as a pool may.)
#[test]
fn thread_counts_are_bounded_by_the_pool_and_by_the_chunk_count() {
    static INLINE: Inline = Inline;
    let items: Vec<u32> = (0..400).collect();
    let me = std::thread::current().id();
    let cases: [(Exec<'static>, usize, usize); 6] = [
        (Exec::serial(), 1, 1),
        (Exec::with_pool(&INLINE), 1, 1),
        (Exec::with_pool(&T3), 3, 1),
        (Exec::with_pool(&T16), 16, 150),
        (Exec::with_pool(&T2), 2, 1),
        (Exec::with_pool(&T512), 512, 7),
    ];
    for (exec, threads, size) in cases {
        // Functions running now, and the most that have run at once.
        let load = Mutex::new((0_usize, 0_usize));
        let elsewhere = AtomicBool::new(false);
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let got = exec.map(&mut meter, PHASE, plan(size), &items, |i, _, _| {
            {
                let mut load = load.lock().unwrap();
                load.0 += 1;
                load.1 = load.1.max(load.0);
            }
            if std::thread::current().id() != me {
                elsewhere.store(true, SeqCst);
            }
            spin(2_000);
            load.lock().unwrap().0 -= 1;
            Ok(i)
        });
        assert!(got.is_ok());
        let chunks = plan(size).count(items.len());
        let peak = load.into_inner().unwrap().1;
        assert!(
            peak <= threads.min(chunks),
            "{exec:?}, {chunks} chunks: {peak} at once"
        );
        if threads == 1 {
            assert!(!elsewhere.load(SeqCst), "{exec:?} left the calling thread");
        }
    }
}

/// Many threads at once: 512 threads over 512 one-item chunks still give the serial answer.
#[test]
fn hundreds_of_threads() {
    let items: Vec<Item> = (0..512).map(|i| Item::Ok(i % 3)).collect();
    for budget in [None, Some(300), Some(511), Some(1000), Some(1)] {
        let expected = oracle_items(&items, budget.unwrap_or(u64::MAX));
        assert_eq!(
            run_items(Exec::with_pool(&T512), budget, 1, &items),
            expected
        );
    }
}
