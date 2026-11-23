//! A room as an element record (construct-model's, unchanged), its recipe, and the recipe the
//! builders take once the regenerator has found its loop.
//!
//! The record: kind [`ROOM_KIND`], placement at the origin (the room's point is where it is), its
//! name in the record's `name`, and in `props` its category ([`construct_categories::ROOMS`]),
//! its number under [`NUMBER_KEY`] (text, as people number rooms: `101`, `G.04`), its base tied
//! to its level (construct-levels' `@level` and `@level.offset`) and, when its upper limit is
//! the level above, its top tied to that level (`@top.level` and `@top.offset`).
//!
//! Its recipe: op [`ROOM_OP`] version [`OP_VERSION`], args
//! - [`AT_ARG`]: the room's point, `[x, y]` as two construct-params stored lengths;
//! - [`HEIGHT_ARG`]: `{"tie":"top"}` when the upper limit is the level above (the regenerator
//!   fills it with the height from the room's base to its top tie, as a stored length), else a
//!   given height as a stored length;
//! - [`WALLS_ARG`]: `{"room":"walls"}` ([`walls_arg`]), which the regenerator fills with the
//!   room's loop, found from the walls of its level.
//!
//! Filled ([`filled`]), the recipe the builders take holds [`HEIGHT_ARG`] (a stored length) and
//! [`LOOPS_ARG`]: the outer loop anticlockwise, then each hole clockwise, each corner as
//! `{"at":[x,y],"wall":<the ID of the wall whose face runs from it>}` in grid steps. Nothing else
//! of a room reaches the builders.
//!
//! Interim: a sloped upper limit is refused by name ([`RoomError::SlopedLimit`]) until the
//! kernel can bound a volume by a sloped plane (the interim register's row).

use crate::error::RoomError;
use crate::plane::Found;
use construct_categories::ROOMS;
use construct_levels::{Tie, top_of, write_base, write_top};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Length, Value};
use construct_types::CATEGORY_KEY;
use kit_core::Id;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

/// A room's element kind.
pub const ROOM_KIND: &str = "room";
/// A room's recipe operation.
pub const ROOM_OP: &str = "room";
/// The version of a room's recipe this crate writes.
pub const OP_VERSION: u32 = 1;
/// The recipe argument that holds the room's point.
pub const AT_ARG: &str = "at";
/// The recipe argument that holds the room's height, or reads it from its top tie.
pub const HEIGHT_ARG: &str = "height";
/// The recipe argument the regenerator fills with the room's loop.
pub const WALLS_ARG: &str = "walls";
/// The filled recipe's argument that holds the loops the builders extrude.
pub const LOOPS_ARG: &str = "loops";
/// On a room: its number, as text.
pub const NUMBER_KEY: &str = "@room.number";

/// A room's upper limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Limit {
    /// The level above (or any level), with an offset from it: the room's top is tied to it and
    /// follows it when it moves.
    Level(Tie),
    /// A given height above the room's base.
    Height(Length),
    /// A sloped limit (under a roof, a sloped ceiling): refused for now.
    Sloped,
}

/// The argument a room's recipe reads its loop with: `{"room":"walls"}`.
#[must_use]
pub fn walls_arg() -> Json {
    json!({ "room": "walls" })
}

/// The argument a recipe reads its top tie with, as construct-regen's `levels` fills it:
/// `{"tie":"top"}`.
fn top_arg() -> Json {
    json!({ "tie": "top" })
}

/// Whether a recipe is a room's (any version: a version this crate does not write is refused
/// when it is read).
#[must_use]
pub fn is_room_recipe(recipe: &Recipe) -> bool {
    recipe.owner == "construct" && recipe.op == ROOM_OP
}

/// Whether a record is a room.
#[must_use]
pub fn is_room(record: &Element) -> bool {
    record.kind == ROOM_KIND
}

/// Whether a recipe reads its loop from the walls around it, so it is filled before it is built.
#[must_use]
pub fn reads_walls(recipe: &Recipe) -> bool {
    is_room_recipe(recipe) && recipe.args.get(WALLS_ARG) == Some(&walls_arg())
}

fn stored(length: Length) -> Json {
    Json::from(Value::Length(length).stored())
}

/// A room's recipe: its point and its upper limit.
///
/// # Errors
/// [`RoomError::SlopedLimit`] for a sloped limit.
pub fn room_recipe(at: [Length; 2], limit: &Limit) -> Result<Recipe, RoomError> {
    let height = match limit {
        Limit::Level(_) => top_arg(),
        Limit::Height(height) => stored(*height),
        Limit::Sloped => return Err(RoomError::SlopedLimit),
    };
    let mut args = BTreeMap::new();
    args.insert(AT_ARG.to_owned(), json!([stored(at[0]), stored(at[1])]));
    args.insert(HEIGHT_ARG.to_owned(), height);
    args.insert(WALLS_ARG.to_owned(), walls_arg());
    let mut recipe = Recipe::construct(ROOM_OP, args);
    recipe.op_version = OP_VERSION;
    Ok(recipe)
}

/// A new room's record: named `name`, numbered `number`, its point `at`, its base tied by `base`,
/// up to `limit`.
///
/// # Errors
/// [`RoomError::SlopedLimit`] for a sloped limit; [`RoomError::NoHeight`] for a given height at
/// or below zero.
pub fn room_record(
    id: &Id,
    document: &Id,
    name: &str,
    number: &str,
    at: [Length; 2],
    base: &Tie,
    limit: &Limit,
) -> Result<Element, RoomError> {
    if let Limit::Height(height) = limit
        && height.steps() <= 0
    {
        return Err(RoomError::NoHeight);
    }
    let recipe = room_recipe(at, limit)?;
    let mut props = BTreeMap::new();
    props.insert(CATEGORY_KEY.to_owned(), ROOMS.0.to_string());
    props.insert(NUMBER_KEY.to_owned(), number.to_owned());
    write_base(&mut props, Some(base));
    if let Limit::Level(top) = limit {
        write_top(&mut props, Some(top));
    }
    Ok(Element {
        id: id.clone(),
        document: document.clone(),
        kind: ROOM_KIND.to_owned(),
        name: name.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe,
        props,
        references: Vec::new(),
    })
}

