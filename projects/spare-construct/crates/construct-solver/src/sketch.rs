//! What a sketch is: points, straight lines, circles and arcs on one plane, and the constraints
//! that relate them. Coordinates and lengths are whole grid steps of the geometry kernel (`Q0`,
//! 1/6,400 mm; geometry architecture chapter 6.1), as every stored length in Spare Construct is
//! (note CON-A2-F3-P1), so a sketch hands to the kernel unrounded. [`mm`] and [`Point::mm`] make
//! them from whole millimetres. Everything is kept in authoring order, and that order is the only
//! order the solver ever uses.
//!
//! Lines, arcs and circles share points, so a later curve kind adds an entity list and constraint
//! kinds here and nothing else changes: [`Constraint`] is non-exhaustive for that reason.
//!
//! Circles and arcs are one list, the curves, named by [`CurveId`]. Each has a centre point and a
//! radius, and the radius is solved like a coordinate. An arc also has a start and an end point,
//! and runs anticlockwise (from the plane's first axis towards its second) about its centre from
//! start to end. Its ends are ordinary points, so a line joins an arc by sharing a point, and the
//! solver keeps both ends on the arc's circle without being asked.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The largest coordinate or length a sketch takes, in grid steps: 2^40 − 1, about 171.8 km, the
/// kernel's own coordinate range (`geom_num::grid::COORD_MAX`, chapter 6.1). Every coordinate and
/// difference is then exact in a double, and every product of two differences in an `i128`.
pub const COORD_LIMIT: i64 = (1 << 40) - 1;

/// Grid steps in one millimetre: the kernel's `GridStep::Q0` (156.25 nm).
pub const STEPS_PER_MM: i64 = 6_400;

/// `mm` whole millimetres in grid steps, exactly: `mm(2_400)` is 15,360,000. Any `i32` fits.
#[must_use]
pub fn mm(mm: i32) -> i64 {
    // |mm| < 2^31, so mm x 6,400 < 2^44: it never saturates.
    i64::from(mm).saturating_mul(STEPS_PER_MM)
}

/// A point, by its place in authoring order.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointId(pub u32);

/// A straight line, by its place in authoring order.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineId(pub u32);

/// A constraint, by its place in authoring order.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstraintId(pub u32);

/// A circle or an arc, by its place in authoring order.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CurveId(pub u32);

/// A circle or an arc. The radius is a whole number of grid steps, more than zero: as drawn
/// before a solve, and as solved after one ([`Sketch::with_positions`]).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    /// A whole circle.
    Circle {
        /// Its centre.
        centre: PointId,
        /// Its radius, in whole grid steps.
        radius: i64,
    },
    /// Part of a circle, anticlockwise about its centre from `start` to `end`. Both ends stay on
    /// the circle: within [`crate::MODEL_TOLERANCE_MM`] of the radius after a solve, or the solve
    /// is refused.
    Arc {
        /// Its centre.
        centre: PointId,
        /// Its radius, in whole grid steps.
        radius: i64,
        /// Where it starts.
        start: PointId,
        /// Where it ends, anticlockwise from the start.
        end: PointId,
    },
}

impl Curve {
    /// Its centre.
    #[must_use]
    pub fn centre(&self) -> PointId {
        match *self {
            Curve::Circle { centre, .. } | Curve::Arc { centre, .. } => centre,
        }
    }

    /// Its radius, in whole grid steps.
    #[must_use]
    pub fn radius(&self) -> i64 {
        match *self {
            Curve::Circle { radius, .. } | Curve::Arc { radius, .. } => radius,
        }
    }

    /// An arc's start and end; `None` for a circle.
    #[must_use]
    pub fn ends(&self) -> Option<(PointId, PointId)> {
        match *self {
            Curve::Circle { .. } => None,
            Curve::Arc { start, end, .. } => Some((start, end)),
        }
    }

    fn with_radius(self, radius: i64) -> Curve {
        match self {
            Curve::Circle { centre, .. } => Curve::Circle { centre, radius },
            Curve::Arc {
                centre, start, end, ..
            } => Curve::Arc {
                centre,
                radius,
                start,
                end,
            },
        }
    }
}

/// How two tangent curves touch. Stored on the constraint, never inferred again;
/// [`Sketch::touch_of`] reads it from the sketch as drawn.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Touch {
    /// Each lies outside the other: the centres are the two radii added apart.
    Outside,
    /// The first lies inside the second: the centres are the second radius less the first apart.
    FirstInside,
    /// The second lies inside the first: the centres are the first radius less the second apart.
    SecondInside,
}

