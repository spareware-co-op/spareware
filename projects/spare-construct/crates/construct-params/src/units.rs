//! How measured values are shown and read: a unit and a number of decimal places for each
//! measured kind, as a project setting. Metric first. A display unit never changes what is stored:
//! every stored value is in its kind's whole unit, so feet and inches, when they come, are a
//! [`Unit`] each with its exact factor (a foot is 3048/10 mm) and a row in the symbol table in
//! `text.rs`, and nothing stored moves.

use crate::definition::Kind;
use crate::error::ParamError;
use serde::{Deserialize, Serialize};

/// The most decimal places a format shows: a cubic metre is 10^9 cubic millimetres, so nine
/// places show every stored volume exactly.
pub const MAX_PLACES: u8 = 9;

/// A unit a measured value is shown in and read from.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unit {
    /// Millimetres, `mm`: the stored length unit.
    #[serde(rename = "mm")]
    Millimetre,
    /// Metres, `m`.
    #[serde(rename = "m")]
    Metre,
    /// Square millimetres, `mm²`: the stored area unit.
    #[serde(rename = "mm2")]
    SquareMillimetre,
    /// Square metres, `m²`.
    #[serde(rename = "m2")]
    SquareMetre,
    /// Cubic millimetres, `mm³`: the stored volume unit.
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

    /// How many stored units one of this unit is, as an exact fraction (numerator, denominator).
    /// Every metric factor is whole; the denominator is the place a foot's 3048/10 will need.
    /// `None` for the two slope forms, which are not a scale of a stored unit.
    #[must_use]
    pub const fn factor(self) -> Option<(i128, i128)> {
        match self {
            Self::Millimetre | Self::SquareMillimetre | Self::CubicMillimetre => Some((1, 1)),
            Self::Metre => Some((1_000, 1)),
            Self::SquareMetre => Some((1_000_000, 1)),
            Self::CubicMetre => Some((1_000_000_000, 1)),
            Self::Degree => Some((9_000_000, 1)),
            Self::Percent | Self::Ratio => None,
        }
    }
}

/// How one measured kind is shown: its unit and how many decimal places, rounded half to even.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    /// The unit shown, and the unit a number typed with none is read in.
    pub unit: Unit,
    /// Decimal places shown, at most nine.
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
    /// The metric defaults: lengths in whole millimetres, areas in square metres and volumes in
    /// cubic metres to two places, angles in degrees to two places, slopes as a percentage to two
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
    /// places are more than nine.
    pub fn set(&mut self, kind: Kind, format: Format) -> Result<(), ParamError> {
        let bad = ParamError::BadFormat {
            kind,
            unit: format.unit,
            places: format.places,
        };
        if format.unit.kind() != kind || format.places > MAX_PLACES {
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
