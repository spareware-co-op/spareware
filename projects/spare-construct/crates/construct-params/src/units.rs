//! How measured values are shown and read: a unit and a number of decimal places for each
//! measured kind, as a project setting. Metric first. A display unit never changes what is stored:
//! every stored value is in its kind's whole unit (a length in the geometry kernel's grid steps,
//! 1/6,400 mm), so feet and inches, when they come, are a [`Unit`] each with its exact factor (a
//! foot is 1,950,720 steps, since 1/256 inch is a whole 635) and a row in the symbol table in
//! `text.rs`, and nothing stored moves.
//!
//! A format shows at most 0.01 mm of a length (two places of a millimetre, five of a metre), the
//! model tolerance, and the same for areas and volumes ([`Unit::max_places`]): every value shown
//! is then a whole number of grid steps, so it reads back as typed text without refusal.

use crate::definition::Kind;
use crate::error::ParamError;
use serde::{Deserialize, Serialize};

/// The most decimal places any format shows. Each unit may allow fewer ([`Unit::max_places`]).
pub const MAX_PLACES: u8 = 9;

/// A unit a measured value is shown in and read from.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unit {
    /// Millimetres, `mm`: 6,400 grid steps, the stored length unit.
    #[serde(rename = "mm")]
    Millimetre,
    /// Metres, `m`.
    #[serde(rename = "m")]
    Metre,
    /// Square millimetres, `mm²`: 40,960,000 square grid steps, the stored area unit.
    #[serde(rename = "mm2")]
    SquareMillimetre,
    /// Square metres, `m²`.
    #[serde(rename = "m2")]
    SquareMetre,
    /// Cubic millimetres, `mm³`: 262,144,000,000 cubic grid steps, the stored volume unit.
    #[serde(rename = "mm3")]
    CubicMillimetre,
    /// Cubic metres, `m³`.
    #[serde(rename = "m3")]
    CubicMetre,
    /// Decimal degrees, `°`.
    #[serde(rename = "degree")]
    Degree,
    /// A slope as a percentage, rise over run times a hundred: `2.50 %`.
    #[serde(rename = "percent")]
    Percent,
    /// A slope as a ratio, `1:40` (or `2:1` when it rises more than it runs).
    #[serde(rename = "ratio")]
    Ratio,
}

impl Unit {
    /// The kind this unit measures.
    #[must_use]
    pub const fn kind(self) -> Kind {
        match self {
            Self::Millimetre | Self::Metre => Kind::Length,
            Self::SquareMillimetre | Self::SquareMetre => Kind::Area,
            Self::CubicMillimetre | Self::CubicMetre => Kind::Volume,
            Self::Degree => Kind::Angle,
            Self::Percent | Self::Ratio => Kind::Slope,
        }
    }

    /// How a shown value ends, after the number: a space and the SI symbol, or nothing for a
    /// ratio, whose `:` is in the number.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Millimetre => " mm",
            Self::Metre => " m",
            Self::SquareMillimetre => " mm²",
            Self::SquareMetre => " m²",
            Self::CubicMillimetre => " mm³",
            Self::CubicMetre => " m³",
            // The SI writes the degree sign against the number, and a space before the percent.
            Self::Degree => "°",
            Self::Percent => " %",
            Self::Ratio => "",
        }
    }

    /// The unit's name in words.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Millimetre => "millimetres",
            Self::Metre => "metres",
            Self::SquareMillimetre => "square millimetres",
            Self::SquareMetre => "square metres",
            Self::CubicMillimetre => "cubic millimetres",
            Self::CubicMetre => "cubic metres",
            Self::Degree => "degrees",
            Self::Percent => "percent",
            Self::Ratio => "a ratio",
        }
    }

    /// How many stored units one of this unit is, as an exact fraction (numerator, denominator):
    /// grid steps for a length (6,400 to the millimetre), square and cubic grid steps for an area
    /// and a volume, angle units for an angle. Every factor is whole; the denominator is the
    /// place a unit that is not a whole number of stored units would need. `None` for the two
    /// slope forms, which are not a scale of a stored unit.
    #[must_use]
    pub const fn factor(self) -> Option<(i128, i128)> {
        match self {
            Self::Millimetre => Some((6_400, 1)),
            Self::Metre => Some((6_400_000, 1)),
            Self::SquareMillimetre => Some((40_960_000, 1)),
            Self::SquareMetre => Some((40_960_000_000_000, 1)),
            Self::CubicMillimetre => Some((262_144_000_000, 1)),
            Self::CubicMetre => Some((262_144_000_000_000_000_000, 1)),
            Self::Degree => Some((9_000_000, 1)),
            Self::Percent | Self::Ratio => None,
        }
    }

    /// The most decimal places a value in this unit is shown to: as fine as 0.01 mm (the model
    /// tolerance) for lengths, 0.0001 mm² and 0.000001 mm³ for areas and volumes, and never more
    /// than nine. Each of these is a whole number of grid steps, square steps or cubic
    /// steps, so a value shown always reads back.
    #[must_use]
    pub const fn max_places(self) -> u8 {
        match self {
            Self::Millimetre => 2,
            Self::Metre => 5,
            Self::SquareMillimetre => 4,
            Self::CubicMillimetre => 6,
            Self::SquareMetre | Self::CubicMetre | Self::Degree | Self::Percent | Self::Ratio => {
                MAX_PLACES
            }
        }
    }
}

