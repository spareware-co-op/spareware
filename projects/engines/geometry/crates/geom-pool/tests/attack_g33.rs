//! Attacks on piece G33's native pool: `geom_pool::native::Rayon`, rayon behind `geom-run`'s
//! `Pool` trait (chapter 23.4 of the kernel architecture).
//!
//! The claims attacked:
//!
//! - **The pool's contract** (`geom_run::Pool`): every task it is handed runs exactly once, for
//!   any number of tasks, on pools of 1, 2, 3 and 8 threads.
//! - **Re-entrant**: a task that calls the helper again with the same pool finishes, on one thread
//!   too, and the nested answers are the serial ones.
//! - **No panic of its own, and a task's panic passed on**: a task that panics makes `run_all`
//!   panic with that task's payload, only after every other task has run.
//! - **The same answer as no pool at all** (23.4, and the board's "identical hashes at 1, 2, 3 and
//!   8 threads ... with chunk sizes forced to 1, 7 and 4,096"): every helper of `geom_run::Exec`
//!   (`map`, `try_map` with budgets, `find_first`, `filter_ordered`, `map_each` with a batch total,
//!   `scan_exclusive`, `count_then_fill`, `sort_stable`), on seeded inputs, gives on rayon pools
//!   of 1, 2, 3 and 8 threads, at chunk sizes 1, 7, 64 and 4,096, exactly the serial executor's
//!   answer, failures and the meter left behind included. `geom-run`'s own golden test holds the
//!   helper's answers to one SHA-256 digest at the same thread counts and chunk sizes with pools
//!   that start threads directly; this holds the rayon pool to the same answers.
//!
//! Native targets only: WebAssembly has no threads here, so the pool does not exist there and the
//! kernel runs serially, with the same answer.
//!
//! Seeds: the inputs come from `geom_test`'s `SplitMix64` from a printed seed;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17).
#![cfg(not(target_family = "wasm"))]
// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their cases.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    reason = "test cases; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken case must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::num::{NonZeroU64, NonZeroUsize};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

use geom_pool::native::Rayon;
use geom_run::{BatchBudget, Budget, ChunkPlan, Exec, Phase, Pool, RunError, Stopped, WorkMeter};
use geom_test::SplitMix64;
use geom_test::seed::{Seed, resolve};

/// The thread counts chapter 21 asks for.
const THREADS: [usize; 4] = [1, 2, 3, 8];

/// The chunk sizes chapter 23.4 forces, 1, 7 and 4,096, and 64 between them.
const SIZES: [usize; 4] = [1, 7, 64, 4_096];

/// The phase every helper call runs as.
const PHASE: Phase = Phase::new("attack g33");

/// A rayon pool of `n` threads.
fn pool(n: usize) -> Rayon {
    Rayon::new(NonZeroUsize::new(n).unwrap()).unwrap()
}

/// A plan of chunk size `size`.
fn plan(size: usize) -> ChunkPlan {
    ChunkPlan::new(NonZeroUsize::new(size).unwrap())
}

/// Uniform in 0..n, n > 0.
fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

#[test]
fn attack_the_pool_has_the_threads_it_was_built_with() {
    for n in THREADS {
        assert_eq!(pool(n).threads(), n);
    }
}

/// Runs `count` tasks on `pool`, each adding one to its own slot, task `fail` (if any) then
/// panicking. Returns every slot's count, and the message `run_all` panicked with, if it did.
fn run_counted(pool: &Rayon, count: usize, fail: Option<usize>) -> (Vec<u32>, Option<String>) {
    let runs: Vec<Mutex<u32>> = (0..count).map(|_| Mutex::new(0)).collect();
    let outcome = {
        let mut closures: Vec<Box<dyn FnMut() + Send + '_>> = runs
            .iter()
            .enumerate()
            .map(|(i, slot)| {
                Box::new(move || {
                    *slot.lock().unwrap() += 1;
                    assert!(Some(i) != fail, "task {i} fails");
                }) as Box<dyn FnMut() + Send + '_>
            })
            .collect();
        let mut tasks: Vec<&mut (dyn FnMut() + Send)> = closures
            .iter_mut()
            .map(|c| &mut **c as &mut (dyn FnMut() + Send))
            .collect();
        catch_unwind(AssertUnwindSafe(|| pool.run_all(&mut tasks)))
    };
    let message = outcome.err().map(|payload| {
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_default()
    });
    // A slot whose task panicked after counting is poisoned; its count is still right.
    let counts = runs
        .into_iter()
        .map(|slot| {
            slot.into_inner()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        })
        .collect();
    (counts, message)
}

