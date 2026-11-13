//! The sketch model: entities and their variables, constraints and their standing, in authoring
//! order.
//!
//! What it does: a [`Sketch`] holds a plane, its [`Entity`]s and its [`Constraint`]s, each named by
//! an [`ItemId`], the sequence number and key the caller stores with it. Everything is kept sorted
//! by that id, older first, so plan order, factorisation order and every floating-point result that
//! follows depend on stored data alone (chapter 15.2). [`Sketch::variables`] lays the entities'
//! unknowns out in that order, each with the power of two that makes it act as a length (chapter
//! 15.4). A constraint is driving, which the solver must meet, or reference, which is measured and
//! never solved; its priority orders pivots and removal suggestions and nothing else (chapter 15.3).
//!
//! Guarantees:
//!
//! - **One order.** The items' order depends only on their ids, never on the order they were added
//!   in, so a shuffled authoring gives the same sketch.
//! - **Finite.** Every value is finite, and a radius, a length or a distance that must be positive
//!   is, or the item is refused.
//! - **References resolve.** An item that names another names one that exists and of the right kind,
//!   and an item with the id of another is refused.
//! - **Exact scales.** A variable's scale is a power of two, so dividing by it rounds nothing.
//! - **Reference only where it means something.** Only a dimension can be a reference.
//! - **Root choices are stored.** A side, a sense or a tangency is a field of its constraint, and
//!   [`Sketch::drawn_side`], [`Sketch::drawn_outside`] and [`Sketch::drawn_touching_outside`] read it
//!   off a drawing by exact predicates, or say it cannot be read (chapter 15.3).
//!
//! Method: section 15.2's table of entities and variables, and sections 15.3 and 15.4
//! (`projects/engines/geometry/architecture/15-sketches.md`).
//!
//! Cost: adding an item costs a binary search and a move of the items after it, nothing when items
//! arrive in order; O(n) memory. No recursion.

extern crate alloc;

use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_linalg::LinalgError;
use geom_num::NumError;
use geom_num::P2;
use geom_num::Sign;
use geom_num::predicate::orient2d;
use geom_run::Stopped;

use crate::plane::{PlaneError, SketchPlane};

/// An item's name in order: the position in the log of the operation that authored it, then its key
/// within that operation. Neither ever changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ItemId {
    /// The authoring sequence number.
    pub sequence: u64,
    /// The key within that operation.
    pub key: u64,
}

impl ItemId {
    /// The id with this sequence number and key.
    #[must_use]
    pub const fn new(sequence: u64, key: u64) -> ItemId {
        ItemId { sequence, key }
    }
}

/// Why a sketch or one of its items could not be made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SketchError {
    /// Two items have this id.
    Duplicate(ItemId),
    /// Nothing has this id.
    Unknown(ItemId),
    /// This item is not the kind the reference needs.
    WrongKind(ItemId),
    /// A value is NaN or infinite.
    NotFinite,
    /// A radius, a length or a distance that must be positive is not. A distance of zero is authored
    /// as a coincidence (section 15.4).
    NotPositive,
    /// A reference dimension that is not a dimension.
    NotADimension(ItemId),
    /// No side can be read from the drawing: the point is exactly on the line, or off the grid. The
    /// author must state the choice.
    RootChoiceUndetermined(ItemId),
    /// The start is on this constraint's singular set, or the answer within a tolerance of it.
    Degenerate(ItemId),
    /// A block would not converge: this is its worst rule.
    Unsolved(ItemId),
    /// A path could not be followed past this rule, this share of the way along it.
    BranchLost {
        /// The rule.
        constraint: ItemId,
        /// How far along, from 0 to 1.
        along: f64,
    },
    /// Committing moved geometry across this constraint's stored root choice (a side, an outside, a
    /// tangency), or onto its boundary where the choice cannot be told: rounding within a few grid
    /// steps of a degenerate configuration (chapter 15.10, the first check).
    CommitFlip(ItemId),
    /// A constraint is held only to this bound, in grid steps, which is past the fit ceiling
    /// (chapter 15.10, the second check; section 22's `Bound`).
    ExactnessNotAccepted {
        /// The constraint.
        constraint: ItemId,
        /// Its residual's bound, in grid steps.
        bound: f64,
    },
    /// A path's two ends are not one sketch with new values.
    NotTheSameSketch,
    /// Text that is not `sketch v1`, at this line, from 1.
    Text {
        /// The line.
        line: usize,
    },
    /// A plane's frame.
    Plane(PlaneError),
    /// The arithmetic.
    Numeric(NumError),
    /// The linear algebra.
    Linear(LinalgError),
    /// An allocation could not be made.
    OutOfMemory,
    /// The work meter stopped: over budget, or cancelled.
    Stopped,
    /// A step its own bounds rule out failed: a defect, reported, never answered.
    Internal,
}

impl From<Stopped> for SketchError {
    fn from(_: Stopped) -> SketchError {
        SketchError::Stopped
    }
}

impl From<NumError> for SketchError {
    fn from(e: NumError) -> SketchError {
        SketchError::Numeric(e)
    }
}

impl From<LinalgError> for SketchError {
    fn from(e: LinalgError) -> SketchError {
        SketchError::Linear(e)
    }
}

impl From<PlaneError> for SketchError {
    fn from(e: PlaneError) -> SketchError {
        SketchError::Plane(e)
    }
}

impl core::fmt::Display for SketchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            SketchError::Duplicate(_) => "two items have one id",
            SketchError::Unknown(_) => "nothing has this id",
            SketchError::WrongKind(_) => "the item is not the kind needed",
            SketchError::NotFinite => "a value is not finite",
            SketchError::NotPositive => "a value that must be positive is not",
            SketchError::NotADimension(_) => "only a dimension can be a reference",
            SketchError::RootChoiceUndetermined(_) => "the drawing does not decide the root",
            SketchError::Degenerate(_) => "the start or the answer is on a singular set",
            SketchError::Unsolved(_) => "a block would not converge",
            SketchError::BranchLost { .. } => "the path could not be followed",
            SketchError::CommitFlip(_) => "committing moved geometry across a stored root choice",
            SketchError::ExactnessNotAccepted { .. } => {
                "a constraint holds only past the fit ceiling"
            }
            SketchError::NotTheSameSketch => "a path's two ends are not one sketch",
            SketchError::Text { .. } => "text that is not sketch v1",
            SketchError::Plane(_) => "the plane's frame is refused",
            SketchError::Numeric(_) => "the arithmetic refused",
            SketchError::Linear(_) => "the linear algebra refused",
            SketchError::OutOfMemory => "an allocation could not be made",
            SketchError::Stopped => "the work meter stopped",
            SketchError::Internal => "a step its own bounds rule out failed (a defect)",
        })
    }
}

/// How far a line runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extent {
    /// Between its two points.
    Segment,
    /// From its first point, through the second.
    Ray,
    /// Both ways: the second point only fixes the direction.
    Infinite,
}