/// A room's number, if it has one.
#[must_use]
pub fn number_of(record: &Element) -> Option<&str> {
    record.props.get(NUMBER_KEY).map(String::as_str)
}

fn not_a_room(recipe: &Recipe) -> Result<(), RoomError> {
    if !is_room_recipe(recipe) || recipe.op_version != OP_VERSION {
        return Err(RoomError::NotARoom(format!(
            "a recipe of {}/{} version {} is not a room's",
            recipe.owner, recipe.op, recipe.op_version
        )));
    }
    Ok(())
}

fn length(value: &Json, what: &str) -> Result<Length, RoomError> {
    let text = value
        .as_str()
        .ok_or_else(|| RoomError::Malformed(format!("{what} is not a stored length")))?;
    match Value::from_stored(text) {
        Some(Value::Length(length)) => Ok(length),
        _ => Err(RoomError::Malformed(format!(
            "{what} is {text}, not a stored length"
        ))),
    }
}

/// A room's point, in grid steps.
///
/// # Errors
/// [`RoomError::NotARoom`] for another recipe; [`RoomError::Malformed`] for a point that is not
/// two stored lengths.
pub fn at_of(recipe: &Recipe) -> Result<[i64; 2], RoomError> {
    not_a_room(recipe)?;
    match recipe
        .args
        .get(AT_ARG)
        .and_then(Json::as_array)
        .map(Vec::as_slice)
    {
        Some([x, y]) => Ok([
            length(x, "its point's x")?.steps(),
            length(y, "its point's y")?.steps(),
        ]),
        _ => Err(RoomError::Malformed(
            "its point is not [x, y] as two stored lengths".to_owned(),
        )),
    }
}

/// A room's upper limit, as its record holds it.
///
/// # Errors
/// [`RoomError::NotARoom`] for another record; [`RoomError::SlopedLimit`] for a limit written
/// `{"slope": ...}`; [`RoomError::Malformed`] for anything else that is not as the module header
/// says, or a top tie that is missing or not as written.
pub fn limit_of(record: &Element) -> Result<Limit, RoomError> {
    not_a_room(&record.recipe)?;
    let height = record
        .recipe
        .args
        .get(HEIGHT_ARG)
        .ok_or_else(|| RoomError::Malformed("it has no height".to_owned()))?;
    if *height == top_arg() {
        let tie = top_of(record)
            .map_err(|e| RoomError::Malformed(e.to_string()))?
            .ok_or_else(|| {
                RoomError::Malformed("its height reads a top tie it does not have".to_owned())
            })?;
        return Ok(Limit::Level(tie));
    }
    if height.as_object().is_some_and(|o| o.contains_key("slope")) {
        return Err(RoomError::SlopedLimit);
    }
    Ok(Limit::Height(length(height, "its height")?))
}

/// The recipe the builders take: the room's height (already a stored length, as the
/// regenerator fills it) and its loops from `found`, as the module header says.
///
/// # Errors
/// [`RoomError::NotARoom`] for another recipe; [`RoomError::SlopedLimit`] for a sloped limit;
/// [`RoomError::Malformed`] for a height that is not yet a stored length.
pub fn filled(recipe: &Recipe, found: &Found) -> Result<Recipe, RoomError> {
    not_a_room(recipe)?;
    let height = recipe
        .args
        .get(HEIGHT_ARG)
        .ok_or_else(|| RoomError::Malformed("it has no height".to_owned()))?;
    if height.as_object().is_some_and(|o| o.contains_key("slope")) {
        return Err(RoomError::SlopedLimit);
    }
    let height = length(height, "its height")?;
    let ring = |corners: &[crate::plane::Corner]| {
        Json::Array(
            corners
                .iter()
                .map(|c| json!({ "at": c.at, "wall": c.wall.as_str() }))
                .collect(),
        )
    };
    let mut loops = vec![ring(&found.outer)];
    loops.extend(found.holes.iter().map(|hole| ring(hole)));
    let mut out = recipe.clone();
    out.args = BTreeMap::new();
    out.args.insert(HEIGHT_ARG.to_owned(), stored(height));
    out.args.insert(LOOPS_ARG.to_owned(), Json::Array(loops));
    Ok(out)
}

/// The next free room number among `used`: one more than the largest number made only of digits
/// (written as wide as that one, so `009` is followed by `010`), or `1` where there is none; a
/// number already used is skipped. What a person is offered when they place a room.
#[must_use]
pub fn next_number<'a>(used: impl IntoIterator<Item = &'a str>) -> String {
    let used: Vec<&str> = used.into_iter().collect();
    let mut largest: Option<(u64, usize)> = None;
    for number in &used {
        let digits = !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit());
        if let (true, Ok(value)) = (digits, number.parse::<u64>())
            && largest.is_none_or(|(most, _)| value >= most)
        {
            largest = Some((value, number.len()));
        }
    }
    let (mut next, width) = largest.map_or((1_u64, 1), |(value, width)| {
        (value.saturating_add(1), width)
    });
    loop {
        let text = format!("{next:0width$}");
        if !used.contains(&text.as_str()) {
            return text;
        }
        next = next.saturating_add(1);
    }
}
