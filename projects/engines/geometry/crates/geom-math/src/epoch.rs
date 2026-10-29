//! Coordinate epochs and deformation models: the two parts of a frame record that say when its map
//! coordinates hold and how observations at other dates were carried to it (chapter 17.1, Epochs).
//!
//! NZGD2000 is semi-dynamic: coordinates are fixed at a reference date, and a deformation model
//! carries observations made on other dates to it. Plate motion and earthquakes move survey marks
//! by centimetres to metres over decades, so two frames on the same CRS whose epochs or models
//! differ are different frames: the frame record holds both, and frames whose epochs or models
//! differ are never mixed without a recorded transformation (`FrameMismatch::Epoch` and
//! `FrameMismatch::DeformationModel`).
//!
//! - A [`DecimalYear`] is a coordinate epoch as a decimal year, for example 2000.0 or 2023.5, held
//!   exactly as the [`Decimal`] it was given as (PROJ and ISO 19111 write epochs as decimal years).
//! - A [`ModelRef`] names a deformation model by the SHA-256 digest of its content (chapter 17.1's
//!   `ModelRef`: name, version and content hash). The digest is the identity: two models with the
//!   same content are the same model, whatever they are called, and a new version has new content.
//!   This crate has no allocator, so the model's name and version text are not stored here: they
//!   travel with the transformation record that used the model and with the native file
//!   format's frame record, beside this digest.
//!
//! Guarantees:
//!
//! - **Exact identity.** Two epochs are equal exactly when their decimal values are (a
//!   [`Decimal`] is in lowest terms, so 2023.50 and 2023.5 are one epoch); two models are equal
//!   exactly when their digests are.
//! - **In range or refused.** [`DecimalYear::new`] refuses a year below 0 or from 10000 on with
//!   [`NumError::OutsideDomain`].
//!
//! Method: the coordinate epoch of a dynamic coordinate reference system and the
//! point-motion operations of ISO 19111:2019, *Geographic information: referencing by
//! coordinates* (<https://www.iso.org/standard/74039.html>), as chapter 17.1 records them
//! (`projects/engines/geometry/architecture/17-survey-terrain-civil.md`), with PROJ's
//! deformation-model operation, which takes the observation date as input; content addressing by
//! SHA-256 (FIPS 180-4, <https://doi.org/10.6028/NIST.FIPS.180-4>), the digest chapters 21 and
//! 28.13 specify.
//!
//! Cost: constant time; no allocation.

use geom_num::NumError;

use crate::decimal::Decimal;

/// The first year an epoch may not reach: 10000.
const YEAR_END: i128 = 10_000;

/// A coordinate epoch: the date at which a frame's map coordinates hold, as a decimal year (for
/// example 2000.0, or 2023.5 for the start of July 2023).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DecimalYear {
    year: Decimal,
}

impl DecimalYear {
    /// The epoch `year`, as given.
    ///
    /// # Errors
    ///
    /// [`NumError::OutsideDomain`] if the year is below 0 or at least 10000.
    pub fn new(year: Decimal) -> Result<DecimalYear, NumError> {
        let m = year.significand();
        let e = year.exponent();
        // 0 ≤ m·10^e < 10^4: m ≥ 0, and for e ≥ 0 the power is at most 10^3 when m ≥ 1 (beyond
        // that the value is at least 10^4); for e < 0 the value is below m < 10^38, compared with
        // 10^(4 − e) exactly when that power fits (otherwise it exceeds m).
        let inside = if m < 0 {
            false
        } else if e >= 0 {
            10i128
                .checked_pow(e.unsigned_abs())
                .and_then(|p| m.checked_mul(p))
                .is_some_and(|v| v < YEAR_END)
        } else {
            4u32.checked_add(e.unsigned_abs())
                .and_then(|k| 10i128.checked_pow(k))
                .is_none_or(|p| m < p)
        };
        if inside {
            Ok(DecimalYear { year })
        } else {
            Err(NumError::OutsideDomain)
        }
    }

    /// The epoch as a decimal year.
    #[must_use]
    pub const fn year(self) -> Decimal {
        self.year
    }
}

/// A deformation model, named by the SHA-256 digest of its content (see the module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModelRef {
    sha256: [u8; 32],
}

impl ModelRef {
    /// The model whose content has this SHA-256 digest.
    #[must_use]
    pub const fn from_sha256(sha256: [u8; 32]) -> ModelRef {
        ModelRef { sha256 }
    }

    /// The model's content digest.
    #[must_use]
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
}
