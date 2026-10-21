//! The rest of chapter 23.4's helper surface: `find_first`, `filter_ordered`, `map_each`,
//! `scan_exclusive`, `count_then_fill` and `sort_stable`.
//!
//! What this file proves, each against a plain serial oracle written without the crate, on every
//! executor and for every chunk size tried:
//!
//! - `find_first` returns the first match in item order, never "any" match, even when a higher
//!   match finishes first; a failure below the match wins, and the phase rule counts only the
//!   items up to the match.
//! - `filter_ordered` returns the kept indices in item order.
//! - `map_each` follows the batch rule: each item's own result under its own budget, independent
//!   of the other items; with a total, summed in the order of the items' keys (equal keys in item
//!   order), the item at which the running sum passes it and every item after it in key order
//!   fail with `BatchTotal`, and items past that point stop being run. Passing the items in
//!   another order only reorders the results, and without a total no key is read.
//! - `scan_exclusive` gives exact offsets and totals, and `OutOfMemory` past `usize::MAX`.
//! - `count_then_fill` puts each item's outputs at its offsets, in item order, gives each item a
//!   slice of exactly its count, and lets items name each other's outputs.
//! - `sort_stable` gives the stable sorted order (equal to the standard library's stable sort),
//!   and an `Ord` that breaks its contract cannot make it panic.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::cmp::Ordering;
use std::time::Duration;

use common::{
    Job, PHASE, SIZES, SplitMix, THREADS_2, THREADS_4, THREADS_8, Tally, batch_model,
    batch_model_keyed, by_index, execs, few_execs, job, plan, same_everywhere, spin, unlimited,
};
use geom_run::{
    BatchBudget, Budget, BudgetKind, Exceeded, Exec, ItemError, Place, RunError, Stop, Stopped,
    Usage, WorkMeter,
};

#[test]
fn find_first_returns_the_first_match_in_item_order() {
    let items: Vec<u64> = (0..5000).collect();
    for target in [0, 1, 63, 64, 731, 4999] {
        // Matches at the target and at every 500th item above it.
        let matches = |i: usize, _: &u64, _: &mut WorkMeter<'_>| {
            if i == target {
                // The first match is slow, so higher matches finish first on several threads.
                spin(200_000, i as u64);
            }
            Ok::<_, Stopped>(i == target || (i > target && i.is_multiple_of(500)))
        };
        for size in SIZES {
            let got = same_everywhere(|e| {
                e.find_first(&mut unlimited(), PHASE, plan(size), &items, matches)
            });
            assert_eq!(got, Ok(Some(target)), "target {target}, size {size}");
        }
    }
    let none = |_: usize, _: &u64, _: &mut WorkMeter<'_>| Ok::<_, Stopped>(false);
    let got = same_everywhere(|e| e.find_first(&mut unlimited(), PHASE, plan(7), &items, none));
    assert_eq!(got, Ok(None));
}

#[test]
fn find_first_follows_the_phase_rule_up_to_the_match() {
    let items: Vec<u64> = (0..200).collect();
    // Every item charges one unit; item 50 matches; item `fails` fails.
    let run = |exec: Exec<'static>, cap: u64, fails: usize, size: usize| {
        let mut meter = WorkMeter::new(Budget::of_work(cap));
        let got = exec.find_first(&mut meter, PHASE, plan(size), &items, |i, _, meter| {
            meter.charge(1).map_err(|_| "stopped")?;
            if i == fails {
                Err("failed")
            } else {
                Ok(i == 50)
            }
        });
        (got, meter.used().work)
    };
    for size in [1, 3, 64, 4096] {
        // The match wins over a later failure; items above it count zero.
        let got = same_everywhere(|e| run(e, 1000, 120, size));
        assert_eq!(got, (Ok(Some(50)), 51), "size {size}");
        // A failure below the match wins.
        let got = same_everywhere(|e| run(e, 1000, 20, size));
        assert_eq!(got, (Err(RunError::Failed("failed")), 21), "size {size}");
        // The items up to the match use 51 units: over a budget of 40, at the end of the phase.
        let got = same_everywhere(|e| run(e, 40, 120, size));
        let exceeded = Exceeded {
            budget: Budget::of_work(40),
            kind: BudgetKind::Work,
            phase: PHASE,
            used: Usage::new(51, 0),
        };
        assert_eq!(
            got,
            (Err(RunError::BudgetExceeded(exceeded)), 51),
            "size {size}"
        );
    }
}

