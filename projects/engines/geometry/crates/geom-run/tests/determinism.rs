//! Pool, thread-count and chunk-size determinism of `geom_run::Exec`.
//!
//! What this file proves: on the serial executor, on pools of 1, 2, 3, 4, 8 and 16 threads, and on
//! pools that run tasks backwards, shuffled, twice, half or not at all, `map` and `try_map` return
//! identical values and identical errors, equal to a plain loop and to an independent model of the
//! phase rule, for every chunk size tried. The cases cover uneven cost (heavy items first, so later
//! chunks finish first), floating-point work, the lowest-numbered failure rule, empty input, one
//! item, chunk sizes larger than the input, problems of many waves, nested phases, and hundreds of
//! seeded random cases mixing failures, work and logical-memory budgets. It also checks that a pool
//! really runs chunks on several threads, that the helper itself never leaves the calling thread,
//! and that chunks past an early failure stop being run within one wave.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Mutex;
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

use common::{
    Fault, INLINE, PHASE, SIZES, Script, SplitMix, THREADS_4, Tally, execs, few_execs, phase_model,
    plan, run_scripted, same_everywhere, spin, unlimited,
};
use geom_run::pool::{WAVE_FIRST, WAVE_MAX};
use geom_run::{Budget, BudgetKind, Exceeded, Exec, Phase, RunError, Stop, Stopped, WorkMeter};

const LENGTHS: [usize; 10] = [0, 1, 2, 5, 63, 64, 65, 257, 1000, 4097];

fn mix(index: usize, value: u64) -> u64 {
    (value ^ (index as u64).rotate_left(17)).wrapping_mul(0x2545_F491_4F6C_DD1D)
}

/// Plain left-to-right float sum.
fn float_sum(values: &[f64]) -> f64 {
    values.iter().fold(0.0, |total, &x| total + x)
}

#[test]
fn map_matches_a_plain_loop_on_every_executor_and_chunk_size() {
    for len in LENGTHS {
        let items: Vec<u64> = (0..len as u64).map(|i| i * 3 + 1).collect();
        let expected: Vec<u64> = items.iter().enumerate().map(|(i, &x)| mix(i, x)).collect();
        for size in [1, 3, 7, 64, 4096, 100_000] {
            let got = same_everywhere(|exec| {
                let mut meter = unlimited();
                let values = exec.map(&mut meter, PHASE, plan(size), &items, |i, x, meter| {
                    meter.charge(1)?;
                    Ok(mix(i, *x))
                });
                (values, meter.used())
            });
            assert_eq!(got.0, Ok(expected.clone()), "len {len}, size {size}");
            assert_eq!(got.1.work, len as u128);
        }
    }
}

#[test]
fn uneven_work_gives_identical_results() {
    let mut rng = SplitMix(7);
    // Heavy items cluster at the start, so on several threads the later chunks finish first.
    let costs: Vec<u64> = (0..600)
        .map(|i| match i {
            0..=40 => 20_000 + rng.below(20_000),
            _ if rng.below(10) == 0 => rng.below(30_000),
            _ => rng.below(50),
        })
        .collect();
    let expected: Vec<u64> = costs
        .iter()
        .enumerate()
        .map(|(i, &cost)| spin(cost, i as u64))
        .collect();
    for size in [1, 5, 16, 128] {
        let got = same_everywhere(|exec| {
            let mut meter = unlimited();
            exec.map(&mut meter, PHASE, plan(size), &costs, |i, cost, meter| {
                meter.charge(*cost)?;
                Ok(spin(*cost, i as u64))
            })
        });
        assert_eq!(got, Ok(expected.clone()), "size {size}");
    }
}

