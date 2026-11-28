//! Area schemes and areas as element records (job 14.6, CON-A4-F7-P2): an area scheme (gross,
//! rentable, or another measure) and, in it, areas found inside area boundary lines.
//!
//! - An area scheme: kind [`SCHEME_KIND`], recipe op [`SCHEME_OP`] with no arguments, the areas
//!   category, its name in the record's `name` and its measure in `props` under [`MEASURE_KEY`]
//!   ([`Measure`]). A scheme has no shape: it is a record, as a level is.
//! - An area: kind [`AREA_KIND`], recipe op [`AREA_OP`], the areas category, its name in `name`,
//!   its number in `props` under [`NUMBER_KEY`], its scheme under
//!   [`crate::line::SCHEME_KEY`], its base tied to its level, and a recipe whose [`AT_ARG`] is its
//!   point `[x, y]` (two stored lengths) and whose [`LINES_ARG`] is `{"area":"lines"}`, which the
//!   regenerator fills with the area's loop, found from the scheme's area boundary lines on its
//!   level by the same search rooms use ([`crate::plane::find`], over lines alone: walls bound
//!   no area, as an area's boundary is drawn or picked from a wall, not read from it).
//!
//! Filled ([`filled_area`]), the recipe the builders take holds only [`crate::room::LOOPS_ARG`],
//! as a room's does: the outer loop anticlockwise, then each hole clockwise, each corner
//! `{"at":[x,y],"wall":<the ID of the line that runs from it>}` in grid steps; an area has no
//! height.
//!
//! What an area measures is its area in square grid steps and its perimeter ([`crate::Flat`]),
//! exact, worked out whenever it is read and never stored. Which areas two area schemes hold is
//! never mixed: an area is found from the lines of its own scheme only.

use crate::error::RoomError;
use crate::line::SCHEME_KEY;
use crate::plane::Found;
use crate::room::{AT_ARG, LOOPS_ARG};
use construct_categories::AREAS;
use construct_levels::{Tie, write_base};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Length, Value};
use construct_types::CATEGORY_KEY;
use kit_core::Id;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

/// An area scheme's element kind.
pub const SCHEME_KIND: &str = "area_scheme";
/// An area scheme's recipe operation.
pub const SCHEME_OP: &str = "area.scheme";
/// An area's element kind.
pub const AREA_KIND: &str = "area";
/// An area's recipe operation.
pub const AREA_OP: &str = "area";
/// The version of these recipes this crate writes.
pub const AREA_OP_VERSION: u32 = 1;
/// The recipe argument the regenerator fills with the area's loop.
pub const LINES_ARG: &str = "lines";
/// On an area scheme: its measure, one of the words of [`Measure`].
pub const MEASURE_KEY: &str = "@area.measure";
/// On an area: its number, as text.
pub const NUMBER_KEY: &str = "@area.number";

/// What an area scheme measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    /// The gross area of a building: to the outside of its exterior walls.
    Gross,
    /// The rentable area of a tenancy.
    Rentable,
    /// Any other measure a project keeps.
    Other,
}

impl Measure {
    /// The word it is stored as.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Gross => "gross",
            Self::Rentable => "rentable",
            Self::Other => "other",
        }
    }

    /// The measure a word names.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "gross" => Some(Self::Gross),
            "rentable" => Some(Self::Rentable),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

/// Whether a record is an area scheme.
#[must_use]
pub fn is_scheme(record: &Element) -> bool {
    record.kind == SCHEME_KIND
}

/// Whether a record is an area.
#[must_use]
pub fn is_area(record: &Element) -> bool {
    record.kind == AREA_KIND
}

/// Whether a recipe is an area's.
#[must_use]
pub fn is_area_recipe(recipe: &Recipe) -> bool {
    recipe.owner == "construct" && recipe.op == AREA_OP
}

/// The argument an area's recipe reads its loop with: `{"area":"lines"}`.
#[must_use]
pub fn lines_arg() -> Json {
    json!({ "area": "lines" })
}

/// Whether a recipe reads its loop from the area boundary lines round it, so it is filled before it
/// is built.
#[must_use]
pub fn reads_lines(recipe: &Recipe) -> bool {
    is_area_recipe(recipe) && recipe.args.get(LINES_ARG) == Some(&lines_arg())
}

