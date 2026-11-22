//! An exact value and its dimension.

use crate::{FormulaError, MAX_BITS};
use dashu_int::IBig;
use dashu_int::ops::{BitTest, UnsignedAbs};
use dashu_ratio::RBig;
use std::fmt;

/// A dimension: the powers of length and angle. A plain number (a count, a ratio) has both zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Dim {
    pub length: i8,
    pub angle: i8,
}

/// The largest power a dimension may reach, either way. Nothing a part needs comes near it.
const MOST_POWER: i8 = 6;

impl Dim {
    pub const NUMBER: Self = Self {
        length: 0,
        angle: 0,
    };
    pub const LENGTH: Self = Self {
        length: 1,
        angle: 0,
    };
    pub const ANGLE: Self = Self {
        length: 0,
        angle: 1,
    };

    pub(crate) fn times(self, other: Self, sign: i8) -> Result<Self, FormulaError> {
        let power = |a: i8, b: i8| {
            a.checked_add(b.checked_mul(sign)?)
                .filter(|p| p.abs() <= MOST_POWER)
        };
        match (
            power(self.length, other.length),
            power(self.angle, other.angle),
        ) {
            (Some(length), Some(angle)) => Ok(Self { length, angle }),
            _ => Err(FormulaError::new(
                "Units",
                "That makes a unit no part needs (a power past six).",
            )),
        }
    }
}

impl fmt::Display for Dim {
    /// In words, with an article: "a length", "an area", "a plain number".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.length, self.angle) {
            (0, 0) => f.write_str("a plain number"),
            (1, 0) => f.write_str("a length"),
            (2, 0) => f.write_str("an area"),
            (3, 0) => f.write_str("a volume"),
            (0, 1) => f.write_str("an angle"),
            (l, a) => {
                let part = |name: &str, p: i8| match p {
                    0 => String::new(),
                    1 => name.to_owned(),
                    p => format!("{name} to the power {p}"),
                };
                let words: Vec<String> = [part("length", l), part("angle", a)]
                    .into_iter()
                    .filter(|w| !w.is_empty())
                    .collect();
                write!(f, "{}", words.join(" times "))
            }
        }
    }
}

/// What a parameter or a field holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Kind {
    /// In millimetres.
    #[default]
    Length,
    /// In degrees.
    Angle,
    /// A plain number.
    Count,
}

impl Kind {
    #[must_use]
    pub fn dim(self) -> Dim {
        match self {
            Self::Length => Dim::LENGTH,
            Self::Angle => Dim::ANGLE,
            Self::Count => Dim::NUMBER,
        }
    }

    /// Reads a kind's name: `length`, `angle` or `count`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "length" => Some(Self::Length),
            "angle" => Some(Self::Angle),
            "count" => Some(Self::Count),
            _ => None,
        }
    }

    /// The unit a value of this kind is shown in.
    #[must_use]
    pub fn unit(self) -> &'static str {
        match self {
            Self::Length => "mm",
            Self::Angle => "deg",
            Self::Count => "",
        }
    }
}

/// An exact value with its dimension: lengths in millimetres, angles in degrees.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quantity {
    pub value: RBig,
    pub dim: Dim,
}

impl Quantity {
    /// A whole number of a dimension.
    #[must_use]
    pub fn whole(n: i64, dim: Dim) -> Self {
        Self {
            value: RBig::from(IBig::from(n)),
            dim,
        }
    }

    /// The value as the nearest double, in its base unit. The one rounding a value meets.
    #[must_use]
    pub fn to_f64(&self) -> f64 {
        self.value.to_f64().value()
    }

    /// Refuses a number grown too large to be worth working with.
    pub(crate) fn checked(self) -> Result<Self, FormulaError> {
        let bits =
            self.value.numerator().unsigned_abs().bit_len() + self.value.denominator().bit_len();
        if bits > MAX_BITS {
            return Err(FormulaError::new(
                "TooLarge",
                "That works out to a number too large to keep exactly.",
            ));
        }
        Ok(self)
    }
}
