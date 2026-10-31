//! The batch driver: one call, many items, one result per item, every item its own operation.
//!
//! What it does: [`Batch`] is the driver every operation schema's batch entry uses (chapters 10.1
//! and 23.6). It takes the batch's [`OpContext`] (its path, cancellation and precision), a
//! [`BatchBudget`], the items and the caller's stable key for each, and runs the operation once per
//! item through `geom-run`'s `map_each` under chapter 23.10's batch rule. Item i runs as its own
//! operation: its [`OpPath`] is the batch's path plus its key ([`OpPath::item`]), its budget is the
//! batch's `per_item`, and its precision and cancellation are the batch's. Each item's outcome is a
//! `Result<U, OpError>`, in the order the items were given. [`Batch::single`] runs one item the same
//! way on its own, which is what a call made one by one gives.
//!
//! Guarantees:
//!
//! - **One result per item, in the items' order.** The batch returns exactly one result per item,
//!   or one error for the whole batch when it could not run: a repeated key
//!   ([`BatchDetail::RepeatedKey`], checked before any item runs), the caller's cancellation, or a
//!   refused allocation.
//! - **One failure fails only itself.** An item's own error, or its own budget running out, is that
//!   item's result alone; the other items keep theirs.
//! - **Batch equals single calls, names included.** An item's path, budget, precision and meter do
//!   not depend on the other items, so with no batch total each item's result is bit for bit the
//!   result [`Batch::single`] gives it: the same value, or the same error at the same path. Any name
//!   an operation derives from its path is therefore the same too.
//! - **Keyed, never positioned.** Paths use the caller's keys, so inserting, removing or reordering
//!   items changes no other item's path or result; only a batch total, summed in key order, can
//!   reach past an item (23.10), and then as [`RunDetail::BatchTotal`], which belongs to the batch
//!   and is never cached as the item's result.
//! - **Repeated keys are refused.** Two items with one key would share a path, and would make the
//!   batch total's cut depend on their order; the driver refuses them as `InvalidInput` before any
//!   item runs. This settles section 31.7's question of which layer rejects them.
//!
//! Method: chapter 10.1's batch-first rule and chapter 23.10's batch rule
//! (`projects/engines/geometry/architecture/10-operations.md`,
//! `projects/engines/geometry/architecture/23-performance.md`), on `geom-run`'s `map_each` (piece
//! G23 and G33), which already holds the batch rule on every thread count.
//!
//! Cost: the repeated-key check sorts the keys once, O(n log n) comparisons of sixteen bytes and
//! one allocation of n entries; each item then costs its own operation plus one child path (one
//! allocation); the results are one vector of n. Nothing recurses.

use alloc::vec::Vec;
use core::fmt;

use geom_run::{BatchBudget, Cancel, ChunkPlan, Exec, ItemError, Phase, WorkMeter};

use crate::context::{OpContext, Precision};
use crate::detail::{OpErrorDetail, RunDetail};
use crate::error::OpError;
use crate::id::OpPath;
use crate::kind::ErrorKind;
use crate::remedy::Remedy;

/// A caller's stable key for one batch item: sixteen bytes, as [`OpPath::item`] takes it.
pub type Key = [u8; 16];

/// Why a batch could not run its items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BatchDetail {
    /// Two items carry this key: they would share one path (chapter 10.1).
    RepeatedKey {
        /// The key, as given.
        key: Key,
    },
}

impl BatchDetail {
    /// The base kind.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            BatchDetail::RepeatedKey { .. } => ErrorKind::InvalidInput,
        }
    }

    /// The remedies: give every item a key of its own.
    #[must_use]
    pub fn remedies(&self) -> Vec<Remedy> {
        match self {
            BatchDetail::RepeatedKey { .. } => alloc::vec![Remedy::ChangeInput {
                input: "keys",
                to: None,
            }],
        }
    }
}

impl fmt::Display for BatchDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BatchDetail::RepeatedKey { key } => {
                f.write_str("two batch items share the key ")?;
                key.iter().try_for_each(|b| write!(f, "{b:02x}"))
            }
        }
    }
}

impl core::error::Error for BatchDetail {}

impl From<BatchDetail> for OpErrorDetail {
    fn from(detail: BatchDetail) -> Self {
        OpErrorDetail::Batch(detail)
    }
}

/// A batch call: where it runs in the operation log, its budget, the caller's cancellation, the
/// precision asked for, the phase its items are reported under and the chunks its pool runs.
#[derive(Clone)]
pub struct Batch<'c> {
    path: OpPath,
    budget: BatchBudget,
    cancel: Option<&'c dyn Cancel>,
    precision: Precision,
    phase: Phase,
    plan: ChunkPlan,
}

impl<'c> Batch<'c> {
    /// The batch at `ctx`'s path, with `ctx`'s cancellation and precision, under `budget`: each item
    /// within `budget.per_item`, and all of them, if `budget.total` is set, within that, summed in
    /// key order. `ctx`'s own budget is not used: a batch is many operations, each with its own.
    /// Items over their budget are reported under `phase`; `plan` sets how many items a pool task
    /// runs, which never changes a result.
    #[must_use]
    pub fn new(ctx: &OpContext<'c>, budget: BatchBudget, phase: Phase, plan: ChunkPlan) -> Self {
        Batch {
            path: ctx.path().clone(),
            budget,
            cancel: ctx.cancel(),
            precision: ctx.precision(),
            phase,
            plan,
        }
    }

