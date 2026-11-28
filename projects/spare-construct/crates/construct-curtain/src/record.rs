//! A curtain wall's element record and its recipe (construct-model's FORMAT.md, "Curtain walls").
//!
//! The record: kind [`CURTAIN_KIND`], its type and its category in `props` as every typed
//! element's, its base tied to a level and its top tied to one as a wall's, and the recipe
//! `construct` / [`OP`] with these arguments:
//!
//! - [`START_ARG`] and [`END_ARG`]: its line in the model's plan, each `[x, y]` as construct-params'
//!   stored lengths; walking from the start to the end its exterior is on the right, as a wall's.
//!   A curtain wall embedded in a wall holds [`LENGTH_ARG`] instead, a stored length: its line is
//!   its host's, from where its hosting puts it ([`with_line`] writes the line it is seated on
//!   into the recipe a builder is handed, derived and never stored).
//! - [`HEIGHT_ARG`]: a stored length, or `{"tie":"top"}` read from its top tie when it is built.
//! - [`SPEC_ARG`]: `{"type":"curtain"}` ([`type_arg`]), filled with its type's grid and parts (the
//!   type's block text, [`filled`]) before it is built.
//! - [`GRID_ARG`]: its own edits ([`Edits::to_json`]); left out when it has none.

use crate::edits::Edits;
use crate::error::CurtainError;
use crate::layout::Curtain;
use crate::length;
use crate::spec::CurtainSpec;
use construct_model::geometry::Recipe;
use construct_params::Length;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

/// The kind of a curtain wall's record.
pub const CURTAIN_KIND: &str = "curtain";
/// The operation of a curtain wall.
pub const OP: &str = "wall.curtain";
/// The argument of its line's start.
pub const START_ARG: &str = "start";
/// The argument of its line's end.
pub const END_ARG: &str = "end";
/// The argument of an embedded curtain wall's length along its host.
pub const LENGTH_ARG: &str = "length";
/// The argument of its height.
pub const HEIGHT_ARG: &str = "height";
/// The argument its type's grid and parts are read into.
pub const SPEC_ARG: &str = "curtain";
/// The argument of its own edits.
pub const GRID_ARG: &str = "grid";
/// The owner of Spare Construct's own operations.
const OWN: &str = "construct";

/// Whether `recipe` is a curtain wall's.
#[must_use]
pub fn is_curtain(recipe: &Recipe) -> bool {
    recipe.owner == OWN && recipe.op == OP
}

/// The argument a curtain wall reads its type's grid and parts with: `{"type":"curtain"}`.
#[must_use]
pub fn type_arg() -> Json {
    json!({ "type": "curtain" })
}

/// Whether `recipe` is a curtain wall's that reads its type's grid and parts.
#[must_use]
pub fn reads_type(recipe: &Recipe) -> bool {
    is_curtain(recipe) && recipe.args.get(SPEC_ARG) == Some(&type_arg())
}

/// Whether `recipe` is an embedded curtain wall's: a length along its host in place of a line.
#[must_use]
pub fn is_embedded(recipe: &Recipe) -> bool {
    is_curtain(recipe) && recipe.args.contains_key(LENGTH_ARG)
}

/// A point as a recipe holds it.
fn stored_point([x, y]: [Length; 2]) -> Json {
    json!([length::write(x), length::write(y)])
}

/// The recipe of a free curtain wall from `start` to `end`, `height` high (a stored length's
/// argument, or the top tie's), with its own `edits`.
///
/// # Errors
/// [`CurtainError::Malformed`] only if the edits do not write, which they do.
pub fn curtain_recipe(
    start: [Length; 2],
    end: [Length; 2],
    height: Json,
    edits: &Edits,
) -> Result<Recipe, CurtainError> {
    let mut args = BTreeMap::new();
    args.insert(START_ARG.to_owned(), stored_point(start));
    args.insert(END_ARG.to_owned(), stored_point(end));
    args.insert(HEIGHT_ARG.to_owned(), height);
    args.insert(SPEC_ARG.to_owned(), type_arg());
    with_edits(&Recipe::construct(OP, args), edits)
}

