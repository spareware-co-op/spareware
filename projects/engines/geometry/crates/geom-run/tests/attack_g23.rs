//! Attacks on piece G23: budgets, the work meter and cancellation in `geom-run` (chapter 23.10 of
//! the kernel architecture). Every test tries to break a claim of the piece's done-when or of the
//! crate's guarantees.
//!
//! Most claims here need no model of the helper, so none is written: the attacks are metamorphic.
//! An answer must not change when only something that may not matter changes:
//!
//! - **Serial and parallel runs give the same `BudgetExceeded` and counts** (done-when): a phase
//!   or a batch gives the same result, the same failure with the same `Exceeded` record, and
//!   leaves the operation's meter with the same use and the same stop, on the serial executor and
//!   on pools of 2, 3 and 8 threads and one that runs tasks backwards, at chunk sizes 1, 7, 64
//!   and 4,096.
//! - **An item's verdict never depends on the other items** (done-when): under per-item budgets,
//!   each item of a batch gets exactly the result it gets alone, and the same when the batch is
//!   reversed.
//!
//! Where an exact answer is needed it is worked out by hand in the test, from chapter 23.10's
//! words:
//!
//! - **The phase rule**: each item runs capped at what the phase has left; the lowest-numbered
//!   failure of any kind wins, an item's own error or an item over the cap; and a total over what
//!   is left fails only at the end of the phase, after every item ran, counting them all.
//! - **The batch rule**: per-item budgets, and a total summed in key order, failing the item at
//!   which the running sum passes it and every item after it in key order.
//! - **Logical bytes from fixed sizes per kind**: a reservation for n values of a kind is exactly
//!   n times its size, counted in `u128`, never wrapped, whatever the target's pointer width.
//! - **Cancellation polled at fixed intervals, never unwinding**: the meter asks the caller's
//!   `Cancel` exactly every `POLL_EVERY` charges and at each `poll`, whatever the timing; a
//!   cancellation seen inside a phase on a thread pool returns `Cancelled` and no partial result.
//!
//! Seeds: each property test prints `seed: <hex>` and runs from a fixed default through
//! `geom_test::prop`; `SPAREKIT_SEED=<hex>` replays another (chapter 28.17). Each case is one
//! 64-bit word, expanded into items, budgets and keys by `geom_test::rng::SplitMix64` in integer
//! arithmetic, so a seed draws the same cases on every target (proptest's own collection and
//! `usize` samplers do not: `geom_test::prop`'s header). Case counts are an eighth in debug builds.

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

mod common;

use std::num::NonZeroU64;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use common::{THREADS_3, few_execs, plan};
use geom_run::{
    BatchBudget, Budget, BudgetKind, Cancel, Exceeded, Exec, ItemError, LogicalSize, POLL_EVERY,
    Phase, RunError, Stop, Usage, WorkMeter,
};
use geom_test::prop;
use geom_test::rng::SplitMix64;
use geom_test::seed::Seed;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

/// The phase every attack runs as.
const PHASE: Phase = Phase::new("attack g23");

/// The chunk sizes chapter 23.4 forces, 1, 7 and 4,096, and 64 between them.
const SIZES: [usize; 4] = [1, 7, 64, 4_096];

/// Cases per property test: an eighth in debug builds, where the pools are slow.
const CASES: u32 = if cfg!(debug_assertions) { 48 } else { 384 };

/// An item's own error, or the note that its meter stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fail {
    /// The item's own error, carrying its index.
    Own(usize),
    /// The item's meter stopped; the helper reads the meter for the reason.
    Stopped,
}

/// One item of a case: the work units it charges, one charge each, the logical bytes it reserves,
/// and the charge before which it fails with its own error, if any.
#[derive(Clone, Debug)]
struct Item {
    charges: Vec<u64>,
    bytes: u64,
    fails_at: Option<usize>,
}

/// What an item returns when it passes: its index and the work it charged.
type Value = (usize, u64);

