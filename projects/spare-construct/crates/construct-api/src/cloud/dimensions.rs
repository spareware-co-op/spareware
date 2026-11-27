//! The commands that make and change a document's linear dimensions (CON-A9-F1-P1), and the read-
//! only listing of a view's dimensions with their values worked out. They go through the same edit
//! log as `element.*`, with the same access check (`model.edit`) and the same claims: a piece
//! another person is working on is refused, and one person's edits land in log order.
//!
//! A dimension is an element record of the dimensions category with no shape
//! ([`construct_dimensions`]): its view, how it lies, and the references it is bound to. Its value
//! is never stored: [`listing`] works it out from where each reference is, at the log's head, every
//! time it is asked ([`locate`] reads the document's records, as the regenerator does: a layered
//! wall's filled recipe, a grid's recipe, a hosted element's place on its wall). A reference that
//! was deleted, or has no place in the view, is reported by name as a warning in the shape
//! construct-warnings uses, and the segments it ends have no value.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `dimension.create` | `dimension` (the new ID), `view`, `layout`, `references` (two or more) | A dimension record |
//! | `dimension.reference.add` | `dimension`, `reference`; optional `at` (the position, from 0; default last) | The dimension, with one more reference |
//! | `dimension.reference.remove` | `dimension`, `at` | The dimension, one reference fewer (it keeps two) |
//! | `dimension.move` | `dimension`, and `line` (a horizontal or vertical one) or `through` (an aligned one) | The dimension, its offset line moved |
//! | `dimension.equal` | `dimension`, `groups`: lists of segment numbers from 0 to be equal (none clears) | The dimension, its equalities stored; nothing enforces them yet |
//! | `dimension.delete` | `dimension` | Its pointer goes |
//!
//! The values: `layout` is `{"kind": "horizontal", "y": <typed length>}`, `{"kind": "vertical",
//! "x": ...}` or `{"kind": "aligned", "through": {"x": ..., "y": ...}, "direction": {"x": ...,
//! "y": ...}}`; a `reference` is `{"element": <ID>, "feature": <key>}` with the feature one of
//! `wall.face.exterior`, `wall.face.interior`, `wall.line`, `grid.line`, `level`, `centre`,
//! `edge.start`, `edge.end` ([`construct_dimensions::Feature`]). Typed lengths are read with
//! construct-params' metric display settings (`5000`, `1.2 m`).
//!
//! Refused by name: a view that is not there, or is not a plan (a section's or elevation's frame is
//! job 8.1's: interim); an ID that is taken; fewer than two references; two neighbours the same; a
//! reference that cannot be found or read now (a deleted element, a level in a plan, a curved
//! grid, a face that runs along the offset line) with the reason; changing the references of a
//! dimension that holds equalities; deleting a view that holds dimensions
//! ([`held_refusal`]). An undo that would leave a dimension in a view that is not there is turned
//! back ([`undo_rules`]).
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a
//! dimension and `element.set`, `element.move` or `element.delete` of one are refused, naming the
//! command to use. Deleting an element a dimension names is not refused: the dimension says so.
//!
//! Interim: the places of a reference ([`locate`]) are read from the records for plans. The one
//! function that learns sections and elevations (job 8.1) is [`locate`]; nothing else changes. A
//! door's or window's centre falls on a half step when its opening is an odd number of steps wide;
//! it is then the nearer step above (a bound of half a step, 78 nm).

use crate::cloud::elements::write_record;
use construct_categories::DIMENSIONS;
use construct_dimensions::{
    DIMENSION_KIND, Dimension, DimensionError, DimensionWarning, Dimensions, Direction, Edge,
    Feature, Layout, Located, Locus, Measured, Missing, Reference, Side, Unmeasured, display,
    labels, measure,
};
use construct_grids::{Datum, Point, Shape, datum_of_recipe};
use construct_model::element::Element;
use construct_model::geometry::{Edition, Recipe};
use construct_model::keys::Place as ModelPlace;
use construct_model::rows::Model;
use construct_params::{Length, Units, Value};
use construct_regen::families as regen_families;
use construct_regen::hosting::{self, Hosting};
use construct_regen::levels::recipe_at;
use construct_regen::parameters::{self as records, At};
use construct_regen::wall_plan::{self, Layered};
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, Reader, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Every command here.
pub const COMMANDS: [&str; 6] = [
    "dimension.create",
    "dimension.reference.add",
    "dimension.reference.remove",
    "dimension.move",
    "dimension.equal",
    "dimension.delete",
];

/// The answer to a command that would make or change a dimension as an element.
pub const BY_DIMENSION_COMMANDS: &str =
    "Dimensions are made and changed by the dimension commands.";

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// The arguments of every command here; each uses some of them.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    dimension: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    view: Option<Id>,
    layout: Option<LayoutArgs>,
    references: Option<Vec<ReferenceArgs>>,
    reference: Option<ReferenceArgs>,
    at: Option<usize>,
    line: Option<String>,
    through: Option<PointArgs>,
    groups: Option<Vec<Vec<usize>>>,
}

/// A layout as a person sends it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LayoutArgs {
    kind: String,
    y: Option<String>,
    x: Option<String>,
    through: Option<PointArgs>,
    direction: Option<PointArgs>,
}

/// A point or a vector as a person sends it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PointArgs {
    x: String,
    y: String,
}

