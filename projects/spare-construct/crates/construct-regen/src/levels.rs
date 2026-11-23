//! How an element's shape and place read its levels, and how a document's levels are read. The
//! server's commands (construct-api's `cloud::levels`), the server's page views and the
//! regenerator all read through this one module, so what a command checks and what a shape is
//! built from cannot differ.
//!
//! What it promises:
//! - An element's base stands on the level of its tie, or of its old integer, through
//!   construct-levels' [`base_of`]; how high it sits is construct-levels' [`height_of`], the one
//!   function, and is written in its built row with the level it is indexed under ([`Placed`]).
//! - A recipe argument written `{"tie":"top"}` is filled, before the shape is built, with the
//!   height from the element's base to its top tie (construct-levels' [`span`]), as the whole
//!   millimetres the stand-in's recipes take, or, for a layered wall (`wall.layered`) and a room
//!   (`room`), as a construct-params stored length, exactly. A wall whose top is tied to the level above so
//!   follows that level when it moves, and its record never changes.
//! - The levels a shape reads are [`levels_read`]: its base level, and its top level when its
//!   recipe reads it. The regenerator lists them in its `reads` table, so a change to one of them
//!   builds the element again.
//! - A level read while building is the document's level record as of how far regeneration has
//!   got ([`At::Regenerated`]), else the implied level of an implied ID: the same shape however
//!   the log was reached.
//!
//! Depends on construct-model (records), construct-levels and construct-params (lengths), and
//! the [`crate::parameters`] reader for records. Interim: [`whole_mm`] turns a height into the
//! stand-in's whole-millimetre recipe number, as [`crate::parameters`] does for a parameter; it
//! changes with the kernel's recipes (job 1.1), and nothing else here knows the stand-in's units.

use crate::parameters::{At, read_at, read_document};
use crate::wall_plan;
use construct_levels::{
    LEVEL_KIND, Length, Level, LevelError, Levels, Tie, base_of, height_of, implied_number, show,
    span, tied_to, top_of,
};
use construct_model::element::Element;
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::{Units, Value};
use kit_core::Id;
use kit_store::{Reader, StoreError};
use serde::Serialize;
use std::collections::BTreeSet;

/// The argument form that reads the height from an element's base to its top tie.
const TIE_ARG: &str = "tie";
/// The one tie an argument reads today: the top.
const TOP: &str = "top";

fn failure(code: &str, message: String) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message,
    }
}

/// Whether an argument is `{"tie":"top"}`.
fn is_top_arg(arg: &serde_json::Value) -> bool {
    arg.as_object().is_some_and(|object| {
        object.len() == 1 && object.get(TIE_ARG).and_then(serde_json::Value::as_str) == Some(TOP)
    })
}

/// The argument a recipe reads its top tie with: `{"tie":"top"}`.
#[must_use]
pub fn top_arg() -> serde_json::Value {
    serde_json::json!({ TIE_ARG: TOP })
}

/// Whether any argument of a recipe reads the element's top tie.
#[must_use]
pub fn reads_top(recipe: &Recipe) -> bool {
    recipe.args.values().any(is_top_arg)
}

/// The IDs of the levels an element's shape and place read: its base level, and its top level
/// when its recipe reads it. A tie that cannot be read is none here; building says why.
#[must_use]
pub fn levels_read(element: &Element) -> BTreeSet<Id> {
    let mut read = BTreeSet::new();
    if let Ok(Some(base)) = base_of(element) {
        read.insert(base.level);
    }
    if reads_top(&element.recipe)
        && let Ok(Some(top)) = top_of(element)
    {
        read.insert(top.level);
    }
    read
}

/// A level of a document by ID, as `at` that state: its level record, read strictly, else the
/// implied level of an ID in the implied form, else `None`. A piece of that ID that is not a
/// level is no level record. The inner error is a level record that cannot be read, in words.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn level_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    id: &Id,
    at: At,
) -> Result<Result<Option<Level>, String>, StoreError> {
    match read_at(reader, project, document, LEVEL_KIND, id, at).await? {
        Some(record) if record.kind == LEVEL_KIND => Ok(Level::from_record(&record)
            .map(Some)
            .map_err(|e| e.to_string())),
        _ => Ok(Ok(implied_number(id).and_then(construct_levels::implied))),
    }
}

