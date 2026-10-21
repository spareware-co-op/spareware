//! Budgets: limits in work units and logical bytes, what was used, and where it is checked.
//!
//! Every operation runs inside the budgets its context carries (architecture chapters 10.1 and
//! 23.10). A [`Budget`] has two limits that fail the same way on every machine:
//!
//! - **Work units** count only what fast paths cannot change: quantities fixed by an operation's
//!   definition and output (entities in and out, pairs of overlapping integer boxes, iterations
//!   whose iterates decide the output bits), never predicate calls, cache probes, tree nodes
//!   visited or allocations. Filters, caches and threads therefore cannot change a count.
//! - **Logical bytes** count the total an operation reserves, from entity and scratch counts at
//!   fixed sizes per kind ([`LogicalSize`]). They never depend on pointer width, the allocator or
//!   the thread count, and they are a total, not a peak, because a peak depends on how many chunks
//!   run at once.
//!
//! A [`Usage`] is what was used, in `u128` so that no total of `u64` charges can wrap. A [`Phase`]
//! names the step whose budget check failed, for the error and its remedies (chapter 22). A
//! [`BatchBudget`] is the budget of a batch call, whose items are separate operations.
//!
//! Method: deterministic work accounting as chapter 23.10 defines it. It is the split Wasmtime
//! makes between fuel, which stops runaway code deterministically, and epoch interruption, which
//! does not (<https://docs.rs/wasmtime/50.0.0/wasmtime/struct.Config.html#method.consume_fuel>):
//! here the budget is the fuel and the caller's [`Cancel`](crate::Cancel) the interruption.
//!
//! Cost: every value here is a few machine words, and every operation a fixed number of integer
//! additions and comparisons; nothing allocates.
//!
//! Invariants:
//!
//! - Every limit is a whole number, and every comparison with a limit is exact: totals are `u128`
//!   and saturate rather than wrap. A saturated total is below the true sum but still above every
//!   `u64` limit, so each comparison with a limit gives the verdict the true sum would.
//! - Work is checked before logical bytes wherever both pass a limit, so the kind reported is
//!   fixed.

use core::fmt;

/// A limit on what an operation, a phase's items or a batch item may use (architecture chapter
/// 23.10). Fails the same way on every machine, thread count and build mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Budget {
    /// Work units: quantities fixed by an operation's definition and output (chapter 23.10).
    pub work: u64,
    /// Logical bytes: the total of fixed sizes per kind of entity and scratch record that the
    /// operation reserves ([`LogicalSize`]).
    pub logical_bytes: u64,
}

impl Budget {
    /// `u64::MAX` of each. Not infinite: a total above it still fails, so no total can wrap.
    pub const UNLIMITED: Budget = Budget::new(u64::MAX, u64::MAX);

    /// A budget of `work` units and `logical_bytes` bytes.
    #[must_use]
    pub const fn new(work: u64, logical_bytes: u64) -> Self {
        Self {
            work,
            logical_bytes,
        }
    }

    /// A budget of `work` units and `u64::MAX` logical bytes.
    #[must_use]
    pub const fn of_work(work: u64) -> Self {
        Self::new(work, u64::MAX)
    }

    /// The limit of one kind.
    #[must_use]
    pub const fn limit(self, kind: BudgetKind) -> u64 {
        match kind {
            BudgetKind::Work => self.work,
            BudgetKind::LogicalBytes => self.logical_bytes,
        }
    }
}

/// Which of a [`Budget`]'s two limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BudgetKind {
    /// The work-unit limit.
    Work,
    /// The logical-memory limit.
    LogicalBytes,
}

impl BudgetKind {
    /// Both kinds, in the order they are checked: work first.
    pub const ALL: [BudgetKind; 2] = [BudgetKind::Work, BudgetKind::LogicalBytes];
}

impl fmt::Display for BudgetKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            BudgetKind::Work => "work",
            BudgetKind::LogicalBytes => "logical-memory",
        })
    }
}

/// Work units and logical bytes used. Wide enough that no total of `u64` charges wraps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Usage {
    /// Work units used.
    pub work: u128,
    /// Logical bytes reserved.
    pub logical_bytes: u128,
}

impl Usage {
    /// Nothing used.
    pub const ZERO: Usage = Usage::new(0, 0);

    /// `work` units and `logical_bytes` bytes used.
    #[must_use]
    pub const fn new(work: u128, logical_bytes: u128) -> Self {
        Self {
            work,
            logical_bytes,
        }
    }

    /// The amount used of one kind.
    #[must_use]
    pub const fn get(self, kind: BudgetKind) -> u128 {
        match kind {
            BudgetKind::Work => self.work,
            BudgetKind::LogicalBytes => self.logical_bytes,
        }
    }