/// A reference as a person sends it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceArgs {
    #[serde(with = "kit_ops::id")]
    element: Id,
    feature: String,
}

/// The part of a person's row a claim needs.
#[derive(Deserialize)]
struct Named {
    name: String,
}

/// A command's answer: a store failure, or the command's own result.
type Answer = Result<Outcome, StoreError>;

fn need<T>(value: Option<T>, what: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("This command needs {what}."))
}

fn length(text: &str) -> Result<Length, String> {
    crate::cloud::hosting::length("Dimension length", text)
}

fn point_of(args: &PointArgs) -> Result<Point, String> {
    Ok(Point::new(length(&args.x)?, length(&args.y)?))
}

fn words(error: &DimensionError) -> String {
    error.to_string()
}

fn layout_of(args: &LayoutArgs) -> Result<Layout, String> {
    let has = (
        args.y.is_some(),
        args.x.is_some(),
        args.through.is_some(),
        args.direction.is_some(),
    );
    let only = |ok: bool, what: &str| {
        if ok {
            Ok(())
        } else {
            Err(format!("A {} layout takes {what}.", args.kind))
        }
    };
    match args.kind.as_str() {
        "horizontal" => {
            only(has == (true, false, false, false), "only y")?;
            Layout::horizontal(length(&need(args.y.clone(), "y")?)?).map_err(|e| words(&e))
        }
        "vertical" => {
            only(has == (false, true, false, false), "only x")?;
            Layout::vertical(length(&need(args.x.clone(), "x")?)?).map_err(|e| words(&e))
        }
        "aligned" => {
            only(
                has == (false, false, true, true),
                "a through point and a direction",
            )?;
            let through = point_of(need(args.through.as_ref(), "a through point")?)?;
            let direction = need(args.direction.as_ref(), "a direction")?;
            let direction = Direction::new(length(&direction.x)?, length(&direction.y)?)
                .map_err(|e| words(&e))?;
            Layout::aligned(through, direction).map_err(|e| words(&e))
        }
        other => Err(format!(
            "{other:?} is not a layout: horizontal, vertical or aligned."
        )),
    }
}

fn reference_of(args: &ReferenceArgs) -> Result<Reference, String> {
    let feature = Feature::from_key(&args.feature).ok_or_else(|| {
        let keys: Vec<&str> = Feature::ALL.iter().map(|f| f.key()).collect();
        format!(
            "{:?} is not something to dimension to: {}.",
            args.feature,
            keys.join(", ")
        )
    })?;
    Ok(Reference::new(args.element.clone(), feature))
}

// ---------------------------------------------------------------------------------------------
// Where a reference is: the records read as the regenerator reads them.

fn steps_of(mm: i64) -> Result<i64, Missing> {
    Length::checked_from_mm(mm)
        .map(Length::steps)
        .ok_or_else(|| Missing::unreadable("its placement is too far from the origin"))
}

fn grid_point(x: i64, y: i64) -> Point {
    Point::new(Length::from_steps(x), Length::from_steps(y))
}

fn too_far() -> Missing {
    Missing::unreadable("it is beyond the kernel's grid")
}

/// A wall as the dimensions read it, in the model's plan.
struct WallFrame {
    /// The reference line's start.
    start: [i64; 2],
    /// The vector from its start to its end.
    direction: [i64; 2],
    /// How far the exterior face is from the line, to the right as walked, in steps.
    exterior: i64,
    /// How far the interior face is, the same way.
    interior: i64,
    /// Its unit axis, when it runs along one.
    axis: Option<[i64; 2]>,
    /// A layered wall's plan, which rounds the faces of a slanted one.
    plan: Option<wall_plan::Plan>,
    /// Where its frame lies in the model: its placement, in steps.
    frame: [i64; 2],
}

impl WallFrame {
    /// The line at `offset` steps to the right of the reference line.
    fn line(&self, offset: i64) -> Result<Locus, Missing> {
        let start = match (&self.plan, self.axis) {
            (Some(plan), _) => {
                let face = plan
                    .face(offset)
                    .map_err(|e| Missing::unreadable(&e.message))?;
                face.first().copied().unwrap_or(self.start)
            }
            (None, Some([ux, uy])) => {
                let right = [uy, ux.checked_neg().ok_or_else(too_far)?];
                let at = |c: usize, r: i64| {
                    offset
                        .checked_mul(r)
                        .and_then(|d| self.start.get(c).and_then(|s| s.checked_add(d)))
                };
                [
                    at(0, right[0]).ok_or_else(too_far)?,
                    at(1, right[1]).ok_or_else(too_far)?,
                ]
            }
            (None, None) => return Err(Missing::unreadable("it has no line")),
        };
        let through = self.in_model(start)?;
        Ok(Locus::Line {
            through,
            direction: self.direction,
        })
    }

    fn in_model(&self, at: [i64; 2]) -> Result<Point, Missing> {
        let sum = |a: i64, b: i64| a.checked_add(b).ok_or_else(too_far);
        Ok(grid_point(
            sum(at[0], self.frame[0])?,
            sum(at[1], self.frame[1])?,
        ))
    }
}

/// The filled recipe of a record, as a command asks a shape's rules of it.
async fn filled<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    record: &Element,
) -> Result<Result<Recipe, Missing>, StoreError> {
    Ok(recipe_at(reader, project, document, record, At::Now)
        .await?
        .map_err(|e| Missing::unreadable(&e.message)))
}

