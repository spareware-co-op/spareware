//! How a layered wall's recipe reads its layers from its wall type (job 4.1, CON-A4-F2-P1). The
//! regenerator and the server's commands fill a recipe through [`crate::parameters::recipe_at`],
//! which fills these too, so what a command checks and what a wall is built from cannot differ.
//!
//! What it promises:
//! - A recipe argument written `{"wall":"type"}` ([`type_arg`]) is filled, before the shape is
//!   built, with the layers of the element's wall type, laid out round the location line the
//!   element chose and on the side it chose (construct-walls' [`offsets_of`]: the type's
//!   `@system.wall` block, the instance's `wall.location-line` and `wall.flipped`). Each layer is
//!   written as [`crate::wall_plan`] reads it: its `function` (construct-walls' stored name),
//!   its `material` (the material's ID) and its two faces' offsets from the reference line,
//!   `exterior` and `interior`, each a construct-params stored length (`length:q0:<steps>`),
//!   positive toward the exterior, from the exterior finish face to the interior finish face.
//! - With the layers, the wall's wrapping ([`WRAP`], job 6.1): its type's wrapping at free ends
//!   and at inserts, each replaced by the wall's own where its record gives one (construct-walls'
//!   [`OwnWrap`], the stored recipe argument [`WRAPPING`]), with each layer's wraps flag and the
//!   core's two boundaries, so the join rule and the builders turn the same layers. Written only
//!   when something wraps: a wall that wraps nothing is filled as before, byte for byte.
//! - A recipe that reads its type this way reads the parameter table and the type ([`reads_type`]):
//!   the regenerator lists both in its `reads` row, so a change to the type's layers builds every
//!   wall of it again, and a wall changed to another type is built with the new type's layers.
//!
//! Depends on construct-model (records), construct-params (stored lengths), construct-types (the
//! type) and construct-walls (the layers and where their faces lie). Nothing here is interim: the
//! layers' faces are exact in construct-params' unit, and nothing is rounded.

use crate::parameters::Context;
use crate::wall_plan::WRAP;
use construct_model::element::Element;
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::Value;
use construct_types::ElementType;
use construct_walls::{Assembly, OwnWrap, Side, WRAPPING, assembly_of, offsets_of};
use serde_json::{Value as Json, json};

/// The key of the argument form that reads the layers of the element's wall type.
const WALL_ARG: &str = "wall";
/// What it reads: the type.
const TYPE: &str = "type";

fn failure(code: &str, message: String) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message,
    }
}

/// Whether an argument is `{"wall":"type"}`.
fn is_type_arg(arg: &Json) -> bool {
    arg.as_object().is_some_and(|object| {
        object.len() == 1 && object.get(WALL_ARG).and_then(Json::as_str) == Some(TYPE)
    })
}

/// The argument a recipe reads its wall type's layers with: `{"wall":"type"}`.
#[must_use]
pub fn type_arg() -> Json {
    json!({ WALL_ARG: TYPE })
}

/// Whether any argument of a recipe reads the layers of the element's wall type.
#[must_use]
pub fn reads_type(recipe: &Recipe) -> bool {
    recipe.args.values().any(is_type_arg)
}

/// The layers of `element`, of the wall type `ty`, as a filled recipe lists them.
///
/// # Errors
/// `BadWallType` for a type that is not a wall type, layers its block does not hold, or a
/// location line or side the element's values do not choose, in construct-walls' words.
fn layers(context: &Context, element: &Element, ty: &ElementType) -> Result<Json, GeometryError> {
    let bad = |why: String| failure("BadWallType", format!("its type {}: {why}", ty.id()));
    let instance = context.instance(element, ty).map_err(bad)?;
    let assembly = assembly_of(ty).map_err(|e| bad(e.to_string()))?;
    let offsets = offsets_of(&context.params, ty, &instance).map_err(|e| bad(e.to_string()))?;
    let faces = offsets.faces();
    let mut listed = Vec::with_capacity(assembly.layers().len());
    for (at, layer) in assembly.layers().iter().enumerate() {
        let (Some(exterior), Some(interior)) = (faces.get(at), faces.get(at.saturating_add(1)))
        else {
            return Err(bad(format!("layer {at} has no faces")));
        };
        let function = serde_json::to_value(layer.function)
            .map_err(|e| bad(format!("layer {at}'s function: {e}")))?;
        listed.push(json!({
            "function": function,
            "material": layer.material.as_str(),
            "exterior": Value::Length(*exterior).stored(),
            "interior": Value::Length(*interior).stored(),
        }));
    }
    Ok(Json::Array(listed))
}

/// The [`WRAP`] argument of a wall of `assembly` whose own wrapping is `own`; `None` when it
/// wraps nothing.
fn wrap_arg(assembly: &Assembly, own: OwnWrap) -> Option<Json> {
    let wrapping = assembly.wrap().overridden(own.ends, own.inserts);
    if wrapping.is_none() {
        return None;
    }
    let flags: Vec<bool> = assembly.layers().iter().map(|layer| layer.wraps).collect();
    Some(json!({
        "core": [assembly.boundary(Side::Exterior), assembly.boundary(Side::Interior)],
        "ends": wrapping.ends.key(),
        "inserts": wrapping.inserts.key(),
        "layers": flags,
    }))
}

/// The recipe a shape is built from: each `{"wall":"type"}` argument filled with the layers of
/// `element`'s wall type `ty`, read against `context`, and its wrapping ([`WRAP`]) where it wraps
/// anything. A recipe that reads no type is returned as it is.
///
/// # Errors
/// A build failure, which is model state like any other: `NoWallType` for an element that names
/// no type, `BadWallType` as [`layers`] says.
pub fn filled(
    recipe: &Recipe,
    context: &Context,
    element: &Element,
    ty: Option<&ElementType>,
) -> Result<Recipe, GeometryError> {
    if !reads_type(recipe) {
        return Ok(recipe.clone());
    }
    let Some(ty) = ty else {
        return Err(failure(
            "NoWallType",
            "its layers are read from its wall type, and it names none".to_owned(),
        ));
    };
    let read = layers(context, element, ty)?;
    let mut out = recipe.clone();
    out.args.remove(WRAP);
    for arg in out.args.values_mut() {
        if is_type_arg(arg) {
            *arg = read.clone();
        }
    }
    let bad = |why: String| failure("BadWallType", format!("its type {}: {why}", ty.id()));
    let assembly = assembly_of(ty).map_err(|e| bad(e.to_string()))?;
    let own = OwnWrap::from_json(recipe.args.get(WRAPPING))
        .map_err(|e| failure("BadWrapping", e.to_string()))?;
    if let Some(wrap) = wrap_arg(&assembly, own) {
        out.args.insert(WRAP.to_owned(), wrap);
    }
    Ok(out)
}
