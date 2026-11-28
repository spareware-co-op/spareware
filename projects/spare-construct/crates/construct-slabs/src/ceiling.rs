//! A ceiling as an element record (construct-model's, unchanged) and its recipe (job A101,
//! CON-A4-F4-P2).
//!
//! A ceiling is a flat layered slab, so it is held as a floor's recipe is and built by the same
//! builders ([`crate::floor`], construct-regen's `floor_plan`): the record's kind and its recipe's
//! operation are [`CEILING_KIND`] and [`CEILING_OP`], its category the ceilings category
//! ([`construct_categories::CEILINGS`]), its recipe version [`crate::OP_VERSION`] with the
//! boundary in its stored form ([`crate::BOUNDARY_ARG`]), the argument that reads its type's
//! layers ([`crate::LAYERS_ARG`]) and, when it has a pattern, [`GRID_ARG`] (the stored
//! [`Grid`]). A ceiling does not slope and has no slab edges: its recipe is never version 2, and
//! one that is is refused.
//!
//! Where it stands differs from a floor's in one way. A floor's tie is its top face; a ceiling's
//! is its exposed face, the underside a person sees from the room: its height is its level's
//! elevation plus its offset, and its layers stand above it, so its upper face is its type's total
//! thickness higher (construct-regen's `levels` places it that way, and a room's volume reads the
//! underside, [`construct_regen`]'s `rooms`). The layers run from the upper face down, so the last
//! is the finish.

use crate::boundary::Boundary;
use crate::error::SlabError;
use crate::floor::{floor_recipe, shape_of};
use crate::tiles::Grid;
use construct_categories::CEILINGS;
use construct_levels::{Tie, write_base};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_types::{CATEGORY_KEY, TYPE_KEY};
use kit_core::Id;
use std::collections::BTreeMap;

/// A ceiling's element kind.
pub const CEILING_KIND: &str = "ceiling";
/// A ceiling's recipe operation.
pub const CEILING_OP: &str = "ceiling";
/// The recipe argument that holds a ceiling's stored grid pattern.
pub const GRID_ARG: &str = "grid";

/// Whether a record is a ceiling.
#[must_use]
pub fn is_ceiling(record: &Element) -> bool {
    record.kind == CEILING_KIND
}

/// A ceiling's recipe from its boundary and its pattern, if it has one.
#[must_use]
pub fn ceiling_recipe(boundary: &Boundary, grid: Option<&Grid>) -> Recipe {
    let mut recipe = floor_recipe(boundary);
    CEILING_OP.clone_into(&mut recipe.op);
    if let Some(grid) = grid {
        recipe.args.insert(GRID_ARG.to_owned(), grid.to_value());
    }
    recipe
}

/// A new ceiling's element record: of type `ceiling_type`, its exposed face tied to a level by
/// `tie`, with `boundary` and `grid`.
#[must_use]
pub fn ceiling_record(
    id: &Id,
    document: &Id,
    name: &str,
    ceiling_type: &Id,
    tie: &Tie,
    boundary: &Boundary,
    grid: Option<&Grid>,
) -> Element {
    let mut props = BTreeMap::new();
    props.insert(CATEGORY_KEY.to_owned(), CEILINGS.0.to_string());
    props.insert(TYPE_KEY.to_owned(), ceiling_type.to_string());
    write_base(&mut props, Some(tie));
    Element {
        id: id.clone(),
        document: document.clone(),
        kind: CEILING_KIND.to_owned(),
        name: name.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: ceiling_recipe(boundary, grid),
        props,
        references: Vec::new(),
    }
}

fn not_a_ceiling(record: &Element) -> SlabError {
    SlabError::NotACeiling(format!("{} is a {}", record.name, record.kind))
}

/// The boundary a ceiling's recipe holds.
///
/// # Errors
/// [`SlabError::NotACeiling`] for a recipe of another operation or version; any refusal of
/// [`Boundary::from_value`].
pub fn ceiling_boundary(recipe: &Recipe) -> Result<Boundary, SlabError> {
    if recipe.op != CEILING_OP {
        return Err(SlabError::NotACeiling(format!(
            "a recipe of {}/{} version {} is not a ceiling's",
            recipe.owner, recipe.op, recipe.op_version
        )));
    }
    shape_of(recipe)
        .map(|shape| shape.boundary().clone())
        .map_err(|error| match error {
            SlabError::NotAFloor(why) => SlabError::NotACeiling(why),
            other => other,
        })
}

/// The grid pattern a ceiling's recipe holds, if it has one.
///
/// # Errors
/// [`SlabError::NotACeiling`] for a recipe of another operation; any refusal of
/// [`Grid::from_value`].
pub fn grid_of(recipe: &Recipe) -> Result<Option<Grid>, SlabError> {
    if recipe.op != CEILING_OP || recipe.owner != "construct" {
        return Err(SlabError::NotACeiling(format!(
            "a recipe of {}/{} is not a ceiling's",
            recipe.owner, recipe.op
        )));
    }
    recipe.args.get(GRID_ARG).map(Grid::from_value).transpose()
}

/// A ceiling's record with `boundary` in place of its own: its recipe changes, nothing else. Its
/// grid is kept as it is.
///
/// # Errors
/// [`SlabError::NotACeiling`] for a record that is not a ceiling's; any refusal of [`grid_of`].
pub fn with_ceiling_boundary(
    mut record: Element,
    boundary: &Boundary,
) -> Result<Element, SlabError> {
    if !is_ceiling(&record) {
        return Err(not_a_ceiling(&record));
    }
    let grid = grid_of(&record.recipe)?;
    record.recipe = ceiling_recipe(boundary, grid.as_ref());
    Ok(record)
}

/// A ceiling's record with its grid set, or taken away with `None`: its recipe changes, nothing
/// else.
///
/// # Errors
/// [`SlabError::NotACeiling`] for a record that is not a ceiling's; any refusal of
/// [`ceiling_boundary`].
pub fn with_grid(mut record: Element, grid: Option<&Grid>) -> Result<Element, SlabError> {
    if !is_ceiling(&record) {
        return Err(not_a_ceiling(&record));
    }
    let boundary = ceiling_boundary(&record.recipe)?;
    record.recipe = ceiling_recipe(&boundary, grid);
    Ok(record)
}
