//! The engine under every helper call: run a phase's items chunk by chunk, then decide the answer
//! as a serial loop would.
//!
//! A helper call is one phase of a meter (architecture chapter 23.10, "Checked at fixed phase
//! boundaries"). With R the budget the meter has left when the phase starts, the answer is exactly
//! what this serial loop gives:
//!
//! ```text
//! total = 0
//! for each item, in index order:
//!     run the item with a meter of its own, capped at R
//!     total += what the item used
//!     if the item went over R:     fail BudgetExceeded  (the item's own limit)
//!     if the item failed:          fail Failed(its error)
//!     if the item is a match:      stop (find_first only)
//! if total > R:                    fail BudgetExceeded  (at the end of the phase)
//! R -= total
//! ```
//!
//! The first failure of any kind wins, and its count is what the loop had used: items above the
//! failing one count zero. A total over R fails only at the end, so a later item's own error beats
//! it. Cancellation and refused allocations stop the loop wherever they are seen.
//!
//! Executors compute chunks in any order, on any threads ([`crate::pool::drive`]); then [`Walk`]
//! reads the chunk results in index order and is that loop. A chunk stops early only at its own
//! first failure or match, which ends the loop there anyway; a chunk the walk never reaches does
//! not matter. So the answer cannot depend on the pool, the thread count or the chunk size: that
//! is the whole determinism argument.
//!
//! Method: the serial-equivalence rule of chapter 23.4 ("Ordered results", "Ordered errors": the
//! error reported is the lowest-numbered failure, as in rayon's `find_first`,
//! <https://github.com/rayon-rs/rayon/blob/ee0a00bdb1ab039e178a215ad5712fb7fa58e58f/src/iter/mod.rs#L1675-L1680>)
//! and the phase rule of chapter 23.10.
//!
//! Cost: per item, one fresh meter (a few words, no allocation), one call of the function and,
//! for a value kept, an amortised push onto its chunk's list; per chunk, one task and one read of
//! the cancellation every [`POLL_EVERY`] items. The walk appends each chunk's list to the result in
//! index order, so a kept value is moved twice. Nothing else is allocated but the pool's wave
//! bookkeeping (`crate::pool`).
//!
//! Invariants:
//!
//! - Every item runs under a fresh meter capped at R, so an item's verdict never depends on the
//!   other items or on how they were chunked.
//! - The meter, not the item's return value, says whether an item stopped.
//! - Results are gathered in index order; a chunk result missing from a wave is computed on the
//!   calling thread, so the walk never skips one.

use alloc::collections::TryReserveError;
use alloc::vec::Vec;
use core::ops::ControlFlow;

use crate::budget::{Budget, BudgetKind, Phase, Usage};
use crate::cancel::Cancel;
use crate::error::RunError;
use crate::meter::{POLL_EVERY, Stop, WorkMeter};
use crate::plan::ChunkPlan;
use crate::pool::{Pool, drive};

/// What every chunk of a phase shares: the cap of each item's meter and the cancellation.
pub(crate) struct Env<'c> {
    cap: Budget,
    cancel: Option<&'c dyn Cancel>,
}

impl<'c> Env<'c> {
    /// Items capped at `cap`, reading `cancel`.
    pub(crate) const fn new(cap: Budget, cancel: Option<&'c dyn Cancel>) -> Self {
        Self { cap, cancel }
    }

    /// The environment of a phase of `meter`: items capped at what it has left.
    pub(crate) fn of(meter: &WorkMeter<'c>) -> Self {
        Self::new(meter.left(), meter.cancellation())
    }

    /// A fresh meter for one item.
    pub(crate) const fn meter(&self) -> WorkMeter<'c> {
        WorkMeter::child(self.cap, self.cancel)
    }

    /// The cap of each item's meter.
    pub(crate) const fn cap(&self) -> Budget {
        self.cap
    }

    /// Whether the caller has asked to stop.
    pub(crate) fn cancelled(&self) -> bool {
        self.cancel.is_some_and(Cancel::requested)
    }

    /// Whether to read the cancellation before the item at `offset` in its chunk: before the
    /// first item and every [`POLL_EVERY`] items after it.
    pub(crate) fn cancelled_at(&self, offset: usize) -> bool {
        offset.is_multiple_of(POLL_EVERY) && self.cancelled()
    }
}

