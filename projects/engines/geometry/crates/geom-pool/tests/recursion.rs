// SPDX-License-Identifier: Apache-2.0
//! The recursion bound of `geom-pool`'s `halves` module header, run (chapter 28.8; the recursion
//! check of piece G13): `run_halves` halves a wave of tasks until one is left, so a wave of 2^16
//! tasks goes 17 frames deep, and every task still runs exactly once. The bound test runs on every
//! target, WebAssembly's small stack included, over [`Inline`], which has no threads; a native
//! test runs the same halving on rayon pools of one thread (where the halves nest on the one
//! worker) and of two. The bound itself, 64 frames, is the logarithm of the largest slice there
//! can be, which no test can build.

use std::sync::atomic::{AtomicU8, Ordering};

use geom_pool::halves::{Inline, run_halves};

/// The number of tasks in a wave: 2^16, the deepest halving a test can afford, and 2^10 under
/// Miri, which uses about 25 GB for 2^16 (11 frames deep).
const TASKS: usize = if cfg!(miri) { 1 << 10 } else { 1 << 16 };

/// Run a wave of `TASKS` tasks that each count themselves, with `run`, and check each ran once.
fn every_task_runs_once(run: impl FnOnce(&mut [&mut (dyn FnMut() + Send)])) {
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
    run(&mut tasks);
    assert!(
        runs.iter().all(|r| r.load(Ordering::Relaxed) == 1),
        "a task did not run exactly once"
    );
}

/// The halving, with no threads, at 17 frames deep.
#[test]
fn halving_reaches_every_task_of_a_large_wave() {
    every_task_runs_once(|tasks| run_halves(&Inline, tasks));
}

/// The same halving on rayon pools of one thread and of two.
#[cfg(not(target_family = "wasm"))]
#[test]
fn halving_on_rayon_pools_reaches_every_task_of_a_large_wave() {
    use core::num::NonZeroUsize;

    use geom_pool::native::Rayon;
    use geom_run::Pool;

    for threads in [1, 2] {
        let pool = Rayon::new(NonZeroUsize::new(threads).unwrap()).unwrap();
        every_task_runs_once(|tasks| pool.run_all(tasks));
    }
}
