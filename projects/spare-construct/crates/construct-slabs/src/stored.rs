//! A floor type's layers as text, in one spelling (FORMAT.md beside this crate). The text is kept
//! in the type's system block under the owner tag [`OWNER`]; construct-types stores it untouched
//! and never reads inside it. The pattern is a wall type's (construct-walls' FORMAT.md): compact
//! JSON, fields in a fixed order, lengths in construct-params' stored spelling, read strictly.
//!
//! A type with a variable layer (job 14.15, [`Layers::with_variable`]) is written as version
//! [`VARIABLE_VERSION`], that layer carrying `"variable":true` after its material; a type without
//! one is written as version [`FORMAT_VERSION`], byte for byte as before, so an older reader
//! refuses a variable layer by its version rather than building it as a plain one.

use crate::error::SlabError;
use crate::layers::{FloorLayer, Function, Layers};
use construct_materials::MaterialId;
use construct_params::{Length, Value};
use serde::{Deserialize, Serialize};

/// The owner tag of a floor type's system block.
pub const OWNER: &str = "floor";

/// The version of the text this crate writes for a type without a variable layer. We may stop
/// writing an old version; we never stop reading it.
pub const FORMAT_VERSION: u32 = 1;

/// The version of the text this crate writes for a type with a variable layer (job 14.15).
pub const VARIABLE_VERSION: u32 = 2;

/// The text, field for field, in the order it is written.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    layers: Vec<StoredLayer>,
}

/// Whether a flag is off, so it is not written.
#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if hands the field by reference"
)]
fn off(flag: &bool) -> bool {
    !*flag
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredLayer {
    function: Function,
    /// construct-params' stored spelling of a length: `length:<payload>`.
    thickness: String,
    material: MaterialId,
    /// Written only on the variable layer, as `true`.
    #[serde(default, skip_serializing_if = "off")]
    variable: bool,
}

/// A length in construct-params' stored spelling.
pub(crate) fn write_length(length: Length) -> String {
    Value::Length(length).stored()
}

/// A length from construct-params' stored spelling; `None` for anything else.
pub(crate) fn read_length(text: &str) -> Option<Length> {
    match Value::from_stored(text)? {
        Value::Length(length) => Some(length),
        _ => None,
    }
}

impl Layers {
    /// Its text: compact JSON with the fields in a fixed order, so the same layers always give
    /// the same bytes.
    ///
    /// # Errors
    /// [`SlabError::Malformed`] only if the JSON writer fails, which it does not for this data.
    pub fn to_text(&self) -> Result<String, SlabError> {
        let stored = Stored {
            version: if self.variable().is_some() {
                VARIABLE_VERSION
            } else {
                FORMAT_VERSION
            },
            layers: self
                .layers()
                .iter()
                .enumerate()
                .map(|(at, layer)| StoredLayer {
                    function: layer.function,
                    thickness: write_length(layer.thickness),
                    material: layer.material.clone(),
                    variable: self.variable() == Some(at),
                })
                .collect(),
        };
        serde_json::to_string(&stored).map_err(|error| SlabError::Malformed(error.to_string()))
    }

    /// Reads the text [`Layers::to_text`] writes. Anything else is refused, never repaired: other
    /// spacing, field order, a field missing or added, a version this crate does not know, a
    /// variable layer in version 1, none or two in version 2, or layers that [`Layers::new`] or
    /// [`Layers::with_variable`] refuses.
    ///
    /// # Errors
    /// [`SlabError::Malformed`] for text that is not this form in its one spelling; any refusal
    /// of [`Layers::new`] and [`Layers::with_variable`] for a form that is well spelt but not a
    /// valid floor type.
    pub fn from_text(text: &str) -> Result<Self, SlabError> {
        let stored: Stored =
            serde_json::from_str(text).map_err(|error| SlabError::Malformed(error.to_string()))?;
        if stored.version != FORMAT_VERSION && stored.version != VARIABLE_VERSION {
            return Err(SlabError::Malformed(format!(
                "version {} is not one this crate reads",
                stored.version
            )));
        }
        let flagged: Vec<usize> = stored
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.variable)
            .map(|(at, _)| at)
            .collect();
        let variable = match (stored.version, flagged.as_slice()) {
            (FORMAT_VERSION, []) => None,
            (VARIABLE_VERSION, [one]) => Some(*one),
            _ => {
                return Err(SlabError::Malformed(format!(
                    "version {} has {} variable layers: version {FORMAT_VERSION} has none and \
                     version {VARIABLE_VERSION} one",
                    stored.version,
                    flagged.len()
                )));
            }
        };
        let mut layers = Vec::with_capacity(stored.layers.len());
        for (at, layer) in stored.layers.into_iter().enumerate() {
            let thickness = read_length(&layer.thickness).ok_or_else(|| {
                SlabError::Malformed(format!(
                    "layer {at}'s thickness {:?} is not a stored length",
                    layer.thickness
                ))
            })?;
            layers.push(FloorLayer::new(layer.function, thickness, layer.material));
        }
        let read = Self::new(layers)?.with_variable(variable)?;
        if read.to_text()? != text {
            return Err(SlabError::Malformed(
                "the text is not in its one spelling".to_owned(),
            ));
        }
        Ok(read)
    }
}
