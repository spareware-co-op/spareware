//! A floor type's layers as text, in one spelling (FORMAT.md beside this crate). The text is kept
//! in the type's system block under the owner tag [`OWNER`]; construct-types stores it untouched
//! and never reads inside it. The pattern is a wall type's (construct-walls' FORMAT.md): compact
//! JSON, fields in a fixed order, lengths in construct-params' stored spelling, read strictly.

use crate::error::SlabError;
use crate::layers::{FloorLayer, Function, Layers};
use construct_materials::MaterialId;
use construct_params::{Length, Value};
use serde::{Deserialize, Serialize};

/// The owner tag of a floor type's system block.
pub const OWNER: &str = "floor";

/// The version of the text this crate writes. We may stop writing an old version; we never stop
/// reading it.
pub const FORMAT_VERSION: u32 = 1;

/// The text, field for field, in the order it is written.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    layers: Vec<StoredLayer>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredLayer {
    function: Function,
    /// construct-params' stored spelling of a length: `length:<payload>`.
    thickness: String,
    material: MaterialId,
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
            version: FORMAT_VERSION,
            layers: self
                .layers()
                .iter()
                .map(|layer| StoredLayer {
                    function: layer.function,
                    thickness: write_length(layer.thickness),
                    material: layer.material.clone(),
                })
                .collect(),
        };
        serde_json::to_string(&stored).map_err(|error| SlabError::Malformed(error.to_string()))
    }

    /// Reads the text [`Layers::to_text`] writes. Anything else is refused, never repaired: other
    /// spacing, field order, a field missing or added, a version this crate does not know, or
    /// layers that [`Layers::new`] refuses.
    ///
    /// # Errors
    /// [`SlabError::Malformed`] for text that is not this form in its one spelling; any refusal
    /// of [`Layers::new`] for a form that is well spelt but not a valid floor type.
    pub fn from_text(text: &str) -> Result<Self, SlabError> {
        let stored: Stored =
            serde_json::from_str(text).map_err(|error| SlabError::Malformed(error.to_string()))?;
        if stored.version != FORMAT_VERSION {
            return Err(SlabError::Malformed(format!(
                "version {} is not one this crate reads",
                stored.version
            )));
        }
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
        let read = Self::new(layers)?;
        if read.to_text()? != text {
            return Err(SlabError::Malformed(
                "the text is not in its one spelling".to_owned(),
            ));
        }
        Ok(read)
    }
}
