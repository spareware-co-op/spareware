//! Errors from the numbers layer.
//!
//! Each variant says what was wrong in plain words, and carries the value that caused it where
//! one value does, so an operation higher up can turn it into a structured error with remedies
//! (chapter 22).

use core::fmt;

/// Why a number could not be made or an exact computation could not proceed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumError {
    /// A coordinate is outside the 40-bit range of a frame (more than 2^40 − 1 grid steps from its
    /// origin).
    ///
    /// `value` is the coordinate, exact whenever it lies strictly between `i128::MIN` and
    /// `i128::MAX`. A coordinate beyond `i128` is reported saturated, so `i128::MAX` stands for
    /// 2^127 − 1 or more and `i128::MIN` for −2^127 or less ([`crate::plane::Tpi::to_grid`] is the
    /// one function that saturates), and the message says "at least" and "at most" for them.
    OutOfRange {
        /// The value that did not fit, in grid steps.
        value: i128,
    },
    /// The inputs are degenerate for this construction: collinear points for a plane, or three
    /// planes that do not meet in exactly one point.
    Degenerate,
    /// A floating-point value was NaN or infinite, or a result would be: an overflow, or a pole
    /// such as the logarithm of zero.
    NotFinite,
    /// A conversion between grid steps was asked to go the wrong way (refine to a coarser step, or
    /// coarsen to a finer one).
    WrongDirection,
    /// An interval division by an interval that contains zero.
    DivisorContainsZero,
    /// An interval square root of an interval with negative values in it.
    NegativeSqrt,
    /// An empty interval (lower bound above upper bound) was asked for.
    EmptyInterval,
    /// A maths function was given an argument outside its domain, where it has no real value: the
    /// square root or logarithm of a negative number, the arcsine or arccosine of a number beyond
    /// ±1, or a negative number to a power that is not a whole number.
    OutsideDomain,
    /// An exact computation did not fit the width its proven bit budget gives it. The budgets are
    /// derived for every input the types allow, and checked when the crate compiles, so this is a
    /// defect in geom-num, never a property of the input. It is reported, rather than answered,
    /// because any answer would be a guess.
    BudgetExceeded,
    /// A bound would exceed the tolerance ceiling it was checked against (chapter 6.5: exceeding
    /// the ceiling is an error that rolls back, never silent growth), or would be too large to
    /// count.
    ToleranceExceeded,
    /// An enclosure needs a maths function (sine, cosine, arctangent, exponential, logarithm) whose
    /// error is not yet proven. `libm` does not prove its error bounds, so an interval built from
    /// its results would not be a certificate. Proven versions are build piece G47.
    UnprovenMaths,
}

impl fmt::Display for NumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NumError::OutOfRange { value } => {
                let bound = match *value {
                    i128::MAX => "at least ",
                    i128::MIN => "at most ",
                    _ => "",
                };
                write!(
                    f,
                    "coordinate {bound}{value} grid steps is outside the frame's range of \
                     ±(2^40 − 1) steps"
                )
            }
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
            NumError::OutsideDomain => {
                f.write_str("maths function argument outside the function's domain")
            }
            NumError::BudgetExceeded => f.write_str(
                "an exact computation exceeded its proven bit budget (a defect in geom-num)",
            ),
            NumError::ToleranceExceeded => {
                f.write_str("a bound would exceed its tolerance ceiling")
            }
            NumError::UnprovenMaths => {
                f.write_str("an enclosure needs a maths function whose error is not yet proven")
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::NumError;

    /// A saturated coordinate says so: `i128::MAX` and `i128::MIN` stand for that value or beyond.
    /// A budget failure is not reported as a coordinate.
    #[test]
    fn saturated_coordinates_say_at_least_and_at_most() {
        let text = |value| NumError::OutOfRange { value }.to_string();
        assert!(text(i128::MAX).starts_with("coordinate at least 1701"));
        assert!(text(i128::MIN).starts_with("coordinate at most -1701"));
        assert!(text(1 << 40).starts_with("coordinate 1099511627776 grid steps"));
        let budget = NumError::BudgetExceeded.to_string();
        assert!(budget.contains("defect in geom-num"));
        assert!(!budget.contains("coordinate"));
    }
}
