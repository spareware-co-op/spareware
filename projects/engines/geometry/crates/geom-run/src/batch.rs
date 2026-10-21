//! Batch calls with one result per item: `map_each` under the batch rule.
//!
//! A batch call is many operations, one per item (architecture chapter 23.10, "Batch calls: one
//! rule, defined here"). Each item runs under its own `per_item` budget and the phase rule, so
//! whether an item passes never depends on how many other items the batch holds or what they cost.
//! If the caller also sets a total, the work and logical bytes the items used are summed in the
//! order of the items' keys once they have run: the caller's stable keys, never their positions
//! (chapter 10.1). The item at which the running sum first passes the total, and every item after
//! it in key order, fail with [`ItemError::BatchTotal`]; the items before it keep their own
//! results. So which items fail depends on the keys, not on the order the caller passes the items
//! in. Serial and parallel runs give exactly this; a parallel run abandons the waves it already
//! knows lie past that point. A cancellation or refused allocation stops the whole batch.
//!
//! With a total, the items are first put in key order by the helper's own sort (`crate::sort`),
//! then run in chunks of that order, and their results are put back in item order. Without one,
//! no item's result depends on any other, so the items run in item order and no key is read.
//!
//! Method: the batch rule of chapter 23.10, decided by a serial walk over results in key order,
//! as for phases (`crate::phase`): the first crossing in key order, however the work was split,
//! as rayon's `find_first` returns the first match in input order
//! (<https://github.com/rayon-rs/rayon/blob/ee0a00bdb1ab039e178a215ad5712fb7fa58e58f/src/iter/mod.rs#L1675-L1680>).
//!
//! Cost, for n items: at most n calls of the function, each with a fresh meter, and one read of
//! the cancellation every [`POLL_EVERY`](crate::POLL_EVERY) items of each chunk. Room for a
//! chunk's results is reserved when the chunk starts, since every item has a result, and the n
//! results are gathered in the order the items run; the items past the crossing that never ran
//! get copies of the one failure. On a pool, at most one wave of chunks past the crossing is run.
//! With a total, add the key sort (`crate::sort`: O(n log n) comparisons, each calling `key`
//! twice, and two buffers of n index–reference pairs, the first kept while the items run), and
//! n slots and the n results again to put the results back in item order.
//!
//! Invariants:
//!
//! - An item's own result is computed from the item alone, under a fresh meter capped at
//!   `per_item`; only the batch-total rule looks at other items, and only at the ones before it
//!   in key order.
//! - The result has exactly one entry per item, in item order. A broken count is
//!   [`Interrupted::Internal`], never a short or misplaced result.

use alloc::vec::Vec;
use core::ops::ControlFlow;

use crate::budget::{BatchBudget, Budget, Phase, Usage};
use crate::cancel::Cancel;
use crate::error::{Exceeded, Interrupted, ItemError};
use crate::meter::{Stop, WorkMeter};
use crate::phase::{Env, chunk_starts};
use crate::plan::ChunkPlan;
use crate::pool::{Pool, drive};
use crate::sort::{Entry, key_order};

/// One item's result in a batch, as returned by `map_each`.
pub type ItemResult<U, E> = Result<U, ItemError<E>>;

/// Where and how a batch runs: everything but its items and functions.
pub(crate) struct Batch<'c> {
    /// The caller's pool, or none.
    pub(crate) pool: Option<&'c dyn Pool>,
    /// Each item's budget and the batch's total.
    pub(crate) budget: BatchBudget,
    /// The caller's cancellation.
    pub(crate) cancel: Option<&'c dyn Cancel>,
    /// The phase failures are reported in.
    pub(crate) phase: Phase,
    /// The chunk plan.
    pub(crate) plan: ChunkPlan,
}

/// One item's own result and what it used.
struct ItemOut<U, E> {
    result: ItemResult<U, E>,
    used: Usage,
}

/// One chunk of a batch: its items' own results, or why the batch stopped in it.
struct BatchChunk<U, E> {
    items: Vec<ItemOut<U, E>>,
    interrupted: Option<Interrupted>,
}

