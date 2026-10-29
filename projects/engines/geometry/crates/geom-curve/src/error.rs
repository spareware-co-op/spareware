//! The one error type of the curve kinds: what a kind refuses, and why.
//!
//! What it does: [`CurveError`] names the failure of an operation of a curve or surface kind (the
//! surface kinds of `geom-surface` share it, a span there being a box): a parameter or span outside
//! the domain, a transform a 2D curve cannot take, a refusal from the NURBS algorithms underneath,
//! a result that is not finite, or a budget that stopped the work. [`ContractError`] names the law
//! of the kind contract (`crate::contract`) a kind broke, and where.
//!
//! Guarantees:
//!
//! - **Typed, never silent.** No operation clamps a parameter into the domain or returns a
//!   best-so-far result beside an error (chapter 29.7).
//! - **Comparable and stable.** Every variant holds plain values, so errors compare with `==` and
//!   print the same on every target.
//!
//! Method: none; error enums and their `Display` and `Error` implementations.
//!
//! Cost: a few machine words; nothing allocates.

use core::fmt;

use geom_math::MathError;
use geom_num::NumError;
use geom_nurbs::NurbsError;
use geom_run::Stopped;

/// Why a curve operation returned no result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CurveError {
    /// The NURBS algorithms refused.
    Nurbs(NurbsError),
    /// A parameter is NaN, infinite or outside the curve's domain, or a split is at an end.
    ParameterOutside,
    /// A span is not ordered, not finite, or not inside the curve's domain.
    SpanOutside,
    /// A transform that moves z, applied to a curve in the plane.
    NotInPlane,
    /// The curve's dimension is not 2 or 3, where an operation needs one of them.
    Dimension {
        /// The dimension.
        dimension: usize,
    },
    /// A result is not finite.
    NotFinite,
    /// The operation needs a finite domain, and the curve is a ray or an infinite line; or its
    /// result would drift without bound along one (an inexact transform of its direction).
    Unbounded,
    /// The segments of a composite curve do not meet: a segment's end and the next one's start
    /// have certified enclosures that do not overlap.
    Disjoint,
    /// The data or a transform collapses the kind: parallel vectors where independent ones are
    /// needed, a zero length or direction, or a singular transform.
    Degenerate,
    /// The work meter stopped the operation (chapter 23.10).
    Stopped(Stopped),
    /// A procedural kind built on itself deeper than its stated bound (an offset of an offset of
    /// …): every operation recurses once per level, and the bound keeps that within the stack
    /// (chapter 28.8).
    TooDeep {
        /// The depth asked for.
        depth: usize,
    },
    /// A step its own bounds rule out failed: a defect in this crate, reported, never a panic.
    Defect,
}

impl fmt::Display for CurveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CurveError::Nurbs(e) => write!(f, "{e}"),
            CurveError::ParameterOutside => {
                f.write_str("the parameter is not finite, or outside the curve's domain")
            }
            CurveError::SpanOutside => {
                f.write_str("the span is not ordered, not finite, or outside the curve's domain")
            }
            CurveError::NotInPlane => f.write_str("the transform moves z; the curve is planar"),
            CurveError::Dimension { dimension } => {
                write!(
                    f,
                    "the operation needs a curve in 2D or 3D, not {dimension}D"
                )
            }
            CurveError::NotFinite => f.write_str("a result is not finite"),
            CurveError::Unbounded => f.write_str(
                "the curve is a ray or an infinite line, and the operation needs a finite domain",
            ),
            CurveError::Disjoint => {
                f.write_str("a composite curve's segments do not meet end to start")
            }
            CurveError::Degenerate => f.write_str(
                "the curve's data or the transform is degenerate (parallel or zero vectors, or a                  singular transform)",
            ),
            CurveError::Stopped(s) => write!(f, "{s}"),
            CurveError::TooDeep { depth } => {
                write!(f, "a procedural kind nested {depth} deep, past its bound")
            }
            CurveError::Defect => f.write_str("an internal step failed (a defect in geom-curve)"),
        }
    }
}

impl core::error::Error for CurveError {}

impl From<NurbsError> for CurveError {
    fn from(e: NurbsError) -> Self {
        match e {
            NurbsError::Stopped(s) => CurveError::Stopped(s),
            NurbsError::ParameterOutside => CurveError::ParameterOutside,
            other => CurveError::Nurbs(other),
        }
    }
}

impl From<Stopped> for CurveError {
    fn from(s: Stopped) -> Self {
        CurveError::Stopped(s)
    }
}

/// An interval or number that left the finite doubles is a result that is not finite.
impl From<NumError> for CurveError {
    fn from(_: NumError) -> Self {
        CurveError::NotFinite
    }
}

impl From<MathError> for CurveError {
    fn from(e: MathError) -> Self {
        match e {
            MathError::NotInPlane => CurveError::NotInPlane,
            _ => CurveError::NotFinite,
        }
    }
}

/// A law of the kind contract (`crate::contract`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Law {
    /// The point lies in the bound over any span holding its parameter.
    BoundHoldsPoints,
    /// Each derivative lies in the derivative bound over any span holding its parameter.
    DerivativeBoundHoldsDerivatives,
    /// The doubles of `point` and `derivatives` lie in the certified enclosures, within the
    /// kind's evaluation error.
    EvaluationIsEnclosed,
    /// The NURBS conversion lies within its stated error of the curve.
    NurbsWithinItsError,
    /// The two halves of a split are the curve on either side, within their deviations.
    SplitKeepsTheCurve,
    /// The reversed curve at −t is the curve at t, within its deviation.
    ReverseKeepsTheCurve,
    /// The transformed curve is the image of the curve, within its deviation.
    TransformMovesTheCurve,
    /// A surface's normal bound keeps the sign of every coordinate it decides (a positive multiple
    /// of the normal lies in it).
    NormalBoundHoldsDirections,
    /// The same call gives the same bits.
    Deterministic,
    /// A parameter or span outside the domain is refused.
    RefusesOutside,
}

/// A law of the kind contract that a kind broke, the parameter where, and the error if a call
/// failed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContractError {
    /// The law broken.
    pub law: Law,
    /// The parameter (or first parameter) where it broke.
    pub at: f64,
    /// The error, if the law broke because a call the law needs failed.
    pub error: Option<CurveError>,
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the kind breaks {:?} at {}", self.law, self.at)?;
        if let Some(e) = self.error {
            write!(f, ": {e}")?;
        }
        Ok(())
    }
}

impl core::error::Error for ContractError {}
