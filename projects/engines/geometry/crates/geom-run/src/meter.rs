//! The work meter: counts work units and logical bytes against a cap, and polls the caller's
//! cancellation at fixed intervals (architecture chapter 23.10).
//!
//! An operation makes one meter for its whole budget, with the caller's [`Cancel`]. Each helper
//! call is a phase of that meter: the helper gives every item a meter of its own, capped at what
//! the phase has left, and settles the phase into the operation's meter when it ends (the phase
//! rule, [`crate::exec`]). A helper call made inside an item runs as a phase of the item's meter,
//! so one budget covers both levels of work and a nested failure stops the item that made it.
//!
//! A meter that stops (over its cap, cancelled, or refused a large allocation) stays stopped: it
//! is a latch, and every later charge returns [`Stopped`] at once. The helper reads the meter, not
//! the item's return value, to see whether an item stopped, so an item cannot turn a stop into a
//! result by ignoring it.
//!
//! Method: fuel metering with periodic interruption checks, as Wasmtime separates fuel (counted,
//! deterministic) from epoch interruption (polled at fixed points, not deterministic)
//! (<https://docs.rs/wasmtime/50.0.0/wasmtime/struct.Config.html#method.consume_fuel>), and
//! cooperative cancellation polled at fixed points as in .NET's `CancellationToken`
//! (<https://learn.microsoft.com/en-us/dotnet/standard/threading/cancellation-in-managed-threads>).
//!
//! Cost: a meter is a few machine words and never allocates. Each charge or reservation is one
//! `u128` addition and comparison, plus one call of the caller's [`Cancel::requested`] every
//! [`POLL_EVERY`] charges; [`WorkMeter::poll`] makes that call at once.
//!
//! Invariants:
//!
//! - A meter that has not stopped has used no more than its cap of either kind.
//! - A stop is never undone, and the first stop is the one kept.
//! - Cancellation is read every [`POLL_EVERY`] charges, counted from the meter's creation, and
//!   whenever [`WorkMeter::poll`] is called: fixed points, whatever the timing.

use core::fmt;

use crate::budget::{Budget, BudgetKind, LogicalSize, Phase, Usage};
use crate::cancel::Cancel;
use crate::error::{Exceeded, RunError};

/// How many charges a meter takes between two reads of the caller's cancellation, and how many
/// items the helper runs between two reads. Small enough that an operation charging for its work
/// stops soon after a cancellation (the 1 ms target of chapter 23.10 assumes a charge at least
/// every few microseconds), large enough that a costly [`Cancel`] (one that reads a clock for a
/// deadline) is asked rarely.
pub const POLL_EVERY: usize = 64;

/// Why a meter stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stop {
    /// Its use of this kind passed its cap. Deterministic: the same inputs stop at the same charge
    /// on every machine and thread count.
    OverBudget(BudgetKind),
    /// The caller's cancellation was seen. Not deterministic; never stored.
    Cancelled,
    /// A large allocation was refused. Not deterministic; never stored.
    OutOfMemory,
}

impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Stop::OverBudget(kind) => write!(f, "over its {kind} budget"),
            Stop::Cancelled => f.write_str("cancelled by the caller"),
            Stop::OutOfMemory => f.write_str("a large allocation was refused"),
        }
    }
}

/// Returned by every meter method once the meter has stopped, so `?` ends the item. Only a meter
/// makes one. Use it as the error type of a function with no errors of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stopped {
    reason: Stop,
}

impl Stopped {
    /// Why the meter stopped.
    #[must_use]
    pub const fn reason(self) -> Stop {
        self.reason
    }
}

impl fmt::Display for Stopped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the work meter stopped: {}", self.reason)
    }
}

impl core::error::Error for Stopped {}

/// Counts work units and logical bytes against a cap, and reads the caller's cancellation every
/// [`POLL_EVERY`] charges.
pub struct WorkMeter<'c> {
    cap: Budget,
    used: Usage,
    cancel: Option<&'c dyn Cancel>,
    /// Charges left before the next read of `cancel`.
    until_poll: usize,
    stop: Option<Stop>,
}