#[test]
fn filter_ordered_keeps_indices_in_item_order() {
    let mut rng = SplitMix(31);
    let items: Vec<u64> = (0..3000).map(|_| rng.below(1000)).collect();
    let keep = |i: usize, x: &u64, _: &mut WorkMeter<'_>| {
        if i.is_multiple_of(97) {
            spin(20_000, *x);
        }
        Ok::<_, Stopped>(x.is_multiple_of(3))
    };
    let expected: Vec<usize> = (0..items.len())
        .filter(|&i| items[i].is_multiple_of(3))
        .collect();
    for size in SIZES {
        let got = same_everywhere(|e| {
            e.filter_ordered(&mut unlimited(), PHASE, plan(size), &items, keep)
        });
        assert_eq!(got, Ok(expected.clone()), "size {size}");
    }
    // The lowest-numbered failure, as for every phase.
    let fail = |i: usize, _: &u64, _: &mut WorkMeter<'_>| {
        if i == 2500 || i == 1700 {
            Err(i)
        } else {
            Ok(true)
        }
    };
    let got =
        same_everywhere(|e| e.filter_ordered(&mut unlimited(), PHASE, plan(64), &items, fail));
    assert_eq!(got, Err(RunError::Failed(1700)));
}

#[test]
fn map_each_follows_the_batch_rule() {
    let mut rng = SplitMix(41);
    for case in 0..60 {
        let len = rng.below(400) as usize;
        let items: Vec<Job> = (0..len)
            .map(|_| Job {
                work: rng.below(20),
                fails: rng.below(25) == 0,
            })
            .collect();
        let sum: u64 = items
            .iter()
            .map(|item| item.work)
            .fold(0, u64::saturating_add);
        let per_item = Budget::of_work([5, 10, 19, 100][rng.below(4) as usize]);
        let total = match rng.below(4) {
            0 => None,
            1 => Some(Budget::of_work(sum)),
            2 => Some(Budget::of_work(sum.saturating_sub(1))),
            _ => Some(Budget::of_work(rng.below(sum + 1))),
        };
        let budget = BatchBudget { per_item, total };
        let expected = batch_model(&items, budget);
        for size in [1, 7, 64] {
            for (name, exec) in execs() {
                let got = exec.map_each(budget, None, PHASE, plan(size), &items, by_index, job);
                assert_eq!(
                    got,
                    Ok(expected.clone()),
                    "case {case}, size {size}, {name}"
                );
            }
        }
    }
}

#[test]
fn a_batch_items_own_result_never_depends_on_the_other_items() {
    let items: Vec<Job> = (0..300)
        .map(|i| Job {
            work: (i * 7 % 13) as u64,
            fails: i % 31 == 0,
        })
        .collect();
    let budget = BatchBudget::new(Budget::of_work(9));
    for (name, exec) in few_execs() {
        let batch = exec
            .map_each(budget, None, PHASE, plan(16), &items, by_index, job)
            .unwrap();
        for (index, result) in batch.iter().enumerate() {
            // The same item alone, as a batch of one, at index 0.
            let alone = exec.map_each(
                budget,
                None,
                PHASE,
                plan(1),
                &items[index..=index],
                by_index,
                job,
            );
            let alone = alone.unwrap().pop().unwrap();
            let shifted = result.map(|_| 0);
            assert_eq!(alone, shifted, "item {index}, {name}");
        }
    }
}

#[test]
fn a_batch_stops_running_items_past_its_total() {
    let items: Vec<Job> = (0..100_000)
        .map(|_| Job {
            work: 1,
            fails: false,
        })
        .collect();
    let budget = BatchBudget::new(Budget::of_work(1)).with_total(Budget::of_work(10));
    for (name, exec) in execs() {
        let ran = Tally::new();
        let counted = |i: usize, item: &Job, meter: &mut WorkMeter<'_>| {
            ran.add();
            job(i, item, meter)
        };
        let got = exec
            .map_each(budget, None, PHASE, plan(1), &items, by_index, counted)
            .unwrap();
        assert_eq!(got.len(), items.len());
        assert!(got[..10].iter().all(Result::is_ok), "{name}");
        let crossing = Exceeded {
            budget: Budget::of_work(10),
            kind: BudgetKind::Work,
            phase: PHASE,
            used: Usage::new(11, 0),
        };
        assert!(
            got[10..]
                .iter()
                .all(|r| *r == Err(ItemError::BatchTotal(crossing))),
            "{name}"
        );
        let ran = ran.get();
        // The serial executor stops at the crossing chunk; a pool within the first wave.
        let bound = if name == "serial" {
            11
        } else {
            2 * geom_run::pool::WAVE_FIRST
        };
        assert!(ran <= bound, "{name}: {ran} items ran");
    }
}

