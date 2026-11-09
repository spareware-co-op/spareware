//! Why an exchange step refused: the one error type of the crate's units, numbers, frames and
//! reports.
//!
//! What it does: [`ExchangeError`] names each refusal with what a caller needs to act on it: the
//! exact rounded value of a number past the grid's range, the finer step that would hold a length
//! that rounds to zero, or the numbers layer's own error.
//!
//! Guarantees:
//!
//! - **A refusal is typed, never a guess.** Every function of the crate returns one of these
//!   kinds instead of a rounded, clamped or defaulted value.
//! - **Plain data.** `Copy`, compared with `==`, with a `Display` in words.
//!
//! Method: none; the error kinds of chapter 24.8's import and 24.10's export, with chapter 25's
//! rule that an error says what to do next.
//!
//! Cost: a few words; formatting writes one sentence.

use core::fmt;

use geom_math::DecimalError;
use geom_num::NumError;
use geom_num::grid::GridStep;

/// Why an exchange step refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExchangeError {
    /// A length unit outside what a frame's linear unit can hold, 2^−32 to 2^32 metres (`tera`
    /// and above, `pico` and below).
    UnitOutOfRange,
    /// An angle unit with no exact count of angle units: the radian, whose values stay doubles.
    InexactAngleUnit,
    /// A value whose grid count lies past the range it was asked for (±(2^40 − 1) steps for a
    /// length, ±2^60 for a frame origin, ±2^40 angle units): the count, rounded exactly once and
    /// saturated at the ends of `i128`.
    OutOfRange {
        /// The rounded count.
        count: i128,
    },
    /// A length that must be positive (a radius, a thickness, a depth) is zero or negative.
    NotPositive,
    /// A positive length rounds to zero steps: the finer step at which it does not, if one does.
    RoundsToZero {
        /// The first finer grid step whose rounding of the length is not zero.
        finer: Option<GridStep>,
    },
    /// A double that is NaN or infinite.
    NotFinite,
    /// A decimal beyond the largest finite double, which no double holds.
    RealOutOfRange,
    /// Text that is not a decimal number.
    Decimal(DecimalError),
    /// A direction given as two numbers that are both zero.
    ZeroDirection,
    /// A scale factor that is zero or negative, or whose exact ratio needs more than 64 bits in its
    /// numerator or denominator.
    ScaleOutOfRange,
    /// The numbers layer refused (a budget its bounds rule out, reported, never answered).
    Numeric(NumError),
    /// A report could not grow: the allocator refused.
    OutOfMemory,
}

impl fmt::Display for ExchangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExchangeError::UnitOutOfRange => {
                f.write_str("the unit is outside 2^-32 to 2^32 metres, which no frame holds")
            }
            ExchangeError::InexactAngleUnit => {
                f.write_str("the radian is no whole number of angle units: its values stay doubles")
            }
            ExchangeError::OutOfRange { count } => {
                write!(f, "the value rounds to {count}, past the grid's range")
            }
            ExchangeError::NotPositive => f.write_str("a positive length is zero or negative"),
            ExchangeError::RoundsToZero { finer: Some(step) } => write!(
                f,
                "the length rounds to zero steps; a frame at {step:?} holds it"
            ),
            ExchangeError::RoundsToZero { finer: None } => {
                f.write_str("the length rounds to zero steps even at the finest grid step")
            }
            ExchangeError::NotFinite => f.write_str("the double is NaN or infinite"),
            ExchangeError::RealOutOfRange => {
                f.write_str("the decimal is beyond the largest finite double")
            }
            ExchangeError::Decimal(e) => write!(f, "{e}"),
            ExchangeError::ZeroDirection => f.write_str("the direction (0, 0) points nowhere"),
            ExchangeError::ScaleOutOfRange => f.write_str(
                "the scale factor is not positive or does not fit a ratio of 64-bit integers",
            ),
            ExchangeError::Numeric(e) => write!(f, "{e}"),
            ExchangeError::OutOfMemory => f.write_str("the report could not grow"),
        }
    }
}

impl core::error::Error for ExchangeError {}

impl From<NumError> for ExchangeError {
    fn from(e: NumError) -> ExchangeError {
        match e {
            NumError::OutOfRange { value } => ExchangeError::OutOfRange { count: value },
            other => ExchangeError::Numeric(other),
        }
    }
}

impl From<DecimalError> for ExchangeError {
    fn from(e: DecimalError) -> ExchangeError {
        ExchangeError::Decimal(e)
    }
}