/// A point on the sketch plane, in whole grid steps (6,400 to the millimetre).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    /// Along the plane's first axis, in grid steps.
    pub x: i64,
    /// Along the plane's second axis, in grid steps.
    pub y: i64,
}

impl Point {
    /// The point `x_mm`, `y_mm` whole millimetres along the plane's axes, exactly.
    #[must_use]
    pub fn mm(x_mm: i32, y_mm: i32) -> Point {
        Point {
            x: mm(x_mm),
            y: mm(y_mm),
        }
    }
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
        /// The distance, in whole grid steps.
        steps: i64,
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
        /// The distance, in whole grid steps.
        steps: i64,
        /// Which side of the line the point stays on.
        side: Side,
    },
    /// A point stays where it is drawn. The solver never moves it.
    Fixed(PointId),
    /// A point lies on a line's carrier (the whole straight line through it).
    PointOnLine(PointId, LineId),
    /// A point lies on a curve's circle. For an arc this is the whole circle the arc is part of,
    /// as a point on a line is on its carrier: only equalities are solved (chapter 15.3).
    PointOnCurve(PointId, CurveId),
    /// Two curves share a centre.
    Concentric(CurveId, CurveId),
    /// Two curves have the same radius.
    EqualRadius(CurveId, CurveId),
    /// A curve's radius, in whole grid steps, more than zero.
    Radius {
        /// The circle or arc.
        curve: CurveId,
        /// The radius.
        steps: i64,
    },
    /// A curve's diameter, in whole grid steps, more than zero. The radius is stored in whole
    /// steps, so an odd number of steps leaves it half a step (78 nm) off, far inside the model
    /// tolerance; an odd number of millimetres, such as 1,601 mm, is a whole radius of steps.
    Diameter {
        /// The circle or arc.
        curve: CurveId,
        /// The diameter.
        steps: i64,
    },
    /// A line's carrier touches a curve's circle, with the curve's centre on the side of the line
    /// given (looking from the line's start to its end). The side is stored, never inferred
    /// again; [`Sketch::side_of`] on the centre reads it from the sketch as drawn. Where the line
    /// and an arc share an end (one point, or points tied by coincidences) they touch there.
    TangentLine {
        /// The line.
        line: LineId,
        /// The circle or arc.
        curve: CurveId,
        /// Which side of the line the curve's centre stays on.
        side: Side,
    },
    /// Two curves' circles touch, the way given. Stored, never inferred again;
    /// [`Sketch::touch_of`] reads it from the sketch as drawn. Where two arcs share an end they
    /// touch there.
    TangentCurves {
        /// One curve.
        a: CurveId,
        /// The other.
        b: CurveId,
        /// How they touch.
        touch: Touch,
    },
}

/// Why a point, line or constraint was not added.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SketchError {
    /// A coordinate or length is beyond [`COORD_LIMIT`].
    OutOfRange,
    /// A point, line or curve it names is not in the sketch.
    Unknown,
    /// A line from a point to itself, a curve whose centre is also an end or whose ends are one
    /// point, or a constraint between a point, line or curve and itself.
    SameTwice,
    /// A distance that must be more than zero is not, or a distance is negative.
    BadDistance,
    /// The sketch already holds `u32::MAX` items of that kind.
    Full,
    /// Positions were given for a different number of points or curves than the sketch holds.
    WrongCount,
}

impl fmt::Display for SketchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            SketchError::OutOfRange => {
                "a coordinate or length is beyond 171.8 km (2^40 grid steps)"
            }
            SketchError::Unknown => "a point or line it names is not in the sketch",
            SketchError::SameTwice => "it relates a point or a line to itself",
            SketchError::BadDistance => "the distance is not allowed here",
            SketchError::Full => "the sketch is full",
            SketchError::WrongCount => "positions were given for a different sketch",
        };
        f.write_str(text)
    }
}

impl std::error::Error for SketchError {}

/// Points, lines, curves and constraints, each in authoring order.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Sketch {
    points: Vec<Point>,
    lines: Vec<Line>,
    constraints: Vec<Constraint>,
    // Written only when there are some, so a sketch of points and lines keeps its bytes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    curves: Vec<Curve>,
}

