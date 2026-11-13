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
//!
//! Method: section 15.2's table of entities and variables, and sections 15.3 and 15.4
//! (`projects/engines/geometry/architecture/15-sketches.md`).
//!
//! Cost: adding an item costs a binary search and a move of the items after it, nothing when items
//! arrive in order; O(n) memory. No recursion.

extern crate alloc;

use alloc::vec::Vec;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    /// The start is on this constraint's singular set.
    Degenerate(ItemId),
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
            SketchError::Degenerate(_) => "the start is on a singular set",
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
    /// Model geometry brought onto the plane: it never moves, and has no variables.
    Reference,
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

/// What a constraint says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConstraintKind {
    /// Two points are one.
    Coincident {
        /// The first point.
        a: ItemId,
        /// The second point.
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
    /// Two points are this far apart.
    Distance {
        /// The first point.
        a: ItemId,
        /// The second point.
        b: ItemId,
        /// The distance, above zero.
        distance: f64,
    },
    /// A point is this far from a line, on a stored side.
    PointLine {
        /// The point.
        point: ItemId,
        /// The line.
        line: ItemId,
        /// The distance, above zero.
        distance: f64,
        /// Which side.
        side: Side,
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
}

impl ConstraintKind {
    /// Whether it is a dimension: a value someone may measure instead of drive.
    #[must_use]
    pub const fn is_dimension(&self) -> bool {
        matches!(
            self,
            ConstraintKind::Distance { .. } | ConstraintKind::PointLine { .. }
        )
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
            EntityKind::Ellipse { axis, minor, .. } => {
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
            EntityKind::Line { .. } | EntityKind::Spline { .. } | EntityKind::Reference => Ok(()),
        }
    }

    /// The ids it names, and whether each must be a point.
    fn check(&self, sketch: &Sketch) -> Result<(), SketchError> {
        let point = |id: ItemId| sketch.point(id).map(|_| ());
        match self {
            EntityKind::Point { x, y } => {
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
            EntityKind::Reference => Ok(()),
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

    /// The point `id`'s coordinates.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::WrongKind`] if it is not a point.
    pub fn point(&self, id: ItemId) -> Result<[f64; 2], SketchError> {
        let EntityKind::Point { x, y } = self.entity(id)?.kind else {
            return Err(SketchError::WrongKind(id));
        };
        Ok([x, y])
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

    fn check_constraint(&self, constraint: &Constraint) -> Result<(), SketchError> {
        if constraint.standing == Standing::Reference && !constraint.kind.is_dimension() {
            return Err(SketchError::NotADimension(constraint.id));
        }
        match constraint.kind {
            ConstraintKind::Coincident { a, b } => {
                self.point(a)?;
                self.point(b)?;
            }
            ConstraintKind::Horizontal { line } | ConstraintKind::Vertical { line } => {
                self.line(line)?;
            }
            ConstraintKind::Distance { a, b, distance } => {
                positive(distance)?;
                self.point(a)?;
                self.point(b)?;
            }
            ConstraintKind::PointLine {
                point,
                line,
                distance,
                ..
            } => {
                positive(distance)?;
                self.point(point)?;
                self.line(line)?;
            }
            ConstraintKind::Fix {
                entity,
                slot,
                value,
            } => {
                finite(value)?;
                let mut held = Vec::new();
                self.entity(entity)?.kind.variables(&mut held)?;
                if usize::from(slot) >= held.len() {
                    return Err(SketchError::WrongKind(entity));
                }
            }
        }
        Ok(())
    }

    /// The side of line `line` that `point` is drawn on, by the exact predicate: the stored choice
    /// of a [`ConstraintKind::PointLine`] authored from a drawing.
    ///
    /// # Errors
    ///
    /// [`SketchError::RootChoiceUndetermined`] if the point is on the line or any of the three is
    /// off the grid; what [`Sketch::line`] and [`Sketch::point`] refuse.
    pub fn drawn_side(&self, point: ItemId, line: ItemId) -> Result<Side, SketchError> {
        let [start, end] = self.line(line)?;
        let undecided = SketchError::RootChoiceUndetermined(point);
        let grid = |p: [f64; 2]| -> Result<P2, SketchError> {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_precision_loss,
                reason = "the value counts only if it converts back to its own bits"
            )]
            let whole = |x: f64| {
                let n = x as i64;
                ((n as f64).to_bits() == x.to_bits()).then_some(n)
            };
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