/// How a chunk's run ended.
pub(crate) enum End<E> {
    /// Every item ran.
    Complete,
    /// The last item run was a match (`find_first`); the walk stops after it.
    Found,
    /// The last item run failed with this error.
    Failed(E),
    /// The last item run went over its cap in this kind.
    Over(BudgetKind),
    /// The caller's cancellation was seen.
    Cancelled,
    /// An allocation was refused.
    OutOfMemory,
}

impl<E> End<E> {
    /// The end that a meter's stop means.
    const fn of_stop(stop: Stop) -> Self {
        match stop {
            Stop::OverBudget(kind) => End::Over(kind),
            Stop::Cancelled => End::Cancelled,
            Stop::OutOfMemory => End::OutOfMemory,
        }
    }
}

/// The verdict on one item: its value, or how it ended the chunk. A stopped meter wins over
/// whatever the item returned.
pub(crate) fn judge<X, E>(meter: &WorkMeter<'_>, result: Result<X, E>) -> Result<X, End<E>> {
    match meter.stopped() {
        Some(stop) => Err(End::of_stop(stop)),
        None => result.map_err(End::Failed),
    }
}

/// What one chunk produced: what it gathered from its items in index order, what its items used,
/// and how it ended.
pub(crate) struct ChunkOut<G, E> {
    pub(crate) gathered: G,
    pub(crate) used: Usage,
    pub(crate) end: End<E>,
}

impl<G, E> ChunkOut<G, E> {
    /// A chunk that has gathered nothing yet.
    pub(crate) const fn new(gathered: G) -> Self {
        Self {
            gathered,
            used: Usage::ZERO,
            end: End::Complete,
        }
    }

    /// This chunk, ended by `end`.
    pub(crate) fn ended(mut self, end: End<E>) -> Self {
        self.end = end;
        self
    }
}

/// What a phase gathers from its chunks, joined in chunk order.
pub(crate) trait Gather: Sized {
    /// Nothing gathered.
    fn empty() -> Self;
    /// Join a later chunk's gathering after this one.
    fn append(&mut self, later: Self) -> Result<(), TryReserveError>;
}

/// A [`Gather`] that takes item values of type `X`.
pub(crate) trait Sink<X>: Gather {
    /// Take item `index`'s value. `Ok(true)` when the item is a match that ends the phase.
    fn add(&mut self, index: usize, value: X) -> Result<bool, TryReserveError>;
}

/// Every item's value, in index order.
pub(crate) struct Values<U>(pub(crate) Vec<U>);

impl<U> Gather for Values<U> {
    fn empty() -> Self {
        Values(Vec::new())
    }

    fn append(&mut self, mut later: Self) -> Result<(), TryReserveError> {
        self.0.try_reserve(later.0.len())?;
        self.0.append(&mut later.0);
        Ok(())
    }
}

impl<U> Sink<U> for Values<U> {
    fn add(&mut self, _: usize, value: U) -> Result<bool, TryReserveError> {
        // Amortised growth: room grows with the items that have run (at most about twice what
        // they need), never with the chunk's length.
        self.0.try_reserve(1)?;
        self.0.push(value);
        Ok(false)
    }
}

/// The indices of the items kept, in index order.
pub(crate) struct Kept(pub(crate) Vec<usize>);

impl Gather for Kept {
    fn empty() -> Self {
        Kept(Vec::new())
    }

    fn append(&mut self, mut later: Self) -> Result<(), TryReserveError> {
        self.0.try_reserve(later.0.len())?;
        self.0.append(&mut later.0);
        Ok(())
    }
}

impl Sink<bool> for Kept {
    fn add(&mut self, index: usize, keep: bool) -> Result<bool, TryReserveError> {
        if keep {
            self.0.try_reserve(1)?;
            self.0.push(index);
        }
        Ok(false)
    }
}

/// The index of the first match.
pub(crate) struct First(pub(crate) Option<usize>);

impl Gather for First {
    fn empty() -> Self {
        First(None)
    }

    fn append(&mut self, later: Self) -> Result<(), TryReserveError> {
        if self.0.is_none() {
            self.0 = later.0;
        }
        Ok(())
    }
}

impl Sink<bool> for First {
    fn add(&mut self, index: usize, hit: bool) -> Result<bool, TryReserveError> {
        if hit {
            self.0 = Some(index);
        }
        Ok(hit)
    }
}

/// The chunks of `items` under `plan`, each with the index of its first item.
pub(crate) fn chunk_starts<T>(plan: ChunkPlan, items: &[T]) -> impl Iterator<Item = (usize, &[T])> {
    items
        .chunks(plan.size().get())
        .scan(0, |next: &mut usize, chunk| {
            let start = *next;
            // The starts are prefix sums of chunk lengths, so they never pass `items.len()`: this
            // never saturates.
            *next = start.saturating_add(chunk.len());
            Some((start, chunk))
        })
}