/// Run every item of one chunk under its own meter. `items` yields each item with its index.
fn run_chunk<'t, T, U, E, F, I>(env: &Env<'_>, phase: Phase, items: I, f: &F) -> BatchChunk<U, E>
where
    T: 't,
    I: ExactSizeIterator<Item = (usize, &'t T)>,
    F: Fn(usize, &T, &mut WorkMeter<'_>) -> Result<U, E>,
{
    let mut out = BatchChunk {
        items: Vec::new(),
        interrupted: None,
    };
    // Every item of the chunk has a result, so the whole chunk's room is needed.
    if out.items.try_reserve_exact(items.len()).is_err() {
        out.interrupted = Some(Interrupted::OutOfMemory);
        return out;
    }
    for (offset, (index, item)) in items.enumerate() {
        if env.cancelled_at(offset) {
            out.interrupted = Some(Interrupted::Cancelled);
            return out;
        }
        let mut meter = env.meter();
        let result = f(index, item, &mut meter);
        let used = meter.used();
        let result = match meter.stopped() {
            None => result.map_err(ItemError::Failed),
            Some(Stop::OverBudget(kind)) => Err(ItemError::BudgetExceeded(Exceeded {
                budget: env.cap(),
                kind,
                phase,
                used,
            })),
            Some(Stop::Cancelled) => {
                out.interrupted = Some(Interrupted::Cancelled);
                return out;
            }
            Some(Stop::OutOfMemory) => {
                out.interrupted = Some(Interrupted::OutOfMemory);
                return out;
            }
        };
        // Within the room reserved above.
        out.items.push(ItemOut { result, used });
    }
    out
}

/// The batch rule's serial walk over item results, in the order the items run.
struct Walk<U, E> {
    results: Vec<ItemResult<U, E>>,
    total: Option<Budget>,
    phase: Phase,
    running: Usage,
    crossing: Option<Exceeded>,
    interrupted: Option<Interrupted>,
}

impl<U, E> Walk<U, E> {
    /// Take the next chunk; break once the batch has stopped or its total has run out.
    fn absorb(&mut self, chunk: BatchChunk<U, E>) -> ControlFlow<()> {
        if let Some(interrupted) = chunk.interrupted {
            self.interrupted = Some(interrupted);
            return ControlFlow::Break(());
        }
        if self.results.try_reserve(chunk.items.len()).is_err() {
            self.interrupted = Some(Interrupted::OutOfMemory);
            return ControlFlow::Break(());
        }
        for item in chunk.items {
            let result = match self.crossing {
                Some(crossing) => Err(ItemError::BatchTotal(crossing)),
                None => {
                    self.running = self.running.plus(item.used);
                    match self.passed() {
                        Some(crossing) => {
                            self.crossing = Some(crossing);
                            Err(ItemError::BatchTotal(crossing))
                        }
                        None => item.result,
                    }
                }
            };
            self.results.push(result);
        }
        match self.crossing {
            Some(_) => ControlFlow::Break(()),
            None => ControlFlow::Continue(()),
        }
    }

    /// The batch-total failure, if the running sum has passed the total.
    fn passed(&self) -> Option<Exceeded> {
        let total = self.total?;
        let kind = self.running.exceeds(total)?;
        Some(Exceeded {
            budget: total,
            kind,
            phase: self.phase,
            used: self.running,
        })
    }
}

/// Run the chunks `payloads` of a batch of `len` items through `run`, and walk their results in
/// order under `total`. Returns one result per item, in the order the chunks cover the items.
fn run_batch<P, U, E, R>(
    pool: Option<&dyn Pool>,
    payloads: impl Iterator<Item = P>,
    run: &R,
    total: Option<Budget>,
    phase: Phase,
    len: usize,
) -> Result<Vec<ItemResult<U, E>>, Interrupted>
where
    P: Send,
    U: Send,
    E: Send,
    R: Fn(&mut P) -> BatchChunk<U, E> + Sync,
{
    let mut walk = Walk {
        results: Vec::new(),
        total,
        phase,
        running: Usage::ZERO,
        crossing: None,
        interrupted: None,
    };
    if drive(pool, payloads, run, |chunk| walk.absorb(chunk)).is_err() {
        return Err(Interrupted::OutOfMemory);
    }
    if let Some(interrupted) = walk.interrupted {
        return Err(interrupted);
    }
    let mut results = walk.results;
    if let Some(crossing) = walk.crossing {
        // The items past the crossing that were never run fail the same way.
        let rest = len.saturating_sub(results.len());
        if results.try_reserve_exact(rest).is_err() {
            return Err(Interrupted::OutOfMemory);
        }
        results.extend((0..rest).map(|_| Err(ItemError::BatchTotal(crossing))));
    }
    if results.len() != len {
        return Err(Interrupted::Internal("map_each: not one result per item"));
    }
    Ok(results)
}

/// Put results made in key order back in item order: `by_key[j]` is the result of the item
/// `entries[j]` names.
fn in_item_order<T, U, E>(
    by_key: Vec<ItemResult<U, E>>,
    entries: &[Entry<'_, T>],
) -> Result<Vec<ItemResult<U, E>>, Interrupted> {
    let len = entries.len();
    if by_key.len() != len {
        return Err(Interrupted::Internal("map_each: not one result per item"));
    }
    let mut slots: Vec<Option<ItemResult<U, E>>> = Vec::new();
    if slots.try_reserve_exact(len).is_err() {
        return Err(Interrupted::OutOfMemory);
    }
    slots.resize_with(len, || None);
    for (&(index, _), result) in entries.iter().zip(by_key) {
        match slots.get_mut(index) {
            Some(slot) if slot.is_none() => *slot = Some(result),
            _ => {
                return Err(Interrupted::Internal(
                    "map_each: the key order is not a permutation",
                ));
            }
        }
    }
    let mut ordered = Vec::new();
    if ordered.try_reserve_exact(len).is_err() {
        return Err(Interrupted::OutOfMemory);
    }
    for slot in slots {
        match slot {
            Some(result) => ordered.push(result),
            None => return Err(Interrupted::Internal("map_each: an item has no result")),
        }
    }
    Ok(ordered)
}

/// `map_each`: see [`Exec::map_each`](crate::Exec::map_each).
pub(crate) fn map_each<'t, T, U, E, K, Key, F>(
    batch: &Batch<'_>,
    items: &'t [T],
    key: &Key,
    f: &F,
) -> Result<Vec<ItemResult<U, E>>, Interrupted>
where
    T: Sync,
    U: Send,
    E: Send,
    K: Ord,
    Key: Fn(usize, &T) -> K + Sync,
    F: Fn(usize, &T, &mut WorkMeter<'_>) -> Result<U, E> + Sync,
{
    let env = Env::new(batch.budget.per_item, batch.cancel);
    let phase = batch.phase;
    let Some(total) = batch.budget.total else {
        // No total: no item's result depends on another's, so the items run in item order and
        // no key is read.
        let run = |chunk: &mut (usize, &'t [T])| {
            let (start, slice) = *chunk;
            // `start + offset` is an item's index, below `items.len()`.
            let indexed = slice
                .iter()
                .enumerate()
                .map(|(offset, item)| (start + offset, item));
            run_chunk(&env, phase, indexed, f)
        };
        let payloads = chunk_starts(batch.plan, items);
        return run_batch(batch.pool, payloads, &run, None, phase, items.len());
    };
    let entries = key_order(batch.pool, &env, batch.plan, items, key)?;
    let run = |chunk: &mut &[Entry<'t, T>]| run_chunk(&env, phase, chunk.iter().copied(), f);
    let payloads = entries.chunks(batch.plan.size().get());
    let by_key = run_batch(batch.pool, payloads, &run, Some(total), phase, items.len())?;
    in_item_order(by_key, &entries)
}
