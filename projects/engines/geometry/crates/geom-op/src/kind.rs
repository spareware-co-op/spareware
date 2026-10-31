//! The base kinds of every error: chapter 22's closed set, each defined once.
//!
//! What it does: [`ErrorKind`] is the shared base of every [`OpError`](crate::OpError). Generic
//! code (the command layer, the MCP tools, AI agents, the test harness) can handle any error by its
//! kind alone; each operation's own detail says the rest (chapter 22). Each kind's doc comment is
//! its definition and what the caller can do, which the map lists as the base of the error
//! catalogue (chapter 26.3).
//!
//! Guarantees:
//!
//! - **Closed.** `ErrorKind` is not `#[non_exhaustive]`, so callers handle every kind; a new
//!   failure is a new detail, never a new kind (chapter 22). [`ErrorKind::ALL`] lists every kind
//!   once, in chapter 22's table order.
//! - **Only two kinds are not replayable.** [`ErrorKind::Cancelled`] and
//!   [`ErrorKind::OutOfMemory`] depend on the caller and the machine, so they are never logged,
//!   stored or cached; every other kind is a deterministic answer (chapters 22 and 23.10).
//! - **Named as chapter 22 names them.** [`ErrorKind::name`] is the kind's name in chapter 22,
//!   for logs, schemas and the map.
//!
//! Method: chapter 22's table of base kinds
//! (`projects/engines/geometry/architecture/22-errors-queries-ai.md`), with chapter 23.10's rule
//! that cancellation and refused allocations are never stored.
//!
//! Cost: constant; nothing allocates.

use core::fmt;

/// The shared base of every error (chapter 22): one of a closed set, each kind defined once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ErrorKind {
    /// An input breaks the operation's preconditions: it fails the checker, is malformed, or
    /// repeats a key. The caller can fix or replace the input.
    InvalidInput,
    /// A valid input is degenerate for this operation: a zero length, angle or radius, coincident
    /// anchors, a folded arc, a singular start. The caller can change the value it names.
    Degenerate,
    /// The input is valid, but no result meets it and the rules the caller set. The caller can
    /// change a value; the remedies give the nearest one that works.
    NoSolution,
    /// More than one result fits, and nothing given chooses between them. The caller can choose,
    /// or add what decides.
    Ambiguous,
    /// A decision the result needs can be neither made exactly nor certified at the data's own
    /// resolution. The caller can move things apart, close the gap explicitly, or take another
    /// route.
    Undecided,
    /// The result would be less exact than the caller accepted: a lower class, a certified bound
    /// past the accepted one, or a commit to the grid that would flip a decision. The caller can
    /// accept explicitly the class and bound it would reach, or make the change that keeps it
    /// exact.
    ExactnessNotAccepted,
    /// For Tolerant data only: an operation would need a larger tolerance than a Tolerant body
    /// carries, or healing would need to close a gap wider than `ε_heal` allows. The caller can
    /// heal first, or raise `ε_heal` up to its cap.
    ToleranceExceeded,
    /// A coordinate would leave ±(2^40 − 1) steps of its frame's origin. The caller can work in a
    /// frame placed at the overlap or on the lattice, split, or ask for a coarser step.
    FrameRangeExceeded,
    /// A deterministic work or logical-memory budget ran out. The caller can raise the budget,
    /// split the call, or ask for a preview.
    BudgetExceeded,
    /// The caller's cancellation token, or a deadline it set, stopped the operation. Not
    /// deterministic, so never logged or cached; there is nothing to do.
    Cancelled,
    /// The machine refused a large allocation. Not deterministic, so never logged or cached; there
    /// is nothing to do.
    OutOfMemory,
    /// No route exists yet for this input. The caller can take a route that exists, or wait for
    /// the build piece that adds one.
    Unsupported,
    /// An invariant broke: a bug, never a panic and never a success. The caller can report it,
    /// with what reproduces it.
    Internal,
}

impl ErrorKind {
    /// Every kind, once, in chapter 22's order.
    pub const ALL: [ErrorKind; 13] = [
        ErrorKind::InvalidInput,
        ErrorKind::Degenerate,
        ErrorKind::NoSolution,
        ErrorKind::Ambiguous,
        ErrorKind::Undecided,
        ErrorKind::ExactnessNotAccepted,
        ErrorKind::ToleranceExceeded,
        ErrorKind::FrameRangeExceeded,
        ErrorKind::BudgetExceeded,
        ErrorKind::Cancelled,
        ErrorKind::OutOfMemory,
        ErrorKind::Unsupported,
        ErrorKind::Internal,
    ];

    /// The kind's name, as chapter 22 writes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            ErrorKind::InvalidInput => "InvalidInput",
            ErrorKind::Degenerate => "Degenerate",
            ErrorKind::NoSolution => "NoSolution",
            ErrorKind::Ambiguous => "Ambiguous",
            ErrorKind::Undecided => "Undecided",
            ErrorKind::ExactnessNotAccepted => "ExactnessNotAccepted",
            ErrorKind::ToleranceExceeded => "ToleranceExceeded",
            ErrorKind::FrameRangeExceeded => "FrameRangeExceeded",
            ErrorKind::BudgetExceeded => "BudgetExceeded",
            ErrorKind::Cancelled => "Cancelled",
            ErrorKind::OutOfMemory => "OutOfMemory",
            ErrorKind::Unsupported => "Unsupported",
            ErrorKind::Internal => "Internal",
        }
    }

    /// Whether an error of this kind is a deterministic answer, the same on every machine, which
    /// may be logged, stored and cached: every kind but [`ErrorKind::Cancelled`] and
    /// [`ErrorKind::OutOfMemory`].
    #[must_use]
    pub const fn is_replayable(self) -> bool {
        !matches!(self, ErrorKind::Cancelled | ErrorKind::OutOfMemory)
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
