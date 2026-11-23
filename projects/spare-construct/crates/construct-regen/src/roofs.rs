//! How a roof's recipe reads its layers from its roof type and has its shape worked out (job 5.3,
//! CON-A4-F4-P3). The regenerator fills a roof's recipe through this one module before it is
//! built, so a roof is built from what construct-roofs says it is.
//!
//! What it promises:
//! - A roof's recipe argument written `{"type":"layers"}` (construct-slabs' `layers_arg`) is
//!   filled, before the shape is built, with the layers of the element's roof type (its
//!   `@system.roof` block, construct-roofs' `layers_of`), and its footprint and edges are checked
//!   and replaced by the shape the builders take (construct-roofs' `filled`: its form, its slopes,
//!   its eaves outline and each layer's solid; [`crate::roof_plan`] reads the result). A recipe
//!   that reads no type this way is returned as it is.
//! - A recipe that reads its type this way reads the document's parameter table and the type
//!   ([`reads_type`]): the regenerator lists both in its `reads` row, so a change to the type's
//!   layers builds every roof of it again, and a roof changed to another type is built with the
//!   new type's layers. Its level is read as every element's is ([`crate::levels`]): the roof's
//!   base stands at its level's elevation plus its offset, which its built row holds, so a moved
//!   level moves it.
//! - What a page shows of a roof ([`read_only_view`]), after the parameters its category has: its
//!   plan and slope areas, thickness, volume, mass, pitch and a gable's ridge height, and each
//!   layer's volume and mass (construct-roofs' `read_only` and `layer_read_only`), worked out each
//!   time it is read, never stored, each shown in the document's units and marked `read-only`, as
//!   a floor's are. Masses come from construct-materials' default library.
//!
//! Depends on construct-model (the recipe), construct-types (the type), construct-roofs (the
//! layers, the roof, the filled form and the quantities), construct-slabs (the layers argument),
//! construct-materials (the library) and construct-params (units). Nothing here is interim.

use crate::parameters::ParameterView;
use crate::slabs::shown;
use construct_materials::Materials;
use construct_model::element::Element;
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::Units;
use construct_roofs::{
    RoofError, filled as roofs_filled, is_roof, layer_read_only, layers_of, quantities, read_only,
    roof_of,
};
use construct_slabs::reads_layers;
use construct_types::ElementType;

fn failure(code: &str, message: String) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message,
    }
}

/// Whether a recipe is a roof's that reads its roof type's layers.
#[must_use]
pub fn reads_type(recipe: &Recipe) -> bool {
    crate::roof_plan::is_roof(recipe) && reads_layers(recipe)
}

/// The recipe a roof is built from: its layers read from its roof type `ty` and its shape worked
/// out. A recipe that reads no type ([`reads_type`]) is returned as it is.
///
/// # Errors
/// A build failure, which is model state like any other: `NoRoofType` for a roof that names no
/// type, `BadRoofType` for a type that is not a roof type or whose layers do not read, and
/// `BadRoof` for a footprint, edges or shape construct-roofs refuses, each in construct-roofs'
/// words.
pub fn filled(recipe: &Recipe, ty: Option<&ElementType>) -> Result<Recipe, GeometryError> {
    if !reads_type(recipe) {
        return Ok(recipe.clone());
    }
    let Some(ty) = ty else {
        return Err(failure(
            "NoRoofType",
            "its layers are read from its roof type, and it names none".to_owned(),
        ));
    };
    let layers = layers_of(ty)
        .map_err(|e: RoofError| failure("BadRoofType", format!("its type {}: {e}", ty.id())))?;
    roofs_filled(recipe, &layers).map_err(|e| failure("BadRoof", e.to_string()))
}

/// A roof's read-only parameters as a page shows them, as the module header says. Not a roof, or
/// one whose footprint or type's layers cannot be read (its built row says why): none.
#[must_use]
pub fn read_only_view(
    element: &Element,
    ty: Option<&ElementType>,
    units: &Units,
) -> Vec<ParameterView> {
    if !is_roof(element) {
        return Vec::new();
    }
    let rows = (|| {
        let roof = roof_of(&element.recipe)?;
        let layers = layers_of(ty.ok_or(RoofError::NotARoof("it names no type".to_owned()))?)?;
        let q = quantities(&roof, &layers, &Materials::default_metric())?;
        let mut rows = read_only(&q)?;
        rows.extend(layer_read_only(&layers, &q)?);
        Ok::<_, RoofError>(rows)
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