#[test]
fn a_batch_reports_cancellation_and_items_over_their_own_budget() {
    let items = [
        Job {
            work: 3,
            fails: false,
        },
        Job {
            work: 30,
            fails: false,
        },
    ];
    let budget = BatchBudget::new(Budget::of_work(10));
    let got = same_everywhere(|e| e.map_each(budget, None, PHASE, plan(1), &items, by_index, job));
    let over = Exceeded {
        budget: Budget::of_work(10),
        kind: BudgetKind::Work,
        phase: PHASE,
        used: Usage::new(30, 0),
    };
    assert_eq!(got, Ok(vec![Ok(0), Err(ItemError::BudgetExceeded(over))]));

    let token = geom_run::CancelToken::new();
    let cancel_at_one = |i: usize, item: &Job, meter: &mut WorkMeter<'_>| {
        if i == 1 {
            token.cancel();
            meter.poll().map_err(|_| "stopped")?;
        }
        job(i, item, meter)
    };
    for (name, exec) in execs() {
        let got = exec.map_each(
            budget,
            Some(&token),
            PHASE,
            plan(1),
            &items,
            by_index,
            cancel_at_one,
        );
        assert_eq!(got, Err(geom_run::Interrupted::Cancelled), "{name}");
    }
}

/// Chapter 23.10: a batch total is summed "in item-key order", and every item after the crossing
/// "in key order" fails; chapter 10.1: items are never keyed by position.
#[test]
fn a_batch_total_is_summed_in_key_order() {
    let mut rng = SplitMix(43);
    for case in 0..40 {
        let len = 1 + rng.below(300) as usize;
        let items: Vec<Job> = (0..len)
            .map(|_| Job {
                work: rng.below(20),
                fails: rng.below(25) == 0,
            })
            .collect();
        // Odd cases: distinct keys in a shuffled order. Even cases: keys from a small range, so
        // many are equal and are taken in item order.
        let keys: Vec<u64> = if case % 2 == 1 {
            let mut keys: Vec<u64> = (0..len as u64).map(|k| 1000 + 3 * k).collect();
            for i in (1..len).rev() {
                keys.swap(i, rng.below(i as u64 + 1) as usize);
            }
            keys
        } else {
            (0..len).map(|_| rng.below(8)).collect()
        };
        let sum: u64 = items
            .iter()
            .map(|item| item.work)
            .fold(0, u64::saturating_add);
        let total = Budget::of_work(rng.below(sum + 1));
        let budget = BatchBudget::new(Budget::of_work(15)).with_total(total);
        let expected = batch_model_keyed(&items, &keys, budget);
        let key = |i: usize, _: &Job| keys[i];
        for size in [1, 7, 64] {
            for (name, exec) in execs() {
                let got = exec.map_each(budget, None, PHASE, plan(size), &items, key, job);
                assert_eq!(
                    got,
                    Ok(expected.clone()),
                    "case {case}, size {size}, {name}"
                );
            }
        }
        // With distinct keys, passing the items in another order only reorders the results.
        if case % 2 == 1 {
            let reversed: Vec<Job> = items.iter().rev().copied().collect();
            let reversed_keys: Vec<u64> = keys.iter().rev().copied().collect();
            let key = |i: usize, _: &Job| reversed_keys[i];
            let got = same_everywhere(|e| {
                e.map_each(budget, None, PHASE, plan(7), &reversed, key, job)
                    .map(|results| {
                        // Item values are indices; map them back to the original numbering.
                        results
                            .into_iter()
                            .map(|result| result.map(|i| len - 1 - i))
                            .rev()
                            .collect::<Vec<_>>()
                    })
            });
            assert_eq!(got, Ok(expected), "case {case}, reversed");
        }
    }
}

/// Without a total no item's result depends on another's, so no key is read.
#[test]
fn a_batch_without_a_total_reads_no_key() {
    let items: Vec<Job> = (0..500)
        .map(|i| Job {
            work: i % 9,
            fails: i % 41 == 0,
        })
        .collect();
    let budget = BatchBudget::new(Budget::of_work(6));
    let read = Tally::new();
    let key = |i: usize, _: &Job| {
        read.add();
        i
    };
    let expected = batch_model(&items, budget);
    for size in [1, 7, 64] {
        let got =
            same_everywhere(|e| e.map_each(budget, None, PHASE, plan(size), &items, key, job));
        assert_eq!(got, Ok(expected.clone()), "size {size}");
    }
    assert_eq!(read.get(), 0);
}

