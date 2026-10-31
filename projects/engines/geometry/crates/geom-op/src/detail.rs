//! Error details: each operation family's own typed failures, each mapped to one base kind; and
//! the two the whole kernel shares, the parallel helper's stops and the numbers layer's failures.
//!
//! What it does: [`OpErrorDetail`] has one variant per family of details (chapter 22), and grows
//! with the kernel: each operation family adds its own enum as a variant, as its piece lands. This
//! module holds the two every operation can meet. [`RunDetail`] is a stop from `geom-run`: a budget
//! that ran out, the caller's cancellation, a refused allocation, or a broken invariant of the
//! helper. [`NumDetail`] is a failure of `geom-num`, wrapped by meaning rather than by name
//! (chapter 22: "A crate's own error type never reaches a caller"). Every detail says its base
//! kind through its `kind` function, which the map reads to list each detail with its base kind
//! (chapter 26.3), and gives the remedies it knows.
//!
//! Guarantees:
//!
//! - **One base kind per detail.** Each variant maps to exactly one [`ErrorKind`] in its `kind`
//!   function, written as a match the map reads; a family variant of [`OpErrorDetail`] takes the
//!   kind of the detail it holds.
//! - **The numbers layer maps by meaning** (chapter 22, and the open question of section 31.7 that
//!   names this piece): an exact computation over its proven bit budget is a defect, so
//!   [`NumDetail::BitBudget`] is `Internal`, never `BudgetExceeded`; a certified bound over its
//!   ceiling is [`NumDetail::BoundOverCeiling`], `ExactnessNotAccepted`; a coordinate past the frame
//!   is `FrameRangeExceeded`; a degenerate construction is `Degenerate`; maths with no proven
//!   enclosure is `Unsupported`. The six section 22 left open are decided here, each with its
//!   reason on its variant: a value that is not finite is `Degenerate`; an argument outside a
//!   function's domain is `NoSolution`; an interval divisor that contains zero and the square root
//!   of an interval with negative values are `Undecided`; an empty interval and a grid-step
//!   conversion the wrong way are `Internal`.
//! - **The helper's stops keep their meaning.** A budget failure keeps its `{ budget, kind, phase,
//!   used }` record, and a batch's total stays the batch's ([`RunDetail::BatchTotal`]), never
//!   cached as an item's result; cancellation and refused allocations stay `Cancelled` and
//!   `OutOfMemory`, which are never stored (chapter 23.10).
//!
//! Method: chapter 22's shared base and per-operation detail, and its mapping of the numbers
//! layer's errors (`projects/engines/geometry/architecture/22-errors-queries-ai.md`), with
//! chapter 23.10's budget and batch rules
//! (`projects/engines/geometry/architecture/23-performance.md`).
//!
//! Cost: constant; the details are a few machine words, and only [`OpErrorDetail::remedies`]
//! allocates, the vector it returns.

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use geom_num::NumError;
use geom_run::{Exceeded, Interrupted, ItemError, RunError};

use geom_num::Exactness;

use crate::evidence::Value;
use crate::kind::ErrorKind;
use crate::remedy::{Bound, Remedy};

/// The operation's own typed failure (chapter 22): one variant per family of details, each holding
/// that family's enum. Grows with the kernel; generic code reads an error's
/// [`kind`](crate::OpError::kind) and the operation's schema instead of matching it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OpErrorDetail {
    /// A stop from the parallel helper, its budgets and its cancellation.
    Run(RunDetail),
    /// A failure of the numbers layer, wrapped by meaning.
    Num(NumDetail),
    /// A failure of the Make and Transform families: primitives, transforms and patterns
    /// (`geom-make`, piece G101).
    Make(MakeDetail),
    /// A failure of a query (chapter 22; piece G102).
    Query(QueryDetail),
}

impl OpErrorDetail {
    /// The base kind: the kind of the detail held.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            OpErrorDetail::Run(detail) => detail.kind(),
            OpErrorDetail::Num(detail) => detail.kind(),
            OpErrorDetail::Make(detail) => detail.kind(),
            OpErrorDetail::Query(detail) => detail.kind(),
        }
    }

    /// The remedies this detail knows by itself; the operation adds those only it can know.
    #[must_use]
    pub fn remedies(&self) -> Vec<Remedy> {
        match self {
            OpErrorDetail::Run(detail) => detail.remedies(),
            OpErrorDetail::Num(detail) => detail.remedies(),
            OpErrorDetail::Make(detail) => detail.remedies(),
            OpErrorDetail::Query(detail) => detail.remedies(),
        }
    }
}

