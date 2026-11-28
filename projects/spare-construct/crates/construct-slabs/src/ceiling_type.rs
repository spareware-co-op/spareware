//! A ceiling type as an element type of construct-types: of the ceilings category, its layers in
//! its system block under the owner tag [`OWNER`], in the one spelling a floor type's layers have
//! ([`Layers::to_text`]). The layers run from the ceiling's upper face down, so the last layer is
//! the finish a person sees from the room, at the ceiling's height ([`crate::ceiling`]). A ceiling
//! instance holds no parameter of its own here; what it reads back is worked out
//! ([`crate::measure`]), never stored.

use crate::error::SlabError;
use crate::layers::Layers;
use construct_categories::{CEILINGS, CategoryTable};
use construct_types::{ElementType, SystemData};
use kit_core::Id;

/// The owner tag of a ceiling type's system block.
pub const OWNER: &str = "ceiling";

/// A new ceiling type of the ceilings category, with `layers` in its system block.
///
/// # Errors
/// [`SlabError::Type`] for a name construct-types refuses.
pub fn new_ceiling_type(
    id: Id,
    name: &str,
    layers: &Layers,
    categories: &CategoryTable,
) -> Result<ElementType, SlabError> {
    let ty = ElementType::new(id, name, CEILINGS, categories)?;
    with_ceiling_layers(ty, layers)
}

/// The same ceiling type with `layers`: how a changed type's result is kept.
///
/// # Errors
/// [`SlabError::NotACeilingType`] if `ty` is not of the ceilings category.
pub fn with_ceiling_layers(ty: ElementType, layers: &Layers) -> Result<ElementType, SlabError> {
    if ty.category() != CEILINGS {
        return Err(SlabError::NotACeilingType {
            category: ty.category(),
        });
    }
    let block = SystemData::new(OWNER, &layers.to_text()?)?;
    Ok(ty.with_system(block))
}

/// A ceiling type's layers, read from its system block.
///
/// # Errors
/// [`SlabError::NotACeilingType`] if `ty` is not of the ceilings category;
/// [`SlabError::NoCeilingLayers`] if it has no ceiling block; [`SlabError::Malformed`] if the
/// text is not in its one spelling; any refusal of [`Layers::new`].
pub fn ceiling_layers_of(ty: &ElementType) -> Result<Layers, SlabError> {
    if ty.category() != CEILINGS {
        return Err(SlabError::NotACeilingType {
            category: ty.category(),
        });
    }
    match ty.system() {
        Some(block) if block.owner() == OWNER => Layers::from_text(block.data()),
        _ => Err(SlabError::NoCeilingLayers {
            ty: ty.id().clone(),
        }),
    }
}

/// The layers of a floor type or a ceiling type: the one reader of a slab's type, for the
/// builders, which build both the same way.
///
/// # Errors
/// [`SlabError::NotAFloorType`] for a type that is of neither category; otherwise the refusal of
/// [`crate::layers_of`] or [`ceiling_layers_of`].
pub fn slab_layers_of(ty: &ElementType) -> Result<Layers, SlabError> {
    if ty.category() == CEILINGS {
        ceiling_layers_of(ty)
    } else {
        crate::floor_type::layers_of(ty)
    }
}
