//! A railing as an element record (construct-model's, unchanged), its recipe, and the recipe the
//! builders take once its lines are known.
//!
//! The record: kind [`RAILING_KIND`], placement at the origin (its path is where it is), `level`
//! 0, no references, and in `props` its category ([`construct_categories::RAILINGS`]) and its
//! base's tie to a level (construct-levels' keys). A railing on a stair is tied to the stair's
//! base level when it is made, and is raised or lowered to wherever the stair's base stands
//! ([`crate::railing::StairHost::shift`]), so it follows its stair. Its recipe: op [`RAILING_OP`]
//! version [`OP_VERSION`], args the railing's stored settings ([`Railing::to_args`]).
//!
//! Filled ([`filled`]), the recipe the builders take holds only [`MATERIAL_ARG`] (the material
//! every piece is drawn in) and [`PIECES_ARG`]: each piece in order,
//! `{"id","part","extrude":[[x,y,z], ...],"by":[x,y,z]}` in grid steps (construct-regen's
//! `railing_plan` reads it). Nothing else of a railing reaches the builders.

use crate::error::RailingError;
use crate::layout::{Layout, lay_out};
use crate::railing::{Railing, StairHost};
use crate::shape::shape;
use construct_categories::RAILINGS;
use construct_levels::{Tie, write_base};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_types::CATEGORY_KEY;
use kit_core::Id;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

/// A railing's element kind.
pub const RAILING_KIND: &str = "railing";
/// A railing's recipe operation.
pub const RAILING_OP: &str = "railing";
/// The version of a railing's recipe this crate writes.
pub const OP_VERSION: u32 = 1;
/// The filled recipe's argument listing the pieces.
pub const PIECES_ARG: &str = "pieces";
/// The filled recipe's argument naming the material.
pub const MATERIAL_ARG: &str = "material";

/// A railing's recipe.
#[must_use]
pub fn railing_recipe(railing: &Railing) -> Recipe {
    let mut recipe = Recipe::construct(RAILING_OP, railing.to_args());
    recipe.op_version = OP_VERSION;
    recipe
}

/// Whether a recipe is a railing's, filled or not.
#[must_use]
pub fn is_railing_recipe(recipe: &Recipe) -> bool {
    recipe.owner == "construct" && recipe.op == RAILING_OP
}

/// Whether a record is a railing.
#[must_use]
pub fn is_railing(record: &Element) -> bool {
    record.kind == RAILING_KIND
}

/// The railing a railing's recipe holds.
///
/// # Errors
/// [`RailingError::NotARailing`] for a recipe of another operation, version or owner; any refusal
/// of [`Railing::from_args`].
pub fn railing_of(recipe: &Recipe) -> Result<Railing, RailingError> {
    if !is_railing_recipe(recipe) || recipe.op_version != OP_VERSION {
        return Err(RailingError::NotARailing(format!(
            "A recipe of {}/{} version {}",
            recipe.owner, recipe.op, recipe.op_version
        )));
    }
    Railing::from_args(&recipe.args)
}

/// The stair a railing's recipe follows, if it reads as one on a stair.
#[must_use]
pub fn stair_of(recipe: &Recipe) -> Option<Id> {
    railing_of(recipe).ok().and_then(|r| r.stair().cloned())
}

/// The railing a recipe holds and its layout along its lines, `host` the stair it follows.
///
/// # Errors
/// As [`railing_of`], [`Railing::lines`] and [`lay_out`].
pub fn laid_out(
    recipe: &Recipe,
    host: Option<StairHost<'_>>,
) -> Result<(Railing, Layout), RailingError> {
    let railing = railing_of(recipe)?;
    let lines = railing.lines(host)?;
    let layout = lay_out(railing.kind(), lines)?;
    Ok((railing, layout))
}

/// The recipe the builders take, as the module header says.
///
/// # Errors
/// As [`laid_out`] and [`crate::shape`].
pub fn filled(recipe: &Recipe, host: Option<StairHost<'_>>) -> Result<Recipe, RailingError> {
    let (railing, layout) = laid_out(recipe, host)?;
    let pieces: Vec<Json> = shape(railing.kind(), &layout)?
        .into_iter()
        .map(|p| {
            json!({
                "by": p.by,
                "extrude": p.profile,
                "id": p.id,
                "part": p.part.name(),
            })
        })
        .collect();
    let mut out = recipe.clone();
    out.args = BTreeMap::new();
    out.args
        .insert(MATERIAL_ARG.to_owned(), json!(railing.kind().material));
    out.args.insert(PIECES_ARG.to_owned(), Json::Array(pieces));
    Ok(out)
}

/// A new railing's element record: its base tied to a level by `base`.
#[must_use]
pub fn railing_record(
    id: &Id,
    document: &Id,
    name: &str,
    base: &Tie,
    railing: &Railing,
) -> Element {
    let mut props = BTreeMap::new();
    props.insert(CATEGORY_KEY.to_owned(), RAILINGS.0.to_string());
    write_base(&mut props, Some(base));
    Element {
        id: id.clone(),
        document: document.clone(),
        kind: RAILING_KIND.to_owned(),
        name: name.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: railing_recipe(railing),
        props,
        references: Vec::new(),
    }
}

/// A railing's record with `railing` in place of its own: its recipe changes, nothing else.
///
/// # Errors
/// [`RailingError::NotARailing`] for a record that is not a railing's.
pub fn with_railing(mut record: Element, railing: &Railing) -> Result<Element, RailingError> {
    if !is_railing(&record) {
        return Err(RailingError::NotARailing(record.name));
    }
    record.recipe = railing_recipe(railing);
    Ok(record)
}
