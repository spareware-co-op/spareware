//! Measured values, each a whole number in its stored unit. Every step is checked: an answer that
//! does not fit is `None`, never a wrapped or rounded number.
//!
//! The stored length unit is the geometry kernel's grid step `Q0`, 1/6,400 of a millimetre
//! (geometry architecture chapter 6.1; `GridStep::Q0` in `engine-geometry`): a length hands to the
//! kernel unchanged, every size to two decimals of a millimetre is exact, and an area and a volume
//! are the square and the cube of it, so products of lengths stay exact.

use num_rational::Ratio;
use std::fmt;

/// A length in whole grid steps of the geometry kernel, 1/6,400 of a millimetre each: 2,400 mm
/// is 15,360,000 steps and 12.5 mm is 80,000. The kernel's points and lengths are in the same
/// steps, so nothing is rounded between the two.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Length(i64);

impl Length {
    /// Grid steps in one millimetre: the kernel's `GridStep::Q0` (156.25 nm).
    pub const STEPS_PER_MM: i64 = 6_400;

    /// A length of `steps` grid steps.
    #[must_use]
    pub const fn from_steps(steps: i64) -> Self {
        Self(steps)
    }

    /// The length in grid steps: the number the kernel takes.
    #[must_use]
    pub const fn steps(self) -> i64 {
        self.0
    }

    /// A length of `mm` whole millimetres, exactly. Any `i32` fits: 2,147 km at most. Usable in
    /// a `const`.
    #[must_use]
    pub const fn from_mm(mm: i32) -> Self {
        // A widening cast (`From` is not usable in a const fn); |mm| < 2^31, so mm x 6,400 < 2^44
        // and it never saturates.
        Self((mm as i64).saturating_mul(Self::STEPS_PER_MM))
    }

    /// A length of `mm` whole millimetres, exactly, if it fits.
    #[must_use]
    pub fn checked_from_mm(mm: i64) -> Option<Self> {
        mm.checked_mul(Self::STEPS_PER_MM).map(Self)
    }

    /// The length in whole millimetres, if it is one; `None` for 12.5 mm.
    #[must_use]
    pub fn whole_mm(self) -> Option<i64> {
        if self.0.checked_rem(Self::STEPS_PER_MM)? == 0 {
            self.0.checked_div(Self::STEPS_PER_MM)
        } else {
            None
        }
    }

    /// The sum, if it fits.
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    /// The difference, if it fits.
    #[must_use]
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }

    /// The area of a rectangle this long and `other` wide. A product of two lengths always fits
    /// an area, so this is never `None`.
    #[must_use]
    pub fn checked_mul(self, other: Self) -> Option<Area> {
        i128::from(self.0)
            .checked_mul(i128::from(other.0))
            .map(Area)
    }
}

/// Shows the length in millimetres, exactly, with every decimal it has and no more: `2400 mm`,
/// `12.5 mm`, `0.00015625 mm` for one step. For a person's own display units and rounding, use
/// [`crate::Definition::format`].
impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mm =
            exact_decimal(i128::from(self.0), i128::from(Self::STEPS_PER_MM)).ok_or(fmt::Error)?;
        write!(f, "{mm} mm")
    }
}

/// An area in whole square grid steps: the square of the stored length unit, so a length times a
/// length is exact. One square millimetre is 40,960,000 of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Area(i128);

impl Area {
    /// Square grid steps in one square millimetre: 6,400 squared.
    pub const SQUARE_STEPS_PER_MM2: i128 = 40_960_000;

    /// An area of `square_steps` square grid steps.
    #[must_use]
    pub const fn from_square_steps(square_steps: i128) -> Self {
        Self(square_steps)
    }

    /// The area in square grid steps.
    #[must_use]
    pub const fn square_steps(self) -> i128 {
        self.0
    }

    /// An area of `mm2` whole square millimetres, exactly. Any `i64` fits.
    #[must_use]
    pub fn from_mm2(mm2: i64) -> Self {
        // |mm2| < 2^63 and the factor is under 2^26, so the product is under 2^89.
        Self(i128::from(mm2).saturating_mul(Self::SQUARE_STEPS_PER_MM2))
    }

    /// The sum, if it fits.
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    /// The difference, if it fits.
    #[must_use]
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }

    /// The volume of a prism of this area and `depth` deep, if it fits.
    #[must_use]
    pub fn checked_mul(self, depth: Length) -> Option<Volume> {
        self.0.checked_mul(i128::from(depth.steps())).map(Volume)
    }
}

/// A volume in whole cubic grid steps: the cube of the stored length unit. One cubic millimetre
/// is 262,144,000,000 of them, and a cubic metre about 2.6 x 10^20, so it is held in 128 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Volume(i128);

impl Volume {
    /// Cubic grid steps in one cubic millimetre: 6,400 cubed.
    pub const CUBIC_STEPS_PER_MM3: i128 = 262_144_000_000;

