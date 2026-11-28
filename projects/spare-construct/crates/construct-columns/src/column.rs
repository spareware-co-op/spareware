//! A column as an element record (construct-model's, unchanged), and its recipe.
//!
//! **The record**: kind [`COLUMN_KIND`], `level` 0, placement at the origin (the position is in the
//! recipe, in exact steps), no references, and in `props` its category
//! ([`construct_categories::COLUMNS`]), its type, its base's tie to a level and, for a column
//! that stands to a level, its top's tie (construct-levels' keys, as a wall's top is tied). Its
//! recipe: op [`COLUMN_OP`] version [`OP_VERSION`], args:
//!
//! | Argument | Holds |
//! | --- | --- |
//! | [`AT_ARG`] | [`Position`]: `{"point":[x,y]}` (stored lengths) or `{"grids":[a,b],"cross":n}` (two grid lines' IDs and which of their crossings, counted from 0 in order of x then y) |
//! | [`TURN_ARG`] | the turn about the column's centre, a stored angle (`angle:<units>`) |
//! | [`SECTION_ARG`] | `{"type":"section"}`: read from the column's type when it is built |
//! | [`HEIGHT_ARG`] | `{"tie":"top"}` (the height from the base to the top's level, read when it is built) or a stored length (an unconnected height) |
//!
//! **Filled** ([`filled_by_type`], then the regenerator reads the height from the levels, then
//! [`finish`]), the recipe the builders take holds only [`RING_ARG`] (the section's corners about
//! the column's centre, turned, in grid steps, anticlockwise from above: `[[x,y],...]`),
//! [`HEIGHT_ARG`] (a stored length above zero), [`MATERIAL_ARG`] (a material ID) and [`GRIDS_ARG`]
//! (the IDs of the grid lines it stands on, for the regenerator's reads: a moved grid builds the
//! column again). The column stands from `z = 0`, its base (the height its built row holds), up
//! to `z = height`.

use crate::error::ColumnError;
use crate::section::Section;
use crate::stored::ColumnSpec;
use construct_categories::COLUMNS;
use construct_grids::Datums;
use construct_levels::{Level, Tie, span, top_of, write_base, write_top};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Angle, Length, Value};
use construct_types::{CATEGORY_KEY, TYPE_KEY};
use kit_core::Id;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

/// A column's element kind.
pub const COLUMN_KIND: &str = "column";
/// A column's recipe operation.
pub const COLUMN_OP: &str = "column";
/// The version of a column's recipe this crate writes.
pub const OP_VERSION: u32 = 1;
/// The recipe argument that holds the position.
pub const AT_ARG: &str = "at";
/// The recipe argument that holds the turn.
pub const TURN_ARG: &str = "turn";
/// The recipe argument that reads the type's section, and, filled by the type, holds it.
pub const SECTION_ARG: &str = "section";
/// The recipe argument that holds the height, or reads it from the top's tie.
pub const HEIGHT_ARG: &str = "height";
/// The filled recipe's argument that holds the ring of corners.
pub const RING_ARG: &str = "ring";
/// The filled recipe's argument that holds the material's ID.
pub const MATERIAL_ARG: &str = "material";
/// The filled recipe's argument that lists the grid lines the column stands on.
pub const GRIDS_ARG: &str = "grids";
/// The record's `props` key that says a column was let go of a grid line: the text it holds is
/// the line's name, so a warning can say which ([`released`]).
pub const RELEASED_KEY: &str = "released_from";
/// The tallest a column is: 100 m.
pub const MOST_HEIGHT: Length = Length::from_mm(100_000);

/// Where a column stands in plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Position {
    /// At a point, in exact steps.
    Point {
        /// Along x.
        x: Length,
        /// Along y.
        y: Length,
    },
    /// Where two grid lines cross, and it follows them when they move.
    Grids {
        /// The first grid line's ID.
        first: Id,
        /// The second grid line's ID.
        second: Id,
        /// Which of their crossings, counted from 0 in order of x then y. A straight line against
        /// a straight line crosses once; an arc crosses a line or an arc twice at most.
        cross: usize,
    },
}

/// How a column's top is given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Top {
    /// At a level's elevation plus an offset: moving the level moves the top.
    Level(Tie),
    /// An unconnected height above the base.
    Height(Length),
}

