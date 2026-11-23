//! How a room's recipe finds its loop from the walls of its level (job 5.4, CON-A4-F7-P1), and
//! which rooms a changed wall or room builds again. The regenerator fills a room's recipe here
//! ([`filled`]), after its height is read from its levels; a command or a page view asks
//! [`outcome`], which reads the same walls the same way, so what is shown and what is built cannot
//! differ.
//!
//! What it promises:
//! - A room's recipe argument `{"room":"walls"}` (construct-rooms' `walls_arg`) is filled with
//!   the room's loop, found by construct-rooms from every wall whose base is on the room's level,
//!   as the document stands at the state asked (for building, as regeneration has reached it, by
//!   the kind index, in log order). A wall is read by its outline in plan: a layered wall's
//!   (`wall.layered`) by the four corners of its outer faces, exactly as both builders lay it out
//!   ([`crate::wall_plan`]), moved by its placement; the old box wall's (`wall.straight`) by its
//!   box. A wall that does not build (its type missing, its recipe not as written) is not there to
//!   bound a room and is passed over, as it is in every view; so is a record of another operation.
//!   A door or a window in a wall does not cut its outline here: an opening does not break a loop.
//! - What becomes of the room is construct-rooms' `assess`, with the other rooms of its level:
//!   enclosed, it is filled for the builders (construct-rooms' `filled`); not enclosed, its build
//!   fails as `NotEnclosed`; in the same loop as a room whose ID comes first, as `Redundant`,
//!   naming it; any refusal under its own code (`PointInWall`, `OffGrid`, `AreaNotExact`, ...).
//!   The record is never changed or dropped; its built row says what became of it.
//! - The walls a room's loop runs along are [`walls_read`] of its filled recipe: the regenerator
//!   lists them in its `reads` row. A wall that changes, is made or is deleted builds again every
//!   room on the levels it was and is on, and every room whose `reads` row lists it
//!   ([`is_spatial`] says which kinds do this), one step deep; a room that changes builds again the
//!   other rooms of its levels, since which of two rooms in one loop comes first may change.
//!
//! Depends on construct-rooms (the search and the record), construct-levels (ties),
//! construct-model, construct-params and kit-store, and on [`crate::parameters`] and
//! [`crate::levels`] for a wall's filled recipe. Interim: none here; a curved wall, when there is
//! one, is to be read as construct-rooms' `Outline::Curved`, which refuses the room by name.

use crate::levels;
use crate::parameters::{self, At};
use crate::wall_plan::{self, Layered};
use construct_levels::base_of;
use construct_model::element::Element;
use construct_model::geometry::{GeometryError, Recipe};
use construct_model::keys::list;
use construct_params::{Length, Value};
use construct_rooms::{
    Bound, HEIGHT_ARG, LOOPS_ARG, Neighbour, Outcome, Outline, ROOM_KIND, RoomError, assess, at_of,
    reads_walls,
};
use kit_core::Id;
use kit_store::{Reader, StoreError, from_row, read_row};
use serde_json::Value as Json;
use std::collections::BTreeSet;

/// The element kind of a wall (construct-api's walls), whose outline bounds rooms.
pub const WALL_KIND: &str = "wall";

fn failure(code: &str, message: String) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message,
    }
}

fn refused(error: &RoomError) -> GeometryError {
    failure(error.code(), error.to_string())
}

/// Whether a change to an element of this kind builds the rooms round it again: a wall's (its
/// outline bounds them) or a room's (which of two rooms in one loop comes first).
#[must_use]
pub fn is_spatial(kind: &str) -> bool {
    kind == WALL_KIND || kind == ROOM_KIND
}

/// The walls a room's filled recipe runs along, by ID, ascending: its `reads` row lists them.
/// Nothing for a recipe that is not a filled room's.
#[must_use]
pub fn walls_read(filled: &Recipe) -> BTreeSet<String> {
    let mut walls = BTreeSet::new();
    if !construct_rooms::is_room_recipe(filled) {
        return walls;
    }
    let loops = filled.args.get(LOOPS_ARG).and_then(Json::as_array);
    for corner in loops
        .into_iter()
        .flatten()
        .filter_map(Json::as_array)
        .flatten()
    {
        if let Some(wall) = corner.get("wall").and_then(Json::as_str) {
            walls.insert(wall.to_owned());
        }
    }
    walls
}