/// Where an element stands, as its levels say: the level it is indexed under, how high its base
/// sits, and the levels and offsets a recipe's tie arguments read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Placed {
    /// The ID of the level its base is tied to; `None` on no level.
    pub level: Option<Id>,
    /// Its base's height above the project's zero ([`height_of`]); `None` on no level.
    pub elevation: Option<Length>,
    base: Option<(Level, Length)>,
    top: Option<(Level, Length)>,
}

impl Placed {
    /// The elevation in construct-params' stored length text, as a built row holds it.
    #[must_use]
    pub fn stored_elevation(&self) -> Option<String> {
        self.elevation.map(|length| Value::Length(length).stored())
    }
}

/// The level a tie stands on, `at` that state, or the build failure that says why not.
async fn tied<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    tie: Tie,
    at: At,
) -> Result<Result<(Level, Length), GeometryError>, StoreError> {
    Ok(
        match level_at(reader, project, document, &tie.level, at).await? {
            Ok(Some(level)) => Ok((level, tie.offset)),
            Ok(None) => Err(failure(
                "MissingLevel",
                format!(
                    "it is tied to level {}, which is not in this model",
                    tie.level
                ),
            )),
            Err(why) => Err(failure("BadLevel", why)),
        },
    )
}

/// Where an element stands, `at` that state ([`Placed`]).
///
/// # Errors
/// A [`StoreError`] when the store fails; the inner error is a build failure, which is model
/// state like any other: `BadLevel` for a tie that is not as written or a height that does not
/// fit, `MissingLevel` for a tie to a level the model does not have.
pub async fn placed<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    at: At,
) -> Result<Result<Placed, GeometryError>, StoreError> {
    let bad = |e: LevelError| failure("BadLevel", e.to_string());
    let base = match base_of(element) {
        Ok(base) => base,
        Err(e) => return Ok(Err(bad(e))),
    };
    let mut placed = Placed::default();
    if let Some(base) = base {
        placed.level = Some(base.level.clone());
        let (level, offset) = match tied(reader, project, document, base, at).await? {
            Ok(found) => found,
            Err(why) => return Ok(Err(why)),
        };
        placed.elevation = match height_of(&level, offset) {
            Ok(height) => Some(height),
            Err(e) => return Ok(Err(bad(e))),
        };
        placed.base = Some((level, offset));
    }
    if reads_top(&element.recipe) {
        let top = match top_of(element) {
            Ok(Some(top)) => top,
            Ok(None) => {
                return Ok(Err(failure(
                    "BadLevel",
                    "its recipe reads its top tie, and it has none".to_owned(),
                )));
            }
            Err(e) => return Ok(Err(bad(e))),
        };
        placed.top = match tied(reader, project, document, top, at).await? {
            Ok(found) => Some(found),
            Err(why) => return Ok(Err(why)),
        };
    }
    Ok(Ok(placed))
}

/// A height as the number a stand-in recipe takes: whole millimetres. Interim (see the module
/// header): the one place here that knows the recipe's unit.
fn whole_mm(height: Length, name: &str) -> Result<i64, GeometryError> {
    height.whole_mm().ok_or_else(|| {
        failure(
            "BadLevel",
            format!(
                "{name} reads its top tie, which is {height} up: the stand-in builder takes whole millimetres only"
            ),
        )
    })
}