/// What an entity is, and the values of its variables.
#[derive(Clone, Debug, PartialEq)]
pub enum EntityKind {
    /// A point: x and y.
    Point {
        /// x.
        x: f64,
        /// y.
        y: f64,
    },
    /// A line on two points.
    Line {
        /// Its first point.
        start: ItemId,
        /// Its second point.
        end: ItemId,
        /// How far it runs.
        extent: Extent,
    },
    /// A circle: its radius.
    Circle {
        /// Its centre, a point.
        centre: ItemId,
        /// The radius.
        radius: f64,
    },
    /// A circular arc: its radius; its ends are ordinary points.
    Arc {
        /// Its centre, a point.
        centre: ItemId,
        /// The radius.
        radius: f64,
        /// Its start, a point.
        start: ItemId,
        /// Its end, a point.
        end: ItemId,
    },
    /// An ellipse: its major-axis vector and minor radius.
    Ellipse {
        /// Its centre, a point.
        centre: ItemId,
        /// The major-axis vector.
        axis: [f64; 2],
        /// The minor radius.
        minor: f64,
    },
    /// An elliptical arc: an ellipse's variables, and its two ends, each a point and a parameter on
    /// the arc, which the arc keeps together (piece G219; chapter 15.2).
    EllipticalArc {
        /// Its centre, a point.
        centre: ItemId,
        /// The major-axis vector.
        axis: [f64; 2],
        /// The minor radius.
        minor: f64,
        /// Its start, a point.
        start: ItemId,
        /// Its end, a point.
        end: ItemId,
        /// The parameter of its start, on the arc; it may come after the arc.
        from: ItemId,
        /// The parameter of its end, on the arc; it may come after the arc.
        to: ItemId,
    },
    /// A spline: its control points are points; degree, knots and weights stay fixed in a solve.
    Spline {
        /// The degree.
        degree: u32,
        /// Its control points.
        points: Vec<ItemId>,
        /// The knots.
        knots: Vec<f64>,
        /// The weights, one for each point.
        weights: Vec<f64>,
    },
    /// A parameter on a curve: t, scaled by the curve's speed.
    Parameter {
        /// The curve.
        curve: ItemId,
        /// t.
        t: f64,
        /// |C′(t)| at the start of the solve.
        speed: f64,
    },
    /// A named value several constraints share.
    Scalar {
        /// The value.
        value: f64,
        /// What one unit of it measures in steps: 1 for a length, the block's size for an angle.
        size: f64,
    },
    /// A rigid group's translation and small rotation.
    Placement {
        /// The translation.
        offset: [f64; 2],
        /// The rotation.
        turn: f64,
        /// The group's size, which scales the rotation.
        size: f64,
    },
    /// A point of model geometry brought onto the plane: it never moves, and has no variables. A
    /// line or a circle on such points is the reference geometry a constraint can name.
    Reference {
        /// x.
        x: f64,
        /// y.
        y: f64,
    },
}

/// One entity of a sketch.
#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    /// Its name.
    pub id: ItemId,
    /// What it is.
    pub kind: EntityKind,
    /// Whether it takes part in constraints but never enters a profile.
    pub construction: bool,
}

/// The side of a directed line a point lies on: a root choice, stored until someone changes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Left of the line's direction.
    Left,
    /// Right of the line's direction.
    Right,
}

impl Side {
    /// +1 for left and −1 for right.
    #[must_use]
    pub const fn sign(self) -> f64 {
        match self {
            Side::Left => 1.0,
            Side::Right => -1.0,
        }
    }
}

/// Which end of a spline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplineEnd {
    /// Its first control point.
    Start,
    /// Its last control point.
    Finish,
}

/// How smooth a join of two splines is (section 15.3): each order holds the ones before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Smoothness {
    /// G1: the tangent lines are one line, the curves leaving the join in opposite directions.
    Tangent,
    /// G2: and the curvature agrees, on the side the curves bend to.
    Curvature,
    /// G3: and the rate of change of curvature along the curve agrees.
    CurvatureRate,
}

impl Smoothness {
    /// The derivative the order reaches: 1, 2 or 3.
    #[must_use]
    pub const fn level(self) -> usize {
        match self {
            Smoothness::Tangent => 1,
            Smoothness::Curvature => 2,
            Smoothness::CurvatureRate => 3,
        }
    }
}

/// A value a dimension takes: a number, or a named scalar several constraints share.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Amount {
    /// This value.
    Value(f64),
    /// The value of this scalar.
    Named(ItemId),
}

/// What section 15.10's table says a relation becomes at commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitClass {
    /// Exact, by the stored form: one point, one carrier, a grid coordinate.
    Exact,
    /// Its residual is evaluated on the stored geometry, and its bound reported.
    Bounded,
}

