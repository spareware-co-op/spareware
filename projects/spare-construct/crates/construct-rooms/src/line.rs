//! Room separation lines and area boundary lines as element records (jobs 6.3 and 14.6,
//! CON-A4-F7-P1 and CON-A4-F7-P2): a line of two grid points on a level, with no thickness, that
//! bounds the space on both sides of it for the search ([`crate::plane::Outline::Line`]).
//!
//! Two kinds, kept apart as their own records and never one for the other (an area boundary is
//! not a room line, as the plan says):
//! - a room separation line: kind [`ROOM_LINE_KIND`], recipe op [`ROOM_LINE_OP`], the rooms
//!   category; it splits what the walls leave open into rooms, and every room on its level reads it
//!   as it reads a wall;
//! - an area boundary line: kind [`AREA_LINE_KIND`], recipe op [`AREA_LINE_OP`], the areas
//!   category, and `@area.scheme` naming the area scheme it belongs to; areas of that scheme (and
//!   no other) are found from it.
//!
//! Both are written the same way: the base tied to the level (construct-levels' `@level` and
//! `@level.offset`; a line is drawn in a plan view of its level), and a recipe with
//! [`FROM_ARG`] and [`TO_ARG`], each a point `[x, y]` of two construct-params stored lengths.
//! The line has no shape of its own beyond a flat display: construct-regen's `flat` module builds
//! it, so a plan and the 3D view can draw it, and rooms and areas read it by its ends.
//!
//! A line that crosses nothing or bounds nothing is still kept: nothing here asks that it
//! touches a wall.

use crate::error::RoomError;
use construct_categories::{AREAS, ROOMS};
use construct_levels::{Tie, write_base};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Length, Value};
use construct_types::CATEGORY_KEY;
use kit_core::Id;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

/// A room separation line's element kind.
pub const ROOM_LINE_KIND: &str = "room_line";
/// A room separation line's recipe operation.
pub const ROOM_LINE_OP: &str = "room.line";
/// An area boundary line's element kind.
pub const AREA_LINE_KIND: &str = "area_line";
/// An area boundary line's recipe operation.
pub const AREA_LINE_OP: &str = "area.line";
/// The version of a line's recipe this crate writes.
pub const LINE_OP_VERSION: u32 = 1;
/// The recipe argument holding a line's first end.
pub const FROM_ARG: &str = "from";
/// The recipe argument holding a line's second end.
pub const TO_ARG: &str = "to";
/// On an area boundary line, an area, and a plan view of areas: the ID of the area scheme.
pub const SCHEME_KEY: &str = "@area.scheme";

/// Which of the two kinds of line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// A room separation line.
    Room,
    /// An area boundary line.
    Area,
}

impl LineKind {
    /// Its element kind.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Room => ROOM_LINE_KIND,
            Self::Area => AREA_LINE_KIND,
        }
    }

    /// Its recipe operation.
    #[must_use]
    pub const fn op(self) -> &'static str {
        match self {
            Self::Room => ROOM_LINE_OP,
            Self::Area => AREA_LINE_OP,
        }
    }
}

fn stored(length: Length) -> Json {
    Json::from(Value::Length(length).stored())
}

/// Whether a record is a room separation line.
#[must_use]
pub fn is_room_line(record: &Element) -> bool {
    record.kind == ROOM_LINE_KIND
}

/// Whether a record is an area boundary line.
#[must_use]
pub fn is_area_line(record: &Element) -> bool {
    record.kind == AREA_LINE_KIND
}

/// Whether a recipe is a line's (of either kind).
#[must_use]
pub fn is_line_recipe(recipe: &Recipe) -> bool {
    recipe.owner == "construct" && (recipe.op == ROOM_LINE_OP || recipe.op == AREA_LINE_OP)
}

/// A line's recipe: its two ends.
///
/// # Errors
/// [`RoomError::ZeroLine`] for two ends that are one point.
pub fn line_recipe(
    kind: LineKind,
    from: [Length; 2],
    to: [Length; 2],
) -> Result<Recipe, RoomError> {
    if from == to {
        return Err(RoomError::ZeroLine);
    }
    let mut args = BTreeMap::new();
    args.insert(
        FROM_ARG.to_owned(),
        json!([stored(from[0]), stored(from[1])]),
    );
    args.insert(TO_ARG.to_owned(), json!([stored(to[0]), stored(to[1])]));
    let mut recipe = Recipe::construct(kind.op(), args);
    recipe.op_version = LINE_OP_VERSION;
    Ok(recipe)
}

