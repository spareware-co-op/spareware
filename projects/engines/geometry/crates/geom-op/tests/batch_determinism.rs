//! The replay hash of piece G104: batches run through the driver over a fixed corpus, every
//! item's result written down, must give the same digest on every target and thread count
//! (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed `SplitMix64` stream: 60 batches of 0 to 300 items with random keys,
//! work and failures, under random per-item budgets, half with a batch total; each run on the
//! serial executor and on pools of 1, 2, 3 and 8 threads, which must agree before anything is
//! written; then each item's result: its path, its value, or its error's kind, path and detail.
//! A batch with a repeated key is refused whole. Any target that orders a key, cuts a total or
//! charges a unit differently changes the digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
// The test pool starts threads (chapter 23.4 allows it in tests).
#![allow(clippy::disallowed_methods)]

use std::num::{NonZeroU64, NonZeroUsize};

use geom_op::{Batch, Key, OpContext, OpError, OpId, OpPath, RunDetail};
use geom_run::{BatchBudget, Budget, ChunkPlan, Exec, Phase, Pool, WorkMeter};
use geom_test::golden::{self, hex, sha256};
use geom_test::{Seed, SplitMix64};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-op/batch";

/// A test pool: `threads` scoped threads, task i on thread i mod threads, run last first.
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

#[derive(Clone, Copy)]
struct Item {
    key: Key,
    work: u64,
    fails: bool,
}

fn op(ctx: &OpContext<'_>, item: &Item, meter: &mut WorkMeter<'_>) -> Result<u64, OpError> {
    for _ in 0..item.work {
        // The helper reads the meter, never this error, to report a stop.
        meter
            .charge(1)
            .map_err(|_| OpError::new(ctx.path().clone(), RunDetail::Cancelled))?;
    }
    if item.fails {
        return Err(OpError::new(
            ctx.path().clone(),
            geom_num::NumError::Degenerate,
        ));
    }
    Ok(item.work.wrapping_mul(0x9e37_79b9) ^ u64::from(item.key[0]))
}

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

fn corpus() -> (Vec<u8>, usize) {
    let mut rng = SplitMix64::new(geom_test::seed::fixed(
        "geom-op batch golden",
        Seed::DEFAULT,
    ));
    let mut bytes = Vec::new();
    let mut items_run = 0;
    let base = OpPath::new(OpId::from_bytes([0x5b; 16]));
    for round in 0..60u64 {
        let n = below(&mut rng, 301) as usize;
        let mut items: Vec<Item> = (0..n)
            .map(|_| {
                let mut key = [0u8; 16];
                key[..8].copy_from_slice(&rng.next_u64().to_le_bytes());
                Item {
                    key,
                    work: below(&mut rng, 60),
                    fails: below(&mut rng, 11) == 0,
                }
            })
            .collect();
        if round % 13 == 12 && n > 1 {
            items[n - 1].key = items[0].key;
        }
        let budget = BatchBudget::new(Budget::of_work(below(&mut rng, 70)));
        let budget = if rng.coin() {
            budget.with_total(Budget::of_work(below(&mut rng, 30 * n as u64 + 1)))
        } else {
            budget
        };
        let path = base
            .step("round")
            .unwrap()
            .item(round.to_be_bytes().repeat(2).try_into().unwrap());
        let ctx = OpContext::new(path, Budget::UNLIMITED);
        let plan = ChunkPlan::new(NonZeroUsize::new(1 + below(&mut rng, 40) as usize).unwrap());
        let batch = Batch::new(&ctx, budget, Phase::new("items"), plan);
        let serial = batch.run(&Exec::serial(), &items, |i| i.key, op);
        for threads in geom_test::golden::THREAD_COUNTS {
            let pool = Threads(threads);
            let pooled = batch.run(&Exec::with_pool(&pool), &items, |i| i.key, op);
            assert_eq!(pooled, serial, "round {round}, {threads} threads");
        }
        match serial {
            Ok(results) => {
                items_run += results.len();
                bytes.extend_from_slice(&(results.len() as u64).to_le_bytes());
                for r in results {
                    match r {
                        Ok(v) => {
                            bytes.push(0);
                            bytes.extend_from_slice(&v.to_le_bytes());
                        }
                        Err(e) => {
                            bytes.push(1);
                            bytes.extend_from_slice(
                                format!("{:?}|{}|{:?}", e.kind(), e.op(), e.detail()).as_bytes(),
                            );
                            bytes.push(0);
                        }
                    }
                }
            }
            Err(e) => {
                bytes.push(2);
                bytes.extend_from_slice(
                    format!("{:?}|{}|{:?}", e.kind(), e.op(), e.detail()).as_bytes(),
                );
                bytes.push(0);
            }
        }
    }
    (bytes, items_run)
}

#[test]
fn batch_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let (bytes, items) = corpus();
        (sha256(&bytes), (bytes, items))
    });
    let (bytes, items) = &checked.value;
    println!(
        "geom-op batch corpus: {items} item results, {} bytes, golden hash {}",
        bytes.len(),
        hex(sha256(bytes))
    );
    assert!(*items > 5000, "only {items} item results");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target orders a key, cuts a batch total or \
             charges a unit differently (compare this log's digest with another target's), or a \
             definition changed in place, which chapter 26.2 forbids"
        )
    });
}