/// What a constraint says. Its residual is in `crate::equation`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConstraintKind {
    /// Two points are one.
    Coincident {
        /// The first point.
        a: ItemId,
        /// The second point.
        b: ItemId,
    },
    /// Two circles or arcs have one centre.
    Concentric {
        /// The first.
        a: ItemId,
        /// The second.
        b: ItemId,
    },
    /// A point is on a line.
    OnLine {
        /// The point.
        point: ItemId,
        /// The line.
        line: ItemId,
    },
    /// A point is on a circle, or an arc's circle.
    OnCircle {
        /// The point.
        point: ItemId,
        /// The circle.
        circle: ItemId,
    },
    /// A point is the middle of two others.
    Midpoint {
        /// The point.
        point: ItemId,
        /// One end.
        a: ItemId,
        /// The other end.
        b: ItemId,
    },
    /// Two lines are on one line.
    Collinear {
        /// The first.
        a: ItemId,
        /// The second.
        b: ItemId,
    },
    /// A line's two points have one y.
    Horizontal {
        /// The line.
        line: ItemId,
    },
    /// A line's two points have one x.
    Vertical {
        /// The line.
        line: ItemId,
    },
    /// Two lines have one direction.
    Parallel {
        /// The first.
        a: ItemId,
        /// The second.
        b: ItemId,
    },
    /// Two lines are at right angles.
    Perpendicular {
        /// The first.
        a: ItemId,
        /// The second.
        b: ItemId,
    },
    /// From line a's direction to line b's is this angle, in half-turns: its sign is the sense
    /// stored.
    Turned {
        /// The first.
        a: ItemId,
        /// The second.
        b: ItemId,
        /// The angle.
        angle: Amount,
    },
    /// A line points this far from the frame's x axis, in half-turns.
    Heading {
        /// The line.
        line: ItemId,
        /// The angle.
        angle: Amount,
    },
    /// Two points are this far apart.
    Distance {
        /// The first point.
        a: ItemId,
        /// The second point.
        b: ItemId,
        /// The distance, above zero.
        distance: Amount,
    },
    /// A point is this far from a line, on a stored side.
    FromLine {
        /// The point.
        point: ItemId,
        /// The line.
        line: ItemId,
        /// The distance, above zero.
        distance: Amount,
        /// Which side.
        side: Side,
    },
    /// A point is this far from a circle, on a stored side.
    FromCircle {
        /// The point.
        point: ItemId,
        /// The circle.
        circle: ItemId,
        /// The distance, above zero.
        distance: Amount,
        /// Whether it is outside the circle.
        outside: bool,
    },
    /// Two parallel lines are this far apart, the second on a stored side of the first.
    LinesApart {
        /// The first.
        a: ItemId,
        /// The second.
        b: ItemId,
        /// The distance, above zero.
        distance: Amount,
        /// Which side.
        side: Side,
    },
    /// The second point's x is this much more than the first's.
    Across {
        /// The first point.
        a: ItemId,
        /// The second point.
        b: ItemId,
        /// The distance, with its sign.
        distance: Amount,
    },
    /// The second point's y is this much more than the first's.
    Upward {
        /// The first point.
        a: ItemId,
        /// The second point.
        b: ItemId,
        /// The distance, with its sign.
        distance: Amount,
    },
    /// A line has this length.
    Length {
        /// The line.
        line: ItemId,
        /// The length, above zero.
        length: Amount,
    },
    /// Two lines have one length.
    EqualLength {
        /// The first.
        a: ItemId,
        /// The second.
        b: ItemId,
    },
    /// A circle or arc has this radius.
    Radius {
        /// The circle.
        circle: ItemId,
        /// The radius, above zero.
        radius: Amount,
    },
    /// A circle or arc has this diameter.
    Diameter {
        /// The circle.
        circle: ItemId,
        /// The diameter, above zero.
        diameter: Amount,
    },
    /// Two circles or arcs have one radius.
    EqualRadius {
        /// The first.
        a: ItemId,
        /// The second.
        b: ItemId,
    },
    /// The second's radius is the first's, more or less by this: an offset chain's arcs.
    Offset {
        /// The first.
        from: ItemId,
        /// The second.
        to: ItemId,
        /// The difference.
        by: Amount,
        /// Whether the second is the larger.
        grows: bool,
    },
    /// An arc sweeps this far from its start to its end, in half-turns.
    Sweep {
        /// The arc.
        arc: ItemId,
        /// The angle.
        angle: Amount,
    },
    /// An arc has this length.
    ArcLength {
        /// The arc.
        arc: ItemId,
        /// The length, above zero.
        length: Amount,
    },
    /// A line touches a circle, on a stored side of the line.
    TangentLine {
        /// The line.
        line: ItemId,
        /// The circle.
        circle: ItemId,
        /// The side of the line the circle's centre is on.
        side: Side,
    },
    /// Two circles touch, the first outside or inside the second.
    TangentCircles {
        /// The first.
        a: ItemId,
        /// The second.
        b: ItemId,
        /// Whether they touch from outside.
        outside: bool,
    },
    /// A line and an arc that share a point are tangent there.
    TangentAtEnd {
        /// The line.
        line: ItemId,
        /// The arc.
        arc: ItemId,
        /// The shared point.
        point: ItemId,
    },
    /// Two arcs that share a point are tangent there.
    ArcsTangentAtEnd {
        /// The first.
        a: ItemId,
        /// The second.
        b: ItemId,
        /// The shared point.
        point: ItemId,
    },
    /// Two splines that share an end point join with the stated smoothness there (section 15.3):
    /// tangent (G1), equal curvature (G2) or equal rate of change of curvature (G3). The shared
    /// point is a coincidence of the two end control points, stated apart.
    SplineJoin {
        /// The first spline.
        a: ItemId,
        /// Its end at the join.
        a_end: SplineEnd,
        /// The second spline.
        b: ItemId,
        /// Its end at the join.
        b_end: SplineEnd,
        /// How smooth.
        order: Smoothness,
    },
    /// A spline leaves its end along a line (G1 with the line).
    SplineTangentLine {
        /// The spline.
        spline: ItemId,
        /// Its end at the line.
        end: SplineEnd,
        /// The line.
        line: ItemId,
    },
    /// A spline end is smooth with a line or an arc, as far as `order` says (section 15.3): G1 is
    /// the tangent, G2 adds the curvature the other curve has there (zero for a line, one over the
    /// radius for an arc, on the side the arc bends to) and G3 its rate of change (zero for both).
    /// The shared point is a coincidence of the spline's end control point and the other curve's
    /// end, stated apart.
    SplineAgainst {
        /// The spline.
        spline: ItemId,
        /// Its end at the join.
        end: SplineEnd,
        /// The line or the arc.
        other: ItemId,
        /// Which end of an arc the join is at ([`SplineEnd::Start`] is the arc's start point);
        /// a line is the same at both ends, and this is not read.
        other_end: SplineEnd,
        /// How smooth.
        order: Smoothness,
    },
    /// A spline has this arc length, by fixed-node Gauss–Legendre quadrature (section 15.3).
    SplineLength {
        /// The spline.
        spline: ItemId,
        /// The length, above zero.
        length: Amount,
    },
    /// A point is on a spline at the stated parameter of it: the pinned curve point is a fixed
    /// linear function of the control points. A fit-point spline is a spline with one of these for
    /// each of its fit points (section 15.2, [`crate::fit`]).
    SplineThrough {
        /// The spline.
        spline: ItemId,
        /// The point.
        point: ItemId,
        /// The parameter, inside the spline's domain.
        at: f64,
    },
    /// A point is on an ellipse (piece G219).
    OnEllipse {
        /// The point.
        point: ItemId,
        /// The ellipse.
        ellipse: ItemId,
    },
    /// A point is where a parameter on an ellipse puts it: c + cos πt · a + sin πt · (b / |a|) · a⊥,
    /// for the ellipse's centre c, major-axis vector a and minor radius b (piece G219).
    AtParameter {
        /// The point.
        point: ItemId,
        /// The parameter, on an ellipse.
        parameter: ItemId,
    },
    /// A point is `distance` from an ellipse, along the ellipse's normal at a parameter, its foot
    /// (piece G219). Which foot, of an ellipse's up to four, is the parameter's start: the stored root
    /// choice of section 15.3.
    FromEllipse {
        /// The point.
        point: ItemId,
        /// The foot, a parameter on the ellipse.
        parameter: ItemId,
        /// The distance, above zero.
        distance: Amount,
    },
    /// An elliptical arc's length, from its start's parameter to its end's (piece G219).
    EllipticalArcLength {
        /// The elliptical arc.
        arc: ItemId,
        /// The length, above zero.
        length: Amount,
    },
    /// An ellipse's major radius, the length of its major-axis vector (piece G219).
    MajorRadius {
        /// The ellipse.
        ellipse: ItemId,
        /// The radius, above zero.
        radius: Amount,
    },
    /// An ellipse's minor radius (piece G219).
    MinorRadius {
        /// The ellipse.
        ellipse: ItemId,
        /// The radius, above zero.
        radius: Amount,
    },
    /// A line touches an ellipse, on a stored side of the line (piece G219).
    TangentEllipse {
        /// The line.
        line: ItemId,
        /// The ellipse.
        ellipse: ItemId,
        /// The side of the line the ellipse's centre is on.
        side: Side,
    },
    /// Two curves (each a line, a circle or arc, or an ellipse or elliptical arc) are tangent at a
    /// point on both, which other rules put there (piece G219). Tangency anywhere is this at a
    /// construction point.
    TangentAtPoint {
        /// The first curve.
        a: ItemId,
        /// The second curve.
        b: ItemId,
        /// The point they touch at.
        point: ItemId,
    },
    /// Two points are mirror images in a line.
    Symmetric {
        /// The first.
        a: ItemId,
        /// The second.
        b: ItemId,
        /// The line.
        line: ItemId,
    },
    /// Two points are mirror images through a third.
    SymmetricAbout {
        /// The centre.
        point: ItemId,
        /// The first.
        a: ItemId,
        /// The second.
        b: ItemId,
    },
    /// Three points are equally spaced along a line through them.
    Spaced {
        /// The first.
        a: ItemId,
        /// The middle one.
        b: ItemId,
        /// The third.
        c: ItemId,
    },
    /// The step from a to b is the step from c to d.
    LinearPattern {
        /// The first of the first step.
        a: ItemId,
        /// The last of the first step.
        b: ItemId,
        /// The first of the second step.
        c: ItemId,
        /// The last of the second step.
        d: ItemId,
    },
    /// b is a turned about a centre by this many half-turns.
    CircularPattern {
        /// The centre.
        centre: ItemId,
        /// The point turned.
        a: ItemId,
        /// Where it goes.
        b: ItemId,
        /// The angle.
        angle: Amount,
    },
    /// A variable of an entity has this value.
    Fix {
        /// The entity.
        entity: ItemId,
        /// Which of its variables.
        slot: u8,
        /// The value.
        value: f64,
    },
    /// Scalars with a = ratio·b + plus.
    Linear {
        /// a.
        a: ItemId,
        /// b.
        b: ItemId,
        /// The ratio.
        ratio: f64,
        /// The constant.
        plus: f64,
    },
    /// Scalars with a = b·c.
    Product {
        /// a.
        a: ItemId,
        /// b.
        b: ItemId,
        /// c.
        c: ItemId,
    },
}