#[test]
fn floating_point_work_is_bit_identical_and_ordered() {
    let mut rng = SplitMix(2026);
    let values: Vec<f64> = (0..5000).map(|_| rng.wide_double(40)).collect();
    // Per-item float work (only IEEE operations): bit-identical, in item order.
    let f = |i: usize, x: &f64, _: &mut WorkMeter<'_>| {
        Ok::<_, Stopped>((x * 1.000_000_1 + i as f64).abs().sqrt().to_bits())
    };
    let expected: Vec<u64> = values
        .iter()
        .enumerate()
        .map(|(i, x)| (x * 1.000_000_1 + i as f64).abs().sqrt().to_bits())
        .collect();
    for size in [1, 7, 64, 333, 4096, 10_000] {
        let got = same_everywhere(|e| e.map(&mut unlimited(), PHASE, plan(size), &values, f));
        assert_eq!(got, Ok(expected.clone()), "size {size}");
    }

    // The values come back in item order, so a serial sum over them is the plain loop's sum on
    // every executor. The test has teeth: summing the same values in another order changes the
    // bits, so a helper that returned them in completion order would be caught.
    let identity = |_: usize, x: &f64, _: &mut WorkMeter<'_>| Ok::<_, Stopped>(*x);
    let in_order = float_sum(&values);
    let reversed = values.iter().rev().fold(0.0, |total, &x| total + x);
    assert_ne!(reversed.to_bits(), in_order.to_bits());
    for size in [1, 7, 4096] {
        let sum = same_everywhere(|exec| {
            let back = exec.map(&mut unlimited(), PHASE, plan(size), &values, identity);
            back.map(|back| float_sum(&back).to_bits())
        });
        assert_eq!(sum, Ok(in_order.to_bits()), "size {size}");
    }
}

#[test]
fn the_lowest_numbered_failure_is_reported() {
    let items: Vec<u64> = (0..2000).collect();
    let failing = [1500, 1999, 731, 732, 1200];
    // The lowest failing item is slow, so higher failures happen first on several threads.
    let check = |i: usize, x: &u64, _: &mut WorkMeter<'_>| {
        if !failing.contains(&i) {
            return Ok(x * 2);
        }
        if i == 731 {
            spin(2_000_000, *x);
        }
        Err(i)
    };
    for size in [1, 4, 64, 731, 732, 5000] {
        let got = same_everywhere(|exec| {
            exec.try_map(&mut unlimited(), PHASE, plan(size), &items, check)
        });
        assert_eq!(got, Err(RunError::Failed(731)), "size {size}");
    }
}

#[test]
fn failures_at_the_first_and_last_items() {
    let items: Vec<u32> = (0..100).collect();
    for failing in [0, 99] {
        let f = |i: usize, x: &u32, _: &mut WorkMeter<'_>| {
            if i == failing { Err(i) } else { Ok(*x) }
        };
        for size in [1, 7, 100, 1000] {
            let got =
                same_everywhere(|e| e.try_map(&mut unlimited(), PHASE, plan(size), &items, f));
            assert_eq!(got, Err(RunError::Failed(failing)), "size {size}");
        }
    }
}

#[test]
fn empty_input_gives_empty_results() {
    let items: [u8; 0] = [];
    let never = |_: usize, _: &u8, _: &mut WorkMeter<'_>| Err::<u8, _>("never called");
    let never_bool = |_: usize, _: &u8, _: &mut WorkMeter<'_>| Err::<bool, _>("never called");
    for size in [1, 64] {
        let p = plan(size);
        let got = same_everywhere(|e| e.try_map(&mut unlimited(), PHASE, p, &items, never));
        assert_eq!(got, Ok(vec![]));
        let got = same_everywhere(|e| e.find_first(&mut unlimited(), PHASE, p, &items, never_bool));
        assert_eq!(got, Ok(None));
        let got =
            same_everywhere(|e| e.filter_ordered(&mut unlimited(), PHASE, p, &items, never_bool));
        assert_eq!(got, Ok(vec![]));
        let got = same_everywhere(|e| e.scan_exclusive::<()>(&mut unlimited(), PHASE, p, &[]));
        assert_eq!(got, Ok((vec![], 0)));
        let got = same_everywhere(|e| {
            e.sort_stable::<_, _, (), _>(&mut unlimited(), PHASE, p, &items, |x| *x)
        });
        assert_eq!(got, Ok(vec![]));
    }
}

