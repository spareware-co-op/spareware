// SPDX-License-Identifier: Apache-2.0
//! The recursion bound of `geom-pool`'s module header, run (chapter 28.8; the recursion check of
//! piece G13): `Rayon::run_all` halves a wave of tasks until one is left, so a wave of 2^16 tasks
//! goes 17 frames deep, and every task still runs exactly once, on a pool of one thread (where the
//! halves nest on the one worker) and on a pool of two. The bound itself, 64 frames, is the
//! logarithm of the largest slice there can be, which no test can build.
#![cfg(not(target_family = "wasm"))]
#![allow(clippy::unwrap_used, reason = "tests: a failure stops the test")]

use core::num::NonZeroUsize;
use std::sync::atomic::{AtomicU8, Ordering};

use geom_pool::native::Rayon;
use geom_run::Pool;

/// A wave of 2^16 tasks, the deepest halving a test can afford under a debugger or Miri's smoke
/// run.
#[test]
fn halving_reaches_every_task_of_a_large_wave() {
    // Miri interprets a wave of 2^16 tasks in tens of gigabytes; 2^10 is 11 frames deep.
    const TASKS: usize = if cfg!(miri) { 1 << 10 } else { 1 << 16 };
    for threads in [1, 2] {
        let pool = Rayon::new(NonZeroUsize::new(threads).unwrap()).unwrap();
        let runs: Vec<AtomicU8> = (0..TASKS).map(|_| AtomicU8::new(0)).collect();
        let mut work: Vec<_> = runs
            .iter()
            .map(|r| {
                move || {
                    r.fetch_add(1, Ordering::Relaxed);
                }
            })
            .collect();
        let mut tasks: Vec<&mut (dyn FnMut() + Send)> = work
            .iter_mut()
            .map(|w| w as &mut (dyn FnMut() + Send))
            .collect();
        pool.run_all(&mut tasks);
        assert!(
            runs.iter().all(|r| r.load(Ordering::Relaxed) == 1),
            "{threads} threads: a task did not run exactly once"
        );
    }
}
