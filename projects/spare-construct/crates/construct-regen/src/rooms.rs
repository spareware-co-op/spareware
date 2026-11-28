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
//!   the kind index, in log order). A wall is read by its outline in plan ([`bounds_on`]): a
//!   layered wall's (`wall.layered`) as its joints with the walls it meets shape it, the whole
//!   wall's outline of the recipe it is built from ([`crate::joins`], construct-walls' join rule),
//!   so where two walls join, the room's faces meet where the joint puts them (one point at a
//!   corner that points into the room, issue 1329); a layered wall no joint changes by the four
//!   corners of its outer faces, exactly as both builders lay it out ([`crate::wall_plan`]); each
//!   moved by its placement; the old box wall's (`wall.straight`) by its box. A wall that does not
//!   build (its type missing, its recipe not as written) is not there to bound a room and is
//!   passed over, as it is in every view; so is a record of another operation. A door or a window
//!   in a wall does not cut its outline here: an opening does not break a loop.
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
//! - A room is bounded above by the ceiling over it ([`ceiling_height`], job A101): the lowest
//!   ceiling of the room's level whose outline holds the room's point (not on an edge, not in
//!   an opening), whose exposed face is above the room's base, caps the room's height where it
//!   is lower than the room's own upper limit, so its volume is its area times the height to the
//!   ceiling. A ceiling made, changed or deleted builds the rooms of its level again
//!   ([`is_spatial`]). A ceiling above the room's own limit changes nothing.
//!
//! Depends on construct-rooms (the search and the record), construct-levels (ties),
//! construct-model, construct-params and kit-store, and on [`crate::parameters`] and
//! [`crate::levels`] for a wall's filled recipe and [`crate::joins`] for its joints. Interim: none
//! here; a curved wall, when there is one, is to be read as construct-rooms' `Outline::Curved`,
//! which refuses the room by name.

use crate::joins::{self, OnLevel};
use crate::levels;
use crate::parameters::ParameterView;
use crate::parameters::{self, At};
use crate::wall_plan::{self, Layered};
use construct_levels::base_of;
use construct_model::element::{Element, Placement};
use construct_model::geometry::{GeometryError, Recipe};
use construct_model::keys::list;
use construct_params::{Definition, Holder, Kind, Length, Units, Value};
use construct_rooms::{
    Bound, HEIGHT_ARG, LOOPS_ARG, Measures, Neighbour, Outcome, Outline, ROOM_KIND, RoomError,
    assess, at_of, reads_walls,
};
use construct_slabs::{CEILING_KIND, ceiling_boundary, contains};
use kit_core::Id;
use kit_store::{Reader, StoreError, from_row, read_row};
use serde::Serialize;

pub use construct_rooms::is_room;
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
/// outline bounds them), a ceiling's (its height bounds them above, job A101) or a room's (which
/// of two rooms in one loop comes first).
#[must_use]
pub fn is_spatial(kind: &str) -> bool {
    kind == WALL_KIND || kind == ROOM_KIND || kind == CEILING_KIND
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
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn of_kind<R: Reader>(
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
#[must_use]
pub fn level_of(element: &Element) -> Option<Id> {
    base_of(element).ok().flatten().map(|tie| tie.level)
}

/// A whole number of millimetres in grid steps, if it fits.
fn mm(value: i64) -> Option<i64> {
    Length::checked_from_mm(value).map(Length::steps)
}

/// A wall in plan as the builders lay it out, in the model's grid steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WallPlan {
    /// The corners of its outline, in the order the builders give them.
    pub corners: Vec<[i64; 2]>,
    /// The direction its reference line runs, from its start to its end.
    pub direction: [i64; 2],
}

/// The old box wall's plan: the box from its placement, `length` along x (or y when its `axis`
/// is `y`) and `thickness` across, in whole millimetres, as the stand-in builds it.
fn straight_outline(wall: &Element) -> Option<WallPlan> {
    let args = &wall.recipe.args;
    let whole = |key: &str| args.get(key).and_then(Json::as_i64).filter(|v| *v > 0);
    let (long, across) = (mm(whole("length")?)?, mm(whole("thickness")?)?);
    let along_y = args.get("axis").and_then(Json::as_str) == Some("y");
    let (w, d) = if along_y {
        (across, long)
    } else {
        (long, across)
    };
    let (x, y) = (mm(wall.placement.x)?, mm(wall.placement.y)?);
    let (x1, y1) = (x.checked_add(w)?, y.checked_add(d)?);
    Some(WallPlan {
        corners: vec![[x, y], [x1, y], [x1, y1], [x, y1]],
        direction: if along_y { [0, 1] } else { [1, 0] },
    })
}

/// A layered wall's filled recipe, its layers and height read as the builders read them but its
/// joints not yet made, and the height of its base; `None` for a wall that does not build, or a
/// record of another operation.
async fn layered_recipe<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    wall: &Element,
    at: At,
) -> Result<Option<(Recipe, Option<Length>)>, StoreError> {
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
    Ok(Some((recipe, placed.elevation)))
}

