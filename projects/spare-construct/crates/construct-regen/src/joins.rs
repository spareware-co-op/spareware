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
//! - Each wall is read with its own choices (construct-walls' [`Joining`], the stored recipe
//!   argument [`JOINING`]: joins allowed or not at each end, and the choice of each joint it
//!   keeps) and its wrapping at free ends ([`wall_plan::WRAP`]), so the rule makes the joints
//!   and wraps the ends a person chose (job 6.1).
//! - [`fill`] also gives the IDs of every wall it read (the walls it meets, and the walls they
//!   meet): the regenerator lists them in the wall's `reads` row, so a change to any of them
//!   builds it again.
//! - [`on_level`] fills every wall of a level at once, each recipe the one [`fill`] gives it: a
//!   room's walls are read so (construct-regen's `rooms`), each by its outline as its joints
//!   shape it, so a room's faces are the walls' faces as they are built.
//!
//! The search reads the level's walls in full: the regenerator's area index is made from built
//! shapes, which joints change, so a search through it would depend on what was built first. A
//! spatial filter over the walls' lines is the improvement for large documents.
//!
//! Depends on construct-model (records), construct-params (lengths), construct-levels (an
//! element's base level), construct-walls (the join rule) and this crate's readers. Nothing here
//! is interim.

use crate::levels;
use crate::parameters::{self, At, read_document};
use crate::wall_plan::{self, JOINS, Layered};
use construct_model::element::{Element, Placement};
use construct_model::geometry::{GeometryError, Recipe};
use construct_model::keys::{Place, list};
use construct_params::{Length, Value};
use construct_walls::joins::{join, meeting};
use construct_walls::{
    AtEnds, Function, JOINING, Joined, Joining, Meeting, PlanLayer, PlanWall, lines_meet,
};
use kit_core::Id;
use kit_store::{Reader, StoreError, from_row, read_row};
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, BTreeSet};

/// The kind layered walls are listed under.
const WALL_KIND: &str = "wall";

/// A wall's line in the model's plan, in steps.
type Line = [[Length; 2]; 2];

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

/// The line of a layered wall's record in the model's plan, read without filling anything.
fn line_in_model(element: &Element) -> Option<Line> {
    let (start, end) = wall_plan::line_of(&element.recipe)?;
    let at = placed_at(&element.placement)?;
    Some([in_model(start, at)?, in_model(end, at)?])
}

/// The level an element's base stands on, as its record says.
fn base_level(element: &Element) -> Option<Id> {
    construct_levels::base_of(element)
        .ok()
        .flatten()
        .map(|tie| tie.level)
}

/// The layered walls of a document, as `at` that state, by ID.
async fn walls_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    at: At,
) -> Result<BTreeMap<Id, Element>, StoreError> {
    let mut found = BTreeMap::new();
    if at == At::Now {
        for (id, (_, record)) in read_document(reader, project, document).await? {
            if record.kind == WALL_KIND && wall_plan::is_layered(&record.recipe) {
                found.insert(id, record);
            }
        }
        return Ok(found);
    }
    let Ok(kind) = Id::parse(WALL_KIND) else {
        return Ok(found);
    };
    for (key, row) in reader.scan(&list::kind(project, document, &kind)).await? {
        let Some(Place::Kind { element, .. }) = Place::parse(&key) else {
            continue;
        };
        let hash: String = from_row(&row)?;
        let hash = Id::parse(&hash).map_err(|e| StoreError::Damaged(e.to_string()))?;
        let record = kit_store::Place::Record {
            project: project.clone(),
            hash,
        };
        let record: Option<Element> = read_row(reader, &record.key()).await?;
        let record = record.ok_or_else(|| StoreError::Damaged("record missing".into()))?;
        if wall_plan::is_layered(&record.recipe) {
            found.insert(element, record);
        }
    }
    Ok(found)
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
    for (k, strip) in wall.layers.iter().enumerate() {
        let function: Function =
            serde_json::from_value(Json::String(strip.function.clone())).ok()?;
        layers.push(PlanLayer {
            function,
            exterior: Length::from_steps(strip.exterior),
            interior: Length::from_steps(strip.interior),
            wraps: wall
                .wrap
                .as_ref()
                .and_then(|w| w.layers.get(k).copied())
                .unwrap_or(false),
        });
    }
    let base = elevation
        .unwrap_or(Length::from_mm(0))
        .checked_add(Length::checked_from_mm(placement.z)?)?;
    // A wall that wraps nothing has no core to read: every layer stands in for it, and nothing
    // turns.
    let (core, wrap_ends) = match &wall.wrap {
        Some(wrap) => ([wrap.core.0, wrap.core.1], wrap.ends),
        None => ([0, layers.len()], AtEnds::None),
    };
    let mut plan = PlanWall::new(
        id,
        (
            in_model(wall.plan.start(), at)?,
            in_model(wall.plan.end(), at)?,
        ),
        (base, base.checked_add(Length::from_steps(wall.height))?),
        layers,
        core,
    );
    plan.wrap_ends = wrap_ends;
    // A wall whose own choices do not read is joined as if it had none.
    plan.joining = Joining::from_json(recipe.args.get(JOINING)).unwrap_or_default();
    Some(plan)
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

/// How the layered walls `a` and `b` of a document meet, as it is `at` that state: construct-walls'
/// [`meeting`] of their lines in the model's plan, each filled as it is built; `None` when either
/// does not read as a layered wall, they stand on different levels, or they do not meet. What a
/// command checks a joint's choice against.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn meeting_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    (a, b): (&Element, &Element),
    at: At,
) -> Result<Option<Meeting>, StoreError> {
    if base_level(a) != base_level(b) {
        return Ok(None);
    }
    let (Some(one), Some(two)) = (
        neighbour(reader, project, document, a, at).await?,
        neighbour(reader, project, document, b, at).await?,
    ) else {
        return Ok(None);
    };
    Ok(meeting(&[one, two], 0, 1))
}

