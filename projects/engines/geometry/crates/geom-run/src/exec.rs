//! The helper: the one way kernel code spreads work over threads (architecture chapter 23.4).
//!
//! An [`Exec`] holds the caller's [`Pool`], or none. Each call names a [`ChunkPlan`] constant
//! chosen at the call site, the items, the [`WorkMeter`] whose budget the call spends, and the
//! [`Phase`] it runs as. Every call returns exactly what a plain serial loop over the items, in
//! index order, returns: the same values, the same lowest-numbered failure and the same budget
//! failure, field for field, with any pool or none, on any number of threads and for any chunk
//! size. The functions passed in see one item at a time with a meter of its own; nothing they
//! return depends on where chunks start and end ("Chunk boundaries never matter").
//!
//! The calls:
//!
//! - [`Exec::map`] and [`Exec::try_map`]: one value per item, in item order.
//! - [`Exec::find_first`]: the first item, in item order, that matches; never "any" match.
//! - [`Exec::filter_ordered`]: the indices of the items kept, in item order.
//! - [`Exec::map_each`]: a batch call, one result per item, under chapter 23.10's batch rule,
//!   with a batch total summed in the order of the caller's item keys.
//! - [`Exec::scan_exclusive`]: exclusive prefix sums of counts.
//! - [`Exec::count_then_fill`]: each item counts its outputs, then fills its own range of the
//!   result; how new entities get arena indices in input order.
//! - [`Exec::sort_stable`]: the stable sorted order by a total key.
//!
//! There is no parallel reduce and no float reduce: a sum of collection values goes through an
//! exact accumulator or an integer helper, never through the thread or chunk order (chapters 21 and
//! 23.4).
//!
//! **The phase rule** (chapter 23.10). Each call is one phase of its meter. With R what the meter
//! has left when the phase starts, every item runs with a meter of its own capped at R. The phase
//! fails with the lowest-numbered failure of any kind: an item's own error, or an item over R.
//! Otherwise, if the items' total exceeds R, it fails with [`RunError::BudgetExceeded`] at the end
//! of the phase. Otherwise R falls by that total. The count reported is what a serial loop would
//! see: items above the failing one count zero. So a phase does at most R work per item, and a
//! budget bounds a phase's total work only once the phase ends: give every phase a meter whose cap
//! fits the work its items may do between them.
//!
//! **Nesting** ("One pool, two levels"). A call that takes a meter (every call but
//! [`map_each`](Exec::map_each), which is the top level of a batch) made inside an item runs as a
//! phase of the item's meter, on the same pool, which must be re-entrant. Such a nested call that
//! goes over budget, is cancelled or runs out of memory stops the item's meter too, so the outer
//! phase reports the item as over R, cancelled or out of memory, whatever error the item then
//! returns.
//!
//! **Panics.** Functions passed in must not panic: kernel code never panics (principle 13), and the
//! helper catches nothing, because `catch_unwind` catches nothing where panics abort, as on
//! WebAssembly. A panic in a function run on the calling thread unwinds through the helper as it
//! would through a loop; one run by a pool reaches the caller as the pool passes it on.
//!
//! Method: the deterministic parallel helper of chapter 23.4, after rayon's indexed parallel
//! iterators, which keep input order when collecting and report the first match in input order
//! (<https://github.com/rayon-rs/rayon/blob/ee0a00bdb1ab039e178a215ad5712fb7fa58e58f/src/iter/mod.rs#L1675-L1680>),
//! but without rayon's unordered `sum`, `reduce` and `fold`, whose grouping is unspecified
//! (evidence note 05, section 7); fixed-size chunks as in the chunk-size form of OpenMP's
//! worksharing-loop `schedule` clause (OpenMP API 5.2, section 11.5.3,
//! <https://www.openmp.org/spec-html/5.2/openmp.html>); and thread-count determinism tested as
//! Rapier tests it
//! (<https://github.com/dimforge/rapier/blob/475caa3ff4646fd9a796b67d14116256f4962bc7/crates/rapier3d/tests/thread_count_determinism.rs#L274-L295>).
//!
//! Cost, for n items in c chunks: [`map`](Exec::map), [`try_map`](Exec::try_map),
//! [`find_first`](Exec::find_first) and [`filter_ordered`](Exec::filter_ordered) make at most n
//! calls of the function, each with a fresh meter of a few words, and read the cancellation once
//! per [`POLL_EVERY`](crate::POLL_EVERY) items of each chunk; they allocate their result as items
//! run and, on a pool, the bookkeeping of one wave (`crate::pool`).
//! [`map_each`](Exec::map_each) costs the same, with one result per item, plus a sort by key
//! when the batch has a total (`crate::batch`).
//! [`scan_exclusive`](Exec::scan_exclusive) makes 2n + c additions;
//! [`count_then_fill`](Exec::count_then_fill) is two phases and a scan (`crate::scan`);
//! [`sort_stable`](Exec::sort_stable) makes O(n log n) comparisons and holds 2n index–reference
//! pairs (`crate::sort`). A call made inside an item costs what it costs at its own level, and is
//! charged to the item's meter.
//!
//! Invariants:
//!
//! - Every answer is decided by a serial walk over chunk results in index order (`crate::phase`;
//!   for a batch's total, in key order, `crate::batch`), so it cannot depend on the pool, the
//!   thread count or the order chunks finished in.
//! - Every allocation the helper makes that grows with the input is fallible, so a refused one is
//!   [`RunError::OutOfMemory`], never an abort; and a phase whose items may stop early reserves
//!   room only as its items run (amortised doubling, so at most about twice what they need), never
//!   for the whole input up front.
//! - No `unsafe`, no panics, no recursion, no lock, no atomic counter.