    /// Both amounts added, saturating at `u128::MAX` (unreachable from fewer than 2^64 charges of
    /// at most `u64::MAX` each, which is every run).
    #[must_use]
    pub const fn plus(self, other: Usage) -> Usage {
        Usage::new(
            self.work.saturating_add(other.work),
            self.logical_bytes.saturating_add(other.logical_bytes),
        )
    }

    /// The first kind, work before logical bytes, in which this usage is over `budget`, if any.
    #[must_use]
    pub const fn exceeds(self, budget: Budget) -> Option<BudgetKind> {
        if self.work > budget.work as u128 {
            Some(BudgetKind::Work)
        } else if self.logical_bytes > budget.logical_bytes as u128 {
            Some(BudgetKind::LogicalBytes)
        } else {
            None
        }
    }

    /// This usage with one kind's amount replaced.
    #[must_use]
    pub(crate) const fn with(self, kind: BudgetKind, amount: u128) -> Usage {
        match kind {
            BudgetKind::Work => Usage::new(amount, self.logical_bytes),
            BudgetKind::LogicalBytes => Usage::new(self.work, amount),
        }
    }

    /// What is left of `cap` after this usage, per kind, never below zero.
    #[must_use]
    pub(crate) const fn left_of(self, cap: Budget) -> Budget {
        Budget::new(
            left(cap.work, self.work),
            left(cap.logical_bytes, self.logical_bytes),
        )
    }
}

/// `limit − used`, or 0 when `used` is over `limit`.
const fn left(limit: u64, used: u128) -> u64 {
    let rest = (limit as u128).saturating_sub(used);
    // `rest <= limit <= u64::MAX`, so the cast is exact.
    rest as u64
}

/// The name of one step of an operation whose budget is checked, as its errors report it
/// ("this fillet exceeded its work budget in phase `blend faces`"). A name, not a number, so a
/// stored budget failure reads the same in every build.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Phase {
    name: &'static str,
}

impl Phase {
    /// A phase called `name`, chosen as a constant by the operation:
    /// `const SPLIT: Phase = Phase::new("split faces");`.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }

    /// The phase's name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

/// The budget of a batch call with one result per item (chapter 23.10, "Batch calls"): each item
/// is its own operation under `per_item`; `total`, if set, caps the sum over the batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BatchBudget {
    /// Each item's own budget, so whether an item passes never depends on the other items.
    pub per_item: Budget,
    /// A cap on the work and logical bytes of all the items together, summed in item order.
    pub total: Option<Budget>,
}

impl BatchBudget {
    /// A batch whose items each get `per_item`, with no cap on the total.
    #[must_use]
    pub const fn new(per_item: Budget) -> Self {
        Self {
            per_item,
            total: None,
        }
    }

    /// The same batch with its total capped at `total`.
    #[must_use]
    pub const fn with_total(self, total: Budget) -> Self {
        Self {
            per_item: self.per_item,
            total: Some(total),
        }
    }
}

/// A kind of entity or scratch record with a fixed logical size, for the logical-memory budget
/// (chapter 23.10, "Logical memory"). Charge with
/// [`WorkMeter::reserve_for`](crate::WorkMeter::reserve_for).
pub trait LogicalSize {
    /// The logical size of one value of this kind, in bytes. Chosen per kind and the same on
    /// every target: never `size_of`, which depends on pointer width and layout. It is part of
    /// the charge schedule, so changing it is a change of operation version (chapter 26.2).
    const LOGICAL_BYTES: u64;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_is_checked_before_logical_bytes() {
        let budget = Budget::new(10, 10);
        assert_eq!(Usage::new(10, 10).exceeds(budget), None);
        assert_eq!(Usage::new(11, 11).exceeds(budget), Some(BudgetKind::Work));
        assert_eq!(
            Usage::new(10, 11).exceeds(budget),
            Some(BudgetKind::LogicalBytes)
        );
        assert_eq!(
            Usage::new(u128::MAX, 0).exceeds(Budget::UNLIMITED),
            Some(BudgetKind::Work)
        );
    }

    #[test]
    fn what_is_left_never_goes_below_zero_and_sums_never_wrap() {
        let cap = Budget::new(10, u64::MAX);
        assert_eq!(Usage::new(4, 0).left_of(cap), Budget::new(6, u64::MAX));
        assert_eq!(Usage::new(40, u128::MAX).left_of(cap), Budget::new(0, 0));
        let huge = Usage::new(u128::MAX, 1);
        assert_eq!(huge.plus(huge), Usage::new(u128::MAX, 2));
        assert_eq!(
            Usage::ZERO.with(BudgetKind::LogicalBytes, 5),
            Usage::new(0, 5)
        );
    }
}