impl fmt::Display for OpErrorDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpErrorDetail::Run(detail) => fmt::Display::fmt(detail, f),
            OpErrorDetail::Num(detail) => fmt::Display::fmt(detail, f),
            OpErrorDetail::Make(detail) => fmt::Display::fmt(detail, f),
            OpErrorDetail::Query(detail) => fmt::Display::fmt(detail, f),
        }
    }
}

impl core::error::Error for OpErrorDetail {}

/// A stop from the parallel helper (`geom-run`): a budget, the caller's cancellation, the
/// machine's memory, or a broken invariant of the helper.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RunDetail {
    /// A deterministic budget of the operation, or of one batch item, ran out: which budget, which
    /// limit, in which phase, and what had been used, counted as a serial loop counts it.
    BudgetExceeded(Exceeded),
    /// A batch's total ran out at or before this item, in key order. It belongs to the batch, not
    /// to the item, so it is never cached as the item's result (chapter 23.10).
    BatchTotal(Exceeded),
    /// The caller's cancellation stopped the operation. Not deterministic; never stored.
    Cancelled,
    /// The machine refused a large allocation. Not deterministic; never stored.
    OutOfMemory,
    /// An invariant of the helper broke: a bug, reported instead of a panic or a wrong answer.
    Internal {
        /// What broke, as the helper says it.
        what: &'static str,
    },
}

impl RunDetail {
    /// The base kind.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            RunDetail::BudgetExceeded(_) | RunDetail::BatchTotal(_) => ErrorKind::BudgetExceeded,
            RunDetail::Cancelled => ErrorKind::Cancelled,
            RunDetail::OutOfMemory => ErrorKind::OutOfMemory,
            RunDetail::Internal { .. } => ErrorKind::Internal,
        }
    }

    /// The remedies chapter 22 gives each stop: for a budget, raise it to at least what was used,
    /// split the call, or ask for a preview; for a defect, report it; for the rest, none.
    #[must_use]
    pub fn remedies(&self) -> Vec<Remedy> {
        match self {
            RunDetail::BudgetExceeded(exceeded) | RunDetail::BatchTotal(exceeded) => vec![
                Remedy::RaiseBudget {
                    kind: exceeded.kind,
                    at_least: exceeded.used.get(exceeded.kind),
                },
                Remedy::SplitCall,
                Remedy::Preview,
            ],
            RunDetail::Cancelled | RunDetail::OutOfMemory => Vec::new(),
            RunDetail::Internal { .. } => vec![Remedy::Report],
        }
    }
}

impl fmt::Display for RunDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunDetail::BudgetExceeded(exceeded) => write!(f, "{exceeded}"),
            RunDetail::BatchTotal(exceeded) => {
                write!(
                    f,
                    "the batch's total ran out at or before this item: {exceeded}"
                )
            }
            RunDetail::Cancelled => f.write_str("cancelled by the caller; nothing was published"),
            RunDetail::OutOfMemory => {
                f.write_str("the machine refused a large allocation; nothing was published")
            }
            RunDetail::Internal { what } => write!(
                f,
                "internal error in the parallel helper ({what}); nothing was published, please \
                 report it"
            ),
        }
    }
}

impl core::error::Error for RunDetail {}

impl From<Interrupted> for RunDetail {
    fn from(interrupted: Interrupted) -> Self {
        match interrupted {
            Interrupted::Cancelled => RunDetail::Cancelled,
            Interrupted::OutOfMemory => RunDetail::OutOfMemory,
            Interrupted::Internal(what) => RunDetail::Internal { what },
        }
    }
}

