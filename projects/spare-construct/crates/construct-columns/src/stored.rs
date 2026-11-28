//! What a column type holds, as text in one spelling (FORMAT.md beside this crate): its section
//! and its material. The text is kept in the type's system block under the owner tag [`OWNER`];
//! construct-types stores it untouched and never reads inside it. The pattern is a floor type's:
//! compact JSON, fields in a fixed order, lengths in construct-params' stored spelling, read
//! strictly.

use crate::error::ColumnError;
use crate::section::Section;
use construct_materials::MaterialId;
use construct_params::{Length, Value};
use serde::{Deserialize, Serialize};

/// The owner tag of a column type's system block.
pub const OWNER: &str = "column";
/// The version of the text this crate writes.
pub const FORMAT_VERSION: u32 = 1;

/// What a column type holds: the section its columns are cut across, and what they are made of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnSpec {
    /// The section.
    pub section: Section,
    /// The material, construct-materials' ID.
    pub material: MaterialId,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    section: StoredSection,
    material: MaterialId,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum StoredSection {
    Rect { width: String, depth: String },
    Round { diameter: String },
}

fn write_length(length: Length) -> String {
    Value::Length(length).stored()
}

fn read_length(text: &str, what: &str) -> Result<Length, ColumnError> {
    match Value::from_stored(text) {
        Some(Value::Length(length)) => Ok(length),
        _ => Err(ColumnError::Malformed(format!(
            "the {what} {text:?} is not a stored length"
        ))),
    }
}

impl ColumnSpec {
    /// The text a type's block holds.
    ///
    /// # Errors
    /// [`ColumnError::Malformed`] only if the JSON writer fails, which it does not for this data.
    pub fn to_text(&self) -> Result<String, ColumnError> {
        let section = match self.section {
            Section::Rectangular { width, depth } => StoredSection::Rect {
                width: write_length(width),
                depth: write_length(depth),
            },
            Section::Round { diameter } => StoredSection::Round {
                diameter: write_length(diameter),
            },
        };
        let stored = Stored {
            version: FORMAT_VERSION,
            section,
            material: self.material.clone(),
        };
        serde_json::to_string(&stored).map_err(|e| ColumnError::Malformed(e.to_string()))
    }

    /// Reads a type's text, strictly: any other spelling of the same data is refused.
    ///
    /// # Errors
    /// [`ColumnError::Malformed`] for text that is not as [`ColumnSpec::to_text`] writes it, a
    /// version this crate does not read, or a length that is not stored text; the sizes'
    /// refusals ([`Section::rectangular`], [`Section::round`]).
    pub fn from_text(text: &str) -> Result<Self, ColumnError> {
        let stored: Stored =
            serde_json::from_str(text).map_err(|e| ColumnError::Malformed(e.to_string()))?;
        if stored.version != FORMAT_VERSION {
            return Err(ColumnError::Malformed(format!(
                "version {} is not one this crate reads",
                stored.version
            )));
        }
        let section = match stored.section {
            StoredSection::Rect { width, depth } => {
                Section::rectangular(read_length(&width, "width")?, read_length(&depth, "depth")?)?
            }
            StoredSection::Round { diameter } => {
                Section::round(read_length(&diameter, "diameter")?)?
            }
        };
        let read = Self {
            section,
            material: stored.material,
        };
        if read.to_text()? != text {
            return Err(ColumnError::Malformed(
                "the text is not in its one spelling".to_owned(),
            ));
        }
        Ok(read)
    }
}
