//! Exclusive prefix sums of counts, and `count_then_fill`: how new entities get their indices.
//!
//! `scan_exclusive` turns counts into offsets: `offsets[i] = counts[0] + … + counts[i − 1]`, with
//! the total. Each chunk sums its counts in parallel, the chunk sums are scanned serially, and
//! each chunk then writes its offsets from its base in parallel. Integer addition is associative,
//! so the offsets do not depend on the chunks or the threads.
//!
//! `count_then_fill` is chapter 23.4's rule for arena indices ("Ordered results"): each item
//! counts what it makes, an exclusive scan turns counts into offsets, and each item fills its own
//! range of the result. Indices are then in input order on any number of threads, with no atomic
//! counter anywhere. The count pass and the fill pass are two phases of the caller's meter, both
//! reported under the caller's phase name.
//!
//! Method: the parallel scan in its blocked form, for fewer processors than elements (sum each
//! block, scan the block sums, then scan within each block from its base), and its use to give
//! each element its output positions, as in Blelloch, "Prefix Sums and Their Applications"
//! (Technical Report CMU-CS-90-190, Carnegie Mellon University, 1990; the 1993 book-chapter
//! version is at <https://www.cs.cmu.edu/~guyb/papers/Ble93.pdf>).
//!
//! Cost: for n counts in c chunks, `scan_exclusive` makes n checked additions to sum the chunks,
//! c to turn the sums into bases and n to write the offsets; it reads the cancellation once per
//! chunk in each pass and allocates the n offsets and the c bases. `count_then_fill` adds a count
//! phase and a fill phase (n calls each, as in `crate::phase`) and room for the n counts. Each
//! item's outputs are made in a buffer of exactly its count, with one `U::default()` per output,
//! then moved onto its chunk's outputs and those onto the result.
//!
//! Invariants:
//!
//! - Offsets are exact. A total past `usize::MAX` is [`RunError::OutOfMemory`]: no result of that
//!   size can exist on this target, and the limit differs between 32-bit and 64-bit targets, so
//!   the failure is never stored.
//! - `count_then_fill` gives each item a slice of exactly the length it counted, so no item can
//!   write outside its own range.

use alloc::vec::Vec;
use core::ops::ControlFlow;

use crate::budget::Phase;
use crate::error::{Interrupted, RunError};
use crate::meter::{Stop, WorkMeter};
use crate::phase::{ChunkOut, End, Env, Values, chunk_starts, judge, run_items, run_phase};
use crate::plan::ChunkPlan;
use crate::pool::{Pool, drive};

/// Where an item's outputs go in the result of
/// [`Exec::count_then_fill`](crate::Exec::count_then_fill).
#[derive(Clone, Copy, Debug)]
pub struct Place<'a> {
    /// The index in the result of this item's first output.
    pub first: usize,
    /// For every item, the index in the result of its first output: the exclusive prefix sums of
    /// the counts, so an item can name the outputs that other items make.
    pub offsets: &'a [usize],
}

/// One chunk's sum of counts, or why it has none.
enum Sum {
    Of(usize),
    TooLarge,
    Cancelled,
}

/// `scan_exclusive`: see [`Exec::scan_exclusive`](crate::Exec::scan_exclusive).
pub(crate) fn scan_exclusive<E>(
    pool: Option<&dyn Pool>,
    meter: &mut WorkMeter<'_>,
    phase: Phase,
    plan: ChunkPlan,
    counts: &[usize],
) -> Result<(Vec<usize>, usize), RunError<E>> {
    if let Some(error) = meter.stop_error(phase) {
        return Err(error);
    }
    let env = Env::of(meter);
    let size = plan.size().get();
    let mut stop = None;

    // Pass 1: each chunk's sum. `bases` then becomes each chunk's base.
    let mut bases: Vec<usize> = Vec::new();
    if bases.try_reserve_exact(plan.count(counts.len())).is_err() {
        return Err(fail(meter, Interrupted::OutOfMemory));
    }
    let add_up = |chunk: &mut &[usize]| {
        if env.cancelled() {
            return Sum::Cancelled;
        }
        chunk
            .iter()
            .try_fold(0_usize, |sum, &count| sum.checked_add(count))
            .map_or(Sum::TooLarge, Sum::Of)
    };
    let absorb = |sum| match sum {
        Sum::Of(sum) => {
            // One push per chunk, within the room reserved above.
            bases.push(sum);
            ControlFlow::Continue(())
        }
        Sum::TooLarge => {
            stop = Some(Interrupted::OutOfMemory);
            ControlFlow::Break(())
        }
        Sum::Cancelled => {
            stop = Some(Interrupted::Cancelled);
            ControlFlow::Break(())
        }
    };
    if drive(pool, counts.chunks(size), &add_up, absorb).is_err() {
        stop = Some(Interrupted::OutOfMemory);
    }
    if let Some(stop) = stop {
        return Err(fail(meter, stop));
    }

    // The serial step: chunk sums to chunk bases.
    let mut total: usize = 0;
    for base in &mut bases {
        let sum = *base;
        *base = total;
        total = match total.checked_add(sum) {
            Some(total) => total,
            None => return Err(fail(meter, Interrupted::OutOfMemory)),
        };
    }

    // Pass 2: each chunk writes its offsets from its base.
    let mut offsets: Vec<usize> = Vec::new();
    if offsets.try_reserve_exact(counts.len()).is_err() {
        return Err(fail(meter, Interrupted::OutOfMemory));
    }
    offsets.resize(counts.len(), 0);
    let write = |chunk: &mut (&mut [usize], &[usize], usize)| {
        if env.cancelled() {
            return false;
        }
        let mut next = chunk.2;
        for (offset, &count) in chunk.0.iter_mut().zip(chunk.1) {
            *offset = next;
            // Never saturates: every partial sum is at most `total`.
            next = next.saturating_add(count);
        }
        true
    };
    let chunks = offsets
        .chunks_mut(size)
        .zip(counts.chunks(size))
        .zip(bases.iter().copied())
        .map(|((offsets, counts), base)| (offsets, counts, base));
    let finished = |written| {
        if written {
            ControlFlow::Continue(())
        } else {
            stop = Some(Interrupted::Cancelled);
            ControlFlow::Break(())
        }
    };
    if drive(pool, chunks, &write, finished).is_err() {
        stop = Some(Interrupted::OutOfMemory);
    }
    if let Some(stop) = stop {
        return Err(fail(meter, stop));
    }
    Ok((offsets, total))
}

