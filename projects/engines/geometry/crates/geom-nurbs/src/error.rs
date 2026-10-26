//! The one error type of the NURBS algorithms: every way an input can be malformed, a parameter
//! can fall outside a domain, a result can fail to be finite, or a budget can stop an operation.
//!
//! What it does: [`NurbsError`] names the failure and, where one exists, the index of the knot,
//! control point or weight at fault, so a caller can report it or repair the input (chapter 22's
//! errors that say what to do). A construction checks the rules of chapter 24.7 ("NURBS well
//! formed") and returns the first rule broken, in the order the constructor's documentation gives.
//!
//! Guarantees:
//!
//! - **Typed, never silent.** Each of `NLib`'s silent fallbacks (a parameter clamped into the
//!   domain, a degenerate span stepped over, a best-so-far result returned beside an error flag)
//!   is a variant here instead (chapter 29.7).
//! - **Comparable and stable.** Every variant holds integers only, so errors compare with `==`
//!   and print the same on every target.
//!
//! Method: none; an error enum and its `Display` and `Error` implementations.
//!
//! Cost: a few machine words; nothing allocates.

use core::fmt;

use geom_num::NumError;
use geom_run::Stopped;

/// Why a NURBS algorithm returned no result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NurbsError {
    /// The degree is 0 or above [`MAX_DEGREE`](crate::MAX_DEGREE).
    Degree {
        /// The degree asked for.
        degree: usize,
    },
    /// The knot vector has too few knots for the degree: a basis of degree p needs at least
    /// 2p + 2 knots (p + 1 functions).
    KnotCount {
        /// How many knots were given.
        knots: usize,
        /// The fewest the degree allows.
        needed: usize,
    },
    /// A knot is NaN or infinite.
    KnotNotFinite {
        /// The knot's index.
        index: usize,
    },
    /// A knot is smaller than the one before it.
    KnotDecreasing {
        /// The index of the smaller knot.
        index: usize,
    },
    /// A knot value is repeated more than degree + 1 times.
    Multiplicity {
        /// The index of the first knot of the run.
        index: usize,
        /// How many times the value is repeated.
        multiplicity: usize,
    },
    /// The parameter domain [`u_p`, `u_(m−p)`] is a single value.
    EmptyDomain,
    /// The knots span more than the largest finite double, so a difference of two knots would
    /// overflow.
    KnotRange,
    /// The number of control points (or of rows or columns of a net) does not match the basis.
    PointCount {
        /// How many were given.
        points: usize,
        /// How many the basis has functions.
        expected: usize,
    },
    /// A control point coordinate is NaN or infinite, or a weighted coordinate would overflow.
    PointNotFinite {
        /// The control point's index (row-major for a surface's net).
        index: usize,
    },
    /// A weight is not a positive finite number.
    Weight {
        /// The weight's index (row-major for a surface's net).
        index: usize,
    },
    /// A parameter is NaN, infinite or outside the domain [`u_p`, `u_(m−p)`].
    ParameterOutside,
    /// A basis function index is beyond the last function.
    Index {
        /// The index asked for.
        index: usize,
        /// How many functions the basis has.
        count: usize,
    },
    /// An output buffer's length does not match what the call writes.
    BufferLength {
        /// The length given.
        given: usize,
        /// The length needed.
        needed: usize,
    },
    /// A result is not finite: a derivative over a span so short that its value overflows, or a
    /// sum of large coordinates that does.
    NotFinite,
    /// A derivative order above [`MAX_DEGREE`](crate::MAX_DEGREE) was asked for.
    DerivativeOrder {
        /// The order asked for.
        order: usize,
    },
    /// The derivative curve asked for does not exist: a lower derivative jumps at a knot repeated
    /// more than p − k + 1 times, so the k-th derivative there is not a function.
    Discontinuous,
    /// The operation needs a nonrational curve or surface.
    Rational,
    /// A tolerance is negative, NaN or infinite.
    Tolerance,
    /// An approximation (degree reduction) cannot be certified within the tolerance asked for.
    OutsideTolerance,
    /// The operation needs a clamped knot vector (each end knot repeated degree + 1 times), as
    /// Bezier extraction and degree change do.
    NotClamped,
    /// A knot to insert, refine with or remove is outside the domain, or a refinement's new knots
    /// are not in non-decreasing order.
    KnotOutside,
    /// Inserting the knot would raise its multiplicity above the degree (or above degree + 1 at an
    /// end of the domain).
    TooManyKnots,
    /// The work meter stopped the operation: over its budget, cancelled, or a large allocation was
    /// refused (chapter 23.10).
    Stopped(Stopped),
    /// An index computed inside an algorithm fell outside its array. Every index is bounded by the
    /// checked sizes of the inputs, so this is a defect in this crate, never a property of the
    /// input; it is returned rather than a panic (principle 13).
    Defect,
}

