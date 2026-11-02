//! Attacks on every claim of piece G104, "Batch runner" (chapter 30): one `Result` per item, item
//! `OpPath` = batch `OpPath` plus the caller's key, under 23.10's batch rule; batch equals single calls
//! bit for bit, names included; one failure fails only itself (chapters 10.1, 23.6 and 23.10).
//! The flat-array half of the piece is attacked in geom-curve's and geom-surface's
//! `tests/points_flat.rs`, and the benchmark is geom-curve's `benches/batch.rs`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "tests may use native arithmetic and casts"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeMap;
use std::num::{NonZeroU64, NonZeroUsize};

use geom_op::{Batch, ErrorKind, Key, OpContext, OpError, OpId, OpPath, RunDetail};
use geom_run::{BatchBudget, Budget, ChunkPlan, Exec, Phase, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

#[derive(Clone, Copy, Debug)]
struct Item {
    key: Key,
    work: u64,
}

fn op(ctx: &OpContext<'_>, item: &Item, meter: &mut WorkMeter<'_>) -> Result<OpPath, OpError> {
    for _ in 0..item.work {
        // The helper reads the meter, never this error, to report a stop.
        meter
            .charge(1)
            .map_err(|_| OpError::new(ctx.path().clone(), RunDetail::Cancelled))?;
    }
    Ok(ctx.path().clone())
}

fn key(n: u64) -> Key {
    let mut k = [0u8; 16];
    k[8..].copy_from_slice(&n.to_be_bytes());
    k
}

fn batch(per_item: u64, total: Option<u64>) -> Batch<'static> {
    let ctx = OpContext::new(OpPath::new(OpId::from_bytes([3; 16])), Budget::UNLIMITED);
    let b = BatchBudget::new(Budget::of_work(per_item));
    let b = match total {
        Some(t) => b.with_total(Budget::of_work(t)),
        None => b,
    };
    Batch::new(
        &ctx,
        b,
        Phase::new("attack"),
        ChunkPlan::new(NonZeroUsize::new(4).unwrap()),
    )
}

/// Each item's result by key.
fn by_key(
    items: &[Item],
    results: Vec<Result<OpPath, OpError>>,
) -> BTreeMap<Key, Result<OpPath, OpError>> {
    assert_eq!(items.len(), results.len(), "one result per item");
    items.iter().map(|i| i.key).zip(results).collect()
}

#[test]
fn inserting_or_moving_items_changes_no_other_items_result() {
    let seed = resolve("geom-op G104 insertion", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let b = batch(30, None);
    for _ in 0..30 {
        let n = 1 + rng.below(NonZeroU64::new(50).unwrap()) as usize;
        let items: Vec<Item> = (0..n)
            .map(|i| Item {
                key: key(i as u64 * 7 + 1),
                work: rng.below(NonZeroU64::new(40).unwrap()),
            })
            .collect();
        let before = by_key(
            &items,
            b.run(&Exec::serial(), &items, |i| i.key, op).unwrap(),
        );
        // Insert a new item at a random place, and rotate the order.
        let mut more = items.clone();
        let at = rng.below(NonZeroU64::new(n as u64 + 1).unwrap()) as usize;
        more.insert(
            at,
            Item {
                key: key(0),
                work: 5,
            },
        );
        more.rotate_left(at);
        let after = by_key(&more, b.run(&Exec::serial(), &more, |i| i.key, op).unwrap());
        for (k, r) in &before {
            assert_eq!(
                after.get(k),
                Some(r),
                "an item's result moved with its neighbours"
            );
        }
    }
}

#[test]
fn an_items_path_is_its_key_never_its_position() {
    let b = batch(10, None);
    let items: Vec<Item> = (0..8)
        .map(|i| Item {
            key: key(100 - i),
            work: 1,
        })
        .collect();
    let results = b.run(&Exec::serial(), &items, |i| i.key, op).unwrap();
    for (i, r) in items.iter().zip(results) {
        let path = r.unwrap();
        assert_eq!(&path, &b.path().item(i.key));
        assert_eq!(path, b.single(i.key, i, op).unwrap());
    }
}

#[test]
fn an_items_own_budget_never_depends_on_the_batch_size() {
    // An item that uses exactly its budget passes alone and among 2,000 others; one unit more
    // fails alone and among them, with the same record.
    let b = batch(25, None);
    for work in [25, 26] {
        let target = Item {
            key: key(5000),
            work,
        };
        let alone = b.single(target.key, &target, op);
        let mut crowd: Vec<Item> = (0..2000)
            .map(|i| Item {
                key: key(i),
                work: 25,
            })
            .collect();
        crowd.push(target);
        let all = b.run(&Exec::serial(), &crowd, |i| i.key, op).unwrap();
        assert_eq!(all.last().unwrap(), &alone);
        assert_eq!(alone.is_ok(), work == 25);
        assert!(all[..2000].iter().all(Result::is_ok));
    }
}

#[test]
fn a_total_cut_never_reaches_back_past_lower_keys() {
    // Items 0..10 of 10 units under a total of 55: keys 0..5 pass, 5 and above fail with the
    // batch's total. Adding an item with a higher key changes nothing below the cut.
    let b = batch(100, Some(55));
    let items: Vec<Item> = (0..10)
        .map(|i| Item {
            key: key(i),
            work: 10,
        })
        .collect();
    let first = b.run(&Exec::serial(), &items, |i| i.key, op).unwrap();
    for (i, r) in first.iter().enumerate() {
        match r {
            Ok(_) => assert!(i < 5),
            Err(e) => {
                assert!(i >= 5);
                assert_eq!(e.kind(), ErrorKind::BudgetExceeded);
                assert!(matches!(
                    e.detail(),
                    geom_op::OpErrorDetail::Run(RunDetail::BatchTotal(_))
                ));
            }
        }
    }
    let mut more = items.clone();
    more.push(Item {
        key: key(99),
        work: 1,
    });
    let second = b.run(&Exec::serial(), &more, |i| i.key, op).unwrap();
    assert_eq!(&second[..5], &first[..5]);
}

#[test]
fn a_repeated_key_is_refused_wherever_it_stands() {
    let b = batch(10, None);
    for (first, second) in [(0, 1), (0, 9), (4, 7), (8, 9)] {
        let mut items: Vec<Item> = (0..10)
            .map(|i| Item {
                key: key(i),
                work: 0,
            })
            .collect();
        items[second].key = items[first].key;
        let e = b.run(&Exec::serial(), &items, |i| i.key, op).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::InvalidInput);
        assert_eq!(e.op(), b.path());
    }
}
