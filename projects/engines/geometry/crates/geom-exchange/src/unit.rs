//! The exact unit table: every length unit an exchange file declares as an exact ratio of metres
//! and of grid steps, and every angle unit as a whole count of angle units, or none.
//!
//! What it does: [`SiPrefix`] is the sixteen prefixes IFC's `IfcSIPrefix` and ISO 10303-41's
//! `si_prefix` name, and [`metre`] the metre with one of them. [`NamedLength`] holds the
//! conversion-based length units by their definitions: the inch, foot, yard, mile and thou of the
//! international yard and pound, and the US survey foot. [`conversion_based`] makes the unit a
//! file defines as a decimal factor of another (STEP's and IFC's conversion-based units), so a
//! file's own factor always decides, never a name. [`steps_per`] gives a unit's exact ratio of grid
//! steps at a step, and [`AngleUnit::angle_units`] an angle unit's count of section 17.2's angle
//! units (1/2,500 of an arc-second; 3,240,000,000 to a turn).
//!
//! The table, at q0 (6,400,000 steps per metre) and q1 (160,000,000):
//!
//! | Unit | Metres | Steps at q0 | Steps at q1 |
//! | --- | --- | --- | --- |
//! | Millimetre | 1/1,000 | 6,400 | 160,000 |
//! | Metre | 1 | 6,400,000 | 160,000,000 |
//! | Inch | 127/5,000 | 162,560 = 2^8·5·127 | 4,064,000 |
//! | Foot | 381/1,250 | 1,950,720 | 48,768,000 |
//! | US survey foot | 1,200/3,937 | 7,680,000,000/3,937 | 192,000,000,000/3,937 |
//!
//! So a decimal inch is exact at q0 only in whole 1/1,280 inches (162,560 steps over 10^k is whole
//! only for k ≤ 1 and multiples of 1/1,280), a foot to 0.1 ft, a millimetre to 1/6,400 mm, and a US
//! survey foot only in multiples of 3,937 feet (chapter 24.8's table).
//!
//! Guarantees:
//!
//! - **Exact, from the definitions.** Every length unit is the exact reduced ratio its defining
//!   document gives: the international yard of 0.9144 m and its inch, foot, mile and thou, and the
//!   US survey foot of 1,200/3,937 m. No unit is rounded.
//! - **The file decides.** [`conversion_based`] multiplies the file's decimal factor into its base
//!   unit exactly; nothing here maps a unit's name to a factor.
//! - **In range or refused.** A unit outside 2^−32 to 2^32 metres (a frame's linear unit range) is
//!   refused with [`ExchangeError::UnitOutOfRange`], never rounded into range.
//! - **Angles in whole units or not at all.** The degree, gon, arc-minute and arc-second are whole
//!   counts of angle units; the radian is not, and says so.
//!
//! Method: the definitions of NIST Special Publication 811 (2008 edition), "Guide for the Use of
//! the International System of Units", Appendix B (<https://doi.org/10.6028/NIST.SP.811e2008>): 1 in
//! = 2.54 cm, 1 ft = 0.3048 m, 1 yd = 0.9144 m, 1 mi = 1,609.344 m, 1 mil = 0.001 in exactly, and
//! the US survey foot = 1,200/3,937 m, which NIST and NOAA deprecated from 1 January 2023
//! (Federal Register 85 FR 62698, <https://www.federalregister.gov/d/2020-21902>) and which survey
//! files keep using; the SI prefixes of the BIPM's SI Brochure, 9th edition (2019), section 3
//! (<https://www.bipm.org/en/publications/si-brochure>), as IFC's `IfcSIPrefix`
//! (<https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcMeasureResource/Types/IfcSIPrefix.md>)
//! lists them; angle units as section 17.2 defines them
//! (`projects/engines/geometry/architecture/17-survey-terrain-civil.md`); exact rationals reduced
//! by Euclid's algorithm (D. E. Knuth, *The Art of Computer Programming*, vol. 2, section 4.5.2,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>).
//!
//! Consulted: IFC 4.3 schema documentation (buildingSMART), CC-BY-ND-4.0,
//!   <https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcMeasureResource/Types/IfcSIPrefix.md>,
//!   for the list of SI prefixes an IFC file may name. No text copied.
//! Consulted: solidcore (step-import), Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/78d1eb12d222652219313c12787b65389c4d9db8/crates/step-import/src/map.rs>,
//!   for how a STEP reader resolves `si_unit` prefixes and `conversion_based_unit` factors
//!   (`unit_factor`). Its factors are doubles; here every factor is an exact ratio.
//!
//! Cost: constant time; [`metre`], [`conversion_based`] and [`steps_per`] reduce by a greatest
//! common divisor (at most 186 remainder steps of 128-bit integers). No allocation.

use geom_math::{Decimal, LinearUnit};
use geom_num::NumError;
use geom_num::grid::GridStep;

use geom_math::frame::steps_per_metre;

use crate::error::ExchangeError;

