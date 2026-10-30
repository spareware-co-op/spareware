//! The batch driver through the public interface: one result per item, item paths from the
//! caller's keys, batch equal to single calls bit for bit on the serial executor and on pools of
//! 1, 2, 3 and 8 threads, one failure failing only itself, the batch total in key order, repeated
//! keys refused before anything runs, and the batch's cancellation and precision passed on.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    reason = "tests may use native arithmetic and casts"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
// The test pool starts threads (chapter 23.4 allows it in tests).
#![allow(clippy::disallowed_methods)]

use std::num::{NonZeroU64, NonZeroUsize};

use geom_num::{GridStep, Steps};
use geom_op::{
    Batch, BatchDetail, ErrorKind, Key, OpContext, OpError, OpErrorDetail, OpId, OpPath, Precision,
    Remedy, RunDetail,
};
use geom_run::{Budget, BudgetKind, CancelToken, ChunkPlan, Exec, Phase, Pool, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// A test pool: `threads` scoped threads, task i on thread i mod threads, each running its tasks
/// last first, so the tasks run in an order unlike the serial one.
struct Threads(usize);

impl Pool for Threads {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        let mut groups: Vec<Vec<&mut (dyn FnMut() + Send)>> =
            (0..self.0).map(|_| Vec::new()).collect();
        for (i, t) in tasks.iter_mut().enumerate() {
            groups[i % self.0].push(&mut **t);
        }
        let run = |group: Vec<&mut (dyn FnMut() + Send)>| {
            for t in group.into_iter().rev() {
                t();
            }
        };
        std::thread::scope(|s| {
            // wasm32-wasip1 has no threads: where none can start, every group runs here, in turn.
            if std::thread::Builder::new().spawn_scoped(s, || {}).is_err() {
                groups.into_iter().for_each(run);
                return;
            }
            for group in groups {
                s.spawn(move || run(group));
            }
        });
    }
}

const PHASE: Phase = Phase::new("panels");

fn plan(n: usize) -> ChunkPlan {
    ChunkPlan::new(NonZeroUsize::new(n).unwrap())
}

fn path() -> OpPath {
    OpPath::new(OpId::from_bytes([9; 16]))
        .step("divide")
        .unwrap()
}

fn key(n: u64) -> Key {
    let mut k = [0u8; 16];
    k[..8].copy_from_slice(&n.to_be_bytes());
    k[8..].copy_from_slice(&(!n).to_le_bytes());
    k
}

/// One panel of a test batch: its key, the work it charges, and whether it fails on its own.
#[derive(Clone, Copy, Debug)]
struct Panel {
    key: Key,
    work: u64,
    fails: bool,
}

/// What a panel's operation gives back: the path and precision it ran at, and a value of its work.
#[derive(Clone, Debug, PartialEq)]
struct Done {
    path: OpPath,
    precision: Precision,
    value: u64,
}

/// The test operation: charges `work` one unit at a time, then fails or returns.
fn panel(ctx: &OpContext<'_>, p: &Panel, meter: &mut WorkMeter<'_>) -> Result<Done, OpError> {
    for _ in 0..p.work {
        // A stopped meter ends the item; the helper reads the meter, never this error, to report
        // the stop (geom-run's batch rule).
        meter
            .charge(1)
            .map_err(|_| OpError::new(ctx.path().clone(), RunDetail::Cancelled))?;
    }
    if p.fails {
        return Err(OpError::new(
            ctx.path().clone(),
            geom_num::NumError::Degenerate,
        ));
    }
    Ok(Done {
        path: ctx.path().clone(),
        precision: ctx.precision(),
        value: p.work * 3 + 1,
    })
}

fn panels(rng: &mut SplitMix64, n: usize) -> Vec<Panel> {
    (0..n)
        .map(|i| Panel {
            key: key(rng.next_u64() ^ i as u64),
            work: rng.below(NonZeroU64::new(40).unwrap()),
            fails: rng.below(NonZeroU64::new(9).unwrap()) == 0,
        })
        .collect()
}

fn batch<'c>(ctx: &OpContext<'c>, per_item: u64, total: Option<u64>) -> Batch<'c> {
    let budget = geom_run::BatchBudget::new(Budget::of_work(per_item));
    let budget = match total {
        Some(t) => budget.with_total(Budget::of_work(t)),
        None => budget,
    };
    Batch::new(ctx, budget, PHASE, plan(3))
}

