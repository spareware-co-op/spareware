//! A roof as an element record (construct-model's, unchanged), its recipe, and the recipe the
//! builder takes once its type's layers are filled in.
//!
//! The record: kind [`ROOF_KIND`], placement at the origin (the footprint's corners are where the
//! roof is), `level` 0, no references, and in `props` its category
//! ([`construct_categories::ROOFS`]), its type and its base's tie to a level (construct-levels'
//! keys). Its recipe: op [`ROOF_OP`] version [`OP_VERSION`], args [`FOOTPRINT_ARG`] (the stored
//! footprint, construct-slabs' boundary), [`EDGES_ARG`] ([`Roof::edges_value`]) and
//! [`LAYERS_ARG`] (`{"type":"layers"}`, construct-slabs' `layers_arg`, which the regenerator fills
//! from the type).
//!
//! Filled ([`filled`]), the recipe the builder takes holds only:
//! - [`FORM_ARG`]: `flat`, `mono`, `gable` or `solved`;
//! - [`SLOPES_ARG`]: the edges the roof rises from, in the footprint's order (none when flat);
//! - [`OUTLINE_ARG`]: the eaves outline anticlockwise from above, each corner
//!   `{"id":<edge ID>,"at":[x,y]}` in grid steps, the footprint edge whose eaves or verge runs
//!   from it to the next;
//! - [`LINES_ARG`]: the lines of the top face for the page to draw, each
//!   `{"kind":"ridge"|"hip"|"valley"|"eave"|"verge","from":[x,y,z],"to":[x,y,z]}` in grid steps,
//!   as [`crate::Shape::lines`] has them (the builders read none of it);
//! - [`LAYERS_ARG`]: each layer from the top down as `{"function","material","thickness"}` (its
//!   thickness in grid steps, square to its faces) and, for a layer with a thickness, `solid`:
//!   `{"extrude":[[x,y,z], ...],"by":[x,y,z]}`, `{"loft":[[[x,y,z], ...],[[x,y,z], ...]]}` or
//!   `{"unite":[[[x,y,z], ...], ...],"by":[x,y,z]}` in grid steps ([`crate::Solid`]).
//!
//! Nothing else of a roof reaches the builder.

use crate::error::RoofError;
use crate::roof::Roof;
use crate::shape::{Solid, shape};
use construct_categories::ROOFS;
use construct_levels::{Tie, write_base};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_slabs::{Layers, layers_arg};
use construct_types::{CATEGORY_KEY, TYPE_KEY};
use kit_core::Id;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// A roof's element kind.
pub const ROOF_KIND: &str = "roof";
/// A roof's recipe operation.
pub const ROOF_OP: &str = "roof";
/// The version of a roof's recipe this crate writes.
pub const OP_VERSION: u32 = 1;
/// The recipe argument that holds the stored footprint.
pub const FOOTPRINT_ARG: &str = "footprint";
/// The recipe argument that holds what each edge does.
pub const EDGES_ARG: &str = "edges";
/// The recipe argument that reads the type's layers, and, filled, holds them and their solids.
pub const LAYERS_ARG: &str = "layers";
/// The filled recipe's argument naming the form.
pub const FORM_ARG: &str = "form";
/// The filled recipe's argument listing the edges the roof rises from.
pub const SLOPES_ARG: &str = "slopes";
/// The filled recipe's argument holding the eaves outline.
pub const OUTLINE_ARG: &str = "outline";
/// The filled recipe's argument listing the lines of the top face, for the page's plan.
pub const LINES_ARG: &str = "lines";

/// A roof's recipe.
#[must_use]
pub fn roof_recipe(roof: &Roof) -> Recipe {
    let mut args = BTreeMap::new();
    args.insert(FOOTPRINT_ARG.to_owned(), roof.footprint().to_value());
    args.insert(EDGES_ARG.to_owned(), roof.edges_value());
    args.insert(LAYERS_ARG.to_owned(), layers_arg());
    let mut recipe = Recipe::construct(ROOF_OP, args);
    recipe.op_version = OP_VERSION;
    recipe
}

/// The roof a roof's recipe holds.
///
/// # Errors
/// [`RoofError::NotARoof`] for a recipe of another operation, version or owner, or without a
/// footprint or edges; any refusal of [`Roof::from_values`].
pub fn roof_of(recipe: &Recipe) -> Result<Roof, RoofError> {
    if recipe.owner != "construct" || recipe.op != ROOF_OP || recipe.op_version != OP_VERSION {
        return Err(RoofError::NotARoof(format!(
            "a recipe of {}/{} version {} is not a roof's",
            recipe.owner, recipe.op, recipe.op_version
        )));
    }
    let arg = |name: &str| {
        recipe
            .args
            .get(name)
            .ok_or_else(|| RoofError::NotARoof(format!("its recipe holds no {name}")))
    };
    Roof::from_values(arg(FOOTPRINT_ARG)?, arg(EDGES_ARG)?)
}

