//! Why an intersection could not be computed (12.16).
//!
//! What it does: [`IntersectError`] names each failure of a pair. Every pair of kinds is supported,
//! so there is no "unsupported" error. A flagged region is not one of these: it is part of a
//! complete, honest answer.
//!
//! Guarantees:
//!
//! - **Never a partial answer.** A failure returns no items: every error replaces the whole result.
//! - **A stop keeps its reason.** A meter's stop is [`IntersectError::Cancelled`] or
//!   [`IntersectError::BudgetExceeded`], so a caller can tell a request from a limit.
//!
//! Method: chapter 12.16's error kinds, with `geom-run`'s stops
//! (`projects/engines/geometry/architecture/12-intersections.md`).
//!
//! Cost: constant.

use core::fmt;

use geom_curve::CurveError;
use geom_num::NumError;
use geom_run::{Interrupted, Stop, Stopped};

/// Why an intersection could not be computed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntersectError {
    /// The caller asked to stop.
    Cancelled,
    /// The pair's own budget ran out, or an allocation was refused.
    BudgetExceeded,
    /// The operands do not fit the finer frame's range (12.1).
    FrameRange,
    /// The pair needs a maths function that has no proven error bound yet (12.3).
    UnprovenMaths,
    /// A broken precondition, such as an operand with no domain. The checker should already have
    /// caught it.
    InvalidInput,
    /// A proof did not close, a derived cap was hit, or an invariant failed. A bug, never a success.
    Internal,
}

impl fmt::Display for IntersectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            IntersectError::Cancelled => "the intersection was cancelled",
            IntersectError::BudgetExceeded => "the pair's budget ran out",
            IntersectError::FrameRange => "the operands do not fit the finer frame's range",
            IntersectError::UnprovenMaths => "a maths function has no proven error bound yet",
            IntersectError::InvalidInput => "an operand breaks a precondition",
            IntersectError::Internal => "an invariant of the intersection failed",
        })
    }
}

impl core::error::Error for IntersectError {}

impl From<Stopped> for IntersectError {
    fn from(stopped: Stopped) -> Self {
        match stopped.reason() {
            Stop::Cancelled => IntersectError::Cancelled,
            Stop::OverBudget(_) | Stop::OutOfMemory => IntersectError::BudgetExceeded,
        }
    }
}

impl From<Interrupted> for IntersectError {
    fn from(interrupted: Interrupted) -> Self {
        match interrupted {
            Interrupted::Cancelled => IntersectError::Cancelled,
            Interrupted::OutOfMemory => IntersectError::BudgetExceeded,
            Interrupted::Internal(_) => IntersectError::Internal,
        }
    }
}

impl From<CurveError> for IntersectError {
    fn from(e: CurveError) -> Self {
        match e {
            CurveError::Stopped(stopped) => stopped.into(),
            CurveError::Defect => IntersectError::Internal,
            _ => IntersectError::InvalidInput,
        }
    }
}

impl From<NumError> for IntersectError {
    fn from(e: NumError) -> Self {
        match e {
            NumError::UnprovenMaths => IntersectError::UnprovenMaths,
            NumError::OutOfRange { .. } => IntersectError::FrameRange,
            _ => IntersectError::InvalidInput,
        }
    }
}