/// Run `f` on each item of one chunk, whose first item is `start`, gathering the values in `S`.
pub(crate) fn run_items<T, X, E, S, F>(
    env: &Env<'_>,
    start: usize,
    items: &[T],
    f: &F,
) -> ChunkOut<S, E>
where
    S: Sink<X>,
    F: Fn(usize, &T, &mut WorkMeter<'_>) -> Result<X, E>,
{
    let mut out = ChunkOut::new(S::empty());
    for (offset, item) in items.iter().enumerate() {
        if env.cancelled_at(offset) {
            return out.ended(End::Cancelled);
        }
        // `start + offset` is an item's index, below `items.len()` of the whole phase: this never
        // saturates.
        let index = start.saturating_add(offset);
        let mut meter = env.meter();
        let result = f(index, item, &mut meter);
        out.used = out.used.plus(meter.used());
        match judge(&meter, result) {
            Ok(value) => match out.gathered.add(index, value) {
                Ok(false) => {}
                Ok(true) => return out.ended(End::Found),
                Err(_) => return out.ended(End::OutOfMemory),
            },
            Err(end) => return out.ended(end),
        }
    }
    out
}

/// The serial loop of the module documentation, over chunk results in index order.
struct Walk<G, E> {
    gathered: G,
    total: Usage,
    end: End<E>,
}

impl<G: Gather, E> Walk<G, E> {
    /// Take the next chunk's result; break when the loop stops in it.
    fn absorb(&mut self, out: ChunkOut<G, E>) -> ControlFlow<()> {
        self.total = self.total.plus(out.used);
        if self.gathered.append(out.gathered).is_err() {
            self.end = End::OutOfMemory;
            return ControlFlow::Break(());
        }
        match out.end {
            End::Complete => ControlFlow::Continue(()),
            end => {
                self.end = end;
                ControlFlow::Break(())
            }
        }
    }
}

/// Run one phase of `meter`: every payload (a chunk) through `run`, on `pool` or serially, and
/// settle the answer into `meter` by the phase rule.
pub(crate) fn run_phase<'c, P, G, E, R>(
    pool: Option<&dyn Pool>,
    meter: &mut WorkMeter<'c>,
    phase: Phase,
    payloads: impl Iterator<Item = P>,
    run: &R,
) -> Result<G, RunError<E>>
where
    P: Send,
    G: Gather + Send,
    E: Send,
    R: Fn(&Env<'c>, &mut P) -> ChunkOut<G, E> + Sync,
{
    if let Some(error) = meter.stop_error(phase) {
        return Err(error);
    }
    let env = Env::of(meter);
    let each = |payload: &mut P| run(&env, payload);
    let mut walk = Walk {
        gathered: G::empty(),
        total: Usage::ZERO,
        end: End::Complete,
    };
    if drive(pool, payloads, &each, |out| walk.absorb(out)).is_err() {
        walk.end = End::OutOfMemory;
    }
    settle(meter, phase, walk.total, walk.end)?;
    Ok(walk.gathered)
}

/// Settle a phase that used `total` and ended with `end` into `meter`.
pub(crate) fn settle<E>(
    meter: &mut WorkMeter<'_>,
    phase: Phase,
    total: Usage,
    end: End<E>,
) -> Result<(), RunError<E>> {
    match end {
        End::Complete | End::Found => match meter.absorb(total) {
            None => Ok(()),
            // The items' total is over what the phase had left.
            Some(kind) => Err(RunError::BudgetExceeded(meter.exceeded(kind, phase))),
        },
        End::Failed(error) => {
            // The phase fails with the item's error; if what it used still takes the operation
            // over its budget, the meter stops, and the next phase reports it.
            meter.absorb(total);
            Err(RunError::Failed(error))
        }
        End::Over(kind) => {
            // An item over its cap: that kind is the one reported, even if the total also passed
            // the other kind first.
            meter.latch(Stop::OverBudget(kind));
            meter.absorb(total);
            Err(RunError::BudgetExceeded(meter.exceeded(kind, phase)))
        }
        End::Cancelled => {
            meter.latch(Stop::Cancelled);
            Err(RunError::Cancelled)
        }
        End::OutOfMemory => {
            meter.latch(Stop::OutOfMemory);
            Err(RunError::OutOfMemory)
        }
    }
}
