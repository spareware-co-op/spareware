//! Errors from this crate that the numbers layer has no word for.
//!
//! Most operations here fail only for numeric reasons (a coordinate out of range, a value that is
//! not finite) and return [`NumError`] directly. Three failures are new: a transform with no
//! inverse, two frames that cannot be related by an exact integer shift, and geometry too wide for
//! the frame it would have to live in. [`MathError`] carries those, and wraps a [`NumError`] for
//! everything else, so an operation higher up can turn it into a structured error with remedies
//! (chapter 22, which names `FrameMismatch` and `FrameRangeExceeded`).
//!
//! A defect is reported, never answered. Every integer step of this crate is a checked method, and
//! each that a bit budget or a stated bound proves cannot fail reports
//! [`NumError::BudgetExceeded`] if it ever does, as the numbers layer's own budgets do: an error,
//! never a guessed value, so a defect can never pass for a result. No input reaches it; the tests
//! hold every corpus result to `Ok` where the crate documents that it is, and the golden corpus
//! and the fuzz harnesses fail on any call that reports it.
//!
//! Guarantees:
//!
//! - **Plain values.** Every error is `Copy`, compares with `==` and holds only enums and integers,
//!   so it can be returned from `no_std` code, matched exhaustively and compared in tests.
//! - **Says what went wrong.** `Display` writes one fixed sentence per variant, with the offending
//!   coordinate for `FrameRangeExceeded` and the numbers layer's own sentence for `Num`. (The
//!   numbers layer's sentence for `BudgetExceeded` names `geom-num`; reported by this crate, the
//!   defect is in `geom-math`.)
//!
//! Method: the kernel's error model, chapter 22 of the architecture: each variant says what was
//! wrong in plain words and carries the value that caused it
//! (<https://github.com/spareware-co-op/spareware/blob/piece/G01/projects/engines/geometry/architecture/22-errors-queries-ai.md>).
//!
//! Cost: constructing, copying and comparing an error is constant time; `Display` writes one short
//! sentence and allocates nothing.

use core::fmt;

use geom_num::NumError;

/// Why two frames cannot be related by an exact integer shift (chapter 17.1: frames are never mixed
/// without an exact shift).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FrameMismatch {
    /// The frames are tied to different (horizontal) coordinate reference systems.
    Crs,
    /// The frames' heights refer to different vertical coordinate reference systems (or one has
    /// none): a height in one is not a height in the other.
    VerticalCrs,
    /// The frames are rotated differently.
    Rotation,
    /// The frames have different scale factors.
    Scale,
    /// The frames' origins differ by an amount that is not a whole number of grid steps along the
    /// target frame's axes, so the frames are not on a shared lattice.
    NotOnLattice,
}

/// Why an operation of this crate could not produce a result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathError {
    /// A numeric error from the numbers layer: a value out of range, not finite, degenerate, a
    /// step conversion in the wrong direction, or a step that a bit budget rules out
    /// ([`NumError::BudgetExceeded`], a defect).
    Num(NumError),
    /// The transform's linear part is singular, or so close to singular that its orientation cannot
    /// be certified, so it has no inverse.
    Singular,
    /// Two frames cannot be related by an exact integer shift.
    FrameMismatch(FrameMismatch),
    /// Geometry does not fit the frame it belongs to: a coordinate would lie beyond ±(2^40 − 1)
    /// steps of that frame's origin (chapters 17.1 and 22).
    FrameRangeExceeded {
        /// The coordinate that does not fit, in the frame's grid steps.
        value: i128,
    },
}

/// A step of a bit budget, or of a bound stated where the step is taken: its value, or
/// [`NumError::BudgetExceeded`] if it did not fit, which that budget rules out.
pub(crate) fn budget<T>(step: Option<T>) -> Result<T, NumError> {
    step.ok_or(NumError::BudgetExceeded)
}

impl From<NumError> for MathError {
    fn from(e: NumError) -> Self {
        MathError::Num(e)
    }
}

impl fmt::Display for FrameMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            FrameMismatch::Crs => "the frames use different coordinate reference systems",
            FrameMismatch::VerticalCrs => {
                "the frames' heights use different vertical coordinate reference systems"
            }
            FrameMismatch::Rotation => "the frames are rotated differently",
            FrameMismatch::Scale => "the frames have different scale factors",
            FrameMismatch::NotOnLattice => {
                "the frames' origins do not differ by whole grid steps along the target's axes"
            }
        })
    }
}

impl fmt::Display for MathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MathError::Num(e) => e.fmt(f),
            MathError::Singular => f.write_str("the transform is singular and has no inverse"),
            MathError::FrameMismatch(m) => write!(f, "no exact shift between the frames: {m}"),
            MathError::FrameRangeExceeded { value } => write!(
                f,
                "coordinate {value} grid steps does not fit the frame's range of ±(2^40 − 1) steps"
            ),
        }
    }
}