/// The recipe of a curtain wall embedded in a wall, `length` along its host's line and `height`
/// high, with its own `edits`.
///
/// # Errors
/// [`CurtainError::Malformed`] only if the edits do not write, which they do.
pub fn embedded_recipe(
    length: Length,
    height: Length,
    edits: &Edits,
) -> Result<Recipe, CurtainError> {
    let mut args = BTreeMap::new();
    args.insert(LENGTH_ARG.to_owned(), json!(length::write(length)));
    args.insert(HEIGHT_ARG.to_owned(), json!(length::write(height)));
    args.insert(SPEC_ARG.to_owned(), type_arg());
    with_edits(&Recipe::construct(OP, args), edits)
}

/// The same recipe with `edits` as its own: its grid argument, or none when there are none.
///
/// # Errors
/// [`CurtainError::NotACurtainWall`] for another operation's recipe.
pub fn with_edits(recipe: &Recipe, edits: &Edits) -> Result<Recipe, CurtainError> {
    if !is_curtain(recipe) {
        return Err(CurtainError::NotACurtainWall(recipe.op.clone()));
    }
    let mut out = recipe.clone();
    match edits.to_json()? {
        Some(grid) => {
            out.args.insert(GRID_ARG.to_owned(), grid);
        }
        None => {
            out.args.remove(GRID_ARG);
        }
    }
    Ok(out)
}

/// A curtain wall's own edits, from its recipe.
///
/// # Errors
/// [`CurtainError::NotACurtainWall`] for another operation's recipe; [`Edits::from_json`]'s
/// refusals.
pub fn edits_of(recipe: &Recipe) -> Result<Edits, CurtainError> {
    if !is_curtain(recipe) {
        return Err(CurtainError::NotACurtainWall(recipe.op.clone()));
    }
    Edits::from_json(recipe.args.get(GRID_ARG))
}

/// The recipe a curtain wall is built from: its type's grid and parts `spec` read in. A recipe
/// that reads no type ([`reads_type`]) is returned as it is.
///
/// # Errors
/// [`CurtainError::Malformed`] only if the spec does not write, which it does.
pub fn filled(recipe: &Recipe, spec: &CurtainSpec) -> Result<Recipe, CurtainError> {
    if !reads_type(recipe) {
        return Ok(recipe.clone());
    }
    let mut out = recipe.clone();
    out.args
        .insert(SPEC_ARG.to_owned(), Json::String(spec.to_text()?));
    Ok(out)
}

/// The same embedded curtain wall's recipe on the line from `start` to `end` (in the frame it is
/// built in): how a builder is handed it, seated on its host's line.
///
/// # Errors
/// [`CurtainError::NotACurtainWall`] for a recipe that is not an embedded curtain wall's.
pub fn with_line(
    recipe: &Recipe,
    start: [Length; 2],
    end: [Length; 2],
) -> Result<Recipe, CurtainError> {
    if !is_embedded(recipe) {
        return Err(CurtainError::NotACurtainWall(format!(
            "a {} that is not embedded",
            recipe.op
        )));
    }
    let mut out = recipe.clone();
    out.args.remove(LENGTH_ARG);
    out.args.insert(START_ARG.to_owned(), stored_point(start));
    out.args.insert(END_ARG.to_owned(), stored_point(end));
    Ok(out)
}

/// A stored length argument, or the refusal naming it.
fn length_arg(recipe: &Recipe, name: &str) -> Result<Length, CurtainError> {
    recipe
        .args
        .get(name)
        .and_then(Json::as_str)
        .and_then(length::read)
        .ok_or_else(|| CurtainError::Malformed(format!("{name} must be a stored length")))
}

