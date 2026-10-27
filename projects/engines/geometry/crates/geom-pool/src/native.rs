//! rayon's work-stealing pool behind `geom-run`'s `Pool` trait, for native targets.
//!
//! What it does: [`Rayon`] owns a `rayon::ThreadPool` of a fixed number of threads and implements
//! [`Pool`]: [`Pool::run_all`] spawns every task into one scope of that pool and returns when all
//! of them have finished. The helper (`geom_run::Exec::with_pool`) decides everything else: the
//! fixed chunks, the order results are read in, the lowest-numbered failure, so every answer is the
//! serial executor's, bit for bit, whatever the pool does (23.4).
//!
//! Guarantees:
//!
//! - **Every task runs exactly once**, on some thread of the pool, in any order: each is spawned
//!   once into the scope, and the scope returns only after every spawned job has finished.
//! - **Re-entrant.** A task may call the helper again with the same pool. The nested scope runs on
//!   the worker thread that opened it, and while it waits that thread runs other jobs of the pool,
//!   rayon's work stealing, instead of blocking; so a nested call makes progress on any number of
//!   threads, one included.
//! - **No panic of its own.** `run_all` panics only to pass on a task's panic, after every task has
//!   finished: rayon's scope catches a job's panic and resumes it when the scope ends. Building a
//!   pool reports a failure to start its threads as [`PoolError`], never a panic.
//! - **The thread count is the caller's.** It is fixed when the pool is built and never read by
//!   the helper, whose chunks do not depend on it (23.4); rayon's own environment variable
//!   (`RAYON_NUM_THREADS`) is not consulted, because the count is set explicitly.
//!
//! Method: rayon's scoped spawning, `ThreadPool::in_place_scope` and `Scope::spawn`, whose
//! documented contract is that the scope returns when every job spawned into it, and every job
//! those spawned, has completed, and that a panic in a job is propagated when the scope ends
//! (<https://docs.rs/rayon/1.12.0/rayon/struct.ThreadPool.html#method.in_place_scope>), over its
//! work-stealing scheduler, after Blumofe and Leiserson, "Scheduling Multithreaded Computations by
//! Work Stealing", Journal of the ACM 46(5), 1999 (`doi:10.1145/324133.324234`). rayon's audit is
//! `projects/kit/notes/G33-rayon-audit.md`.
//!
//! Cost: one scope and one spawned job per task, each a small heap allocation inside rayon, and
//! the pool's threads, started once when it is built and stopped when it is dropped.

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
        self.pool.in_place_scope(|scope| {
            for task in tasks.iter_mut() {
                scope.spawn(move |_| (**task)());
            }
        });
    }
}