impl ConstraintKind {
    /// Whether it is a dimension: a value someone may measure instead of drive.
    #[must_use]
    pub const fn is_dimension(&self) -> bool {
        matches!(
            self,
            ConstraintKind::Distance { .. }
                | ConstraintKind::FromLine { .. }
                | ConstraintKind::FromCircle { .. }
                | ConstraintKind::LinesApart { .. }
                | ConstraintKind::Across { .. }
                | ConstraintKind::Upward { .. }
                | ConstraintKind::Length { .. }
                | ConstraintKind::Radius { .. }
                | ConstraintKind::Diameter { .. }
                | ConstraintKind::Turned { .. }
                | ConstraintKind::Heading { .. }
                | ConstraintKind::Sweep { .. }
                | ConstraintKind::ArcLength { .. }
                | ConstraintKind::SplineLength { .. }
                | ConstraintKind::MajorRadius { .. }
                | ConstraintKind::MinorRadius { .. }
                | ConstraintKind::FromEllipse { .. }
                | ConstraintKind::EllipticalArcLength { .. }
        )
    }

    /// The value a dimension is stated with, when it is a number and not another item's: what its
    /// author asked for, as against what the drawing measures.
    #[must_use]
    pub const fn stated(&self) -> Option<f64> {
        let amount = match self {
            ConstraintKind::Turned { angle, .. }
            | ConstraintKind::Heading { angle, .. }
            | ConstraintKind::Sweep { angle, .. } => *angle,
            ConstraintKind::Distance { distance, .. }
            | ConstraintKind::FromLine { distance, .. }
            | ConstraintKind::FromCircle { distance, .. }
            | ConstraintKind::LinesApart { distance, .. }
            | ConstraintKind::Across { distance, .. }
            | ConstraintKind::Upward { distance, .. } => *distance,
            ConstraintKind::Length { length, .. } | ConstraintKind::ArcLength { length, .. } => {
                *length
            }
            ConstraintKind::Radius { radius, .. } => *radius,
            ConstraintKind::Diameter { diameter, .. } => *diameter,
            _ => return None,
        };
        match amount {
            Amount::Value(x) => Some(x),
            Amount::Named(_) => None,
        }
    }

    /// This dimension stated with the number `value` in place of its amount, whatever it was; `None`
    /// for a constraint that is not a dimension.
    #[must_use]
    pub fn with_stated(&self, value: f64) -> Option<ConstraintKind> {
        let mut kind = *self;
        let amount = match &mut kind {
            ConstraintKind::Turned { angle, .. }
            | ConstraintKind::Heading { angle, .. }
            | ConstraintKind::Sweep { angle, .. } => angle,
            ConstraintKind::Distance { distance, .. }
            | ConstraintKind::FromLine { distance, .. }
            | ConstraintKind::FromCircle { distance, .. }
            | ConstraintKind::LinesApart { distance, .. }
            | ConstraintKind::Across { distance, .. }
            | ConstraintKind::Upward { distance, .. } => distance,
            ConstraintKind::Length { length, .. } | ConstraintKind::ArcLength { length, .. } => {
                length
            }
            ConstraintKind::Radius { radius, .. } => radius,
            ConstraintKind::Diameter { diameter, .. } => diameter,
            _ => return None,
        };
        *amount = Amount::Value(value);
        Some(kind)
    }

    /// What it becomes at commit, by section 15.10's table. An angle counts as exact when it is a
    /// number and a multiple of a quarter turn. Symmetry about a line is exact only about a
    /// horizontal or vertical one, a radius only on a full circle, and a point on a circle only if
    /// it is a defining point: all three need the sketch, and are the commit plan's (G225), so they
    /// are bounded here.
    #[must_use]
    pub fn commit(&self) -> CommitClass {
        let square = |amount: &Amount| match *amount {
            Amount::Value(x) => whole(x * 2.0).is_some(),
            Amount::Named(_) => false,
        };
        let exact = match self {
            ConstraintKind::Coincident { .. }
            | ConstraintKind::Concentric { .. }
            | ConstraintKind::OnLine { .. }
            | ConstraintKind::Collinear { .. }
            | ConstraintKind::Horizontal { .. }
            | ConstraintKind::Vertical { .. }
            | ConstraintKind::Parallel { .. }
            | ConstraintKind::Perpendicular { .. }
            | ConstraintKind::Across { .. }
            | ConstraintKind::Upward { .. }
            | ConstraintKind::EqualRadius { .. }
            | ConstraintKind::TangentAtEnd { .. }
            | ConstraintKind::Fix { .. } => true,
            ConstraintKind::Turned { angle, .. } | ConstraintKind::Heading { angle, .. } => {
                square(angle)
            }
            _ => false,
        };
        if exact {
            CommitClass::Exact
        } else {
            CommitClass::Bounded
        }
    }
}

/// Whether a constraint binds the solver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// The solver must meet it.
    Driving,
    /// It is measured and reported, and never solved.
    Reference,
}

/// One constraint of a sketch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Constraint {
    /// Its name.
    pub id: ItemId,
    /// What it says.
    pub kind: ConstraintKind,
    /// Whether it binds the solver.
    pub standing: Standing,
    /// Lower goes first as a pivot, and last as a suggestion to remove. It never leaves a constraint
    /// out of a solve.
    pub priority: u8,
}

/// One unknown of a sketch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Variable {
    /// The entity it belongs to.
    pub owner: ItemId,
    /// Which of the entity's variables it is.
    pub slot: u8,
    /// Its value.
    pub value: f64,
    /// A power of two: the variable the solver sees is value / scale, which acts as a length.
    pub scale: f64,
}

/// The power of two at or below `x`, for a positive finite `x`: a scale whose division rounds
/// nothing (section 15.4). A subnormal gives the smallest normal power.
#[must_use]
pub fn power_of_two(x: f64) -> f64 {
    let power = f64::from_bits(x.to_bits() & 0x7FF0_0000_0000_0000);
    if power > 0.0 {
        power
    } else {
        f64::MIN_POSITIVE
    }
}

/// The whole number `x` is, if it is one: the value counts only if it converts back to its own bits.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "the value counts only if it converts back to its own bits"
)]
fn whole(x: f64) -> Option<i64> {
    let n = x as i64;
    ((n as f64).to_bits() == x.to_bits()).then_some(n)
}

/// A step's square, added to `base`: a closure, for `and_then`.
fn reach_of(base: i128) -> impl Fn(i64) -> Option<i128> {
    move |r| {
        i128::from(r)
            .checked_mul(i128::from(r))
            .and_then(|square| square.checked_add(base))
    }
}

/// |pq|², if both are grid points.
fn grid_square(p: [f64; 2], q: [f64; 2]) -> Option<i128> {
    let part = |a: f64, b: f64| {
        let along = i128::from(whole(a)?).checked_sub(i128::from(whole(b)?))?;
        along.checked_mul(along)
    };
    part(p[0], q[0])?.checked_add(part(p[1], q[1])?)
}

/// `x`, or [`SketchError::NotFinite`].
pub(crate) fn finite(x: f64) -> Result<f64, SketchError> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(SketchError::NotFinite)
    }
}

