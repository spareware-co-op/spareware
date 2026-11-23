//! A grid line's geometry in plan: a straight segment or an arc, in exact grid steps.
//!
//! Lengths are [`Length`] (whole steps of the geometry kernel, 1/6,400 mm) and angles are
//! [`Angle`] (whole 1/9,000,000 degrees), so what is stored is exact. Every coordinate, and an
//! arc's centre plus or minus its radius, lies within [`LIMIT`] steps (171.8 km) of the origin:
//! with that, every product of two coordinates fits 128 bits, and every operation here is a
//! checked one.
//!
//! **Exact and derived.** What the shape is, is stored: a line's two points; an arc's centre,
//! radius and the start and end angles. What follows from it is worked out, and where it is not a
//! whole number of steps it is the nearest step (a half rounds away from zero), and the
//! function says so:
//!
//! - a line's length: the square root of the sum of two squares, rounded to the nearest step, in
//!   whole numbers (exact arithmetic, an integer square root);
//! - an arc's end points, its length and its outline: from sine and cosine in `f64`, through the
//!   `libm` crate's own pure-Rust functions, so the same input gives the same bits on every
//!   machine; each coordinate is within one step of the true value for any arc inside the limit
//!   (an `f64` carries 53 bits, a coordinate needs 41, and a sine is good to a few units of its
//!   last bit);
//! - [`Shape::extended`]: a line lengthened by a length along its own direction; exact when the
//!   line runs along x or y, otherwise each new coordinate is the nearest step to the true one.
//!
//! An arc runs anticlockwise from its start angle to its end angle (angles from the positive x
//! axis, anticlockwise, as the plan sees them). A start equal to the end is refused: an arc is
//! never empty, and never a full turn.

use crate::GridError;
use construct_params::{Angle, Length};

/// The furthest a coordinate may be from the origin, in steps: 2^40 - 1 (171.8 km). The same
/// limit as the sketch solver's coordinates.
pub const LIMIT: Length = Length::from_steps(1_099_511_627_775);

/// A full turn, in angle units: 360 degrees of 9,000,000 units.
pub const FULL_TURN: i64 = 3_240_000_000;

/// The most segments an arc's outline is cut into.
const MOST_SEGMENTS: i128 = 1024;

/// A point in plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    /// Along x.
    pub x: Length,
    /// Along y.
    pub y: Length,
}

impl Point {
    /// A point.
    #[must_use]
    pub const fn new(x: Length, y: Length) -> Self {
        Self { x, y }
    }

    /// A point from whole millimetres.
    #[must_use]
    pub const fn mm(x: i32, y: i32) -> Self {
        Self {
            x: Length::from_mm(x),
            y: Length::from_mm(y),
        }
    }

    /// Whether both coordinates are within [`LIMIT`].
    #[must_use]
    pub fn in_range(self) -> bool {
        self.x.steps().unsigned_abs() <= LIMIT.steps().unsigned_abs()
            && self.y.steps().unsigned_abs() <= LIMIT.steps().unsigned_abs()
    }
}

/// One end of a grid line: where it starts or where it finishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum End {
    /// The first point of a line; an arc's start angle.
    Start,
    /// The second point of a line; an arc's end angle.
    Finish,
}

/// A grid line's geometry in plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A straight segment from one point to another.
    Line {
        /// Where it starts.
        from: Point,
        /// Where it finishes.
        to: Point,
    },
    /// An arc of a circle, anticlockwise from `from` to `to`.
    Arc {
        /// The circle's centre.
        centre: Point,
        /// The circle's radius, above zero.
        radius: Length,
        /// The angle it starts at, in `[0, 360)` degrees.
        from: Angle,
        /// The angle it finishes at, in `[0, 360)` degrees, not equal to `from`.
        to: Angle,
    },
}

// ----- checked whole-number helpers, shared with `cross` -----

/// A length as a 128-bit whole number of steps.
pub(crate) fn wide(length: Length) -> i128 {
    i128::from(length.steps())
}

/// The cross product of two vectors, if it fits.
pub(crate) fn cross(a: (i128, i128), b: (i128, i128)) -> Option<i128> {
    a.0.checked_mul(b.1)?.checked_sub(a.1.checked_mul(b.0)?)
}

/// The dot product of two vectors, if it fits.
pub(crate) fn dot(a: (i128, i128), b: (i128, i128)) -> Option<i128> {
    a.0.checked_mul(b.0)?.checked_add(a.1.checked_mul(b.1)?)
}