/// An SI prefix, as IFC's `IfcSIPrefix` and ISO 10303-41's `si_prefix` name them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SiPrefix {
    /// 10^18.
    Exa,
    /// 10^15.
    Peta,
    /// 10^12.
    Tera,
    /// 10^9.
    Giga,
    /// 10^6.
    Mega,
    /// 10^3.
    Kilo,
    /// 10^2.
    Hecto,
    /// 10^1.
    Deca,
    /// 10^−1.
    Deci,
    /// 10^−2.
    Centi,
    /// 10^−3.
    Milli,
    /// 10^−6.
    Micro,
    /// 10^−9.
    Nano,
    /// 10^−12.
    Pico,
    /// 10^−15.
    Femto,
    /// 10^−18.
    Atto,
}

impl SiPrefix {
    /// Every prefix, largest first.
    pub const ALL: [SiPrefix; 16] = [
        SiPrefix::Exa,
        SiPrefix::Peta,
        SiPrefix::Tera,
        SiPrefix::Giga,
        SiPrefix::Mega,
        SiPrefix::Kilo,
        SiPrefix::Hecto,
        SiPrefix::Deca,
        SiPrefix::Deci,
        SiPrefix::Centi,
        SiPrefix::Milli,
        SiPrefix::Micro,
        SiPrefix::Nano,
        SiPrefix::Pico,
        SiPrefix::Femto,
        SiPrefix::Atto,
    ];

    /// The prefix's power of ten.
    #[must_use]
    pub const fn power(self) -> i32 {
        match self {
            SiPrefix::Exa => 18,
            SiPrefix::Peta => 15,
            SiPrefix::Tera => 12,
            SiPrefix::Giga => 9,
            SiPrefix::Mega => 6,
            SiPrefix::Kilo => 3,
            SiPrefix::Hecto => 2,
            SiPrefix::Deca => 1,
            SiPrefix::Deci => -1,
            SiPrefix::Centi => -2,
            SiPrefix::Milli => -3,
            SiPrefix::Micro => -6,
            SiPrefix::Nano => -9,
            SiPrefix::Pico => -12,
            SiPrefix::Femto => -15,
            SiPrefix::Atto => -18,
        }
    }
}

/// The metre, or the metre with an SI prefix (`None` is the metre itself).
///
/// # Errors
///
/// [`ExchangeError::UnitOutOfRange`] for tera and above and pico and below, which lie outside a
/// frame's 2^−32 to 2^32 metres.
pub fn metre(prefix: Option<SiPrefix>) -> Result<LinearUnit, ExchangeError> {
    let power = prefix.map_or(0, SiPrefix::power);
    // |power| ≤ 18, so 10^|power| < 2^60 fits u64.
    let ten = 10_u64
        .checked_pow(power.unsigned_abs())
        .ok_or(ExchangeError::UnitOutOfRange)?;
    let unit = if power >= 0 {
        LinearUnit::new(ten, 1)
    } else {
        LinearUnit::new(1, ten)
    };
    unit.map_err(|_| ExchangeError::UnitOutOfRange)
}

/// A length unit defined by a conversion from the metre: the international yard and pound's units
/// (NIST SP 811, Appendix B), and the US survey foot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NamedLength {
    /// The inch, 0.0254 m exactly.
    Inch,
    /// The foot, 0.3048 m exactly.
    Foot,
    /// The yard, 0.9144 m exactly.
    Yard,
    /// The international mile, 1,609.344 m exactly.
    Mile,
    /// The thou or mil, 0.001 inch: 0.0000254 m exactly.
    Thou,
    /// The US survey foot, 1,200/3,937 m exactly; deprecated from 1 January 2023, and still the
    /// unit of many state plane coordinate files.
    UsSurveyFoot,
}

impl NamedLength {
    /// Every named unit.
    pub const ALL: [NamedLength; 6] = [
        NamedLength::Inch,
        NamedLength::Foot,
        NamedLength::Yard,
        NamedLength::Mile,
        NamedLength::Thou,
        NamedLength::UsSurveyFoot,
    ];

    /// The unit as an exact ratio of metres, reduced: inch 127/5,000, foot 381/1,250, yard
    /// 1,143/1,250, mile 201,168/125, thou 127/5,000,000, US survey foot 1,200/3,937.
    ///
    /// # Errors
    ///
    /// None in practice: each ratio lies inside a frame's range. [`ExchangeError::Numeric`] would
    /// report a defect in `geom-math`'s unit type.
    pub fn unit(self) -> Result<LinearUnit, ExchangeError> {
        let (num, den) = match self {
            NamedLength::Inch => (127, 5_000),
            NamedLength::Foot => (381, 1_250),
            NamedLength::Yard => (1_143, 1_250),
            NamedLength::Mile => (201_168, 125),
            NamedLength::Thou => (127, 5_000_000),
            NamedLength::UsSurveyFoot => (1_200, 3_937),
        };
        Ok(LinearUnit::new(num, den)?)
    }
}

/// Euclid's greatest common divisor of two 128-bit integers; 0 only for (0, 0).
pub(crate) fn gcd(a: u128, b: u128) -> u128 {
    let (mut a, mut b) = (a, b);
    // Each step at least halves the larger of the two every second step: at most 186 steps for
    // 128-bit integers (Lamé's bound).
    while b != 0 {
        let r = a.checked_rem(b).unwrap_or(0);
        a = b;
        b = r;
    }
    a
}