impl fmt::Debug for WorkMeter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkMeter")
            .field("cap", &self.cap)
            .field("used", &self.used)
            .field("cancellable", &self.cancel.is_some())
            .field("stop", &self.stop)
            .finish()
    }
}

impl<'c> WorkMeter<'c> {
    /// A meter for an operation with budget `cap`, never cancelled.
    #[must_use]
    pub const fn new(cap: Budget) -> Self {
        Self::child(cap, None)
    }

    /// The same meter, reading `cancel` every [`POLL_EVERY`] charges and when polled.
    #[must_use]
    pub fn with_cancel(self, cancel: &'c dyn Cancel) -> Self {
        Self {
            cancel: Some(cancel),
            ..self
        }
    }

    /// A fresh meter with cap `cap` and the given cancellation: what the helper gives each item.
    pub(crate) const fn child(cap: Budget, cancel: Option<&'c dyn Cancel>) -> Self {
        Self {
            cap,
            used: Usage::ZERO,
            cancel,
            until_poll: POLL_EVERY,
            stop: None,
        }
    }

    /// Charge `work` units.
    ///
    /// # Errors
    ///
    /// [`Stopped`] if the meter had stopped, if this charge takes its work over the cap, or if the
    /// periodic read of the cancellation finds a request.
    pub fn charge(&mut self, work: u64) -> Result<(), Stopped> {
        self.add(BudgetKind::Work, u128::from(work))
    }

    /// Reserve `logical_bytes` bytes of logical memory.
    ///
    /// # Errors
    ///
    /// As [`charge`](Self::charge), for the logical-memory cap.
    pub fn reserve(&mut self, logical_bytes: u64) -> Result<(), Stopped> {
        self.add(BudgetKind::LogicalBytes, u128::from(logical_bytes))
    }

    /// Reserve logical memory for `count` values of kind `K`, at `K`'s fixed logical size. The
    /// product is exact (it is computed in `u128`).
    ///
    /// # Errors
    ///
    /// As [`reserve`](Self::reserve).
    pub fn reserve_for<K: LogicalSize>(&mut self, count: u64) -> Result<(), Stopped> {
        let bytes = u128::from(count).saturating_mul(u128::from(K::LOGICAL_BYTES));
        self.add(BudgetKind::LogicalBytes, bytes)
    }

    /// Read the caller's cancellation now. Long loops that charge rarely call this at fixed
    /// points (chapter 23.10, "Polling").
    ///
    /// # Errors
    ///
    /// [`Stopped`] if the meter had stopped or the caller has asked to stop.
    pub fn poll(&mut self) -> Result<(), Stopped> {
        self.check()?;
        self.until_poll = POLL_EVERY;
        if self.cancel.is_some_and(Cancel::requested) {
            return Err(self.latch(Stop::Cancelled));
        }
        Ok(())
    }

    /// Whether the meter has stopped, without charging or reading the cancellation.
    ///
    /// # Errors
    ///
    /// [`Stopped`] if it has.
    pub fn check(&self) -> Result<(), Stopped> {
        match self.stop {
            Some(reason) => Err(Stopped { reason }),
            None => Ok(()),
        }
    }

    /// Stop the meter because a large allocation was refused (chapter 23.10, "Real memory"):
    /// `buffer.try_reserve(n).map_err(|_| meter.out_of_memory())?`. A meter that had already
    /// stopped keeps its first reason.
    pub fn out_of_memory(&mut self) -> Stopped {
        self.latch(Stop::OutOfMemory)
    }

    /// The budget this meter counts against.
    #[must_use]
    pub const fn cap(&self) -> Budget {
        self.cap
    }

    /// What has been charged and reserved so far.
    #[must_use]
    pub const fn used(&self) -> Usage {
        self.used
    }

    /// What is left of the cap, per kind; zero for a kind that has passed it.
    #[must_use]
    pub const fn left(&self) -> Budget {
        self.used.left_of(self.cap)
    }

    /// Why the meter stopped, or `None` while it runs.
    #[must_use]
    pub const fn stopped(&self) -> Option<Stop> {
        self.stop
    }

