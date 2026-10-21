//! `sort_stable`: the stable sorted order of items by a total key, the same whatever the chunks.
//!
//! The answer is the order of the items by `(key, index)`. Indices are distinct, so for a key whose
//! `Ord` is a total order this order is unique: it is the stable sorted order, and no way of
//! splitting the work can change it (architecture chapter 23.4, "sort_stable (by a total key: one
//! answer whatever the chunks)"). The helper returns the order as item indices and leaves the items
//! untouched, so an arena builder can both reorder its records and remap the indices that point
//! at them.
//!
//! Each chunk is sorted on its own, in parallel; then adjacent sorted runs are merged in rounds,
//! the merges of a round in parallel, until one run is left. A batch call with a total uses the
//! same sort to put its items in key order (`crate::batch`).
//!
//! Method: bottom-up two-way merge sort (Knuth, The Art of Computer Programming, volume 3, 2nd
//! edition, 1998, section 5.2.4, "Sorting by merging", straight two-way merging; the book's page is
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>), first within chunks, then across
//! them. Written here rather than taken from `core`'s sorts so that nothing allocates infallibly
//! and no comparison, even of an `Ord` that breaks its contract, can make the helper panic.
//!
//! Cost, for n items in chunks of s: at most n · (⌈log₂ s⌉ + ⌈log₂ ⌈n / s⌉⌉) comparisons, so
//! O(n log n), each calling `key` twice; two buffers of n (index, reference) pairs and the result
//! of n indices; one read of the cancellation per chunk and per merge.
//!
//! Invariants:
//!
//! - Every merge writes each element of its two runs exactly once into a destination of exactly
//!   their combined length, so the result is always a permutation of the indices.
//! - No recursion: every loop is bounded by the number of items.

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::ops::ControlFlow;

use crate::budget::Phase;
use crate::error::{Interrupted, RunError};
use crate::meter::{Stop, WorkMeter};
use crate::phase::Env;
use crate::plan::ChunkPlan;
use crate::pool::{Pool, drive};

/// An item's index and the item: what is sorted.
pub(crate) type Entry<'t, T> = (usize, &'t T);

/// Merge the sorted runs `from[..width]` and `from[width..]` into `to`, which has the same length
/// as `from`. On equal elements the left run's comes first, so the merge is stable.
fn merge<X, B>(from: &[X], to: &mut [X], width: usize, before: &B)
where
    X: Copy,
    B: Fn(&X, &X) -> bool,
{
    // A pair shorter than `width` is a left run alone.
    let (left, right) = from.split_at_checked(width).unwrap_or((from, &[]));
    let mut left = left.iter().peekable();
    let mut right = right.iter().peekable();
    for slot in to.iter_mut() {
        let take_right = match (left.peek(), right.peek()) {
            (Some(&l), Some(&r)) => before(r, l),
            (None, Some(_)) => true,
            _ => false,
        };
        let next = if take_right {
            right.next()
        } else {
            left.next()
        };
        if let Some(&x) = next {
            *slot = x;
        }
    }
}

/// One round of merges: runs of `width` in `from` merged in pairs into `to`.
fn merge_round<X, B>(from: &[X], to: &mut [X], width: usize, before: &B)
where
    X: Copy,
    B: Fn(&X, &X) -> bool,
{
    let span = width.saturating_mul(2);
    for (pair, out) in from.chunks(span).zip(to.chunks_mut(span)) {
        merge(pair, out, width, before);
    }
}

/// Sort `run` by bottom-up merge sort, using `spare` (of the same length) as the other buffer.
fn merge_sort<X, B>(run: &mut [X], spare: &mut [X], before: &B)
where
    X: Copy,
    B: Fn(&X, &X) -> bool,
{
    let mut width = 1;
    let mut in_run = true;
    while width < run.len() {
        if in_run {
            merge_round(run, spare, width, before);
        } else {
            merge_round(spare, run, width, before);
        }
        in_run = !in_run;
        width = width.saturating_mul(2);
    }
    if !in_run {
        for (to, from) in run.iter_mut().zip(spare.iter()) {
            *to = *from;
        }
    }
}

/// Stop `meter` because the sort was interrupted, and return the error that means.
fn fail<E>(meter: &mut WorkMeter<'_>, phase: Phase, interrupted: Interrupted) -> RunError<E> {
    let stop = match interrupted {
        Interrupted::Cancelled => Stop::Cancelled,
        Interrupted::OutOfMemory => Stop::OutOfMemory,
        // A bug, not a stop of the meter: reported as it is.
        Interrupted::Internal(what) => return RunError::Internal(what),
    };
    let stopped = meter.latch(stop);
    meter.error(stopped, phase)
}

/// Whether a parallel step ran to the end.
fn finished(done: bool, cancelled: &mut bool) -> ControlFlow<()> {
    if done {
        ControlFlow::Continue(())
    } else {
        *cancelled = true;
        ControlFlow::Break(())
    }
}

