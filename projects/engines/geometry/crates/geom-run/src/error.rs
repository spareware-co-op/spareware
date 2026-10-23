//! Why a phase, an operation or a batch item returned no result.
//!
//! [`RunError::Failed`] and [`RunError::BudgetExceeded`] are deterministic: the same inputs give
//! the same error, field for field, on every machine, thread count and chunk size, because both are
//! decided by walking the items in index order, as a serial loop would (architecture chapters 23.4,
//! "Ordered errors", and 23.10). So a budget failure may be stored as model state. Its shape is
//! chapter 22's `{ budget, phase, used }`, with the kind of limit passed. [`RunError::Cancelled`]
//! and [`RunError::OutOfMemory`] depend on the caller and the machine, so they are never stored,
//! logged or cached, and nothing is published (chapter 23.10). [`RunError::Internal`] is a broken
//! invariant of this crate: a bug, reported instead of a panic or a wrong answer (chapter 22).
//!
//! Method: the lowest-index rule of rayon's `find_first`, which returns the first match in input
//! order however the work was split
//! (<https://github.com/rayon-rs/rayon/blob/ee0a00bdb1ab039e178a215ad5712fb7fa58e58f/src/iter/mod.rs#L1675-L1680>),
//! applied to failures, and chapter 22's error kinds and remedies.
//!
//! Cost: the errors are plain values of a few machine words, plus the function's own error;
//! making one never allocates, and `Display` writes straight to the formatter.

use core::fmt;

use crate::budget::{Budget, BudgetKind, Phase, Usage};
use crate::meter::Stopped;

/// A budget failure: which budget, which of its limits, in which phase, and what had been used
/// (chapter 22, `BudgetExceeded { budget, phase, used }`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Exceeded {
    /// The budget that was passed: the operation's, an item's, or a batch's total.
    pub budget: Budget,
    /// Which of its limits.
    pub kind: BudgetKind,
    /// The phase whose check failed.
    pub phase: Phase,
    /// What had been used when the check failed, counted as a serial loop counts it. For an
    /// operation's phase: everything before the phase, and every item of the phase up to and
    /// including the one that failed, nothing above it. For a batch item over its own budget:
    /// what that item used. For a batch total: what the items used, summed in key order, up to
    /// and including the one at which the sum passed the total. When the helper reports it,
    /// `used.get(kind)` is above `budget.limit(kind)`.
    pub used: Usage,
}

impl fmt::Display for Exceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (phase, kind) = (self.phase, self.kind);
        let (limit, used) = (self.budget.limit(kind), self.used.get(kind));
        write!(
            f,
            "phase `{phase}` passed its {kind} budget of {limit} with {used} used; raise the \
             budget, split the call, or ask for a preview"
        )
    }
}

/// Why a phase or an operation returned no result. `E` is the error type of the function run;
/// for functions with no errors of their own it is [`Stopped`], the error of a meter's `?`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RunError<E = Stopped> {
    /// The function failed on an item: the lowest-numbered failure of any kind, so no item below
    /// it failed or went over its cap, whatever the threads and chunks. The one a serial loop
    /// meets first.
    Failed(E),
    /// A deterministic budget ran out (chapter 23.10's phase rule).
    BudgetExceeded(Exceeded),
    /// The caller's cancellation stopped the operation. Not deterministic; never stored.
    Cancelled,
    /// The machine refused a large allocation, or a result would not fit in this target's memory
    /// at all (a count past `usize::MAX`). Not deterministic across machines; never stored.
    OutOfMemory,
    /// An invariant of this crate broke: a bug. Never a panic and never a wrong answer instead.
    Internal(&'static str),
}

impl<E: fmt::Display> fmt::Display for RunError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Failed(error) => write!(f, "{error}"),
            RunError::BudgetExceeded(exceeded) => write!(f, "{exceeded}"),
            RunError::Cancelled => f.write_str("cancelled by the caller; nothing was published"),
            RunError::OutOfMemory => {
                f.write_str("the machine refused a large allocation; nothing was published")
            }
            RunError::Internal(what) => write!(
                f,
                "internal error in the parallel helper ({what}); nothing was published, please \
                 report it"
            ),
        }
    }
}

impl<E: core::error::Error + 'static> core::error::Error for RunError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            RunError::Failed(error) => Some(error),
            _ => None,
        }
    }
}

impl<E> From<Interrupted> for RunError<E> {
    fn from(interrupted: Interrupted) -> Self {
        match interrupted {
            Interrupted::Cancelled => RunError::Cancelled,
            Interrupted::OutOfMemory => RunError::OutOfMemory,
            Interrupted::Internal(what) => RunError::Internal(what),
        }
    }
}

/// Why a whole batch call returned no results: a stop that is not deterministic, or a broken
/// invariant of this crate. None of them belongs to one item, and none is ever stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Interrupted {
    /// The caller's cancellation stopped the batch.
    Cancelled,
    /// The machine refused a large allocation.
    OutOfMemory,
    /// An invariant of this crate broke: a bug. Never a panic and never a wrong answer instead.
    Internal(&'static str),
}

impl fmt::Display for Interrupted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Interrupted::Cancelled => f.write_str("cancelled by the caller; nothing was published"),
            Interrupted::OutOfMemory => {
                f.write_str("the machine refused a large allocation; nothing was published")
            }
            Interrupted::Internal(what) => write!(
                f,
                "internal error in the parallel helper ({what}); nothing was published, please \
                 report it"
            ),
        }
    }
}

impl core::error::Error for Interrupted {}

/// Why one item of a batch call has no result (chapter 23.10, "Batch calls").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemError<E> {
    /// The item's own function failed. Belongs to the item.
    Failed(E),
    /// The item went over its own `per_item` budget. Belongs to the item, so it may be cached as
    /// the item's result.
    BudgetExceeded(Exceeded),
    /// The batch's total ran out at or before this item, in key order. Belongs to the batch, not
    /// to the item, so it is never cached as the item's result. Every item from the one at which
    /// the running sum passed the total, in key order, carries the same failure.
    BatchTotal(Exceeded),
}

impl<E: fmt::Display> fmt::Display for ItemError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ItemError::Failed(error) => write!(f, "{error}"),
            ItemError::BudgetExceeded(exceeded) => write!(f, "{exceeded}"),
            ItemError::BatchTotal(exceeded) => {
                write!(
                    f,
                    "the batch's total ran out at or before this item: {exceeded}"
                )
            }
        }
    }
}

impl<E: core::error::Error + 'static> core::error::Error for ItemError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            ItemError::Failed(error) => Some(error),
            _ => None,
        }
    }
}
