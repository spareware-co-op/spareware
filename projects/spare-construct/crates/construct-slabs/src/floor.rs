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
//! A sloped floor, or one with slab edges ([`crate::shape`], job 5.2), is op version
//! [`SHAPED_VERSION`], its args holding [`SLOPE_ARG`] (the stored slope arrow) or [`EDGES_ARG`]
//! (the stored slab edges, in order) beside the two above; a floor with neither stays version
//! [`OP_VERSION`], byte for byte as before, so an older reader refuses a shaped floor by its
//! version rather than building it flat.
//!
//! Filled ([`filled`]), the recipe the builder takes holds [`LOOPS_ARG`] (the profile of
//! [`Boundary::profile`]: the outer loop anticlockwise, then each hole clockwise, each corner as
//! `{"id":<edge ID>,"at":[x,y]}` in grid steps) and [`LAYERS_ARG`] (each layer from the top down
//! as `{"function":<stored name>,"material":<material ID>,"thickness":<grid steps>}`, as a layered
//! wall's filled layers name their function and material); a sloped floor's also [`TOPS_ARG`]
//! (the top face's height at each corner of each loop, in the loops' order, in grid steps), and
//! one with slab edges [`EDGES_ARG`] (each piece as `{"id":<edge ID>,"kind":<kind>,"top":[[x,y,z]
//! x4],"foot":[[x,y,z] x4]}`, its plan at the floor's underside and at its foot, in grid steps).
//! Nothing else of a floor reaches the builder.

use crate::boundary::Boundary;
use crate::error::SlabError;
use crate::layers::Layers;
use crate::shape::{Shape, SlabEdge, SlopeArrow};
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
/// The version of a floor's recipe this crate writes for a sloped floor or one with slab edges.
pub const SHAPED_VERSION: u32 = 2;
/// The recipe argument that holds a floor's stored slope arrow.
pub const SLOPE_ARG: &str = "slope";
/// The recipe argument that holds a floor's stored slab edges, and, filled, their pieces.
pub const EDGES_ARG: &str = "edges";
/// The filled recipe's argument that holds a sloped floor's top face height at each corner.
pub const TOPS_ARG: &str = "tops";

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

/// A floor's recipe from its whole shape: [`floor_recipe`] for a plain floor, and for a sloped
/// one or one with slab edges version [`SHAPED_VERSION`] with [`SLOPE_ARG`] or [`EDGES_ARG`].
#[must_use]
pub fn shaped_recipe(shape: &Shape) -> Recipe {
    let mut recipe = floor_recipe(shape.boundary());
    if shape.is_plain() {
        return recipe;
    }
    recipe.op_version = SHAPED_VERSION;
    if let Some(slope) = shape.slope() {
        recipe.args.insert(SLOPE_ARG.to_owned(), slope.to_value());
    }
    if !shape.edges().is_empty() {
        let edges = shape.edges().iter().map(SlabEdge::to_value).collect();
        recipe
            .args
            .insert(EDGES_ARG.to_owned(), Value::Array(edges));
    }
    recipe
}