#[test]
fn scan_exclusive_gives_exact_offsets() {
    let mut rng = SplitMix(51);
    for len in [0, 1, 2, 63, 64, 65, 1000, 5000] {
        let counts: Vec<usize> = (0..len).map(|_| rng.below(9) as usize).collect();
        let mut expected = Vec::new();
        let mut total = 0;
        for &count in &counts {
            expected.push(total);
            total += count;
        }
        for size in SIZES {
            let got = same_everywhere(|e| {
                e.scan_exclusive::<()>(&mut unlimited(), PHASE, plan(size), &counts)
            });
            assert_eq!(got, Ok((expected.clone(), total)), "len {len}, size {size}");
        }
    }
    // A total past `usize::MAX` cannot exist on this target: `OutOfMemory`, and the meter stops.
    for counts in [
        vec![usize::MAX, 1],
        vec![1, usize::MAX],
        vec![usize::MAX / 2 + 1; 2],
    ] {
        for size in [1, 2] {
            for (name, exec) in execs() {
                let mut meter = unlimited();
                let got = exec.scan_exclusive::<()>(&mut meter, PHASE, plan(size), &counts);
                assert_eq!(got, Err(RunError::OutOfMemory), "{counts:?}, {name}");
                assert_eq!(meter.stopped(), Some(Stop::OutOfMemory));
            }
        }
    }
    let got = Exec::serial().scan_exclusive::<()>(&mut unlimited(), PHASE, plan(1), &[usize::MAX]);
    assert_eq!(got, Ok((vec![0], usize::MAX)));
}

#[test]
fn count_then_fill_puts_outputs_at_their_offsets() {
    // Item i makes i % 4 outputs; each output names the item, its position, and where the next
    // item's outputs start (an index into the result made by another item).
    let items: Vec<u32> = (0..2000).collect();
    let count = |_: usize, x: &u32, meter: &mut WorkMeter<'_>| {
        meter.charge(1)?;
        Ok::<_, Stopped>((x % 4) as usize)
    };
    type Output = (u32, usize, usize);
    type Meter<'m> = WorkMeter<'m>;
    let fill = |i: usize, x: &u32, out: &mut [Output], place: Place<'_>, meter: &mut Meter<'_>| {
        meter.charge(out.len() as u64)?;
        assert_eq!(out.len(), (x % 4) as usize);
        let next = place.offsets.get(i + 1).copied().unwrap_or(usize::MAX);
        for (k, slot) in out.iter_mut().enumerate() {
            *slot = (*x, place.first + k, next);
        }
        Ok(())
    };
    let mut values = Vec::new();
    let mut offsets = Vec::new();
    for x in &items {
        offsets.push(values.len());
        for _ in 0..x % 4 {
            values.push(*x);
        }
    }
    let expected: Vec<(u32, usize, usize)> = values
        .iter()
        .enumerate()
        .map(|(position, &x)| {
            let next = offsets.get(x as usize + 1).copied().unwrap_or(usize::MAX);
            (x, position, next)
        })
        .collect();
    for size in SIZES {
        let got = same_everywhere(|e| {
            let mut meter = unlimited();
            let got = e.count_then_fill(&mut meter, PHASE, plan(size), &items, count, fill);
            (got, meter.used().work)
        });
        // One unit per item counted, one per output filled.
        let work = (items.len() + values.len()) as u128;
        assert_eq!(
            got,
            (Ok((expected.clone(), offsets.clone())), work),
            "size {size}"
        );
    }
}