/// `corners` of an element's frame moved by its placement into the model's plan.
fn moved(corners: &[[i64; 2]], placement: &Placement) -> Option<Vec<[i64; 2]>> {
    let (dx, dy) = (mm(placement.x)?, mm(placement.y)?);
    corners
        .iter()
        .map(|&[x, y]| Some([x.checked_add(dx)?, y.checked_add(dy)?]))
        .collect()
}

/// A layered wall's strip as drawn, between its two finish faces, in its frame: its outline
/// before any joint shapes its ends.
fn strip(layered: &Layered) -> Option<Vec<[i64; 2]>> {
    let (exterior, interior) = layered.outer();
    let (corners, _) = layered.plan.strip(exterior, interior).ok()?;
    Some(corners.to_vec())
}

/// The outline a room reads of a layered wall whose recipe is filled with its joints
/// ([`joins::on_level`]), in the model's plan: the whole wall's outline as its joints shape it
/// when any does, else its strip; `None` for a recipe the builders refuse. Whatever way the wall
/// runs: the outline is the join rule's polygon, read as it is.
fn joined_outline(recipe: &Recipe, placement: &Placement) -> Option<Vec<[i64; 2]>> {
    let layered = Layered::of(recipe).ok()?;
    let corners = match &layered.joined {
        Some(joints) => joints.outline.clone(),
        None => strip(&layered)?,
    };
    moved(&corners, placement)
}

/// The walls of `level` as the document stands `at` that state, in the order the kind lists them,
/// each by its outline in plan as the builders lay it out, in the model's grid steps: a layered
/// wall's outline as its joints with the walls it meets shape it ([`joins::on_level`], the
/// recipe the regenerator builds it from), so a room's faces are the walls' faces as joined and
/// meet where the joint puts them; a layered wall no joint changes by its strip; the old box
/// wall's (`wall.straight`) by its box. A wall that does not build is passed over.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn bounds_on<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    level: &Id,
    at: At,
) -> Result<Vec<Bound>, StoreError> {
    /// A wall of the level as it is read: the old box's outline, or a layered wall's filled
    /// recipe and the height of its base, for the join rule.
    enum Read {
        Boxed(Vec<[i64; 2]>),
        Layered(Recipe, Option<Length>),
    }
    let mut walls = Vec::new();
    for wall in of_kind(reader, project, document, WALL_KIND, at).await? {
        if level_of(&wall).as_ref() != Some(level) {
            continue;
        }
        let read = if wall.recipe.op == "wall.straight" {
            straight_outline(&wall).map(|plan| Read::Boxed(plan.corners))
        } else {
            layered_recipe(reader, project, document, &wall, at)
                .await?
                .map(|(recipe, elevation)| Read::Layered(recipe, elevation))
        };
        if let Some(read) = read {
            walls.push((wall, read));
        }
    }
    let level_walls: Vec<OnLevel<'_>> = walls
        .iter()
        .filter_map(|(wall, read)| match read {
            Read::Layered(recipe, elevation) => Some(OnLevel {
                id: wall.id.as_str(),
                placement: &wall.placement,
                recipe,
                elevation: *elevation,
            }),
            Read::Boxed(_) => None,
        })
        .collect();
    let mut joined = joins::on_level(&level_walls).into_iter();
    let mut bounds = Vec::with_capacity(walls.len());
    for (wall, read) in &walls {
        let corners = match read {
            Read::Boxed(corners) => Some(corners.clone()),
            Read::Layered(..) => joined
                .next()
                .and_then(|recipe| joined_outline(&recipe, &wall.placement)),
        };
        if let Some(corners) = corners {
            bounds.push(Bound {
                wall: wall.id.clone(),
                outline: Outline::Polygon(corners),
            });
        }
    }
    Ok(bounds)
}

