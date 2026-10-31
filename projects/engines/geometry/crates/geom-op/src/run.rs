//! Running an operation inside its context's budgets: the charge schedule, fallible reservations,
//! and a run that publishes nothing once stopped.
//!
//! What it does: every operation runs inside the budgets its [`OpContext`] carries (chapter
//! 23.10). A [`Charge`] is one line of an operation version's charge schedule: in one phase,
//! so many work units per counted thing (an input face, a pair of overlapping boxes, a Newton
//! step), fixed by the operation's version and listed in its schema (chapter 26.2).
//! [`Charge::charge`] charges a count of them exactly as a serial loop charging one at a time
//! would. [`reserve`] reserves room for a number of values: their logical bytes on the meter
//! first, then the real allocation, fallibly. [`OpContext::run`] runs an operation's body with a
//! meter over the context's budget and cancellation, and publishes its result only if the meter
//! never stopped and no cancellation was asked for by the time it returned.
//!
//! Guarantees:
//!
//! - **Counts a serial loop would make.** Charging `count` things at `units` each passes exactly
//!   when charging them one by one would, and on failure the meter has used exactly what that loop
//!   had used up to and including the first thing over the cap: never more, never a saturated
//!   total. So a `BudgetExceeded` is the same on every target, thread count and build, and may be
//!   stored (chapter 23.10).
//! - **Large reservations are fallible.** [`reserve`] charges the logical bytes first, so a budget
//!   failure comes before any allocation and is the same everywhere; a count the target cannot
//!   address, or an allocation the machine refuses, stops the meter with `OutOfMemory`, which is
//!   never stored. Nothing aborts.
//! - **A stopped operation publishes nothing.** [`OpContext::run`] returns the body's result only
//!   when the body succeeded, its meter never stopped, and a last read of the caller's
//!   cancellation finds no request; otherwise it returns the stop as an error at the context's
//!   path. A body that ignored a stop cannot publish a result past it.
//!
//! Method: chapter 23.10's budgets and cancellation, with owner action G71's reading that time
//! reaches an operation only as the caller's cancellation (section 32.15)
//! (`projects/engines/geometry/architecture/23-performance.md`,
//! `projects/engines/geometry/architecture/32-decisions.md`), on `geom-run`'s meter, which the
//! chapter's fuel accounting follows.
//!
//! Cost: [`Charge::charge`] is constant: at most two charges on the meter whatever the count.
//! [`reserve`] is one charge and one allocation. [`OpContext::run`] adds one meter and one read of
//! the cancellation to the body.

use alloc::vec::Vec;

use geom_run::{LogicalSize, Stop, Stopped, WorkMeter};

use crate::context::OpContext;
use crate::detail::{OpErrorDetail, RunDetail};
use crate::error::OpError;

pub use geom_run::Phase;

/// The phase a stop that the body did not report is charged to, in [`OpContext::run`].
pub const RUN: Phase = Phase::new("operation");

/// One line of an operation version's charge schedule (chapter 23.10): in `phase`, `units` work
/// units for each `per`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Charge {
    /// The phase it is charged in.
    pub phase: Phase,
    /// What is counted, in plain words: `input face`, `box pair`, `Newton step`. Only quantities
    /// fixed by the operation's definition and output, never predicate calls, cache probes or
    /// allocations (chapter 23.10).
    pub per: &'static str,
    /// Work units for each one.
    pub units: u64,
}

impl Charge {
    /// `units` work units for each `per`, in `phase`.
    #[must_use]
    pub const fn new(phase: Phase, per: &'static str, units: u64) -> Self {
        Self { phase, per, units }
    }