/// What a column's record says apart from its type and its base: where it stands, its turn and
/// how its top is given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    /// Where it stands in plan.
    pub position: Position,
    /// Its turn about its centre.
    pub turn: Angle,
    /// How its top is given.
    pub top: Top,
}

fn stored_length(length: Length) -> String {
    Value::Length(length).stored()
}

fn read_length(text: &Json) -> Option<Length> {
    match Value::from_stored(text.as_str()?)? {
        Value::Length(length) if Value::Length(length).stored() == text.as_str()? => Some(length),
        _ => None,
    }
}

impl Position {
    fn to_value(&self) -> Json {
        match self {
            Self::Point { x, y } => json!({ "point": [stored_length(*x), stored_length(*y)] }),
            Self::Grids {
                first,
                second,
                cross,
            } => json!({ "grids": [first.as_str(), second.as_str()], "cross": cross }),
        }
    }

    fn from_value(value: &Json) -> Result<Self, ColumnError> {
        let bad = |why: &str| ColumnError::NotAColumn(format!("its {AT_ARG} {why}"));
        let object = value.as_object().ok_or_else(|| bad("is not an object"))?;
        if let Some(point) = object.get("point").filter(|_| object.len() == 1) {
            let point = point.as_array().ok_or_else(|| bad("has no point"))?;
            let (Some(x), Some(y), 2) = (
                point.first().and_then(read_length),
                point.get(1).and_then(read_length),
                point.len(),
            ) else {
                return Err(bad("holds a point that is not two stored lengths"));
            };
            return Ok(Self::Point { x, y });
        }
        let grids = object
            .get("grids")
            .and_then(Json::as_array)
            .filter(|_| object.len() == 2)
            .ok_or_else(|| bad("is neither a point nor two grids"))?;
        let cross = object
            .get("cross")
            .and_then(Json::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| bad("holds a crossing that is not a whole number"))?;
        let id = |at: usize| {
            grids
                .get(at)
                .and_then(Json::as_str)
                .and_then(|text| Id::parse(text).ok())
        };
        let (Some(first), Some(second), 2) = (id(0), id(1), grids.len()) else {
            return Err(bad("names grids that are not two IDs"));
        };
        Ok(Self::Grids {
            first,
            second,
            cross,
        })
    }

    /// The IDs of the grid lines it stands on: none for a point.
    #[must_use]
    pub fn grids(&self) -> Vec<&Id> {
        match self {
            Self::Point { .. } => Vec::new(),
            Self::Grids { first, second, .. } => vec![first, second],
        }
    }

    /// The point it stands at, in grid steps, as `datums` have the grid lines now.
    ///
    /// # Errors
    /// [`ColumnError::Grid`] for a grid `datums` lack; [`ColumnError::NoCrossing`] when the two
    /// do not cross; [`ColumnError::NoSuchCrossing`] when they cross fewer times than `cross`.
    pub fn resolve(&self, datums: &Datums) -> Result<[i64; 2], ColumnError> {
        match self {
            Self::Point { x, y } => Ok([x.steps(), y.steps()]),
            Self::Grids {
                first,
                second,
                cross,
            } => {
                let found = datums.crossings_of(first, second)?;
                let at = found.get(*cross).ok_or_else(|| {
                    if found.is_empty() {
                        ColumnError::NoCrossing {
                            first: first.clone(),
                            second: second.clone(),
                        }
                    } else {
                        ColumnError::NoSuchCrossing {
                            first: first.clone(),
                            second: second.clone(),
                            wanted: *cross,
                            found: found.len(),
                        }
                    }
                })?;
                Ok([at.point.x.steps(), at.point.y.steps()])
            }
        }
    }
}

/// The argument a column's recipe reads its type's section with: `{"type":"section"}`.
#[must_use]
pub fn section_arg() -> Json {
    json!({ "type": "section" })
}

/// The argument a column's recipe reads its height from its top's tie with: `{"tie":"top"}`.
#[must_use]
pub fn tie_arg() -> Json {
    json!({ "tie": "top" })
}

/// Whether a recipe is a column's, filled or not.
#[must_use]
pub fn is_column_recipe(recipe: &Recipe) -> bool {
    recipe.owner == "construct" && recipe.op == COLUMN_OP
}

