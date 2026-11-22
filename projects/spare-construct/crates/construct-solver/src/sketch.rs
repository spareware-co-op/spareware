//! What a sketch is: points and straight lines on one plane, and the constraints that relate them.
//! Coordinates and lengths are whole millimetres, as every stored length in Spare Construct is
//! (construct-model's FORMAT.md). Everything is kept in authoring order, and that order is the
//! only order the solver ever uses.
//!
//! Lines, arcs and circles share points, so a later curve kind adds an entity list and constraint
//! kinds here and nothing else changes: [`Constraint`] is non-exhaustive for that reason.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The largest coordinate or length a sketch takes, in millimetres: 1,000 km. Every coordinate,
/// difference and product of two differences then fits the solver's numbers exactly.
pub const COORD_LIMIT_MM: i64 = 1_000_000_000;

/// A point, by its place in authoring order.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointId(pub u32);

/// A straight line, by its place in authoring order.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineId(pub u32);

/// A constraint, by its place in authoring order.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstraintId(pub u32);

/// A point on the sketch plane, in whole millimetres.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    /// Along the plane's first axis.
    pub x_mm: i64,
    /// Along the plane's second axis.
    pub y_mm: i64,
}

/// A straight line between two different points.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    /// Where it starts.
    pub start: PointId,
    /// Where it ends.
    pub end: PointId,
}

/// Which side of a line a point is on, looking from the line's start to its end.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// To the left (anticlockwise).
    Left,
    /// To the right (clockwise).
    Right,
}

/// A rule the solver makes hold.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Constraint {
    /// Two points are one place.
    Coincident(PointId, PointId),
    /// A line runs along the plane's first axis.
    Horizontal(LineId),
    /// A line runs along the plane's second axis.
    Vertical(LineId),
    /// Two lines run the same way.
    Parallel(LineId, LineId),
    /// Two lines meet at a right angle.
    Perpendicular(LineId, LineId),
    /// Two lines are the same length.
    EqualLength(LineId, LineId),
    /// Two points are this far apart. More than zero: two points at no distance are
    /// [`Constraint::Coincident`].
    Distance {
        /// One point.
        a: PointId,
        /// The other.
        b: PointId,
        /// The distance, in whole millimetres.
        mm: i64,
    },
    /// A point is this far from a line's carrier (the whole straight line through it), on the
    /// side given. The side is stored, never inferred again, so the same sketch always solves to
    /// the same shape; [`Sketch::side_of`] reads it from the sketch as drawn. At zero distance the
    /// point is on the carrier and the side is not used.
    PointLineDistance {
        /// The point.
        point: PointId,
        /// The line.
        line: LineId,
        /// The distance, in whole millimetres.
        mm: i64,
        /// Which side of the line the point stays on.
        side: Side,
    },
    /// A point stays where it is drawn. The solver never moves it.
    Fixed(PointId),
}

/// Why a point, line or constraint was not added.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SketchError {
    /// A coordinate or length is beyond [`COORD_LIMIT_MM`].
    OutOfRange,
    /// A point or line it names is not in the sketch.
    Unknown,
    /// A line from a point to itself, or a constraint between a point or line and itself.
    SameTwice,
    /// A distance that must be more than zero is not, or a distance is negative.
    BadDistance,
    /// The sketch already holds `u32::MAX` items of that kind.
    Full,
}

impl fmt::Display for SketchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            SketchError::OutOfRange => "a coordinate or length is beyond 1,000 km",
            SketchError::Unknown => "a point or line it names is not in the sketch",
            SketchError::SameTwice => "it relates a point or a line to itself",
            SketchError::BadDistance => "the distance is not allowed here",
            SketchError::Full => "the sketch is full",
        };
        f.write_str(text)
    }
}

impl std::error::Error for SketchError {}

/// Points, lines and constraints, each in authoring order.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Sketch {
    points: Vec<Point>,
    lines: Vec<Line>,
    constraints: Vec<Constraint>,
}

fn next_id(len: usize) -> Result<u32, SketchError> {
    u32::try_from(len).map_err(|_| SketchError::Full)
}

fn in_range(mm: i64) -> bool {
    (-COORD_LIMIT_MM..=COORD_LIMIT_MM).contains(&mm)
}

impl Sketch {
    /// An empty sketch.
    #[must_use]
    pub fn new() -> Self {
        Sketch::default()
    }

    /// Adds a point at whole-millimetre coordinates.
    ///
    /// # Errors
    ///
    /// [`SketchError::OutOfRange`] beyond [`COORD_LIMIT_MM`]; [`SketchError::Full`].
    pub fn add_point(&mut self, x_mm: i64, y_mm: i64) -> Result<PointId, SketchError> {
        if !in_range(x_mm) || !in_range(y_mm) {
            return Err(SketchError::OutOfRange);
        }
        let id = PointId(next_id(self.points.len())?);
        self.points.push(Point { x_mm, y_mm });
        Ok(id)
    }