/// A failure of the numbers layer (`geom-num`'s `NumError`), named and mapped by what it means for
/// the operation (chapter 22).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NumDetail {
    /// A coordinate would leave ±(2^40 − 1) grid steps of its frame's origin (`NumError::OutOfRange`).
    OutOfRange {
        /// The coordinate, in grid steps; `i128::MAX` and `i128::MIN` stand for at least and at
        /// most those values.
        value: i128,
    },
    /// The inputs are degenerate for a construction: collinear points for a plane, or three planes
    /// that do not meet in exactly one point (`NumError::Degenerate`).
    Degenerate,
    /// A value or a result was not finite: a pole, such as a division by zero or the logarithm of
    /// zero, or an overflow (`NumError::NotFinite`). Base kind `Degenerate`, decided by this piece:
    /// kernel values are bounded (coordinates within ±2^40 steps), so on valid input a computation
    /// leaves the finite doubles only at a singularity of the configuration, which is chapter 22's
    /// "degenerate for this operation"; an operation that reads a caller's number checks it first,
    /// and reports a non-finite one as its own `InvalidInput` detail.
    NotFinite,
    /// A maths function was asked for a value it does not have: the square root or logarithm of a
    /// negative number, the arcsine of a number beyond ±1 (`NumError::OutsideDomain`). Base kind
    /// `NoSolution`, decided by this piece: no real value exists, so no result meets the input,
    /// which is chapter 22's `NoSolution`; it is not a malformed input, because the argument was
    /// computed from valid ones.
    OutsideDomain,
    /// An interval division by an interval that contains zero (`NumError::DivisorContainsZero`).
    /// Base kind `Undecided`, decided by this piece: the enclosure cannot say whether the divisor
    /// vanishes, so the decision cannot be certified at this resolution; an exact zero is found by
    /// the exact predicates first, and is `Degenerate`.
    DivisorContainsZero,
    /// An interval square root of an interval with negative values in it
    /// (`NumError::NegativeSqrt`). Base kind `Undecided`, decided by this piece for the reason
    /// [`NumDetail::DivisorContainsZero`] gives: the enclosure cannot say whether the value is
    /// negative.
    NegativeSqrt,
    /// An empty interval was asked for: a lower bound above its upper bound
    /// (`NumError::EmptyInterval`). Base kind `Internal`, decided by this piece: the kernel builds
    /// intervals from bounds it computed, so an empty one is a defect; a range a caller gives is
    /// checked by the operation as its own input.
    EmptyInterval,
    /// A conversion between grid steps was asked to go the wrong way (`NumError::WrongDirection`).
    /// Base kind `Internal`, decided by this piece: the direction is chosen by kernel code, never
    /// by data, so going the wrong way is a defect.
    WrongDirection,
    /// An exact computation did not fit the width its proven bit budget gives it
    /// (`NumError::BudgetExceeded`). Only a defect can cause it, so its base kind is `Internal`,
    /// never `BudgetExceeded` (chapter 22).
    BitBudget,
    /// A certified bound would pass the ceiling it was checked against
    /// (`NumError::ToleranceExceeded`): the result would be less exact than accepted, so its base
    /// kind is `ExactnessNotAccepted`, never `ToleranceExceeded` (chapter 22).
    BoundOverCeiling,
    /// An enclosure needs a maths function whose error is not proven (`NumError::UnprovenMaths`).
    /// The wrapping operation names the function and the piece that adds its proof (chapters 12.16
    /// and 22).
    UnprovenMaths,
}

impl NumDetail {
    /// The base kind.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            NumDetail::OutOfRange { .. } => ErrorKind::FrameRangeExceeded,
            NumDetail::Degenerate | NumDetail::NotFinite => ErrorKind::Degenerate,
            NumDetail::OutsideDomain => ErrorKind::NoSolution,
            NumDetail::DivisorContainsZero | NumDetail::NegativeSqrt => ErrorKind::Undecided,
            NumDetail::EmptyInterval | NumDetail::WrongDirection | NumDetail::BitBudget => {
                ErrorKind::Internal
            }
            NumDetail::BoundOverCeiling => ErrorKind::ExactnessNotAccepted,
            NumDetail::UnprovenMaths => ErrorKind::Unsupported,
        }
    }

    /// The remedies the numbers layer knows by itself: a frame for a coordinate out of range, a
    /// report for a defect, and none for the rest, whose remedies only the operation knows.
    #[must_use]
    pub fn remedies(&self) -> Vec<Remedy> {
        match self.kind() {
            ErrorKind::FrameRangeExceeded => vec![Remedy::ChangeFrame],
            ErrorKind::Internal => vec![Remedy::Report],
            _ => Vec::new(),
        }
    }
}

