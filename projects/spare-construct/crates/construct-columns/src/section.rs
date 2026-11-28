//! A column's section: the shape it is cut across, rectangular or round, in whole grid steps, and
//! the ring of corners the builders extrude, turned about the column's centre.
//!
//! - **Rectangular**: a width along x and a depth along y when the column is not turned. Each is
//!   above zero, a whole even number of steps (so the centre is a step, and the four corners are
//!   at half the width and half the depth from it), and at most [`MOST_SIDE`].
//! - **Round**: a diameter, with the same three rules. The engine seam's extrusion takes polygons
//!   only (geometry kernel, `Engine::extrude`), so a round section is built as a regular polygon
//!   of [`ROUND_SIDES`] corners on its circle: the INTERIM that the kernel's exact sweep of arcs
//!   (geom-sweep, chapter 13.4) replaces (see the interim register). Its quantities are the
//!   polygon's, so what a schedule reads is what is built: 0.28 per cent under the true circle's
//!   area. Nothing else here changes the day arcs are built.
//! - **Turn**: an exact angle ([`construct_params::Angle`], any value, main's any-angle rule of
//!   28 Nov). Each corner is the true turn of its offset from the centre, rounded once to the grid
//!   step (half away from zero): within half a step of the true corner on each axis, plus 2^-24 of
//!   a step for the floating-point sine and cosine, which are `libm`'s so the bits are the same on
//!   every machine, for every section up to [`MOST_SIDE`]. Quarter turns are exact (no sine is
//!   taken). INTERIM: the exact rational turn of construct-modify (piece A80, `exact::Turn`, a
//!   bound of 2^-94 of a step) replaces `libm` here once it is on main and below the builders.
//!
//! The ring runs anticlockwise seen from above, a rectangle's from its lower left corner before it is
//! turned and a round section's from the corner at the turn, so one section turned one way has one
//! ring.

use crate::error::ColumnError;
use construct_params::{Angle, Area, Length};

/// The corners of a round section's polygon.
pub const ROUND_SIDES: usize = 48;
/// The largest width, depth or diameter a column has: 5 m.
pub const MOST_SIDE: Length = Length::from_mm(5_000);
/// Angle units in a full turn.
const FULL_TURN: i64 = 360 * Angle::UNITS_PER_DEGREE;
/// Angle units in a quarter turn.
const QUARTER: i64 = 90 * Angle::UNITS_PER_DEGREE;

/// The section a column is cut across.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    /// A rectangle: `width` along x and `depth` along y before the column is turned.
    Rectangular {
        /// Along x.
        width: Length,
        /// Along y.
        depth: Length,
    },
    /// A circle, built as a polygon of [`ROUND_SIDES`] corners (interim, see the module header).
    Round {
        /// Across.
        diameter: Length,
    },
}

/// One size, checked: above zero, a whole even number of steps, at most [`MOST_SIDE`].
fn check(what: &str, size: Length) -> Result<(), ColumnError> {
    if size.steps() <= 0 {
        return Err(ColumnError::BadSize(format!(
            "a column's {what} is above zero, and this one is {size}"
        )));
    }
    if size.steps().checked_rem(2) != Some(0) {
        return Err(ColumnError::BadSize(format!(
            "a column's {what} is a whole even number of grid steps, so that its centre is on a step; {size} is odd"
        )));
    }
    if size > MOST_SIDE {
        return Err(ColumnError::BadSize(format!(
            "a column's {what} is at most {MOST_SIDE}, and this one is {size}"
        )));
    }
    Ok(())
}

/// Half of a checked size.
fn half(size: Length) -> i64 {
    size.steps().checked_div(2).unwrap_or(0)
}

/// An `i64` as an `f64`: exact for every size here, which is below 2^26.
#[allow(
    clippy::cast_precision_loss,
    reason = "sizes are at most 5 m, 2^25 steps, exact in an f64"
)]
fn float(steps: i64) -> f64 {
    steps as f64
}

/// The nearest whole step to a float, a half rounding away from zero.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the value is a size's offset turned, below 2^27 in size"
)]
fn nearest(value: f64) -> i64 {
    value.round() as i64
}

/// An angle in radians.
fn radians(units: i64) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "angle units in a turn are below 2^32, exact in an f64"
    )]
    let units = units as f64;
    units * (core::f64::consts::PI / (180.0 * 9_000_000.0))
}

/// The cosine and sine of an angle in angle units: exact (0 or 1) for a quarter turn.
fn unit(units: i64) -> (f64, f64) {
    let units = units.rem_euclid(FULL_TURN);
    if units.rem_euclid(QUARTER) == 0 {
        return match units.checked_div(QUARTER) {
            Some(1) => (0.0, 1.0),
            Some(2) => (-1.0, 0.0),
            Some(3) => (0.0, -1.0),
            _ => (1.0, 0.0),
        };
    }
    let at = radians(units);
    (libm::cos(at), libm::sin(at))
}