/// The IDs of the walls of `walls` within two meetings of the wall on `line`: the walls it meets,
/// and the walls they meet.
fn within_two(line: Line, me: &Id, walls: &BTreeMap<Id, (Element, Line)>) -> BTreeSet<Id> {
    let meets = |a: Line| {
        walls
            .iter()
            .filter(move |(id, (_, b))| *id != me && lines_meet(a, *b))
            .map(|(id, _)| id.clone())
    };
    let first: BTreeSet<Id> = meets(line).collect();
    let mut all = first.clone();
    for id in &first {
        if let Some((_, b)) = walls.get(id) {
            all.extend(meets(*b));
        }
    }
    all
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
/// or does not read as one, is returned as it is, without `joins`.
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
    let mut recipe = without_joins(recipe);
    let (Some(me), Some(origin)) = (
        plan_wall(element, &recipe, elevation),
        placed_at(&element.placement),
    ) else {
        return Ok((recipe, BTreeSet::new()));
    };
    let Some(line) = line_in_model(element) else {
        return Ok((recipe, BTreeSet::new()));
    };
    let level = base_level(element);
    let mut lines = BTreeMap::new();
    for (id, record) in walls_of(reader, project, document, at).await? {
        if id != element.id
            && base_level(&record) == level
            && let Some(other) = line_in_model(&record)
        {
            lines.insert(id, (record, other));
        }
    }
    let near = within_two(line, &element.id, &lines);
    let mut walls: BTreeMap<String, PlanWall> = BTreeMap::new();
    for id in &near {
        if let Some((record, _)) = lines.get(id)
            && let Some(wall) = neighbour(reader, project, document, record, at).await?
        {
            walls.insert(id.to_string(), wall);
        }
    }
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

/// Each of `walls`, the walls of one level, with its `joins` filled among all the others: the
/// recipe [`fill`] gives each from a store. [`fill`] hands the join rule the walls within two
/// meetings of the wall; this hands it every wall of the level, and more do no harm
/// (construct-walls' [`join`]); both in the order of their IDs. What a reader of a whole level
/// takes (a room reads every wall of its level), so it reads each wall once and not once for each
/// wall it meets. A recipe that does not read as a layered wall is returned as it is, without
/// `joins`.
#[must_use]
pub fn on_level(walls: &[OnLevel<'_>]) -> Vec<Recipe> {
    let mut plans: BTreeMap<String, PlanWall> = BTreeMap::new();
    for wall in walls {
        let recipe = without_joins(wall.recipe);
        if let Some(plan) =
            plan_wall_at(wall.id.to_owned(), wall.placement, &recipe, wall.elevation)
        {
            plans.insert(wall.id.to_owned(), plan);
        }
    }
    let ids: Vec<String> = plans.keys().cloned().collect();
    let plans: Vec<PlanWall> = plans.into_values().collect();
    walls
        .iter()
        .map(|wall| {
            let mut out = without_joins(wall.recipe);
            if let Some(me) = ids.iter().position(|known| known == wall.id)
                && let Some(origin) = placed_at(wall.placement)
                && let Some(arg) = joins_arg(&join(&plans, me), origin)
            {
                out.args.insert(JOINS.to_owned(), arg);
            }
            out
        })
        .collect()
}
