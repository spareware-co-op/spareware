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
//! **The short decimals of a file, in 128 bits.** That conversion carries 1536-bit integers, tens
//! of microseconds a value, for any decimal. A file's numbers are short: a decimal m·10^e with
//! m·p·S·10^e (e ≥ 0) or q·10^−e (e < 0) within 128 bits, for a unit of p/q metres, is the
//! fraction (m·p·S)/(q·10^−e) of two `u128`s, whose quotient and remainder decide the rounding, ties
//! to even, exactly. Every metre, millimetre, inch or foot value of up to about 20 significant
//! digits takes this path; anything longer takes the frame conversion. A double m·2^k is taken the
//! same way, as (m·p·S)/(q·2^−k), whenever that fits (every double of the units above from about
//! 2^−60 to 2^40 in size). Both paths are exact, so the count is the same either way.
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
//! Cost: a short decimal, a few 128-bit multiplications and one division; otherwise one frame
//! conversion per value: for a decimal, exact integer arithmetic of at most 1536 bits; for a double
//! in metres, a double-double product and one exact rounding. [`map_coordinate`]
//! converts twice when the coordinate lies beyond 2^40 steps, the second time to learn whether it
//! was exact. [`positive_length`] converts at most twice more when the length rounds to zero. No
//! allocation.

use geom_math::frame::{ORIGIN_MAX, steps_per_metre};
use geom_math::{CrsId, Decimal, DecimalPoint, FrameRecord, LinearUnit, MapPoint, Rounded};
use geom_num::NumError;
use geom_num::grid::{COORD_MAX, GridStep, P3};

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

/// num/den rounded to the nearest whole number, ties to even, with the sign of `negative`, and
/// whether it was whole; `None` if a step does not fit.
fn rounded(num: u128, den: u128, negative: bool) -> Option<(i128, bool)> {
    let (whole, rest) = (num.checked_div(den)?, num.checked_rem(den)?);
    // Half to even: up when the remainder is more than half the divisor, or exactly half with an
    // odd quotient. rest < den, so den − rest does not underflow.
    let other = den.checked_sub(rest)?;
    let up = rest > other || (rest == other && !whole.is_multiple_of(2));
    let magnitude = if up { whole.checked_add(1)? } else { whole };
    let count = i128::try_from(magnitude).ok()?;
    let count = if negative {
        count.checked_neg()?
    } else {
        count
    };
    Some((count, rest == 0))
}

/// `value`·u·S rounded to the nearest whole count, ties to even, and whether it was whole, when the
/// fraction fits 128 bits (see the module documentation); `None` otherwise.
fn short(value: Decimal, unit: LinearUnit, step: GridStep) -> Option<(i128, bool)> {
    let e = value.exponent();
    let ten = 10_u128.checked_pow(e.unsigned_abs())?;
    let ps = u128::from(unit.num()).checked_mul(u128::from(steps_per_metre(step)))?;
    let top = value.significand().unsigned_abs().checked_mul(ps)?;
    let q = u128::from(unit.den());
    let (num, den) = if e >= 0 {
        (top.checked_mul(ten)?, q)
    } else {
        (top, q.checked_mul(ten)?)
    };
    rounded(num, den, value.significand() < 0)
}

/// The same for a finite double x = m·2^k, its exact value (IEEE 754's binary64 encoding: the
/// significand with its hidden bit, and the exponent less 1075; a subnormal's is −1074).
fn short_double(value: f64, unit: LinearUnit, step: GridStep) -> Option<(i128, bool)> {
    let bits = value.to_bits();
    let field = (bits >> 52) & 0x7ff;
    let fraction = bits & ((1 << 52) - 1);
    let (m, k) = if field == 0 {
        (fraction, -1074_i32)
    } else {
        (
            fraction | (1 << 52),
            i32::try_from(field).ok()?.checked_sub(1075)?,
        )
    };
    let two = 1_u128.checked_shl(k.unsigned_abs())?;
    let ps = u128::from(unit.num()).checked_mul(u128::from(steps_per_metre(step)))?;
    let top = u128::from(m).checked_mul(ps)?;
    let q = u128::from(unit.den());
    let (num, den) = if k >= 0 {
        (top.checked_mul(two)?, q)
    } else {
        (top, q.checked_mul(two)?)
    };
    rounded(num, den, value.is_sign_negative())
}

/// A short decimal's count within `limit`, or refused as out of range.
fn within(count: i128, exact: bool, limit: i64) -> Result<Gridded, ExchangeError> {
    match i64::try_from(count) {
        Ok(c) if c.unsigned_abs() <= limit.unsigned_abs() => Ok(Gridded { count: c, exact }),
        _ => Err(ExchangeError::OutOfRange { count }),
    }
}

/// A length of `value` units of `unit` in steps of `step`, rounded once, half to even.
///
/// # Errors
///
/// [`ExchangeError::OutOfRange`] beyond ±(2^40 − 1) steps, with the exactly rounded count.
pub fn length(value: Decimal, unit: LinearUnit, step: GridStep) -> Result<Gridded, ExchangeError> {
    if let Some((count, exact)) = short(value, unit, step) {
        return within(count, exact, COORD_MAX);
    }
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
    if let Some((count, exact)) = short_double(value, unit, step) {
        return within(count, exact, COORD_MAX);
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
    if let Some((count, exact)) = short(value, unit, step) {
        return within(count, exact, ORIGIN_MAX);
    }
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
