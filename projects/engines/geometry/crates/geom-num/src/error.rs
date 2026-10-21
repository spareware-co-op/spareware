//! Errors from the numbers layer.
//!
//! Each variant says what was wrong in plain words and carries the value that caused it, so an
//! operation higher up can turn it into a structured error with remedies (chapter 22).

use core::fmt;

/// Why a number could not be made or an exact computation could not proceed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumError {
    /// A coordinate is outside the 40-bit range of a frame (more than 2^40 − 1 grid steps from its origin).
    OutOfRange {
        /// The value that did not fit, in grid steps.
        value: i128,
    },
    /// The inputs are degenerate for this construction: collinear points for a plane, or three planes
    /// that do not meet in exactly one point.
    Degenerate,
    /// A floating-point value was NaN or infinite.
    NotFinite,
    /// A conversion between grid steps was asked to go the wrong way (refine to a coarser step, or
    /// coarsen to a finer one).
    WrongDirection,
    /// An interval division by an interval that contains zero.
    DivisorContainsZero,
    /// An interval square root of an interval with negative values.
    NegativeSqrt,
    /// An empty interval (lower bound above upper bound) was asked for.
    EmptyInterval,
}

impl fmt::Display for NumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NumError::OutOfRange { value } => write!(
                f,
                "coordinate {value} grid steps is outside the frame's range of ±(2^40 − 1) steps"
            ),
            NumError::Degenerate => f.write_str("the inputs are degenerate for this construction"),
            NumError::NotFinite => f.write_str("a floating-point value was NaN or infinite"),
            NumError::WrongDirection => {
                f.write_str("grid-step conversion asked to go the wrong way")
            }
            NumError::DivisorContainsZero => {
                f.write_str("interval division by an interval that contains zero")
            }
            NumError::NegativeSqrt => f.write_str("interval square root of negative values"),
            NumError::EmptyInterval => f.write_str("interval lower bound is above its upper bound"),
        }
    }
}