/// Whether a recipe reads its type's section, so it is built from its type.
#[must_use]
pub fn reads_section(recipe: &Recipe) -> bool {
    is_column_recipe(recipe) && recipe.args.get(SECTION_ARG) == Some(&section_arg())
}

/// A column's recipe from its position, turn and the way its top is given.
#[must_use]
pub fn column_recipe(column: &Column) -> Recipe {
    let mut args = BTreeMap::new();
    args.insert(AT_ARG.to_owned(), column.position.to_value());
    args.insert(
        TURN_ARG.to_owned(),
        Json::from(Value::Angle(column.turn).stored()),
    );
    args.insert(SECTION_ARG.to_owned(), section_arg());
    args.insert(
        HEIGHT_ARG.to_owned(),
        match &column.top {
            Top::Level(_) => tie_arg(),
            Top::Height(height) => Json::from(stored_length(*height)),
        },
    );
    let mut recipe = Recipe::construct(COLUMN_OP, args);
    recipe.op_version = OP_VERSION;
    recipe
}

/// The position and the turn a column's recipe holds, and its height argument: `None` for a top
/// read from its tie, the length for an unconnected height.
fn parts_of(recipe: &Recipe) -> Result<(Position, Angle, Option<Length>), ColumnError> {
    if recipe.owner != "construct" || recipe.op != COLUMN_OP || recipe.op_version != OP_VERSION {
        return Err(ColumnError::NotAColumn(format!(
            "a recipe of {}/{} version {} is not a column's",
            recipe.owner, recipe.op, recipe.op_version
        )));
    }
    let arg = |key: &str| {
        recipe
            .args
            .get(key)
            .ok_or_else(|| ColumnError::NotAColumn(format!("its recipe holds no {key}")))
    };
    let position = Position::from_value(arg(AT_ARG)?)?;
    let turn_text = arg(TURN_ARG)?.as_str().unwrap_or_default();
    let turn = match Value::from_stored(turn_text) {
        Some(Value::Angle(turn)) if Value::Angle(turn).stored() == turn_text => turn,
        _ => {
            return Err(ColumnError::NotAColumn(
                "its turn is not a stored angle".to_owned(),
            ));
        }
    };
    let height = arg(HEIGHT_ARG)?;
    let height = if *height == tie_arg() {
        None
    } else {
        Some(read_length(height).ok_or_else(|| {
            ColumnError::NotAColumn(
                "its height is neither its top's tie nor a stored length".to_owned(),
            )
        })?)
    };
    Ok((position, turn, height))
}

/// What a column's record says: its position, turn and top.
///
/// # Errors
/// [`ColumnError::NotAColumn`] for a record that is not a column's, or whose recipe or top's tie
/// is not as this crate writes it; [`ColumnError::Level`] for a tie that is not as written.
pub fn column_of(record: &Element) -> Result<Column, ColumnError> {
    if record.kind != COLUMN_KIND {
        return Err(ColumnError::NotAColumn(format!(
            "{} is a {}",
            record.name, record.kind
        )));
    }
    if record.level != 0 || record.placement != (Placement { x: 0, y: 0, z: 0 }) {
        return Err(ColumnError::NotAColumn(
            "it has a level or a placement of its own".to_owned(),
        ));
    }
    if !record.references.is_empty() {
        return Err(ColumnError::NotAColumn("it has references".to_owned()));
    }
    let (position, turn, height) = parts_of(&record.recipe)?;
    let top = match height {
        Some(height) => Top::Height(height),
        None => Top::Level(top_of(record)?.ok_or_else(|| {
            ColumnError::NotAColumn(
                "its height is read from its top's tie, and it has none".to_owned(),
            )
        })?),
    };
    let column = Column {
        position,
        turn,
        top,
    };
    if column_recipe(&column) != record.recipe {
        return Err(ColumnError::NotAColumn(
            "its recipe is not as a column's is written".to_owned(),
        ));
    }
    Ok(column)
}

/// Whether a record is a column.
#[must_use]
pub fn is_column(record: &Element) -> bool {
    record.kind == COLUMN_KIND
}