fn next_id(len: usize) -> Result<u32, SketchError> {
    u32::try_from(len).map_err(|_| SketchError::Full)
}

fn in_range(steps: i64) -> bool {
    (-COORD_LIMIT..=COORD_LIMIT).contains(&steps)
}

impl Sketch {
    /// An empty sketch.
    #[must_use]
    pub fn new() -> Self {
        Sketch::default()
    }

    /// Adds a point at grid-step coordinates: `add_point(mm(1_200), mm(0))` for one at 1,200 mm.
    ///
    /// # Errors
    ///
    /// [`SketchError::OutOfRange`] beyond [`COORD_LIMIT`]; [`SketchError::Full`].
    pub fn add_point(&mut self, x: i64, y: i64) -> Result<PointId, SketchError> {
        if !in_range(x) || !in_range(y) {
            return Err(SketchError::OutOfRange);
        }
        let id = PointId(next_id(self.points.len())?);
        self.points.push(Point { x, y });
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
            Constraint::Distance { a, b, steps } => {
                self.check_two_points(a, b)?;
                if !in_range(steps) {
                    return Err(SketchError::OutOfRange);
                }
                if steps <= 0 {
                    return Err(SketchError::BadDistance);
                }
            }
            Constraint::PointLineDistance {
                point, line, steps, ..
            } => {
                self.check_point(point)?;
                self.check_line(line)?;
                if !in_range(steps) {
                    return Err(SketchError::OutOfRange);
                }
                if steps < 0 {
                    return Err(SketchError::BadDistance);
                }
            }
            Constraint::Fixed(point) => self.check_point(point)?,
            Constraint::PointOnLine(point, line) => {
                self.check_point(point)?;
                self.check_line(line)?;
            }
            Constraint::PointOnCurve(point, curve) => {
                self.check_point(point)?;
                if self.check_curve(curve)?.centre() == point {
                    return Err(SketchError::SameTwice);
                }
            }
            Constraint::Concentric(a, b)
            | Constraint::EqualRadius(a, b)
            | Constraint::TangentCurves { a, b, .. } => {
                self.check_curve(a)?;
                self.check_curve(b)?;
                if a == b {
                    return Err(SketchError::SameTwice);
                }
            }
            Constraint::Radius { curve, steps } | Constraint::Diameter { curve, steps } => {
                self.check_curve(curve)?;
                if !in_range(steps) {
                    return Err(SketchError::OutOfRange);
                }
                if steps <= 0 {
                    return Err(SketchError::BadDistance);
                }
            }
            Constraint::TangentLine { line, curve, .. } => {
                self.check_line(line)?;
                self.check_curve(curve)?;
            }
        }
        let id = ConstraintId(next_id(self.constraints.len())?);
        self.constraints.push(constraint);
        Ok(id)
    }

    /// Adds a circle about a point already in the sketch.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::BadDistance`] for a radius of zero or less;
    /// [`SketchError::OutOfRange`] beyond [`COORD_LIMIT`]; [`SketchError::Full`].
    pub fn add_circle(&mut self, centre: PointId, radius: i64) -> Result<CurveId, SketchError> {
        self.check_point(centre)?;
        if !in_range(radius) {
            return Err(SketchError::OutOfRange);
        }
        if radius <= 0 {
            return Err(SketchError::BadDistance);
        }
        let id = CurveId(next_id(self.curves.len())?);
        self.curves.push(Curve::Circle { centre, radius });
        Ok(id)
    }

    /// Adds an arc about `centre`, anticlockwise from `start` to `end`, three different points
    /// already in the sketch. Its radius as drawn is the distance from the centre to the start,
    /// to the nearest grid step (exactly, from the integer squared distance); the end
    /// need not be on that circle as drawn, since the solver puts it there.
    ///
    /// # Errors
    ///
    /// [`SketchError::Unknown`]; [`SketchError::SameTwice`] unless the three points are
    /// different; [`SketchError::BadDistance`] when the start is drawn at the centre;
    /// [`SketchError::OutOfRange`] for a radius beyond [`COORD_LIMIT`]; [`SketchError::Full`].
    pub fn add_arc(
        &mut self,
        centre: PointId,
        start: PointId,
        end: PointId,
    ) -> Result<CurveId, SketchError> {
        self.check_point(centre)?;
        self.check_point(start)?;
        self.check_point(end)?;
        if centre == start || centre == end || start == end {
            return Err(SketchError::SameTwice);
        }
        let (Some(c), Some(s)) = (self.point(centre), self.point(start)) else {
            return Err(SketchError::Unknown);
        };
        let radius = diff(c, s)
            .and_then(|u| dot(u, u))
            .and_then(nearest_root)
            .and_then(|r| i64::try_from(r).ok())
            .ok_or(SketchError::OutOfRange)?;
        if !in_range(radius) {
            return Err(SketchError::OutOfRange);
        }
        if radius <= 0 {
            return Err(SketchError::BadDistance);
        }
        let id = CurveId(next_id(self.curves.len())?);
        self.curves.push(Curve::Arc {
            centre,
            radius,
            start,
            end,
        });
        Ok(id)
    }

    /// The same sketch with every point and every curve's radius moved to the values given, in
    /// authoring order: how a solve's answer ([`crate::Solved`]'s `points` and `radii`) is
    /// written back, so that the next solve or drag starts from it.
    ///
    /// # Errors
    ///
    /// [`SketchError::WrongCount`] unless there is one point for each point and one radius for
    /// each curve; [`SketchError::OutOfRange`]; [`SketchError::BadDistance`] for a radius of
    /// zero or less.
    pub fn with_positions(&self, points: &[Point], radii: &[i64]) -> Result<Sketch, SketchError> {
        if points.len() != self.points.len() || radii.len() != self.curves.len() {
            return Err(SketchError::WrongCount);
        }
        if points.iter().any(|p| !in_range(p.x) || !in_range(p.y))
            || radii.iter().any(|r| !in_range(*r))
        {
            return Err(SketchError::OutOfRange);
        }
        if radii.iter().any(|r| *r <= 0) {
            return Err(SketchError::BadDistance);
        }
        let mut moved = self.clone();
        moved.points = points.to_vec();
        moved.curves = self
            .curves
            .iter()
            .zip(radii)
            .map(|(curve, r)| curve.with_radius(*r))
            .collect();
        Ok(moved)
    }

    /// The curves, in authoring order.
    #[must_use]
    pub fn curves(&self) -> &[Curve] {
        &self.curves
    }

    /// One curve, if it is in the sketch.
    #[must_use]
    pub fn curve(&self, id: CurveId) -> Option<Curve> {
        self.curves.get(usize::try_from(id.0).ok()?).copied()
    }

    /// How two curves touch as drawn, decided exactly in whole grid steps: outside when the
    /// centres are at least the larger radius apart, otherwise the smaller inside the larger;
    /// `None` exactly at the larger radius, for equal radii closer than that, or when either is
    /// not in the sketch.
    #[must_use]
    pub fn touch_of(&self, a: CurveId, b: CurveId) -> Option<Touch> {
        let (a, b) = (self.curve(a)?, self.curve(b)?);
        let gap = diff(self.point(a.centre())?, self.point(b.centre())?)?;
        let gap_squared = dot(gap, gap)?;
        let larger = i128::from(a.radius().max(b.radius()));
        let larger_squared = larger.checked_mul(larger)?;
        match gap_squared.cmp(&larger_squared) {
            std::cmp::Ordering::Greater => Some(Touch::Outside),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Less => match a.radius().cmp(&b.radius()) {
                std::cmp::Ordering::Less => Some(Touch::FirstInside),
                std::cmp::Ordering::Greater => Some(Touch::SecondInside),
                std::cmp::Ordering::Equal => None,
            },
        }
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

    /// Which side of a line's carrier a point is on as drawn, decided exactly in whole grid
    /// steps; `None` when it is on the carrier or either is not in the sketch.
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

    fn check_curve(&self, id: CurveId) -> Result<Curve, SketchError> {
        self.curve(id).ok_or(SketchError::Unknown)
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
    let dx = i128::from(to.x).checked_sub(i128::from(from.x))?;
    let dy = i128::from(to.y).checked_sub(i128::from(from.y))?;
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

/// The whole number nearest the square root of `n`, exactly. No root of a whole number is a
/// half, so there is no tie. `None` for a negative `n`.
pub(crate) fn nearest_root(n: i128) -> Option<i128> {
    if n < 0 {
        return None;
    }
    let floor = n.isqrt();
    // The root is nearer floor + 1 when n > (floor + 1/2)^2 = floor^2 + floor + 1/4.
    let above = n.checked_sub(floor.checked_mul(floor)?)? > floor;
    floor.checked_add(i128::from(above))
}