/// Every task runs exactly once, for any number of tasks and threads.
#[test]
fn attack_every_task_runs_exactly_once() {
    for n in THREADS {
        let pool = pool(n);
        for count in [0, 1, 2, 7, 64, 1_000] {
            let (counts, message) = run_counted(&pool, count, None);
            assert_eq!(message, None, "{n} threads, {count} tasks");
            assert!(
                counts.iter().all(|&r| r == 1),
                "{n} threads, {count} tasks: {counts:?}"
            );
        }
    }
}

/// A task that calls the helper again with the same pool finishes, on one thread too, with the
/// serial answer.
#[test]
fn attack_the_pool_is_reentrant() {
    let outer: Vec<u64> = (0..64).collect();
    let serial = nested(Exec::serial(), &outer);
    for n in THREADS {
        let pool = pool(n);
        assert_eq!(
            nested(Exec::with_pool(&pool), &outer),
            serial,
            "{n} threads"
        );
    }
}

/// Each outer item sums the squares of 0..=item through a nested helper call on the same executor.
fn nested(exec: Exec<'_>, outer: &[u64]) -> Vec<u64> {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    exec.map(&mut meter, PHASE, plan(3), outer, |_, &item, meter| {
        let inner: Vec<u64> = (0..=item).collect();
        // The meter is unlimited and nothing cancels it, so the nested call always answers.
        let squares = exec
            .map(meter, Phase::new("inner"), plan(2), &inner, |_, &x, _| {
                Ok(x * x)
            })
            .unwrap();
        Ok(squares.iter().sum())
    })
    .unwrap()
}

/// A task's panic is passed on, with its payload, after every other task has run.
#[test]
fn attack_a_tasks_panic_is_passed_on_after_the_others_finish() {
    for n in THREADS {
        let pool = pool(n);
        let (counts, message) = run_counted(&pool, 50, Some(17));
        assert_eq!(message.as_deref(), Some("task 17 fails"), "{n} threads");
        assert!(
            counts.iter().all(|&r| r == 1),
            "{n} threads: every task ran once: {counts:?}"
        );
        // The pool still works afterwards.
        assert_eq!(nested(Exec::with_pool(&pool), &[3, 4]), vec![14, 30]);
    }
}

/// An item: the work it charges, whether it fails with its own error, and a key.
#[derive(Clone, Copy, Debug)]
struct Item {
    work: u64,
    fails: bool,
    key: u64,
}

