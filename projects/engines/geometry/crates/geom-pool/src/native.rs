//! rayon's work-stealing pool behind `geom-run`'s `Pool` trait, for native targets.
//!
//! What it does: [`Rayon`] owns a `rayon::ThreadPool` of a fixed number of threads and implements
//! [`Pool`]: [`Pool::run_all`] splits the tasks in halves and runs the halves with rayon's `join`,
//! on that pool, and returns when all of them have finished. The helper (`geom_run::Exec::with_pool`) decides everything else: the
//! fixed chunks, the order results are read in, the lowest-numbered failure, so every answer is the
//! serial executor's, bit for bit, whatever the pool does (23.4).
//!
//! Guarantees:
//!
//! - **Every task runs exactly once**, on some thread of the pool, in any order: the slice is split
//!   in two until one task is left, and `join` returns only after both of its halves have run. (A
//!   lone task, which the helper never hands over, runs on the calling thread.)
//! - **Re-entrant.** A task may call the helper again with the same pool. The nested call runs on
//!   the worker thread that made it, and while it waits for a half that another thread took, that
//!   thread runs other jobs of the pool, rayon's work stealing, instead of blocking; so a nested
//!   call makes progress on any number of threads, one included.
//! - **No panic of its own.** `run_all` panics only to pass on a task's panic, after every task has
//!   finished: `join` waits for its other half when one panics and then resumes the panic. Building
//!   a pool reports a failure to start its threads as [`PoolError`], never a panic.
//! - **The thread count is the caller's.** It is fixed when the pool is built and never read by
//!   the helper, whose chunks do not depend on it (23.4); rayon's own environment variable
//!   (`RAYON_NUM_THREADS`) is not consulted, because the count is set explicitly.
//!
//! Method: rayon's fork-join, `ThreadPool::join`, whose documented contract is that both closures
//! have run when it returns, that it runs them on the pool (on a worker already, it runs in
//! place), and that a panic in either is propagated after the other has finished
//! (<https://docs.rs/rayon/1.12.0/rayon/struct.ThreadPool.html#method.join>), over its
//! work-stealing scheduler, after Blumofe and Leiserson, "Scheduling Multithreaded Computations by
//! Work Stealing", Journal of the ACM 46(5), 1999 (`doi:10.1145/324133.324234`). rayon's audit is
//! `projects/kit/notes/G33-rayon-audit.md`.
//!
//! Why `join` and not a scope: rayon-core 1.13.0's `Scope::spawn` lets the thread that opened the
//! scope free it while a finishing job still holds a reference into it, which Miri rejects under
//! Tree Borrows (rayon-rs/rayon#1328, fixed on rayon's main branch by rayon-rs/rayon#1330, merged
//! 2026-10-27, in no release yet). `join` keeps its jobs on the caller's stack and has no such
//! window. Once a rayon-core release carries #1330 a scope would do again, but `join` is no worse.
//!
//! Recursion:
//!
//! - Cycle: `run_halves`.
//!   Bound: 64.
//!   Why: each call hands `join` two halves, each at most half the slice (rounded up), and a slice
//!   of one task or none ends the recursion, so `n` tasks need at most `ceil(log2 n) + 1` frames,
//!   and a slice is at most `isize::MAX` long, under 2^63 tasks.
//!   Test: `halving_reaches_every_task_of_a_large_wave`.
//!
//! Cost: `n - 1` calls of `join` for `n` tasks, each pushing one job on the worker's own deque
//! without a heap allocation, and the pool's threads, started once when it is built and stopped
//! when it is dropped.

use core::fmt;
use core::num::NonZeroUsize;

use geom_run::Pool;

/// A failure to build a [`Rayon`] pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolError {
    /// The operating system would not start the pool's threads.
    CannotStartThreads,
}

impl fmt::Display for PoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PoolError::CannotStartThreads => f.write_str("the pool's threads could not be started"),
        }
    }
}

impl core::error::Error for PoolError {}

/// A rayon thread pool of a fixed number of threads, as a `geom-run` [`Pool`].
#[derive(Debug)]
pub struct Rayon {
    /// The pool, owned: its threads stop when this is dropped.
    pool: rayon::ThreadPool,
}

impl Rayon {
    /// A pool of `threads` threads.
    ///
    /// # Errors
    ///
    /// [`PoolError::CannotStartThreads`] if the operating system will not start them.
    pub fn new(threads: NonZeroUsize) -> Result<Self, PoolError> {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads.get())
            .build()
            .map(|pool| Self { pool })
            .map_err(|_| PoolError::CannotStartThreads)
    }

    /// How many threads the pool has.
    #[must_use]
    pub fn threads(&self) -> usize {
        self.pool.current_num_threads()
    }
}

impl Pool for Rayon {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        run_halves(&self.pool, tasks);
    }
}

/// Run every task of `tasks` on `pool`: split them in halves and run the two with `join`.
///
/// The first call comes from any thread and `ThreadPool::join` moves it onto a worker; a later
/// call is already on one, where `join` runs in place and the second half is open to thieves.
#[allow(
    clippy::disallowed_methods,
    reason = "clippy.toml bans `ThreadPool::join` for every kernel crate; this is the one adapter               that may call it, and the only place that does"
)]
fn run_halves(pool: &rayon::ThreadPool, tasks: &mut [&mut (dyn FnMut() + Send)]) {
    match tasks {
        [] => {}
        [task] => (**task)(),
        _ => {
            let (first, second) = tasks.split_at_mut(tasks.len() / 2);
            pool.join(|| run_halves(pool, first), || run_halves(pool, second));
        }
    }
}