/// A wall's plan as drawn: its strip between its finish faces before any joint shapes its ends,
/// and the direction of its reference line; `None` for a wall that does not build, or a record
/// of another operation. What the warnings read walls by (two walls joined at a corner do not
/// overlap along their length); a room reads each wall's outline as joined ([`bounds_on`]).
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn wall_plan_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    wall: &Element,
    at: At,
) -> Result<Option<WallPlan>, StoreError> {
    if wall.recipe.op == "wall.straight" {
        return Ok(straight_outline(wall));
    }
    let Some((recipe, _)) = layered_recipe(reader, project, document, wall, at).await? else {
        return Ok(None);
    };
    let Ok(layered) = Layered::of(&recipe) else {
        return Ok(None);
    };
    let corners = strip(&layered).and_then(|corners| moved(&corners, &wall.placement));
    Ok(corners.map(|corners| WallPlan {
        corners,
        direction: layered.plan.direction(),
    }))
}

/// The height from a room's base to the lowest ceiling over its point, if any: the ceilings of
/// `level` whose outline holds `point` and whose exposed face is above the room's base, as the
/// document stands `at` that state. A ceiling that does not read (its tie, its type) is passed
/// over, as a wall that does not build is.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn ceiling_height<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    room: &Element,
    point: [i64; 2],
    level: &Id,
    at: At,
) -> Result<Option<Length>, StoreError> {
    let Ok(own) = levels::placed(reader, project, document, room, at).await? else {
        return Ok(None);
    };
    let Some(base) = own.elevation else {
        return Ok(None);
    };
    let mut lowest: Option<i64> = None;
    for ceiling in of_kind(reader, project, document, CEILING_KIND, at).await? {
        if level_of(&ceiling).as_ref() != Some(level) {
            continue;
        }
        let Ok(boundary) = ceiling_boundary(&ceiling.recipe) else {
            continue;
        };
        if !contains(&boundary, point) {
            continue;
        }
        let Ok(placed) = levels::placed(reader, project, document, &ceiling, at).await? else {
            continue;
        };
        let Some(under) = placed.underside() else {
            continue;
        };
        let Some(height) = under.steps().checked_sub(base.steps()).filter(|h| *h > 0) else {
            continue;
        };
        lowest = Some(lowest.map_or(height, |most| most.min(height)));
    }
    Ok(lowest.map(Length::from_steps))
}

/// `recipe` with its height lowered to the ceiling over the room's point ([`ceiling_height`]),
/// when that is lower than the height it holds; as it is otherwise, and when its height is not
/// yet a stored length or its point is not read.
async fn under_ceiling<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    room: &Element,
    recipe: Recipe,
    level: &Id,
    at: At,
) -> Result<Recipe, StoreError> {
    let own = match recipe.args.get(HEIGHT_ARG) {
        Some(Json::String(text)) => match Value::from_stored(text) {
            Some(Value::Length(length)) => length,
            _ => return Ok(recipe),
        },
        _ => return Ok(recipe),
    };
    let Ok(point) = at_of(&recipe) else {
        return Ok(recipe);
    };
    let Some(lower) = ceiling_height(reader, project, document, room, point, level, at)
        .await?
        .filter(|ceiling| ceiling.steps() < own.steps())
    else {
        return Ok(recipe);
    };
    let mut out = recipe;
    out.args.insert(
        HEIGHT_ARG.to_owned(),
        Json::String(Value::Length(lower).stored()),
    );
    Ok(out)
}

/// The loop of walls of `level` round the grid point `point`, as the document stands `at` that
/// state, found by the very search a room uses ([`construct_rooms::find`]) among the walls'
/// outlines as joined ([`bounds_on`]): `None` when the walls do not close round it. What a
/// ceiling placed by clicking in a room is outlined by (job A101). The inner error is the
/// search's refusal, in words.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn loop_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    level: &Id,
    point: [i64; 2],
    at: At,
) -> Result<Result<Option<construct_rooms::Found>, String>, StoreError> {
    let bounds = bounds_on(reader, project, document, level, at).await?;
    Ok(construct_rooms::find(&bounds, point).map_err(|e| e.to_string()))
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
    let capped = under_ceiling(reader, project, document, room, recipe.clone(), level, at).await?;
    let recipe = &capped;
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
    let bounds = bounds_on(reader, project, document, level, at).await?;
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
    let (filled, _) = filled_at(
        reader,
        project,
        document,
        room,
        recipe,
        level,
        At::Regenerated,
    )
    .await?;
    Ok(filled)
}

