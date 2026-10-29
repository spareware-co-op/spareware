//! Why the box tree could not build, change or answer.
//!
//! What it does: [`BvhError`] names each failure of the tree's construction, refit and queries,
//! and maps `geom-run`'s stops and phase errors onto them, so a caller sees one error type.
//!
//! Guarantees:
//!
//! - **Typed, never partial.** Every failure is one of these kinds and returns no tree or answer;
//!   a tree that fails to refit is left exactly as it was before the call.
//! - **Stops keep their reason.** A meter's stop (budget, cancellation, memory) and a phase's
//!   budget failure keep `geom-run`'s own record of it.
//!
//! Method: none; an error enum and its conversions, with `geom-run`'s stops (chapter 23.10).
//!
//! Cost: constant.

use core::fmt;

use geom_run::{Exceeded, RunError, Stop, Stopped};

/// Why the box tree could not build, change or answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BvhError {
    /// The meter stopped: its budget ran out, the caller cancelled, or memory was refused.
    Stopped(Stop),
    /// A build phase went over its budget, by `geom-run`'s phase rule.
    BudgetExceeded(Exceeded),
    /// Two entries, or two updates of one refit, share this key.
    DuplicateKey(u64),
    /// A refit names a key the tree does not hold.
    UnknownKey(u64),
    /// More entries than a tree indexes (`u32::MAX`).
    TooManyEntries,
    /// A box is not two- or three-dimensional.
    Dimension,
    /// A box's lower corner is above its upper corner on some axis.
    Empty,
    /// A coordinate, or a coordinate padded by its bound, lies beyond ±[`crate::LIMIT`].
    OutOfRange,
    /// A padding bound is negative, or a float box's coordinate is not finite.
    BadBound,
    /// An allocation was refused.
    OutOfMemory,
    /// A step the tree's own bounds rule out failed: a defect, reported, never answered.
    Defect,
}

impl fmt::Display for BvhError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BvhError::Stopped(stop) => write!(f, "the box tree stopped: {stop}"),
            BvhError::BudgetExceeded(exceeded) => write!(f, "the box tree: {exceeded}"),
            BvhError::DuplicateKey(key) => write!(f, "two entries share the key {key}"),
            BvhError::UnknownKey(key) => write!(f, "the tree holds no entry with the key {key}"),
            BvhError::TooManyEntries => f.write_str("more entries than a box tree indexes"),
            BvhError::Dimension => f.write_str("a box tree's boxes are 2D or 3D"),
            BvhError::Empty => f.write_str("a box's lower corner is above its upper corner"),
            BvhError::OutOfRange => f.write_str("a box lies beyond the tree's coordinate limit"),
            BvhError::BadBound => {
                f.write_str("a padding bound is negative, or a coordinate is not finite")
            }
            BvhError::OutOfMemory => f.write_str("an allocation was refused"),
            BvhError::Defect => {
                f.write_str("a step the box tree's own bounds rule out failed (a defect)")
            }
        }
    }
}

impl From<Stopped> for BvhError {
    fn from(stopped: Stopped) -> Self {
        BvhError::Stopped(stopped.reason())
    }
}

impl From<RunError<BvhError>> for BvhError {
    fn from(error: RunError<BvhError>) -> Self {
        match error {
            RunError::Failed(e) => e,
            RunError::BudgetExceeded(exceeded) => BvhError::BudgetExceeded(exceeded),
            RunError::Cancelled => BvhError::Stopped(Stop::Cancelled),
            RunError::OutOfMemory => BvhError::OutOfMemory,
            RunError::Internal(_) => BvhError::Defect,
        }
    }
}
