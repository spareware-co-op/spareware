//! Spare Construct: 2D sketch constraint solver.
//!
//! Points, straight lines, circles and arcs on one plane, held by coincident, horizontal,
//! vertical, parallel, perpendicular, equal-length, point-to-point and point-to-line distance,
//! point on line, point on circle or arc, concentric, equal-radius, radius, diameter, tangent
//! (line to circle or arc, and circle or arc to circle or arc) and fixed-point constraints, all
//! solved together. The outlines floors, roofs, rooms, stairs and families are drawn with. Map:
//! projects/spare-construct/ARCHITECTURE.md, "Regeneration and constraints"; notes
//! CON-A3-F2-P1-solver-home (where the solver lives for now, and why) and
//! CON-A3-F2-P1-curves-and-drag (how curves are stored and solved, and how a drag works).
//!
//! - [`sketch`]: what a sketch is, in whole grid steps of the geometry kernel (`Q0`, 1/6,400 mm)
//!   and authoring order, with [`mm`] and [`Point::mm`] to give it whole millimetres.
//! - [`outcome`]: what a solve gives back: points and radii in grid steps and a verdict (fully
//!   or under-constrained, with the degrees of freedom left and the points and radii that carry
//!   them, and any redundant constraints), or an error value (over-constrained with the
//!   conflicting constraints, not converged, out of work, refused at rounding); and what a drag
//!   gives back.
//! - [`solve`] and [`drag`]: the two calls; and [`check_drawn`], whether a sketch as drawn
//!   already meets its constraints, by the same measure a solve's answer is checked with.
//!
//! Promises: the same sketch gives the same bytes on every run and every machine (no clocks, no
//! random starts, no hash-map order; IEEE `+ - * /` and `sqrt` only); work is counted in steps,
//! never time; fixed points never move; nothing is dropped or relaxed silently; a solved sketch
//! meets every constraint, and every arc has its ends on its circle, within the model tolerance
//! (0.01 mm) after rounding points and radii to the grid, or is refused. Rounding moves a point
//! at most half a step (78 nm) along each axis, so sloped and curved sketches are accepted.
//!
//! Interim: the engine behind [`solve`] and [`drag`] is a private module of this crate, our own
//! code following the kernel's chapter 15 method. The kernel's `geom-sketch` (pieces G216 to
//! G228, the solver CON-A27-F1-P2 names) will answer behind the same types through
//! `engine-geometry` when it lands, and the module then goes; callers and the tests here do not
//! change.
//!
//! Depends on nalgebra (its singular value decomposition, default features off), num-dual (dual
//! numbers for derivatives) and serde. Not here yet, from chapter 15.3's list: an angle between
//! lines (it needs a stored angle unit and sine and cosine, which this crate's arithmetic rule
//! does not allow), and midpoints, symmetry, arc sweep and length, ellipses and splines.

mod engine;
pub mod outcome;
pub mod sketch;

pub use outcome::{
    Budget, Dragged, FreePoint, MODEL_TOLERANCE_MM, MODEL_TOLERANCE_STEPS, Reach, SolveError,
    Solved, Unmet, Verdict,
};
pub use sketch::{
    COORD_LIMIT, Constraint, ConstraintId, Curve, CurveId, Line, LineId, Point, PointId,
    STEPS_PER_MM, Side, Sketch, SketchError, Touch, mm,
};

/// Solves a sketch from its points and radii as drawn, within a work budget.
///
/// Each solved coordinate and radius is rounded to the nearest grid step, halves to even as the
/// kernel rounds onto its grid (chapter 6.1), and every arc and every constraint is then measured again on the rounded values: a
/// sketch that no longer meets one within [`MODEL_TOLERANCE_MM`] is refused with
/// [`SolveError::RoundingRefused`] or [`SolveError::ArcEndRefused`].
///
/// # Errors
///
/// Every [`SolveError`]: over-constrained (with the conflicting constraints), not converged,
/// out of work, refused at rounding, degenerate as drawn, rank too close to call, out of range.
pub fn solve(sketch: &Sketch, budget: Budget) -> Result<Solved, SolveError> {
    engine::solve(sketch, budget)
}

/// Checks a sketch as drawn, without solving it: whether its stored points and radii already
/// meet every arc and every constraint within [`MODEL_TOLERANCE_MM`], measured exactly as
/// [`solve`] measures its rounded answer. A family recipe evaluated with its sketches as stored
/// asks this; solving a recipe's sketch is a later job's.
///
/// # Errors
///
/// The first miss, as [`solve`]'s check after rounding reports it:
/// [`SolveError::ArcEndRefused`] or [`SolveError::DegenerateCurve`] for an arc, then
/// [`SolveError::RoundingRefused`] (here: not met as drawn) or [`SolveError::Degenerate`] for the
/// first constraint, in authoring order, that does not hold.
pub fn check_drawn(sketch: &Sketch) -> Result<(), SolveError> {
    engine::check_drawn(sketch)
}

/// Drags one point of a solved sketch towards a target, keeping every constraint (chapter 15.9,
/// one frame).
///
/// `sketch` is the solved sketch: its points and radii are the last solve's answer, written back
/// with [`Sketch::with_positions`]. The point is held at the target and the rest of the sketch is
/// moved by least change until every constraint holds again ([`Reach::Target`]). If the
/// constraints cannot hold with the point there, it is released and the sketch solved from the
/// target, so the point stops at the nearest place they allow ([`Reach::Limit`]); if that is
/// where it was, nothing moves ([`Reach::Refused`]). A fixed point is refused at once. The result
/// is committed by [`solve`]'s rounding rule, and the whole drag counts against one `budget`.
///
/// # Errors
///
/// [`SolveError::UnknownPoint`]; [`SolveError::OutOfRange`] for a target beyond
/// [`COORD_LIMIT`]; any error [`solve`] gives other than a conflict or no convergence with the
/// point held, which release instead.
pub fn drag(
    sketch: &Sketch,
    point: PointId,
    target: Point,
    budget: Budget,
) -> Result<Dragged, SolveError> {
    engine::drag(sketch, point, target, budget)
}
