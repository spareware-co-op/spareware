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
//! - What a page shows of a floor ([`read_only_view`]), after the parameters its category has:
//!   its area, perimeter, thickness, volume and mass, and each layer's volume and mass
//!   (construct-slabs' `read_only` and `layer_read_only`), worked out from its boundary and its
//!   type each time it is read, never stored, each shown in the document's units and marked
//!   `read-only`. Masses come from construct-materials' default library, the one a project reads
//!   until it keeps its own; a layer of a material the library lacks has no mass, and then nor
//!   has the floor. The openings and shafts that cut a floor (construct-openings, job 4.2) are more
//!   holes in the boundary the quantities are worked out of ([`read_only_view_cut`]), so the area,
//!   volume and mass of a cut floor are exact and no second formula is kept.
//!
//! Depends on construct-model (the recipe), construct-types (the type), construct-slabs (the
//! layers, the boundary, the filled form and the quantities), construct-materials (the library)
//! and construct-params (units). Nothing here is interim.

use crate::parameters::ParameterView;
use construct_materials::Materials;
use construct_model::element::Element;
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::{Definition, Units, Value};
use construct_slabs::{
    SlabError, boundary_of, filled as slabs_filled, is_floor, layer_read_only, layers_of,
    quantities, read_only, reads_layers,
};
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

/// Where a read-only value came from, as a page shows it: worked out, never set.
pub const READ_ONLY: &str = "read-only";

/// A floor's read-only parameters as a page shows them, as the module header says: the floor's
/// five, then each layer's volume and mass, top first. Not a floor, or one whose boundary or
/// type's layers cannot be read (its built row says why): none.
#[must_use]
pub fn read_only_view(
    element: &Element,
    ty: Option<&ElementType>,
    units: &Units,
) -> Vec<ParameterView> {
    read_only_view_cut(element, ty, units, &[])
}

/// [`read_only_view`] of a floor that `cuts` cut (the outlines of the openings and shafts that
/// pass through it, grid steps, its own frame): each is one more hole in every layer, so the
/// quantities are those of the floor after the cut. Cuts that do not pass the floor's own checks
/// show none, as a boundary that does not read does.
#[must_use]
pub fn read_only_view_cut(
    element: &Element,
    ty: Option<&ElementType>,
    units: &Units,
    cuts: &[Vec<[i64; 2]>],
) -> Vec<ParameterView> {
    if !is_floor(element) {
        return Vec::new();
    }
    let rows = (|| {
        let boundary = boundary_of(&element.recipe)?;
        let boundary = if cuts.is_empty() {
            boundary
        } else {
            construct_openings::with_cuts(&boundary, cuts)?
        };
        let layers = layers_of(ty.ok_or(SlabError::NotAFloor("it names no type".to_owned()))?)?;
        let q = quantities(&boundary, &layers, &Materials::default_metric())?;
        let mut rows = read_only(&q)?;
        rows.extend(layer_read_only(&layers, &q)?);
        Ok::<_, SlabError>(rows)
    })();
    rows.map_or_else(
        |_| Vec::new(),
        |rows| {
            rows.into_iter()
                .map(|(def, value)| shown(&def, value, units))
                .collect()
        },
    )
}

/// One read-only parameter as a page shows it: a floor's, and a roof's ([`crate::roofs`]).
pub(crate) fn shown(def: &Definition, value: Option<Value>, units: &Units) -> ParameterView {
    ParameterView {
        id: def.id().to_string(),
        name: def.name().to_owned(),
        kind: def.kind().to_string(),
        holder: "instance",
        shown: value.as_ref().and_then(|v| def.format(v, units).ok()),
        value: value.map(|value| value.stored()),
        from: READ_ONLY,
        formula: None,
        reason: None,
    }
}
