//! How a layered wall's recipe is filled with its joints with the walls it meets (job 5.1, the
//! first half of CON-A4-F2-P2; note CON-A4-F2-P2-wall-joins). The regenerator fills a wall's
//! recipe through [`fill`] before building it, and the server's `/filled` reads it through
//! [`recipe_at`], so what a page builds and what the server builds cannot differ.
//!
//! What it promises:
//! - The walls a wall meets are found from the stored walls of its document on its base level,
//!   as of the state read ([`At`]): each layered wall's line in the model's plan (its recipe's
//!   `start` and `end` plus its placement), tested exactly with construct-walls'
//!   [`lines_meet`]; the walls it meets and the walls they meet are filled as they would be
//!   built and handed, in the order of their IDs, to construct-walls' join rule ([`join`]). So
//!   the same model gives the same joints in the same order on every machine.
//! - The recipe's `joins` argument ([`wall_plan::JOINS`]) is then what the rule gave: each
//!   layer's pieces and the whole outline (each corner a stored length, in the element's frame),
//!   the walls it is joined with, and why each wall that meets it is not joined with it. A wall
//!   that meets none has no `joins`, and one in a stored record is dropped first: joints are
//!   derived, never stored.
//! - [`fill`] also gives the IDs of every wall it read (the walls it meets, and the walls they
//!   meet): the regenerator lists them in the wall's `reads` row, so a change to any of them
//!   builds it again.
//! - [`on_level`] fills every wall of a level at once, each recipe the one [`fill`] gives it: a
//!   room's walls are read so (construct-regen's `rooms`), each by its outline as its joints
//!   shape it, so a room's faces are the walls' faces as they are built. Each wall's joints are
//!   made among the walls within two meetings of it there too, found through a grid of their
//!   lines, so filling a level reads each wall's neighbours and not the whole level for each.
//!
//! The walls a wall meets are found through [`WallIndex`], the document's walls by where their
//! lines run, which tests exactly the walls a full read would and gives the same set: the
//! regenerator keeps one in step with the kind index ([`fill_among`]), so building a wall reads
//! its neighbours and not every wall of the document; [`fill`] and [`recipe_at`] read one first.
//! It is made from the walls' records, never from built shapes: the regenerator's area index is
//! made from built shapes, which joints change, so a search through it would depend on what was
//! built first.
//!
//! Depends on construct-model (records), construct-params (lengths), construct-levels (an
//! element's base level), construct-walls (the join rule) and this crate's readers. Nothing here
//! is interim.

use crate::levels;
use crate::parameters::{self, At};
use crate::read_memo::{Memo, Recording, Written};
use crate::wall_index::{Grid, WallIndex, line_in_model};
use crate::wall_plan::{self, JOINS, Layered};
use construct_model::element::{Element, Placement};
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::{Length, Value};
use construct_walls::joins::join;
use construct_walls::{Function, Joined, PlanLayer, PlanWall};
use kit_core::Id;
use kit_store::{Reader, StoreError};
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, BTreeSet};

/// A placement's plan corner in steps.
fn placed_at(placement: &Placement) -> Option<[i64; 2]> {
    Some([
        Length::checked_from_mm(placement.x)?.steps(),
        Length::checked_from_mm(placement.y)?.steps(),
    ])
}

/// `point` of an element's frame in the model's plan.
fn in_model(point: [i64; 2], at: [i64; 2]) -> Option<[Length; 2]> {
    let ([x, y], [ax, ay]) = (point, at);
    Some([
        Length::from_steps(x.checked_add(ax)?),
        Length::from_steps(y.checked_add(ay)?),
    ])
}

/// The recipe without a `joins` argument: a stored one is never read.
fn without_joins(recipe: &Recipe) -> Recipe {
    let mut out = recipe.clone();
    out.args.remove(JOINS);
    out
}

/// A filled layered wall as the join rule reads it, in the model's plan: `None` for one that does
/// not read as a wall.
fn plan_wall(element: &Element, recipe: &Recipe, elevation: Option<Length>) -> Option<PlanWall> {
    plan_wall_at(
        element.id.to_string(),
        &element.placement,
        recipe,
        elevation,
    )
}