    /// Charge `count` of this charge's things to `meter`, as a loop charging `units` for each in
    /// turn would.
    ///
    /// # Errors
    ///
    /// [`Stopped`] if the meter had stopped, if the loop would pass the work cap (the meter then
    /// holds what the loop had used up to and including the first thing over it), or if the read
    /// of the cancellation finds a request.
    pub fn charge(&self, meter: &mut WorkMeter<'_>, count: u64) -> Result<(), Stopped> {
        let total = u128::from(count).saturating_mul(u128::from(self.units));
        let left = meter.left().work;
        match u64::try_from(total) {
            Ok(total) if total <= left => meter.charge(total),
            _ => {
                // The loop passes the cap at the first thing that does not fit in what is left:
                // `fit` things fit (fewer than `count`, because the total does not), and the next
                // one passes. `units` is not zero here, since a zero total always fits.
                let fit = left.checked_div(self.units).unwrap_or(0);
                meter.charge(fit.saturating_mul(self.units))?;
                meter.charge(self.units)
            }
        }
    }
}

/// Room for `count` values of kind `T`: their logical bytes reserved on `meter` first (at `T`'s
/// fixed logical size), then a vector with that capacity, allocated fallibly.
///
/// # Errors
///
/// [`Stopped`] if the meter had stopped or the logical bytes pass its cap (deterministic), or with
/// `OutOfMemory` if `count` does not fit this target's address space or the machine refuses the
/// allocation (not deterministic, never stored).
pub fn reserve<T: LogicalSize>(meter: &mut WorkMeter<'_>, count: u64) -> Result<Vec<T>, Stopped> {
    meter.reserve_for::<T>(count)?;
    let Ok(count) = usize::try_from(count) else {
        return Err(meter.out_of_memory());
    };
    let mut values = Vec::new();
    match values.try_reserve_exact(count) {
        Ok(()) => Ok(values),
        Err(_) => Err(meter.out_of_memory()),
    }
}

impl<'c> OpContext<'c> {
    /// A work meter over this context's budget, reading its cancellation.
    #[must_use]
    pub fn meter(&self) -> WorkMeter<'c> {
        let meter = WorkMeter::new(self.budget());
        match self.cancel() {
            Some(cancel) => meter.with_cancel(cancel),
            None => meter,
        }
    }

    /// The error that `stopped`, returned by `meter`, means in `phase`, at this context's path:
    /// `BudgetExceeded` with the meter's record, `Cancelled` or `OutOfMemory`.
    #[must_use]
    pub fn stopped(&self, meter: &WorkMeter<'_>, stopped: Stopped, phase: Phase) -> OpError {
        let detail = OpErrorDetail::from(meter.error::<OpErrorDetail>(stopped, phase));
        OpError::new(self.path().clone(), detail)
    }

    /// Run an operation's `body` with a meter over this context's budget and cancellation, and
    /// publish its result only if the body succeeded, the meter never stopped, and a last read of
    /// the cancellation finds no request.
    ///
    /// # Errors
    ///
    /// The body's own error; or, if the body returned a result after its meter had stopped, that
    /// stop at this context's path, charged to [`RUN`]; or `Cancelled` if the caller asked to stop
    /// before the body returned.
    pub fn run<T>(
        &self,
        body: impl FnOnce(&mut WorkMeter<'c>) -> Result<T, OpError>,
    ) -> Result<T, OpError> {
        let mut meter = self.meter();
        let value = body(&mut meter)?;
        match meter.poll() {
            Ok(()) => Ok(value),
            Err(stopped) => Err(self.stopped(&meter, stopped, RUN)),
        }
    }
}

/// A meter's stop that reached an error without its meter: the error of a function run by the
/// helper without errors of its own (`geom-run`'s `Exec::map`), when it passes up a stop that is
/// not its item's. Cancellation and refused allocations keep their meaning; a budget stop has lost
/// its `{ budget, kind, phase, used }` record, which only [`OpContext::stopped`] can give, so it is
/// a defect of the code that dropped it: `Internal`, never a `BudgetExceeded` without its record.
impl From<Stopped> for OpErrorDetail {
    fn from(stopped: Stopped) -> Self {
        OpErrorDetail::Run(match stopped.reason() {
            Stop::Cancelled => RunDetail::Cancelled,
            Stop::OutOfMemory => RunDetail::OutOfMemory,
            Stop::OverBudget(_) => RunDetail::Internal {
                what: "a budget stop lost its record; report it with OpContext::stopped",
            },
        })
    }
}
