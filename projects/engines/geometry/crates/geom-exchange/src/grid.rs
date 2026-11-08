//! Numbers to grid: a length or an angle from a file, given as decimal text or as a double, rounded
//! once to a whole count of grid steps or angle units, half to even, never through another
//! rounding.
//!
//! What it does: [`length`] and [`length_of_double`] take a length in a unit to steps of a grid
//! step; [`positive_length`] and [`positive_length_of_double`] do the same for a length the schema
//! says is positive (IFC's `IfcPositiveLengthMeasure`, a radius or a depth), refusing one that
//! rounds to zero with the finer step that would hold it; [`map_coordinate`] takes a map coordinate
//! as far from zero as a frame origin may be (2^60 steps), for frames from files; [`angle`] and
//! [`angle_of_double`] take an angle to section 17.2's angle units. Each returns the count and
//! whether it was exact ([`Gridded`]).
//!
//! **One rounding, done once, by the frame conversion.** The value v in a unit of u metres is
//! v·u·S steps (S steps per metre). That is exactly the height of a map point in a Map frame at
//! origin zero whose linear unit is u, and `geom-math`'s frame conversion
//! (`FrameRecord::to_frame_decimal` for decimals, `FrameRecord::to_frame` for doubles) already
//! rounds it once and correctly, ties to even, from the decimal's or the double's exact value: in
//! integers, never through `f64` for a decimal. This module calls it, so the kernel has one
//! correctly rounded conversion, not two. An angle in a unit of k angle units is converted the same
//! way, as a length of k/6,400,000 metres at q0 (a degree is 45/32 m), whose image is exactly v·k.
//!
//! Guarantees:
//!
//! - **One rounding, half to even, from the exact value.** The count is the exact value rounded
//!   once to the nearest whole count, ties to the even one, and [`Gridded::is_exact`] says whether
//!   it was already whole. A decimal never passes through a double.
//! - **Never out of range silently.** A length beyond ±(2^40 − 1) steps, a map coordinate beyond
//!   ±2^60 steps and an angle beyond ±(2^40 − 1) angle units are refused with
//!   [`ExchangeError::OutOfRange`], carrying the exactly rounded count.
//! - **A positive length is positive.** Zero or negative is [`ExchangeError::NotPositive`]; one
//!   that rounds to zero steps is [`ExchangeError::RoundsToZero`] with the first finer step that
//!   holds it, never a zero length.
//! - **The radian is never rounded here.** An angle in radians is
//!   [`ExchangeError::InexactAngleUnit`]: it stays a double (chapter 24.8, "Angles and trims").
//!
//! Method: chapter 24.8's "Numbers to grid"
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`), through the frame
//! conversion of chapter 17.1 (`projects/engines/geometry/architecture/17-survey-terrain-civil.md`),
//! which rounds to nearest, ties to even (IEEE 754-2019's roundTiesToEven, section 4.3.1,
//! <https://doi.org/10.1109/IEEESTD.2019.8766229>).
//!
//! Cost: one frame conversion per value: for a decimal, exact integer arithmetic of at most 1536
//! bits; for a double in metres, a double-double product and one exact rounding. [`map_coordinate`]
//! converts twice when the coordinate lies beyond 2^40 steps, the second time to learn whether it
//! was exact. [`positive_length`] converts at most twice more when the length rounds to zero. No
//! allocation.

use geom_math::frame::ORIGIN_MAX;
use geom_math::{CrsId, Decimal, DecimalPoint, FrameRecord, LinearUnit, MapPoint, Rounded};
use geom_num::NumError;
use geom_num::grid::{GridStep, P3};

use crate::error::ExchangeError;
use crate::unit::AngleUnit;

/// A value rounded once to a whole count (of grid steps, or of angle units), with whether no
/// rounding happened. Made only by this crate's conversions, so [`Gridded::is_exact`] is the truth
/// about the rounding that made it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gridded {
    count: i64,
    exact: bool,
}

impl Gridded {
    /// The count.
    #[must_use]
    pub const fn count(self) -> i64 {
        self.count
    }

    /// Whether the value was already a whole count, so no rounding happened.
    #[must_use]
    pub const fn is_exact(self) -> bool {
        self.exact
    }
}

/// A Map frame at `height` steps above zero whose map coordinates are in `unit`.
fn frame(unit: LinearUnit, step: GridStep, height: i64) -> Result<FrameRecord, ExchangeError> {
    Ok(FrameRecord::map(CrsId::Local(0), step, [0, 0, height])?.with_linear_unit(unit))
}

/// The height of a converted point, as a count.
fn height(r: Rounded<P3>) -> Gridded {
    Gridded {
        count: r.value().z.get(),
        exact: r.is_exact(),
    }
}

/// The decimal at the height of a map point.
fn at_height(value: Decimal) -> DecimalPoint {
    DecimalPoint {
        easting: Decimal::ZERO,
        northing: Decimal::ZERO,
        height: value,
    }
}

/// A length of `value` units of `unit` in steps of `step`, rounded once, half to even.
///
/// # Errors
///
/// [`ExchangeError::OutOfRange`] beyond ±(2^40 − 1) steps, with the exactly rounded count.
pub fn length(value: Decimal, unit: LinearUnit, step: GridStep) -> Result<Gridded, ExchangeError> {
    Ok(height(
        frame(unit, step, 0)?.to_frame_decimal(at_height(value))?,
    ))
}

/// A length given as a double, at its exact value, in steps of `step`, rounded once, half to even.
///
/// # Errors
///
/// [`ExchangeError::NotFinite`] for NaN or an infinity; [`ExchangeError::OutOfRange`] beyond
/// ±(2^40 − 1) steps, with the exactly rounded count.
pub fn length_of_double(
    value: f64,
    unit: LinearUnit,
    step: GridStep,
) -> Result<Gridded, ExchangeError> {
    if !value.is_finite() {
        return Err(ExchangeError::NotFinite);
    }
    let point = MapPoint {
        easting: 0.0,
        northing: 0.0,
        height: value,
    };
    Ok(height(frame(unit, step, 0)?.to_frame(point)?))
}

/// The grid steps finer than `step`, coarsest first.
fn finer(step: GridStep) -> &'static [GridStep] {
    match step {
        GridStep::Q0 => &[GridStep::Q1, GridStep::Q2],
        GridStep::Q1 => &[GridStep::Q2],
        GridStep::Q2 => &[],
    }
}

/// The first step finer than `step` at which `convert` gives a nonzero count.
fn holding(
    step: GridStep,
    convert: impl Fn(GridStep) -> Result<Gridded, ExchangeError>,
) -> Result<Option<GridStep>, ExchangeError> {
    for &s in finer(step) {
        if convert(s)?.count != 0 {
            return Ok(Some(s));
        }
    }
    Ok(None)
}

/// A length the schema says is positive, in steps of `step`, rounded once, half to even.
///
/// # Errors
///
/// [`ExchangeError::NotPositive`] for zero or a negative value; [`ExchangeError::RoundsToZero`]
/// if it rounds to zero steps, with the first finer step that holds it; and [`length`]'s errors.
pub fn positive_length(
    value: Decimal,
    unit: LinearUnit,
    step: GridStep,
) -> Result<Gridded, ExchangeError> {
    if value.significand() <= 0 {
        return Err(ExchangeError::NotPositive);
    }
    let got = length(value, unit, step)?;
    if got.count == 0 {
        let finer = holding(step, |s| length(value, unit, s))?;
        return Err(ExchangeError::RoundsToZero { finer });
    }
    Ok(got)
}

/// A positive length given as a double, in steps of `step`, rounded once, half to even.
///
/// # Errors
///
/// [`ExchangeError::NotFinite`] for NaN or an infinity; [`ExchangeError::NotPositive`] for zero
/// (either sign) or a negative value; [`ExchangeError::RoundsToZero`] as for [`positive_length`];
/// and [`length_of_double`]'s errors.
pub fn positive_length_of_double(
    value: f64,
    unit: LinearUnit,
    step: GridStep,
) -> Result<Gridded, ExchangeError> {
    if !value.is_finite() {
        return Err(ExchangeError::NotFinite);
    }
    if value <= 0.0 {
        return Err(ExchangeError::NotPositive);
    }
    let got = length_of_double(value, unit, step)?;
    if got.count == 0 {
        let finer = holding(step, |s| length_of_double(value, unit, s))?;
        return Err(ExchangeError::RoundsToZero { finer });
    }
    Ok(got)
}

/// A map coordinate (an easting, a northing or a height) in steps of `step`, rounded once, half to
/// even, as far from zero as a frame origin may be: ±2^60 steps (`ORIGIN_MAX`).
///
/// # Errors
///
/// [`ExchangeError::OutOfRange`] beyond ±2^60 steps, with the exactly rounded count.
pub fn map_coordinate(
    value: Decimal,
    unit: LinearUnit,
    step: GridStep,
) -> Result<Gridded, ExchangeError> {
    match frame(unit, step, 0)?.to_frame_decimal(at_height(value)) {
        Ok(r) => Ok(height(r)),
        Err(NumError::OutOfRange { value: count }) => {
            // The conversion reports the exactly rounded count. Within the origin range, a frame
            // at that height converts the value again, to its offset from the count: zero (an
            // offset of at most half a step rounds to the even zero), exact only when the value was
            // the count itself.
            let within = i64::try_from(count)
                .ok()
                .filter(|c| c.unsigned_abs() <= ORIGIN_MAX.unsigned_abs());
            let Some(count64) = within else {
                return Err(ExchangeError::OutOfRange { count });
            };
            let offset = height(frame(unit, step, count64)?.to_frame_decimal(at_height(value))?);
            if offset.count != 0 {
                return Err(ExchangeError::Numeric(NumError::BudgetExceeded));
            }
            Ok(Gridded {
                count: count64,
                exact: offset.exact,
            })
        }
        Err(e) => Err(e.into()),
    }
}

/// The length in metres whose image at q0 is `k` per unit: `k / 6,400,000` m, so an angle in a unit
/// of k angle units converts as a length.
fn angle_as_length(unit: AngleUnit) -> Result<LinearUnit, ExchangeError> {
    let k = unit.angle_units().ok_or(ExchangeError::InexactAngleUnit)?;
    let k = u64::try_from(k).map_err(|_| ExchangeError::Numeric(NumError::BudgetExceeded))?;
    Ok(LinearUnit::new(k, 6_400_000)?)
}

/// An angle of `value` in `unit`, in section 17.2's angle units (1/2,500 of an arc-second), rounded
/// once, half to even.
///
/// # Errors
///
/// [`ExchangeError::InexactAngleUnit`] for the radian; [`ExchangeError::OutOfRange`] beyond
/// ±(2^40 − 1) angle units (about 339 turns), with the exactly rounded count.
pub fn angle(value: Decimal, unit: AngleUnit) -> Result<Gridded, ExchangeError> {
    length(value, angle_as_length(unit)?, GridStep::Q0)
}

/// An angle given as a double, at its exact value, in angle units, rounded once, half to even.
///
/// # Errors
///
/// As [`angle`], and [`ExchangeError::NotFinite`] for NaN or an infinity.
pub fn angle_of_double(value: f64, unit: AngleUnit) -> Result<Gridded, ExchangeError> {
    length_of_double(value, angle_as_length(unit)?, GridStep::Q0)
}