/// The recipe a shape is built from: each `{"tie":"top"}` argument filled with the height from
/// the element's base to its top ([`span`]). A recipe that reads no tie is returned as it is.
///
/// # Errors
/// A build failure: `BadLevel` for a top read with no base, a height that does not fit, or one
/// that is not whole millimetres.
pub fn filled(recipe: &Recipe, placed: &Placed) -> Result<Recipe, GeometryError> {
    let mut out = recipe.clone();
    for (name, arg) in &mut out.args {
        if !is_top_arg(arg) {
            continue;
        }
        let (Some((base, from)), Some((top, to))) = (&placed.base, &placed.top) else {
            return Err(failure(
                "BadLevel",
                format!("{name} reads the height to its top tie, and it stands on no level"),
            ));
        };
        let height =
            span((base, *from), (top, *to)).map_err(|e| failure("BadLevel", e.to_string()))?;
        // A layered wall, a shaft and a room hold their lengths as stored lengths, exactly; the
        // stand-in's old box takes whole millimetres.
        *arg = if wall_plan::is_layered(recipe)
            || construct_openings::is_shaft(recipe)
            || crate::room_plan::is_room(recipe)
        {
            serde_json::Value::from(Value::Length(height).stored())
        } else {
            serde_json::Value::from(whole_mm(height, name)?)
        };
    }
    Ok(out)
}

/// The recipe `element` is built from, as the document is `at` that state, every argument that
/// reads a record filled: its parameters and its wall type's layers
/// ([`crate::parameters::recipe_at`]), then its top tie ([`filled`]). What a command asks of a
/// shape's rules (a door's fit in its wall) is asked of this, as the regenerator builds it.
///
/// # Errors
/// A [`StoreError`] when the store fails. The inner error is the build failure, as
/// [`crate::parameters::recipe_at`], [`placed`] and [`filled`] give it.
pub async fn recipe_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    at: At,
) -> Result<Result<Recipe, GeometryError>, StoreError> {
    let recipe = match crate::parameters::recipe_at(reader, project, document, element, at).await? {
        Ok(recipe) => recipe,
        Err(e) => return Ok(Err(e)),
    };
    if !reads_top(&recipe) {
        return Ok(Ok(recipe));
    }
    Ok(
        match placed(reader, project, document, element, at).await? {
            Ok(placed) => filled(&recipe, &placed),
            Err(e) => Err(e),
        },
    )
}

/// Every level of a document as it is now: its level records and the implied levels its
/// elements use. The inner error is a level record or tie that cannot be read, in words.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn read_levels<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<(Levels, Vec<Element>), String>, StoreError> {
    let records: Vec<Element> = read_document(reader, project, document)
        .await?
        .into_values()
        .map(|(_, record)| record)
        .collect();
    Ok(Levels::from_records(&records)
        .map(|levels| (levels, records))
        .map_err(|e| e.to_string()))
}

/// A level, as a page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LevelView {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its elevation, in construct-params' stored length text.
    pub elevation: String,
    /// The same elevation shown in the document's units, such as `3200 mm`.
    pub shown: String,
    /// Whether it is a building storey.
    pub storey: bool,
    /// Whether it is implied by old records' integers, with no record of its own yet.
    pub implied: bool,
    /// How many elements are tied to it.
    pub tied: usize,
}

/// A document's levels as a page shows them, lowest first.
///
/// # Errors
/// A tie that cannot be read, in words.
pub fn views(
    levels: &Levels,
    records: &[Element],
    units: &Units,
) -> Result<Vec<LevelView>, String> {
    let mut out = Vec::new();
    for level in levels.ordered() {
        out.push(LevelView {
            id: level.id().to_string(),
            name: level.name().to_owned(),
            elevation: Value::Length(level.elevation()).stored(),
            shown: show(level.elevation(), units),
            storey: level.is_storey(),
            implied: levels.is_implied(level.id()),
            tied: tied_to(level.id(), records).map_err(|e| e.to_string())?,
        });
    }
    Ok(out)
}

/// A document's levels as a page shows them now, lowest first, with lengths in construct-params'
/// metric display settings until a project keeps its own. The inner error is a level record or a
/// tie that cannot be read, in words.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn read_views<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<Vec<LevelView>, String>, StoreError> {
    Ok(match read_levels(reader, project, document).await? {
        Ok((levels, records)) => views(&levels, &records, &Units::default()),
        Err(why) => Err(why),
    })
}