/// How one measured kind is shown: its unit and how many decimal places, rounded half to even.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    /// The unit shown, and the unit a number typed with none is read in.
    pub unit: Unit,
    /// Decimal places shown, at most the unit's [`Unit::max_places`].
    pub places: u8,
}

/// The display settings for every measured kind: a project or view setting, stored as a JSON row.
/// Every format in it is for its kind, as set and as read back.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(try_from = "UnitsRow")]
pub struct Units {
    length: Format,
    area: Format,
    volume: Format,
    angle: Format,
    slope: Format,
}

/// Unit settings as read, before their checks.
#[derive(Deserialize)]
struct UnitsRow {
    length: Format,
    area: Format,
    volume: Format,
    angle: Format,
    slope: Format,
}

impl TryFrom<UnitsRow> for Units {
    type Error = ParamError;

    fn try_from(row: UnitsRow) -> Result<Self, ParamError> {
        let mut units = Self::metric();
        units.set(Kind::Length, row.length)?;
        units.set(Kind::Area, row.area)?;
        units.set(Kind::Volume, row.volume)?;
        units.set(Kind::Angle, row.angle)?;
        units.set(Kind::Slope, row.slope)?;
        Ok(units)
    }
}

impl Default for Units {
    fn default() -> Self {
        Self::metric()
    }
}

impl Units {
    /// The metric defaults: lengths shown in whole millimetres (a stored 12.5 mm shows as `12 mm`,
    /// half to even, and as `12.5 mm` at one place), areas in square metres and volumes in cubic
    /// metres to two places, angles in degrees to two places, slopes as a percentage to two
    /// places.
    #[must_use]
    pub const fn metric() -> Self {
        Self {
            length: Format {
                unit: Unit::Millimetre,
                places: 0,
            },
            area: Format {
                unit: Unit::SquareMetre,
                places: 2,
            },
            volume: Format {
                unit: Unit::CubicMetre,
                places: 2,
            },
            angle: Format {
                unit: Unit::Degree,
                places: 2,
            },
            slope: Format {
                unit: Unit::Percent,
                places: 2,
            },
        }
    }

    /// How `kind` is shown; `None` for a kind with no unit.
    #[must_use]
    pub const fn format(&self, kind: Kind) -> Option<Format> {
        match kind {
            Kind::Length => Some(self.length),
            Kind::Area => Some(self.area),
            Kind::Volume => Some(self.volume),
            Kind::Angle => Some(self.angle),
            Kind::Slope => Some(self.slope),
            _ => None,
        }
    }

    /// Sets how `kind` is shown.
    ///
    /// # Errors
    /// [`ParamError::BadFormat`], changing nothing, if the unit does not measure `kind` or the
    /// places are more than the unit's [`Unit::max_places`].
    pub fn set(&mut self, kind: Kind, format: Format) -> Result<(), ParamError> {
        let bad = ParamError::BadFormat {
            kind,
            unit: format.unit,
            places: format.places,
        };
        if format.unit.kind() != kind || format.places > format.unit.max_places() {
            return Err(bad);
        }
        let slot = match kind {
            Kind::Length => &mut self.length,
            Kind::Area => &mut self.area,
            Kind::Volume => &mut self.volume,
            Kind::Angle => &mut self.angle,
            Kind::Slope => &mut self.slope,
            _ => return Err(bad),
        };
        *slot = format;
        Ok(())
    }
}