use alloc::vec::Vec;
use core::fmt;

use crate::batch::{self, Batch, ItemResult};
use crate::budget::{BatchBudget, Phase};
use crate::cancel::Cancel;
use crate::error::{Interrupted, RunError};
use crate::meter::{Stopped, WorkMeter};
use crate::phase::{Env, First, Kept, Values, chunk_starts, run_items, run_phase};
use crate::plan::ChunkPlan;
use crate::pool::Pool;
use crate::scan::{self, Place};
use crate::sort;

/// Where a phase's chunks run: on the caller's [`Pool`], or on the calling thread, in order. Every
/// choice gives the same answer; they differ only in speed.
#[derive(Clone, Copy)]
pub struct Exec<'p> {
    pool: Option<&'p dyn Pool>,
}

impl fmt::Debug for Exec<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Exec")
            .field("pool", &self.pool.is_some())
            .finish()
    }
}

impl<'p> Exec<'p> {
    /// Every chunk on the calling thread, in index order: the reference every pool must match.
    #[must_use]
    pub const fn serial() -> Self {
        Self { pool: None }
    }

    /// Chunks on `pool`'s threads.
    #[must_use]
    pub const fn with_pool(pool: &'p dyn Pool) -> Self {
        Self { pool: Some(pool) }
    }