/// A new area scheme's record, named `name`, measuring `measure`.
///
/// # Errors
/// [`RoomError::Malformed`] for a blank name.
pub fn scheme_record(
    id: &Id,
    document: &Id,
    name: &str,
    measure: Measure,
) -> Result<Element, RoomError> {
    if name.trim().is_empty() {
        return Err(RoomError::Malformed(
            "an area scheme has no name".to_owned(),
        ));
    }
    let mut props = BTreeMap::new();
    props.insert(CATEGORY_KEY.to_owned(), AREAS.0.to_string());
    props.insert(MEASURE_KEY.to_owned(), measure.word().to_owned());
    Ok(Element {
        id: id.clone(),
        document: document.clone(),
        kind: SCHEME_KIND.to_owned(),
        name: name.trim().to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: Recipe::construct(SCHEME_OP, BTreeMap::new()),
        props,
        references: Vec::new(),
    })
}

/// A scheme's measure, as its record holds it.
#[must_use]
pub fn measure_of(record: &Element) -> Option<Measure> {
    record
        .props
        .get(MEASURE_KEY)
        .and_then(|word| Measure::from_word(word))
}

fn stored(length: Length) -> Json {
    Json::from(Value::Length(length).stored())
}

/// An area's recipe: its point, and the loop to be found from the lines round it.
#[must_use]
pub fn area_recipe(at: [Length; 2]) -> Recipe {
    let mut args = BTreeMap::new();
    args.insert(AT_ARG.to_owned(), json!([stored(at[0]), stored(at[1])]));
    args.insert(LINES_ARG.to_owned(), lines_arg());
    let mut recipe = Recipe::construct(AREA_OP, args);
    recipe.op_version = AREA_OP_VERSION;
    recipe
}

/// A new area's record: named `name`, numbered `number`, in the scheme `scheme`, its point `at`,
/// on the level `base` ties it to.
#[must_use]
pub fn area_record(
    id: &Id,
    document: &Id,
    scheme: &Id,
    name: &str,
    number: &str,
    at: [Length; 2],
    base: &Tie,
) -> Element {
    let mut props = BTreeMap::new();
    props.insert(CATEGORY_KEY.to_owned(), AREAS.0.to_string());
    props.insert(NUMBER_KEY.to_owned(), number.to_owned());
    props.insert(SCHEME_KEY.to_owned(), scheme.to_string());
    write_base(&mut props, Some(base));
    Element {
        id: id.clone(),
        document: document.clone(),
        kind: AREA_KIND.to_owned(),
        name: name.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: area_recipe(at),
        props,
        references: Vec::new(),
    }
}

/// An area's number, if it has one.
#[must_use]
pub fn number_of(record: &Element) -> Option<&str> {
    record.props.get(NUMBER_KEY).map(String::as_str)
}

/// An area's point, in grid steps.
///
/// # Errors
/// [`RoomError::NotARoom`] for another recipe; [`RoomError::Malformed`] for a point that is not two
/// stored lengths.
pub fn at_of(recipe: &Recipe) -> Result<[i64; 2], RoomError> {
    if !is_area_recipe(recipe) || recipe.op_version != AREA_OP_VERSION {
        return Err(RoomError::NotARoom(format!(
            "a recipe of {}/{} version {} is not an area's",
            recipe.owner, recipe.op, recipe.op_version
        )));
    }
    let one = |value: &Json| -> Result<i64, RoomError> {
        let text = value.as_str().ok_or_else(|| {
            RoomError::Malformed("its point is not two stored lengths".to_owned())
        })?;
        match Value::from_stored(text) {
            Some(Value::Length(length)) => Ok(length.steps()),
            _ => Err(RoomError::Malformed(format!(
                "its point holds {text}, not a stored length"
            ))),
        }
    };
    match recipe
        .args
        .get(AT_ARG)
        .and_then(Json::as_array)
        .map(Vec::as_slice)
    {
        Some([x, y]) => Ok([one(x)?, one(y)?]),
        _ => Err(RoomError::Malformed(
            "its point is not [x, y] as two stored lengths".to_owned(),
        )),
    }
}

/// The recipe the builders take: the area's loops from `found`, as the module header says.
///
/// # Errors
/// [`RoomError::NotARoom`] for another recipe.
pub fn filled_area(recipe: &Recipe, found: &Found) -> Result<Recipe, RoomError> {
    if !is_area_recipe(recipe) || recipe.op_version != AREA_OP_VERSION {
        return Err(RoomError::NotARoom(format!(
            "a recipe of {}/{} version {} is not an area's",
            recipe.owner, recipe.op, recipe.op_version
        )));
    }
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
    out.args.insert(LOOPS_ARG.to_owned(), Json::Array(loops));
    Ok(out)
}