/// `n / d` rounded to the nearest whole number, a half rounding up, for `d` above zero.
pub(crate) fn round_div(n: i128, d: i128) -> Option<i128> {
    n.checked_mul(2)?
        .checked_add(d)?
        .checked_div_euclid(d.checked_mul(2)?)
}

/// The square root of `n` rounded to the nearest whole number, for `n` of zero or more.
pub(crate) fn round_sqrt(n: i128) -> Option<i128> {
    let n = u128::try_from(n).ok()?;
    let root = n.isqrt();
    // (root + 1/2)^2 is root^2 + root + 1/4, so the next whole number is nearer above root^2 + root.
    let above = n.checked_sub(root.checked_mul(root)?)? > root;
    let rounded = if above { root.checked_add(1)? } else { root };
    i128::try_from(rounded).ok()
}

/// A length from a 128-bit whole number of steps, if it fits.
pub(crate) fn length(steps: i128) -> Option<Length> {
    i64::try_from(steps).ok().map(Length::from_steps)
}

/// A whole number of steps as `f64`. An `f64` holds 53 bits and a coordinate needs 41, so the
/// numbers that are turned into floats here are exact; a product is rounded to 53 bits.
#[allow(
    clippy::cast_precision_loss,
    reason = "coordinates are below 2^41; the rounding of larger products is allowed for in the stated bounds"
)]
pub(crate) fn float(steps: i128) -> f64 {
    steps as f64
}

/// The nearest whole step to a float, a half rounding away from zero; `None` if it is not a finite
/// number inside 64 bits.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the value is rounded and checked to be below 2^62 in size first"
)]
pub(crate) fn nearest_step(value: f64) -> Option<i64> {
    (value.is_finite() && value.abs() < 4.0e18).then(|| value.round() as i64)
}

/// An angle in radians.
pub(crate) fn radians(units: i64) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "angle units are below 2^32, exact in an f64"
    )]
    let units = units as f64;
    units * (core::f64::consts::PI / (180.0 * 9_000_000.0))
}

fn normal(angle: Angle) -> Angle {
    Angle::from_units(angle.units().rem_euclid(FULL_TURN))
}

/// The sweep of an arc from `from` to `to`, in angle units, in `(0, FULL_TURN)`.
pub(crate) fn sweep_units(from: Angle, to: Angle) -> i64 {
    to.units()
        .saturating_sub(from.units())
        .rem_euclid(FULL_TURN)
}

/// The point on a circle at an angle, the nearest steps to the true one.
pub(crate) fn on_circle(centre: Point, radius: Length, angle_units: i64) -> Option<Point> {
    let theta = radians(angle_units.rem_euclid(FULL_TURN));
    let r = float(wide(radius));
    let x = float(wide(centre.x)) + r * libm::cos(theta);
    let y = float(wide(centre.y)) + r * libm::sin(theta);
    Some(Point::new(
        Length::from_steps(nearest_step(x)?),
        Length::from_steps(nearest_step(y)?),
    ))
}

impl Shape {
    /// A straight line.
    ///
    /// # Errors
    ///
    /// [`GridError::ZeroLength`] for two equal points; [`GridError::OutOfRange`] past [`LIMIT`].
    pub fn line(from: Point, to: Point) -> Result<Self, GridError> {
        let line = Self::Line { from, to };
        line.check()?;
        Ok(line)
    }

    /// An arc, anticlockwise from `from` to `to`. The angles are taken into `[0, 360)` degrees.
    ///
    /// # Errors
    ///
    /// [`GridError::ZeroRadius`], [`GridError::NoSweep`] or [`GridError::OutOfRange`].
    pub fn arc(centre: Point, radius: Length, from: Angle, to: Angle) -> Result<Self, GridError> {
        let arc = Self::Arc {
            centre,
            radius,
            from: normal(from),
            to: normal(to),
        };
        arc.check()?;
        Ok(arc)
    }

    /// Whether the shape is one this crate makes: what [`Shape::line`] and [`Shape::arc`] check,
    /// and that an arc's angles are in `[0, 360)`.
    ///
    /// # Errors
    ///
    /// As [`Shape::line`] and [`Shape::arc`].
    pub fn check(&self) -> Result<(), GridError> {
        match *self {
            Self::Line { from, to } => {
                if !from.in_range() || !to.in_range() {
                    return Err(GridError::OutOfRange);
                }
                if from == to {
                    return Err(GridError::ZeroLength);
                }
            }
            Self::Arc {
                centre,
                radius,
                from,
                to,
            } => {
                if radius.steps() <= 0 {
                    return Err(GridError::ZeroRadius);
                }
                let reach = |c: Length| {
                    c.steps()
                        .unsigned_abs()
                        .checked_add(radius.steps().unsigned_abs())
                };
                let inside =
                    |c: Length| reach(c).is_some_and(|r| r <= LIMIT.steps().unsigned_abs());
                if !inside(centre.x) || !inside(centre.y) {
                    return Err(GridError::OutOfRange);
                }
                let turn = 0..FULL_TURN;
                if !turn.contains(&from.units()) || !turn.contains(&to.units()) {
                    return Err(GridError::Malformed(
                        "an arc's angles are from 0 up to 360 degrees".to_owned(),
                    ));
                }
                if from == to {
                    return Err(GridError::NoSweep);
                }
            }
        }
        Ok(())
    }