#[test]
fn one_item() {
    let items = [41u64];
    let item = |i: usize, x: &u64, meter: &mut WorkMeter<'_>| {
        meter.charge(3)?;
        Ok((i, x + 1))
    };
    let fail = |i: usize, _: &u64, _: &mut WorkMeter<'_>| Err::<u64, _>(i);
    for size in [1, 2, 4096] {
        let got = same_everywhere(|exec| {
            let mut meter = unlimited();
            let values = exec.map(&mut meter, PHASE, plan(size), &items, item);
            (values, meter.used().work)
        });
        assert_eq!(got, (Ok(vec![(0, 42)]), 3));
        let failed =
            same_everywhere(|e| e.try_map(&mut unlimited(), PHASE, plan(size), &items, fail));
        assert_eq!(failed, Err(RunError::Failed(0)));
    }
}

#[test]
fn random_cases_match_the_phase_model() {
    let mut rng = SplitMix(0x5EED);
    // How often each kind of answer came up: passed, an item's own error, an item over R, and a
    // total over R at the end of the phase.
    let mut kinds = [0_usize; 4];
    for case in 0..250 {
        let len = rng.below(700) as usize;
        let fail_one_in = [0, 1000, 200, 20][rng.below(4) as usize];
        let items: Vec<Script> = (0..len)
            .map(|_| Script {
                work: rng.below(6),
                bytes: rng.below(3) * rng.below(4),
                fails: fail_one_in > 0 && rng.below(fail_one_in) == 0,
            })
            .collect();
        let work: u64 = items
            .iter()
            .map(|item| item.work)
            .fold(0, u64::saturating_add);
        let bytes: u64 = items
            .iter()
            .map(|item| item.bytes)
            .fold(0, u64::saturating_add);
        let pick = |rng: &mut SplitMix, total: u64| match rng.below(6) {
            0 => u64::MAX,
            1 => total,
            2 => total.saturating_sub(1),
            3 => rng.below(total + 1),
            4 => rng.below(6),
            _ => 0,
        };
        let cap = Budget::new(pick(&mut rng, work), pick(&mut rng, bytes));
        let expected = phase_model(&items, cap, PHASE);
        let over_r = |item: &Script| item.work > cap.work || item.bytes > cap.logical_bytes;
        kinds[match &expected.answer {
            Ok(_) => 0,
            Err(RunError::Failed(_)) => 1,
            Err(_) if items.iter().any(over_r) => 2,
            Err(_) => 3,
        }] += 1;
        let size = 1 + rng.below(90) as usize;
        for (name, exec) in execs() {
            let got = run_scripted(exec, &items, cap, size);
            assert_eq!(got, expected, "case {case}, {name}, size {size}");
        }
    }
    // The cases have teeth: every kind of answer comes up often.
    assert!(kinds.iter().all(|&count| count >= 10), "{kinds:?}");
}

#[test]
fn problems_of_many_waves_and_huge_chunk_counts() {
    // More chunks than the largest wave, so every executor runs several waves.
    let len = 3 * WAVE_MAX + 17;
    let items: Vec<Script> = (0..len as u64)
        .map(|i| Script {
            work: i % 3,
            bytes: 0,
            fails: false,
        })
        .collect();
    for cap in [
        Budget::UNLIMITED,
        Budget::of_work(len as u64),
        Budget::of_work(10),
    ] {
        let expected = phase_model(&items, cap, PHASE);
        for (name, exec) in execs() {
            assert_eq!(
                run_scripted(exec, &items, cap, 1),
                expected,
                "{name}, {cap:?}"
            );
        }
    }
}

#[test]
fn chunks_past_an_early_failure_stop_within_one_wave() {
    let items: Vec<u32> = (0..100_000).collect();
    let failing = 100;
    // The failure is in the second wave of one-item chunks: chunks WAVE_FIRST .. 3 · WAVE_FIRST.
    let wave_end = 3 * WAVE_FIRST;
    assert!(failing >= WAVE_FIRST && failing < wave_end);
    for (name, exec) in execs() {
        let ran = Tally::new();
        let f = |i: usize, x: &u32, _: &mut WorkMeter<'_>| {
            ran.add();
            if i == failing { Err(i) } else { Ok(*x) }
        };
        let got = exec.try_map(&mut unlimited(), PHASE, plan(1), &items, f);
        assert_eq!(got, Err(RunError::Failed(failing)), "{name}");
        let ran = ran.get();
        if name == "serial" {
            assert_eq!(ran, failing + 1);
        } else {
            // A pool that runs tasks twice may run a chunk twice; no pool runs past the wave.
            assert!(ran <= 2 * wave_end, "{name}: {ran} items ran");
        }
    }
}

