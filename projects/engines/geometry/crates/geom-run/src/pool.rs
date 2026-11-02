//! Threads come from the caller: the [`Pool`] trait, and how the helper hands a pool its work.
//!
//! Kernel crates never start threads (architecture chapter 23.4), and this crate is `no_std`: it
//! has no thread of its own. A program that wants threads passes a [`Pool`] in with each call, as
//! it passes its [`Cancel`](crate::Cancel). The pool adapter lives outside the kernel, beside the
//! facade (`geom-pool`: rayon on native targets, Web Workers in a browser). With no pool the
//! helper runs every task on the calling thread, in order, and the answer is the same.
//!
//! The helper hands a pool its tasks in waves: a bounded batch of independent chunk tasks, then a
//! barrier while the calling thread reads their results in index order, then the next batch.
//! Waves start at [`WAVE_FIRST`] chunks and double up to [`WAVE_MAX`], a schedule fixed by the
//! chunk count alone. So the helper never holds more than `WAVE_MAX` tasks, its bookkeeping does
//! not grow with the problem, and after a failure it runs at most one wave of chunks that the
//! answer does not need. A wave of a single task, a phase of one chunk or a phase's last, runs on
//! the calling thread as it is read: a pool could only add a wake-up and a barrier to it.
//!
//! Method: the superstep of Valiant's bulk-synchronous parallel model ("A bridging model for
//! parallel computation", Communications of the ACM 33(8), 1990,
//! <https://doi.org/10.1145/79173.79181>): independent work, a barrier, then a serial step that
//! reads the results. No atomic counter, lock or shared mutable state is used; each task writes
//! only its own slot.
//!
//! Cost: a phase of c chunks runs in at most 7 + ⌈c / `WAVE_MAX`⌉ waves (six doublings from
//! `WAVE_FIRST` reach `WAVE_MAX`). Each wave of w > 1 chunks costs two lists of w entries (the
//! tasks and the handles passed to the pool) and one barrier, and a wave of one task neither. The
//! slots of the current wave, each a chunk's payload and output, are the only other storage. With
//! no pool there is no list and no barrier: each chunk runs just before it is read.
//!
//! Invariants:
//!
//! - Whatever a pool does, the answer is right. A task run twice writes its slot once (a filled
//!   slot is skipped); a task never run, as a wave's only task never is, runs on the calling
//!   thread when its result is read, with the same payload and so the same meter. Only a panic
//!   inside the pool escapes, and pools must not panic.
//! - Results are read in index order, never in the order tasks finished.

use alloc::collections::TryReserveError;
use alloc::vec::Vec;
use core::ops::ControlFlow;

/// Threads for the helper, supplied by the caller (architecture chapter 23.4).
///
/// Implementations live outside the kernel, in `std` code that may start threads. The helper's
/// answer never depends on the pool: a pool decides only how fast the answer comes. The helper
/// hands it at least two tasks at a time, and runs a lone task itself.
pub trait Pool: Sync {
    /// Run every task exactly once, on any threads, in any order, and return when all have
    /// finished. Must be re-entrant: a task may call the helper again with this pool, and that
    /// call must make progress without waiting for a thread that is blocked on it (work-stealing
    /// pools such as rayon's are re-entrant). Must not panic, except to pass on a task's panic.
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]);
}

/// The size of the first wave, in chunk tasks.
pub const WAVE_FIRST: usize = 64;

/// The size of the largest wave, in chunk tasks.
pub const WAVE_MAX: usize = 4096;

/// Run `work` on every slot of one wave, on `pool`.
fn run_wave<S, W>(pool: &dyn Pool, slots: &mut [S], work: &W) -> Result<(), TryReserveError>
where
    S: Send,
    W: Fn(&mut S) + Sync,
{
    let mut tasks = Vec::new();
    tasks.try_reserve_exact(slots.len())?;
    for slot in slots.iter_mut() {
        tasks.push(move || work(slot));
    }
    let mut handles: Vec<&mut (dyn FnMut() + Send)> = Vec::new();
    handles.try_reserve_exact(tasks.len())?;
    for task in &mut tasks {
        handles.push(task);
    }
    pool.run_all(&mut handles);
    Ok(())
}

/// Run `run` on every payload, then hand the outputs to `absorb` in payload order until it
/// breaks. With a pool, payloads run in waves; without one, each runs on the calling thread just
/// before it is absorbed, so nothing after a break runs at all.
///
/// # Errors
///
/// A refused allocation for a wave's bookkeeping.
pub(crate) fn drive<P, O, R, A>(
    pool: Option<&dyn Pool>,
    mut payloads: impl Iterator<Item = P>,
    run: &R,
    mut absorb: A,
) -> Result<(), TryReserveError>
where
    P: Send,
    O: Send,
    R: Fn(&mut P) -> O + Sync,
    A: FnMut(O) -> ControlFlow<()>,
{
    let Some(pool) = pool else {
        for mut payload in payloads {
            if absorb(run(&mut payload)).is_break() {
                break;
            }
        }
        return Ok(());
    };
    let fill = |slot: &mut (P, Option<O>)| {
        if slot.1.is_none() {
            slot.1 = Some(run(&mut slot.0));
        }
    };
    let mut slots: Vec<(P, Option<O>)> = Vec::new();
    let mut wave = WAVE_FIRST;
    loop {
        // `slots` is empty here, so this reserves room for the whole wave.
        slots.try_reserve(wave)?;
        slots.extend(payloads.by_ref().take(wave).map(|payload| (payload, None)));
        if slots.is_empty() {
            return Ok(());
        }
        // A wave of one task runs on the calling thread, when its result is read below: handing
        // it to the pool would cost a wake-up and a barrier and change nothing else.
        if slots.len() > 1 {
            run_wave(pool, &mut slots, &fill)?;
        }
        for (mut payload, done) in slots.drain(..) {
            // `None` for a wave of one task, or if the pool broke its contract and skipped it.
            let out = done.unwrap_or_else(|| run(&mut payload));
            if absorb(out).is_break() {
                return Ok(());
            }
        }
        wave = wave.saturating_mul(2).min(WAVE_MAX);
    }
}
