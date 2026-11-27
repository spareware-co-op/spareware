//! A stair as an element record (construct-model's, unchanged), its recipe, and the recipe the
//! builders take once its rise is read from its levels.
//!
//! The record: kind [`STAIR_KIND`], placement at the origin (the stair's start is where it is),
//! `level` 0, no references, and in `props` its category ([`construct_categories::STAIRS`]), its
//! base's tie to a level and its top's tie to a level (construct-levels' keys). Its recipe: op
//! [`STAIR_OP`] version [`OP_VERSION`], args the stair's stored settings ([`Stair::to_args`]) and
//! [`RISE_ARG`], `{"tie":"top"}`: the regenerator fills it with the height from the stair's base
//! to its top, construct-params' stored length, before it is built, so a stair follows its levels
//! and its record never changes when they move.
//!
//! Filled ([`filled`]), the recipe the builders take holds only [`MATERIAL_ARG`] (the material
//! every piece is drawn in) and [`PIECES_ARG`]: each piece from the bottom, a run
//! `{"id":"run.<k>","treads":<n>,"plan":[[x,y], ...],"loft":[[[x,y,z], ...],[[x,y,z], ...]]}`, a
//! landing `{"id":"landing.<k>","plan":[[x,y], ...],"extrude":[[x,y,z], ...],"by":[0,0,t]}`, in
//! grid steps, each `plan` its outline anticlockwise from above. Nothing else of a stair reaches
//! the builders.

use crate::error::StairError;
use crate::layout::{Layout, layout};
use crate::shape::{MATERIAL, PieceKind, Solid, shape};
use crate::stair::Stair;
use construct_categories::STAIRS;
use construct_levels::{Tie, write_base, write_top};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Length, Value};
use construct_types::CATEGORY_KEY;
use kit_core::Id;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

/// A stair's element kind.
pub const STAIR_KIND: &str = "stair";
/// A stair's recipe operation.
pub const STAIR_OP: &str = "stair";
/// The version of a stair's recipe this crate writes.
pub const OP_VERSION: u32 = 1;
/// The recipe argument the stair's rise is read into from its levels.
pub const RISE_ARG: &str = "rise";
/// The filled recipe's argument listing the pieces.
pub const PIECES_ARG: &str = "pieces";
/// The filled recipe's argument naming the material.
pub const MATERIAL_ARG: &str = "material";

/// The argument that reads the height from an element's base to its top tie (construct-regen's
/// levels module fills it).
fn top_arg() -> Json {
    json!({ "tie": "top" })
}

/// A stair's recipe: its settings, and its rise read from its levels.
#[must_use]
pub fn stair_recipe(stair: &Stair) -> Recipe {
    let mut args = stair.to_args();
    args.insert(RISE_ARG.to_owned(), top_arg());
    let mut recipe = Recipe::construct(STAIR_OP, args);
    recipe.op_version = OP_VERSION;
    recipe
}

/// Whether a recipe is a stair's, filled or not.
#[must_use]
pub fn is_stair_recipe(recipe: &Recipe) -> bool {
    recipe.owner == "construct" && recipe.op == STAIR_OP
}

/// The stair a stair's recipe holds (filled with its rise or not).
///
/// # Errors
/// [`StairError::NotAStair`] for a recipe of another operation, version or owner;
/// [`StairError::Malformed`] for a rise that is neither `{"tie":"top"}` nor a stored length; any
/// refusal of [`Stair::from_args`].
pub fn stair_of(recipe: &Recipe) -> Result<Stair, StairError> {
    if !is_stair_recipe(recipe) || recipe.op_version != OP_VERSION {
        return Err(StairError::NotAStair(format!(
            "a recipe of {}/{} version {}",
            recipe.owner, recipe.op, recipe.op_version
        )));
    }
    rise_of(recipe)?;
    let mut args = recipe.args.clone();
    args.remove(RISE_ARG);
    let expected = Stair::from_args(&args)?;
    if expected.to_args().len() != args.len() {
        return Err(StairError::Malformed(
            "A stair's recipe holds an argument it does not have.".to_owned(),
        ));
    }
    Ok(expected)
}