impl fmt::Display for NumDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NumDetail::OutOfRange { value } => {
                write!(f, "{}", NumError::OutOfRange { value: *value })
            }
            NumDetail::Degenerate => f.write_str("the inputs are degenerate for this construction"),
            NumDetail::NotFinite => f.write_str(
                "a value was not finite: the configuration is singular here (a division by zero, \
                 a pole or an overflow)",
            ),
            NumDetail::OutsideDomain => {
                f.write_str("a maths function was asked for a value it does not have")
            }
            NumDetail::DivisorContainsZero => {
                f.write_str("whether a divisor is zero cannot be decided at this resolution")
            }
            NumDetail::NegativeSqrt => f.write_str(
                "whether a value under a square root is negative cannot be decided at this \
                 resolution",
            ),
            NumDetail::EmptyInterval => {
                f.write_str("internal error: an empty interval was asked for; please report it")
            }
            NumDetail::WrongDirection => f.write_str(
                "internal error: a grid-step conversion went the wrong way; please report it",
            ),
            NumDetail::BitBudget => f.write_str(
                "internal error: an exact computation exceeded its proven bit budget; please \
                 report it",
            ),
            NumDetail::BoundOverCeiling => {
                f.write_str("a certified bound would pass the ceiling the caller accepted")
            }
            NumDetail::UnprovenMaths => {
                f.write_str("this needs a maths function with no certified enclosure yet")
            }
        }
    }
}

impl core::error::Error for NumDetail {}

impl From<NumError> for NumDetail {
    fn from(error: NumError) -> Self {
        match error {
            NumError::OutOfRange { value } => NumDetail::OutOfRange { value },
            NumError::Degenerate => NumDetail::Degenerate,
            NumError::NotFinite => NumDetail::NotFinite,
            NumError::WrongDirection => NumDetail::WrongDirection,
            NumError::DivisorContainsZero => NumDetail::DivisorContainsZero,
            NumError::NegativeSqrt => NumDetail::NegativeSqrt,
            NumError::EmptyInterval => NumDetail::EmptyInterval,
            NumError::OutsideDomain => NumDetail::OutsideDomain,
            NumError::BudgetExceeded => NumDetail::BitBudget,
            NumError::ToleranceExceeded => NumDetail::BoundOverCeiling,
            NumError::UnprovenMaths => NumDetail::UnprovenMaths,
        }
    }
}

/// A failure of the Make and Transform families (`geom-make`, piece G101): an input the shape
/// cannot be made from, a transform the result could not keep exact or certified, or a defect
/// found before anything was returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MakeDetail {
    /// A length the shape needs to be positive is zero: an extent, a radius or a height, the input
    /// `input` names.
    ZeroLength {
        /// The input, by its name in the operation's schema.
        input: &'static str,
    },
    /// A prism's base has fewer than three corners.
    TooFewCorners {
        /// How many it has.
        corners: u64,
    },
    /// A prism's base corners do not all lie in one plane perpendicular to the z axis.
    BaseNotLevel,
    /// A prism's base is not a strictly convex polygon running counter-clockwise seen from above:
    /// three corners in a row on one line, a turn the other way, or corners repeated.
    BaseNotConvex,
    /// A torus's minor radius is not below its major radius: it would be a horn or spindle torus,
    /// not a primitive (chapter 10.2).
    MinorNotBelowMajor {
        /// The major radius, in grid steps; the minor radius must be below it.
        major: u64,
    },
    /// A transform would leave the body less exact than the class the caller accepted.
    NotAccepted {
        /// The class and bound the result would have.
        would: Exactness,
    },
    /// A transform would turn a curved face or edge into a kind the checker cannot certify yet:
    /// a scale that is not uniform makes cylinders, cones, spheres and tori affine surfaces and
    /// circles ellipses.
    KindNotCertified,
    /// A transform that rounds corners or moves curved geometry by a certified deviation gave a
    /// body the checker cannot certify yet: rounded corners leave their faces' planes, and a tilted
    /// curved face meets its caps where only exact data is followed today.
    NotCertifiedYet {
        /// The piece that widens what the checker certifies, by its board number.
        piece: &'static str,
    },
    /// A placed instance names a row past the document's side table of transforms.
    NoSuchPlacement {
        /// The row.
        index: u32,
    },
    /// The body given carries no names: it was not published in name order, so its entities
    /// cannot keep their names through the transform.
    BodyNotNamed,
    /// A pattern has no instance.
    EmptyPattern,
    /// Two instances of a pattern have one key.
    RepeatedKey,
    /// A pattern's transform has no inverse: it would collapse the body onto a plane, a line or a
    /// point.
    SingularTransform,
    /// The result did not pass the checker. A defect: nothing was returned.
    ResultInvalid,
    /// The body could not be assembled from its rows. A defect.
    BuildFailed,
}

