//! The exact values a drawing's pens and patterns are made of.
//!
//! A printed line is a fraction of a millimetre wide, and it is a size on paper, not a model
//! length (model lengths are the geometry kernel's grid steps, note CON-A2-F3-P1, on which a
//! micrometre is not whole). Printed lengths are stored as whole micrometres in a `u32`: 0.13 mm
//! is 130. A drawing's printed sizes do not change when the model or the view scale does. A float appears only in
//! [`Micrometres::as_mm`], for showing a value.

use crate::error::StyleError;
use serde::{Deserialize, Serialize};

/// The longest printed length stored: one metre.
const LONGEST_MICROMETRES: u32 = 1_000_000;

/// The largest view-scale denominator stored: 1:1,000,000.
const LARGEST_DENOMINATOR: u32 = 1_000_000;

/// A printed length in whole micrometres, above zero and at most one metre.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(try_from = "u32", into = "u32")]
pub struct Micrometres(u32);

impl Micrometres {
    /// A length of `micrometres`, refused if it is zero or above one metre.
    ///
    /// # Errors
    /// [`StyleError::BadLength`] when it is zero or above 1,000,000.
    pub fn new(micrometres: u32) -> Result<Self, StyleError> {
        if micrometres == 0 || micrometres > LONGEST_MICROMETRES {
            return Err(StyleError::BadLength { micrometres });
        }
        Ok(Self(micrometres))
    }

    /// A length this crate fixes itself, such as a built-in pattern's. Callers pass values inside
    /// the allowed range.
    pub(crate) const fn built_in(micrometres: u32) -> Self {
        Self(micrometres)
    }

    /// The length in whole micrometres.
    #[must_use]
    pub fn get(self) -> u32 {
        self.0
    }

    /// The length in millimetres, for display only.
    #[must_use]
    pub fn as_mm(self) -> f64 {
        f64::from(self.0) / 1000.0
    }
}

impl TryFrom<u32> for Micrometres {
    type Error = StyleError;

    fn try_from(micrometres: u32) -> Result<Self, StyleError> {
        Self::new(micrometres)
    }
}

impl From<Micrometres> for u32 {
    fn from(length: Micrometres) -> u32 {
        length.0
    }
}

/// A view scale 1:n, held as its denominator `n`. A bigger `n` is a smaller scale: the drawing
/// shows more of the building on the sheet and its lines print thinner.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(try_from = "u32", into = "u32")]
pub struct Scale(u32);

impl Scale {
    /// The scale 1:`denominator`.
    ///
    /// # Errors
    /// [`StyleError::BadScale`] when it is zero or above 1,000,000.
    pub fn new(denominator: u32) -> Result<Self, StyleError> {
        if denominator == 0 || denominator > LARGEST_DENOMINATOR {
            return Err(StyleError::BadScale { denominator });
        }
        Ok(Self(denominator))
    }

    /// The `n` of 1:n.
    #[must_use]
    pub fn denominator(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for Scale {
    type Error = StyleError;

    fn try_from(denominator: u32) -> Result<Self, StyleError> {
        Self::new(denominator)
    }
}

impl From<Scale> for u32 {
    fn from(scale: Scale) -> u32 {
        scale.0
    }
}

/// The most bytes a pen's or a pattern's name may have.
const LONGEST_NAME: usize = 64;

/// Refuses a name that is empty, longer than 64 bytes, has a control character, or has a space at
/// either end.
pub(crate) fn check_name(name: &str) -> Result<(), StyleError> {
    let trimmed = name.trim();
    if name.is_empty()
        || name.len() > LONGEST_NAME
        || trimmed.len() != name.len()
        || name.chars().any(char::is_control)
    {
        return Err(StyleError::BadName);
    }
    Ok(())
}