/// The rise a stair's recipe holds: `None` while it reads its levels (`{"tie":"top"}`), the
/// height once filled.
///
/// # Errors
/// [`StairError::Malformed`] for a rise that is neither, or none.
pub fn rise_of(recipe: &Recipe) -> Result<Option<Length>, StairError> {
    let bad = || {
        StairError::Malformed("A stair's rise is {\"tie\":\"top\"} or a stored length.".to_owned())
    };
    let arg = recipe.args.get(RISE_ARG).ok_or_else(bad)?;
    if *arg == top_arg() {
        return Ok(None);
    }
    let text = arg.as_str().ok_or_else(bad)?;
    match Value::from_stored(text) {
        Some(Value::Length(l)) if Value::Length(l).stored() == text => Ok(Some(l)),
        _ => Err(bad()),
    }
}

/// Grid points as JSON.
fn points3(points: &[[i64; 3]]) -> Json {
    Json::Array(points.iter().map(|p| json!(p)).collect())
}

/// The stair and its layout from a recipe filled with its rise.
///
/// # Errors
/// As [`stair_of`]; [`StairError::Malformed`] for a recipe whose rise is not read yet; any
/// refusal of [`crate::layout`].
pub fn laid_out(recipe: &Recipe) -> Result<(Stair, Layout), StairError> {
    let stair = stair_of(recipe)?;
    let rise = rise_of(recipe)?.ok_or_else(|| {
        StairError::Malformed(
            "A stair's rise is read from its levels before it is built.".to_owned(),
        )
    })?;
    let laid = layout(&stair, rise)?;
    Ok((stair, laid))
}

/// The recipe the builders take, as the module header says, from a recipe filled with its rise.
///
/// # Errors
/// Any refusal of [`laid_out`] or [`crate::shape`].
pub fn filled(recipe: &Recipe) -> Result<Recipe, StairError> {
    let (stair, laid) = laid_out(recipe)?;
    let built = shape(&stair, &laid)?;
    let pieces: Vec<Json> = built
        .pieces
        .iter()
        .map(|piece| {
            let plan = Json::Array(piece.plan.iter().map(|p| json!(p)).collect());
            let mut entry = serde_json::Map::new();
            entry.insert("id".to_owned(), json!(piece.id));
            entry.insert("plan".to_owned(), plan);
            if let PieceKind::Run { treads } = piece.kind {
                entry.insert("treads".to_owned(), json!(treads));
            }
            match &piece.solid {
                Solid::Loft {
                    sections: [first, second],
                } => {
                    entry.insert("loft".to_owned(), json!([points3(first), points3(second)]));
                }
                Solid::Extrude { profile, by } => {
                    entry.insert("extrude".to_owned(), points3(profile));
                    entry.insert("by".to_owned(), json!(by));
                }
            }
            Json::Object(entry)
        })
        .collect();
    let mut out = recipe.clone();
    out.args = BTreeMap::new();
    out.args.insert(MATERIAL_ARG.to_owned(), json!(MATERIAL));
    out.args.insert(PIECES_ARG.to_owned(), Json::Array(pieces));
    Ok(out)
}

/// Whether a record is a stair.
#[must_use]
pub fn is_stair(record: &Element) -> bool {
    record.kind == STAIR_KIND
}

/// A new stair's element record: its base tied to a level by `base`, its top by `top`.
#[must_use]
pub fn stair_record(
    id: &Id,
    document: &Id,
    name: &str,
    base: &Tie,
    top: &Tie,
    stair: &Stair,
) -> Element {
    let mut props = BTreeMap::new();
    props.insert(CATEGORY_KEY.to_owned(), STAIRS.0.to_string());
    write_base(&mut props, Some(base));
    write_top(&mut props, Some(top));
    Element {
        id: id.clone(),
        document: document.clone(),
        kind: STAIR_KIND.to_owned(),
        name: name.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: stair_recipe(stair),
        props,
        references: Vec::new(),
    }
}

/// A stair's record with `stair` in place of its own: its recipe changes, nothing else.
///
/// # Errors
/// [`StairError::NotAStair`] for a record that is not a stair's.
pub fn with_stair(mut record: Element, stair: &Stair) -> Result<Element, StairError> {
    if !is_stair(&record) {
        return Err(StairError::NotAStair(format!(
            "{} is a {}",
            record.name, record.kind
        )));
    }
    record.recipe = stair_recipe(stair);
    Ok(record)
}