fn every_exec(f: impl Fn(&Exec<'_>)) {
    f(&Exec::serial());
    for threads in geom_test::golden::THREAD_COUNTS {
        let pool = Threads(threads);
        f(&Exec::with_pool(&pool));
    }
}

#[test]
fn every_item_runs_at_the_batch_path_plus_its_key() {
    let ctx = OpContext::new(path(), Budget::UNLIMITED);
    let b = batch(&ctx, u64::MAX, None);
    assert_eq!(b.path(), &path());
    let items: Vec<Panel> = (0..20)
        .map(|i| Panel {
            key: key(i),
            work: i,
            fails: false,
        })
        .collect();
    every_exec(|exec| {
        let results = b.run(exec, &items, |p| p.key, panel).unwrap();
        assert_eq!(results.len(), items.len());
        for (p, r) in items.iter().zip(&results) {
            let done = r.as_ref().unwrap();
            assert_eq!(done.path, path().item(p.key));
            assert_eq!(done.value, p.work * 3 + 1);
            assert_eq!(done.precision, Precision::Final);
        }
    });
    let ctx_item = b.item_context(key(7));
    assert_eq!(ctx_item.path(), &path().item(key(7)));
    assert_eq!(ctx_item.budget(), Budget::of_work(u64::MAX));
    assert!(ctx_item.cancel().is_none());
}

#[test]
fn a_batch_equals_its_items_called_one_by_one() {
    let seed = resolve("geom-op batch equals singles", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    for round in 0..40 {
        let n = 1 + rng.below(NonZeroU64::new(60).unwrap()) as usize;
        let items = panels(&mut rng, n);
        let per_item = rng.below(NonZeroU64::new(45).unwrap());
        let ctx = OpContext::new(path(), Budget::UNLIMITED);
        let b = batch(&ctx, per_item, None);
        let singles: Vec<Result<Done, OpError>> =
            items.iter().map(|p| b.single(p.key, p, panel)).collect();
        every_exec(|exec| {
            let results = b.run(exec, &items, |p| p.key, panel).unwrap();
            assert_eq!(results, singles, "round {round}");
        });
        // Over its own budget, an item fails at its own path with its own record, and alone.
        for (p, r) in items.iter().zip(&singles) {
            match r {
                Ok(done) => assert!(p.work <= per_item && !p.fails, "{done:?}"),
                Err(e) if p.work > per_item => {
                    assert_eq!(e.kind(), ErrorKind::BudgetExceeded);
                    assert_eq!(e.op(), &path().item(p.key));
                    match e.detail() {
                        OpErrorDetail::Run(RunDetail::BudgetExceeded(x)) => {
                            assert_eq!(x.kind, BudgetKind::Work);
                            assert_eq!(x.phase, PHASE);
                            assert_eq!(x.budget, Budget::of_work(per_item));
                            assert_eq!(x.used.work, u128::from(per_item) + 1);
                        }
                        other => panic!("{other:?}"),
                    }
                }
                Err(e) => {
                    assert!(p.fails);
                    assert_eq!(e.kind(), ErrorKind::Degenerate);
                    assert_eq!(e.op(), &path().item(p.key));
                }
            }
        }
    }
}

#[test]
fn one_failure_fails_only_itself() {
    let ctx = OpContext::new(path(), Budget::UNLIMITED);
    let b = batch(&ctx, 100, None);
    let mut items: Vec<Panel> = (0..10)
        .map(|i| Panel {
            key: key(i),
            work: 5,
            fails: false,
        })
        .collect();
    let all_ok = b.run(&Exec::serial(), &items, |p| p.key, panel).unwrap();
    items[4].fails = true;
    items[7].work = 101;
    every_exec(|exec| {
        let results = b.run(exec, &items, |p| p.key, panel).unwrap();
        for (i, r) in results.iter().enumerate() {
            match i {
                4 => assert_eq!(r.as_ref().unwrap_err().kind(), ErrorKind::Degenerate),
                7 => assert_eq!(r.as_ref().unwrap_err().kind(), ErrorKind::BudgetExceeded),
                _ => assert_eq!(r, &all_ok[i]),
            }
        }
    });
}

#[test]
fn the_batch_total_cuts_in_key_order_and_belongs_to_the_batch() {
    let ctx = OpContext::new(path(), Budget::UNLIMITED);
    // Ten items of 10 units each, keys 0..10, given in a scrambled order; a total of 45 lets the
    // four lowest keys through and cuts at the fifth.
    let order = [7u64, 2, 9, 0, 5, 1, 8, 3, 6, 4];
    let items: Vec<Panel> = order
        .iter()
        .map(|&k| Panel {
            key: key(k),
            work: 10,
            fails: false,
        })
        .collect();
    let b = batch(&ctx, 100, Some(45));
    every_exec(|exec| {
        let results = b.run(exec, &items, |p| p.key, panel).unwrap();
        for (k, r) in order.iter().zip(&results) {
            if *k < 4 {
                assert_eq!(r.as_ref().unwrap().value, 31);
            } else {
                let e = r.as_ref().unwrap_err();
                assert_eq!(e.op(), &path().item(key(*k)));
                match e.detail() {
                    OpErrorDetail::Run(RunDetail::BatchTotal(x)) => {
                        assert_eq!(x.budget, Budget::of_work(45));
                        assert_eq!(x.used.work, 50);
                    }
                    other => panic!("{other:?}"),
                }
            }
        }
        // Reordering the items only reorders the results.
        let mut reversed = items.clone();
        reversed.reverse();
        let mut again = b.run(exec, &reversed, |p| p.key, panel).unwrap();
        again.reverse();
        assert_eq!(again, results);
    });
    // Alone, an item never meets a total: one over the batch's total but within its own budget
    // succeeds alone, though in a batch of itself it meets the total.
    let alone = b.single(key(9), &items[2], panel).unwrap();
    assert_eq!(alone.value, 31);
    let big = Panel {
        key: key(77),
        work: 60,
        fails: false,
    };
    assert_eq!(b.single(big.key, &big, panel).unwrap().value, 181);
    let in_batch = b.run(&Exec::serial(), &[big], |p| p.key, panel).unwrap();
    assert!(matches!(
        in_batch[0].as_ref().unwrap_err().detail(),
        OpErrorDetail::Run(RunDetail::BatchTotal(_))
    ));
}

#[test]
fn a_repeated_key_is_refused_before_any_item_runs() {
    let ctx = OpContext::new(path(), Budget::UNLIMITED);
    let b = batch(&ctx, 100, None);
    let mut items: Vec<Panel> = (0..6)
        .map(|i| Panel {
            key: key(i),
            work: 1,
            fails: false,
        })
        .collect();
    items[5].key = key(3);
    items[1].key = key(4);
    every_exec(|exec| {
        let e = b
            .run(
                exec,
                &items,
                |p| p.key,
                |_, _, _| -> Result<(), OpError> { panic!("no item runs") },
            )
            .unwrap_err();
        assert_eq!(e.kind(), ErrorKind::InvalidInput);
        assert_eq!(e.op(), &path());
        // The least key two items share.
        assert_eq!(
            e.detail(),
            &OpErrorDetail::Batch(BatchDetail::RepeatedKey { key: key(3) })
        );
        assert_eq!(
            e.remedies(),
            [Remedy::ChangeInput {
                input: "keys",
                to: None
            }]
        );
    });
    let detail = BatchDetail::RepeatedKey { key: [0xab; 16] };
    assert_eq!(detail.kind(), ErrorKind::InvalidInput);
    assert_eq!(
        detail.to_string(),
        "two batch items share the key abababababababababababababababab"
    );
    // An empty batch and a batch of one have no repeated key.
    assert_eq!(
        b.run(&Exec::serial(), &items[..0], |p| p.key, panel)
            .unwrap(),
        []
    );
    assert_eq!(
        b.run(&Exec::serial(), &items[..1], |p| p.key, panel)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn the_batch_passes_on_its_cancellation_and_precision() {
    let token = CancelToken::new();
    let preview = Precision::Preview {
        bound: Steps(4),
        step: GridStep::Q0,
    };
    let ctx = OpContext::new(path(), Budget::UNLIMITED)
        .with_cancel(&token)
        .with_precision(preview);
    let b = batch(&ctx, 1000, None);
    let item = b.item_context(key(1));
    assert!(item.cancel().is_some());
    assert_eq!(item.precision(), preview);
    let items: Vec<Panel> = (0..5)
        .map(|i| Panel {
            key: key(i),
            work: 2,
            fails: false,
        })
        .collect();
    let results = b.run(&Exec::serial(), &items, |p| p.key, panel).unwrap();
    assert!(
        results
            .iter()
            .all(|r| r.as_ref().unwrap().precision == preview)
    );
    assert_eq!(
        b.single(key(0), &items[0], panel).unwrap().precision,
        preview
    );
    // Cancelled: the whole batch stops at the batch's path, and no item has a result.
    token.cancel();
    let many: Vec<Panel> = (0..300)
        .map(|i| Panel {
            key: key(i),
            work: 200,
            fails: false,
        })
        .collect();
    let e = b.run(&Exec::serial(), &many, |p| p.key, panel).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::Cancelled);
    assert_eq!(e.op(), &path());
    assert_eq!(e.detail(), &OpErrorDetail::Run(RunDetail::Cancelled));
    assert_eq!(
        b.single(key(0), &many[0], panel).unwrap_err().kind(),
        ErrorKind::Cancelled
    );
}

#[test]
fn a_batch_prints_what_it_is_never_where() {
    let token = CancelToken::new();
    let ctx = OpContext::new(path(), Budget::UNLIMITED).with_cancel(&token);
    let b = batch(&ctx, 7, Some(9));
    let text = format!("{b:?}");
    assert!(text.starts_with("Batch { path: "), "{text}");
    assert!(text.contains("cancel: true"), "{text}");
    assert!(text.contains("panels"), "{text}");
    assert!(!text.contains("0x"), "no address: {text}");
    let none = batch(&OpContext::new(path(), Budget::UNLIMITED), 7, None);
    assert!(format!("{none:?}").contains("cancel: false"));
}