/// A point argument, or the refusal naming it.
fn point_arg(recipe: &Recipe, name: &str) -> Result<[Length; 2], CurtainError> {
    let bad = || CurtainError::Malformed(format!("{name} must be [x, y] as stored lengths"));
    let list = recipe
        .args
        .get(name)
        .and_then(Json::as_array)
        .ok_or_else(bad)?;
    let [x, y] = list.as_slice() else {
        return Err(bad());
    };
    let read = |v: &Json| v.as_str().and_then(length::read).ok_or_else(bad);
    Ok([read(x)?, read(y)?])
}

/// The length of a line from `start` to `end`, to the nearest grid step: exact along an axis.
///
/// # Errors
/// [`CurtainError::NotPositive`] for a line of no length; [`CurtainError::OutOfRange`] for one
/// the arithmetic cannot hold.
pub fn line_length(start: [Length; 2], end: [Length; 2]) -> Result<Length, CurtainError> {
    let [sx, sy] = start;
    let [ex, ey] = end;
    let dx = length::difference(ex, sx).ok_or(CurtainError::OutOfRange)?;
    let dy = length::difference(ey, sy).ok_or(CurtainError::OutOfRange)?;
    let length = length::hypot(dx.steps(), dy.steps()).ok_or(CurtainError::OutOfRange)?;
    if length <= length::ZERO {
        return Err(CurtainError::NotPositive {
            what: "curtain wall's length".to_owned(),
            value: length,
        });
    }
    Ok(length)
}

/// The length an embedded curtain wall's recipe says, before it is seated.
///
/// # Errors
/// [`CurtainError::NotACurtainWall`] for a recipe that is not an embedded curtain wall's;
/// [`CurtainError::Malformed`] for a length that does not read.
pub fn embedded_length(recipe: &Recipe) -> Result<Length, CurtainError> {
    if !is_embedded(recipe) {
        return Err(CurtainError::NotACurtainWall(format!(
            "a {} that is not embedded",
            recipe.op
        )));
    }
    length_arg(recipe, LENGTH_ARG)
}

/// The height a curtain wall's recipe says, once its top tie is read.
///
/// # Errors
/// [`CurtainError::Malformed`] for a height that is not a stored length.
pub fn height_of(recipe: &Recipe) -> Result<Length, CurtainError> {
    length_arg(recipe, HEIGHT_ARG)
}

/// A curtain wall as its filled recipe says: its line, its height and its layout's inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filled {
    /// Its line's start, in the frame it is built in.
    pub start: [Length; 2],
    /// Its line's end.
    pub end: [Length; 2],
    /// Its layout's inputs: its length (the line's, to the nearest step), height, type's grid and
    /// parts, and own edits.
    pub curtain: Curtain,
}

impl Filled {
    /// The curtain wall a filled recipe says.
    ///
    /// # Errors
    /// [`CurtainError::NotACurtainWall`] for another operation's recipe;
    /// [`CurtainError::Malformed`] for an argument that does not read, a type's grid and parts not
    /// read in yet, or an embedded wall not yet seated on its host's line; and the refusals of
    /// [`line_length`], [`Curtain::new`], [`CurtainSpec::from_text`] and [`Edits::from_json`].
    pub fn of(recipe: &Recipe) -> Result<Filled, CurtainError> {
        if !is_curtain(recipe) {
            return Err(CurtainError::NotACurtainWall(recipe.op.clone()));
        }
        if is_embedded(recipe) {
            return Err(CurtainError::Malformed(
                "an embedded curtain wall is laid out on its host's line once it is seated there"
                    .to_owned(),
            ));
        }
        let start = point_arg(recipe, START_ARG)?;
        let end = point_arg(recipe, END_ARG)?;
        let height = height_of(recipe)?;
        let text = recipe
            .args
            .get(SPEC_ARG)
            .and_then(Json::as_str)
            .ok_or_else(|| {
                CurtainError::Malformed(format!(
                    "{SPEC_ARG} must hold its type's grid and parts, read in before it is built"
                ))
            })?;
        let spec = CurtainSpec::from_text(text)?;
        let edits = edits_of(recipe)?;
        let curtain = Curtain::new(line_length(start, end)?, height, spec, edits)?;
        Ok(Filled {
            start,
            end,
            curtain,
        })
    }
}