/// A new column's element record: of type `column_type`, its base tied by `base`, with `column`.
#[must_use]
pub fn column_record(
    id: &Id,
    document: &Id,
    name: &str,
    column_type: &Id,
    base: &Tie,
    column: &Column,
) -> Element {
    let mut props = BTreeMap::new();
    props.insert(CATEGORY_KEY.to_owned(), COLUMNS.0.to_string());
    props.insert(TYPE_KEY.to_owned(), column_type.to_string());
    write_base(&mut props, Some(base));
    write_top(
        &mut props,
        match &column.top {
            Top::Level(tie) => Some(tie),
            Top::Height(_) => None,
        },
    );
    Element {
        id: id.clone(),
        document: document.clone(),
        kind: COLUMN_KIND.to_owned(),
        name: name.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: column_recipe(column),
        props,
        references: Vec::new(),
    }
}

/// A column's record with `column` in place of its own position, turn and top: its recipe and its
/// top's tie change, nothing else.
///
/// # Errors
/// [`ColumnError::NotAColumn`] for a record that is not a column's.
pub fn with_column(mut record: Element, column: &Column) -> Result<Element, ColumnError> {
    if !is_column(&record) {
        return Err(ColumnError::NotAColumn(format!(
            "{} is a {}",
            record.name, record.kind
        )));
    }
    record.recipe = column_recipe(column);
    write_top(
        &mut record.props,
        match &column.top {
            Top::Level(tie) => Some(tie),
            Top::Height(_) => None,
        },
    );
    Ok(record)
}

/// A column let go of its grid lines: the same column standing at `at` (grid steps) as a plain
/// point, with `grids` (what it stood on that is deleted, as a person reads it: `grid A`) kept in
/// its `props` under
/// [`RELEASED_KEY`] so a warning can say what it stood on. Used when a grid is deleted under a
/// column: it keeps its place, never fails to build. Its turn, type, base and top are as they were.
///
/// # Errors
/// [`ColumnError::NotAColumn`] for a record that is not a column's, or `at` that is not a point
/// a length holds.
pub fn released(record: Element, at: [i64; 2], grids: &str) -> Result<Element, ColumnError> {
    let mut column = column_of(&record)?;
    column.position = Position::Point {
        x: Length::from_steps(at[0]),
        y: Length::from_steps(at[1]),
    };
    let mut record = with_column(record, &column)?;
    record
        .props
        .insert(RELEASED_KEY.to_owned(), grids.to_owned());
    Ok(record)
}

/// What a column was let go of, when it was ([`released`]).
#[must_use]
pub fn released_from(record: &Element) -> Option<&str> {
    if record.kind != COLUMN_KIND {
        return None;
    }
    record.props.get(RELEASED_KEY).map(String::as_str)
}

/// A column's record with the note that it was let go of its grid lines taken away (a person
/// has placed it again).
#[must_use]
pub fn kept(mut record: Element) -> Element {
    record.props.remove(RELEASED_KEY);
    record
}

/// The height from a base to a top: construct-levels' [`span`], and checked to stand up.
///
/// # Errors
/// [`ColumnError::NoHeight`] when the top is at the base; [`ColumnError::TopBelowBase`], naming how
/// far below, when it is under it; [`ColumnError::Level`] for a height that does not fit a length.
pub fn height_between(
    base: (&Level, Length),
    top: (&Level, Length),
) -> Result<Length, ColumnError> {
    let height = span(base, top)?;
    check_height(height)
}

/// A height checked to be above zero and at most [`MOST_HEIGHT`].
///
/// # Errors
/// [`ColumnError::NoHeight`] for zero; [`ColumnError::TopBelowBase`] for a negative height;
/// [`ColumnError::BadSize`] for one past the tallest a column is.
pub fn check_height(height: Length) -> Result<Length, ColumnError> {
    if height.steps() == 0 {
        return Err(ColumnError::NoHeight);
    }
    if height.steps() < 0 {
        return Err(ColumnError::TopBelowBase {
            by: Length::from_steps(
                height
                    .steps()
                    .checked_neg()
                    .ok_or(ColumnError::OutOfRange)?,
            ),
        });
    }
    if height > MOST_HEIGHT {
        return Err(ColumnError::BadSize(format!(
            "a column is at most {MOST_HEIGHT} tall, and this one is {height}"
        )));
    }
    Ok(height)
}