    /// The point at one end of the shape: a line's end, or the point of an arc at its start or end
    /// angle (the nearest steps).
    #[must_use]
    pub fn end_point(&self, end: End) -> Option<Point> {
        match (*self, end) {
            (Self::Line { from, .. }, End::Start) => Some(from),
            (Self::Line { to, .. }, End::Finish) => Some(to),
            (
                Self::Arc {
                    centre,
                    radius,
                    from,
                    ..
                },
                End::Start,
            ) => on_circle(centre, radius, from.units()),
            (
                Self::Arc {
                    centre, radius, to, ..
                },
                End::Finish,
            ) => on_circle(centre, radius, to.units()),
        }
    }

    /// The length along the shape, the nearest step. A line's is exact whole-number arithmetic; an
    /// arc's is its radius times its sweep in `f64`, within one step.
    #[must_use]
    pub fn length(&self) -> Length {
        let steps = match *self {
            Self::Line { from, to } => {
                let d = (
                    wide(to.x).saturating_sub(wide(from.x)),
                    wide(to.y).saturating_sub(wide(from.y)),
                );
                dot(d, d).and_then(round_sqrt).and_then(length)
            }
            Self::Arc {
                radius, from, to, ..
            } => {
                let arc = float(wide(radius)) * radians(sweep_units(from, to));
                nearest_step(arc).map(Length::from_steps)
            }
        };
        steps.unwrap_or(Length::from_steps(i64::MAX))
    }

    /// The shape moved by `dx` and `dy`.
    ///
    /// # Errors
    ///
    /// [`GridError::OutOfRange`] if it would go past [`LIMIT`].
    pub fn moved(&self, dx: Length, dy: Length) -> Result<Self, GridError> {
        let shift = |p: Point| -> Result<Point, GridError> {
            let x = p.x.checked_add(dx).ok_or(GridError::OutOfRange)?;
            let y = p.y.checked_add(dy).ok_or(GridError::OutOfRange)?;
            Ok(Point::new(x, y))
        };
        let moved = match *self {
            Self::Line { from, to } => Self::Line {
                from: shift(from)?,
                to: shift(to)?,
            },
            Self::Arc {
                centre,
                radius,
                from,
                to,
            } => Self::Arc {
                centre: shift(centre)?,
                radius,
                from,
                to,
            },
        };
        moved.check()?;
        Ok(moved)
    }

    /// A line with one end put at another point (an end dragged).
    ///
    /// # Errors
    ///
    /// [`GridError::WrongShape`] for an arc; [`GridError::ZeroLength`] or
    /// [`GridError::OutOfRange`] as [`Shape::line`].
    pub fn with_end(&self, end: End, point: Point) -> Result<Self, GridError> {
        match *self {
            Self::Line { from, to } => match end {
                End::Start => Self::line(point, to),
                End::Finish => Self::line(from, point),
            },
            Self::Arc { .. } => Err(GridError::WrongShape(
                "An arc's ends are angles: use the angle of the end.",
            )),
        }
    }

    /// An arc with one end put at another angle.
    ///
    /// # Errors
    ///
    /// [`GridError::WrongShape`] for a line; [`GridError::NoSweep`] if the angle is the other
    /// end's.
    pub fn with_angle(&self, end: End, angle: Angle) -> Result<Self, GridError> {
        match *self {
            Self::Arc {
                centre,
                radius,
                from,
                to,
            } => match end {
                End::Start => Self::arc(centre, radius, angle, to),
                End::Finish => Self::arc(centre, radius, from, angle),
            },
            Self::Line { .. } => Err(GridError::WrongShape(
                "A line's ends are points: use the point of the end.",
            )),
        }
    }