/// A room's loop worked out at the state asked, with what it measures: the recipe the builders
/// take (or the build failure, in words), and the measures when it is enclosed.
pub type Worked = (Result<Recipe, GeometryError>, Option<Measures>);

/// [`filled`] as the document stands `at` the state asked, with the room's measures: what a page
/// is told a room is built from (`At::Now`), the same walls read the same way.
///
/// # Errors
/// As [`filled`].
pub async fn filled_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    room: &Element,
    recipe: Recipe,
    level: Option<&Id>,
    at: At,
) -> Result<Worked, StoreError> {
    if !reads_walls(&recipe) {
        return Ok((Ok(recipe), None));
    }
    let recipe = match level {
        Some(level) => under_ceiling(reader, project, document, room, recipe, level, at).await?,
        None => recipe,
    };
    let outcome = outcome(reader, project, document, room, &recipe, level, at).await?;
    let who = match construct_rooms::number_of(room) {
        Some(number) if !number.is_empty() => format!("room {number} {}", room.name),
        _ => format!("room {}", room.name),
    };
    Ok(match outcome {
        Ok(Outcome::Enclosed { found, measures }) => (
            construct_rooms::filled(&recipe, &found).map_err(|e| refused(&e)),
            Some(measures),
        ),
        Ok(Outcome::NotEnclosed) => (
            Err(failure(
                "NotEnclosed",
                format!("{who} is not enclosed: the walls round its point do not close"),
            )),
            None,
        ),
        Ok(Outcome::Redundant { first }) => (
            Err(failure(
                "Redundant",
                format!("{who} is redundant: room {first} is already in the same loop"),
            )),
            None,
        ),
        Err(e) => (Err(e), None),
    })
}

/// What a page is told of a room besides its record: its state, in a word and in words, and what
/// it measures, shown in the document's units. Worked out each time it is read, never stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RoomView {
    /// `enclosed`, `not enclosed`, `redundant`, or `refused` for a room the search refuses.
    pub state: &'static str,
    /// Why the room has no area, in words; empty when it is enclosed.
    pub words: String,
    /// Its floor area, as `12.5 m²`; none unless enclosed.
    pub area: Option<String>,
    /// Its perimeter; none unless enclosed.
    pub perimeter: Option<String>,
    /// Its volume; none unless enclosed.
    pub volume: Option<String>,
}

impl RoomView {
    /// The view of a room from what became of it ([`filled_at`]), its measures shown in `units`.
    #[must_use]
    pub fn of(worked: &Worked, units: &Units) -> Self {
        let (filled, measures) = worked;
        let shown = |definition: Option<Definition>, value: Value| {
            definition.and_then(|def| def.format(&value, units).ok())
        };
        let (state, words) = match filled {
            Ok(_) => ("enclosed", String::new()),
            Err(failed) if failed.code == "NotEnclosed" => ("not enclosed", failed.message.clone()),
            Err(failed) if failed.code == "Redundant" => ("redundant", failed.message.clone()),
            Err(failed) => ("refused", failed.message.clone()),
        };
        let measures = measures.filter(|_| filled.is_ok());
        Self {
            state,
            words,
            area: measures
                .and_then(|m| shown(definition(AREA, "Area", Kind::Area), Value::Area(m.area))),
            perimeter: measures.and_then(|m| {
                shown(
                    definition(PERIMETER, "Perimeter", Kind::Length),
                    Value::Length(m.perimeter),
                )
            }),
            volume: measures.and_then(|m| {
                shown(
                    definition(VOLUME, "Volume", Kind::Volume),
                    Value::Volume(m.volume),
                )
            }),
        }
    }

    /// The same as read-only parameters, as `/parameters/<room>` lists them after the ones its
    /// category has: its state (in words when it is not enclosed), then, when enclosed, its area,
    /// perimeter and volume. Each is marked `read-only`: nobody sets it.
    #[must_use]
    pub fn rows(&self) -> Vec<ParameterView> {
        let row =
            |id: &str, name: &str, kind: &str, shown: &str, reason: Option<String>| ParameterView {
                id: id.to_owned(),
                name: name.to_owned(),
                kind: kind.to_owned(),
                holder: "instance",
                value: Some(shown.to_owned()),
                shown: Some(shown.to_owned()),
                from: crate::slabs::READ_ONLY,
                formula: None,
                reason,
            };
        let mut rows = vec![row(
            STATE,
            "State",
            "text",
            self.state,
            (!self.words.is_empty()).then(|| self.words.clone()),
        )];
        for (id, name, kind, shown) in [
            (AREA, "Area", "area", &self.area),
            (PERIMETER, "Perimeter", "length", &self.perimeter),
            (VOLUME, "Volume", "volume", &self.volume),
        ] {
            if let Some(shown) = shown {
                rows.push(row(id, name, kind, shown, None));
            }
        }
        rows
    }
}