impl fmt::Display for NurbsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NurbsError::Degree { degree } => {
                write!(f, "degree {degree} is outside 1 to {}", crate::MAX_DEGREE)
            }
            NurbsError::KnotCount { knots, needed } => {
                write!(f, "{knots} knots, but the degree needs at least {needed}")
            }
            NurbsError::KnotNotFinite { index } => write!(f, "knot {index} is not finite"),
            NurbsError::KnotDecreasing { index } => {
                write!(f, "knot {index} is smaller than the knot before it")
            }
            NurbsError::Multiplicity {
                index,
                multiplicity,
            } => write!(
                f,
                "the knot at {index} is repeated {multiplicity} times, more than degree + 1"
            ),
            NurbsError::EmptyDomain => f.write_str("the parameter domain is a single value"),
            NurbsError::KnotRange => {
                f.write_str("the knots span more than the largest finite double")
            }
            NurbsError::PointCount { points, expected } => {
                write!(f, "{points} control points, but the basis has {expected}")
            }
            NurbsError::PointNotFinite { index } => {
                write!(f, "control point {index} is not finite")
            }
            NurbsError::Weight { index } => {
                write!(f, "weight {index} is not a positive finite number")
            }
            NurbsError::ParameterOutside => {
                f.write_str("the parameter is not finite or is outside the domain")
            }
            NurbsError::Index { index, count } => {
                write!(
                    f,
                    "basis function {index} asked for, but the basis has {count}"
                )
            }
            NurbsError::BufferLength { given, needed } => {
                write!(
                    f,
                    "an output buffer of length {given}, but the call writes {needed}"
                )
            }
            NurbsError::NotFinite => f.write_str("a result is not finite"),
            NurbsError::DerivativeOrder { order } => {
                write!(f, "derivative order {order} is above {}", crate::MAX_DEGREE)
            }
            NurbsError::Discontinuous => f.write_str(
                "a lower derivative jumps at a knot, so the derivative curve does not exist",
            ),
            NurbsError::Rational => {
                f.write_str("the operation needs a nonrational curve or surface")
            }
            NurbsError::Tolerance => f.write_str("the tolerance is negative or not finite"),
            NurbsError::OutsideTolerance => {
                f.write_str("the approximation cannot be certified within the tolerance")
            }
            NurbsError::NotClamped => f.write_str("the operation needs a clamped knot vector"),
            NurbsError::KnotOutside => {
                f.write_str("a knot is outside the domain, or the knots to insert are not in order")
            }
            NurbsError::TooManyKnots => {
                f.write_str("the knot's multiplicity would exceed what the degree allows")
            }
            NurbsError::Stopped(stopped) => write!(f, "{stopped}"),
            NurbsError::Defect => {
                f.write_str("an internal index fell outside its array (a defect in geom-nurbs)")
            }
        }
    }
}

impl core::error::Error for NurbsError {}

impl From<Stopped> for NurbsError {
    fn from(stopped: Stopped) -> Self {
        NurbsError::Stopped(stopped)
    }
}

/// A coordinate that is not finite, from the point types of `geom-math`, is a result that is not
/// finite: the NURBS algorithms only build points from values they have computed.
impl From<NumError> for NurbsError {
    fn from(_: NumError) -> Self {
        NurbsError::NotFinite
    }
}
