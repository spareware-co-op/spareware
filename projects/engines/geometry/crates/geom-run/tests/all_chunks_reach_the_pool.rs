//! Tests that `geom_run`'s executor hands every chunk to its pool, in the waves `geom_run::pool`
//! describes.
//!
//! What this file proves: a phase of 200 one-item chunks gives the pool a first wave of
//! `WAVE_FIRST` tasks, then one of twice that, then the rest, and its answer is the serial one.
//!
//! The file's name sorts before every other test file's on purpose. Cargo runs a crate's
//! integration test binaries one after another in name order and stops at the first that fails
//! (chapter 28.8: the first test binary that fails or hangs decides). An executor that never
//! handed its tasks to the pool would still answer correctly, since a task the pool never ran is
//! run on the calling thread when its result is read, but `attack_g04.rs`'s tests that wait for a
//! helper thread would each wait out their bound (more than two minutes in all), and the mutation
//! gate would see a timeout before `interface.rs` could report the pool unused. Here it fails at
//! once.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(clippy::unwrap_used)]

use std::num::NonZeroUsize;
use std::sync::Mutex;

use geom_run::pool::WAVE_FIRST;
use geom_run::{Budget, ChunkPlan, Exec, Phase, Pool, Stopped, WorkMeter};

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