/// Runs one item against its meter, as every helper call below does.
fn run(index: usize, item: &Item, meter: &mut WorkMeter<'_>) -> Result<Value, Fail> {
    meter.reserve(item.bytes).map_err(|_| Fail::Stopped)?;
    for (k, &work) in item.charges.iter().enumerate() {
        if item.fails_at == Some(k) {
            return Err(Fail::Own(index));
        }
        meter.charge(work).map_err(|_| Fail::Stopped)?;
    }
    Ok((index, item.charges.iter().sum()))
}

/// Uniform in 0..n, n > 0.
fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

/// A case: items, a budget for the operation, what it used before the phase, and batch keys.
#[derive(Clone, Debug)]
struct Case {
    items: Vec<Item>,
    cap: Budget,
    before: Usage,
    keys: Vec<u64>,
    per_item: Budget,
    total: Option<Budget>,
}

/// The case one word makes. Budgets are drawn around what the items need, so that passing, an
/// item over the cap, a total over the cap and an item's own error all happen often.
fn case(word: u64) -> Case {
    let mut rng = SplitMix64::new(Seed(word));
    let n = below(&mut rng, 160) as usize;
    let fail_one_in = 1 + below(&mut rng, 40);
    let items: Vec<Item> = (0..n)
        .map(|_| {
            let count = below(&mut rng, 6) as usize;
            let charges = (0..count).map(|_| below(&mut rng, 9)).collect();
            let fails_at = (below(&mut rng, fail_one_in) == 0)
                .then(|| below(&mut rng, count as u64 + 1) as usize);
            Item {
                charges,
                bytes: below(&mut rng, 5),
                fails_at,
            }
        })
        .collect();
    let need: u64 = items.iter().flat_map(|i| i.charges.iter()).sum();
    let bytes: u64 = items.iter().map(|i| i.bytes).sum();
    let around = |rng: &mut SplitMix64, x: u64| match below(rng, 4) {
        0 => below(rng, 50),
        1 => x.saturating_sub(below(rng, 20)) + below(rng, 20),
        2 => u64::MAX,
        _ => below(rng, 2 * x + 2),
    };
    let cap = Budget::new(around(&mut rng, need + 40), around(&mut rng, bytes + 40));
    let before = Usage::new(
        u128::from(below(&mut rng, 40)),
        u128::from(below(&mut rng, 40)),
    );
    let keys = (0..n).map(|_| below(&mut rng, 24)).collect();
    let per_item = Budget::new(below(&mut rng, 50), below(&mut rng, 6));
    let total = (below(&mut rng, 2) == 0)
        .then(|| Budget::new(around(&mut rng, need), around(&mut rng, bytes)));
    Case {
        items,
        cap,
        before,
        keys,
        per_item,
        total,
    }
}

/// An operation's meter that has used `before` of `cap` (a charge and a reservation; the case
/// skips the phase if that alone stops it).
fn meter_after(cap: Budget, before: Usage) -> WorkMeter<'static> {
    let mut meter = WorkMeter::new(cap);
    let _ = meter.charge(before.work as u64);
    let _ = meter.reserve(before.logical_bytes as u64);
    meter
}

/// Runs `test` on every case of a seeded property, failing the test with its replay line.
fn check(label: &str, seed: u64, test: impl Fn(Case) -> Result<(), TestCaseError>) {
    if let Err(failure) = prop::run(label, Seed(seed), CASES, &any::<u64>(), |word| {
        test(case(word))
    }) {
        panic!("{failure}");
    }
}

// ---------------------------------------------------------------------------------------------
// Serial and parallel runs agree.
// ---------------------------------------------------------------------------------------------

/// What a phase leaves: its answer, and the operation meter's use and stop afterwards.
type PhaseOutcome = (Result<Vec<Value>, RunError<Fail>>, Usage, Option<Stop>);

