//! A floor as an element record (construct-model's, unchanged), and its recipe: the boundary in
//! its stored form and the argument that reads the floor's type's layers; and the recipe the
//! builder takes once those are filled in.
//!
//! The record: kind [`FLOOR_KIND`], placement at the origin (the boundary's corners are where the
//! floor is; a move changes the placement and rebuilds nothing), `level` 0, no references, and
//! in `props` its category ([`construct_categories::FLOORS`]), its type and its tie to a level
//! (construct-levels' keys). Its recipe: op [`FLOOR_OP`] version [`OP_VERSION`], args
//! [`BOUNDARY_ARG`] (the stored boundary, [`Boundary::to_value`]) and [`LAYERS_ARG`]
//! ([`layers_arg`], `{"type":"layers"}`, which the regenerator fills from the type).
//!
//! Filled ([`filled`]), the recipe the builder takes holds only [`LOOPS_ARG`] (the profile of
//! [`Boundary::profile`]: the outer loop anticlockwise, then each hole clockwise, each corner as
//! `{"id":<edge ID>,"at":[x,y]}` in grid steps) and [`LAYERS_ARG`] (each layer's thickness in grid
//! steps, top first). Nothing else of a floor reaches the builder.

use crate::boundary::Boundary;
use crate::error::SlabError;
use crate::layers::Layers;
use construct_categories::FLOORS;
use construct_levels::{Tie, write_base};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_types::{CATEGORY_KEY, TYPE_KEY};
use kit_core::Id;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// A floor's element kind.
pub const FLOOR_KIND: &str = "floor";
/// A floor's recipe operation.
pub const FLOOR_OP: &str = "floor";
/// The version of a floor's recipe this crate writes.
pub const OP_VERSION: u32 = 1;
/// The recipe argument that holds the stored boundary.
pub const BOUNDARY_ARG: &str = "boundary";
/// The recipe argument that reads the type's layers, and, filled, holds their thicknesses.
pub const LAYERS_ARG: &str = "layers";
/// The filled recipe's argument that holds the loops the builder extrudes.
pub const LOOPS_ARG: &str = "loops";

/// The argument a floor's recipe reads its type's layers with: `{"type":"layers"}`.
#[must_use]
pub fn layers_arg() -> Value {
    json!({ "type": "layers" })
}

/// Whether a recipe reads its element's type's layers, so it is built from its type.
#[must_use]
pub fn reads_layers(recipe: &Recipe) -> bool {
    recipe.args.values().any(|arg| *arg == layers_arg())
}

/// A floor's recipe from its boundary.
#[must_use]
pub fn floor_recipe(boundary: &Boundary) -> Recipe {
    let mut args = BTreeMap::new();
    args.insert(BOUNDARY_ARG.to_owned(), boundary.to_value());
    args.insert(LAYERS_ARG.to_owned(), layers_arg());
    let mut recipe = Recipe::construct(FLOOR_OP, args);
    recipe.op_version = OP_VERSION;
    recipe
}

/// The boundary a floor's recipe holds.
///
/// # Errors
/// [`SlabError::NotAFloor`] for a recipe of another operation, version or owner, or without a
/// boundary; any refusal of [`Boundary::from_value`].
pub fn boundary_of(recipe: &Recipe) -> Result<Boundary, SlabError> {
    if recipe.owner != "construct" || recipe.op != FLOOR_OP || recipe.op_version != OP_VERSION {
        return Err(SlabError::NotAFloor(format!(
            "a recipe of {}/{} version {} is not a floor's",
            recipe.owner, recipe.op, recipe.op_version
        )));
    }
    let stored = recipe
        .args
        .get(BOUNDARY_ARG)
        .ok_or_else(|| SlabError::NotAFloor("its recipe holds no boundary".to_owned()))?;
    Boundary::from_value(stored)
}

/// The recipe the builder takes: the boundary's loops and the layers' thicknesses, as the
/// module header says.
///
/// # Errors
/// Any refusal of [`boundary_of`].
pub fn filled(recipe: &Recipe, layers: &Layers) -> Result<Recipe, SlabError> {
    let boundary = boundary_of(recipe)?;
    let loops: Vec<Value> = boundary
        .profile()
        .into_iter()
        .map(|ring| {
            Value::Array(
                ring.into_iter()
                    .map(|(id, at)| json!({ "id": id.as_str(), "at": [at.x, at.y] }))
                    .collect(),
            )
        })
        .collect();
    let thicknesses: Vec<Value> = layers
        .layers()
        .iter()
        .map(|layer| Value::from(layer.thickness.steps()))
        .collect();
    let mut out = recipe.clone();
    out.args = BTreeMap::new();
    out.args.insert(LOOPS_ARG.to_owned(), Value::Array(loops));
    out.args
        .insert(LAYERS_ARG.to_owned(), Value::Array(thicknesses));
    Ok(out)
}

/// Whether a record is a floor.
#[must_use]
pub fn is_floor(record: &Element) -> bool {
    record.kind == FLOOR_KIND
}

/// A new floor's element record: of type `floor_type`, its top tied to a level by `tie`, with
/// `boundary`.
#[must_use]
pub fn floor_record(
    id: &Id,
    document: &Id,
    name: &str,
    floor_type: &Id,
    tie: &Tie,
    boundary: &Boundary,
) -> Element {
    let mut props = BTreeMap::new();
    props.insert(CATEGORY_KEY.to_owned(), FLOORS.0.to_string());
    props.insert(TYPE_KEY.to_owned(), floor_type.to_string());
    write_base(&mut props, Some(tie));
    Element {
        id: id.clone(),
        document: document.clone(),
        kind: FLOOR_KIND.to_owned(),
        name: name.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: floor_recipe(boundary),
        props,
        references: Vec::new(),
    }
}

/// A floor's record with `boundary` in place of its own: its recipe changes, nothing else.
///
/// # Errors
/// [`SlabError::NotAFloor`] for a record that is not a floor's.
pub fn with_boundary(mut record: Element, boundary: &Boundary) -> Result<Element, SlabError> {
    if !is_floor(&record) {
        return Err(SlabError::NotAFloor(format!(
            "{} is a {}",
            record.name, record.kind
        )));
    }
    record.recipe = floor_recipe(boundary);
    Ok(record)
}