/// The unit a file defines as `factor` of `base`: STEP's `conversion_based_unit` and IFC's
/// `IfcConversionBasedUnit`, whose `MEASURE_WITH_UNIT` gives a decimal factor of another unit
/// (0.0254 of a metre for an inch). The product is exact.
///
/// # Errors
///
/// [`ExchangeError::ScaleOutOfRange`] if the factor is zero or negative;
/// [`ExchangeError::UnitOutOfRange`] if the product lies outside 2^−32 to 2^32 metres or its
/// reduced ratio needs more than 64 bits in either part.
pub fn conversion_based(factor: Decimal, base: LinearUnit) -> Result<LinearUnit, ExchangeError> {
    if factor.significand() <= 0 {
        return Err(ExchangeError::ScaleOutOfRange);
    }
    let of_factor = LinearUnit::from_decimal(factor).map_err(|e| match e {
        NumError::OutsideDomain => ExchangeError::UnitOutOfRange,
        other => ExchangeError::Numeric(other),
    })?;
    // (a/b)·(c/d) with every part below 2^64: each product is below 2^128.
    let num = u128::from(of_factor.num())
        .checked_mul(u128::from(base.num()))
        .ok_or(ExchangeError::UnitOutOfRange)?;
    let den = u128::from(of_factor.den())
        .checked_mul(u128::from(base.den()))
        .ok_or(ExchangeError::UnitOutOfRange)?;
    let g = gcd(num, den);
    let (num, den) = (
        num.checked_div(g).ok_or(ExchangeError::UnitOutOfRange)?,
        den.checked_div(g).ok_or(ExchangeError::UnitOutOfRange)?,
    );
    match (u64::try_from(num), u64::try_from(den)) {
        (Ok(n), Ok(d)) => LinearUnit::new(n, d).map_err(|_| ExchangeError::UnitOutOfRange),
        _ => Err(ExchangeError::UnitOutOfRange),
    }
}

/// A unit's exact count of grid steps, `num / den`, reduced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StepsPer {
    num: u128,
    den: u128,
}

impl StepsPer {
    /// The numerator.
    #[must_use]
    pub const fn num(self) -> u128 {
        self.num
    }

    /// The denominator.
    #[must_use]
    pub const fn den(self) -> u128 {
        self.den
    }

    /// The count, when it is a whole number of steps.
    #[must_use]
    pub const fn whole(self) -> Option<u128> {
        if self.den == 1 { Some(self.num) } else { None }
    }
}

/// How many grid steps of `step` one `unit` is, exactly: `unit · S` for S steps per metre.
///
/// # Errors
///
/// [`ExchangeError::Numeric`] only for a defect: a unit's parts are below 2^64 and S below 2^32,
/// so the product fits 128 bits.
pub fn steps_per(unit: LinearUnit, step: GridStep) -> Result<StepsPer, ExchangeError> {
    let num = u128::from(unit.num())
        .checked_mul(u128::from(steps_per_metre(step)))
        .ok_or(ExchangeError::Numeric(NumError::BudgetExceeded))?;
    let den = u128::from(unit.den());
    let g = gcd(num, den);
    Ok(StepsPer {
        num: num
            .checked_div(g)
            .ok_or(ExchangeError::Numeric(NumError::BudgetExceeded))?,
        den: den
            .checked_div(g)
            .ok_or(ExchangeError::Numeric(NumError::BudgetExceeded))?,
    })
}

/// Section 17.2's angle units in one turn: 1/2,500 of an arc-second, 360·3,600·2,500.
pub const ANGLE_UNITS_PER_TURN: i64 = 3_240_000_000;

/// An angle unit a file may declare.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AngleUnit {
    /// The degree, a 360th of a turn.
    Degree,
    /// The gon (grad), a 400th of a turn.
    Gon,
    /// The arc-minute, a 60th of a degree.
    ArcMinute,
    /// The arc-second, a 60th of an arc-minute.
    ArcSecond,
    /// The radian: no whole number of angle units, so its values stay doubles.
    Radian,
}

impl AngleUnit {
    /// Every angle unit.
    pub const ALL: [AngleUnit; 5] = [
        AngleUnit::Degree,
        AngleUnit::Gon,
        AngleUnit::ArcMinute,
        AngleUnit::ArcSecond,
        AngleUnit::Radian,
    ];

    /// The unit's count of angle units: 9,000,000 for a degree, 8,100,000 for a gon, 150,000 for an
    /// arc-minute, 2,500 for an arc-second; `None` for the radian (2π is irrational).
    #[must_use]
    pub const fn angle_units(self) -> Option<i64> {
        match self {
            AngleUnit::Degree => Some(9_000_000),
            AngleUnit::Gon => Some(8_100_000),
            AngleUnit::ArcMinute => Some(150_000),
            AngleUnit::ArcSecond => Some(2_500),
            AngleUnit::Radian => None,
        }
    }
}