/// The wall a record is, if it is one: a layered wall, or the old box wall.
fn wall_of(record: &Element, recipe: &Recipe) -> Result<Option<WallFrame>, Missing> {
    let frame = [steps_of(record.placement.x)?, steps_of(record.placement.y)?];
    if wall_plan::is_layered(recipe) {
        let wall = Layered::of(recipe).map_err(|e| Missing::unreadable(&e.message))?;
        let (exterior, interior) = wall.outer();
        return Ok(Some(WallFrame {
            start: wall.plan.start(),
            direction: wall.plan.direction(),
            exterior,
            interior,
            axis: wall.plan.axis().map(wall_plan::Axis::unit),
            plan: Some(wall.plan),
            frame,
        }));
    }
    if recipe.owner == "construct" && recipe.op == hosting::HOST_OP {
        let whole = |arg: &str| {
            recipe
                .args
                .get(arg)
                .and_then(serde_json::Value::as_i64)
                .ok_or_else(|| Missing::unreadable(&format!("its {arg} is not whole millimetres")))
        };
        let (long, thick) = (steps_of(whole("length")?)?, steps_of(whole("thickness")?)?);
        let along_y = recipe.args.get("axis").and_then(serde_json::Value::as_str) == Some("y");
        // The box wall's exterior is the side at y = 0 along x, and at x = its thickness along y;
        // its reference line is the middle of its thickness, so each face is half of it away.
        let half = thick.div_euclid(2);
        let (start, direction, axis) = if along_y {
            ([half, 0], [0, long], [0, 1])
        } else {
            ([0, half], [long, 0], [1, 0])
        };
        return Ok(Some(WallFrame {
            start,
            direction,
            exterior: half,
            interior: half.checked_neg().ok_or_else(too_far)?,
            axis: Some(axis),
            plan: None,
            frame,
        }));
    }
    Ok(None)
}

async fn edition_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Option<Edition>, StoreError> {
    let place = ModelPlace::Model {
        project: project.clone(),
        model: document.clone(),
    };
    let model: Option<Model> = read_row(reader, &place.key()).await?;
    Ok(model.map(|model| model.edition))
}

/// Where along its wall a hosted element's opening lies, from the wall's start, low then high, in
/// steps; `None` with a reason when the element has no opening to read.
fn opening_along(
    recipe: &Recipe,
    element: &Id,
    along: Length,
    outward: i64,
) -> Result<[i64; 2], Missing> {
    let from = |low: i64, high: i64| -> Result<[i64; 2], Missing> {
        let (a, b) = (
            along.steps().checked_add(low).ok_or_else(too_far)?,
            along.steps().checked_add(high).ok_or_else(too_far)?,
        );
        Ok([a.min(b), a.max(b)])
    };
    if construct_openings::is_wall_opening(recipe) {
        let opening = construct_openings::WallOpening::of(element.as_str(), recipe)
            .map_err(|e| Missing::unreadable(&e.to_string()))?;
        return from(0, opening.width.steps());
    }
    if regen_families::is_instance(recipe) {
        // The family's origin is the middle of its opening, and its +x runs against the wall's
        // walk when it is on the exterior side (construct-regen's `hosting::seat`).
        let (_, span) = regen_families::opening_of(&*regen_families::baked(), recipe)
            .map_err(|e| Missing::unreadable(&e.message))?;
        let [low, high] = span.x;
        let turn = |x: Length| x.steps().checked_mul(outward.checked_neg().unwrap_or(0));
        let (a, b) = (
            turn(low).ok_or_else(too_far)?,
            turn(high).ok_or_else(too_far)?,
        );
        return from(a, b);
    }
    if recipe.owner == "construct" && recipe.op == "door.single" {
        let width = recipe
            .args
            .get("width")
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| Missing::unreadable("its width is not whole millimetres"))?;
        return from(0, steps_of(width)?);
    }
    Err(Missing::no_such_part(&format!(
        "a {} has no centre or edges",
        recipe.op
    )))
}