/// [`plan_wall`] of a wall known by its ID and placement.
fn plan_wall_at(
    id: String,
    placement: &Placement,
    recipe: &Recipe,
    elevation: Option<Length>,
) -> Option<PlanWall> {
    let wall = Layered::of(recipe).ok()?;
    let at = placed_at(placement)?;
    let mut layers = Vec::with_capacity(wall.layers.len());
    for strip in &wall.layers {
        let function: Function =
            serde_json::from_value(Json::String(strip.function.clone())).ok()?;
        layers.push(PlanLayer {
            function,
            exterior: Length::from_steps(strip.exterior),
            interior: Length::from_steps(strip.interior),
        });
    }
    let base = elevation
        .unwrap_or(Length::from_mm(0))
        .checked_add(Length::checked_from_mm(placement.z)?)?;
    Some(PlanWall {
        id,
        start: in_model(wall.plan.start(), at)?,
        end: in_model(wall.plan.end(), at)?,
        base,
        top: base.checked_add(Length::from_steps(wall.height))?,
        layers,
    })
}

/// A neighbour's recipe filled as it is built, without its joints, and where its base stands.
async fn neighbour<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    at: At,
) -> Result<Option<PlanWall>, StoreError> {
    let Ok(recipe) = parameters::recipe_at(reader, project, document, element, at).await? else {
        return Ok(None);
    };
    let Ok(placed) = levels::placed(reader, project, document, element, at).await? else {
        return Ok(None);
    };
    let Ok(recipe) = levels::filled(&without_joins(&recipe), &placed) else {
        return Ok(None);
    };
    Ok(plan_wall(element, &recipe, placed.elevation))
}

/// The neighbours a pass has filled ([`neighbour`]), each kept with the rows it read
/// ([`crate::read_memo`]): a wall near many that are built in one pass is filled once.
pub type Fills = Memo<Option<PlanWall>>;

/// [`neighbour`], or what `fills` kept of it while every row it read reads the same.
async fn neighbour_kept<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    at: At,
    fills: &mut Fills,
    written: Option<&Written>,
) -> Result<Option<PlanWall>, StoreError> {
    if let Some(kept) = fills.kept(reader, element, written).await? {
        return Ok(kept);
    }
    let mut recording = Recording::new(reader);
    let made = neighbour(&mut recording, project, document, element, at).await?;
    let noted = recording.noted();
    fills.keep(element, noted, made.clone(), written);
    Ok(made)
}

/// A length in its stored spelling.
fn stored(length: Length) -> Json {
    Json::String(Value::Length(length).stored())
}

/// A polygon of the model's plan in the element's frame, each corner as two stored lengths.
fn framed(polygon: &[[Length; 2]], at: [i64; 2]) -> Option<Json> {
    let [ax, ay] = at;
    let mut out = Vec::with_capacity(polygon.len());
    for [x, y] in polygon {
        out.push(json!([
            stored(Length::from_steps(x.steps().checked_sub(ax)?)),
            stored(Length::from_steps(y.steps().checked_sub(ay)?))
        ]));
    }
    Some(Json::Array(out))
}

/// The `joins` argument for what the rule gave, in the element's frame; `None` when nothing
/// meets the wall.
fn joins_arg(joined: &Joined, at: [i64; 2]) -> Option<Json> {
    let mut out = serde_json::Map::new();
    if !joined.layers.is_empty() {
        let mut layers = Vec::with_capacity(joined.layers.len());
        for pieces in &joined.layers {
            let pieces = pieces
                .iter()
                .map(|p| framed(p, at))
                .collect::<Option<Vec<_>>>()?;
            layers.push(Json::Array(pieces));
        }
        out.insert("layers".to_owned(), Json::Array(layers));
        out.insert("outline".to_owned(), framed(&joined.outline, at)?);
        out.insert("with".to_owned(), json!(joined.with));
    }
    if !joined.unjoined.is_empty() {
        out.insert("unjoined".to_owned(), json!(joined.unjoined));
    }
    (!out.is_empty()).then_some(Json::Object(out))
}