    /// The pool, if any.
    #[must_use]
    pub fn pool(&self) -> Option<&'p dyn Pool> {
        self.pool
    }

    /// `f(i, &items[i], meter)` for every item, in item order, as one phase of `meter`. For
    /// functions with no errors of their own: they stop only through their meter's `?`.
    ///
    /// # Errors
    ///
    /// As [`try_map`](Self::try_map).
    pub fn map<T, U, F>(
        &self,
        meter: &mut WorkMeter<'_>,
        phase: Phase,
        plan: ChunkPlan,
        items: &[T],
        f: F,
    ) -> Result<Vec<U>, RunError>
    where
        T: Sync,
        U: Send,
        F: Fn(usize, &T, &mut WorkMeter<'_>) -> Result<U, Stopped> + Sync,
    {
        self.try_map(meter, phase, plan, items, f)
    }

    /// `f(i, &items[i], meter)` for every item, in item order, as one phase of `meter`.
    ///
    /// # Errors
    ///
    /// By the phase rule (module documentation): [`RunError::Failed`] with the error of the
    /// lowest-numbered item that failed, or [`RunError::BudgetExceeded`] for the lowest-numbered
    /// item over R, whichever comes first; otherwise [`RunError::BudgetExceeded`] if the items'
    /// total exceeds R. [`RunError::Cancelled`] or [`RunError::OutOfMemory`] if the caller
    /// cancelled or an allocation was refused.
    pub fn try_map<T, U, E, F>(
        &self,
        meter: &mut WorkMeter<'_>,
        phase: Phase,
        plan: ChunkPlan,
        items: &[T],
        f: F,
    ) -> Result<Vec<U>, RunError<E>>
    where
        T: Sync,
        U: Send,
        E: Send,
        F: Fn(usize, &T, &mut WorkMeter<'_>) -> Result<U, E> + Sync,
    {
        let run = |env: &Env<'_>, chunk: &mut (usize, &[T])| {
            run_items::<_, _, _, Values<U>, _>(env, chunk.0, chunk.1, &f)
        };
        let Values(values) = run_phase(self.pool, meter, phase, chunk_starts(plan, items), &run)?;
        Ok(values)
    }

    /// The index of the first item, in item order, for which `matches` is true, or `None`. Items
    /// above the first match may be abandoned; items below it all run.
    ///
    /// # Errors
    ///
    /// As [`try_map`](Self::try_map), over the items up to the first match: a failure or an item
    /// over R below the match wins over the match.
    pub fn find_first<T, E, F>(
        &self,
        meter: &mut WorkMeter<'_>,
        phase: Phase,
        plan: ChunkPlan,
        items: &[T],
        matches: F,
    ) -> Result<Option<usize>, RunError<E>>
    where
        T: Sync,
        E: Send,
        F: Fn(usize, &T, &mut WorkMeter<'_>) -> Result<bool, E> + Sync,
    {
        let run = |env: &Env<'_>, chunk: &mut (usize, &[T])| {
            run_items::<_, _, _, First, _>(env, chunk.0, chunk.1, &matches)
        };
        let First(found) = run_phase(self.pool, meter, phase, chunk_starts(plan, items), &run)?;
        Ok(found)
    }

    /// The indices of the items for which `keep` is true, in item order.
    ///
    /// # Errors
    ///
    /// As [`try_map`](Self::try_map).
    pub fn filter_ordered<T, E, F>(
        &self,
        meter: &mut WorkMeter<'_>,
        phase: Phase,
        plan: ChunkPlan,
        items: &[T],
        keep: F,
    ) -> Result<Vec<usize>, RunError<E>>
    where
        T: Sync,
        E: Send,
        F: Fn(usize, &T, &mut WorkMeter<'_>) -> Result<bool, E> + Sync,
    {
        let run = |env: &Env<'_>, chunk: &mut (usize, &[T])| {
            run_items::<_, _, _, Kept, _>(env, chunk.0, chunk.1, &keep)
        };
        let Kept(kept) = run_phase(self.pool, meter, phase, chunk_starts(plan, items), &run)?;
        Ok(kept)
    }

    /// A batch call: `f(i, &items[i], meter)` for every item, each its own operation under
    /// `budget.per_item`, with one result per item, in item order (chapter 23.10, "Batch calls").
    ///
    /// An item's own result never depends on the other items. If `budget.total` is set, what the
    /// items used is summed in the order of their keys, `key(i, &items[i])`: the caller's stable
    /// keys, never their positions (chapter 10.1). The item at which the sum first passes the
    /// total, and every item after it in key order, get
    /// [`ItemError::BatchTotal`](crate::ItemError::BatchTotal) naming the total, and the items
    /// before it keep their own results. So passing the same items in another order only reorders
    /// the results. Items past that point may never run. Keys should be distinct, as chapter 10.1's
    /// keys are; items with equal keys are taken in item order. `key`'s `Ord` must be a total
    /// order (as `Ord` requires). `key` is read only when there is a total, twice per comparison
    /// of the key sort.
    ///
    /// `map_each` is the top level of a batch, which is many operations: it takes no meter, so it
    /// is never a phase of an operation. Called inside an item it would neither charge that item's
    /// meter nor stop it; code inside an item uses the calls that take a meter.
    ///
    /// # Errors
    ///
    /// [`Interrupted`] if the caller cancelled or an allocation was refused: then no item has a
    /// result. [`Interrupted::Internal`] for a broken invariant of this crate.
    #[allow(
        clippy::too_many_arguments,
        reason = "chapter 23.10's batch rule takes all seven: the budget, the caller's \
                  cancellation, the phase, the chunk plan, the items, their keys and the function; \
                  none belongs inside another, and a batch has no meter to carry any of them"
    )]
    pub fn map_each<T, U, E, K, Key, F>(
        &self,
        budget: BatchBudget,
        cancel: Option<&dyn Cancel>,
        phase: Phase,
        plan: ChunkPlan,
        items: &[T],
        key: Key,
        f: F,
    ) -> Result<Vec<ItemResult<U, E>>, Interrupted>
    where
        T: Sync,
        U: Send,
        E: Send,
        K: Ord,
        Key: Fn(usize, &T) -> K + Sync,
        F: Fn(usize, &T, &mut WorkMeter<'_>) -> Result<U, E> + Sync,
    {
        let batch = Batch {
            pool: self.pool,
            budget,
            cancel,
            phase,
            plan,
        };
        batch::map_each(&batch, items, &key, &f)
    }

    /// Exclusive prefix sums: `offsets[i] = counts[0] + … + counts[i − 1]`, and the total of all
    /// the counts. Charges nothing; the caller charges by its own schedule.
    ///
    /// # Errors
    ///
    /// [`RunError::OutOfMemory`] if the total passes `usize::MAX` (no result of that size can
    /// exist on this target) or an allocation was refused; [`RunError::Cancelled`]; the meter's
    /// own error if it had already stopped.
    pub fn scan_exclusive<E>(
        &self,
        meter: &mut WorkMeter<'_>,
        phase: Phase,
        plan: ChunkPlan,
        counts: &[usize],
    ) -> Result<(Vec<usize>, usize), RunError<E>> {
        scan::scan_exclusive(self.pool, meter, phase, plan, counts)
    }

    /// Each item counts its outputs (`count`), an exclusive scan turns the counts into offsets,
    /// and each item fills its own range of the result (`fill`), which starts as `U::default()`
    /// and has exactly the length the item counted. Returns the outputs, in item order, and the
    /// offsets. The count pass and the fill pass are two phases of `meter`, both reported as
    /// `phase`. This is how new entities get arena indices in input order on any number of threads
    /// (chapter 23.4, "Ordered results").
    ///
    /// # Errors
    ///
    /// As [`try_map`](Self::try_map) for each pass, and as
    /// [`scan_exclusive`](Self::scan_exclusive) between them.
    pub fn count_then_fill<T, U, E, C, F>(
        &self,
        meter: &mut WorkMeter<'_>,
        phase: Phase,
        plan: ChunkPlan,
        items: &[T],
        count: C,
        fill: F,
    ) -> Result<(Vec<U>, Vec<usize>), RunError<E>>
    where
        T: Sync,
        U: Default + Send,
        E: Send,
        C: Fn(usize, &T, &mut WorkMeter<'_>) -> Result<usize, E> + Sync,
        F: Fn(usize, &T, &mut [U], Place<'_>, &mut WorkMeter<'_>) -> Result<(), E> + Sync,
    {
        scan::count_then_fill(self.pool, meter, phase, plan, items, &count, &fill)
    }

    /// The stable sorted order of `items` by `key`: `order[j]` is the index of the item at sorted
    /// position `j`, and items with equal keys keep their input order. `key`'s `Ord` must be a
    /// total order (as `Ord` requires); then the order is unique, whatever the chunks and threads.
    /// `key` is called once per comparison, so it should be cheap. Charges nothing; the caller
    /// charges by its own schedule.
    ///
    /// # Errors
    ///
    /// [`RunError::Cancelled`], [`RunError::OutOfMemory`], or the meter's own error if it had
    /// already stopped.
    pub fn sort_stable<T, K, E, F>(
        &self,
        meter: &mut WorkMeter<'_>,
        phase: Phase,
        plan: ChunkPlan,
        items: &[T],
        key: F,
    ) -> Result<Vec<usize>, RunError<E>>
    where
        T: Sync,
        K: Ord,
        F: Fn(&T) -> K + Sync,
    {
        sort::sort_stable(self.pool, meter, phase, plan, items, &key)
    }
}