/// `sort_stable`: see [`Exec::sort_stable`](crate::Exec::sort_stable).
pub(crate) fn sort_stable<T, K, E, F>(
    pool: Option<&dyn Pool>,
    meter: &mut WorkMeter<'_>,
    phase: Phase,
    plan: ChunkPlan,
    items: &[T],
    key: &F,
) -> Result<Vec<usize>, RunError<E>>
where
    T: Sync,
    K: Ord,
    F: Fn(&T) -> K + Sync,
{
    if let Some(error) = meter.stop_error(phase) {
        return Err(error);
    }
    let env = Env::of(meter);
    let by_item = |_: usize, item: &T| key(item);
    let sorted = match key_order(pool, &env, plan, items, &by_item) {
        Ok(sorted) => sorted,
        Err(interrupted) => return Err(fail(meter, phase, interrupted)),
    };
    let mut order: Vec<usize> = Vec::new();
    if order.try_reserve_exact(sorted.len()).is_err() {
        return Err(fail(meter, phase, Interrupted::OutOfMemory));
    }
    order.extend(sorted.iter().map(|&(index, _)| index));
    Ok(order)
}

/// Every item with its index, in the order of `(key(index, item), index)`: the stable sorted
/// order by `key`, sorted chunk by chunk on `pool` and then merged in rounds. Reads `env`'s
/// cancellation once per chunk and per merge.
pub(crate) fn key_order<'t, T, K, F>(
    pool: Option<&dyn Pool>,
    env: &Env<'_>,
    plan: ChunkPlan,
    items: &'t [T],
    key: &F,
) -> Result<Vec<Entry<'t, T>>, Interrupted>
where
    T: Sync,
    K: Ord,
    F: Fn(usize, &T) -> K + Sync,
{
    let size = plan.size().get();
    let len = items.len();

    let mut sorted: Vec<Entry<'t, T>> = Vec::new();
    let mut spare: Vec<Entry<'t, T>> = Vec::new();
    if sorted.try_reserve_exact(len).is_err() || spare.try_reserve_exact(len).is_err() {
        return Err(Interrupted::OutOfMemory);
    }
    sorted.extend(items.iter().enumerate());
    spare.extend_from_slice(&sorted);
    let before = |a: &Entry<'t, T>, b: &Entry<'t, T>| {
        key(a.0, a.1).cmp(&key(b.0, b.1)).then(a.0.cmp(&b.0)) == Ordering::Less
    };
    let mut cancelled = false;

    // Sort each chunk.
    let sort_chunk = |chunk: &mut (&mut [Entry<'t, T>], &mut [Entry<'t, T>])| {
        if env.cancelled() {
            return false;
        }
        merge_sort(chunk.0, chunk.1, &before);
        true
    };
    let chunks = sorted.chunks_mut(size).zip(spare.chunks_mut(size));
    if drive(pool, chunks, &sort_chunk, |done| {
        finished(done, &mut cancelled)
    })
    .is_err()
    {
        return Err(Interrupted::OutOfMemory);
    }

    // Merge sorted runs of `width` in rounds until one is left.
    let mut width = size;
    while width < len && !cancelled {
        let span = width.saturating_mul(2);
        let merge_pair = |pair: &mut (&[Entry<'t, T>], &mut [Entry<'t, T>])| {
            if env.cancelled() {
                return false;
            }
            merge(pair.0, pair.1, width, &before);
            true
        };
        let pairs = sorted.chunks(span).zip(spare.chunks_mut(span));
        if drive(pool, pairs, &merge_pair, |done| {
            finished(done, &mut cancelled)
        })
        .is_err()
        {
            return Err(Interrupted::OutOfMemory);
        }
        core::mem::swap(&mut sorted, &mut spare);
        width = span;
    }
    if cancelled {
        return Err(Interrupted::Cancelled);
    }
    Ok(sorted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn less(a: &u32, b: &u32) -> bool {
        a < b
    }

    #[test]
    fn a_pair_shorter_than_the_width_is_copied_as_a_left_run() {
        let mut to = [0; 3];
        merge(&[1, 5, 9], &mut to, 4, &less);
        assert_eq!(to, [1, 5, 9]);
        merge(&[1, 5, 9], &mut to, 3, &less);
        assert_eq!(to, [1, 5, 9]);
    }

    #[test]
    fn merges_interleave_and_keep_the_left_element_first_on_ties() {
        // Pairs (key, tag): the left run's tag-0 elements come before equal right-run ones.
        let from = [(1, 0), (3, 0), (3, 0), (2, 1), (3, 1), (4, 1)];
        let mut to = [(0, 9); 6];
        merge(&from, &mut to, 3, &|a: &(u8, u8), b: &(u8, u8)| a.0 < b.0);
        assert_eq!(to, [(1, 0), (2, 1), (3, 0), (3, 0), (3, 1), (4, 1)]);
    }

    #[test]
    fn merge_sort_sorts_every_length_whichever_buffer_it_ends_in() {
        for len in 0..40_u32 {
            let mut run: Vec<u32> = (0..len).map(|i| (i * 7 + 3) % 11).collect();
            let mut spare = vec![0; run.len()];
            let mut expected = run.clone();
            expected.sort_unstable();
            merge_sort(&mut run, &mut spare, &less);
            assert_eq!(run, expected, "len {len}");
        }
    }
}
