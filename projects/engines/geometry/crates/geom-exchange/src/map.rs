//! Frames from files: IFC's `IfcMapConversion` and `IfcMapConversionScaled` made into chapter
//! 17.1's frame record, with local coordinates becoming frame coordinates unchanged.
//!
//! What it does: [`MapConversion`] holds the conversion's numbers as the file gives them, as exact
//! decimals. [`place`] turns it, with the target system's id, the map's length unit, the project's
//! length unit and the grid step, into a [`Placement`]: the [`FrameRecord`], whether its origin
//! and rotation are exact, and the factors the frame cannot hold.
//!
//! **The correspondence.** IFC 4.3 maps a local point (x, y, z), in the project's length unit
//! U, to the map (in the map's unit u) as E = `Eastings` + `Scale`·(`FactorX`·cos θ·x −
//! `FactorY`·sin θ·y), N = `Northings` + `Scale`·(`FactorX`·sin θ·x + `FactorY`·cos θ·y) and
//! H = `OrthogonalHeight` + `Scale`·`FactorZ`·z, with θ the direction of (`XAxisAbscissa`,
//! `XAxisOrdinate`); `Scale` converts units, the factors scale coordinates. A frame (chapter 17.1)
//! holds E·u·S = `O_E` + k·(cos θ·x' − sin θ·y') for frame coordinates x' = x·U·S in steps, and
//! heights unscaled. So:
//!
//! - the origin is (`Eastings`, `Northings`, `OrthogonalHeight`)·u·S, each rounded once
//!   ([`crate::grid::map_coordinate`]);
//! - the rotation is the direction (`XAxisAbscissa`, `XAxisOrdinate`);
//! - k = `Scale`·u/U times `FactorX` is the frame's horizontal scale when `FactorX` equals
//!   `FactorY`, and the frame holds heights unscaled, so `Scale`·u/U·`FactorZ` must be one;
//!   whatever the frame cannot hold (unequal x and y factors, or a height factor other than one)
//!   is returned as [`Placement::residual`]: the factors the importer applies to local coordinates
//!   before they enter the frame, once, rounded and recorded (chapter 24.8). A consistent file,
//!   whose `Scale` only converts its units, has k = 1.
//!
//! **The rotation's form.** The frame stores an integer direction (c, s) with |c|, |s| ≤ 2^31
//! (chapter 17.1). The two decimals are put over one power of ten and reduced; when both then fit
//! 31 bits the direction is exact. Otherwise the larger magnitude becomes 2^30 and the other is its
//! exact ratio times 2^30, rounded once, half to even, by binary long division; the angle then
//! moves by at most half of 2^−30 in its tangent, so less than 2^−31 rad, and
//! [`Placement::is_rotation_exact`] is false. Chapter 24.8 names the nearest angle unit as the
//! final form; that waits for the angle-unit frame of piece "Angle units and core COGO", and the
//! two numbers stay with the caller's recipe so an unedited frame writes back exactly.
//!
//! Guarantees:
//!
//! - **Nothing guessed.** Every number comes from the conversion as given; an omitted Scale or
//!   factor is IFC's documented 1.0, and an omitted direction is the easting axis.
//! - **Exact where it can be, and says so.** The origin is exact when each value lies on the grid,
//!   the rotation when the direction's reduced integers fit 31 bits, the scale always (an exact
//!   ratio, or refused); [`Placement`] reports each.
//! - **Heights are never scaled by the frame.** A height factor other than one is returned in the
//!   residual, never folded into the frame.
//! - **Refused, not clamped.** An origin beyond ±2^60 steps is [`ExchangeError::OutOfRange`]; a
//!   zero direction is [`ExchangeError::ZeroDirection`]; a scale or factor that is not positive or
//!   whose exact ratio needs more than 64 bits is [`ExchangeError::ScaleOutOfRange`].
//!
//! Method: chapter 24.8's "Frames from files" and chapter 17.1's frame record
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`,
//! `projects/engines/geometry/architecture/17-survey-terrain-civil.md`), with `IfcMapConversion`
//! and `IfcMapConversionScaled` as IFC 4.3 defines them
//! (<https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcRepresentationResource/Entities/IfcMapConversionScaled.md>);
//! binary long division (D. E. Knuth, *The Art of Computer Programming*, vol. 2, section 4.3.1,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>).
//!
//! Consulted: IFC 4.3 schema documentation (buildingSMART), CC-BY-ND-4.0,
//!   <https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcRepresentationResource/Entities/IfcMapConversionScaled.md>,
//!   for the order and meaning of Scale, the three factors, the rotation and the translation. No
//!   text adapted.
//!
//! Cost: three map coordinates ([`crate::grid::map_coordinate`]), a greatest common divisor of
//! 128-bit integers for the direction and each scale, and for an inexact direction 31 steps of
//! long division in 384-bit integers. No allocation.

#![allow(
    clippy::many_single_char_names,
    reason = "variable names follow the formulas of the method each function cites"
)]

use geom_math::{CrsId, Decimal, FrameRecord, LinearUnit, Rotation, Scale};
use geom_num::NumError;
use geom_num::grid::GridStep;
use geom_num::wide::I384;

use crate::error::ExchangeError;
use crate::grid::map_coordinate;
use crate::unit::gcd;

/// An `IfcMapConversion` (or `IfcMapConversionScaled`) as the file gives it, every number an exact
/// decimal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapConversion {
    /// The map easting of the local origin, in the map's unit.
    pub eastings: Decimal,
    /// The map northing of the local origin, in the map's unit.
    pub northings: Decimal,
    /// The height of the local origin above the vertical datum, in the map's unit.
    pub orthogonal_height: Decimal,
    /// The local x axis in map coordinates, (`XAxisAbscissa`, `XAxisOrdinate`); `None` for the
    /// easting axis.
    pub x_axis: Option<[Decimal; 2]>,
    /// The unit conversion from the project's unit to the map's; `None` for 1.0.
    pub scale: Option<Decimal>,
    /// `IfcMapConversionScaled`'s `FactorX`, `FactorY` and `FactorZ`; `None` for a plain
    /// `IfcMapConversion`.
    pub factors: Option<[Decimal; 3]>,
}

/// A frame record made from a map conversion, what of it was exact, and what it cannot hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Placement {
    frame: FrameRecord,
    origin_exact: bool,
    rotation_exact: bool,
    residual: Option<[Scale; 3]>,
}

impl Placement {
    /// The frame record.
    #[must_use]
    pub const fn frame(&self) -> FrameRecord {
        self.frame
    }

    /// Whether every origin coordinate lay on the grid.
    #[must_use]
    pub const fn is_origin_exact(&self) -> bool {
        self.origin_exact
    }

    /// Whether the frame's direction is exactly the file's.
    #[must_use]
    pub const fn is_rotation_exact(&self) -> bool {
        self.rotation_exact
    }

    /// The factors along local x, y and z that the frame cannot hold, which the importer applies to
    /// local coordinates before they enter the frame; `None` when every one is one.
    #[must_use]
    pub const fn residual(&self) -> Option<[Scale; 3]> {
        self.residual
    }
}

/// A positive exact ratio, `num / den`, reduced, in 128 bits.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Ratio {
    num: u128,
    den: u128,
}

impl Ratio {
    const ONE: Ratio = Ratio { num: 1, den: 1 };

    fn reduced(num: u128, den: u128) -> Result<Ratio, ExchangeError> {
        if num == 0 || den == 0 {
            return Err(ExchangeError::ScaleOutOfRange);
        }
        let g = gcd(num, den);
        Ok(Ratio {
            num: num.checked_div(g).ok_or(ExchangeError::ScaleOutOfRange)?,
            den: den.checked_div(g).ok_or(ExchangeError::ScaleOutOfRange)?,
        })
    }

    /// A positive decimal as a ratio: m·10^e.
    fn of(d: Decimal) -> Result<Ratio, ExchangeError> {
        let m = u128::try_from(d.significand()).map_err(|_| ExchangeError::ScaleOutOfRange)?;
        let ten = 10_u128
            .checked_pow(d.exponent().unsigned_abs())
            .ok_or(ExchangeError::ScaleOutOfRange)?;
        if d.exponent() >= 0 {
            Ratio::reduced(m.checked_mul(ten).ok_or(ExchangeError::ScaleOutOfRange)?, 1)
        } else {
            Ratio::reduced(m, ten)
        }
    }

    /// The product, reduced across before multiplying so that parts stay small.
    fn times(self, other: Ratio) -> Result<Ratio, ExchangeError> {
        let g1 = gcd(self.num, other.den);
        let g2 = gcd(other.num, self.den);
        let part = |a: u128, g: u128| a.checked_div(g).ok_or(ExchangeError::ScaleOutOfRange);
        let num = part(self.num, g1)?
            .checked_mul(part(other.num, g2)?)
            .ok_or(ExchangeError::ScaleOutOfRange)?;
        let den = part(self.den, g2)?
            .checked_mul(part(other.den, g1)?)
            .ok_or(ExchangeError::ScaleOutOfRange)?;
        Ratio::reduced(num, den)
    }

    /// The frame's scale type: both parts must fit 64 bits.
    fn scale(self) -> Result<Scale, ExchangeError> {
        match (u64::try_from(self.num), u64::try_from(self.den)) {
            (Ok(n), Ok(d)) => Scale::new(n, d).map_err(|_| ExchangeError::ScaleOutOfRange),
            _ => Err(ExchangeError::ScaleOutOfRange),
        }
    }
}

/// The largest magnitude of a rounded direction's component, 2^30: within an `i32`, so the frame
/// takes it as it is.
const DIRECTION_ONE: u32 = 1 << 30;

/// A defect: a step the bit budgets rule out failed.
fn defect() -> ExchangeError {
    ExchangeError::Numeric(NumError::BudgetExceeded)
}

/// Two nonzero decimals' magnitudes as integers over one power of ten, in 384 bits: the larger
/// exponent's significand (below 10^38 < 2^127) is multiplied by 10^gap, below 2^127·10^48 < 2^287.
/// `None` for a gap past 48: then the value with the larger exponent is more than 10^10 times the
/// other, whose share of 2^30 rounds to zero.
fn aligned(a: Decimal, b: Decimal) -> Result<Option<[I384; 2]>, ExchangeError> {
    let low = a.exponent().min(b.exponent());
    let up = |d: Decimal| -> Result<Option<I384>, ExchangeError> {
        let gap = d.exponent().checked_sub(low).ok_or_else(defect)?;
        if gap > 48 {
            return Ok(None);
        }
        let m = i128::try_from(d.significand().unsigned_abs()).map_err(|_| defect())?;
        let mut v = I384::from_i128(m);
        let ten = I384::from_i64(10);
        for _ in 0..gap {
            v = v.checked_mul(&ten).ok_or_else(defect)?;
        }
        Ok(Some(v))
    };
    match (up(a)?, up(b)?) {
        (Some(x), Some(y)) => Ok(Some([x, y])),
        _ => Ok(None),
    }
}

/// round(small·2^30 / big), half to even, for 0 ≤ small ≤ big and big > 0, by binary long
/// division: 30 quotient bits, then the remainder against half the divisor.
fn ratio_bits(small: I384, big: I384) -> Result<u32, ExchangeError> {
    let mut q = 0_u32;
    let mut r = small;
    for _ in 0..30 {
        // r < 2·big < 2^288 before each doubling, so it fits 384 bits.
        r = r.checked_add(&r).ok_or_else(defect)?;
        q = q.checked_mul(2).ok_or_else(defect)?;
        if r >= big {
            r = r.checked_sub(&big).ok_or_else(defect)?;
            q = q.checked_add(1).ok_or_else(defect)?;
        }
    }
    let twice = r.checked_add(&r).ok_or_else(defect)?;
    if twice > big || (twice == big && !q.is_multiple_of(2)) {
        q = q.checked_add(1).ok_or_else(defect)?;
    }
    Ok(q)
}

/// `magnitude`, negated when `negative`.
fn with_sign(magnitude: i32, negative: bool) -> Result<i32, ExchangeError> {
    if negative {
        magnitude.checked_neg().ok_or_else(defect)
    } else {
        Ok(magnitude)
    }
}

/// The frame direction of the axis (a, b), and whether it is exact.
fn direction(axis: Option<[Decimal; 2]>) -> Result<(Rotation, bool), ExchangeError> {
    let Some([a, b]) = axis else {
        return Ok((Rotation::IDENTITY, true));
    };
    let (na, nb) = (a.significand() < 0, b.significand() < 0);
    match (a.is_zero(), b.is_zero()) {
        (true, true) => return Err(ExchangeError::ZeroDirection),
        (true, false) => return Ok((Rotation::direction(0, with_sign(1, nb)?)?, true)),
        (false, true) => return Ok((Rotation::direction(with_sign(1, na)?, 0)?, true)),
        (false, false) => {}
    }
    let Some([x, y]) = aligned(a, b)? else {
        // One component is less than 10^−10 of the other: the direction is the other's axis,
        // rounded.
        let r = if a.exponent() > b.exponent() {
            Rotation::direction(with_sign(1, na)?, 0)?
        } else {
            Rotation::direction(0, with_sign(1, nb)?)?
        };
        return Ok((r, false));
    };
    // Exact when both fit 128 bits and, reduced by their greatest common divisor, 31.
    if let (Some(xi), Some(yi)) = (x.to_i128(), y.to_i128()) {
        let (xu, yu) = (xi.unsigned_abs(), yi.unsigned_abs());
        let g = gcd(xu, yu);
        let xr = xu.checked_div(g).and_then(|v| i32::try_from(v).ok());
        let yr = yu.checked_div(g).and_then(|v| i32::try_from(v).ok());
        if let (Some(xr), Some(yr)) = (xr, yr) {
            let r = Rotation::direction(with_sign(xr, na)?, with_sign(yr, nb)?)?;
            return Ok((r, true));
        }
    }
    // Rounded: the larger magnitude becomes 2^30, the other its exact share of 2^30, rounded once.
    let one = i32::try_from(DIRECTION_ONE).map_err(|_| defect())?;
    let (along_a, big, small) = if x >= y { (true, x, y) } else { (false, y, x) };
    let other = i32::try_from(ratio_bits(small, big)?).map_err(|_| defect())?;
    let (c, s) = if along_a {
        (with_sign(one, na)?, with_sign(other, nb)?)
    } else {
        (with_sign(other, na)?, with_sign(one, nb)?)
    };
    Ok((Rotation::direction(c, s)?, false))
}

/// The frame record of a map conversion (see the module documentation): `crs` the target map's
/// system, `map_unit` its length unit, `project_unit` the file's length unit, `step` the grid step.
///
/// # Errors
///
/// [`ExchangeError::OutOfRange`] for an origin coordinate beyond ±2^60 steps;
/// [`ExchangeError::ZeroDirection`] for an x axis of (0, 0); [`ExchangeError::ScaleOutOfRange`] for
/// a scale or factor that is not positive or whose exact ratio needs more than 64 bits.
pub fn place(
    conversion: &MapConversion,
    crs: CrsId,
    map_unit: LinearUnit,
    project_unit: LinearUnit,
    step: GridStep,
) -> Result<Placement, ExchangeError> {
    let e = map_coordinate(conversion.eastings, map_unit, step)?;
    let n = map_coordinate(conversion.northings, map_unit, step)?;
    let h = map_coordinate(conversion.orthogonal_height, map_unit, step)?;
    let (rotation, rotation_exact) = direction(conversion.x_axis)?;
    // k = Scale·u/U: the unit conversion in the frame's terms, one for a consistent file.
    let scale = match conversion.scale {
        Some(d) => Ratio::of(d)?,
        None => Ratio::ONE,
    };
    let units = Ratio::reduced(
        u128::from(map_unit.num())
            .checked_mul(u128::from(project_unit.den()))
            .ok_or(ExchangeError::ScaleOutOfRange)?,
        u128::from(map_unit.den())
            .checked_mul(u128::from(project_unit.num()))
            .ok_or(ExchangeError::ScaleOutOfRange)?,
    )?;
    let k = scale.times(units)?;
    let [fx, fy, fz] = match conversion.factors {
        Some([x, y, z]) => [Ratio::of(x)?, Ratio::of(y)?, Ratio::of(z)?],
        None => [Ratio::ONE; 3],
    };
    let (kx, ky, kz) = (k.times(fx)?, k.times(fy)?, k.times(fz)?);
    let (horizontal, residual) = if kx == ky {
        (kx.scale()?, [Ratio::ONE, Ratio::ONE, kz])
    } else {
        (Scale::ONE, [kx, ky, kz])
    };
    let [rx, ry, rz] = residual;
    let residual = if [rx, ry, rz].iter().all(|r| *r == Ratio::ONE) {
        None
    } else {
        Some([rx.scale()?, ry.scale()?, rz.scale()?])
    };
    let frame = FrameRecord::new(
        crs,
        step,
        [e.count(), n.count(), h.count()],
        rotation,
        horizontal,
    )?
    .with_linear_unit(map_unit);
    Ok(Placement {
        frame,
        origin_exact: e.is_exact() && n.is_exact() && h.is_exact(),
        rotation_exact,
        residual,
    })
}
