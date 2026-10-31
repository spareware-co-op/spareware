//! What every operation is given besides its input: which operation it is, its budgets, the
//! caller's cancellation and the precision asked for.
//!
//! What it does: every operation has chapter 10.1's shape, `fn operation(ctx: &OpContext, input:
//! &OpInput) -> Result<OpOutput, OpError>`. An [`OpContext`] carries the operation's [`OpPath`]
//! (its permanent ID from the operation log, and the step or batch item it is), its
//! [`Budget`] of work units and logical bytes (chapter 23.10), the caller's [`Cancel`], and its
//! [`Precision`]: final, or a coarser preview the caller asked for (chapter 23.6).
//!
//! Guarantees:
//!
//! - **No clock and no randomness.** An `OpContext` holds a path, a budget, a precision and the
//!   caller's cancellation, and nothing else; none of them reads a clock or draws a random number.
//!   A time limit reaches an operation only as the caller's [`Cancel`] (owner action G71, section
//!   32.15), whose only effect is to stop it, and a stopped operation publishes nothing.
//! - **Final unless asked.** A context is made with [`Precision::Final`]; only
//!   [`OpContext::with_precision`] asks for a preview.
//! - **Shown as its value.** `Debug` prints the path, the budget, the precision and whether a
//!   cancellation is attached, never the cancellation's address or state.
//!
//! Method: chapter 10.1's operation contract and chapter 23.10's budgets and cancellation, as
//! decided in owner action G71 (`projects/engines/geometry/architecture/10-operations.md`,
//! `projects/engines/geometry/architecture/23-performance.md`, section 32.15 of
//! `projects/engines/geometry/architecture/32-decisions.md`).
//!
//! Cost: a context is a path (one vector of labels) and a few machine words; making one and
//! reading it never allocates beyond the path it is given.

use core::fmt;

use geom_num::{GridStep, Steps};
use geom_run::{Budget, Cancel};

use crate::id::{OpId, OpPath};

/// How exactly a result is asked for (chapters 10.1 and 23.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Precision {
    /// The model's own precision: every certified bound within the operation's ceiling. The only
    /// precision whose results are stored or logged.
    Final,
    /// A preview, as while a door is dragged: curved constructions may be fitted to `bound` steps
    /// of `step` instead. A preview carries its true class and bound, is cached under its own key,
    /// and is never stored as final (chapter 23.6; piece G459 builds what it changes).
    Preview {
        /// The coarser bound curved constructions may be fitted to, in steps of `step`.
        bound: Steps,
        /// The grid step `bound` counts.
        step: GridStep,
    },
}

impl Precision {
    /// Whether this is [`Precision::Final`].
    #[must_use]
    pub const fn is_final(self) -> bool {
        matches!(self, Precision::Final)
    }
}

/// What an operation is given besides its input (chapter 10.1).
#[derive(Clone)]
pub struct OpContext<'c> {
    path: OpPath,
    budget: Budget,
    cancel: Option<&'c dyn Cancel>,
    precision: Precision,
}

impl<'c> OpContext<'c> {
    /// The context of the operation or step at `path`, under `budget`, at [`Precision::Final`],
    /// with no cancellation.
    #[must_use]
    pub const fn new(path: OpPath, budget: Budget) -> Self {
        Self {
            path,
            budget,
            cancel: None,
            precision: Precision::Final,
        }
    }

    /// The same context, stopped when `cancel` is requested.
    #[must_use]
    pub fn with_cancel(self, cancel: &'c dyn Cancel) -> Self {
        Self {
            cancel: Some(cancel),
            ..self
        }
    }

    /// The same context at `precision`.
    #[must_use]
    pub fn with_precision(self, precision: Precision) -> Self {
        Self { precision, ..self }
    }

    /// The operation's path: its permanent ID and the step or batch item it is.
    #[must_use]
    pub const fn path(&self) -> &OpPath {
        &self.path
    }

    /// The operation's permanent ID.
    #[must_use]
    pub const fn op(&self) -> OpId {
        self.path.op()
    }

    /// The budget the operation runs inside.
    #[must_use]
    pub const fn budget(&self) -> Budget {
        self.budget
    }

    /// The caller's cancellation, if any.
    #[must_use]
    pub const fn cancel(&self) -> Option<&'c dyn Cancel> {
        self.cancel
    }

    /// The precision asked for.
    #[must_use]
    pub const fn precision(&self) -> Precision {
        self.precision
    }
}

impl fmt::Debug for OpContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpContext")
            .field("path", &self.path)
            .field("budget", &self.budget)
            .field("cancellable", &self.cancel.is_some())
            .field("precision", &self.precision)
            .finish()
    }
}