#[test]
fn count_then_fill_runs_both_passes_under_the_phase_rule() {
    let items: Vec<u32> = (0..100).collect();
    let count = |i: usize, _: &u32, meter: &mut WorkMeter<'_>| {
        meter.charge(1).map_err(|_| "stopped")?;
        if i == 70 { Err("count failed") } else { Ok(2) }
    };
    let fill = |_: usize, x: &u32, out: &mut [u32], _: Place<'_>, _: &mut WorkMeter<'_>| {
        out.fill(*x);
        Ok(())
    };
    let got = same_everywhere(|e| {
        e.count_then_fill(&mut unlimited(), PHASE, plan(8), &items, count, fill)
    });
    assert_eq!(got, Err(RunError::Failed("count failed")));

    // The count pass uses 100 units, so the fill pass's items are capped at 50: item 3's fill,
    // charging 51, is over its cap.
    let count = |_: usize, _: &u32, meter: &mut WorkMeter<'_>| meter.charge(1).map(|()| 1);
    let fill = |i: usize, _: &u32, out: &mut [u32], _: Place<'_>, meter: &mut WorkMeter<'_>| {
        meter.charge(if i == 3 { 51 } else { 0 })?;
        out.fill(7);
        Ok::<_, Stopped>(())
    };
    let got = same_everywhere(|e| {
        let mut meter = WorkMeter::new(Budget::of_work(150));
        e.count_then_fill(&mut meter, PHASE, plan(8), &items, count, fill)
    });
    let exceeded = Exceeded {
        budget: Budget::of_work(150),
        kind: BudgetKind::Work,
        phase: PHASE,
        used: Usage::new(151, 0),
    };
    assert_eq!(got, Err(RunError::BudgetExceeded(exceeded)));
}

#[test]
fn sort_stable_gives_the_stable_sorted_order() {
    let mut rng = SplitMix(61);
    for len in [0, 1, 2, 3, 7, 64, 65, 1000, 5000] {
        // Few distinct keys, so most keys tie and stability matters.
        let items: Vec<(u8, u32)> = (0..len).map(|i| (rng.below(5) as u8, i)).collect();
        let mut expected: Vec<usize> = (0..len as usize).collect();
        expected.sort_by_key(|&i| items[i].0);
        for size in [1, 2, 3, 7, 64, 4096] {
            let got = same_everywhere(|e| {
                let key = |item: &(u8, u32)| item.0;
                e.sort_stable::<_, _, (), _>(&mut unlimited(), PHASE, plan(size), &items, key)
            });
            assert_eq!(got, Ok(expected.clone()), "len {len}, size {size}");
        }
    }
}

/// A key whose `Ord` breaks its contract: it says "less" and "greater" at random.
struct Liar(u64);

impl PartialEq for Liar {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl Eq for Liar {}

impl PartialOrd for Liar {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Liar {
    fn cmp(&self, other: &Self) -> Ordering {
        if (self.0 ^ other.0.rotate_left(7))
            .count_ones()
            .is_multiple_of(2)
        {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }
}

#[test]
fn a_broken_ord_cannot_make_sort_stable_panic() {
    let items: Vec<u64> = (0..3000).map(|i| i * 0x9E37_79B9).collect();
    for size in [1, 7, 64] {
        for (name, exec) in execs() {
            let key = |x: &u64| Liar(*x);
            let got =
                exec.sort_stable::<_, _, (), _>(&mut unlimited(), PHASE, plan(size), &items, key);
            // Some order, but always a permutation of the indices.
            let mut order = got.unwrap();
            order.sort_unstable();
            assert_eq!(
                order,
                (0..items.len()).collect::<Vec<_>>(),
                "{name}, size {size}"
            );
        }
    }
}

#[test]
fn helpers_nest_inside_items_on_the_same_pool() {
    // An item that sorts, scans and filters its own small list, on the same pool as the outer
    // phase: one pool, two levels.
    let items: Vec<u64> = (0..64).collect();
    let nested = |exec: Exec<'static>| {
        exec.try_map(&mut unlimited(), PHASE, plan(3), &items, |i, x, meter| {
            let list: Vec<u64> = (0..(x % 9 + 1)).map(|k| (k * 37 + x) % 11).collect();
            let order = exec.sort_stable(meter, PHASE, plan(2), &list, |v| *v)?;
            let counts: Vec<usize> = list.iter().map(|&v| v as usize).collect();
            let (offsets, total) = exec.scan_exclusive(meter, PHASE, plan(2), &counts)?;
            let odd = |_: usize, v: &u64, _: &mut WorkMeter<'_>| Ok(v % 2 == 1);
            let odd = exec.filter_ordered(meter, PHASE, plan(2), &list, odd)?;
            std::thread::sleep(Duration::from_micros(10 * (i as u64 % 3)));
            Ok::<_, RunError>((order, offsets, total, odd))
        })
    };
    let reference = nested(Exec::serial());
    assert!(reference.is_ok());
    for pool in [&THREADS_2, &THREADS_4, &THREADS_8] {
        let exec = Exec::with_pool(pool);
        assert_eq!(nested(exec), reference);
    }
}
