//! Tests that `geom_run`'s executor hands every chunk to its pool, in the waves `geom_run::pool`
//! describes.
//!
//! What this file proves: a phase of 200 one-item chunks gives the pool a first wave of
//! `WAVE_FIRST` tasks, then one of twice that, then the rest, and its answer is the serial one; a
//! wave of a single task never reaches the pool, and runs on the calling thread with the same
//! answer, the same charge, the same budget stop and the same reads of the cancellation as the
//! serial executor and as a wave the pool runs.
//!
//! The file's name sorts before every other test file's on purpose. Cargo runs a crate's
//! integration test binaries one after another in name order and stops at the first that fails
//! (chapter 28.8: the first test binary that fails or hangs decides). An executor that never
//! handed its tasks to the pool would still answer correctly, since a task the pool never ran is
//! run on the calling thread when its result is read, but `attack_g04.rs`'s tests that wait for a
//! helper thread would each wait out their bound (more than two minutes in all), and the mutation
//! gate would see a timeout before `interface.rs` could report the pool unused. Here it fails at
//! once.

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

use std::num::NonZeroUsize;
use std::sync::Mutex;

use common::Countdown;
use geom_run::pool::WAVE_FIRST;
use geom_run::{Budget, ChunkPlan, Exec, Phase, Pool, RunError, Stopped, Usage, WorkMeter};

/// A pool that runs every task on the calling thread and records how many tasks each call gives
/// it.
struct Recording(Mutex<Vec<usize>>);

impl Pool for Recording {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        self.0.lock().unwrap().push(tasks.len());
        for task in tasks {
            task();
        }
    }
}

#[test]
fn every_chunk_reaches_the_pool_in_doubling_waves() {
    let pool = Recording(Mutex::new(Vec::new()));
    let items: Vec<u32> = (0..200).collect();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let one = ChunkPlan::new(NonZeroUsize::new(1).unwrap());
    let got =
        Exec::with_pool(&pool).map(&mut meter, Phase::new("waves"), one, &items, |_, x, _| {
            Ok::<u32, Stopped>(x + 1)
        });
    assert_eq!(got, Ok(items.iter().map(|x| x + 1).collect()));
    assert_eq!(
        *pool.0.lock().unwrap(),
        [WAVE_FIRST, 2 * WAVE_FIRST, 200 - 3 * WAVE_FIRST],
        "the waves the pool was given"
    );
}

/// What a phase of `items` in chunks of `size`, each item charging 3 units, gives on `exec` under
/// `budget` with a cancellation that says stop at its `stop`th question: the answer, the meter's
/// use and how often the cancellation was asked.
fn charged(
    exec: &Exec<'_>,
    (items, size): (u32, usize),
    budget: Budget,
    stop: usize,
) -> (Result<Vec<u32>, RunError>, Usage, usize) {
    let source = Countdown::new(stop);
    let items: Vec<u32> = (0..items).collect();
    let mut meter = WorkMeter::new(budget).with_cancel(&source);
    let plan = ChunkPlan::new(NonZeroUsize::new(size).unwrap());
    let got = exec.map(&mut meter, Phase::new("lone"), plan, &items, |_, x, m| {
        m.charge(3)?;
        Ok::<u32, Stopped>(x * 2)
    });
    (got, meter.used(), source.asked())
}

#[test]
fn a_lone_task_runs_on_the_calling_thread() {
    let pool = Recording(Mutex::new(Vec::new()));
    let items: Vec<u32> = (0..5).collect();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let eight = ChunkPlan::new(NonZeroUsize::new(8).unwrap());
    let got =
        Exec::with_pool(&pool).map(&mut meter, Phase::new("one"), eight, &items, |_, x, _| {
            Ok::<u32, Stopped>(x + 1)
        });
    assert_eq!(got, Ok(vec![1, 2, 3, 4, 5]));
    assert!(
        pool.0.lock().unwrap().is_empty(),
        "a phase of one chunk reached the pool"
    );
    // 65 chunks: a first wave of 64 for the pool, then the last chunk alone, on this thread.
    let items: Vec<u32> = (0..65).collect();
    let one = ChunkPlan::new(NonZeroUsize::new(1).unwrap());
    let got = Exec::with_pool(&pool).map(&mut meter, Phase::new("last"), one, &items, |_, x, _| {
        Ok::<u32, Stopped>(x + 1)
    });
    assert_eq!(got, Ok(items.iter().map(|x| x + 1).collect()));
    assert_eq!(*pool.0.lock().unwrap(), [WAVE_FIRST]);
}

#[test]
fn a_lone_task_gives_what_the_pool_gives() {
    let pool = Recording(Mutex::new(Vec::new()));
    let on_pool = Exec::with_pool(&pool);
    let serial = Exec::serial();
    // Unlimited, over budget part way through, and cancelled at the second question.
    for (budget, stop) in [
        (Budget::UNLIMITED, usize::MAX),
        (Budget::of_work(20), usize::MAX),
        (Budget::UNLIMITED, 2),
    ] {
        // One chunk of 10 items: a wave of one, run here.
        pool.0.lock().unwrap().clear();
        let alone = charged(&on_pool, (10, 16), budget, stop);
        assert!(
            pool.0.lock().unwrap().is_empty(),
            "the lone task reached the pool"
        );
        assert_eq!(alone, charged(&serial, (10, 16), budget, stop));
        // Two chunks of the same items: one wave of two, run by the pool.
        let pair = charged(&on_pool, (10, 5), budget, stop);
        assert_eq!(*pool.0.lock().unwrap(), [2]);
        assert_eq!(pair, charged(&serial, (10, 5), budget, stop));
        assert_eq!(pair.0, alone.0);
        assert_eq!(pair.1, alone.1);
    }
}