/// `recipe`, the filled recipe of `element` (its layers and its height filled, `elevation` its
/// base's height), with its `joins` argument filled from the walls it meets as the document is
/// `at` that state; and the IDs of every wall read for it. A recipe that is not a layered wall's,
/// or does not read as one, is returned as it is, without `joins`. It reads the document's walls
/// first ([`WallIndex::read`]); a reader that fills many walls keeps one and calls
/// [`fill_among`].
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn fill<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    recipe: &Recipe,
    elevation: Option<Length>,
    at: At,
) -> Result<(Recipe, BTreeSet<String>), StoreError> {
    if plan_wall(element, &without_joins(recipe), elevation).is_none()
        || line_in_model(element).is_none()
    {
        return Ok((without_joins(recipe), BTreeSet::new()));
    }
    let walls = WallIndex::read(reader, project, document, at).await?;
    let mut fills = Fills::default();
    fill_among(
        reader, project, document, element, recipe, elevation, at, &walls, &mut fills, None,
    )
    .await
}

/// [`fill`] with the document's walls as `at` that state already read, in `walls`: the walls it
/// meets are found through it, and nothing else of the document is read but each of those walls'
/// type, parameters and levels, each neighbour filled once while what it read stands (`fills`;
/// `written`, the rows a pass of the regenerator wrote, when this is one).
///
/// # Errors
/// A [`StoreError`] when the store fails.
#[allow(
    clippy::too_many_arguments,
    reason = "fill's arguments and the walls it reads them among"
)]
pub async fn fill_among<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    recipe: &Recipe,
    elevation: Option<Length>,
    at: At,
    walls: &WallIndex,
    fills: &mut Fills,
    written: Option<&Written>,
) -> Result<(Recipe, BTreeSet<String>), StoreError> {
    let mut recipe = without_joins(recipe);
    let (Some(me), Some(origin)) = (
        plan_wall(element, &recipe, elevation),
        placed_at(&element.placement),
    ) else {
        return Ok((recipe, BTreeSet::new()));
    };
    if line_in_model(element).is_none() {
        return Ok((recipe, BTreeSet::new()));
    }
    let near = walls.within_two(element);
    let mut met: BTreeMap<String, PlanWall> = BTreeMap::new();
    for id in &near {
        if let Some(record) = walls.get(id)
            && let Some(wall) =
                neighbour_kept(reader, project, document, record, at, fills, written).await?
        {
            met.insert(id.to_string(), wall);
        }
    }
    let mut walls = met;
    let read: BTreeSet<String> = near.iter().map(ToString::to_string).collect();
    walls.insert(me.id.clone(), me);
    let at_me = walls.keys().position(|id| *id == element.id.to_string());
    let walls: Vec<PlanWall> = walls.into_values().collect();
    if let Some(me) = at_me
        && let Some(arg) = joins_arg(&join(&walls, me), origin)
    {
        recipe.args.insert(JOINS.to_owned(), arg);
    }
    Ok((recipe, read))
}

/// The recipe `element` is built from, as the document is `at` that state, every argument that
/// reads a record filled ([`levels::recipe_at`]) and its joints with the walls it meets
/// ([`fill`]): what a page builds a layered wall from.
///
/// # Errors
/// A [`StoreError`] when the store fails. The inner error is the build failure, as
/// [`levels::recipe_at`] gives it.
pub async fn recipe_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    at: At,
) -> Result<Result<Recipe, GeometryError>, StoreError> {
    let recipe = match levels::recipe_at(reader, project, document, element, at).await? {
        Ok(recipe) => recipe,
        Err(e) => return Ok(Err(e)),
    };
    if !wall_plan::is_layered(&recipe) {
        return Ok(Ok(recipe));
    }
    let elevation = match levels::placed(reader, project, document, element, at).await? {
        Ok(placed) => placed.elevation,
        Err(e) => return Ok(Err(e)),
    };
    let (recipe, _) = fill(reader, project, document, element, &recipe, elevation, at).await?;
    Ok(Ok(recipe))
}