/// Grid points as JSON.
fn points(points: &[[i64; 3]]) -> Value {
    Value::Array(points.iter().map(|p| json!(p)).collect())
}

/// The recipe the builder takes, as the module header says.
///
/// # Errors
/// Any refusal of [`roof_of`] or [`crate::shape`]; [`RoofError::Malformed`] only if the JSON
/// writer fails, which it does not for this data.
pub fn filled(recipe: &Recipe, layers: &Layers) -> Result<Recipe, RoofError> {
    let roof = roof_of(recipe)?;
    let built = shape(&roof, layers)?;
    let outline: Vec<Value> = built
        .outline
        .iter()
        .map(|(id, at)| json!({ "id": id.as_str(), "at": at }))
        .collect();
    let slopes: Vec<Value> = roof
        .form()
        .eaves()
        .iter()
        .map(|id| json!(id.as_str()))
        .collect();
    let lines: Vec<Value> = built
        .lines
        .iter()
        .map(|line| json!({ "kind": line.kind.name(), "from": line.from, "to": line.to }))
        .collect();
    let mut listed = Vec::with_capacity(layers.layers().len());
    for (at, layer) in layers.layers().iter().enumerate() {
        let function = serde_json::to_value(layer.function)
            .map_err(|e| RoofError::Malformed(format!("a layer's function: {e}")))?;
        let mut entry = serde_json::Map::new();
        entry.insert("function".to_owned(), function);
        entry.insert("material".to_owned(), json!(layer.material.as_str()));
        entry.insert("thickness".to_owned(), json!(layer.thickness.steps()));
        if let Some(solid) = built.solids.iter().find(|solid| solid.layer == at) {
            let solid = match &solid.solid {
                Solid::Extrude { profile, by } => {
                    json!({ "extrude": points(profile), "by": by })
                }
                Solid::Loft {
                    sections: [first, second],
                } => {
                    json!({ "loft": [points(first), points(second)] })
                }
                Solid::Unite { profiles, by } => {
                    let each: Vec<Value> = profiles.iter().map(|p| points(p)).collect();
                    json!({ "unite": each, "by": by })
                }
            };
            entry.insert("solid".to_owned(), solid);
        }
        listed.push(Value::Object(entry));
    }
    let mut out = recipe.clone();
    out.args = BTreeMap::new();
    out.args
        .insert(FORM_ARG.to_owned(), json!(roof.form().name()));
    out.args.insert(SLOPES_ARG.to_owned(), Value::Array(slopes));
    out.args
        .insert(OUTLINE_ARG.to_owned(), Value::Array(outline));
    out.args.insert(LINES_ARG.to_owned(), Value::Array(lines));
    out.args.insert(LAYERS_ARG.to_owned(), Value::Array(listed));
    Ok(out)
}

/// Whether a record is a roof.
#[must_use]
pub fn is_roof(record: &Element) -> bool {
    record.kind == ROOF_KIND
}

/// A new roof's element record: of type `roof_type`, its base tied to a level by `tie`, on `roof`'s
/// footprint with its edges.
#[must_use]
pub fn roof_record(
    id: &Id,
    document: &Id,
    name: &str,
    roof_type: &Id,
    tie: &Tie,
    roof: &Roof,
) -> Element {
    let mut props = BTreeMap::new();
    props.insert(CATEGORY_KEY.to_owned(), ROOFS.0.to_string());
    props.insert(TYPE_KEY.to_owned(), roof_type.to_string());
    write_base(&mut props, Some(tie));
    Element {
        id: id.clone(),
        document: document.clone(),
        kind: ROOF_KIND.to_owned(),
        name: name.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: roof_recipe(roof),
        props,
        references: Vec::new(),
    }
}

/// A roof's record with `roof` (a footprint and edges) in place of its own: its recipe changes,
/// nothing else.
///
/// # Errors
/// [`RoofError::NotARoof`] for a record that is not a roof's.
pub fn with_roof(mut record: Element, roof: &Roof) -> Result<Element, RoofError> {
    if !is_roof(&record) {
        return Err(RoofError::NotARoof(format!(
            "{} is a {}",
            record.name, record.kind
        )));
    }
    record.recipe = roof_recipe(roof);
    Ok(record)
}
