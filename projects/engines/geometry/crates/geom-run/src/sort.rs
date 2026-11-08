//! `sort_stable`: the stable sorted order of items by a total key, the same whatever the chunks.
//!
//! The answer is the order of the items by `(key, index)`. Indices are distinct, so for a key whose
//! `Ord` is a total order this order is unique: it is the stable sorted order, and no way of
//! splitting the work can change it (architecture chapter 23.4, "`sort_stable` (by a total key:
//! one answer whatever the chunks)"). The helper returns the order as item indices and leaves the
//! items untouched, so an arena builder can both reorder its records and remap the indices that
//! point at them.
//!
//! Each chunk is sorted on its own, in parallel; then adjacent sorted runs are merged in rounds
//! until one run is left. Each round's output is cut into pieces of the chunk size and the pieces
//! are merged in parallel, so the last rounds, with only a pair or two of runs to merge, keep the
//! pool as busy as the first: each piece finds where its pair's merge stands at its start and end
//! by co-ranking, a binary search of the two runs. A batch call with a total uses the same sort to
//! put its items in key order (`crate::batch`).
//!
//! Method: bottom-up two-way merge sort (Knuth, The Art of Computer Programming, volume 3, 2nd
//! edition, 1998, section 5.2.4, "Sorting by merging", straight two-way merging; the book's page is
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>), first within chunks, then across
//! them; each merge split into pieces of equal output by co-ranking (Siebert and Träff, "Perfectly
//! load-balanced, optimal, stable, parallel merge", Parallel Processing Letters 23(1), 2013,
//! <https://doi.org/10.1142/S0129626413500059>). Written here rather than taken from `core`'s sorts so that nothing allocates infallibly
//! and no comparison, even of an `Ord` that breaks its contract, can make the helper panic.
//!
//! Cost: for n items in chunks of s, at most n · (⌈log₂ s⌉ + ⌈log₂ ⌈n / s⌉⌉) comparisons for the
//! merging, and 2 · ⌈n / s⌉ binary searches of at most ⌈log₂ n⌉ + 1 comparisons a round for the
//! co-ranking, so O(n log n), each calling `key` twice (a round is merged a second time, by pairs,
//! only if a broken `Ord` put its pieces out of step); two buffers of n (index, reference) pairs
//! and the result of n indices; one read of the cancellation per chunk and per piece of a round.
//!
//! Invariants:
//!
//! - Every merge writes each element of its two runs exactly once into a destination of exactly
//!   their combined length, so the result is always a permutation of the indices. A piece's two
//!   split points are the co-ranks of its start and end, so neighbouring pieces share theirs and
//!   the pieces of a pair share out its runs exactly whenever each piece's own take is in step
//!   (no less than nothing from each run); a piece that finds its take out of step, which only an
//!   `Ord` that breaks its contract can cause, has the whole round merged again by pairs.
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
    merge_runs(left, right, to, before);
}