    /// The batch's path.
    #[must_use]
    pub const fn path(&self) -> &OpPath {
        &self.path
    }

    /// The context item `key` runs in: the batch's path plus `key`, the batch's `per_item` budget,
    /// precision and cancellation.
    #[must_use]
    pub fn item_context(&self, key: Key) -> OpContext<'c> {
        let ctx = OpContext::new(self.path.item(key), self.budget.per_item)
            .with_precision(self.precision);
        match self.cancel {
            Some(cancel) => ctx.with_cancel(cancel),
            None => ctx,
        }
    }

    /// Runs `op` on every item, each as its own operation, and returns one result per item in the
    /// items' order. `key` gives each item's stable key. `op` is given the item's context, the item
    /// and the item's meter, which counts against `per_item`; it must charge its work there.
    ///
    /// # Errors
    ///
    /// One error for the whole batch, at the batch's path, when no item has a result:
    /// [`BatchDetail::RepeatedKey`] if two items share a key, before any item runs;
    /// [`RunDetail::Cancelled`] if the caller cancelled; [`RunDetail::OutOfMemory`] if an
    /// allocation was refused; [`RunDetail::Internal`] for a broken invariant of the helper.
    pub fn run<T, U, KeyFn, F>(
        &self,
        exec: &Exec<'_>,
        items: &[T],
        key: KeyFn,
        op: F,
    ) -> Result<Vec<Result<U, OpError>>, OpError>
    where
        T: Sync,
        U: Send,
        KeyFn: Fn(&T) -> Key + Sync,
        F: Fn(&OpContext<'c>, &T, &mut WorkMeter<'_>) -> Result<U, OpError> + Sync,
    {
        self.distinct(items, &key)?;
        let results = exec
            .map_each(
                self.budget,
                self.cancel,
                self.phase,
                self.plan,
                items,
                |_, item| key(item),
                |_, item, meter| op(&self.item_context(key(item)), item, meter),
            )
            .map_err(|stop| OpError::new(self.path.clone(), stop))?;
        let mut out = Vec::new();
        out.try_reserve_exact(results.len())
            .map_err(|_| OpError::new(self.path.clone(), RunDetail::OutOfMemory))?;
        for (item, result) in items.iter().zip(results) {
            out.push(result.map_err(|e| self.item_error(key(item), e)));
        }
        Ok(out)
    }

    /// Runs `op` on one item as the batch would, alone: the result a call made one by one gives.
    ///
    /// # Errors
    ///
    /// As [`Batch::run`] for a batch of this one item, whose result this is.
    pub fn single<T, U, F>(&self, key: Key, item: &T, op: F) -> Result<U, OpError>
    where
        T: Sync,
        U: Send,
        F: Fn(&OpContext<'c>, &T, &mut WorkMeter<'_>) -> Result<U, OpError> + Sync,
    {
        let alone = Batch {
            budget: BatchBudget {
                per_item: self.budget.per_item,
                total: None,
            },
            ..self.clone()
        };
        let mut results = alone.run(&Exec::serial(), core::slice::from_ref(item), |_| key, op)?;
        results.pop().unwrap_or_else(|| {
            Err(OpError::new(
                self.path.clone(),
                RunDetail::Internal {
                    what: "a batch of one item returned no result",
                },
            ))
        })
    }

    /// [`BatchDetail::RepeatedKey`] for the least key two items share.
    fn distinct<T, KeyFn: Fn(&T) -> Key>(&self, items: &[T], key: &KeyFn) -> Result<(), OpError> {
        let mut keys = Vec::new();
        keys.try_reserve_exact(items.len())
            .map_err(|_| OpError::new(self.path.clone(), RunDetail::OutOfMemory))?;
        keys.extend(items.iter().map(key));
        keys.sort_unstable();
        match keys.windows(2).find(|pair| pair.first() == pair.last()) {
            Some(pair) => Err(OpError::new(
                self.path.clone(),
                BatchDetail::RepeatedKey {
                    key: pair.first().copied().unwrap_or_default(),
                },
            )),
            None => Ok(()),
        }
    }

    /// An item's failure, at the item's path: its own error as it returned it, or the helper's
    /// budget stop for it.
    fn item_error(&self, key: Key, error: ItemError<OpError>) -> OpError {
        match error {
            ItemError::Failed(e) => e,
            ItemError::BudgetExceeded(exceeded) => {
                OpError::new(self.path.item(key), RunDetail::BudgetExceeded(exceeded))
            }
            ItemError::BatchTotal(exceeded) => {
                OpError::new(self.path.item(key), RunDetail::BatchTotal(exceeded))
            }
        }
    }
}

impl fmt::Debug for Batch<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Batch")
            .field("path", &self.path)
            .field("budget", &self.budget)
            .field("cancel", &self.cancel.is_some())
            .field("precision", &self.precision)
            .field("phase", &self.phase)
            .field("plan", &self.plan)
            .finish()
    }
}