impl MakeDetail {
    /// The base kind.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            MakeDetail::ZeroLength { .. } => ErrorKind::Degenerate,
            MakeDetail::TooFewCorners { .. }
            | MakeDetail::BaseNotLevel
            | MakeDetail::BaseNotConvex
            | MakeDetail::MinorNotBelowMajor { .. }
            | MakeDetail::BodyNotNamed
            | MakeDetail::NoSuchPlacement { .. }
            | MakeDetail::EmptyPattern
            | MakeDetail::RepeatedKey
            | MakeDetail::SingularTransform => ErrorKind::InvalidInput,
            MakeDetail::NotAccepted { .. } => ErrorKind::ExactnessNotAccepted,
            MakeDetail::KindNotCertified | MakeDetail::NotCertifiedYet { .. } => {
                ErrorKind::Unsupported
            }
            MakeDetail::ResultInvalid | MakeDetail::BuildFailed => ErrorKind::Internal,
        }
    }

    /// The remedies this detail knows: the input to change and, where it is known, the nearest
    /// value that works; the class to accept; the piece that adds a route; or a report.
    #[must_use]
    pub fn remedies(&self) -> Vec<Remedy> {
        let change =
            |input: &'static str, to: Option<(Bound, Value)>| Remedy::ChangeInput { input, to };
        match self {
            MakeDetail::ZeroLength { input } => {
                vec![change(input, Some((Bound::AtLeast, Value::Count(1))))]
            }
            MakeDetail::TooFewCorners { .. } => {
                vec![change("base", Some((Bound::AtLeast, Value::Count(3))))]
            }
            MakeDetail::BaseNotLevel | MakeDetail::BaseNotConvex => vec![change("base", None)],
            MakeDetail::MinorNotBelowMajor { major } => vec![change(
                "minor",
                Some((
                    Bound::AtMost,
                    Value::Count(u128::from(major.saturating_sub(1))),
                )),
            )],
            MakeDetail::NotAccepted { would } => vec![Remedy::Accept { class: *would }],
            MakeDetail::KindNotCertified => vec![Remedy::AwaitPiece { piece: "G177" }],
            MakeDetail::NotCertifiedYet { piece } => vec![Remedy::AwaitPiece { piece }],
            MakeDetail::BodyNotNamed => vec![change("body", None)],
            MakeDetail::NoSuchPlacement { .. } => vec![change("placement", None)],
            MakeDetail::EmptyPattern | MakeDetail::RepeatedKey | MakeDetail::SingularTransform => {
                vec![change("instances", None)]
            }
            MakeDetail::ResultInvalid | MakeDetail::BuildFailed => vec![Remedy::Report],
        }
    }
}

impl fmt::Display for MakeDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MakeDetail::ZeroLength { input } => write!(f, "the length {input} is zero"),
            MakeDetail::TooFewCorners { corners } => {
                write!(f, "the base has {corners} corners, fewer than three")
            }
            MakeDetail::BaseNotLevel => f.write_str("the base corners are not at one height"),
            MakeDetail::BaseNotConvex => f.write_str(
                "the base is not a strictly convex polygon running counter-clockwise from above",
            ),
            MakeDetail::MinorNotBelowMajor { major } => {
                write!(f, "the minor radius is not below the major radius, {major}")
            }
            MakeDetail::NotAccepted { would } => {
                write!(f, "the result would be {would:?}, which was not accepted")
            }
            MakeDetail::KindNotCertified => f.write_str(
                "the transform would make curved geometry of a kind the checker cannot certify yet",
            ),
            MakeDetail::NotCertifiedYet { piece } => write!(
                f,
                "the checker cannot certify the transformed body yet; piece {piece} widens it"
            ),
            MakeDetail::BodyNotNamed => f.write_str("the body carries no names"),
            MakeDetail::NoSuchPlacement { index } => {
                write!(f, "row {index} is past the table of placements")
            }
            MakeDetail::EmptyPattern => f.write_str("the pattern has no instance"),
            MakeDetail::RepeatedKey => f.write_str("two instances of the pattern have one key"),
            MakeDetail::SingularTransform => {
                f.write_str("a transform of the pattern has no inverse")
            }
            MakeDetail::ResultInvalid => f.write_str("the result did not pass the checker"),
            MakeDetail::BuildFailed => f.write_str("the body could not be assembled"),
        }
    }
}

impl core::error::Error for MakeDetail {}

/// Why a query could not answer (chapter 22).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QueryDetail {
    /// A link into a row its arena does not have, or a list that does not end: never in a body
    /// published by the builder and passed by the checker.
    Malformed,
    /// An edge is not a straight line between two vertices, and the query needs polygons.
    CurvedEdge {
        /// The edge's row.
        edge: u32,
    },
    /// A face is not on a plane, and the query needs polygons.
    CurvedFace {
        /// The face's row.
        face: u32,
    },
    /// A face has no loop, and the query needs polygons.
    NoLoops {
        /// The face's row.
        face: u32,
    },
    /// A volume asked of a body that is not a solid.
    NotSolid,
    /// A sum past its integer's width: never for a body within its arenas' limits.
    Overflow,
}