/// Merge the sorted runs `left` and `right` into `to`, which is as long as both. On equal
/// elements the left run's comes first, so the merge is stable.
fn merge_runs<X, B>(left: &[X], right: &[X], to: &mut [X], before: &B)
where
    X: Copy,
    B: Fn(&X, &X) -> bool,
{
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

/// Co-ranking: how many of the first `t` elements of the stable merge of the sorted runs `left` and
/// `right` come from `left`, found by a binary search. The merge takes `right`'s next element only
/// when it is before `left`'s, so `left[i]` is among the first `t` when the `right` element it
/// would otherwise follow, `right[t - i - 1]`, is not before it. Always at least
/// `t - right.len()` and at most `left.len()` and `t`.
fn co_rank<X, B>(t: usize, left: &[X], right: &[X], before: &B) -> usize
where
    B: Fn(&X, &X) -> bool,
{
    let mut low = t.saturating_sub(right.len());
    let mut high = t.min(left.len());
    while low < high {
        let mid = low.saturating_add(high.saturating_sub(low) / 2);
        // Inside the search both exist: mid < left.len(), and t - mid - 1 < right.len().
        let after = t.saturating_sub(mid).saturating_sub(1);
        let too_few = left
            .get(mid)
            .zip(right.get(after))
            .is_some_and(|(l, r)| !before(r, l));
        if too_few {
            low = mid.saturating_add(1);
        } else {
            high = mid;
        }
    }
    low
}

/// Merge the piece of the merge of `pair`'s runs (`pair[..width]` and the rest) that starts `done`
/// elements in, into `out`, which is as long as the piece. Its split points are the co-ranks of its
/// start and its end, so a piece and the next share one. Returns `false`, and writes nothing, if
/// its take from one run would be less than nothing, which only a `before` that is not a strict
/// order can make happen.
fn merge_piece<X, B>(pair: &[X], width: usize, done: usize, out: &mut [X], before: &B) -> bool
where
    X: Copy,
    B: Fn(&X, &X) -> bool,
{
    let (left, right) = pair.split_at_checked(width).unwrap_or((pair, &[]));
    let end = done.saturating_add(out.len());
    let (first, last) = (
        co_rank(done, left, right, before),
        co_rank(end, left, right, before),
    );
    // Each co-rank is at most its own count, so both right bounds are counts of `right` taken.
    let taken = left
        .get(first..last)
        .zip(right.get(done.saturating_sub(first)..end.saturating_sub(last)));
    let Some((lefts, rights)) = taken else {
        return false;
    };
    merge_runs(lefts, rights, out, before);
    true
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

    // Merge sorted runs of `width` in rounds until one is left, each round in pieces of `size`.
    let mut width = size;
    while width < len && !cancelled {
        let span = width.saturating_mul(2);
        let from: &[Entry<'t, T>] = &sorted;
        // A piece's pair, and where in it the piece starts: a span is a whole number of pieces,
        // so a piece lies in one pair.
        let merge_in_piece = |piece: &mut (usize, &mut [Entry<'t, T>])| {
            if env.cancelled() {
                return (false, true);
            }
            let begin = piece
                .0
                .saturating_sub(piece.0.checked_rem(span).unwrap_or(0));
            let pair = from
                .get(begin..begin.saturating_add(span).min(len))
                .unwrap_or(&[]);
            let done = piece.0.saturating_sub(begin);
            (true, merge_piece(pair, width, done, piece.1, &before))
        };
        let mut in_step = true;
        let pieces = spare
            .chunks_mut(size)
            .enumerate()
            .map(|(k, out)| (k.saturating_mul(size), out));
        if drive(pool, pieces, &merge_in_piece, |(done, fits)| {
            in_step &= fits;
            finished(done, &mut cancelled)
        })
        .is_err()
        {
            return Err(Interrupted::OutOfMemory);
        }
        if !in_step && !cancelled {
            // Out of step only under a broken `Ord`: the round again, a pair to a task.
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

    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "`merge` and `merge_sort` take their comparison as Fn(&X, &X) -> bool"
    )]
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

    /// Values tagged with their run, compared by value alone: ties show which run went first.
    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "`merge_runs` and `co_rank` take their comparison as Fn(&X, &X) -> bool"
    )]
    fn by_value(a: &(u32, u8), b: &(u32, u8)) -> bool {
        a.0 < b.0
    }

    #[test]
    fn co_ranking_finds_where_the_merge_stands_after_each_element() {
        let left: Vec<(u32, u8)> = [1, 3, 3, 8, 9].iter().map(|&v| (v, 0)).collect();
        let right: Vec<(u32, u8)> = [0, 2, 3, 4, 10, 11].iter().map(|&v| (v, 1)).collect();
        let mut whole = vec![(0, 9); left.len() + right.len()];
        merge_runs(&left, &right, &mut whole, &by_value);
        for t in 0..=whole.len() {
            let i = co_rank(t, &left, &right, &by_value);
            let mut first = vec![(0, 9); t];
            merge_runs(&left[..i], &right[..t - i], &mut first, &by_value);
            assert_eq!(first, whole[..t], "t {t}");
        }
        // One run empty: everything comes from the other.
        assert_eq!(co_rank(3, &left, &[], &by_value), 3);
        assert_eq!(co_rank(3, &[], &right, &by_value), 0);
    }

    #[test]
    fn a_round_in_pieces_is_the_round_in_pairs() {
        // Runs of four, sorted, merged in pairs whole and in pieces of one, two and four.
        let from: Vec<(u32, u8)> = (0..23_u32)
            .map(|i| ((i * 7 + 3) % 11, u8::from(i % 8 >= 4)))
            .collect::<Vec<_>>()
            .chunks(4)
            .flat_map(|run| {
                let mut run = run.to_vec();
                run.sort_by_key(|x| x.0);
                run
            })
            .collect();
        let mut whole = vec![(0, 9); from.len()];
        merge_round(&from, &mut whole, 4, &by_value);
        for size in [1, 2, 4] {
            let mut pieces = vec![(0, 9); from.len()];
            for (k, out) in pieces.chunks_mut(size).enumerate() {
                let start = k * size;
                let begin = start - start % 8;
                let pair = &from[begin..(begin + 8).min(from.len())];
                assert!(merge_piece(pair, 4, start - begin, out, &by_value));
            }
            assert_eq!(pieces, whole, "pieces of {size}");
        }
    }

    #[test]
    fn a_piece_out_of_step_says_so_and_writes_nothing() {
        // An order whose answer depends on how often it was asked: the co-ranks of a piece's start
        // and end can cross, and the piece must refuse rather than take less than nothing.
        let asked = core::cell::Cell::new(0u32);
        let fickle = |_: &u32, _: &u32| {
            asked.set(asked.get() + 1);
            asked.get().is_multiple_of(3)
        };
        let pair: Vec<u32> = (0..16).collect();
        let mut refused = 0;
        for done in 0..16 {
            for len in 1..=16 - done {
                let mut out = vec![99; len];
                if !merge_piece(&pair, 8, done, &mut out, &fickle) {
                    refused += 1;
                    assert!(out.iter().all(|&x| x == 99));
                }
            }
        }
        assert!(refused > 0);
        // With a true order every piece is in step.
        let mut out = [0; 5];
        assert!(merge_piece(&pair, 8, 3, &mut out, &less));
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