    /// The caller's cancellation, if any.
    #[must_use]
    pub fn cancellation(&self) -> Option<&'c dyn Cancel> {
        self.cancel
    }

    /// The error that `stopped`, returned by this meter, means for `phase`. For an operation's
    /// own serial steps: `meter.charge(n).map_err(|stopped| meter.error(stopped, PHASE))?`.
    #[must_use]
    pub const fn error<E>(&self, stopped: Stopped, phase: Phase) -> RunError<E> {
        match stopped.reason {
            Stop::OverBudget(kind) => RunError::BudgetExceeded(self.exceeded(kind, phase)),
            Stop::Cancelled => RunError::Cancelled,
            Stop::OutOfMemory => RunError::OutOfMemory,
        }
    }

    /// The error this meter's stop means for `phase`, or `None` while it runs.
    #[must_use]
    pub fn stop_error<E>(&self, phase: Phase) -> Option<RunError<E>> {
        self.stop
            .map(|reason| self.error(Stopped { reason }, phase))
    }

    /// Add `amount` of `kind`, stop if it passes the cap, and read the cancellation every
    /// [`POLL_EVERY`] calls.
    fn add(&mut self, kind: BudgetKind, amount: u128) -> Result<(), Stopped> {
        self.check()?;
        self.used = self
            .used
            .with(kind, self.used.get(kind).saturating_add(amount));
        if self.used.get(kind) > u128::from(self.cap.limit(kind)) {
            return Err(self.latch(Stop::OverBudget(kind)));
        }
        self.until_poll = self.until_poll.saturating_sub(1);
        if self.until_poll == 0 {
            return self.poll();
        }
        Ok(())
    }

    /// Stop the meter for `stop`, unless it had already stopped; returns the reason kept.
    pub(crate) fn latch(&mut self, stop: Stop) -> Stopped {
        let reason = *self.stop.get_or_insert(stop);
        Stopped { reason }
    }

    /// Add a settled phase's total to what this meter has used. Stops the meter if the total takes
    /// it over its cap (work first), and returns that kind.
    pub(crate) fn absorb(&mut self, total: Usage) -> Option<BudgetKind> {
        self.used = self.used.plus(total);
        let over = self.used.exceeds(self.cap);
        if let Some(kind) = over {
            self.latch(Stop::OverBudget(kind));
        }
        over
    }

    /// The budget failure of this meter in `kind`, reported for `phase`.
    pub(crate) const fn exceeded(&self, kind: BudgetKind, phase: Phase) -> Exceeded {
        Exceeded {
            budget: self.cap,
            kind,
            phase,
            used: self.used,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cancel::CancelToken;

    #[test]
    fn a_settled_total_over_the_cap_stops_the_meter() {
        let mut meter = WorkMeter::new(Budget::new(10, 10));
        assert_eq!(meter.absorb(Usage::new(10, 10)), None);
        assert_eq!(meter.stopped(), None);
        assert_eq!(
            meter.absorb(Usage::new(0, 1)),
            Some(BudgetKind::LogicalBytes)
        );
        assert_eq!(
            meter.stopped(),
            Some(Stop::OverBudget(BudgetKind::LogicalBytes))
        );
        // The first stop is kept.
        let kept = meter.latch(Stop::Cancelled).reason();
        assert_eq!(kept, Stop::OverBudget(BudgetKind::LogicalBytes));
    }

    #[test]
    fn polling_restarts_the_countdown() {
        let token = CancelToken::new();
        let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
        for _ in 0..POLL_EVERY - 1 {
            assert_eq!(meter.charge(0), Ok(()));
        }
        assert_eq!(meter.poll(), Ok(()));
        token.cancel();
        // The countdown restarted at the poll, so the next read is POLL_EVERY charges later.
        for _ in 0..POLL_EVERY - 1 {
            assert_eq!(meter.charge(0), Ok(()));
        }
        assert_eq!(
            meter.charge(0).map_err(Stopped::reason),
            Err(Stop::Cancelled)
        );
    }
}
