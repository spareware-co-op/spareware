//! How a floor's recipe reads its layers from its floor type and has its boundary checked (job
//! 4.3, CON-A4-F4-P1). The regenerator fills a floor's recipe through this one module before it
//! is built, so a floor is built from what construct-slabs says it is.
//!
//! What it promises:
//! - A floor's recipe argument written `{"type":"layers"}` (construct-slabs' `layers_arg`) is
//!   filled, before the shape is built, with the layers of the element's floor type (its
//!   `@system.floor` block, construct-slabs' `layers_of`), and its stored boundary is checked and
//!   replaced by the loops the builders take (construct-slabs' `filled`; [`crate::floor_plan`]
//!   reads the result). A recipe that reads no type this way is returned as it is.
//! - A recipe that reads its type this way reads the document's parameter table and the type
//!   ([`reads_type`]): the regenerator lists both in its `reads` row, so a change to the type's
//!   layers builds every floor of it again, and a floor changed to another type is built with the
//!   new type's layers. Its level is read as every element's is ([`crate::levels`]): the floor's
//!   top face stands at its level's elevation plus its offset, which its built row holds.
//!
//! Depends on construct-model (the recipe), construct-types (the type) and construct-slabs (the
//! layers, the boundary and the filled form). Nothing here is interim.

use construct_model::geometry::{GeometryError, Recipe};
use construct_slabs::{SlabError, filled as slabs_filled, layers_of, reads_layers};
use construct_types::ElementType;

fn failure(code: &str, message: String) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message,
    }
}

/// Whether a recipe is a floor's that reads its floor type's layers.
#[must_use]
pub fn reads_type(recipe: &Recipe) -> bool {
    crate::floor_plan::is_floor(recipe) && reads_layers(recipe)
}

/// The recipe a floor is built from: its layers read from its floor type `ty` and its boundary
/// checked and written as its loops. A recipe that reads no type ([`reads_type`]) is returned as
/// it is.
///
/// # Errors
/// A build failure, which is model state like any other: `NoFloorType` for a floor that names no
/// type, `BadFloorType` for a type that is not a floor type or whose layers do not read, and
/// `BadBoundary` for a boundary construct-slabs refuses, each in construct-slabs' words.
pub fn filled(recipe: &Recipe, ty: Option<&ElementType>) -> Result<Recipe, GeometryError> {
    if !reads_type(recipe) {
        return Ok(recipe.clone());
    }
    let Some(ty) = ty else {
        return Err(failure(
            "NoFloorType",
            "its layers are read from its floor type, and it names none".to_owned(),
        ));
    };
    let layers = layers_of(ty)
        .map_err(|e: SlabError| failure("BadFloorType", format!("its type {}: {e}", ty.id())))?;
    slabs_filled(recipe, &layers).map_err(|e| failure("BadBoundary", e.to_string()))
}