#[test]
fn nested_phases_give_the_serial_answer_and_spend_the_items_budget() {
    const INNER: Phase = Phase::new("inner");
    let items: Vec<u64> = (0..300).collect();
    let nested = |outer: Exec<'static>, inner: Exec<'static>, inner_cap: u64| {
        let mut meter = WorkMeter::new(Budget::of_work(10_000));
        let answer = outer.try_map(&mut meter, PHASE, plan(37), &items, |i, x, meter| {
            meter.charge(1)?;
            let parts: Vec<u64> = (0..=*x % 5).collect();
            let inner_items = if *x == 250 {
                vec![1; inner_cap as usize]
            } else {
                parts
            };
            inner
                .map(meter, INNER, plan(2), &inner_items, |j, y, meter| {
                    meter.charge(1)?;
                    Ok(y * 2 + j as u64)
                })
                .map(|values| (i, values))
                .map_err(|_| Fault::Own(i))
        });
        (answer, meter.used(), meter.stopped())
    };
    for cap in [5, 10_050] {
        let reference = nested(Exec::serial(), Exec::serial(), cap);
        for (outer_name, outer) in few_execs() {
            for (inner_name, inner) in few_execs() {
                let got = nested(outer, inner, cap);
                assert_eq!(got, reference, "{outer_name} / {inner_name}, cap {cap}");
            }
        }
    }
    // With a huge inner phase, the item's own meter passes the phase's R, so the outer phase
    // reports the item as over its cap, whatever error the item returned.
    let (answer, _, stopped) = nested(Exec::serial(), Exec::serial(), 10_050);
    match answer {
        Err(RunError::BudgetExceeded(Exceeded { kind, phase, .. })) => {
            assert_eq!((kind, phase), (BudgetKind::Work, PHASE));
        }
        other => panic!("expected the item over its cap, got {other:?}"),
    }
    assert_eq!(stopped, Some(Stop::OverBudget(BudgetKind::Work)));
}

#[test]
#[cfg_attr(
    target_os = "wasi",
    ignore = "needs threads to run chunks side by side; wasm32-wasip1 has no threads"
)]
fn a_thread_pool_really_runs_chunks_side_by_side() {
    let items: Vec<u32> = (0..64).collect();
    let seen: Mutex<Vec<ThreadId>> = Mutex::new(Vec::new());
    let deadline = Instant::now() + Duration::from_secs(20);
    let f = |_: usize, x: &u32, _: &mut WorkMeter<'_>| {
        let me = thread::current().id();
        {
            let mut seen = seen.lock().unwrap();
            if !seen.contains(&me) {
                seen.push(me);
            }
        }
        // Wait until a second thread has run an item too (or give up at the deadline, and fail
        // below).
        while seen.lock().unwrap().len() < 2 && Instant::now() < deadline {
            thread::yield_now();
        }
        Ok::<_, Stopped>(*x)
    };
    let got = Exec::with_pool(&THREADS_4).map(&mut unlimited(), PHASE, plan(1), &items, f);
    assert_eq!(got, Ok(items));
    assert!(seen.into_inner().unwrap().len() >= 2, "one thread only");
}

#[test]
fn the_helper_itself_never_leaves_the_calling_thread() {
    let items: Vec<u32> = (0..10_000).collect();
    let me = thread::current().id();
    for exec in [Exec::serial(), Exec::with_pool(&INLINE)] {
        let f = |_: usize, x: &u32, _: &mut WorkMeter<'_>| {
            assert_eq!(thread::current().id(), me);
            Ok::<_, Stopped>(x + 1)
        };
        for size in SIZES {
            let got = exec.map(&mut unlimited(), PHASE, plan(size), &items, f);
            assert_eq!(got.map(|v| v.len()), Ok(items.len()));
        }
        let order =
            exec.sort_stable::<_, _, (), _>(&mut unlimited(), PHASE, plan(7), &items, |x| {
                assert_eq!(thread::current().id(), me);
                std::cmp::Reverse(*x)
            });
        assert_eq!(order.map(|o| o[0]), Ok(items.len() - 1));
    }
}