impl QueryDetail {
    /// The base kind.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            QueryDetail::Malformed | QueryDetail::NotSolid => ErrorKind::InvalidInput,
            QueryDetail::CurvedEdge { .. }
            | QueryDetail::CurvedFace { .. }
            | QueryDetail::NoLoops { .. } => ErrorKind::Unsupported,
            QueryDetail::Overflow => ErrorKind::Internal,
        }
    }

    /// The remedies this detail knows: the input to change, the piece that adds a route, or a
    /// report.
    #[must_use]
    pub fn remedies(&self) -> Vec<Remedy> {
        let change = |input: &'static str| Remedy::ChangeInput { input, to: None };
        match self {
            QueryDetail::Malformed | QueryDetail::NotSolid => vec![change("body")],
            QueryDetail::CurvedEdge { .. }
            | QueryDetail::CurvedFace { .. }
            | QueryDetail::NoLoops { .. } => vec![Remedy::AwaitPiece { piece: "G177" }],
            QueryDetail::Overflow => vec![Remedy::Report],
        }
    }
}

impl fmt::Display for QueryDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryDetail::Malformed => f.write_str("the body's links do not make a body"),
            QueryDetail::CurvedEdge { edge } => {
                write!(f, "edge {edge} is not a straight line between two vertices")
            }
            QueryDetail::CurvedFace { face } => write!(f, "face {face} is not on a plane"),
            QueryDetail::NoLoops { face } => write!(f, "face {face} has no loop"),
            QueryDetail::NotSolid => f.write_str("the body is not a solid"),
            QueryDetail::Overflow => f.write_str("a sum went past its integer's width"),
        }
    }
}

impl core::error::Error for QueryDetail {}

impl From<QueryDetail> for OpErrorDetail {
    fn from(detail: QueryDetail) -> Self {
        OpErrorDetail::Query(detail)
    }
}

impl From<MakeDetail> for OpErrorDetail {
    fn from(detail: MakeDetail) -> Self {
        OpErrorDetail::Make(detail)
    }
}

impl From<RunDetail> for OpErrorDetail {
    fn from(detail: RunDetail) -> Self {
        OpErrorDetail::Run(detail)
    }
}

impl From<NumDetail> for OpErrorDetail {
    fn from(detail: NumDetail) -> Self {
        OpErrorDetail::Num(detail)
    }
}

impl From<NumError> for OpErrorDetail {
    fn from(error: NumError) -> Self {
        OpErrorDetail::Num(error.into())
    }
}

impl From<Interrupted> for OpErrorDetail {
    fn from(interrupted: Interrupted) -> Self {
        OpErrorDetail::Run(interrupted.into())
    }
}

/// A phase's failure: the function's own error becomes its detail, and the helper's stops become
/// [`RunDetail`]s.
impl<E: Into<OpErrorDetail>> From<RunError<E>> for OpErrorDetail {
    fn from(error: RunError<E>) -> Self {
        match error {
            RunError::Failed(error) => error.into(),
            RunError::BudgetExceeded(exceeded) => {
                OpErrorDetail::Run(RunDetail::BudgetExceeded(exceeded))
            }
            RunError::Cancelled => OpErrorDetail::Run(RunDetail::Cancelled),
            RunError::OutOfMemory => OpErrorDetail::Run(RunDetail::OutOfMemory),
            RunError::Internal(what) => OpErrorDetail::Run(RunDetail::Internal { what }),
        }
    }
}

/// A batch item's failure: its own error becomes its detail, its own budget a
/// [`RunDetail::BudgetExceeded`], and the batch's total a [`RunDetail::BatchTotal`].
impl<E: Into<OpErrorDetail>> From<ItemError<E>> for OpErrorDetail {
    fn from(error: ItemError<E>) -> Self {
        match error {
            ItemError::Failed(error) => error.into(),
            ItemError::BudgetExceeded(exceeded) => {
                OpErrorDetail::Run(RunDetail::BudgetExceeded(exceeded))
            }
            ItemError::BatchTotal(exceeded) => OpErrorDetail::Run(RunDetail::BatchTotal(exceeded)),
        }
    }
}