/// The records of a document of one kind, as the kind index lists them: what regeneration has
/// reached, in log order.
async fn listed<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    kind: &str,
) -> Result<Vec<Element>, StoreError> {
    let Ok(kind) = Id::parse(kind) else {
        return Ok(Vec::new());
    };
    let mut found = Vec::new();
    for (_, row) in reader.scan(&list::kind(project, document, &kind)).await? {
        let hash: String = from_row(&row)?;
        let hash = Id::parse(&hash).map_err(|e| StoreError::Damaged(e.to_string()))?;
        let record = kit_store::Place::Record {
            project: project.clone(),
            hash,
        };
        let element: Option<Element> = read_row(reader, &record.key()).await?;
        found.push(element.ok_or_else(|| StoreError::Damaged("record missing".into()))?);
    }
    Ok(found)
}

/// The records of a document of one kind, `at` that state.
async fn of_kind<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    kind: &str,
    at: At,
) -> Result<Vec<Element>, StoreError> {
    if at == At::Regenerated {
        return listed(reader, project, document, kind).await;
    }
    Ok(parameters::read_document(reader, project, document)
        .await?
        .into_values()
        .map(|(_, record)| record)
        .filter(|record| record.kind == kind)
        .collect())
}

/// The level an element's base is tied to, if it can be read.
fn level_of(element: &Element) -> Option<Id> {
    base_of(element).ok().flatten().map(|tie| tie.level)
}

/// A whole number of millimetres in grid steps, if it fits.
fn mm(value: i64) -> Option<i64> {
    Length::checked_from_mm(value).map(Length::steps)
}

/// The old box wall's outline: the box from its placement, `length` along x (or y when its
/// `axis` is `y`) and `thickness` across, in whole millimetres, as the stand-in builds it.
fn straight_outline(wall: &Element) -> Option<Vec<[i64; 2]>> {
    let args = &wall.recipe.args;
    let whole = |key: &str| args.get(key).and_then(Json::as_i64).filter(|v| *v > 0);
    let (long, across) = (mm(whole("length")?)?, mm(whole("thickness")?)?);
    let (w, d) = if args.get("axis").and_then(Json::as_str) == Some("y") {
        (across, long)
    } else {
        (long, across)
    };
    let (x, y) = (mm(wall.placement.x)?, mm(wall.placement.y)?);
    let (x1, y1) = (x.checked_add(w)?, y.checked_add(d)?);
    Some(vec![[x, y], [x1, y], [x1, y1], [x, y1]])
}

/// A wall's outline in plan, as the builders lay it out, in the model's grid steps; `None` for a
/// wall that does not build, or a record of another operation.
async fn outline_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    wall: &Element,
    at: At,
) -> Result<Option<Vec<[i64; 2]>>, StoreError> {
    if wall.recipe.op == "wall.straight" {
        return Ok(straight_outline(wall));
    }
    if !wall_plan::is_layered(&wall.recipe) {
        return Ok(None);
    }
    let Ok(recipe) = parameters::recipe_at(reader, project, document, wall, at).await? else {
        return Ok(None);
    };
    let Ok(placed) = levels::placed(reader, project, document, wall, at).await? else {
        return Ok(None);
    };
    let Ok(recipe) = levels::filled(&recipe, &placed) else {
        return Ok(None);
    };
    let Ok(layered) = Layered::of(&recipe) else {
        return Ok(None);
    };
    let (exterior, interior) = layered.outer();
    let Ok((corners, _)) = layered.plan.strip(exterior, interior) else {
        return Ok(None);
    };
    let (Some(dx), Some(dy)) = (mm(wall.placement.x), mm(wall.placement.y)) else {
        return Ok(None);
    };
    Ok(corners
        .iter()
        .map(|&[x, y]| Some([x.checked_add(dx)?, y.checked_add(dy)?]))
        .collect())
}