fn line_record(
    kind: LineKind,
    id: &Id,
    document: &Id,
    scheme: Option<&Id>,
    base: &Tie,
    from: [Length; 2],
    to: [Length; 2],
) -> Result<Element, RoomError> {
    let recipe = line_recipe(kind, from, to)?;
    let mut props = BTreeMap::new();
    let category = match kind {
        LineKind::Room => ROOMS,
        LineKind::Area => AREAS,
    };
    props.insert(CATEGORY_KEY.to_owned(), category.0.to_string());
    if let Some(scheme) = scheme {
        props.insert(SCHEME_KEY.to_owned(), scheme.to_string());
    }
    write_base(&mut props, Some(base));
    Ok(Element {
        id: id.clone(),
        document: document.clone(),
        kind: kind.kind().to_owned(),
        name: String::new(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe,
        props,
        references: Vec::new(),
    })
}

/// A new room separation line's record, on the level `base` ties it to.
///
/// # Errors
/// [`RoomError::ZeroLine`] for two ends that are one point.
pub fn room_line_record(
    id: &Id,
    document: &Id,
    base: &Tie,
    from: [Length; 2],
    to: [Length; 2],
) -> Result<Element, RoomError> {
    line_record(LineKind::Room, id, document, None, base, from, to)
}

/// A new area boundary line's record, in the area scheme `scheme`, on the level `base` ties it to.
///
/// # Errors
/// [`RoomError::ZeroLine`] for two ends that are one point.
pub fn area_line_record(
    id: &Id,
    document: &Id,
    scheme: &Id,
    base: &Tie,
    from: [Length; 2],
    to: [Length; 2],
) -> Result<Element, RoomError> {
    line_record(LineKind::Area, id, document, Some(scheme), base, from, to)
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

fn end(recipe: &Recipe, key: &str) -> Result<[i64; 2], RoomError> {
    match recipe
        .args
        .get(key)
        .and_then(Json::as_array)
        .map(Vec::as_slice)
    {
        Some([x, y]) => Ok([
            length(x, &format!("its {key}'s x"))?.steps(),
            length(y, &format!("its {key}'s y"))?.steps(),
        ]),
        _ => Err(RoomError::Malformed(format!(
            "its {key} is not [x, y] as two stored lengths"
        ))),
    }
}

/// A line's two ends, in grid steps.
///
/// # Errors
/// [`RoomError::NotARoom`] for a recipe that is not a line's or of a version not read here;
/// [`RoomError::Malformed`] for an end that is not two stored lengths; [`RoomError::ZeroLine`] for
/// two ends that are one point.
pub fn ends_of(recipe: &Recipe) -> Result<[[i64; 2]; 2], RoomError> {
    if !is_line_recipe(recipe) || recipe.op_version != LINE_OP_VERSION {
        return Err(RoomError::NotARoom(format!(
            "a recipe of {}/{} version {} is not a line's",
            recipe.owner, recipe.op, recipe.op_version
        )));
    }
    let ends = [end(recipe, FROM_ARG)?, end(recipe, TO_ARG)?];
    if ends[0] == ends[1] {
        return Err(RoomError::ZeroLine);
    }
    Ok(ends)
}

/// The area scheme an area boundary line (or an area) belongs to, if its record names one.
#[must_use]
pub fn scheme_of(record: &Element) -> Option<Id> {
    record
        .props
        .get(SCHEME_KEY)
        .and_then(|text| Id::parse(text).ok())
}

/// The edge of the closed `outline` (a wall's outline in plan, grid points) that runs nearest the
/// point `near`, as its two end points: what picking a wall's face by clicking beside it asks. The
/// distances are compared exactly, as rationals; a tie goes to the first edge. `None` for an
/// outline of fewer than two points.
#[must_use]
pub fn nearest_edge(outline: &[[i64; 2]], near: [i64; 2]) -> Option<[[i64; 2]; 2]> {
    crate::exact::nearest_edge(outline, near)
}
