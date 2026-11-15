//! What the tests do differently under Miri: the thread pool they run the parallel path on, and how
//! much work a test of many cases does.
//!
//! [`pool`] gives rayon's pool, through `geom-pool`, the pool the kernel ships with, on a native
//! build; and under Miri a pool of scoped standard-library threads, [`Threads`], with the same
//! number of threads. Either way the kernel's parallel path runs on real threads, chunk by chunk
//! through `geom_run::Exec::with_pool`, and the tests compare its answer with the serial one.
//!
//! Why not rayon under Miri: rayon-core 1.13.0's work-stealing queues use crossbeam-epoch 0.9.21,
//! which turns a reference to an entry of its list of threads into one to the whole thread record
//! around it (`<Local as IsElement>::element_of`), and Miri's Stacked Borrows reports that inside
//! crossbeam whenever a worker collects garbage while stealing (crossbeam-rs/crossbeam#545). The
//! fix (crossbeam-rs/crossbeam#871) changes a public trait and is held for crossbeam-epoch 0.10,
//! which is not released. `geom-pool`'s own tests run rayon under Miri with Tree Borrows instead
//! (the recorded exception in `.github/workflows/kernel-quality.yml`), so rayon's part is checked
//! there, and these tests keep Stacked Borrows on the kernel's own code: every chunk task, the
//! helper's slots and barriers and the mesh code they run, on several threads, where Miri also
//! looks for data races.
//!
//! [`Threads`] keeps the contract of `geom_run::Pool`: every task runs exactly once, each thread
//! taking the next task when it finishes one; a task's nested call starts threads of its own, so
//! it is re-entrant; and a task's panic reaches the caller once every thread has stopped.
//!
//! [`sized`]: Miri interprets each step thousands of times slower than it runs, and stops a test
//! at ten minutes (`.config/nextest.toml`), so a test of thousands of cases or rounds runs fewer of
//! them there: the same inputs, drawn the same way, only fewer, and never so few that a kernel
//! function the whole test reaches is left out (measured natively, building the test with
//! `--cfg miri` and listing the functions each run enters). The one exception says so where it
//! is, `exact.rs`'s pool test. A test keeps its counts' checks at the smaller size.

#![allow(
    dead_code,
    reason = "each test file that includes this module uses only some of it"
)]

/// `native` on a native or WebAssembly build, `miri` under Miri (the module header says why).
pub const fn sized(native: usize, miri: usize) -> usize {
    if cfg!(miri) { miri } else { native }
}

/// Whether a seeded loop runs round `round`: every round natively, and under Miri only those of
/// `chosen`, the fewest rounds that between them reach every kernel function the whole loop does
/// (for the default seed, which is the one Miri runs: its isolation hides the environment). The
/// loop still draws every round's input, so a chosen round is the same input as natively.
pub fn keeps(round: usize, chosen: &[usize]) -> bool {
    !cfg!(miri) || chosen.contains(&round)
}

#[cfg(not(target_family = "wasm"))]
#[allow(
    unused_imports,
    reason = "the test files without a pool test include this module too"
)]
pub use threads::pool;

#[cfg(not(target_family = "wasm"))]
mod threads {
    #![allow(
        clippy::disallowed_methods,
        reason = "the test pool starts threads, which clippy.toml bans in kernel code (chapter 23.4)"
    )]
    #![allow(clippy::unwrap_used, reason = "a poisoned lock is a test failure")]

    use std::num::NonZeroUsize;
    use std::panic::{self, AssertUnwindSafe};
    use std::sync::Mutex;
    use std::thread;

    use geom_run::Pool;

    /// A pool of `n` threads: the calling thread and up to `n - 1` scoped threads, never more than
    /// tasks, each claiming the next task when it finishes one.
    pub struct Threads(pub NonZeroUsize);

    impl Pool for Threads {
        fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
            let helpers = self.0.get().min(tasks.len()).saturating_sub(1);
            let queue = Mutex::new(tasks.iter_mut());
            let work = || {
                loop {
                    let next = queue.lock().unwrap().next();
                    match next {
                        Some(task) => task(),
                        None => return,
                    }
                }
            };
            let panicked = thread::scope(|scope| {
                let handles: Vec<_> = (0..helpers).map(|_| scope.spawn(work)).collect();
                let mut first = panic::catch_unwind(AssertUnwindSafe(&work)).err();
                for handle in handles {
                    if let Err(payload) = handle.join() {
                        first.get_or_insert(payload);
                    }
                }
                first
            });
            if let Some(payload) = panicked {
                panic::resume_unwind(payload);
            }
        }
    }

    /// A pool of `threads` threads: rayon's natively, [`Threads`] under Miri (the module header
    /// says why).
    pub fn pool(threads: usize) -> Box<dyn Pool> {
        let threads = NonZeroUsize::new(threads).unwrap();
        if cfg!(miri) {
            Box::new(Threads(threads))
        } else {
            Box::new(geom_pool::native::Rayon::new(threads).unwrap())
        }
    }
}