/// A phase gives the same answer, failure, `Exceeded` record, use and stop on every executor and
/// chunk size.
#[test]
fn attack_serial_and_parallel_phases_agree_exactly() {
    let seen = Mutex::new([0_u32; 4]);
    check("phases agree", 0x6723_0000_5eed_0246, |c| {
        let outcome = |exec: &Exec<'_>, size: usize| -> PhaseOutcome {
            let mut meter = meter_after(c.cap, c.before);
            let got = exec.try_map(&mut meter, PHASE, plan(size), &c.items, run);
            (got, meter.used(), meter.stopped())
        };
        let reference = outcome(&Exec::serial(), 1);
        for (name, exec) in few_execs() {
            for size in SIZES {
                prop_assert_eq!(
                    &outcome(&exec, size),
                    &reference,
                    "{} at chunk size {}",
                    name,
                    size
                );
            }
        }
        let mut seen = seen.lock().unwrap();
        seen[match reference.0 {
            Ok(_) => 0,
            Err(RunError::BudgetExceeded(_)) => 1,
            Err(RunError::Failed(_)) => 2,
            Err(_) => 3,
        }] += 1;
        Ok(())
    });
    // Passing, a budget failure and an item's own error were each reached; nothing else was.
    let seen = seen.into_inner().unwrap();
    assert!(
        seen[0] > 0 && seen[1] > 0 && seen[2] > 0 && seen[3] == 0,
        "{seen:?}"
    );
}

/// A batch gives the same per-item results on every executor and chunk size, with and without a
/// total.
#[test]
fn attack_serial_and_parallel_batches_agree_exactly() {
    let totals = Mutex::new([0_u32; 2]);
    check("batches agree", 0x6723_0001_5eed_0246, |c| {
        let budget = BatchBudget {
            per_item: c.per_item,
            total: c.total,
        };
        let key = |i: usize, _: &Item| c.keys[i];
        let reference = Exec::serial().map_each(budget, None, PHASE, plan(1), &c.items, key, run);
        for (name, exec) in few_execs() {
            for size in SIZES {
                let got = exec.map_each(budget, None, PHASE, plan(size), &c.items, key, run);
                prop_assert_eq!(&got, &reference, "{} at chunk size {}", name, size);
            }
        }
        let crossed = reference
            .unwrap()
            .iter()
            .any(|r| matches!(r, Err(ItemError::BatchTotal(_))));
        totals.lock().unwrap()[usize::from(crossed)] += 1;
        Ok(())
    });
    let totals = totals.into_inner().unwrap();
    assert!(totals[0] > 0 && totals[1] > 0, "{totals:?}");
}

/// Under per-item budgets each item gets exactly the result it gets alone, also with the batch
/// reversed: no item's verdict depends on the others.
#[test]
fn attack_an_items_verdict_never_depends_on_the_other_items() {
    check("verdicts", 0x6723_0002_5eed_0246, |c| {
        let budget = BatchBudget::new(c.per_item);
        let key = |i: usize, _: &Item| i;
        let alone: Vec<_> = c
            .items
            .iter()
            .map(|item| {
                let one = core::slice::from_ref(item);
                let mut r = Exec::serial()
                    .map_each(budget, None, PHASE, plan(1), one, key, run)
                    .unwrap();
                r.pop().unwrap()
            })
            .collect();
        // An item's own error carries its index, which alone is 0: compare with that rebased.
        let rebase = |r: &Result<Value, ItemError<Fail>>| match *r {
            Ok((_, w)) => Ok((0, w)),
            Err(ItemError::Failed(Fail::Own(_))) => Err(ItemError::Failed(Fail::Own(0))),
            Err(e) => Err(e),
        };
        let reversed: Vec<Item> = c.items.iter().rev().cloned().collect();
        let n = c.items.len();
        for (name, exec) in few_execs() {
            let together = exec
                .map_each(budget, None, PHASE, plan(7), &c.items, key, run)
                .unwrap();
            let backwards = exec
                .map_each(budget, None, PHASE, plan(7), &reversed, key, run)
                .unwrap();
            for i in 0..n {
                prop_assert_eq!(rebase(&together[i]), alone[i], "{}: item {}", name, i);
                prop_assert_eq!(
                    rebase(&backwards[n - 1 - i]),
                    alone[i],
                    "{}: item {} reversed",
                    name,
                    i
                );
            }
        }
        Ok(())
    });
}