/// [`recipe_at`] with the document's walls as `at` that state already read, in `walls`, and the
/// neighbours filled among them kept in `fills`: what a reader that fills many walls of one state
/// calls.
///
/// # Errors
/// As [`recipe_at`].
pub async fn recipe_among<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    at: At,
    walls: &WallIndex,
    fills: &mut Fills,
) -> Result<Result<Recipe, GeometryError>, StoreError> {
    let recipe = match levels::recipe_at(reader, project, document, element, at).await? {
        Ok(recipe) => recipe,
        Err(e) => return Ok(Err(e)),
    };
    if !wall_plan::is_layered(&recipe) {
        return Ok(Ok(recipe));
    }
    let elevation = match levels::placed(reader, project, document, element, at).await? {
        Ok(placed) => placed.elevation,
        Err(e) => return Ok(Err(e)),
    };
    let (recipe, _) = fill_among(
        reader, project, document, element, &recipe, elevation, at, walls, fills, None,
    )
    .await?;
    Ok(Ok(recipe))
}

/// Each of `walls`, an ID and a filled layered wall's recipe placed at the model's origin with its
/// base at zero, all on one level, with its `joins` filled among the others as [`fill`] fills it
/// from a store: for tests and tools that hold their walls in hand. A recipe that does not read
/// as a layered wall is returned as it is.
#[must_use]
pub fn among(walls: &[(&str, &Recipe)]) -> Vec<Recipe> {
    let origin = Placement { x: 0, y: 0, z: 0 };
    let level: Vec<OnLevel<'_>> = walls
        .iter()
        .map(|&(id, recipe)| OnLevel {
            id,
            placement: &origin,
            recipe,
            elevation: None,
        })
        .collect();
    on_level(&level)
}

/// A wall of one level as [`on_level`] takes it.
#[derive(Clone, Copy, Debug)]
pub struct OnLevel<'a> {
    /// Its element's ID.
    pub id: &'a str,
    /// Where its frame sits in the model.
    pub placement: &'a Placement,
    /// Its filled recipe (its layers and its height filled), as the regenerator hands [`fill`] it.
    pub recipe: &'a Recipe,
    /// The height of its base, from its level.
    pub elevation: Option<Length>,
}

/// Each of `walls`, the walls of one level, with its `joins` filled among the others: the recipe
/// [`fill`] gives each from a store. Like [`fill`], it hands the join rule the walls within two
/// meetings of each wall (construct-walls' [`join`] reads no other), in the order of their IDs,
/// found through a [`Grid`] of their lines, so a level of many walls costs each wall its
/// neighbours and not the whole level. What a reader of a whole level takes (a room reads every
/// wall of its level), so it reads each wall once and not once for each wall it meets. A recipe
/// that does not read as a layered wall is returned as it is, without `joins`.
#[must_use]
pub fn on_level(walls: &[OnLevel<'_>]) -> Vec<Recipe> {
    let mut plans: BTreeMap<String, PlanWall> = BTreeMap::new();
    let mut grid: Grid<String> = Grid::default();
    for wall in walls {
        let recipe = without_joins(wall.recipe);
        if let Some(plan) =
            plan_wall_at(wall.id.to_owned(), wall.placement, &recipe, wall.elevation)
        {
            grid.insert(wall.id.to_owned(), None, Some([plan.start, plan.end]));
            plans.insert(wall.id.to_owned(), plan);
        }
    }
    walls
        .iter()
        .map(|wall| {
            let mut out = without_joins(wall.recipe);
            let Some(plan) = plans.get(wall.id) else {
                return out;
            };
            let me = wall.id.to_owned();
            let mut near = grid.within_two(None, [plan.start, plan.end], &me);
            near.insert(me);
            let given: Vec<PlanWall> = near
                .iter()
                .filter_map(|id| plans.get(id).cloned())
                .collect();
            if let Some(at) = near.iter().position(|id| id == wall.id)
                && let Some(origin) = placed_at(wall.placement)
                && let Some(arg) = joins_arg(&join(&given, at), origin)
            {
                out.args.insert(JOINS.to_owned(), arg);
            }
            out
        })
        .collect()
}
