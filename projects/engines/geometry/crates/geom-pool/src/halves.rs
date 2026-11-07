//! Running a wave of tasks by halving it, over any fork-join, for every target.
//!
//! What it does: [`run_halves`] runs every task of a slice by splitting the slice in two and
//! handing the two halves to a [`Join`], which runs both and returns when both are done; each half
//! is split again until one task is left. [`native::Rayon`](crate::native::Rayon) gives it rayon's
//! `join` on a pool. [`Inline`] runs the two halves one after the other on the calling thread, so
//! the same halving runs with no threads, on WebAssembly too, and its depth is tested there.
//!
//! Guarantees:
//!
//! - **Every task runs exactly once**: a slice of no tasks does nothing, a slice of one runs it,
//!   and any other slice is split into two non-empty halves that between them hold every task.
//! - **The caller's `Join` decides where tasks run and in what order**; this code only decides the
//!   split, which depends on the number of tasks and on nothing else.
//! - **No panic of its own.** A task's panic passes through [`Join::join`], whose contract it is
//!   to finish both halves before passing it on.
//!
//! Method: divide and conquer over a fork-join (Blumofe and Leiserson, "Scheduling Multithreaded
//! Computations by Work Stealing", Journal of the ACM 46(5), 1999, `doi:10.1145/324133.324234`), the
//! shape of rayon's own `join`-based parallel iterators, with the halves cut at the midpoint so
//! that the depth is the logarithm of the number of tasks.
//!
//! The halving's recursion bound is stated, with the rayon `Join` it also runs over, in the
//! header of [`native`](crate::native).
//!
//! Cost: `n - 1` calls of `Join::join` for `n` tasks, no allocation, and `ceil(log2 n) + 1` frames
//! of stack at the deepest.

// SPDX-License-Identifier: Apache-2.0

/// A fork-join: run two closures, possibly on different threads, and return when both are done.
pub trait Join {
    /// Run `first` and `second`, each exactly once, and return when both have finished. If one
    /// panics, finish the other and then pass the panic on.
    fn join<A, B>(&self, first: A, second: B)
    where
        A: FnOnce() + Send,
        B: FnOnce() + Send;
}

/// A [`Join`] with no threads: `first`, then `second`, on the calling thread.
#[derive(Clone, Copy, Debug, Default)]
pub struct Inline;

impl Join for Inline {
    fn join<A, B>(&self, first: A, second: B)
    where
        A: FnOnce() + Send,
        B: FnOnce() + Send,
    {
        first();
        second();
    }
}

/// Run every task of `tasks` by halving: split them in two and `join` the halves, down to one task.
pub fn run_halves<J: Join + Sync>(join: &J, tasks: &mut [&mut (dyn FnMut() + Send)]) {
    match tasks {
        [] => {}
        [task] => (**task)(),
        _ => {
            let (first, second) = tasks.split_at_mut(tasks.len() / 2);
            join.join(|| run_halves(join, first), || run_halves(join, second));
        }
    }
}