/// Stop `meter` because the scan was interrupted, and return the error that means.
fn fail<E>(meter: &mut WorkMeter<'_>, interrupted: Interrupted) -> RunError<E> {
    match interrupted {
        Interrupted::Cancelled => {
            meter.latch(Stop::Cancelled);
        }
        Interrupted::OutOfMemory => {
            meter.latch(Stop::OutOfMemory);
        }
        // A bug, not a stop of the meter; the scan never reports one.
        Interrupted::Internal(_) => {}
    }
    RunError::from(interrupted)
}

/// One chunk of the fill pass: its items, their counts and their first outputs.
struct FillChunk<'a, T> {
    start: usize,
    items: &'a [T],
    counts: &'a [usize],
    firsts: &'a [usize],
}

/// Fill one chunk: each item writes its outputs into a buffer of exactly its count, which is
/// then appended to the chunk's outputs.
fn fill_chunk<T, U, E, F>(
    env: &Env<'_>,
    chunk: &FillChunk<'_, T>,
    offsets: &[usize],
    fill: &F,
) -> ChunkOut<Values<U>, E>
where
    U: Default,
    F: Fn(usize, &T, &mut [U], Place<'_>, &mut WorkMeter<'_>) -> Result<(), E>,
{
    let mut out = ChunkOut::new(Values(Vec::new()));
    let mut buffer: Vec<U> = Vec::new();
    let items = chunk.items.iter().zip(chunk.counts).zip(chunk.firsts);
    for (offset, ((item, &count), &first)) in items.enumerate() {
        if env.cancelled_at(offset) {
            return out.ended(End::Cancelled);
        }
        // An item's index, below the phase's length: this never saturates.
        let index = chunk.start.saturating_add(offset);
        buffer.clear();
        if buffer.try_reserve_exact(count).is_err() {
            return out.ended(End::OutOfMemory);
        }
        buffer.resize_with(count, U::default);
        let mut meter = env.meter();
        let result = fill(
            index,
            item,
            &mut buffer,
            Place { first, offsets },
            &mut meter,
        );
        out.used = out.used.plus(meter.used());
        if let Err(end) = judge(&meter, result) {
            return out.ended(end);
        }
        if out.gathered.0.try_reserve(count).is_err() {
            return out.ended(End::OutOfMemory);
        }
        out.gathered.0.append(&mut buffer);
    }
    out
}

/// `count_then_fill`: see [`Exec::count_then_fill`](crate::Exec::count_then_fill).
pub(crate) fn count_then_fill<T, U, E, C, F>(
    pool: Option<&dyn Pool>,
    meter: &mut WorkMeter<'_>,
    phase: Phase,
    plan: ChunkPlan,
    items: &[T],
    count: &C,
    fill: &F,
) -> Result<(Vec<U>, Vec<usize>), RunError<E>>
where
    T: Sync,
    U: Default + Send,
    E: Send,
    C: Fn(usize, &T, &mut WorkMeter<'_>) -> Result<usize, E> + Sync,
    F: Fn(usize, &T, &mut [U], Place<'_>, &mut WorkMeter<'_>) -> Result<(), E> + Sync,
{
    let counting = |env: &Env<'_>, chunk: &mut (usize, &[T])| {
        run_items::<_, _, _, Values<usize>, _>(env, chunk.0, chunk.1, count)
    };
    let Values(counts) = run_phase(pool, meter, phase, chunk_starts(plan, items), &counting)?;
    if counts.len() != items.len() {
        return Err(RunError::Internal(
            "count_then_fill: not one count per item",
        ));
    }
    let (offsets, total) = scan_exclusive(pool, meter, phase, plan, &counts)?;

    let size = plan.size().get();
    let chunks = chunk_starts(plan, items)
        .zip(counts.chunks(size))
        .zip(offsets.chunks(size))
        .map(|(((start, items), counts), firsts)| FillChunk {
            start,
            items,
            counts,
            firsts,
        });
    let filling =
        |env: &Env<'_>, chunk: &mut FillChunk<'_, T>| fill_chunk(env, chunk, &offsets, fill);
    let Values(values) = run_phase(pool, meter, phase, chunks, &filling)?;
    if values.len() != total {
        return Err(RunError::Internal(
            "count_then_fill: outputs differ from the counts",
        ));
    }
    Ok((values, offsets))
}
