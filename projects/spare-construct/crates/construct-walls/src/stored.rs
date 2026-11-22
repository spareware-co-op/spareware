//! A wall type's layers and core as text, in one spelling (FORMAT.md beside this crate). The text
//! is kept in the type's system block under the owner tag [`OWNER`]; construct-types stores it
//! untouched and never reads inside it.

use crate::assembly::{Assembly, Side};
use crate::error::WallError;
use crate::layer::{Function, Layer};
use crate::length;
use construct_materials::MaterialId;
use serde::{Deserialize, Serialize};

/// The owner tag of a wall type's system block.
pub const OWNER: &str = "wall";

/// The version of the text this crate writes. We may stop writing an old version; we never stop
/// reading it.
pub const FORMAT_VERSION: u32 = 1;

/// The text, field for field, in the order it is written.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    core: StoredCore,
    layers: Vec<StoredLayer>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredCore {
    exterior: usize,
    interior: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredLayer {
    function: Function,
    /// construct-params' stored spelling of a length: `length:<payload>`.
    thickness: String,
    material: MaterialId,
    wraps: bool,
}

impl Assembly {
    /// Its text: compact JSON with the fields in a fixed order, so the same layers and core
    /// always give the same bytes.
    ///
    /// # Errors
    /// [`WallError::Malformed`] only if the JSON writer fails, which it does not for this data.
    pub fn to_text(&self) -> Result<String, WallError> {
        let stored = Stored {
            version: FORMAT_VERSION,
            core: StoredCore {
                exterior: self.boundary(Side::Exterior),
                interior: self.boundary(Side::Interior),
            },
            layers: self
                .layers()
                .iter()
                .map(|layer| StoredLayer {
                    function: layer.function,
                    thickness: length::write(layer.thickness),
                    material: layer.material.clone(),
                    wraps: layer.wraps,
                })
                .collect(),
        };
        serde_json::to_string(&stored).map_err(|error| WallError::Malformed(error.to_string()))
    }

    /// Reads the text [`Assembly::to_text`] writes. Anything else is refused, never repaired:
    /// other spacing, field order, a field missing or added, a version this crate does not
    /// know, or layers and a core that [`Assembly::new`] refuses.
    ///
    /// # Errors
    /// [`WallError::Malformed`] for text that is not this form in its one spelling; any refusal
    /// of [`Assembly::new`] for a form that is well spelt but not a valid wall type.
    pub fn from_text(text: &str) -> Result<Self, WallError> {
        let stored: Stored =
            serde_json::from_str(text).map_err(|error| WallError::Malformed(error.to_string()))?;
        if stored.version != FORMAT_VERSION {
            return Err(WallError::Malformed(format!(
                "version {} is not one this crate reads",
                stored.version
            )));
        }
        let mut layers = Vec::with_capacity(stored.layers.len());
        for (at, layer) in stored.layers.into_iter().enumerate() {
            let thickness = length::read(&layer.thickness).ok_or_else(|| {
                WallError::Malformed(format!(
                    "layer {at}'s thickness {:?} is not a stored length",
                    layer.thickness
                ))
            })?;
            layers.push(Layer::new(
                layer.function,
                thickness,
                layer.material,
                layer.wraps,
            ));
        }
        let assembly = Self::new(layers, stored.core.exterior, stored.core.interior)?;
        if assembly.to_text()? != text {
            return Err(WallError::Malformed(
                "the text is not in its one spelling".to_owned(),
            ));
        }
        Ok(assembly)
    }
}