/// The whole shape a floor's recipe holds: its boundary, and its slope and slab edges when it is
/// of version [`SHAPED_VERSION`].
///
/// # Errors
/// [`SlabError::NotAFloor`] for a recipe of another operation, version or owner, without a
/// boundary, of version [`OP_VERSION`] with a slope or slab edges, or of version
/// [`SHAPED_VERSION`] with neither; [`SlabError::Malformed`] for a slope or slab edges not in
/// their one spelling; any refusal of [`Boundary::from_value`] and [`Shape::new`].
pub fn shape_of(recipe: &Recipe) -> Result<Shape, SlabError> {
    let version = recipe.op_version;
    if recipe.owner != "construct"
        || recipe.op != FLOOR_OP
        || !(version == OP_VERSION || version == SHAPED_VERSION)
    {
        return Err(SlabError::NotAFloor(format!(
            "a recipe of {}/{} version {} is not a floor's",
            recipe.owner, recipe.op, recipe.op_version
        )));
    }
    let stored = recipe
        .args
        .get(BOUNDARY_ARG)
        .ok_or_else(|| SlabError::NotAFloor("its recipe holds no boundary".to_owned()))?;
    let boundary = Boundary::from_value(stored)?;
    let slope = recipe.args.get(SLOPE_ARG);
    let edges = recipe.args.get(EDGES_ARG);
    if version == OP_VERSION {
        if slope.is_some() || edges.is_some() {
            return Err(SlabError::NotAFloor(format!(
                "a floor of version {OP_VERSION} has no slope and no slab edges"
            )));
        }
        return Ok(Shape::flat(boundary));
    }
    if slope.is_none() && edges.is_none() {
        return Err(SlabError::NotAFloor(format!(
            "a floor of version {SHAPED_VERSION} has a slope or slab edges"
        )));
    }
    let slope = slope.map(SlopeArrow::from_value).transpose()?;
    let edges = match edges {
        Some(list) => list
            .as_array()
            .filter(|list| !list.is_empty())
            .ok_or_else(|| SlabError::Malformed("slab edges are a list of one or more".to_owned()))?
            .iter()
            .map(SlabEdge::from_value)
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    Shape::new(boundary, slope, edges)
}

/// The boundary a floor's recipe holds.
///
/// # Errors
/// Any refusal of [`shape_of`].
pub fn boundary_of(recipe: &Recipe) -> Result<Boundary, SlabError> {
    shape_of(recipe).map(|shape| shape.boundary().clone())
}

/// Grid points `[x, y]` at height `z`, as the builder takes them.
fn lifted(points: &[construct_solver::Point; 4], z: i64) -> Value {
    Value::Array(points.iter().map(|p| json!([p.x, p.y, z])).collect())
}

/// The recipe the builder takes: the boundary's loops and the layers' thicknesses, and a shaped
/// floor's tops and slab edges, as the module header says.
///
/// # Errors
/// Any refusal of [`shape_of`]; [`SlabError::OutOfRange`] for a slab edge's foot beyond the
/// grid; [`SlabError::Malformed`] only if the JSON writer fails, which it does not for this data.
pub fn filled(recipe: &Recipe, layers: &Layers) -> Result<Recipe, SlabError> {
    let shape = shape_of(recipe)?;
    let boundary = shape.boundary();
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
    let mut listed = Vec::with_capacity(layers.layers().len());
    for layer in layers.layers() {
        let function = serde_json::to_value(layer.function)
            .map_err(|e| SlabError::Malformed(format!("a layer's function: {e}")))?;
        listed.push(json!({
            "function": function,
            "material": layer.material.as_str(),
            "thickness": layer.thickness.steps(),
        }));
    }
    let mut out = recipe.clone();
    out.args = BTreeMap::new();
    out.args.insert(LOOPS_ARG.to_owned(), Value::Array(loops));
    out.args.insert(LAYERS_ARG.to_owned(), Value::Array(listed));
    if !shape.tops().is_empty() {
        out.args.insert(TOPS_ARG.to_owned(), json!(shape.tops()));
    }
    if !shape.pieces().is_empty() {
        let under = layers.total().steps().saturating_neg();
        let mut pieces = Vec::with_capacity(shape.pieces().len());
        for piece in shape.pieces() {
            let foot = under
                .checked_sub(piece.profile.depth().steps())
                .ok_or(SlabError::OutOfRange)?;
            pieces.push(json!({
                "id": piece.edge.as_str(),
                "kind": piece.profile.kind(),
                "top": lifted(&piece.top, under),
                "foot": lifted(&piece.foot, foot),
            }));
        }
        out.args.insert(EDGES_ARG.to_owned(), Value::Array(pieces));
    }
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

/// A floor's record with `boundary` in place of its own: its recipe changes, nothing else. Its
/// slope and slab edges are kept, checked again on the new boundary; a record whose recipe does
/// not read is given a plain floor's.
///
/// # Errors
/// [`SlabError::NotAFloor`] for a record that is not a floor's; any refusal of [`Shape::new`] of
/// the new boundary with the slope and slab edges kept, such as a slab edge on an edge the new
/// boundary lacks.
pub fn with_boundary(mut record: Element, boundary: &Boundary) -> Result<Element, SlabError> {
    if !is_floor(&record) {
        return Err(SlabError::NotAFloor(format!(
            "{} is a {}",
            record.name, record.kind
        )));
    }
    let shape = match shape_of(&record.recipe) {
        Ok(old) => Shape::new(boundary.clone(), old.slope().copied(), old.edges().to_vec())?,
        Err(_) => Shape::flat(boundary.clone()),
    };
    record.recipe = shaped_recipe(&shape);
    Ok(record)
}

/// A floor's record with its slope set, or taken away with `None`: its recipe changes, nothing
/// else.
///
/// # Errors
/// [`SlabError::NotAFloor`] for a record that is not a floor's; any refusal of [`shape_of`] of
/// its recipe and of [`Shape::new`] with the slope.
pub fn with_slope(mut record: Element, slope: Option<SlopeArrow>) -> Result<Element, SlabError> {
    let old = floor_shape(&record)?;
    let shape = Shape::new(old.boundary().clone(), slope, old.edges().to_vec())?;
    record.recipe = shaped_recipe(&shape);
    Ok(record)
}

/// A floor's record with its slab edges set (none for none): its recipe changes, nothing else.
///
/// # Errors
/// [`SlabError::NotAFloor`] for a record that is not a floor's; any refusal of [`shape_of`] of
/// its recipe and of [`Shape::new`] with the slab edges.
pub fn with_edges(mut record: Element, edges: Vec<SlabEdge>) -> Result<Element, SlabError> {
    let old = floor_shape(&record)?;
    let shape = Shape::new(old.boundary().clone(), old.slope().copied(), edges)?;
    record.recipe = shaped_recipe(&shape);
    Ok(record)
}

/// A floor record's shape.
fn floor_shape(record: &Element) -> Result<Shape, SlabError> {
    if !is_floor(record) {
        return Err(SlabError::NotAFloor(format!(
            "{} is a {}",
            record.name, record.kind
        )));
    }
    shape_of(&record.recipe)
}