// ---------------------------------------------------------------------------------------------
// The phase rule, by hand.
// ---------------------------------------------------------------------------------------------

/// Items that each charge `work` once, failing with their own error where `own` says.
fn flat(work: &[u64], own: &[usize]) -> Vec<Item> {
    work.iter()
        .enumerate()
        .map(|(i, &w)| Item {
            charges: vec![w],
            bytes: 0,
            fails_at: own.contains(&i).then_some(0),
        })
        .collect()
}

/// Every item runs capped at exactly what the phase has left.
#[test]
fn attack_each_item_is_capped_at_what_the_phase_has_left() {
    let items = flat(&[1; 40], &[]);
    for (name, exec) in few_execs() {
        for size in SIZES {
            let caps = Mutex::new(Vec::new());
            let mut meter = meter_after(Budget::new(1_000, 300), Usage::new(250, 20));
            exec.try_map(&mut meter, PHASE, plan(size), &items, |i, item, m| {
                caps.lock().unwrap().push(m.cap());
                run(i, item, m)
            })
            .unwrap();
            let caps = caps.into_inner().unwrap();
            assert!(caps.len() >= items.len(), "{name}");
            assert!(
                caps.iter().all(|&c| c == Budget::new(750, 280)),
                "{name}: {caps:?}"
            );
            assert_eq!(meter.used(), Usage::new(290, 20), "{name}");
        }
    }
}

/// The lowest-numbered failure wins, whichever kind it is and wherever the work runs.
#[test]
fn attack_the_lowest_numbered_failure_wins() {
    // Item 5 is over the cap of 100 on its own; items 10 and 20 fail with their own errors.
    let mut work = vec![1_u64; 30];
    work[5] = 101;
    let over_first = flat(&work, &[10, 20]);
    let own_first = flat(&work, &[3, 20]);
    for (name, exec) in few_execs() {
        for size in SIZES {
            let mut meter = WorkMeter::new(Budget::of_work(100));
            match exec.try_map(&mut meter, PHASE, plan(size), &over_first, run) {
                // Items 0 to 4 charged 1 each, and item 5 its 101.
                Err(RunError::BudgetExceeded(Exceeded {
                    kind: BudgetKind::Work,
                    phase: PHASE,
                    used,
                    ..
                })) => assert_eq!(used.work, 106, "{name}"),
                other => panic!("{name} at {size}: {other:?}"),
            }
            let mut meter = WorkMeter::new(Budget::of_work(100));
            let got = exec.try_map(&mut meter, PHASE, plan(size), &own_first, run);
            assert_eq!(got, Err(RunError::Failed(Fail::Own(3))), "{name} at {size}");
        }
    }
}

