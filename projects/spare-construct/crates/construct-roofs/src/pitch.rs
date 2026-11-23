//! How steeply an edge slopes: its pitch, held as it was given, and the exact slope the shape is
//! worked from.
//!
//! A pitch is given one of two ways, and each is held as given, in construct-params' own values:
//! a rise over a run ([`Slope`]: `1:4`, `25%`), exact as two whole numbers in lowest terms; or an
//! angle ([`Angle`]: `30°`), exact in the kernel's angle units (1/2,500 of an arc-second). Which
//! form a person gave is the primary one, as the geometry kernel's chapter 17 holds for bearings.
//!
//! The shape is worked from a slope. A slope given as a rise over a run is used as it is. An angle
//! becomes the slope whose run is [`ANGLE_RUN`] (10^9) and whose rise is the nearest whole number
//! to 10^9 times its tangent ([`Pitch::slope`]): 30 degrees is 577,350,269 in 1,000,000,000. That
//! is the one float in this crate: the tangent is libm's (a pure Rust port of musl's), whose
//! answer is within a unit in the last place and the same bits on every machine, and it is
//! rounded to a whole rise at once and never stored. The slope it gives is within 5 x 10^-10 of the
//! angle's tangent, so a height worked from it is off the true height by at most 5 x 10^-10 times
//! its run: 0.000025 mm over 50 m, four hundred times finer than the model's tolerance of 0.01 mm.
//!
//! A pitch rises: a slope of 0 or less, or an angle of 0 or less, is refused (an edge that does
//! not slope is given no pitch), and so is one steeper than [`MOST_SLOPE`], 10 in 1 (about 84.3
//! degrees), or an angle of 90 degrees or more.

use crate::error::RoofError;
use construct_params::{Angle, Slope, Value};
use kit_core::Id;

/// The run of the slope an angle is worked as: 10^9.
pub const ANGLE_RUN: i64 = 1_000_000_000;

/// The steepest slope a roof's edge may have, as a rise over a run: 10 in 1, about 84.3 degrees.
pub const MOST_SLOPE: (i64, i64) = (10, 1);

/// An edge's pitch, as it was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pitch {
    /// A rise over a run.
    Slope(Slope),
    /// An angle above the level.
    Angle(Angle),
}

/// The slope nearest an angle's tangent with a run of [`ANGLE_RUN`], for an angle above 0 and
/// below 90 degrees; `None` otherwise.
fn slope_of_angle(angle: Angle) -> Option<Slope> {
    let right = Angle::UNITS_PER_DEGREE.checked_mul(90)?;
    if angle.units() <= 0 || angle.units() >= right {
        return None;
    }
    // Below 90 degrees an angle is under 810,000,000 units, which an i32 and an f64 hold exactly.
    let units = f64::from(i32::try_from(angle.units()).ok()?);
    let per_degree = f64::from(i32::try_from(Angle::UNITS_PER_DEGREE).ok()?);
    let radians = units / per_degree * (std::f64::consts::PI / 180.0);
    let rise = libm::round(libm::tan(radians) * 1.0e9);
    // The steepest angle below 90 degrees is one unit short of it, whose tangent is under 10^9:
    // the rise is under 10^18, which an i64 holds.
    if !rise.is_finite() || !(1.0..1.0e18).contains(&rise) {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a whole number from 1 to 10^18, checked above, which an i64 holds exactly"
    )]
    let rise = rise as i64;
    Slope::new(rise, ANGLE_RUN)
}

impl Pitch {
    /// The slope the shape is worked from, as the module header says; `None` for an angle not
    /// above 0 and below 90 degrees, which [`Pitch::checked`] refuses.
    #[must_use]
    pub fn slope(self) -> Option<Slope> {
        match self {
            Self::Slope(slope) => Some(slope),
            Self::Angle(angle) => slope_of_angle(angle),
        }
    }

    /// The slope of the pitch of edge `edge`, checked: it rises, and no steeper than
    /// [`MOST_SLOPE`].
    ///
    /// # Errors
    /// [`RoofError::NoRise`] for a slope or an angle of 0 or less; [`RoofError::TooSteep`] for one
    /// steeper than 10 in 1, or an angle of 90 degrees or more.
    pub fn checked(self, edge: &Id) -> Result<Slope, RoofError> {
        let no_rise = || RoofError::NoRise { edge: edge.clone() };
        let too_steep = || RoofError::TooSteep { edge: edge.clone() };
        let slope = match self {
            Self::Slope(slope) if slope.rise() <= 0 => return Err(no_rise()),
            Self::Angle(angle) if angle.units() <= 0 => return Err(no_rise()),
            other => other.slope().ok_or_else(too_steep)?,
        };
        let (most_rise, most_run) = MOST_SLOPE;
        // rise / run <= 10 / 1, compared crosswise in 128 bits.
        let steep = i128::from(slope.rise()).saturating_mul(i128::from(most_run))
            > i128::from(most_rise).saturating_mul(i128::from(slope.run()));
        if steep {
            return Err(too_steep());
        }
        Ok(slope)
    }

    /// The pitch as a construct-params value: a slope or an angle.
    #[must_use]
    pub fn value(self) -> Value {
        match self {
            Self::Slope(slope) => Value::Slope(slope),
            Self::Angle(angle) => Value::Angle(angle),
        }
    }

    /// The pitch in construct-params' stored spelling: `slope:1/4` or `angle:270000000`.
    #[must_use]
    pub fn stored(self) -> String {
        self.value().stored()
    }

    /// A pitch from construct-params' stored spelling of a slope or an angle; `None` for anything
    /// else.
    #[must_use]
    pub fn from_stored(text: &str) -> Option<Self> {
        match Value::from_stored(text)? {
            Value::Slope(slope) => Some(Self::Slope(slope)),
            Value::Angle(angle) => Some(Self::Angle(angle)),
            _ => None,
        }
    }
}