/// The place of a door's, a window's or an opening's centre or edge: a line across its wall.
async fn hosted_locus<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    record: &Element,
    feature: Feature,
) -> Result<Result<Locus, Missing>, StoreError> {
    let placed = match hosting::read(record) {
        Ok(Some(Hosting::OnFace {
            host, face, along, ..
        })) => (host, face, along),
        Ok(_) => {
            return Ok(Err(Missing::no_such_part(
                "it is not hosted on a wall at an offset, so it has no centre or edges",
            )));
        }
        Err(why) => return Ok(Err(Missing::unreadable(&why))),
    };
    let (host_id, face, along) = placed;
    let Some((_, host)) = records::read_element(reader, project, document, &host_id).await? else {
        return Ok(Err(Missing::no_such_part(
            "the wall it is hosted on is gone",
        )));
    };
    let host_recipe = match filled(reader, project, document, &host).await? {
        Ok(recipe) => recipe,
        Err(missing) => return Ok(Err(missing)),
    };
    let recipe = match filled(reader, project, document, record).await? {
        Ok(recipe) => recipe,
        Err(missing) => return Ok(Err(missing)),
    };
    let wall = match wall_of(&host, &host_recipe) {
        Ok(Some(wall)) => wall,
        Ok(None) => {
            return Ok(Err(Missing::no_such_part(
                "the element it is hosted on is not a wall",
            )));
        }
        Err(missing) => return Ok(Err(missing)),
    };
    let Some([ux, uy]) = wall.axis else {
        return Ok(Err(Missing::no_such_part(
            "its wall runs at an angle: only a wall along x or y hosts",
        )));
    };
    // Which side the element is on decides which way a family's own x runs: only a family's
    // opening is not the same either way round, so only a family needs the side.
    let interior = if regen_families::is_instance(&recipe) {
        let edition = edition_of(reader, project, document).await?;
        let names =
            edition.and_then(|edition| construct_regen::face_names(&edition, &host_recipe).ok());
        let Some(names) = names else {
            return Ok(Err(Missing::unreadable(
                "the faces of its wall cannot be named, so the side it is on is not known",
            )));
        };
        names
            .iter()
            .any(|name| name.name == face && name.role == "interior")
    } else {
        false
    };
    let exterior_is_right: i64 = if wall.exterior > wall.interior { 1 } else { -1 };
    let outward = if interior {
        exterior_is_right.checked_neg().unwrap_or(0)
    } else {
        exterior_is_right
    };
    let [low, high] = match opening_along(&recipe, &record.id, along, outward) {
        Ok(span) => span,
        Err(missing) => return Ok(Err(missing)),
    };
    let at = match feature {
        Feature::Edge(Edge::Start) => low,
        Feature::Edge(Edge::End) => high,
        // The middle, a half step rounding up.
        _ => match low.checked_add(high) {
            Some(sum) => sum
                .div_euclid(2)
                .saturating_add(i64::from(sum.rem_euclid(2) == 1)),
            None => return Ok(Err(too_far())),
        },
    };
    let point = |c: i64, s: i64, u: i64| {
        c.checked_mul(u)
            .and_then(|d| s.checked_add(d))
            .ok_or_else(too_far)
    };
    let on_line = || -> Result<Point, Missing> {
        // The reference line of the wall, at the distance along it, in the wall's frame.
        let line_start = match &wall.plan {
            Some(plan) => plan.start(),
            None => wall.start,
        };
        wall.in_model([point(at, line_start[0], ux)?, point(at, line_start[1], uy)?])
    };
    Ok(on_line().map(|through| Locus::Line {
        through,
        direction: [uy, ux.checked_neg().unwrap_or(0)],
    }))
}

/// Where a reference is, in the plane of a plan, read from the document as it is now.
///
/// # Errors
/// A [`StoreError`] when the store fails; a reference that cannot be found is the inner error.
pub async fn locate<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    reference: &Reference,
) -> Result<Result<Locus, Missing>, StoreError> {
    let Some((_, record)) =
        records::read_element(reader, project, document, reference.element()).await?
    else {
        return Ok(Err(Missing::deleted()));
    };
    match reference.feature() {
        Feature::WallFace(_) | Feature::WallLine => {
            let recipe = match filled(reader, project, document, &record).await? {
                Ok(recipe) => recipe,
                Err(missing) => return Ok(Err(missing)),
            };
            Ok(match wall_of(&record, &recipe) {
                Ok(Some(wall)) => {
                    let offset = match reference.feature() {
                        Feature::WallFace(Side::Exterior) => wall.exterior,
                        Feature::WallFace(Side::Interior) => wall.interior,
                        _ => 0,
                    };
                    wall.line(offset)
                }
                Ok(None) => Err(Missing::no_such_part("it is not a wall")),
                Err(missing) => Err(missing),
            })
        }
        Feature::GridLine => Ok(match datum_of_recipe(&record.recipe) {
            Some(Ok(Datum::Grid(Shape::Line { from, to }))) => {
                match (to.x.checked_sub(from.x), to.y.checked_sub(from.y)) {
                    (Some(dx), Some(dy)) => Ok(Locus::line(from, dx, dy)),
                    _ => Err(too_far()),
                }
            }
            Some(Ok(Datum::Grid(Shape::Arc { .. }))) => Err(Missing::no_such_part(
                "a curved grid has no straight line to measure to",
            )),
            Some(Ok(Datum::Plane(_))) | None => Err(Missing::no_such_part("it is not a grid line")),
            Some(Err(e)) => Err(Missing::unreadable(&e.to_string())),
        }),
        Feature::Level => Ok(Err(Missing::no_such_part(
            "a level has no place in a plan: dimension to levels in a section or an elevation",
        ))),
        Feature::Centre | Feature::Edge(_) => {
            hosted_locus(reader, project, document, &record, reference.feature()).await
        }
    }
}

/// The places of every reference of `dimensions`, as a table a dimension is worked out from.
async fn located<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    references: &[&Reference],
) -> Result<Located, StoreError> {
    let mut table = Located::new();
    for reference in references {
        let answer = locate(reader, project, document, reference).await?;
        table.set((*reference).clone(), answer);
    }
    Ok(table)
}

// ---------------------------------------------------------------------------------------------
// The document's dimensions, and what a page reads.

/// A document as the dimension commands read it: its dimensions and views, and each record's hash.
pub struct World {
    dimensions: Dimensions,
    views: construct_views::Views,
    all: BTreeMap<Id, (Id, Element)>,
}

impl World {
    /// The document's dimensions.
    #[must_use]
    pub fn dimensions(&self) -> &Dimensions {
        &self.dimensions
    }

    /// The document's views.
    #[must_use]
    pub fn views(&self) -> &construct_views::Views {
        &self.views
    }