    /// Adds a line between two different points already in the sketch.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`], [`SketchError::SameTwice`], [`SketchError::Full`].
    pub fn add_line(&mut self, start: PointId, end: PointId) -> Result<LineId, SketchError> {
        self.check_point(start)?;
        self.check_point(end)?;
        if start == end {
            return Err(SketchError::SameTwice);
        }
        let id = LineId(next_id(self.lines.len())?);
        self.lines.push(Line { start, end });
        Ok(id)
    }

    /// Adds a constraint on points and lines already in the sketch.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`], [`SketchError::SameTwice`], [`SketchError::BadDistance`],
    /// [`SketchError::OutOfRange`], [`SketchError::Full`].
    pub fn add_constraint(&mut self, constraint: Constraint) -> Result<ConstraintId, SketchError> {
        match constraint {
            Constraint::Coincident(a, b) => self.check_two_points(a, b)?,
            Constraint::Horizontal(line) | Constraint::Vertical(line) => self.check_line(line)?,
            Constraint::Parallel(a, b)
            | Constraint::Perpendicular(a, b)
            | Constraint::EqualLength(a, b) => {
                self.check_line(a)?;
                self.check_line(b)?;
                if a == b {
                    return Err(SketchError::SameTwice);
                }
            }
            Constraint::Distance { a, b, mm } => {
                self.check_two_points(a, b)?;
                if !in_range(mm) {
                    return Err(SketchError::OutOfRange);
                }
                if mm <= 0 {
                    return Err(SketchError::BadDistance);
                }
            }
            Constraint::PointLineDistance {
                point, line, mm, ..
            } => {
                self.check_point(point)?;
                self.check_line(line)?;
                if !in_range(mm) {
                    return Err(SketchError::OutOfRange);
                }
                if mm < 0 {
                    return Err(SketchError::BadDistance);
                }
            }
            Constraint::Fixed(point) => self.check_point(point)?,
        }
        let id = ConstraintId(next_id(self.constraints.len())?);
        self.constraints.push(constraint);
        Ok(id)
    }

    /// The points, in authoring order: [`PointId`] `n` is the `n`th.
    #[must_use]
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    /// The lines, in authoring order.
    #[must_use]
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// The constraints, in authoring order.
    #[must_use]
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    /// One point, if it is in the sketch.
    #[must_use]
    pub fn point(&self, id: PointId) -> Option<Point> {
        self.points.get(usize::try_from(id.0).ok()?).copied()
    }

    /// One line, if it is in the sketch.
    #[must_use]
    pub fn line(&self, id: LineId) -> Option<Line> {
        self.lines.get(usize::try_from(id.0).ok()?).copied()
    }

    /// Which side of a line's carrier a point is on as drawn, decided exactly in whole
    /// millimetres; `None` when it is on the carrier or either is not in the sketch.
    #[must_use]
    pub fn side_of(&self, point: PointId, line: LineId) -> Option<Side> {
        let line = self.line(line)?;
        let a = self.point(line.start)?;
        let b = self.point(line.end)?;
        let p = self.point(point)?;
        let turn = cross(diff(a, b)?, diff(a, p)?)?;
        match turn.signum() {
            1 => Some(Side::Left),
            -1 => Some(Side::Right),
            _ => None,
        }
    }

    fn check_point(&self, id: PointId) -> Result<(), SketchError> {
        self.point(id).map(|_| ()).ok_or(SketchError::Unknown)
    }

    fn check_line(&self, id: LineId) -> Result<(), SketchError> {
        self.line(id).map(|_| ()).ok_or(SketchError::Unknown)
    }

    fn check_two_points(&self, a: PointId, b: PointId) -> Result<(), SketchError> {
        self.check_point(a)?;
        self.check_point(b)?;
        if a == b {
            return Err(SketchError::SameTwice);
        }
        Ok(())
    }
}

/// `to - from`, exactly. `None` only beyond what a sketch can hold.
pub(crate) fn diff(from: Point, to: Point) -> Option<(i128, i128)> {
    let dx = i128::from(to.x_mm).checked_sub(i128::from(from.x_mm))?;
    let dy = i128::from(to.y_mm).checked_sub(i128::from(from.y_mm))?;
    Some((dx, dy))
}

/// The cross product of two exact differences.
pub(crate) fn cross(u: (i128, i128), v: (i128, i128)) -> Option<i128> {
    u.0.checked_mul(v.1)?.checked_sub(u.1.checked_mul(v.0)?)
}

/// The dot product of two exact differences.
pub(crate) fn dot(u: (i128, i128), v: (i128, i128)) -> Option<i128> {
    u.0.checked_mul(v.0)?.checked_add(u.1.checked_mul(v.1)?)
}
