//! What a solve gives back: the solved points in whole millimetres with the sketch's verdict, or
//! an error the caller can read. Both serialise to the same bytes on every machine.

use crate::sketch::{ConstraintId, Point, PointId};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The model tolerance, in millimetres (main's ruling, 22 Nov 2026). A solved sketch is accepted
/// only if every constraint still holds within it after its points are rounded to whole
/// millimetres.
pub const MODEL_TOLERANCE_MM: f64 = 0.01;

/// How much work one solve may do, counted in steps, never in time. A step is one evaluation of
/// the constraints with one factorisation, or one trial move. Running out is
/// [`SolveError::WorkLimit`], never a best-so-far answer.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    steps: u32,
}

impl Budget {
    /// Enough for any sketch that converges at all: a well-drawn outline takes a few steps.
    pub const DEFAULT: Budget = Budget { steps: 200 };

    /// A budget of this many steps.
    #[must_use]
    pub const fn steps(steps: u32) -> Self {
        Budget { steps }
    }

    /// The number of steps allowed.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.steps
    }
}

/// A point that can still move, and in how many independent ways (1 or 2).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreePoint {
    /// The point.
    pub point: PointId,
    /// Its own freedoms: 1, it slides along a path; 2, it moves anywhere nearby.
    pub dof: u8,
}

/// Whether the constraints pin the sketch down.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing can move.
    FullyConstrained,
    /// The sketch can still move. `dof` counts every independent way it can, placement included,
    /// so a sketch with no fixed point keeps at least three (two slides and a turn). `free` lists
    /// each point that can move, in authoring order; points that only move together share the
    /// freedoms, so their counts can add up to more than `dof`.
    UnderConstrained {
        /// The degrees of freedom left.
        dof: u32,
        /// The points that can move.
        free: Vec<FreePoint>,
    },
}

/// A solved sketch.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Solved {
    /// Every point, in authoring order, in whole millimetres. Fixed points are as drawn.
    pub points: Vec<Point>,
    /// Fully or under-constrained.
    pub verdict: Verdict,
    /// Constraints that hold but repeat what others already say: a consistent over-constraint,
    /// such as a fourth equal length on a square, or a point fixed twice. Each is in a group of
    /// constraints one of which could go. Empty when nothing repeats.
    pub redundant: Vec<ConstraintId>,
    /// The steps used.
    pub steps: u32,
}

/// A constraint the solve could not meet, and by how much it misses, in millimetres.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Unmet {
    /// The constraint.
    pub constraint: ConstraintId,
    /// How far it is from holding, in millimetres.
    pub residual_mm: f64,
}

/// Why a sketch was not solved. Nothing is changed by a failed solve.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum SolveError {
    /// Over-constrained: these constraints contradict each other, and no position meets them
    /// all. Removing any one of them may fix it.
    OverConstrained {
        /// The conflicting constraints, in authoring order.
        conflicting: Vec<ConstraintId>,
    },
    /// No position meeting every constraint was found from the sketch as drawn. This says "not
    /// found from this start", never "none exists": the constraints are independent, so it is not
    /// a conflict between them.
    NotConverged {
        /// The constraints left unmet, in authoring order.
        unmet: Vec<Unmet>,
    },
    /// The work limit ran out.
    WorkLimit {
        /// The steps used.
        steps: u32,
    },
    /// The solved sketch, rounded to whole millimetres by the stated rule (`crate::solve`), no
    /// longer meets this constraint within [`MODEL_TOLERANCE_MM`]. The sketch is refused rather
    /// than stored loose.
    RoundingRefused {
        /// The first constraint, in authoring order, that rounding breaks.
        constraint: ConstraintId,
        /// How far it misses after rounding, in millimetres.
        residual_mm: f64,
    },
    /// A constraint needs a direction or a distance that the sketch as drawn (or as rounded) does
    /// not have: a line of no length where a direction is asked for, or two points drawn at one
    /// place with a distance between them. Draw it roughly first.
    Degenerate {
        /// The constraint.
        constraint: ConstraintId,
    },
    /// Whether these constraints are independent cannot be told at a millimetre's resolution
    /// (the sketch is too close to a special shape, such as two lines nearly in line). Neither
    /// verdict is guessed.
    RankUnclear {
        /// The constraints involved, in authoring order.
        constraints: Vec<ConstraintId>,
    },
    /// A solved point lies beyond the sketch's range.
    OutOfRange {
        /// The point.
        point: PointId,
    },
    /// The matrix decomposition did not settle within its own iteration cap.
    Decomposition,
}

impl fmt::Display for SolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolveError::OverConstrained { conflicting } => {
                write!(
                    f,
                    "over-constrained: {} constraints conflict",
                    conflicting.len()
                )
            }
            SolveError::NotConverged { unmet } => {
                write!(f, "not solved: {} constraints unmet", unmet.len())
            }
            SolveError::WorkLimit { steps } => write!(f, "work limit reached after {steps} steps"),
            SolveError::RoundingRefused {
                constraint,
                residual_mm,
            } => write!(
                f,
                "constraint {} misses by {residual_mm} mm after rounding to whole millimetres",
                constraint.0
            ),
            SolveError::Degenerate { constraint } => {
                write!(
                    f,
                    "constraint {} has no direction or distance to work from",
                    constraint.0
                )
            }
            SolveError::RankUnclear { constraints } => write!(
                f,
                "cannot tell whether {} constraints are independent",
                constraints.len()
            ),
            SolveError::OutOfRange { point } => {
                write!(f, "point {} is solved beyond 1,000 km", point.0)
            }
            SolveError::Decomposition => f.write_str("the matrix decomposition did not settle"),
        }
    }
}

impl std::error::Error for SolveError {}