    fn record(&self, id: &Id) -> Option<&(Id, Element)> {
        self.all.get(id)
    }
}

/// A document's dimensions and views as they are now. The inner error is a record that is not as
/// written, in words.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn read_world<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<World, String>, StoreError> {
    let all = records::read_document(reader, project, document).await?;
    let found: Vec<&Element> = all.values().map(|(_, record)| record).collect();
    let dimensions = match Dimensions::from_records(found.iter().copied()) {
        Ok(dimensions) => dimensions,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let views = match construct_views::Views::from_records(found.iter().copied()) {
        Ok(views) => views,
        Err(e) => return Ok(Err(e.to_string())),
    };
    Ok(Ok(World {
        dimensions,
        views,
        all,
    }))
}

/// A point as a page reads it: each coordinate in construct-params' stored length text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PointView {
    /// Across.
    pub x: String,
    /// Up.
    pub y: String,
}

fn point_view(point: Point) -> PointView {
    PointView {
        x: Value::Length(point.x).stored(),
        y: Value::Length(point.y).stored(),
    }
}

fn stored(length: Length) -> String {
    Value::Length(length).stored()
}

/// How a dimension lies, as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LayoutView {
    /// `horizontal`, `vertical` or `aligned`.
    pub kind: String,
    /// The y of a horizontal dimension's line.
    pub y: Option<String>,
    /// The x of a vertical dimension's line.
    pub x: Option<String>,
    /// A point of an aligned dimension's line.
    pub through: Option<PointView>,
    /// An aligned dimension's direction.
    pub direction: Option<PointView>,
}

fn layout_view(layout: &Layout) -> LayoutView {
    let mut view = LayoutView {
        kind: layout.key().to_owned(),
        y: None,
        x: None,
        through: None,
        direction: None,
    };
    match *layout {
        Layout::Horizontal { y } => view.y = Some(stored(y)),
        Layout::Vertical { x } => view.x = Some(stored(x)),
        Layout::Aligned { through, direction } => {
            view.through = Some(point_view(through));
            view.direction = Some(PointView {
                x: stored(direction.dx()),
                y: stored(direction.dy()),
            });
        }
    }
    view
}

/// A reference as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReferenceView {
    /// The element it is bound to.
    pub element: String,
    /// The part of it, by key.
    pub feature: String,
    /// It in a sentence: `the exterior face of W-1`.
    pub words: String,
    /// Whether it can be used now.
    pub usable: bool,
}

/// One segment of a dimension as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SegmentView {
    /// The reference it starts at, from 0.
    pub from: usize,
    /// The reference it ends at.
    pub to: usize,
    /// Its value in stored length text, or `None` when it cannot be worked out.
    pub value: Option<String>,
    /// Its value as the person's units show it: `2400 mm`.
    pub text: Option<String>,
    /// Whether the value is the true one, to the step.
    pub exact: bool,
    /// Where its text is centred, on the offset line.
    pub middle: Option<PointView>,
    /// Whether its text is no wider than the segment, once the text engine can say (always
    /// `None` until job 7.4's text exists).
    pub fits: Option<bool>,
}

/// A witness line as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WitnessView {
    /// The reference it belongs to, from 0.
    pub reference: usize,
    /// Where it starts, on the reference.
    pub from: PointView,
    /// Where it ends, on the offset line.
    pub to: PointView,
}

/// A stored equality as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EqualView {
    /// The segments held equal, from 0.
    pub segments: Vec<usize>,
    /// Whether they are equal now; `None` when one cannot be worked out. Nothing enforces it yet.
    pub equal: Option<bool>,
}

/// A warning as a page reads it (the shape construct-warnings gives every warning).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WarningView {
    /// Its kind's stable name.
    pub kind: String,
    /// `notice`, `warning` or `error`.
    pub severity: String,
    /// The elements involved: the dimension, then the element referenced.
    pub elements: Vec<String>,
    /// Its identity, `kind|id|id`.
    pub identity: String,
    /// A plain sentence.
    pub words: String,
}

fn warning_view(warning: &DimensionWarning) -> WarningView {
    WarningView {
        kind: warning.kind.to_owned(),
        severity: warning.severity.word().to_owned(),
        elements: warning.elements.iter().map(ToString::to_string).collect(),
        identity: warning.identity(),
        words: warning.words.clone(),
    }
}

/// One dimension with its values worked out, as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DimensionView {
    /// Its permanent ID.
    pub id: String,
    /// How it lies.
    pub layout: LayoutView,
    /// Its references, in order.
    pub references: Vec<ReferenceView>,
    /// Its segments, in order.
    pub segments: Vec<SegmentView>,
    /// The distance from the first reference to the last, when every reference can be used.
    pub overall: Option<String>,
    /// The overall distance as the person's units show it.
    pub overall_text: Option<String>,
    /// A witness line for each reference that can be drawn.
    pub witnesses: Vec<WitnessView>,
    /// Its stored equalities and whether each holds now.
    pub equalities: Vec<EqualView>,
    /// Its warnings: what keeps a value from being worked out.
    pub warnings: Vec<WarningView>,
}

/// One view's dimensions with their values, as of the log's head.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DimensionListing {
    /// The view.
    pub view: String,
    /// The view's name.
    pub view_name: String,
    /// Its dimensions, by ID.
    pub dimensions: Vec<DimensionView>,
}