    /// A line made longer at one end by `by` along its own direction (`by` below zero shortens
    /// it). Exact when the line runs along x or y; otherwise each new coordinate is the nearest
    /// step to the true one.
    ///
    /// # Errors
    ///
    /// [`GridError::WrongShape`] for an arc (see [`Shape::swept`]); [`GridError::ZeroLength`] if
    /// the line would have no length or turn round; [`GridError::OutOfRange`].
    pub fn extended(&self, end: End, by: Length) -> Result<Self, GridError> {
        let Self::Line { from, to } = *self else {
            return Err(GridError::WrongShape(
                "An arc is lengthened by an angle: use the sweep.",
            ));
        };
        let (fixed, moving) = match end {
            End::Start => (to, from),
            End::Finish => (from, to),
        };
        // The unit vector from the fixed end to the moving end.
        let d = (
            wide(moving.x).saturating_sub(wide(fixed.x)),
            wide(moving.y).saturating_sub(wide(fixed.y)),
        );
        let long = dot(d, d).ok_or(GridError::OutOfRange)?;
        let root = float(long).sqrt();
        let by = float(wide(by));
        let step = |component: i128, base: Length| -> Result<Length, GridError> {
            let more = nearest_step(by * (float(component) / root)).ok_or(GridError::OutOfRange)?;
            base.checked_add(Length::from_steps(more))
                .ok_or(GridError::OutOfRange)
        };
        let new = Point::new(step(d.0, moving.x)?, step(d.1, moving.y)?);
        // The new end must stay on the same side of the fixed end: a line that turns round is refused.
        let after = (
            wide(new.x).saturating_sub(wide(fixed.x)),
            wide(new.y).saturating_sub(wide(fixed.y)),
        );
        if dot(d, after).is_none_or(|along| along <= 0) {
            return Err(GridError::ZeroLength);
        }
        match end {
            End::Start => Self::line(new, to),
            End::Finish => Self::line(from, new),
        }
    }

    /// An arc made longer at one end by `by` of sweep: its start angle goes back, or its end angle
    /// goes on (`by` below zero shortens it).
    ///
    /// # Errors
    ///
    /// [`GridError::WrongShape`] for a line; [`GridError::NoSweep`] if nothing would be left;
    /// [`GridError::FullTurn`] if it would be a full turn or more.
    pub fn swept(&self, end: End, by: Angle) -> Result<Self, GridError> {
        let Self::Arc {
            centre,
            radius,
            from,
            to,
        } = *self
        else {
            return Err(GridError::WrongShape(
                "A line is lengthened by a length: use extend.",
            ));
        };
        let sweep = sweep_units(from, to);
        let longer = sweep.checked_add(by.units()).ok_or(GridError::FullTurn)?;
        if longer <= 0 {
            return Err(GridError::NoSweep);
        }
        if longer >= FULL_TURN {
            return Err(GridError::FullTurn);
        }
        let moved = |angle: Angle, units: i64| {
            Angle::from_units(angle.units().saturating_add(units).rem_euclid(FULL_TURN))
        };
        match end {
            End::Start => Self::arc(centre, radius, moved(from, by.units().saturating_neg()), to),
            End::Finish => Self::arc(centre, radius, from, moved(to, by.units())),
        }
    }

    /// The shape as a chain of straight pieces, none straying from it by more than `deviation`
    /// (at least one step): a line is its two points, an arc as many chords as that needs, at
    /// most 1,024, every point one the arc passes through (the nearest steps). The ends are the
    /// shape's own ends.
    #[must_use]
    pub fn polyline(&self, deviation: Length) -> Vec<Point> {
        match *self {
            Self::Line { from, to } => vec![from, to],
            Self::Arc {
                centre,
                radius,
                from,
                to,
            } => {
                let sweep = sweep_units(from, to);
                // A chord of angle a strays r(1 - cos(a/2)) from its arc.
                let r = float(wide(radius));
                let dev = float(wide(deviation)).max(1.0);
                let chord = if dev >= r {
                    core::f64::consts::PI
                } else {
                    2.0 * libm::acos(1.0 - dev / r)
                };
                let wanted = (radians(sweep) / chord).ceil();
                let parts = nearest_step(wanted)
                    .map_or(MOST_SEGMENTS, i128::from)
                    .clamp(1, MOST_SEGMENTS);
                let mut points = Vec::new();
                for k in 0..=parts {
                    let units = if k == parts {
                        to.units()
                    } else {
                        let part = i128::from(sweep)
                            .checked_mul(k)
                            .and_then(|n| n.checked_div(parts))
                            .and_then(|n| i64::try_from(n).ok())
                            .unwrap_or(0);
                        from.units().saturating_add(part)
                    };
                    if let Some(point) = on_circle(centre, radius, units) {
                        points.push(point);
                    }
                }
                points
            }
        }
    }
}
