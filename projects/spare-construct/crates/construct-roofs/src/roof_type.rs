//! A roof type as an element type of construct-types: of the roofs category, its layers in its
//! system block under the owner tag [`OWNER`], written in a floor type's own spelling
//! (construct-slabs' [`Layers::to_text`]): a roof's layers are a floor's, read from the top (the
//! outer face) down, each measured square to its faces. A roof instance holds no parameter of its
//! own here; what it reads back is worked out ([`crate::read_only`]), never stored.

use crate::error::RoofError;
use construct_categories::{CategoryTable, ROOFS};
use construct_slabs::Layers;
use construct_types::{ElementType, SystemData};
use kit_core::Id;

/// The owner tag of a roof type's system block: its props key is `@system.roof`.
pub const OWNER: &str = "roof";

/// A new roof type of the roofs category, with `layers` in its system block.
///
/// # Errors
/// [`RoofError::Type`] for a name construct-types refuses.
pub fn new_roof_type(
    id: Id,
    name: &str,
    layers: &Layers,
    categories: &CategoryTable,
) -> Result<ElementType, RoofError> {
    let ty = ElementType::new(id, name, ROOFS, categories)?;
    with_layers(ty, layers)
}

/// The same roof type with `layers`: how a changed type's result is kept.
///
/// # Errors
/// [`RoofError::NotARoofType`] if `ty` is not of the roofs category.
pub fn with_layers(ty: ElementType, layers: &Layers) -> Result<ElementType, RoofError> {
    if ty.category() != ROOFS {
        return Err(RoofError::NotARoofType {
            category: ty.category(),
        });
    }
    let block = SystemData::new(OWNER, &layers.to_text()?)?;
    Ok(ty.with_system(block))
}

/// A roof type's layers, read from its system block.
///
/// # Errors
/// [`RoofError::NotARoofType`] if `ty` is not of the roofs category;
/// [`RoofError::NoRoofLayers`] if it has no roof block; construct-slabs' refusals of the text
/// ([`Layers::from_text`]).
pub fn layers_of(ty: &ElementType) -> Result<Layers, RoofError> {
    if ty.category() != ROOFS {
        return Err(RoofError::NotARoofType {
            category: ty.category(),
        });
    }
    match ty.system() {
        Some(block) if block.owner() == OWNER => Ok(Layers::from_text(block.data())?),
        _ => Err(RoofError::NoRoofLayers {
            ty: ty.id().clone(),
        }),
    }
}
