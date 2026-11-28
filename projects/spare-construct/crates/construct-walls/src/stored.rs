//! A wall type's layers, core and wrapping as text, in one spelling (FORMAT.md beside this crate).
//! The text is kept in the type's system block under the owner tag [`OWNER`]; construct-types
//! stores it untouched and never reads inside it. Version 1 holds the layers and the core; version
//! 2 (job 6.1) adds the wrapping at free ends and at inserts. A type that wraps nothing is written
//! as version 1, so every type made before version 2 keeps its bytes; both are read for ever.

use crate::assembly::{Assembly, Side};
use crate::error::WallError;
use crate::layer::{Function, Layer};
use crate::length;
use crate::wrap::{AtEnds, AtInserts, Wrapping};
use construct_materials::MaterialId;
use serde::{Deserialize, Serialize};

/// The owner tag of a wall type's system block.
pub const OWNER: &str = "wall";

/// The newest version of the text this crate writes: 2 for a type that wraps, 1 for one that
/// does not. We may stop writing an old version; we never stop reading it.
pub const FORMAT_VERSION: u32 = 2;

/// The text, field for field, in the order it is written. `wrap` is version 2's only.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    core: StoredCore,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wrap: Option<StoredWrap>,
    layers: Vec<StoredLayer>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredWrap {
    ends: AtEnds,
    inserts: AtInserts,
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
        let wrapping = self.wrap();
        let wrap = (!wrapping.is_none()).then_some(StoredWrap {
            ends: wrapping.ends,
            inserts: wrapping.inserts,
        });
        let stored = Stored {
            version: if wrap.is_some() { FORMAT_VERSION } else { 1 },
            core: StoredCore {
                exterior: self.boundary(Side::Exterior),
                interior: self.boundary(Side::Interior),
            },
            wrap,
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
        let wrapping = match (stored.version, &stored.wrap) {
            (1, None) => Wrapping::default(),
            (2, Some(wrap)) => Wrapping {
                ends: wrap.ends,
                inserts: wrap.inserts,
            },
            (1, Some(_)) => {
                return Err(WallError::Malformed(
                    "version 1 holds no wrap: a type that wraps is version 2".to_owned(),
                ));
            }
            (2, None) => {
                return Err(WallError::Malformed(
                    "version 2 holds a wrap, and it has none".to_owned(),
                ));
            }
            (version, _) => {
                return Err(WallError::Malformed(format!(
                    "version {version} is not one this crate reads"
                )));
            }
        };
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
        let assembly =
            Self::new(layers, stored.core.exterior, stored.core.interior)?.with_wrapping(wrapping);
        if assembly.to_text()? != text {
            return Err(WallError::Malformed(
                "the text is not in its one spelling".to_owned(),
            ));
        }
        Ok(assembly)
    }
}