fn view_of_dimension(
    dimension: &Dimension,
    measured: &Measured,
    units: &Units,
) -> Result<DimensionView, String> {
    let shown = labels(measured, units, &Unmeasured).map_err(|e| e.to_string())?;
    let segments = measured
        .segments
        .iter()
        .map(|segment| {
            let label = shown.iter().find(|label| label.segment == segment.from);
            SegmentView {
                from: segment.from,
                to: segment.to,
                value: segment.value.map(stored),
                text: label.map(|label| label.text.clone()),
                exact: segment.exact,
                middle: segment.middle.map(point_view),
                fits: label.and_then(|label| label.fits),
            }
        })
        .collect();
    let overall_text = measured
        .overall
        .map(|overall| display(overall, units))
        .transpose()
        .map_err(|e| e.to_string())?;
    Ok(DimensionView {
        id: dimension.id().to_string(),
        layout: layout_view(dimension.layout()),
        references: dimension
            .references()
            .iter()
            .enumerate()
            .map(|(at, reference)| ReferenceView {
                element: reference.element().to_string(),
                feature: reference.feature().key().to_owned(),
                words: reference.describe(),
                usable: !measured.unusable.contains(&at),
            })
            .collect(),
        segments,
        overall: measured.overall.map(stored),
        overall_text,
        witnesses: measured
            .witnesses
            .iter()
            .flatten()
            .map(|witness| WitnessView {
                reference: witness.reference,
                from: point_view(witness.from),
                to: point_view(witness.to),
            })
            .collect(),
        equalities: measured
            .equalities
            .iter()
            .map(|state| EqualView {
                segments: state.segments.clone(),
                equal: state.equal,
            })
            .collect(),
        warnings: measured.warnings.iter().map(warning_view).collect(),
    })
}

/// The dimensions of a view with their values worked out from the document as it is now. `None`
/// when the document has no such view.
///
/// # Errors
/// A [`StoreError`] when the store fails; the inner error is a record that does not read, or a
/// value that cannot be shown, in words.
pub async fn listing<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    view: &Id,
) -> Result<Result<Option<DimensionListing>, String>, StoreError> {
    let world = match read_world(reader, project, document).await? {
        Ok(world) => world,
        Err(why) => return Ok(Err(why)),
    };
    let Some(shown) = world.views.get(view) else {
        return Ok(Ok(None));
    };
    let units = match records::read_context(reader, project, document).await? {
        Ok(context) => context.units,
        Err(why) => return Ok(Err(why)),
    };
    let mut found = Vec::new();
    for dimension in world.dimensions.of_view(view) {
        let references: Vec<&Reference> = dimension.references().iter().collect();
        let table = located(reader, project, document, &references).await?;
        let measured = measure(dimension, &table);
        match view_of_dimension(dimension, &measured, &units) {
            Ok(one) => found.push(one),
            Err(why) => return Ok(Err(why)),
        }
    }
    Ok(Ok(Some(DimensionListing {
        view: view.to_string(),
        view_name: shown.name().to_owned(),
        dimensions: found,
    })))
}

// ---------------------------------------------------------------------------------------------
// The commands.

/// A write a command makes: a record put (with the hash it replaces), or a piece taken away (with
/// the hash it had).
enum Write {
    Put(Box<Element>, Option<Id>),
    Remove(Id, Id),
}

/// What a command is run against: the store, where, who, and when the orderer says it is.
struct Run<'a, T: Txn> {
    txn: &'a mut T,
    project: &'a Id,
    document: &'a Id,
    edit: &'a Edit,
    now_ms: u64,
    claim_ms: u64,
}

impl<T: Txn> Run<'_, T> {
    /// The claim this person takes on a piece, or who else holds it.
    async fn claim(&mut self, piece: &Id) -> Result<Result<Claim, String>, StoreError> {
        let holder = claims::held_by_another(
            self.txn,
            self.project,
            self.document,
            piece,
            &self.edit.person,
            self.now_ms,
        )
        .await?;
        if let Some(claim) = holder {
            return Ok(Err(format!("{} is working on this.", claim.name)));
        }
        let person: Option<Named> =
            read_row(self.txn, &Place::Person(self.edit.person.clone()).key()).await?;
        let name = person.map_or_else(|| self.edit.person.to_string(), |person| person.name);
        Ok(Ok(Claim {
            person: self.edit.person.clone(),
            name,
            // milliseconds since 1970 plus a claim's length: far below u64::MAX
            until_ms: self.now_ms.saturating_add(self.claim_ms),
        }))
    }

    /// Makes the writes of one command, taking this person's claim on every piece first.
    async fn write_all(&mut self, writes: Vec<Write>) -> Answer {
        let mut held = Vec::with_capacity(writes.len());
        for write in &writes {
            let piece = match write {
                Write::Put(record, _) => &record.id,
                Write::Remove(piece, _) => piece,
            };
            match self.claim(piece).await? {
                Ok(claim) => held.push(claim),
                Err(refusal) => return Ok(Err(refusal)),
            }
        }
        let mut changes = Vec::with_capacity(writes.len());
        let mut taken = Vec::with_capacity(writes.len());
        for (write, claim) in writes.into_iter().zip(held) {
            match write {
                Write::Put(record, before) => {
                    let hash = write_record(self.txn, self.project, &record).await?;
                    let pointer = Place::Piece {
                        project: self.project.clone(),
                        document: self.document.clone(),
                        piece: record.id.clone(),
                    };
                    self.txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
                    taken.push(
                        claims::take(self.txn, self.project, self.document, &record.id, claim)
                            .await?,
                    );
                    changes.push(Change {
                        piece: record.id,
                        before,
                        after: Some(hash),
                    });
                }
                Write::Remove(piece, hash) => {
                    let pointer = Place::Piece {
                        project: self.project.clone(),
                        document: self.document.clone(),
                        piece: piece.clone(),
                    };
                    self.txn.delete(pointer.key()).await?;
                    taken.push(
                        claims::release(self.txn, self.project, self.document, &piece).await?,
                    );
                    changes.push(Change {
                        piece,
                        before: Some(hash),
                        after: None,
                    });
                }
            }
        }
        Ok(Ok(Applied {
            changes,
            claims: taken,
            ..Applied::default()
        }))
    }
}