/// The read-only parameter of a room's state.
pub const STATE: &str = "room.state";
/// The read-only parameter of a room's floor area.
pub const AREA: &str = "room.area";
/// The read-only parameter of a room's perimeter.
pub const PERIMETER: &str = "room.perimeter";
/// The read-only parameter of a room's volume.
pub const VOLUME: &str = "room.volume";

fn definition(id: &str, name: &str, kind: Kind) -> Option<Definition> {
    let id = Id::parse(id).ok()?;
    Definition::new(id, name, kind, Holder::Instance).ok()
}

/// What becomes of a room as the document stands `at` that state, from its record alone: its
/// levels read, then [`filled_at`]. What the page is told a room is built from. The inner error
/// is a level that cannot be read, in words.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn worked_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    room: &Element,
    at: At,
) -> Result<Result<Worked, GeometryError>, StoreError> {
    let placed = match levels::placed(reader, project, document, room, at).await? {
        Ok(placed) => placed,
        Err(e) => return Ok(Err(e)),
    };
    let recipe = match levels::filled(&room.recipe, &placed) {
        Ok(recipe) => recipe,
        Err(e) => return Ok(Err(e)),
    };
    Ok(Ok(filled_at(
        reader,
        project,
        document,
        room,
        recipe,
        placed.level.as_ref(),
        at,
    )
    .await?))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "tests; a broken fixture must stop the test loudly"
)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    const MM: i64 = Length::STEPS_PER_MM;

    fn stored(steps: i64) -> Json {
        Json::String(Value::Length(Length::from_steps(steps)).stored())
    }

    fn point([x, y]: [i64; 2]) -> Json {
        json!([stored(x), stored(y)])
    }

    fn polygon(corners: &[[i64; 2]]) -> Json {
        Json::Array(corners.iter().copied().map(point).collect())
    }

    /// A one-layer wall 1000 steps thick from (0, 0) to (30000, 40000) steps, along neither axis
    /// (its direction is 3-4-5), with `joins` as given.
    fn slanted(joins: Option<Json>) -> Recipe {
        let mut args = BTreeMap::new();
        args.insert("start".to_owned(), point([0, 0]));
        args.insert("end".to_owned(), point([30_000, 40_000]));
        args.insert("height".to_owned(), stored(2700 * MM));
        args.insert(
            "layers".to_owned(),
            json!([{
                "function": "structure",
                "material": "concrete-regular",
                "exterior": stored(500),
                "interior": stored(-500),
            }]),
        );
        if let Some(joins) = joins {
            args.insert(wall_plan::JOINS.to_owned(), joins);
        }
        Recipe::construct(wall_plan::OP, args)
    }

    /// A room reads a joined wall by the joint's own outline, whatever way the wall runs: here a
    /// wall at 3-4-5 with its start mitred (its exterior corner 500 steps back along the line, its
    /// interior corner 500 on), moved by its placement. Without a joint it is read by its strip.
    #[test]
    fn a_joined_wall_at_an_angle_is_read_by_its_joined_outline() {
        // Its faces 500 steps either side: (4, -3) / 5 to the right of (3, 4).
        let strip = [[400, -300], [30_400, 39_700], [29_600, 40_300], [-400, 300]];
        let mitred = [[100, -700], [30_400, 39_700], [29_600, 40_300], [-100, 700]];
        let placement = Placement {
            x: 1000,
            y: -2000,
            z: 0,
        };
        let moved = |corners: &[[i64; 2]]| -> Vec<[i64; 2]> {
            corners
                .iter()
                .map(|&[x, y]| [x + 1000 * MM, y - 2000 * MM])
                .collect()
        };
        assert_eq!(
            joined_outline(&slanted(None), &placement),
            Some(moved(&strip))
        );
        let joins = json!({
            "layers": [[polygon(&mitred)]],
            "outline": polygon(&mitred),
            "with": ["W-2"],
        });
        assert_eq!(
            joined_outline(&slanted(Some(joins)), &placement),
            Some(moved(&mitred))
        );
    }
}