/// `x`, or the reason it is not a positive finite value.
pub(crate) fn positive(x: f64) -> Result<f64, SketchError> {
    if finite(x)? > 0.0 {
        Ok(x)
    } else {
        Err(SketchError::NotPositive)
    }
}

fn push<T>(list: &mut Vec<T>, value: T) -> Result<(), SketchError> {
    list.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
    list.push(value);
    Ok(())
}

/// What a spline's constraints read of it: see [`Sketch::spline_parts`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct SplineParts<'a> {
    /// The degree.
    pub degree: usize,
    /// The control points.
    pub points: &'a [ItemId],
    /// The knots.
    pub knots: &'a [f64],
    /// The weights, or none for all ones.
    pub weights: &'a [f64],
}

/// A sketch: a plane and its items, older first.
#[derive(Clone, Debug, PartialEq)]
pub struct Sketch {
    plane: SketchPlane,
    entities: Vec<Entity>,
    constraints: Vec<Constraint>,
}

impl EntityKind {
    /// Each variable's value and scale, in slot order.
    fn variables(&self, out: &mut Vec<(f64, f64)>) -> Result<(), SketchError> {
        match self {
            EntityKind::Point { x, y } => {
                push(out, (*x, 1.0))?;
                push(out, (*y, 1.0))
            }
            EntityKind::Circle { radius, .. } | EntityKind::Arc { radius, .. } => {
                push(out, (*radius, 1.0))
            }
            EntityKind::Ellipse { axis, minor, .. }
            | EntityKind::EllipticalArc { axis, minor, .. } => {
                push(out, (axis[0], 1.0))?;
                push(out, (axis[1], 1.0))?;
                push(out, (*minor, 1.0))
            }
            EntityKind::Parameter { t, speed, .. } => push(out, (*t, power_of_two(1.0 / *speed))),
            EntityKind::Scalar { value, size } => push(out, (*value, power_of_two(1.0 / *size))),
            EntityKind::Placement { offset, turn, size } => {
                push(out, (offset[0], 1.0))?;
                push(out, (offset[1], 1.0))?;
                push(out, (*turn, power_of_two(1.0 / *size)))
            }
            EntityKind::Line { .. } | EntityKind::Spline { .. } | EntityKind::Reference { .. } => {
                Ok(())
            }
        }
    }

    /// The ids it names, and whether each must be a point.
    fn check(&self, sketch: &Sketch) -> Result<(), SketchError> {
        let point = |id: ItemId| sketch.point(id).map(|_| ());
        match self {
            EntityKind::Point { x, y } | EntityKind::Reference { x, y } => {
                finite(*x)?;
                finite(*y)?;
                Ok(())
            }
            EntityKind::Line { start, end, .. } => {
                point(*start)?;
                point(*end)
            }
            EntityKind::Circle { centre, radius } => {
                positive(*radius)?;
                point(*centre)
            }
            EntityKind::Arc {
                centre,
                radius,
                start,
                end,
            } => {
                positive(*radius)?;
                point(*centre)?;
                point(*start)?;
                point(*end)
            }
            EntityKind::Ellipse {
                centre,
                axis,
                minor,
            } => {
                finite(axis[0])?;
                finite(axis[1])?;
                positive(*minor)?;
                point(*centre)
            }
            // Its parameters name it as their curve, so they come after it: the assembly reads them
            // when it lays out the rules of its ends.
            EntityKind::EllipticalArc {
                centre,
                axis,
                minor,
                start,
                end,
                ..
            } => {
                finite(axis[0])?;
                finite(axis[1])?;
                positive(*minor)?;
                point(*centre)?;
                point(*start)?;
                point(*end)
            }
            EntityKind::Spline {
                points,
                knots,
                weights,
                ..
            } => {
                for id in points {
                    point(*id)?;
                }
                for x in knots {
                    finite(*x)?;
                }
                for w in weights {
                    positive(*w)?;
                }
                Ok(())
            }
            EntityKind::Parameter { curve, t, speed } => {
                finite(*t)?;
                positive(1.0 / positive(*speed)?)?;
                sketch.entity(*curve).map(|_| ())
            }
            EntityKind::Scalar { value, size } => {
                finite(*value)?;
                positive(1.0 / positive(*size)?)?;
                Ok(())
            }
            EntityKind::Placement { offset, turn, size } => {
                finite(offset[0])?;
                finite(offset[1])?;
                finite(*turn)?;
                positive(1.0 / positive(*size)?)?;
                Ok(())
            }
        }
    }
}

impl Sketch {
    /// An empty sketch on `plane`.
    #[must_use]
    pub const fn new(plane: SketchPlane) -> Sketch {
        Sketch {
            plane,
            entities: Vec::new(),
            constraints: Vec::new(),
        }
    }

    /// Its plane.
    #[must_use]
    pub const fn plane(&self) -> &SketchPlane {
        &self.plane
    }

    /// Its entities, older first.
    #[must_use]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Its constraints, older first.
    #[must_use]
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    /// The entity `id`.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`] if there is none.
    pub fn entity(&self, id: ItemId) -> Result<&Entity, SketchError> {
        self.entities
            .binary_search_by_key(&id, |e| e.id)
            .ok()
            .and_then(|place| self.entities.get(place))
            .ok_or(SketchError::Unknown(id))
    }

    /// The point `id`'s coordinates, whether it moves or is a reference.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::WrongKind`] if it is not a point.
    pub fn point(&self, id: ItemId) -> Result<[f64; 2], SketchError> {
        match self.entity(id)?.kind {
            EntityKind::Point { x, y } | EntityKind::Reference { x, y } => Ok([x, y]),
            _ => Err(SketchError::WrongKind(id)),
        }
    }

    /// The line `id`'s two points.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::WrongKind`] if it is not a line.
    pub fn line(&self, id: ItemId) -> Result<[ItemId; 2], SketchError> {
        let EntityKind::Line { start, end, .. } = self.entity(id)?.kind else {
            return Err(SketchError::WrongKind(id));
        };
        Ok([start, end])
    }

    /// The centre of the circle or arc `id`.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::WrongKind`] if it is neither.
    pub fn centre(&self, id: ItemId) -> Result<ItemId, SketchError> {
        match self.entity(id)?.kind {
            EntityKind::Circle { centre, .. } | EntityKind::Arc { centre, .. } => Ok(centre),
            _ => Err(SketchError::WrongKind(id)),
        }
    }

    /// The radius of the circle or arc `id`.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::WrongKind`] if it is neither.
    pub fn radius(&self, id: ItemId) -> Result<f64, SketchError> {
        match self.entity(id)?.kind {
            EntityKind::Circle { radius, .. } | EntityKind::Arc { radius, .. } => Ok(radius),
            _ => Err(SketchError::WrongKind(id)),
        }
    }

    /// The ellipse or elliptical arc `id`'s centre, major-axis vector and minor radius.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::WrongKind`] if it is neither.
    pub fn ellipse(&self, id: ItemId) -> Result<(ItemId, [f64; 2], f64), SketchError> {
        match self.entity(id)?.kind {
            EntityKind::Ellipse {
                centre,
                axis,
                minor,
            }
            | EntityKind::EllipticalArc {
                centre,
                axis,
                minor,
                ..
            } => Ok((centre, axis, minor)),
            _ => Err(SketchError::WrongKind(id)),
        }
    }

    /// The elliptical arc `id`'s parameters, of its start and of its end.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::WrongKind`] if it is not an elliptical arc.
    pub fn elliptical_arc(&self, id: ItemId) -> Result<[ItemId; 2], SketchError> {
        match self.entity(id)?.kind {
            EntityKind::EllipticalArc { from, to, .. } => Ok([from, to]),
            _ => Err(SketchError::WrongKind(id)),
        }
    }