/// What becomes of the room `room`, its recipe `recipe` with its height already read from its
/// levels, standing on `level`: its loop among the walls of that level and the other rooms on
/// it, as the module header says. The inner error is a refusal, in words, under its code.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn outcome<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    room: &Element,
    recipe: &Recipe,
    level: Option<&Id>,
    at: At,
) -> Result<Result<Outcome, GeometryError>, StoreError> {
    let Some(level) = level else {
        return Ok(Err(failure(
            "NoLevel",
            format!("room {}: a room stands on a level", room.name),
        )));
    };
    let point = match at_of(recipe) {
        Ok(point) => point,
        Err(e) => return Ok(Err(refused(&e))),
    };
    let height = match recipe.args.get(HEIGHT_ARG) {
        Some(Json::String(text)) => match Value::from_stored(text) {
            Some(Value::Length(length)) => length,
            _ => {
                return Ok(Err(refused(&RoomError::Malformed(format!(
                    "its height is {text}"
                )))));
            }
        },
        Some(other) if other.as_object().is_some_and(|o| o.contains_key("slope")) => {
            return Ok(Err(refused(&RoomError::SlopedLimit)));
        }
        _ => {
            return Ok(Err(refused(&RoomError::Malformed(
                "its height was not read from its levels".to_owned(),
            ))));
        }
    };
    let mut bounds = Vec::new();
    for wall in of_kind(reader, project, document, WALL_KIND, at).await? {
        if level_of(&wall).as_ref() != Some(level) {
            continue;
        }
        if let Some(corners) = outline_of(reader, project, document, &wall, at).await? {
            bounds.push(Bound {
                wall: wall.id.clone(),
                outline: Outline::Polygon(corners),
            });
        }
    }
    let mut others = Vec::new();
    for other in of_kind(reader, project, document, ROOM_KIND, at).await? {
        if other.id == room.id || level_of(&other).as_ref() != Some(level) {
            continue;
        }
        if let Ok(at) = at_of(&other.recipe) {
            others.push(Neighbour {
                id: other.id.clone(),
                at,
            });
        }
    }
    Ok(assess(&room.id, point, height, &bounds, &others).map_err(|e| refused(&e)))
}

/// What becomes of the room `room` as the document stands `at` that state, from its record alone:
/// its height read from its levels as the regenerator reads it, then [`outcome`]. What a page or a
/// command reads a room's area, perimeter and volume by.
///
/// # Errors
/// A [`StoreError`] when the store fails. The inner error is a level that cannot be read, or a
/// refusal, in words.
pub async fn found_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    room: &Element,
    at: At,
) -> Result<Result<Outcome, GeometryError>, StoreError> {
    let placed = match levels::placed(reader, project, document, room, at).await? {
        Ok(placed) => placed,
        Err(e) => return Ok(Err(e)),
    };
    let recipe = match levels::filled(&room.recipe, &placed) {
        Ok(recipe) => recipe,
        Err(e) => return Ok(Err(e)),
    };
    outcome(
        reader,
        project,
        document,
        room,
        &recipe,
        placed.level.as_ref(),
        at,
    )
    .await
}

/// The recipe a room is built from: `{"room":"walls"}` filled with its loop ([`outcome`], at the
/// state regeneration has reached). A recipe that reads no walls is returned as it is.
///
/// # Errors
/// A [`StoreError`] when the store fails. The inner error is the room's build failure:
/// `NotEnclosed`, `Redundant` or a refusal, in words.
pub async fn filled<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    room: &Element,
    recipe: Recipe,
    level: Option<&Id>,
) -> Result<Result<Recipe, GeometryError>, StoreError> {
    if !reads_walls(&recipe) {
        return Ok(Ok(recipe));
    }
    let outcome = outcome(
        reader,
        project,
        document,
        room,
        &recipe,
        level,
        At::Regenerated,
    )
    .await?;
    let who = match construct_rooms::number_of(room) {
        Some(number) if !number.is_empty() => format!("room {number} {}", room.name),
        _ => format!("room {}", room.name),
    };
    Ok(match outcome {
        Ok(Outcome::Enclosed { found, .. }) => {
            construct_rooms::filled(&recipe, &found).map_err(|e| refused(&e))
        }
        Ok(Outcome::NotEnclosed) => Err(failure(
            "NotEnclosed",
            format!("{who} is not enclosed: the walls round its point do not close"),
        )),
        Ok(Outcome::Redundant { first }) => Err(failure(
            "Redundant",
            format!("{who} is redundant: room {first} is already in the same loop"),
        )),
        Err(e) => Err(e),
    })
}
