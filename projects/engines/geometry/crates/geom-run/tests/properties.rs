//! Property tests (proptest) of every helper against its plain serial oracle.
//!
//! What this file proves: for generated items, budgets and chunk sizes, on the serial executor
//! and on pools of 2, 3 and 8 threads and a pool that runs tasks backwards, `try_map` follows the
//! phase rule exactly (values, failures, budget failures and the meter left behind),
//! `find_first` is the first match, `filter_ordered` is a plain filter, `map_each` follows the
//! batch rule with its total summed in key order, `scan_exclusive` is a prefix sum,
//! `count_then_fill` is a flatten at the scanned offsets, and `sort_stable` is the standard
//! library's stable sort. The seed is fixed, so every run tries the same cases; proptest shrinks
//! any failure to a minimal case.

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

mod common;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

use common::{
    Job, PHASE, Script, batch_model_keyed, few_execs, job, phase_model, plan, run_scripted,
    unlimited,
};
use geom_run::{BatchBudget, Budget, Place, Stopped, WorkMeter};

/// Cases per property, with a fixed seed and no failure files.
fn config(cases: u32) -> Config {
    Config {
        cases,
        rng_seed: RngSeed::Fixed(0x0004_6704),
        failure_persistence: None,
        ..Config::default()
    }
}

fn script() -> impl Strategy<Value = Script> {
    (0_u64..8, 0_u64..4, prop::bool::weighted(0.02)).prop_map(|(work, bytes, fails)| Script {
        work,
        bytes,
        fails,
    })
}

fn cap() -> impl Strategy<Value = u64> {
    prop_oneof![0_u64..20, 0_u64..2000, Just(u64::MAX)]
}

proptest! {
    #![proptest_config(config(192))]

    #[test]
    fn try_map_follows_the_phase_rule(
        items in prop::collection::vec(script(), 0..400),
        work in cap(),
        bytes in cap(),
        size in 1_usize..80,
    ) {
        let cap = Budget::new(work, bytes);
        let expected = phase_model(&items, cap, PHASE);
        for (name, exec) in few_execs() {
            prop_assert_eq!(&run_scripted(exec, &items, cap, size), &expected, "{}", name);
        }
    }

    #[test]
    fn find_first_is_the_first_match(
        hits in prop::collection::vec(prop::bool::weighted(0.05), 0..400),
        size in 1_usize..80,
    ) {
        let expected = hits.iter().position(|&hit| hit);
        let matches = |_: usize, hit: &bool, _: &mut WorkMeter<'_>| Ok::<_, Stopped>(*hit);
        for (name, exec) in few_execs() {
            let got = exec.find_first(&mut unlimited(), PHASE, plan(size), &hits, matches);
            prop_assert_eq!(got, Ok(expected), "{}", name);
        }
    }

    #[test]
    fn filter_ordered_is_a_plain_filter(
        values in prop::collection::vec(0_u32..10, 0..400),
        size in 1_usize..80,
    ) {
        let expected: Vec<usize> = (0..values.len()).filter(|&i| values[i] < 3).collect();
        let keep = |_: usize, x: &u32, _: &mut WorkMeter<'_>| Ok::<_, Stopped>(*x < 3);
        for (name, exec) in few_execs() {
            let got = exec.filter_ordered(&mut unlimited(), PHASE, plan(size), &values, keep);
            prop_assert_eq!(got, Ok(expected.clone()), "{}", name);
        }
    }

    #[test]
    fn map_each_follows_the_batch_rule(
        jobs in prop::collection::vec(
            (0_u64..30, prop::bool::weighted(0.05), 0_u64..64),
            0..300,
        ),
        per_item in 0_u64..40,
        total in prop::option::of(0_u64..3000),
        size in 1_usize..80,
    ) {
        // Keys from a small range, so many are equal and the tie rule (item order) is tried too.
        let items: Vec<Job> = jobs.iter().map(|&(work, fails, _)| Job { work, fails }).collect();
        let keys: Vec<u64> = jobs.iter().map(|&(_, _, key)| key).collect();
        let budget = BatchBudget {
            per_item: Budget::of_work(per_item),
            total: total.map(Budget::of_work),
        };
        let expected = batch_model_keyed(&items, &keys, budget);
        let key = |i: usize, _: &Job| keys[i];
        for (name, exec) in few_execs() {
            let got = exec.map_each(budget, None, PHASE, plan(size), &items, key, job);
            prop_assert_eq!(got, Ok(expected.clone()), "{}", name);
        }
    }

    #[test]
    fn scan_exclusive_is_a_prefix_sum(
        counts in prop::collection::vec(0_usize..1000, 0..500),
        size in 1_usize..80,
    ) {
        let mut offsets = Vec::new();
        let mut total = 0;
        for &count in &counts {
            offsets.push(total);
            total += count;
        }
        for (name, exec) in few_execs() {
            let got = exec.scan_exclusive::<()>(&mut unlimited(), PHASE, plan(size), &counts);
            prop_assert_eq!(got, Ok((offsets.clone(), total)), "{}", name);
        }
    }

    #[test]
    fn count_then_fill_is_a_flatten_at_the_offsets(
        counts in prop::collection::vec(0_usize..6, 0..300),
        size in 1_usize..80,
    ) {
        type Out = (usize, usize);
        type Meter<'m> = WorkMeter<'m>;
        let mut values = Vec::new();
        let mut offsets = Vec::new();
        for (i, &count) in counts.iter().enumerate() {
            offsets.push(values.len());
            values.extend((0..count).map(|k| (i, k)));
        }
        let count = |_: usize, n: &usize, _: &mut WorkMeter<'_>| Ok::<_, Stopped>(*n);
        let fill = |i: usize, _: &usize, out: &mut [Out], place: Place<'_>, _: &mut Meter<'_>| {
            assert_eq!(place.offsets.get(i), Some(&place.first));
            for (k, slot) in out.iter_mut().enumerate() {
                *slot = (i, k);
            }
            Ok(())
        };
        for (name, exec) in few_execs() {
            let mut meter = unlimited();
            let got = exec.count_then_fill(&mut meter, PHASE, plan(size), &counts, count, fill);
            prop_assert_eq!(got, Ok((values.clone(), offsets.clone())), "{}", name);
        }
    }

    #[test]
    fn sort_stable_is_a_stable_sort(
        keys in prop::collection::vec(0_u8..8, 0..500),
        size in 1_usize..80,
    ) {
        let mut expected: Vec<usize> = (0..keys.len()).collect();
        expected.sort_by_key(|&i| keys[i]);
        for (name, exec) in few_execs() {
            let mut meter = unlimited();
            let got = exec.sort_stable::<_, _, (), _>(&mut meter, PHASE, plan(size), &keys, |k| *k);
            prop_assert_eq!(got, Ok(expected.clone()), "{}", name);
        }
    }
}