    /// The ellipse the parameter `id` is on.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::WrongKind`] if it is not a parameter, or its curve
    /// is not an ellipse.
    pub fn parameter_ellipse(&self, id: ItemId) -> Result<ItemId, SketchError> {
        match self.entity(id)?.kind {
            EntityKind::Parameter { curve, .. } => {
                self.ellipse(curve)
                    .map_err(|_| SketchError::WrongKind(id))?;
                Ok(curve)
            }
            _ => Err(SketchError::WrongKind(id)),
        }
    }

    /// The arc `id`'s centre, start and end, swept anticlockwise from its start.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::WrongKind`] if it is not an arc.
    pub fn arc(&self, id: ItemId) -> Result<[ItemId; 3], SketchError> {
        let EntityKind::Arc {
            centre, start, end, ..
        } = self.entity(id)?.kind
        else {
            return Err(SketchError::WrongKind(id));
        };
        Ok([centre, start, end])
    }

    /// The spline `id`'s degree, control points, knots and weights, when it is one whose ends have
    /// `level` derivatives the constraints can read: clamped at both ends (the first and last
    /// `degree` + 1 knots equal), of degree at least `level`, with more than `degree` control
    /// points and the knot count a spline of that degree and size has. Weights may be any positive
    /// numbers; none stored means all ones.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::WrongKind`] if it is not such a spline.
    pub(crate) fn spline_parts(
        &self,
        id: ItemId,
        level: usize,
    ) -> Result<SplineParts<'_>, SketchError> {
        let EntityKind::Spline {
            degree,
            points,
            knots,
            weights,
        } = &self.entity(id)?.kind
        else {
            return Err(SketchError::WrongKind(id));
        };
        let p = usize::try_from(*degree).map_err(|_| SketchError::WrongKind(id))?;
        let n = points.len();
        let clamped = |ks: &[f64]| ks.windows(2).all(|w| w.first() == w.get(1));
        let at_start = knots.get(..=p).is_some_and(clamped);
        let at_end = knots
            .len()
            .checked_sub(p.saturating_add(1))
            .and_then(|from| knots.get(from..))
            .is_some_and(clamped);
        let counted = n.checked_add(p).and_then(|k| k.checked_add(1)) == Some(knots.len());
        let weighed = weights.is_empty() || weights.len() == n;
        // A rational curve has derivatives of every order; a polynomial one has none above its
        // degree, which the constraints do not read.
        let rational = weights.iter().any(|w| *w != 1.0);
        let enough = (p >= level || rational) && n > p;
        if at_start && at_end && counted && weighed && enough {
            Ok(SplineParts {
                degree: p,
                points,
                knots,
                weights,
            })
        } else {
            Err(SketchError::WrongKind(id))
        }
    }

    /// The spline `id`'s degree, control points and knots, when it is one whose ends have `level`
    /// derivatives the constraints can read and it is not rational (every weight one): see
    /// [`Sketch::spline_parts`].
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::WrongKind`] if it is not such a spline.
    pub fn spline_jets(
        &self,
        id: ItemId,
        level: usize,
    ) -> Result<(usize, &[ItemId], &[f64]), SketchError> {
        let parts = self.spline_parts(id, level)?;
        if parts.weights.iter().all(|w| *w == 1.0) {
            Ok((parts.degree, parts.points, parts.knots))
        } else {
            Err(SketchError::WrongKind(id))
        }
    }

    /// Whether the spline `id` has a weight other than one.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::WrongKind`] if it is not a spline.
    pub fn spline_is_rational(&self, id: ItemId) -> Result<bool, SketchError> {
        match &self.entity(id)?.kind {
            EntityKind::Spline { weights, .. } => Ok(weights.iter().any(|w| *w != 1.0)),
            _ => Err(SketchError::WrongKind(id)),
        }
    }

    /// The spline `id` as a `geom-nurbs` curve at the sketch's values: its basis, its control
    /// points as they stand and its weights.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::WrongKind`] if it is not a spline or its degree,
    /// knots or weights are refused by `geom-nurbs`.
    pub fn spline_curve(&self, id: ItemId) -> Result<geom_nurbs::Curve<2>, SketchError> {
        let EntityKind::Spline {
            degree,
            points,
            knots,
            weights,
        } = &self.entity(id)?.kind
        else {
            return Err(SketchError::WrongKind(id));
        };
        let wrong = |_| SketchError::WrongKind(id);
        let p = usize::try_from(*degree).map_err(|_| SketchError::WrongKind(id))?;
        let basis = geom_nurbs::Basis::new(p, knots.clone()).map_err(wrong)?;
        let mut coords: Vec<[f64; 2]> = Vec::new();
        coords
            .try_reserve_exact(points.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        for point in points {
            coords.push(self.point(*point)?);
        }
        if weights.iter().all(|w| *w == 1.0) {
            geom_nurbs::Curve::new(basis, &coords).map_err(wrong)
        } else {
            geom_nurbs::Curve::new_rational(basis, &coords, weights).map_err(wrong)
        }
    }

    /// Adds `entity`, which may name only entities already here.
    ///
    /// # Errors
    ///
    /// [`SketchError::Duplicate`]; what [`Sketch::entity`] and [`Sketch::point`] refuse;
    /// [`SketchError::NotFinite`]; [`SketchError::NotPositive`]; [`SketchError::OutOfMemory`].
    pub fn add_entity(&mut self, entity: Entity) -> Result<(), SketchError> {
        entity.kind.check(self)?;
        let Err(place) = self.entities.binary_search_by_key(&entity.id, |e| e.id) else {
            return Err(SketchError::Duplicate(entity.id));
        };
        self.entities
            .try_reserve(1)
            .map_err(|_| SketchError::OutOfMemory)?;
        self.entities.insert(place, entity);
        Ok(())
    }

    /// Adds `constraint`.
    ///
    /// # Errors
    ///
    /// [`SketchError::Duplicate`]; [`SketchError::NotADimension`] for a reference that is not a
    /// dimension; what its names and values refuse, as [`Sketch::add_entity`].
    pub fn add_constraint(&mut self, constraint: Constraint) -> Result<(), SketchError> {
        self.check_constraint(&constraint)?;
        let Err(place) = self
            .constraints
            .binary_search_by_key(&constraint.id, |c| c.id)
        else {
            return Err(SketchError::Duplicate(constraint.id));
        };
        self.constraints
            .try_reserve(1)
            .map_err(|_| SketchError::OutOfMemory)?;
        self.constraints.insert(place, constraint);
        Ok(())
    }

    fn check_amount(&self, amount: Amount, above_zero: bool) -> Result<(), SketchError> {
        match amount {
            Amount::Value(x) if above_zero => positive(x).map(|_| ()),
            Amount::Value(x) => finite(x).map(|_| ()),
            Amount::Named(id) => self.scalar(id),
        }
    }

    fn scalar(&self, id: ItemId) -> Result<(), SketchError> {
        match self.entity(id)?.kind {
            EntityKind::Scalar { .. } => Ok(()),
            _ => Err(SketchError::WrongKind(id)),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one arm for each kind, naming what it holds"
    )]
    fn check_constraint(&self, constraint: &Constraint) -> Result<(), SketchError> {
        if constraint.standing == Standing::Reference && !constraint.kind.is_dimension() {
            return Err(SketchError::NotADimension(constraint.id));
        }
        let point = |id: ItemId| self.point(id).map(|_| ());
        let line = |id: ItemId| self.line(id).map(|_| ());
        let round = |id: ItemId| self.centre(id).map(|_| ());
        let arc = |id: ItemId| self.arc(id).map(|_| ());
        match constraint.kind {
            ConstraintKind::Coincident { a, b } => {
                point(a)?;
                point(b)
            }
            ConstraintKind::Concentric { a, b }
            | ConstraintKind::EqualRadius { a, b }
            | ConstraintKind::TangentCircles { a, b, .. } => {
                round(a)?;
                round(b)
            }
            ConstraintKind::OnLine { point: p, line: l } => {
                point(p)?;
                line(l)
            }
            ConstraintKind::OnCircle { point: p, circle } => {
                point(p)?;
                round(circle)
            }
            ConstraintKind::Midpoint { point: p, a, b }
            | ConstraintKind::SymmetricAbout { point: p, a, b }
            | ConstraintKind::Spaced { a: p, b: a, c: b } => {
                point(p)?;
                point(a)?;
                point(b)
            }
            ConstraintKind::Collinear { a, b }
            | ConstraintKind::Parallel { a, b }
            | ConstraintKind::Perpendicular { a, b }
            | ConstraintKind::EqualLength { a, b } => {
                line(a)?;
                line(b)
            }
            ConstraintKind::Horizontal { line: l } | ConstraintKind::Vertical { line: l } => {
                line(l)
            }
            ConstraintKind::Turned { a, b, angle } => {
                self.check_amount(angle, false)?;
                line(a)?;
                line(b)
            }
            ConstraintKind::Heading { line: l, angle } => {
                self.check_amount(angle, false)?;
                line(l)
            }
            ConstraintKind::Distance { a, b, distance } => {
                self.check_amount(distance, true)?;
                point(a)?;
                point(b)
            }
            ConstraintKind::FromLine {
                point: p,
                line: l,
                distance,
                ..
            } => {
                self.check_amount(distance, true)?;
                point(p)?;
                line(l)
            }
            ConstraintKind::FromCircle {
                point: p,
                circle,
                distance,
                ..
            } => {
                self.check_amount(distance, true)?;
                point(p)?;
                round(circle)
            }
            ConstraintKind::LinesApart { a, b, distance, .. } => {
                self.check_amount(distance, true)?;
                line(a)?;
                line(b)
            }
            ConstraintKind::Across { a, b, distance }
            | ConstraintKind::Upward { a, b, distance } => {
                self.check_amount(distance, false)?;
                point(a)?;
                point(b)
            }
            ConstraintKind::Length { line: l, length } => {
                self.check_amount(length, true)?;
                line(l)
            }
            ConstraintKind::Radius { circle, radius } => {
                self.check_amount(radius, true)?;
                round(circle)
            }
            ConstraintKind::Diameter { circle, diameter } => {
                self.check_amount(diameter, true)?;
                round(circle)
            }
            ConstraintKind::Offset { from, to, by, .. } => {
                self.check_amount(by, false)?;
                round(from)?;
                round(to)
            }
            ConstraintKind::Sweep { arc: a, angle } => {
                self.check_amount(angle, false)?;
                arc(a)
            }
            ConstraintKind::ArcLength { arc: a, length } => {
                self.check_amount(length, true)?;
                arc(a)
            }
            ConstraintKind::TangentLine {
                line: l, circle, ..
            } => {
                line(l)?;
                round(circle)
            }
            ConstraintKind::OnEllipse { point: p, ellipse } => {
                point(p)?;
                self.ellipse(ellipse).map(|_| ())
            }
            ConstraintKind::AtParameter {
                point: p,
                parameter,
            } => {
                point(p)?;
                self.parameter_ellipse(parameter).map(|_| ())
            }
            ConstraintKind::FromEllipse {
                point: p,
                parameter,
                distance,
            } => {
                self.check_amount(distance, true)?;
                point(p)?;
                self.parameter_ellipse(parameter).map(|_| ())
            }
            ConstraintKind::EllipticalArcLength { arc, length } => {
                self.check_amount(length, true)?;
                self.elliptical_arc(arc).map(|_| ())
            }
            ConstraintKind::MajorRadius { ellipse, radius }
            | ConstraintKind::MinorRadius { ellipse, radius } => {
                self.check_amount(radius, true)?;
                self.ellipse(ellipse).map(|_| ())
            }
            ConstraintKind::TangentEllipse {
                line: l, ellipse, ..
            } => {
                line(l)?;
                self.ellipse(ellipse).map(|_| ())
            }
            ConstraintKind::TangentAtPoint { a, b, point: p } => {
                point(p)?;
                for curve in [a, b] {
                    match self.entity(curve)?.kind {
                        EntityKind::Line { .. }
                        | EntityKind::Circle { .. }
                        | EntityKind::Arc { .. }
                        | EntityKind::Ellipse { .. }
                        | EntityKind::EllipticalArc { .. } => {}
                        _ => return Err(SketchError::WrongKind(curve)),
                    }
                }
                Ok(())
            }
            ConstraintKind::TangentAtEnd {
                line: l,
                arc: a,
                point: p,
            } => {
                line(l)?;
                arc(a)?;
                point(p)
            }
            ConstraintKind::ArcsTangentAtEnd { a, b, point: p } => {
                arc(a)?;
                arc(b)?;
                point(p)
            }
            ConstraintKind::SplineJoin { a, b, order, .. } => {
                self.spline_parts(a, order.level())?;
                self.spline_parts(b, order.level()).map(|_| ())
            }
            ConstraintKind::SplineTangentLine {
                spline, line: l, ..
            } => {
                self.spline_parts(spline, 1)?;
                line(l)
            }
            ConstraintKind::SplineAgainst {
                spline,
                other,
                order,
                ..
            } => {
                self.spline_parts(spline, order.level())?;
                if self.line(other).is_ok() {
                    Ok(())
                } else {
                    arc(other)
                }
            }
            ConstraintKind::SplineLength { spline, length } => {
                self.check_amount(length, true)?;
                self.spline_curve(spline).map(|_| ())
            }
            ConstraintKind::SplineThrough {
                spline,
                point: p,
                at,
            } => {
                finite(at)?;
                point(p)?;
                let (lo, hi) = self.spline_curve(spline)?.basis().domain();
                if lo <= at && at <= hi {
                    Ok(())
                } else {
                    Err(SketchError::WrongKind(spline))
                }
            }
            ConstraintKind::Symmetric { a, b, line: l } => {
                point(a)?;
                point(b)?;
                line(l)
            }
            ConstraintKind::LinearPattern { a, b, c, d } => {
                point(a)?;
                point(b)?;
                point(c)?;
                point(d)
            }
            ConstraintKind::CircularPattern {
                centre,
                a,
                b,
                angle,
            } => {
                self.check_amount(angle, false)?;
                point(centre)?;
                point(a)?;
                point(b)
            }
            ConstraintKind::Fix {
                entity,
                slot,
                value,
            } => {
                finite(value)?;
                let mut held = Vec::new();
                self.entity(entity)?.kind.variables(&mut held)?;
                if usize::from(slot) < held.len() {
                    Ok(())
                } else {
                    Err(SketchError::WrongKind(entity))
                }
            }
            ConstraintKind::Linear { a, b, ratio, plus } => {
                finite(ratio)?;
                finite(plus)?;
                self.scalar(a)?;
                self.scalar(b)
            }
            ConstraintKind::Product { a, b, c } => {
                self.scalar(a)?;
                self.scalar(b)?;
                self.scalar(c)
            }
        }
    }

    /// The side of line `line` that `point` is drawn on, by the exact predicate: the stored choice
    /// of a [`ConstraintKind::FromLine`], [`ConstraintKind::LinesApart`] or
    /// [`ConstraintKind::TangentLine`] authored from a drawing.
    ///
    /// # Errors
    ///
    /// [`SketchError::RootChoiceUndetermined`] if the point is on the line or any of the three is
    /// off the grid; what [`Sketch::line`] and [`Sketch::point`] refuse.
    pub fn drawn_side(&self, point: ItemId, line: ItemId) -> Result<Side, SketchError> {
        let [start, end] = self.line(line)?;
        let undecided = SketchError::RootChoiceUndetermined(point);
        let grid = |p: [f64; 2]| -> Result<P2, SketchError> {
            match (whole(p[0]), whole(p[1])) {
                (Some(x), Some(y)) => P2::new(x, y).map_err(|_| undecided),
                _ => Err(undecided),
            }
        };
        let (a, b, c) = (
            grid(self.point(start)?)?,
            grid(self.point(end)?)?,
            grid(self.point(point)?)?,
        );
        match orient2d(a, b, c)? {
            Sign::Positive => Ok(Side::Left),
            Sign::Negative => Ok(Side::Right),
            Sign::Zero => Err(undecided),
        }
    }

    /// Whether `point` is drawn outside the circle `circle`, by exact integers: the stored choice of
    /// a [`ConstraintKind::FromCircle`] authored from a drawing.
    ///
    /// # Errors
    ///
    /// [`SketchError::RootChoiceUndetermined`] if the point is on the circle or any of them is off
    /// the grid; what [`Sketch::centre`], [`Sketch::radius`] and [`Sketch::point`] refuse.
    pub fn drawn_outside(&self, point: ItemId, circle: ItemId) -> Result<bool, SketchError> {
        let undecided = SketchError::RootChoiceUndetermined(point);
        let p = self.point(point)?;
        let c = self.point(self.centre(circle)?)?;
        let r = self.radius(circle)?;
        let (Some(square), Some(reach)) = (grid_square(p, c), whole(r).and_then(reach_of(0)))
        else {
            return Err(undecided);
        };
        match square.cmp(&reach) {
            Ordering::Greater => Ok(true),
            Ordering::Less => Ok(false),
            Ordering::Equal => Err(undecided),
        }
    }

    /// Whether the circles `a` and `b` are drawn nearer touching from outside than from inside, by
    /// exact integers: the stored choice of a [`ConstraintKind::TangentCircles`] authored from a
    /// drawing. The two tangent distances squared are (ra ± rb)², so the test is the square of the
    /// distance between the centres against their mean, ra² + rb².
    ///
    /// # Errors
    ///
    /// [`SketchError::RootChoiceUndetermined`] if the two are equally near or any of them is off the
    /// grid; what [`Sketch::centre`], [`Sketch::radius`] and [`Sketch::point`] refuse.
    pub fn drawn_touching_outside(&self, a: ItemId, b: ItemId) -> Result<bool, SketchError> {
        let undecided = SketchError::RootChoiceUndetermined(a);
        let p = self.point(self.centre(a)?)?;
        let q = self.point(self.centre(b)?)?;
        let (ra, rb) = (self.radius(a)?, self.radius(b)?);
        let mean = whole(ra).and_then(reach_of(0)).and_then(|first| {
            whole(rb)
                .and_then(reach_of(0))
                .and_then(|second| first.checked_add(second))
        });
        let (Some(square), Some(mean)) = (grid_square(p, q), mean) else {
            return Err(undecided);
        };
        match square.cmp(&mean) {
            Ordering::Greater => Ok(true),
            Ordering::Less => Ok(false),
            Ordering::Equal => Err(undecided),
        }
    }

    /// This sketch with the entity that has `entity`'s id replaced by `entity`, which must be the same
    /// kind of thing and name only entities already here (a re-interpolated spline's new knots, say).
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`] if nothing has that id; [`SketchError::WrongKind`] for another kind;
    /// what [`Sketch::add_entity`] refuses.
    pub fn with_entity(&self, entity: Entity) -> Result<Sketch, SketchError> {
        let mut sketch = self.clone();
        let place = sketch
            .entities
            .binary_search_by_key(&entity.id, |e| e.id)
            .map_err(|_| SketchError::Unknown(entity.id))?;
        let held = sketch
            .entities
            .get_mut(place)
            .ok_or(SketchError::Internal)?;
        if core::mem::discriminant(&held.kind) != core::mem::discriminant(&entity.kind) {
            return Err(SketchError::WrongKind(entity.id));
        }
        entity.kind.check(self)?;
        *held = entity;
        for e in &sketch.entities {
            e.kind.check(&sketch)?;
        }
        Ok(sketch)
    }

    /// This sketch with the constraint that has `constraint`'s id replaced by `constraint`.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`] if nothing has that id; what [`Sketch::add_constraint`] refuses.
    pub fn with_constraint(&self, constraint: Constraint) -> Result<Sketch, SketchError> {
        let mut sketch = self.clone();
        let place = sketch
            .constraints
            .binary_search_by_key(&constraint.id, |c| c.id)
            .map_err(|_| SketchError::Unknown(constraint.id))?;
        sketch.check_constraint(&constraint)?;
        *sketch
            .constraints
            .get_mut(place)
            .ok_or(SketchError::Internal)? = constraint;
        Ok(sketch)
    }

    /// This sketch with `values`, one for each of [`Sketch::variables`], in that order.
    ///
    /// # Errors
    ///
    /// [`SketchError::Internal`] for too few; what [`Sketch::add_entity`] refuses of a value, such as
    /// a radius that is no longer positive.
    pub fn with_values(&self, values: &[f64]) -> Result<Sketch, SketchError> {
        let mut sketch = self.clone();
        let mut next = values.iter().copied();
        let mut take = |held: &mut f64| -> Result<(), SketchError> {
            *held = next.next().ok_or(SketchError::Internal)?;
            Ok(())
        };
        for entity in &mut sketch.entities {
            match &mut entity.kind {
                EntityKind::Point { x, y } => {
                    take(x)?;
                    take(y)?;
                }
                EntityKind::Circle { radius, .. } | EntityKind::Arc { radius, .. } => take(radius)?,
                EntityKind::Ellipse { axis, minor, .. }
                | EntityKind::EllipticalArc { axis, minor, .. } => {
                    let [first, second] = axis;
                    take(first)?;
                    take(second)?;
                    take(minor)?;
                }
                EntityKind::Parameter { t, .. } => take(t)?,
                EntityKind::Scalar { value, .. } => take(value)?,
                EntityKind::Placement { offset, turn, .. } => {
                    let [first, second] = offset;
                    take(first)?;
                    take(second)?;
                    take(turn)?;
                }
                EntityKind::Line { .. }
                | EntityKind::Spline { .. }
                | EntityKind::Reference { .. } => {}
            }
        }
        for entity in &sketch.entities {
            entity.kind.check(&sketch)?;
        }
        Ok(sketch)
    }

    /// Every variable, in authoring order of its entity and slot order within it.
    ///
    /// # Errors
    ///
    /// [`SketchError::OutOfMemory`].
    pub fn variables(&self) -> Result<Vec<Variable>, SketchError> {
        let mut out = Vec::new();
        let mut held = Vec::new();
        for entity in &self.entities {
            held.clear();
            entity.kind.variables(&mut held)?;
            for (slot, (value, scale)) in (0_u8..).zip(held.iter().copied()) {
                push(
                    &mut out,
                    Variable {
                        owner: entity.id,
                        slot,
                        value,
                        scale,
                    },
                )?;
            }
        }
        Ok(out)
    }
}