    /// A volume of `cubic_steps` cubic grid steps.
    #[must_use]
    pub const fn from_cubic_steps(cubic_steps: i128) -> Self {
        Self(cubic_steps)
    }

    /// The volume in cubic grid steps.
    #[must_use]
    pub const fn cubic_steps(self) -> i128 {
        self.0
    }

    /// A volume of `mm3` whole cubic millimetres, exactly. Any `i64` fits.
    #[must_use]
    pub fn from_mm3(mm3: i64) -> Self {
        // |mm3| < 2^63 and the factor is under 2^38, so the product is under 2^101.
        Self(i128::from(mm3).saturating_mul(Self::CUBIC_STEPS_PER_MM3))
    }

    /// The sum, if it fits.
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    /// The difference, if it fits.
    #[must_use]
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
}

/// `n / d` written exactly as a decimal, every digit it has and no trailing zero, for `d` above
/// zero made only of twos and fives (every grid factor is: 6,400 is 2^8 x 5^2), so the digits
/// end. `None` if they do not end within forty places, or `d` is not above zero.
pub(crate) fn exact_decimal(n: i128, d: i128) -> Option<String> {
    if d <= 0 {
        return None;
    }
    let sign = if n < 0 { "-" } else { "" };
    let n = n.unsigned_abs();
    let d = d.unsigned_abs();
    let whole = n.checked_div(d)?;
    let mut rest = n.checked_rem(d)?;
    let mut digits = String::new();
    for _ in 0..40 {
        if rest == 0 {
            break;
        }
        // rest < d < 2^127, so ten times it fits a u128.
        let tenfold = rest.checked_mul(10)?;
        let digit = u32::try_from(tenfold.checked_div(d)?).ok()?;
        digits.push(char::from_digit(digit, 10)?);
        rest = tenfold.checked_rem(d)?;
    }
    if rest != 0 {
        return None;
    }
    Some(if digits.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{digits}")
    })
}

/// An angle in whole angle units of the geometry kernel: 1/2,500 of an arc-second, so decimal
/// degrees to six places, degrees, minutes and seconds to 0.01″, and gon to 0.00001 are all
/// exact (geometry architecture chapter 17, "Angles are whole numbers").
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Angle(i64);

impl Angle {
    /// Angle units in one degree: 3,600 arc-seconds of 2,500 units each.
    pub const UNITS_PER_DEGREE: i64 = 9_000_000;

    /// An angle of `units` angle units.
    #[must_use]
    pub const fn from_units(units: i64) -> Self {
        Self(units)
    }

    /// The angle in angle units.
    #[must_use]
    pub const fn units(self) -> i64 {
        self.0
    }

    /// The sum, if it fits.
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    /// The difference, if it fits.
    #[must_use]
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }

    /// The angle in degrees, exactly, in lowest terms: the form a formula holds an angle in.
    #[must_use]
    pub fn degrees(self) -> Ratio<i128> {
        Ratio::new(i128::from(self.0), i128::from(Self::UNITS_PER_DEGREE))
    }

    /// The angle of exactly `degrees`; `None` unless it is a whole number of angle units that
    /// fits. Nothing is rounded here: a caller with a finer angle rounds it first, by its rule.
    #[must_use]
    pub fn from_degrees(degrees: Ratio<i128>) -> Option<Self> {
        let scaled = degrees
            .numer()
            .checked_mul(i128::from(Self::UNITS_PER_DEGREE))?;
        let denom = *degrees.denom();
        if denom <= 0 || scaled.checked_rem(denom)? != 0 {
            return None;
        }
        i64::try_from(scaled.checked_div(denom)?).ok().map(Self)
    }
}

/// A slope: a rise over a run, as a ratio of whole numbers in lowest terms with the run above
/// zero. Falls such as 1:60 and grades such as 2.5% are exact; a slope in degrees is an
/// [`Angle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slope(Ratio<i64>);

impl Slope {
    /// The slope `rise` over `run`, in lowest terms; `None` for a run of zero (a vertical face has
    /// no slope) or for `i64::MIN` either way, whose negation does not fit.
    #[must_use]
    pub fn new(rise: i64, run: i64) -> Option<Self> {
        if run == 0 || rise == i64::MIN || run == i64::MIN {
            return None;
        }
        // Ratio::new reduces and moves the sign to the rise. With neither side i64::MIN and the
        // run not zero, it neither divides by zero nor overflows.
        Some(Self(Ratio::new(rise, run)))
    }

    /// The rise, in lowest terms; negative for a fall.
    #[must_use]
    pub fn rise(self) -> i64 {
        *self.0.numer()
    }

    /// The run, in lowest terms; always above zero.
    #[must_use]
    pub fn run(self) -> i64 {
        *self.0.denom()
    }
}