/// Runs one of these commands.
///
/// # Errors
///
/// A store error; a command that cannot be made is the outcome's error instead.
pub async fn apply<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Ordered<'_>,
    edit: &Edit,
    claim_ms: u64,
) -> Result<Outcome, StoreError> {
    let args: Args = match serde_json::from_value(edit.args.clone()) {
        Ok(args) => args,
        Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
    };
    let mut run = Run {
        txn,
        project,
        document: &edit.target,
        edit,
        now_ms: context.now_ms,
        claim_ms,
    };
    let world = match read_world(run.txn, project, &edit.target).await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    match edit.cmd.as_str() {
        "dimension.create" => create(&mut run, &world, args).await,
        "dimension.reference.add" => add_reference(&mut run, &world, args).await,
        "dimension.reference.remove" => remove_reference(&mut run, &world, args).await,
        "dimension.move" => move_line(&mut run, &world, args).await,
        "dimension.equal" => set_equal(&mut run, &world, args).await,
        "dimension.delete" => delete(&mut run, &world, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// Why a view cannot hold dimensions now: it is not a plan.
fn plan_refusal(world: &World, view: &Id) -> Result<(), String> {
    let Some(found) = world.views.get(view) else {
        return Err(format!("There is no view {view} in this model."));
    };
    if found.kind().plan().is_some() {
        Ok(())
    } else {
        Err(format!(
            "The view {view} is a {}: dimensions are made in plans for now, because a section's or \
             an elevation's drawing frame comes with the vertical sections (job 8.1).",
            found.kind().key()
        ))
    }
}

/// The dimension a command is about, with its record's hash.
fn dimension_of(world: &World, id: Option<Id>) -> Result<(&Id, &Dimension), String> {
    let id = need(id, "a dimension")?;
    let missing = || DimensionError::Unknown(id.clone()).to_string();
    let dimension = world.dimensions.get(&id).ok_or_else(missing)?;
    let (hash, _) = world.record(&id).ok_or_else(missing)?;
    Ok((hash, dimension))
}

/// The refusal a changed dimension earns: the first reference among `positions` that cannot be
/// used now, in the measurement's own words (each unusable reference has one warning, whose
/// sentence names it by position).
fn unusable_among(measured: &Measured, positions: &[usize]) -> Result<(), String> {
    for at in measured.unusable.iter().filter(|at| positions.contains(at)) {
        let marker = format!(": reference {} (", at.saturating_add(1));
        if let Some(warning) = measured.warnings.iter().find(|w| w.words.contains(&marker)) {
            return Err(warning.words.clone());
        }
    }
    Ok(())
}

async fn measure_now<T: Txn>(
    run: &mut Run<'_, T>,
    dimension: &Dimension,
) -> Result<Measured, StoreError> {
    let references: Vec<&Reference> = dimension.references().iter().collect();
    let table = located(run.txn, run.project, run.document, &references).await?;
    Ok(measure(dimension, &table))
}

async fn create<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let made = (|| {
        let id = need(args.dimension.clone(), "a dimension")?;
        let view = need(args.view.clone(), "a view")?;
        let layout = layout_of(need(args.layout.as_ref(), "a layout")?)?;
        let references = need(args.references.as_ref(), "references")?
            .iter()
            .map(reference_of)
            .collect::<Result<Vec<_>, _>>()?;
        plan_refusal(world, &view)?;
        Dimension::new(id, view, layout, references).map_err(|e| words(&e))
    })();
    let dimension = match made {
        Ok(dimension) => dimension,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if world.record(dimension.id()).is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let measured = measure_now(run, &dimension).await?;
    let every: Vec<usize> = (0..dimension.references().len()).collect();
    if let Err(refusal) = unusable_among(&measured, &every) {
        return Ok(Err(refusal));
    }
    run.write_all(vec![Write::Put(
        Box::new(dimension.to_record(run.document)),
        None,
    )])
    .await
}

/// Writes `changed` over `before`, or nothing at all when it is the same.
async fn put<T: Txn>(
    run: &mut Run<'_, T>,
    hash: &Id,
    before: &Dimension,
    changed: Dimension,
) -> Answer {
    if changed == *before {
        return Ok(Ok(Applied::default()));
    }
    run.write_all(vec![Write::Put(
        Box::new(changed.to_record(run.document)),
        Some(hash.clone()),
    )])
    .await
}

async fn add_reference<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, old) = match dimension_of(world, args.dimension.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = (|| {
        let reference = reference_of(need(args.reference.as_ref(), "a reference")?)?;
        let at = args.at.unwrap_or(old.references().len());
        let changed = old
            .clone()
            .with_reference_added(at, reference)
            .map_err(|e| words(&e))?;
        Ok::<_, String>((changed, at))
    })();
    let (changed, at) = match changed {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let measured = measure_now(run, &changed).await?;
    if let Err(refusal) = unusable_among(&measured, &[at]) {
        return Ok(Err(refusal));
    }
    put(run, hash, old, changed).await
}

async fn remove_reference<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, old) = match dimension_of(world, args.dimension.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = need(args.at, "the position of a reference").and_then(|at| {
        old.clone()
            .with_reference_removed(at)
            .map_err(|e| words(&e))
    });
    match changed {
        Ok(changed) => put(run, hash, old, changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn move_line<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, old) = match dimension_of(world, args.dimension.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let moved = (|| {
        let layout = match (&args.line, &args.through) {
            (Some(line), None) => old.layout().with_line(length(line)?),
            (None, Some(through)) => old.layout().with_through(point_of(through)?),
            _ => {
                return Err("This command needs a line (horizontal or vertical) or a through point (aligned).".to_owned());
            }
        }
        .map_err(|e| words(&e))?;
        Ok::<_, String>(old.clone().with_layout(layout))
    })();
    match moved {
        Ok(changed) => put(run, hash, old, changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn set_equal<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, old) = match dimension_of(world, args.dimension.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = need(args.groups.clone(), "groups of segments (none clears them)")
        .and_then(|groups| old.clone().with_equalities(groups).map_err(|e| words(&e)));
    match changed {
        Ok(changed) => put(run, hash, old, changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn delete<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, old) = match dimension_of(world, args.dimension.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    run.write_all(vec![Write::Remove(old.id().clone(), hash.clone())])
        .await
}

/// Why a view cannot be deleted: it holds dimensions. `None` when it holds none (or no dimension
/// record reads, which the dimension commands report).
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn held_refusal<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    view: &Id,
) -> Result<Option<String>, StoreError> {
    let Ok(world) = read_world(reader, project, document).await? else {
        return Ok(None);
    };
    let held = world.dimensions.of_view(view);
    Ok(held.first().map(|first| {
        format!(
            "The view holds {} dimension(s), such as {}; delete them with dimension.delete first.",
            held.len(),
            first.id()
        )
    }))
}

/// The part of a generic command the rules below read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    recipe: Option<Recipe>,
}

/// The generic commands do not go behind the dimension commands: `element.create` of a dimension
/// and `element.set`, `element.move` or `element.delete` of one are refused, naming the command to
/// use. Anything else is left to the command.
///
/// # Errors
///
/// A [`StoreError`] when the store fails.
pub async fn generic_rules<T: Txn>(
    txn: &mut T,
    project: &Id,
    edit: &Edit,
) -> Result<Result<(), String>, StoreError> {
    let Ok(args) = serde_json::from_value::<GenericArgs>(edit.args.clone()) else {
        return Ok(Ok(()));
    };
    let by_recipe = args
        .recipe
        .as_ref()
        .is_some_and(|recipe| records::category_of(recipe).is_ok_and(|c| c == DIMENSIONS));
    let by_kind = args.kind.as_deref() == Some(DIMENSION_KIND);
    Ok(match edit.cmd.as_str() {
        "element.create" if by_recipe || by_kind => Err(format!(
            "A dimension is made: use dimension.create. {BY_DIMENSION_COMMANDS}"
        )),
        "element.set" | "element.move" | "element.delete" => {
            let Some(id) = &args.element else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, &edit.target, id).await? {
                Some((_, record)) if record.kind == DIMENSION_KIND => {
                    Err(match edit.cmd.as_str() {
                        "element.delete" => {
                            "A dimension is deleted: use dimension.delete.".to_owned()
                        }
                        _ => {
                            "A dimension is changed: use dimension.move, dimension.reference.add, \
                          dimension.reference.remove or dimension.equal."
                                .to_owned()
                        }
                    })
                }
                Some(_) if by_recipe => Err(format!(
                    "A dimension is made: use dimension.create. {BY_DIMENSION_COMMANDS}"
                )),
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    })
}

/// Whether the dimensions an undo wrote still stand: every record reads and every dimension is in
/// a plan that is there. The whole document is checked whenever a dimension or a view was written
/// or taken away.
///
/// # Errors
/// A store error; the inner error names what would not stand.
pub async fn undo_rules<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    written: &[Element],
    deleted: &[Id],
) -> Result<Result<(), String>, StoreError> {
    let touched = written
        .iter()
        .any(|record| matches!(record.kind.as_str(), DIMENSION_KIND | "view"));
    // A piece taken away may be a view or a dimension: its kind is not known any more.
    if !touched && deleted.is_empty() {
        return Ok(Ok(()));
    }
    let world = match read_world(txn, project, document).await? {
        Ok(world) => world,
        Err(why) => return Ok(Err(why)),
    };
    for dimension in world.dimensions.all() {
        if let Err(why) = plan_refusal(&world, dimension.view()) {
            return Ok(Err(format!(
                "the dimension {} would be left in a view that is not a plan there: {why}",
                dimension.id()
            )));
        }
    }
    Ok(Ok(()))
}