/// The offset `(dx, dy)` turned anticlockwise by `turn`, rounded once to a step.
fn turned(dx: i64, dy: i64, turn: Angle) -> [i64; 2] {
    let (c, s) = unit(turn.units());
    let (dx, dy) = (float(dx), float(dy));
    [nearest(dx * c - dy * s), nearest(dx * s + dy * c)]
}

impl Section {
    /// A rectangular section.
    ///
    /// # Errors
    /// [`ColumnError::BadSize`] for a width or a depth that is not above zero, is odd in steps, or
    /// is past [`MOST_SIDE`].
    pub fn rectangular(width: Length, depth: Length) -> Result<Self, ColumnError> {
        check("width", width)?;
        check("depth", depth)?;
        Ok(Self::Rectangular { width, depth })
    }

    /// A round section.
    ///
    /// # Errors
    /// [`ColumnError::BadSize`] for a diameter that is not above zero, is odd in steps, or is past
    /// [`MOST_SIDE`].
    pub fn round(diameter: Length) -> Result<Self, ColumnError> {
        check("diameter", diameter)?;
        Ok(Self::Round { diameter })
    }

    /// Whether every size of this value passes the checks a constructor makes.
    ///
    /// # Errors
    /// [`ColumnError::BadSize`], as [`Section::rectangular`] and [`Section::round`].
    pub fn checked(self) -> Result<Self, ColumnError> {
        match self {
            Self::Rectangular { width, depth } => Self::rectangular(width, depth),
            Self::Round { diameter } => Self::round(diameter),
        }
    }

    /// The ring of corners about `centre` (grid steps), anticlockwise from above, the section
    /// turned about the centre by `turn` and each corner rounded once, as the module header says.
    ///
    /// # Errors
    /// [`ColumnError::OutOfRange`] when a corner does not fit the grid's coordinates; the sizes'
    /// refusals.
    pub fn ring(self, centre: [i64; 2], turn: Angle) -> Result<Vec<[i64; 2]>, ColumnError> {
        let offsets: Vec<[i64; 2]> = match self.checked()? {
            Self::Rectangular { width, depth } => {
                let (w, d) = (half(width), half(depth));
                vec![
                    turned(w.saturating_neg(), d.saturating_neg(), turn),
                    turned(w, d.saturating_neg(), turn),
                    turned(w, d, turn),
                    turned(w.saturating_neg(), d, turn),
                ]
            }
            Self::Round { diameter } => {
                let r = float(half(diameter));
                let each = FULL_TURN
                    .checked_div(i64::try_from(ROUND_SIDES).map_err(|_| ColumnError::OutOfRange)?)
                    .ok_or(ColumnError::OutOfRange)?;
                let mut corners = Vec::with_capacity(ROUND_SIDES);
                let mut at = turn.units();
                for _ in 0..ROUND_SIDES {
                    let (c, s) = unit(at);
                    corners.push([nearest(r * c), nearest(r * s)]);
                    at = at.checked_add(each).ok_or(ColumnError::OutOfRange)?;
                }
                corners
            }
        };
        offsets
            .into_iter()
            .map(|[dx, dy]| {
                Ok([
                    centre[0].checked_add(dx).ok_or(ColumnError::OutOfRange)?,
                    centre[1].checked_add(dy).ok_or(ColumnError::OutOfRange)?,
                ])
            })
            .collect()
    }

    /// The section's area as built: the area of its ring unturned, exact in whole square steps
    /// (the polygon of a round section sits half a square step off a whole number about half the
    /// time, and a half rounds up). A turn changes the built ring by under a step a corner and is
    /// not in it.
    ///
    /// # Errors
    /// [`ColumnError::OutOfRange`] for a sum past 128 bits; the sizes' refusals.
    pub fn area(self) -> Result<Area, ColumnError> {
        let ring = self.ring([0, 0], Angle::from_units(0))?;
        let twice = twice_area(&ring).ok_or(ColumnError::OutOfRange)?;
        let whole = twice
            .checked_add(1)
            .and_then(|up| up.checked_div(2))
            .ok_or(ColumnError::OutOfRange)?;
        Ok(Area::from_square_steps(whole))
    }
}

/// Twice the area of a ring (anticlockwise above zero), the shoelace sum in 128 bits.
#[must_use]
pub fn twice_area(ring: &[[i64; 2]]) -> Option<i128> {
    let mut twice = 0_i128;
    for (at, [x0, y0]) in ring.iter().enumerate() {
        let [x1, y1] = ring.get(at.checked_add(1)?).or_else(|| ring.first())?;
        let part = i128::from(*x0)
            .checked_mul(i128::from(*y1))?
            .checked_sub(i128::from(*x1).checked_mul(i128::from(*y0))?)?;
        twice = twice.checked_add(part)?;
    }
    Some(twice)
}
