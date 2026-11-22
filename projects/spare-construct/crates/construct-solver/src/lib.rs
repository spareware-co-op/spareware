//! Spare Construct: 2D sketch constraint solver.
//!
//! Points and straight lines on one plane, held by coincident, horizontal, vertical, parallel,
//! perpendicular, equal-length, point-to-point and point-to-line distance and fixed-point
//! constraints, all solved together. The outlines floors, roofs, rooms, stairs and families are
//! drawn with. Map: projects/spare-construct/ARCHITECTURE.md, "Regeneration and constraints";
//! note CON-A3-F2-P1-solver-home (where the solver lives for now, and why).
//!
//! - [`sketch`]: what a sketch is, in whole millimetres and authoring order.
//! - [`outcome`]: what a solve gives back: whole-millimetre points and a verdict (fully or
//!   under-constrained, with the degrees of freedom left and the points that carry them, and any
//!   redundant constraints), or an error value (over-constrained with the conflicting constraints,
//!   not converged, out of work, refused at rounding).
//! - [`solve`]: the one call.
//!
//! Promises: the same sketch gives the same bytes on every run and every machine (no clocks, no
//! random starts, no hash-map order; IEEE `+ - * /` and `sqrt` only); work is counted in steps,
//! never time; fixed points never move; nothing is dropped or relaxed silently; a solved sketch
//! meets every constraint within the model tolerance (0.01 mm) after rounding to whole
//! millimetres, or is refused.
//!
//! Interim: the engine behind [`solve`] is a private module of this crate, our own code following
//! the kernel's chapter 15 method. The kernel's `geom-sketch` (pieces G216 to G228, the solver
//! CON-A27-F1-P2 names) will answer behind the same types through `engine-geometry` when it lands,
//! and the module then goes; callers and the tests here do not change.
//!
//! Depends on nalgebra (its singular value decomposition, default features off), num-dual (dual
//! numbers for derivatives) and serde. Arcs, circles, tangency and dragging come next (job 2.8):
//! they add entities to [`sketch`], residuals to the engine and nothing else.

mod engine;
pub mod outcome;
pub mod sketch;

pub use outcome::{Budget, FreePoint, MODEL_TOLERANCE_MM, SolveError, Solved, Unmet, Verdict};
pub use sketch::{
    COORD_LIMIT_MM, Constraint, ConstraintId, Line, LineId, Point, PointId, Side, Sketch,
    SketchError,
};

/// Solves a sketch from its points as drawn, within a work budget.
///
/// Each solved coordinate is rounded to the nearest whole millimetre, halves away from zero, and
/// every constraint is then measured again on the rounded points: a sketch that no longer meets
/// one within [`MODEL_TOLERANCE_MM`] is refused with [`SolveError::RoundingRefused`].
///
/// # Errors
///
/// Every [`SolveError`]: over-constrained (with the conflicting constraints), not converged,
/// out of work, refused at rounding, degenerate as drawn, rank too close to call, out of range.
pub fn solve(sketch: &Sketch, budget: Budget) -> Result<Solved, SolveError> {
    engine::solve(sketch, budget)
}
