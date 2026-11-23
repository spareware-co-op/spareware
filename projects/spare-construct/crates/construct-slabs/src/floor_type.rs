//! A floor type as an element type of construct-types: of the floors category, its layers in its
//! system block under the owner tag [`OWNER`]. A floor instance holds no parameter of its own
//! here; what it reads back (area, perimeter, volume, mass) is worked out ([`crate::read_only`]),
//! never stored.

use crate::error::SlabError;
use crate::layers::Layers;
use crate::stored::OWNER;
use construct_categories::{CategoryTable, FLOORS};
use construct_types::{ElementType, SystemData};
use kit_core::Id;

/// A new floor type of the floors category, with `layers` in its system block.
///
/// # Errors
/// [`SlabError::Type`] for a name construct-types refuses.
pub fn new_floor_type(
    id: Id,
    name: &str,
    layers: &Layers,
    categories: &CategoryTable,
) -> Result<ElementType, SlabError> {
    let ty = ElementType::new(id, name, FLOORS, categories)?;
    with_layers(ty, layers)
}

/// The same floor type with `layers`: how a changed type's result is kept.
///
/// # Errors
/// [`SlabError::NotAFloorType`] if `ty` is not of the floors category.
pub fn with_layers(ty: ElementType, layers: &Layers) -> Result<ElementType, SlabError> {
    if ty.category() != FLOORS {
        return Err(SlabError::NotAFloorType {
            category: ty.category(),
        });
    }
    let block = SystemData::new(OWNER, &layers.to_text()?)?;
    Ok(ty.with_system(block))
}

/// A floor type's layers, read from its system block.
///
/// # Errors
/// [`SlabError::NotAFloorType`] if `ty` is not of the floors category;
/// [`SlabError::NoFloorLayers`] if it has no floor block; [`SlabError::Malformed`] if the text
/// is not in its one spelling; any refusal of [`Layers::new`].
pub fn layers_of(ty: &ElementType) -> Result<Layers, SlabError> {
    if ty.category() != FLOORS {
        return Err(SlabError::NotAFloorType {
            category: ty.category(),
        });
    }
    match ty.system() {
        Some(block) if block.owner() == OWNER => Layers::from_text(block.data()),
        _ => Err(SlabError::NoFloorLayers {
            ty: ty.id().clone(),
        }),
    }
}