/// A total over what is left fails only at the end of the phase, after every item ran, counting
/// them all; an item's own error, even the last item's, still wins.
#[test]
fn attack_a_total_over_the_cap_fails_at_the_end_of_the_phase() {
    let items = flat(&[11; 10], &[]);
    let last_fails = flat(&[11; 10], &[9]);
    for (name, exec) in few_execs() {
        for size in SIZES {
            let ran = Mutex::new(vec![false; items.len()]);
            let mut meter = WorkMeter::new(Budget::of_work(100));
            let got = exec.try_map(&mut meter, PHASE, plan(size), &items, |i, item, m| {
                ran.lock().unwrap()[i] = true;
                run(i, item, m)
            });
            match got {
                Err(RunError::BudgetExceeded(e)) => {
                    assert_eq!(
                        (e.kind, e.used.work, e.budget),
                        (BudgetKind::Work, 110, Budget::of_work(100))
                    );
                }
                other => panic!("{name} at {size}: {other:?}"),
            }
            assert!(
                ran.into_inner().unwrap().iter().all(|&r| r),
                "{name}: every item ran"
            );
            assert_eq!(
                meter.stopped(),
                Some(Stop::OverBudget(BudgetKind::Work)),
                "{name}"
            );
            let mut meter = WorkMeter::new(Budget::of_work(100));
            let got = exec.try_map(&mut meter, PHASE, plan(size), &last_fails, run);
            assert_eq!(got, Err(RunError::Failed(Fail::Own(9))), "{name} at {size}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The batch rule, by hand.
// ---------------------------------------------------------------------------------------------

/// Per-item budgets, and a total summed in key order: the item at which the running sum passes the
/// total, and every item after it in key order, fail with `BatchTotal`; the others keep their own
/// results.
#[test]
fn attack_a_batch_total_is_summed_in_key_order() {
    // Work 10 each, except item 2 over its per-item budget of 25; keys put them in the order
    // 4, 3, 2, 1, 0 (item 5 shares item 0's key and follows it, in item order).
    let items = flat(&[10, 10, 30, 10, 10, 10], &[]);
    let keys = [5_u64, 4, 3, 2, 1, 5];
    let total = |work| BatchBudget::new(Budget::of_work(25)).with_total(Budget::of_work(work));
    for (name, exec) in few_execs() {
        for size in SIZES {
            // A total of 55. In key order the running sum is 10 (item 4), 20 (item 3), 50 (item
            // 2, whose 30 counts though it is over its own budget) and 60 (item 1, past 55). So
            // items 4 and 3 pass, item 2 keeps its own failure, and item 1, the crossing, and
            // items 0 and 5 after it fail with the batch total, which records the 60.
            let got = exec
                .map_each(
                    total(55),
                    None,
                    PHASE,
                    plan(size),
                    &items,
                    |i, _| keys[i],
                    run,
                )
                .unwrap();
            assert_eq!(got[4], Ok((4, 10)), "{name}");
            assert_eq!(got[3], Ok((3, 10)), "{name}");
            assert!(
                matches!(got[2], Err(ItemError::BudgetExceeded(e)) if e.used.work == 30),
                "{name}: {:?}",
                got[2]
            );
            for i in [1, 0, 5] {
                assert!(
                    matches!(got[i], Err(ItemError::BatchTotal(e)) if e.used.work == 60),
                    "{name}: item {i}: {:?}",
                    got[i]
                );
            }
            // A total of 35: the crossing is item 2 itself (50), which then fails with the batch
            // total like every item after it, its own failure replaced (chapter 23.10).
            let got = exec
                .map_each(
                    total(35),
                    None,
                    PHASE,
                    plan(size),
                    &items,
                    |i, _| keys[i],
                    run,
                )
                .unwrap();
            assert_eq!((got[4], got[3]), (Ok((4, 10)), Ok((3, 10))), "{name}");
            for i in [2, 1, 0, 5] {
                assert!(
                    matches!(got[i], Err(ItemError::BatchTotal(e)) if e.used.work == 50),
                    "{name}: item {i}: {:?}",
                    got[i]
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Logical bytes.
// ---------------------------------------------------------------------------------------------

/// A record of 24 logical bytes, whatever the target.
struct Record;
impl LogicalSize for Record {
    const LOGICAL_BYTES: u64 = 24;
}

/// The largest kind there can be.
struct Huge;
impl LogicalSize for Huge {
    const LOGICAL_BYTES: u64 = u64::MAX;
}

/// Reservations for n values of a kind are exactly n times its fixed size, counted in `u128`.
#[test]
fn attack_logical_bytes_are_fixed_sizes_times_counts() {
    let mut meter = WorkMeter::new(Budget::new(0, 24 * 1_000));
    meter.reserve_for::<Record>(999).unwrap();
    meter.reserve_for::<Record>(1).unwrap();
    assert_eq!(meter.used(), Usage::new(0, 24_000));
    let stopped = meter.reserve_for::<Record>(1).unwrap_err();
    assert_eq!(stopped.reason(), Stop::OverBudget(BudgetKind::LogicalBytes));
    // u64::MAX values of u64::MAX bytes: (2^64 − 1)^2, exactly, in the failure's record.
    let mut meter = WorkMeter::new(Budget::new(0, u64::MAX));
    let stopped = meter.reserve_for::<Huge>(u64::MAX).unwrap_err();
    let want = u128::from(u64::MAX) * u128::from(u64::MAX);
    assert_eq!(meter.used().logical_bytes, want);
    match meter.error::<Fail>(stopped, PHASE) {
        RunError::BudgetExceeded(e) => {
            assert_eq!(
                (e.kind, e.used.logical_bytes),
                (BudgetKind::LogicalBytes, want)
            );
        }
        other => panic!("{other:?}"),
    }
    // Work is checked first when both are over (chapter 23.10).
    assert_eq!(
        Usage::new(2, 2).exceeds(Budget::new(1, 1)),
        Some(BudgetKind::Work)
    );
}

// ---------------------------------------------------------------------------------------------
// Cancellation.
// ---------------------------------------------------------------------------------------------

/// A cancellation that counts how often it is asked (behind a lock: the crate bans atomic
/// counters, tests included), and asks to stop once `cancel` is set.
#[derive(Default)]
struct Counting {
    asked: Mutex<usize>,
    cancel: AtomicBool,
}

impl Counting {
    /// How often it has been asked.
    fn asked(&self) -> usize {
        *self.asked.lock().unwrap()
    }
}

impl Cancel for Counting {
    fn requested(&self) -> bool {
        *self.asked.lock().unwrap() += 1;
        self.cancel.load(Ordering::SeqCst)
    }
}

/// The meter asks exactly every `POLL_EVERY` charges, counted from its creation, and at each
/// `poll`, which restarts the count; a request is seen at the next of those points, never between.
#[test]
fn attack_cancellation_is_asked_at_fixed_points() {
    let counting = Counting::default();
    let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&counting);
    for k in 1..=5 * POLL_EVERY {
        meter.charge(1).unwrap();
        assert_eq!(counting.asked(), k / POLL_EVERY, "after {k} charges");
    }
    // A poll asks at once and restarts the count.
    for _ in 0..10 {
        meter.charge(1).unwrap();
    }
    meter.poll().unwrap();
    assert_eq!(counting.asked(), 6);
    for _ in 0..POLL_EVERY - 1 {
        meter.charge(1).unwrap();
    }
    assert_eq!(counting.asked(), 6);
    // Asked to stop now: the charges before the next read pass, and that read stops the meter.
    counting.cancel.store(true, Ordering::SeqCst);
    let stopped = meter.charge(1).unwrap_err();
    assert_eq!((stopped.reason(), counting.asked()), (Stop::Cancelled, 7));
    // A latch: it stays stopped for the first reason, without asking again.
    assert_eq!(meter.charge(1).unwrap_err().reason(), Stop::Cancelled);
    assert_eq!(counting.asked(), 7);
}

/// A cancellation requested by an item while a phase runs on a thread pool ends the phase with
/// `Cancelled` and no partial result; nothing unwinds.
#[test]
fn attack_cancellation_inside_a_phase_never_unwinds_or_leaks_results() {
    let items = flat(&vec![1; 5_000], &[]);
    for size in SIZES {
        let counting = Counting::default();
        let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&counting);
        let got = Exec::with_pool(&THREADS_3).try_map(
            &mut meter,
            PHASE,
            plan(size),
            &items,
            |i, item, m| {
                if i == 1_234 {
                    counting.cancel.store(true, Ordering::SeqCst);
                }
                run(i, item, m)
            },
        );
        assert_eq!(got, Err(RunError::Cancelled), "chunk size {size}");
        assert_eq!(meter.stopped(), Some(Stop::Cancelled));
    }
}
