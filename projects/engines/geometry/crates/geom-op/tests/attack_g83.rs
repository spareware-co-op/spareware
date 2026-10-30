//! Attacks on every claim piece G83 makes about budgets in the operation contract.
//!
//! What this file proves:
//!
//! - **A cancelled operation stops within 1 ms natively.** An operation charging one unit per step
//!   is cancelled from another thread while it runs; the median time from the cancellation to the
//!   operation's return, over 51 runs, is under 1 ms, and every run returns `Cancelled` with
//!   nothing published (native targets only: WebAssembly here has no threads).
//! - **Nothing hidden changes a budget failure.** The same operation run twice in one process,
//!   and on fresh contexts, gives the same error field for field: no cache or state outlives a run.
//! - **A run never publishes past a stop.** For generated budgets and charges, a run returns a
//!   result exactly when the loop charging the same things one at a time stays within the budget.
//! - **The schedule is part of the schema.** An operation's schema holds its charges as constant
//!   data, with the phase, what is counted and the units.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "test cases and oracles; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken case must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_op::{Charge, Class, ErrorKind, Family, OpContext, OpError, OpId, OpPath, OpSchema, RUN};
use geom_run::{Budget, Phase, WorkMeter};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

const PHASE: Phase = Phase::new("attack");
/// The name of the phase a run charges a stop its body ignored to.
const RUN_NAME: &str = RUN.name();

fn config(cases: u32) -> Config {
    Config {
        cases,
        rng_seed: RngSeed::Fixed(0x0008_3083),
        failure_persistence: None,
        ..Config::default()
    }
}

fn path() -> OpPath {
    OpPath::new(OpId::from_bytes([0x38; 16]))
}

/// An operation that charges `units` for each of `count` steps in turn, then returns the work it
/// used.
fn stepping(ctx: &OpContext<'_>, units: u64, count: u64) -> Result<u128, OpError> {
    let step = Charge::new(PHASE, "step", units);
    ctx.run(|meter| {
        for _ in 0..count {
            step.charge(meter, 1)
                .map_err(|s| ctx.stopped(meter, s, PHASE))?;
        }
        Ok(meter.used().work)
    })
}

#[cfg(not(target_family = "wasm"))]
#[test]
#[allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "the test measures the stop with the clock and cancels from a thread, as a caller does"
)]
fn a_cancelled_operation_stops_within_1_ms() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use geom_run::CancelToken;

    let mut stops = Vec::new();
    for _ in 0..51 {
        let token = CancelToken::new();
        let started = AtomicBool::new(false);
        let ctx = OpContext::new(path(), Budget::UNLIMITED).with_cancel(&token);
        let (result, cancelled_at, returned_at) = std::thread::scope(|scope| {
            let runner = scope.spawn(|| {
                let result = ctx.run(|meter| {
                    started.store(true, Ordering::Release);
                    loop {
                        meter.charge(1).map_err(|s| ctx.stopped(meter, s, PHASE))?;
                    }
                });
                (result, Instant::now())
            });
            while !started.load(Ordering::Acquire) {
                std::hint::spin_loop();
            }
            std::thread::sleep(Duration::from_micros(200));
            let cancelled_at = Instant::now();
            token.cancel();
            let (result, returned_at): (Result<(), OpError>, Instant) = runner.join().unwrap();
            (result, cancelled_at, returned_at)
        });
        let error = result.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Cancelled);
        assert!(!error.is_replayable());
        stops.push(returned_at.saturating_duration_since(cancelled_at));
    }
    stops.sort_unstable();
    let median = stops[stops.len() / 2];
    println!(
        "stop after cancellation: median {median:?}, worst {:?}",
        stops[stops.len() - 1]
    );
    assert!(median < Duration::from_millis(1), "median {median:?}");
}

#[test]
fn the_same_run_twice_gives_the_same_error() {
    let budget = Budget::of_work(1_000);
    let first = stepping(&OpContext::new(path(), budget), 7, 1_000).unwrap_err();
    let second = stepping(&OpContext::new(path(), budget), 7, 1_000).unwrap_err();
    assert_eq!(first, second);
    let ctx = OpContext::new(path(), budget);
    assert_eq!(stepping(&ctx, 7, 1_000).unwrap_err(), first);
    assert_eq!(stepping(&ctx, 7, 1_000).unwrap_err(), first);
    assert_eq!(first.kind(), ErrorKind::BudgetExceeded);
    assert_eq!(first.to_string().matches("1001").count(), 1, "{first}");
}

proptest! {
    #![proptest_config(config(256))]

    #[test]
    fn a_run_publishes_exactly_when_the_loop_stays_within_its_budget(
        cap in 0_u64..2_000,
        units in 0_u64..50,
        count in 0_u64..200,
    ) {
        let ctx = OpContext::new(path(), Budget::of_work(cap));
        let result = stepping(&ctx, units, count);
        let total = u128::from(units) * u128::from(count);
        if total <= u128::from(cap) {
            prop_assert_eq!(result, Ok(total));
        } else {
            let error = result.unwrap_err();
            prop_assert_eq!(error.kind(), ErrorKind::BudgetExceeded);
            prop_assert!(error.to_string().contains("`attack`"));
        }
        // A body that charges everything at once and ignores the stop publishes nothing either.
        let all = Charge::new(PHASE, "step", units);
        let ignored = ctx.run(|meter| {
            let _ignored = all.charge(meter, count);
            Ok(())
        });
        prop_assert_eq!(ignored.is_ok(), total <= u128::from(cap));
        if let Err(error) = ignored {
            let run = format!("`{RUN_NAME}`");
            prop_assert!(error.to_string().contains(&run));
        }
    }
}

/// A schema's charges are constant data.
const SCHEMA: OpSchema = OpSchema {
    name: "make.box",
    version: 1,
    family: Family::Make,
    doc: "A box.",
    inputs: &[],
    outputs: &[],
    roles: &[],
    errors: &[],
    diagnostics: &[],
    classes: &[Class::Exact],
    charges: &[
        Charge::new(Phase::new("faces"), "output face", 4),
        Charge::new(Phase::new("edges"), "output edge", 1),
    ],
    batch: false,
    example: "a box of 1 m",
};

#[test]
fn the_schedule_is_part_of_the_schema() {
    assert_eq!(SCHEMA.charges.len(), 2);
    assert_eq!(SCHEMA.charges[0].phase, Phase::new("faces"));
    assert_eq!(SCHEMA.charges[0].per, "output face");
    assert_eq!(SCHEMA.charges[0].units, 4);
    let mut meter = WorkMeter::new(Budget::of_work(28));
    // Six faces and twelve edges: 24 + 12 is over 28, at the fifth edge.
    SCHEMA.charges[0].charge(&mut meter, 6).unwrap();
    assert!(SCHEMA.charges[1].charge(&mut meter, 12).is_err());
    assert_eq!(meter.used().work, 29);
}