/// What one run of every helper gives: each answer, as text, in a fixed order.
fn every_helper(exec: Exec<'_>, size: usize, items: &[Item], cap: u64, total: u64) -> Vec<String> {
    let mut out = Vec::new();
    let job =
        |i: usize, item: &Item, meter: &mut WorkMeter<'_>| -> Result<u64, Result<usize, Stopped>> {
            meter.charge(item.work).map_err(Err)?;
            if item.fails {
                return Err(Ok(i));
            }
            Ok(item.work * 3 + i as u64)
        };
    let mut meter = WorkMeter::new(Budget::of_work(cap));
    let mapped = exec.try_map(&mut meter, PHASE, plan(size), items, job);
    out.push(format!(
        "try_map {mapped:?} {:?} {:?}",
        meter.used(),
        meter.stopped()
    ));
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let found =
        exec.find_first::<_, Stopped, _>(&mut meter, PHASE, plan(size), items, |_, item, _| {
            Ok(item.work % 5 == 4)
        });
    out.push(format!("find_first {found:?}"));
    let kept =
        exec.filter_ordered::<_, Stopped, _>(&mut meter, PHASE, plan(size), items, |_, item, _| {
            Ok(item.key % 3 == 0)
        });
    out.push(format!("filter_ordered {kept:?}"));
    let budget = BatchBudget::new(Budget::of_work(cap / 4 + 1)).with_total(Budget::of_work(total));
    let each = exec.map_each(
        budget,
        None,
        PHASE,
        plan(size),
        items,
        |_, item| item.key,
        job,
    );
    out.push(format!("map_each {each:?}"));
    let counts: Vec<usize> = items.iter().map(|item| (item.work % 4) as usize).collect();
    let scanned = exec.scan_exclusive::<Stopped>(&mut meter, PHASE, plan(size), &counts);
    out.push(format!("scan_exclusive {scanned:?}"));
    let filled = exec.count_then_fill::<_, u64, Stopped, _, _>(
        &mut meter,
        PHASE,
        plan(size),
        items,
        |_, item, _| Ok((item.work % 4) as usize),
        |i, item, out, place, _| {
            for (k, slot) in out.iter_mut().enumerate() {
                *slot = item.key * 1_000 + (place.first + k) as u64 + i as u64;
            }
            Ok(())
        },
    );
    out.push(format!("count_then_fill {filled:?}"));
    let sorted =
        exec.sort_stable::<_, _, Stopped, _>(&mut meter, PHASE, plan(size), items, |item| item.key);
    out.push(format!("sort_stable {sorted:?} {:?}", meter.used()));
    out
}

/// Every helper gives the serial answer on rayon pools of 1, 2, 3 and 8 threads, at every forced
/// chunk size, on seeded inputs whose budgets make every outcome happen.
#[test]
fn attack_rayon_pools_give_the_serial_answer_at_every_thread_count_and_chunk_size() {
    let mut rng =
        SplitMix64::new(resolve("rayon against serial", Seed(0x6733_0000_5eed_0253)).unwrap());
    let pools: Vec<(usize, Rayon)> = THREADS.iter().map(|&n| (n, pool(n))).collect();
    let rounds = if cfg!(debug_assertions) { 8 } else { 48 };
    let mut outcomes = [0_u32; 3];
    for round in 0..rounds {
        let n = below(&mut rng, 600) as usize;
        let items: Vec<Item> = (0..n)
            .map(|_| Item {
                work: below(&mut rng, 10),
                // Failing items only in odd rounds, so even rounds can pass or run out of budget.
                fails: round % 2 == 1 && below(&mut rng, 150) == 0,
                key: below(&mut rng, 40),
            })
            .collect();
        let need: u64 = items.iter().map(|item| item.work).sum();
        let cap = match round % 3 {
            0 => u64::MAX,
            1 => need.saturating_sub(below(&mut rng, 30)),
            _ => below(&mut rng, need + 1),
        };
        let total = below(&mut rng, need + 2);
        let serial = every_helper(Exec::serial(), 1, &items, cap, total);
        outcomes[if serial[0].starts_with("try_map Ok(") {
            0
        } else if serial[0].contains("BudgetExceeded") {
            1
        } else {
            2
        }] += 1;
        for (threads, pool) in &pools {
            for size in SIZES {
                let got = every_helper(Exec::with_pool(pool), size, &items, cap, total);
                assert_eq!(
                    got, serial,
                    "round {round}: {threads} threads, chunk size {size}"
                );
            }
        }
    }
    assert!(
        outcomes.iter().all(|&o| o > 0),
        "passing, budget and own-error phases all ran: {outcomes:?}"
    );
    // The cancellation path: a cancelled run gives Cancelled on every pool, as on none.
    let token = geom_run::CancelToken::new();
    token.cancel();
    let items = vec![1_u64; 100];
    for (threads, pool) in &pools {
        let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
        let got = Exec::with_pool(pool).map(&mut meter, PHASE, plan(7), &items, |_, &x, _| Ok(x));
        assert_eq!(got, Err(RunError::Cancelled), "{threads} threads");
    }
}
