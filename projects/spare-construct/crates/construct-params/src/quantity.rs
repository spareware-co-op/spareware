//! Measured values, each a whole number in its stored unit. Every step is checked: an answer that
//! does not fit is `None`, never a wrapped or rounded number.

use num_rational::Ratio;

/// A length in whole millimetres, the repository's stored unit (construct-model's FORMAT.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Length(i64);

impl Length {
    /// A length of `mm` millimetres.
    #[must_use]
    pub const fn from_mm(mm: i64) -> Self {
        Self(mm)
    }

    /// The length in millimetres.
    #[must_use]
    pub const fn mm(self) -> i64 {
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

    /// The area of a rectangle this long and `other` wide, if it fits.
    #[must_use]
    pub fn checked_mul(self, other: Self) -> Option<Area> {
        self.0.checked_mul(other.0).map(Area)
    }
}

/// An area in whole square millimetres: the square of the stored length unit, so a length times a
/// length is exact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Area(i64);

impl Area {
    /// An area of `mm2` square millimetres.
    #[must_use]
    pub const fn from_mm2(mm2: i64) -> Self {
        Self(mm2)
    }

    /// The area in square millimetres.
    #[must_use]
    pub const fn mm2(self) -> i64 {
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

    /// The volume of a prism of this area and `depth` deep, if it fits.
    #[must_use]
    pub fn checked_mul(self, depth: Length) -> Option<Volume> {
        self.0.checked_mul(depth.mm()).map(Volume)
    }
}

/// A volume in whole cubic millimetres: the cube of the stored length unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Volume(i64);

impl Volume {
    /// A volume of `mm3` cubic millimetres.
    #[must_use]
    pub const fn from_mm3(mm3: i64) -> Self {
        Self(mm3)
    }

    /// The volume in cubic millimetres.
    #[must_use]
    pub const fn mm3(self) -> i64 {
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