/// The recipe with its section read from the type's `spec`: [`SECTION_ARG`] filled with the
/// section as the type stores it, and [`MATERIAL_ARG`] with its material. A recipe that reads no
/// type this way is returned as it is.
///
/// # Errors
/// [`ColumnError::Malformed`] only if the JSON writer fails, which it does not for this data.
pub fn filled_by_type(recipe: &Recipe, spec: &ColumnSpec) -> Result<Recipe, ColumnError> {
    if !reads_section(recipe) {
        return Ok(recipe.clone());
    }
    let text = spec.to_text()?;
    let stored: Json =
        serde_json::from_str(&text).map_err(|e| ColumnError::Malformed(e.to_string()))?;
    let mut out = recipe.clone();
    out.args.insert(
        SECTION_ARG.to_owned(),
        stored.get("section").cloned().unwrap_or(Json::Null),
    );
    out.args
        .insert(MATERIAL_ARG.to_owned(), Json::from(spec.material.as_str()));
    Ok(out)
}

/// The section a recipe filled by its type holds.
fn section_of(recipe: &Recipe) -> Result<Section, ColumnError> {
    let value = recipe.args.get(SECTION_ARG).ok_or_else(|| {
        ColumnError::NotAColumn("its section is read from its type, and was not".to_owned())
    })?;
    let bad =
        || ColumnError::NotAColumn("its filled section is not as a type writes it".to_owned());
    let object = value.as_object().ok_or_else(bad)?;
    let length = |key: &str| object.get(key).and_then(read_length).ok_or_else(bad);
    match object.get("kind").and_then(Json::as_str) {
        Some("rect") => Section::rectangular(length("width")?, length("depth")?),
        Some("round") => Section::round(length("diameter")?),
        _ => Err(bad()),
    }
}

/// The recipe the builders take, as the module header says: the section's ring about the column's
/// position as `datums` have its grid lines now, turned; the height; the material; and the grid
/// lines it stands on. `recipe` is filled by the type ([`filled_by_type`]) and has its height
/// read from the levels as a stored length.
///
/// # Errors
/// [`ColumnError::NotAColumn`] for a recipe not so filled; the position's refusals
/// ([`Position::resolve`]); the sizes' and the height's refusals ([`check_height`]);
/// [`ColumnError::OutOfRange`] for a corner past the grid.
pub fn finish(recipe: &Recipe, datums: &Datums) -> Result<Recipe, ColumnError> {
    let (position, turn, _) = parts_of(recipe)?;
    let section = section_of(recipe)?;
    let height = recipe
        .args
        .get(HEIGHT_ARG)
        .and_then(read_length)
        .ok_or_else(|| {
            ColumnError::NotAColumn("its height was not read from its levels".to_owned())
        })?;
    let height = check_height(height)?;
    let material = recipe.args.get(MATERIAL_ARG).cloned().ok_or_else(|| {
        ColumnError::NotAColumn("its material is read from its type, and was not".to_owned())
    })?;
    let centre = position.resolve(datums)?;
    let ring = section.ring(centre, turn)?;
    let mut out = recipe.clone();
    out.args = BTreeMap::new();
    out.args.insert(
        RING_ARG.to_owned(),
        Json::Array(ring.iter().map(|[x, y]| json!([x, y])).collect()),
    );
    out.args
        .insert(HEIGHT_ARG.to_owned(), Json::from(stored_length(height)));
    out.args.insert(MATERIAL_ARG.to_owned(), material);
    out.args.insert(
        GRIDS_ARG.to_owned(),
        Json::Array(
            position
                .grids()
                .iter()
                .map(|id| Json::from(id.as_str()))
                .collect(),
        ),
    );
    Ok(out)
}

/// The grid lines a column's own recipe stands on, whether or not it is filled: what the
/// regenerator lists as read, so a moved or made grid builds the column again even when the
/// column failed to build before.
#[must_use]
pub fn grids_read(recipe: &Recipe) -> Vec<Id> {
    if !is_column_recipe(recipe) {
        return Vec::new();
    }
    match recipe.args.get(AT_ARG).map(Position::from_value) {
        Some(Ok(position)) => position.grids().into_iter().cloned().collect(),
        _ => recipe
            .args
            .get(GRIDS_ARG)
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .filter_map(Json::as_str)
            .filter_map(|text| Id::parse(text).ok())
            .collect(),
    }
}
