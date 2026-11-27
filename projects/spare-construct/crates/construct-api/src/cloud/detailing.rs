//! The commands that make and change a view's detail items (CON-A9, job 14.8): detail lines,
//! filled and masking regions, detail components (placed, and repeated along a line), spot slopes
//! and a person's own line styles, and the read-only listing of a view's items. They go through the
//! same edit log as `element.*`, with the same access check (`model.edit`) and the same claims: a
//! piece another person is working on is refused, and one person's edits land in log order.
//!
//! An item is an element record of its own kind with no shape ([`construct_detailing`]); it names
//! the one view it is in and is drawn over the model in that view only. A spot slope's value is
//! never stored: [`listing`] works it out from the floor or roof it reads, at the log's head, every
//! time it is asked, and a face that was deleted or made flat is reported by name.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `detail.style.create` | `style` (the new ID), `name`, `pen` (a model pen's number), `line_pattern` (a pattern's number), `colour` (`#rrggbb`) | A line style record |
//! | `detail.style.set` | `style`, and any of `name`, `pen`, `line_pattern`, `colour` | The style; every line that uses it follows |
//! | `detail.style.delete` | `style` | Its pointer goes; refused while a line or a region's boundary uses it |
//! | `detail.line.create` | `line`, `view`, `curve`; optional `style` | A detail line |
//! | `detail.line.points` | `line`, `curve` | The line, its points edited |
//! | `detail.line.style` | `line`, `style` | The line, its style set |
//! | `detail.line.delete` | `line` | Its pointer goes |
//! | `detail.region.create` | `region`, `view`, `kind` (`filled` or `masking`), `outline`; for a filled one `pattern` and `colour`; optional `boundary` (a style, or null for none) | A region |
//! | `detail.region.points` | `region`, `outline` | The region, its corners edited |
//! | `detail.region.style` | `region`, `boundary` (a style, or null) | The region, its boundary line style set |
//! | `detail.region.fill` | `region`, `pattern`, `colour` | A filled region, its fill set |
//! | `detail.region.delete` | `region` | Its pointer goes |
//! | `detail.component.create` | `component`, `view`, `recipe`, `position`; optional `rotation` | A placed component |
//! | `detail.component.move` | `component`, and any of `position` and `rotation` | The component, moved and turned |
//! | `detail.component.delete` | `component` | Its pointer goes |
//! | `detail.repeat.create` | `repeat`, `view`, `recipe`, `from`, `to`, `spacing`; optional `rotation` | A component repeated along a line |
//! | `detail.repeat.line` | `repeat`, `from`, `to` | The repeat, its line moved |
//! | `detail.repeat.spacing` | `repeat`, and any of `spacing` and `rotation` | The repeat, its spacing and extra turn set |
//! | `detail.repeat.delete` | `repeat` | Its pointer goes |
//! | `detail.slope.create` | `slope`, `view` (a plan), `face` (a floor or a roof), `at` | A spot slope; refused when the face has no slope there |
//! | `detail.slope.move` | `slope`, `at` | The spot slope, moved; refused when the face has none there |
//! | `detail.slope.delete` | `slope` | Its pointer goes |
//!
//! The values: a point is `{"x": <typed length>, "y": ...}`, typed lengths read with construct-params'
//! metric display settings (`5000`, `1.2 m`); a rotation is a typed angle, any angle (`90°`,
//! `37.5°`); a `curve` is `{"kind": "straight", "start": point, "end": point}` or `{"kind": "arc",
//! "centre": point, "start": point, "end": point}` (anticlockwise from the start to the end); a
//! style is `builtin:fine`, `builtin:standard`, `builtin:bold`, `builtin:centre`, `builtin:hidden`,
//! `builtin:dotted`, or the ID of one of the document's own; a `recipe` is the key of a detail
//! component of the library; a `pattern` is a fill pattern's name.
//!
//! Refused by name: a view that is not there; an ID that is taken; a straight line of no length, an
//! arc that is not one, an outline that crosses itself or has no area, a style, fill pattern or
//! component that is not there, a pen or line pattern the project lacks, a spacing that is not
//! above zero, a repeat of more copies than one may make, a slope on a face that is missing, flat,
//! upright or not under the point (with the reason); a slope in a view that is not a plan (its
//! point is in plan); deleting a style that is in use; deleting a view that holds items
//! ([`held_refusal`]). An undo that would leave an item in a view that is not there, a line with a
//! style that is not there, or a record that does not read is turned back ([`undo_rules`]).
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a detail
//! item and `element.set`, `element.move` or `element.delete` of one are refused, naming the
//! commands to use. Deleting a floor or a roof a spot slope reads is not refused: the listing says
//! so.
//!
//! Interim: fill patterns are construct-drawing's table and the pens and line patterns are the
//! default metric set, as the plan's drawing uses them, until a project keeps its own.

use crate::cloud::elements::write_record;
use construct_detailing::{
    BuiltIn, Curve, DetailComponent, DetailError, DetailLine, DetailRepeat, Detailing, Faces, Fill,
    ItemKind, Library, LineStyle, OwnStyle, OwnStyles, Plane, Reading, Region, SLOPE_KIND,
    SlopeRefusal, SpotSlope, StyleRef, ViewItems, colour_of, colour_spelled, is_detail_kind, read,
};
use construct_drawing::hatch::FillPatterns;
use construct_grids::Point;
use construct_model::element::Element;
use construct_model::geometry::Recipe;
use construct_params::{Angle, Definition, Holder, Kind, Length, Units, Value};
use construct_regen::parameters::{self as records, TYPE_KIND};
use construct_roofs::{is_roof, layers_of, roof_of, shape};
use construct_slabs::{Ring, boundary_of, is_floor};
use construct_styles::{LineStyles, PatternId};
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, Reader, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Every command here.
pub const COMMANDS: [&str; 22] = [
    "detail.style.create",
    "detail.style.set",
    "detail.style.delete",
    "detail.line.create",
    "detail.line.points",
    "detail.line.style",
    "detail.line.delete",
    "detail.region.create",
    "detail.region.points",
    "detail.region.style",
    "detail.region.fill",
    "detail.region.delete",
    "detail.component.create",
    "detail.component.move",
    "detail.component.delete",
    "detail.repeat.create",
    "detail.repeat.line",
    "detail.repeat.spacing",
    "detail.repeat.delete",
    "detail.slope.create",
    "detail.slope.move",
    "detail.slope.delete",
];

/// The answer to a command that would make or change a detail item as an element.
pub const BY_DETAIL_COMMANDS: &str = "Detail items are made and changed by the detail commands.";

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
    line: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    region: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    component: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    repeat: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    slope: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    view: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    face: Option<Id>,
    /// A style: an own style's ID in the `detail.style.*` commands, a style reference elsewhere.
    style: Option<String>,
    name: Option<String>,
    pen: Option<u16>,
    line_pattern: Option<u32>,
    colour: Option<String>,
    curve: Option<CurveArgs>,
    kind: Option<String>,
    outline: Option<Vec<PointArgs>>,
    pattern: Option<String>,
    /// A style reference, or `null` for none: absent and null differ.
    #[serde(default, deserialize_with = "present")]
    boundary: Option<serde_json::Value>,
    recipe: Option<String>,
    position: Option<PointArgs>,
    from: Option<PointArgs>,
    to: Option<PointArgs>,
    at: Option<PointArgs>,
    rotation: Option<String>,
    spacing: Option<String>,
}

/// A field that is there, null or not: `Some(Null)` for `null`, where plain `Option` would give
/// `None` for it (a boundary of none and no boundary at all are different answers).
fn present<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<serde_json::Value>, D::Error> {
    serde_json::Value::deserialize(deserializer).map(Some)
}

/// A point as a person sends it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PointArgs {
    x: String,
    y: String,
}

/// A curve as a person sends it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CurveArgs {
    kind: String,
    centre: Option<PointArgs>,
    start: Option<PointArgs>,
    end: Option<PointArgs>,
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

fn words(error: &DetailError) -> String {
    error.to_string()
}

fn length(text: &str) -> Result<Length, String> {
    crate::cloud::hosting::length("Detail length", text)
}

fn point_of(args: &PointArgs) -> Result<Point, String> {
    Ok(Point::new(length(&args.x)?, length(&args.y)?))
}

fn points_of(args: &[PointArgs]) -> Result<Vec<Point>, String> {
    args.iter().map(point_of).collect()
}

/// A typed angle, read as construct-params reads an angle parameter's value: any angle.
fn angle(typed: &str, what: &str) -> Result<Angle, String> {
    let id = Id::parse("detail-angle").map_err(|e| e.to_string())?;
    let definition =
        Definition::new(id, what, Kind::Angle, Holder::Instance).map_err(|e| e.to_string())?;
    match definition.parse(typed, &Units::default()) {
        Ok(Value::Angle(angle)) => Ok(angle),
        Ok(_) => Err(format!("{what} takes an angle")),
        Err(e) => Err(e.to_string()),
    }
}

fn curve_of(args: &CurveArgs) -> Result<Curve, String> {
    let point = |p: Option<&PointArgs>, what: &str| point_of(need(p, what)?);
    match args.kind.as_str() {
        "straight" => {
            if args.centre.is_some() {
                return Err("A straight line takes a start and an end, and no centre.".to_owned());
            }
            Curve::straight(
                point(args.start.as_ref(), "a start")?,
                point(args.end.as_ref(), "an end")?,
            )
            .map_err(|e| words(&e))
        }
        "arc" => Curve::arc(
            point(args.centre.as_ref(), "a centre")?,
            point(args.start.as_ref(), "a start")?,
            point(args.end.as_ref(), "an end")?,
        )
        .map_err(|e| words(&e)),
        other => Err(format!("{other:?} is not a curve: straight or arc.")),
    }
}

// ---------------------------------------------------------------------------------------------
// The document's items, and what a page reads.

/// A document as the detail commands read it: its items, its views and each record's hash.
pub struct World {
    detailing: Detailing,
    views: construct_views::Views,
    library: Library,
    lines: LineStyles,
    fills: FillPatterns,
    all: BTreeMap<Id, (Id, Element)>,
}

impl World {
    /// The document's detail items.
    #[must_use]
    pub fn detailing(&self) -> &Detailing {
        &self.detailing
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

/// A document's detail items and views as they are now. The inner error is a record that is not as
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
    let library = match Library::built_in() {
        Ok(library) => library,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let detailing = match Detailing::from_records(found.iter().copied(), &library) {
        Ok(found) => found,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let views = match construct_views::Views::from_records(found.iter().copied()) {
        Ok(views) => views,
        Err(e) => return Ok(Err(e.to_string())),
    };
    Ok(Ok(World {
        detailing,
        views,
        library,
        lines: LineStyles::default_metric(),
        fills: FillPatterns::default_metric(),
        all,
    }))
}

fn stored(length: Length) -> String {
    Value::Length(length).stored()
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
        x: stored(point.x),
        y: stored(point.y),
    }
}

/// A line style as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StyleView {
    /// How an item names it: `builtin:standard` or the style's ID.
    pub style: String,
    /// Its name.
    pub name: String,
    /// Whether it is one of the six built in.
    pub built_in: bool,
    /// Its model pen's number.
    pub pen: u16,
    /// Its line pattern's number.
    pub line_pattern: u32,
    /// Its colour, `#rrggbb`.
    pub colour: String,
}

fn style_view(reference: &StyleRef, name: &str, made: LineStyle) -> StyleView {
    StyleView {
        style: reference.stored(),
        name: name.to_owned(),
        built_in: matches!(reference, StyleRef::Built(_)),
        pen: made.pen,
        line_pattern: made.pattern.0,
        colour: colour_spelled(made.colour),
    }
}

/// A detail line as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LineView {
    /// Its permanent ID.
    pub id: String,
    /// Its style, as an item names one.
    pub style: String,
    /// `straight` or `arc`.
    pub kind: String,
    /// A straight line's ends, or an arc's centre, start and end, in order.
    pub points: Vec<PointView>,
}

/// A region as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RegionView {
    /// Its permanent ID.
    pub id: String,
    /// `filled` or `masking`.
    pub kind: String,
    /// A filled region's fill pattern.
    pub pattern: Option<String>,
    /// The colour of its pattern's lines.
    pub colour: Option<String>,
    /// Its boundary's style, or none.
    pub boundary: Option<String>,
    /// Its corners, in order.
    pub outline: Vec<PointView>,
    /// Its area in stored area text.
    pub area: String,
    /// Its area as the person's units show it: `12000000 mm²`.
    pub area_text: String,
    /// Whether the area is the true one, to the square step.
    pub area_exact: bool,
}

/// A placed component as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ComponentView {
    /// Its permanent ID.
    pub id: String,
    /// The recipe's key.
    pub recipe: String,
    /// The recipe's name.
    pub recipe_name: String,
    /// Its origin.
    pub position: PointView,
    /// Its turn, a stored angle.
    pub rotation: String,
}

/// A repeating component as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RepeatView {
    /// Its permanent ID.
    pub id: String,
    /// The recipe's key.
    pub recipe: String,
    /// The recipe's name.
    pub recipe_name: String,
    /// The line's first point.
    pub from: PointView,
    /// The line's far end.
    pub to: PointView,
    /// The distance between copies, stored length text.
    pub spacing: String,
    /// The extra turn of every copy, a stored angle.
    pub rotation: String,
    /// How many copies it makes.
    pub count: u64,
    /// Where each copy's origin is.
    pub positions: Vec<PointView>,
}

/// A slope's reading as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReadingView {
    /// Rise over run as a fraction in lowest terms, `3/4`, when it is one.
    pub rise_over_run: Option<String>,
    /// The square of rise over run, `numerator/denominator`, exact.
    pub squared: String,
    /// The angle above the level, a stored angle.
    pub angle: String,
    /// The slope in words: `36.87 degrees, rise over run 3/4`.
    pub text: String,
    /// The way water runs, in plan, as a whole vector.
    pub fall: [i128; 2],
}

/// A warning as a page reads it (the shape construct-warnings gives every warning).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WarningView {
    /// Its kind's stable name.
    pub kind: String,
    /// `notice`, `warning` or `error`.
    pub severity: String,
    /// The elements involved: the item, then the element it reads.
    pub elements: Vec<String>,
    /// Its identity, `kind|id|id`.
    pub identity: String,
    /// A plain sentence.
    pub words: String,
}

/// A spot slope as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SlopeView {
    /// Its permanent ID.
    pub id: String,
    /// The floor or roof it reads.
    pub face: String,
    /// Where it reads.
    pub at: PointView,
    /// What the face's slope is there now, or `None` when it has none to read.
    pub reading: Option<ReadingView>,
    /// Why it has none, by name.
    pub warnings: Vec<WarningView>,
}

/// A detail component of the library as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RecipeView {
    /// Its key.
    pub key: String,
    /// Its name.
    pub name: String,
}

/// One view's detail items, as of the log's head.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DetailListing {
    /// The view.
    pub view: String,
    /// The view's name.
    pub view_name: String,
    /// The line styles a line or a boundary here may name: the six built in, then the document's
    /// own.
    pub styles: Vec<StyleView>,
    /// The components of the library.
    pub library: Vec<RecipeView>,
    /// Its detail lines, by ID.
    pub lines: Vec<LineView>,
    /// Its filled and masking regions, by ID.
    pub regions: Vec<RegionView>,
    /// Its placed components, by ID.
    pub components: Vec<ComponentView>,
    /// Its repeating components, by ID.
    pub repeats: Vec<RepeatView>,
    /// Its spot slopes, by ID.
    pub slopes: Vec<SlopeView>,
}

fn styles_view(world: &World) -> Vec<StyleView> {
    let mut found: Vec<StyleView> = BuiltIn::ALL
        .into_iter()
        .map(|b| style_view(&StyleRef::Built(b), b.key(), b.style()))
        .collect();
    found.extend(
        world
            .detailing
            .styles()
            .all()
            .into_iter()
            .map(|own| style_view(&StyleRef::Own(own.id().clone()), own.name(), own.style())),
    );
    found
}

fn name_of(library: &Library, key: &str) -> String {
    library
        .get(key)
        .map_or_else(|| key.to_owned(), |r| r.name().to_owned())
}

fn reading_view(reading: &Reading) -> ReadingView {
    ReadingView {
        rise_over_run: reading
            .rise_over_run
            .as_ref()
            .map(|r| format!("{}/{}", r.numer(), r.denom())),
        squared: format!("{}/{}", reading.squared.0, reading.squared.1),
        angle: Value::Angle(reading.angle).stored(),
        text: reading.text(),
        fall: reading.fall,
    }
}

fn slope_view(slope: &SpotSlope, faces: &Table) -> SlopeView {
    let (reading, warnings) = match slope.read(faces) {
        Ok(reading) => (Some(reading_view(&reading)), Vec::new()),
        Err(DetailError::Slope(refusal)) => {
            let kind = format!("detail.slope.{}", refusal.kind());
            let identity = format!("{kind}|{}|{}", slope.id(), slope.face());
            (
                None,
                vec![WarningView {
                    kind,
                    severity: "warning".to_owned(),
                    elements: vec![slope.id().to_string(), slope.face().to_string()],
                    identity,
                    words: refusal.to_string(),
                }],
            )
        }
        Err(other) => {
            let kind = "detail.slope.unreadable".to_owned();
            let identity = format!("{kind}|{}|{}", slope.id(), slope.face());
            (
                None,
                vec![WarningView {
                    kind,
                    severity: "warning".to_owned(),
                    elements: vec![slope.id().to_string(), slope.face().to_string()],
                    identity,
                    words: other.to_string(),
                }],
            )
        }
    };
    SlopeView {
        id: slope.id().to_string(),
        face: slope.face().to_string(),
        at: point_view(slope.at()),
        reading,
        warnings,
    }
}

fn region_view(region: &Region, units: &Units) -> Result<RegionView, String> {
    let area = region.area().map_err(|e| e.to_string())?;
    let definition = Definition::new(
        Id::parse("detail-area").map_err(|e| e.to_string())?,
        "Area",
        Kind::Area,
        Holder::Instance,
    )
    .map_err(|e| e.to_string())?;
    let value = Value::Area(area.area);
    let (pattern, colour) = match region.fill() {
        Fill::Pattern { pattern, colour } => (Some(pattern.clone()), Some(colour_spelled(*colour))),
        Fill::Masking => (None, None),
    };
    Ok(RegionView {
        id: region.id().to_string(),
        kind: if region.is_masking() {
            "masking"
        } else {
            "filled"
        }
        .to_owned(),
        pattern,
        colour,
        boundary: region.boundary().map(StyleRef::stored),
        outline: region.outline().iter().map(|p| point_view(*p)).collect(),
        area: value.stored(),
        area_text: definition
            .format(&value, units)
            .map_err(|e| e.to_string())?,
        area_exact: area.exact,
    })
}

/// A view's items as a page reads them, kind by kind.
struct Shown {
    lines: Vec<LineView>,
    regions: Vec<RegionView>,
    components: Vec<ComponentView>,
    repeats: Vec<RepeatView>,
    slopes: Vec<SlopeView>,
}

fn view_of_items(
    world: &World,
    items: &ViewItems<'_>,
    faces: &Table,
    units: &Units,
) -> Result<Shown, String> {
    let lines = items
        .lines
        .iter()
        .map(|l| LineView {
            id: l.id().to_string(),
            style: l.style().stored(),
            kind: if l.curve().is_arc() {
                "arc"
            } else {
                "straight"
            }
            .to_owned(),
            points: l.curve().points().into_iter().map(point_view).collect(),
        })
        .collect();
    let regions = items
        .regions
        .iter()
        .map(|r| region_view(r, units))
        .collect::<Result<Vec<_>, _>>()?;
    let components = items
        .components
        .iter()
        .map(|c| ComponentView {
            id: c.id().to_string(),
            recipe: c.recipe().to_owned(),
            recipe_name: name_of(&world.library, c.recipe()),
            position: point_view(c.position()),
            rotation: Value::Angle(c.rotation()).stored(),
        })
        .collect();
    let mut repeats = Vec::new();
    for r in &items.repeats {
        let positions = r.positions().map_err(|e| e.to_string())?;
        let (from, to) = r.line();
        repeats.push(RepeatView {
            id: r.id().to_string(),
            recipe: r.recipe().to_owned(),
            recipe_name: name_of(&world.library, r.recipe()),
            from: point_view(from),
            to: point_view(to),
            spacing: stored(r.spacing()),
            rotation: Value::Angle(r.rotation()).stored(),
            count: r.count().map_err(|e| e.to_string())?,
            positions: positions.into_iter().map(point_view).collect(),
        });
    }
    let slopes = items.slopes.iter().map(|s| slope_view(s, faces)).collect();
    Ok(Shown {
        lines,
        regions,
        components,
        repeats,
        slopes,
    })
}

/// The detail items of a view with their values worked out from the document as it is now. `None`
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
) -> Result<Result<Option<DetailListing>, String>, StoreError> {
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
    let items = world.detailing.of_view(view);
    let wanted: BTreeSet<Id> = items.slopes.iter().map(|s| s.face().clone()).collect();
    let faces = Table::read(reader, project, document, &wanted).await?;
    let Shown {
        lines,
        regions,
        components,
        repeats,
        slopes,
    } = match view_of_items(&world, &items, &faces, &units) {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    Ok(Ok(Some(DetailListing {
        view: view.to_string(),
        view_name: shown.name().to_owned(),
        styles: styles_view(&world),
        library: world
            .library
            .all()
            .iter()
            .map(|r| RecipeView {
                key: r.key().to_owned(),
                name: r.name().to_owned(),
            })
            .collect(),
        lines,
        regions,
        components,
        repeats,
        slopes,
    })))
}

// ---------------------------------------------------------------------------------------------
// The faces a spot slope reads: floors and roofs, as the records hold them.

/// What the records say of one floor or roof, in plan and grid steps.
enum Face {
    /// A floor: its outer loop and its holes, flat.
    Floor {
        outer: Vec<[i64; 2]>,
        holes: Vec<Vec<[i64; 2]>>,
    },
    /// A roof: the flat pieces of its top face.
    Roof { pieces: Vec<Vec<[i64; 3]>> },
    /// There is no such floor or roof, or it is not one: a whole sentence.
    Missing(String),
    /// A floor or roof that cannot be read now: the reason, to follow its ID.
    Unreadable(String),
}

/// The faces a set of spot slopes read, as the log's head has them: the table construct-detailing
/// asks ([`Faces`]).
pub struct Table(BTreeMap<Id, Face>);

fn corner_steps(x: i64, y: i64, shift: [i64; 2]) -> Option<[i64; 2]> {
    Some([x.checked_add(shift[0])?, y.checked_add(shift[1])?])
}

/// The shift a record's placement gives its frame, in steps.
fn shift_of(record: &Element) -> Result<[i64; 2], String> {
    let steps = |mm: i64| {
        Length::checked_from_mm(mm)
            .map(Length::steps)
            .ok_or_else(|| "its placement is too far from the origin".to_owned())
    };
    Ok([steps(record.placement.x)?, steps(record.placement.y)?])
}

fn floor_face(record: &Element) -> Face {
    let shift = match shift_of(record) {
        Ok(shift) => shift,
        Err(why) => return Face::Unreadable(why),
    };
    let boundary = match boundary_of(&record.recipe) {
        Ok(boundary) => boundary,
        Err(e) => return Face::Unreadable(e.to_string()),
    };
    let ring = |ring: Ring| -> Option<Vec<[i64; 2]>> {
        boundary
            .corners(ring)?
            .into_iter()
            .map(|(_, p)| corner_steps(p.x, p.y, shift))
            .collect()
    };
    let Some(outer) = ring(Ring::Outer) else {
        return Face::Unreadable("its boundary is too far from the origin".to_owned());
    };
    let holes: Option<Vec<Vec<[i64; 2]>>> = (0..boundary.holes().len())
        .map(|i| ring(Ring::Hole(i)))
        .collect();
    match holes {
        Some(holes) => Face::Floor { outer, holes },
        None => Face::Unreadable("its boundary is too far from the origin".to_owned()),
    }
}

async fn roof_face<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    record: &Element,
) -> Result<Face, StoreError> {
    let shift = match shift_of(record) {
        Ok(shift) => shift,
        Err(why) => return Ok(Face::Unreadable(why)),
    };
    let context = match records::read_context(reader, project, document).await? {
        Ok(context) => context,
        Err(why) => return Ok(Face::Unreadable(why)),
    };
    let Some(type_id) = records::type_of(record) else {
        return Ok(Face::Unreadable(
            "it names no roof type, so its layers cannot be read".to_owned(),
        ));
    };
    let ty = match records::read_type(reader, project, document, &type_id, &context).await? {
        Ok(ty) => ty,
        Err(why) => return Ok(Face::Unreadable(why)),
    };
    let built = layers_of(&ty)
        .and_then(|layers| roof_of(&record.recipe).and_then(|roof| shape(&roof, &layers)));
    let shape = match built {
        Ok(shape) => shape,
        Err(e) => return Ok(Face::Unreadable(e.to_string())),
    };
    let mut pieces = Vec::new();
    for plane in &shape.planes {
        for piece in &plane.pieces {
            let moved: Option<Vec<[i64; 3]>> = piece
                .iter()
                .map(|c| {
                    Some([
                        c[0].checked_add(shift[0])?,
                        c[1].checked_add(shift[1])?,
                        c[2],
                    ])
                })
                .collect();
            match moved {
                Some(moved) => pieces.push(moved),
                None => {
                    return Ok(Face::Unreadable(
                        "its roof is too far from the origin".to_owned(),
                    ));
                }
            }
        }
    }
    Ok(Face::Roof { pieces })
}

impl Table {
    /// The faces of the elements `wanted`, as the document has them now.
    ///
    /// # Errors
    /// A [`StoreError`] when the store fails.
    pub async fn read<R: Reader>(
        reader: &mut R,
        project: &Id,
        document: &Id,
        wanted: &BTreeSet<Id>,
    ) -> Result<Self, StoreError> {
        let mut found = BTreeMap::new();
        for id in wanted {
            let face = match records::read_element(reader, project, document, id).await? {
                None => Face::Missing(format!("There is no floor or roof {id} in this model.")),
                Some((_, record)) if is_floor(&record) => floor_face(&record),
                Some((_, record)) if is_roof(&record) => {
                    roof_face(reader, project, document, &record).await?
                }
                Some((_, record)) => Face::Missing(format!(
                    "{id} is a {}: a spot slope reads a floor or a roof.",
                    record.kind
                )),
            };
            found.insert(id.clone(), face);
        }
        Ok(Self(found))
    }
}

/// The plane through the first three corners of a piece that are not on one line.
fn plane_of(piece: &[[i64; 3]]) -> Result<Plane, SlopeRefusal> {
    let first = piece.first().copied();
    let Some(origin) = first else {
        return Err(SlopeRefusal::Missing(
            "The roof has an empty face.".to_owned(),
        ));
    };
    for (i, along) in piece.iter().enumerate().skip(1) {
        for across in piece.iter().skip(i.saturating_add(1)) {
            if let Ok(plane) = Plane::through_points(origin, *along, *across) {
                return Ok(plane);
            }
        }
    }
    Err(SlopeRefusal::Missing(
        "The roof has a face whose corners are on one line.".to_owned(),
    ))
}

impl Faces for Table {
    fn plane_at(&self, face: &Id, at: Point) -> Result<Plane, SlopeRefusal> {
        let point = [at.x.steps(), at.y.steps()];
        match self.0.get(face) {
            None => Err(SlopeRefusal::Missing(format!(
                "There is no floor or roof {face} in this model."
            ))),
            Some(Face::Missing(why)) => Err(SlopeRefusal::Missing(why.clone())),
            Some(Face::Unreadable(why)) => Err(SlopeRefusal::Missing(format!("{face}: {why}"))),
            Some(Face::Floor { outer, holes }) => {
                let inside = construct_detailing::polygon_covers(outer, point)
                    && !holes
                        .iter()
                        .any(|hole| construct_detailing::polygon_covers_inside(hole, point));
                if inside {
                    Ok(Plane {
                        through: [point[0], point[1], 0],
                        normal: [0, 0, 1],
                    })
                } else {
                    Err(SlopeRefusal::OffFace(format!(
                        "The point is not over the floor {face}."
                    )))
                }
            }
            Some(Face::Roof { pieces }) => {
                for piece in pieces {
                    let plan: Vec<[i64; 2]> = piece.iter().map(|c| [c[0], c[1]]).collect();
                    if construct_detailing::polygon_covers(&plan, point) {
                        return plane_of(piece);
                    }
                }
                Err(SlopeRefusal::OffFace(format!(
                    "The point is not over the roof {face}."
                )))
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// What a drawing of a view needs.

/// A view's detail items, and the faces its spot slopes read, as the log's head has them: what the
/// server's drawing of the view lays over the model (construct-detailing's `render`).
pub struct Prepared {
    world: World,
    view: Id,
    faces: Table,
}

impl Prepared {
    /// The items of the view.
    #[must_use]
    pub fn items(&self) -> ViewItems<'_> {
        self.world.detailing.of_view(&self.view)
    }

    /// The document's own line styles.
    #[must_use]
    pub fn own_styles(&self) -> &OwnStyles {
        self.world.detailing.styles()
    }

    /// The components of the library.
    #[must_use]
    pub fn library(&self) -> &Library {
        &self.world.library
    }

    /// The faces a spot slope reads.
    #[must_use]
    pub fn faces(&self) -> &dyn Faces {
        &self.faces
    }
}

/// The detail items of `view`, ready to be drawn. The inner error is a record that does not read.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn prepare<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    view: &Id,
) -> Result<Result<Prepared, String>, StoreError> {
    let world = match read_world(reader, project, document).await? {
        Ok(world) => world,
        Err(why) => return Ok(Err(why)),
    };
    let wanted: BTreeSet<Id> = world
        .detailing
        .of_view(view)
        .slopes
        .iter()
        .map(|s| s.face().clone())
        .collect();
    let faces = Table::read(reader, project, document, &wanted).await?;
    Ok(Ok(Prepared {
        world,
        view: view.clone(),
        faces,
    }))
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
        "detail.style.create" => style_create(&mut run, &world, args).await,
        "detail.style.set" => style_set(&mut run, &world, args).await,
        "detail.style.delete" => style_delete(&mut run, &world, args).await,
        "detail.line.create" => line_create(&mut run, &world, args).await,
        "detail.line.points" => line_points(&mut run, &world, args).await,
        "detail.line.style" => line_style(&mut run, &world, args).await,
        "detail.region.create" => region_create(&mut run, &world, args).await,
        "detail.region.points" => region_points(&mut run, &world, args).await,
        "detail.region.style" => region_style(&mut run, &world, args).await,
        "detail.region.fill" => region_fill(&mut run, &world, args).await,
        "detail.component.create" => component_create(&mut run, &world, args).await,
        "detail.component.move" => component_move(&mut run, &world, args).await,
        "detail.repeat.create" => repeat_create(&mut run, &world, args).await,
        "detail.repeat.line" => repeat_line(&mut run, &world, args).await,
        "detail.repeat.spacing" => repeat_spacing(&mut run, &world, args).await,
        "detail.slope.create" => slope_create(&mut run, &world, args).await,
        "detail.slope.move" => slope_move(&mut run, &world, args).await,
        "detail.line.delete"
        | "detail.region.delete"
        | "detail.component.delete"
        | "detail.repeat.delete"
        | "detail.slope.delete" => delete(&mut run, &world, args, &edit.cmd).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// Why a view cannot hold an item: it is not there.
fn view_refusal(world: &World, view: &Id) -> Result<(), String> {
    if world.views.get(view).is_some() {
        Ok(())
    } else {
        Err(format!("There is no view {view} in this model."))
    }
}

/// Why a view cannot hold a spot slope: it is not a plan (the slope's point is in plan).
fn plan_refusal(world: &World, view: &Id) -> Result<(), String> {
    view_refusal(world, view)?;
    match world.views.get(view) {
        Some(found) if found.kind().plan().is_none() => Err(format!(
            "The view {view} is a {}: a spot slope is read at a point in plan, so it is made in a plan.",
            found.kind().key()
        )),
        _ => Ok(()),
    }
}

/// An ID that is free for a new item.
fn free(world: &World, id: &Id) -> Result<(), String> {
    if world.record(id).is_some() {
        Err("That element already exists.".to_owned())
    } else {
        Ok(())
    }
}

/// A style as an item names it: `builtin:...` or an own style that is there.
fn style_ref(world: &World, text: &str) -> Result<StyleRef, String> {
    let found = StyleRef::from_stored(text).map_err(|e| words(&e))?;
    if let StyleRef::Own(id) = &found
        && world.detailing.styles().get(id).is_none()
    {
        return Err(words(&DetailError::UnknownStyle(id.to_string())));
    }
    Ok(found)
}

fn line_style_of(world: &World, pen: u16, pattern: u32, colour: &str) -> Result<LineStyle, String> {
    let made = LineStyle::new(
        pen,
        PatternId(pattern),
        colour_of(colour).map_err(|e| words(&e))?,
    );
    made.check(&world.lines).map_err(|e| words(&e))?;
    Ok(made)
}

/// Writes `record` as a new piece.
async fn create_record<T: Txn>(run: &mut Run<'_, T>, record: Result<Element, String>) -> Answer {
    match record {
        Ok(record) => {
            run.write_all(vec![Write::Put(Box::new(record), None)])
                .await
        }
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// Writes `after` over the piece with hash `hash`, or nothing at all when it is the same.
async fn replace_record<T: Txn>(
    run: &mut Run<'_, T>,
    world: &World,
    after: Result<Element, String>,
) -> Answer {
    let after = match after {
        Ok(after) => after,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some((hash, before)) = world.record(&after.id) else {
        return Ok(Err(DetailError::Unknown(after.id).to_string()));
    };
    if *before == after {
        return Ok(Ok(Applied::default()));
    }
    run.write_all(vec![Write::Put(Box::new(after), Some(hash.clone()))])
        .await
}

fn found<'a, T>(item: Option<&'a T>, id: &Id) -> Result<&'a T, String> {
    item.ok_or_else(|| DetailError::Unknown(id.clone()).to_string())
}

// -- styles

async fn style_create<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(
            args.style.as_deref().and_then(|s| Id::parse(s).ok()),
            "a style (the new ID)",
        )?;
        free(world, &id)?;
        let made = line_style_of(
            world,
            need(args.pen, "a pen")?,
            need(args.line_pattern, "a line pattern")?,
            &need(args.colour.clone(), "a colour")?,
        )?;
        let style =
            OwnStyle::new(id, &need(args.name.clone(), "a name")?, made).map_err(|e| words(&e))?;
        if world.detailing.styles().name_taken(style.name(), None) {
            return Err(format!(
                "There is already a line style named {:?}.",
                style.name()
            ));
        }
        Ok(style.to_record(run.document))
    })();
    create_record(run, record).await
}

async fn style_set<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(
            args.style.as_deref().and_then(|s| Id::parse(s).ok()),
            "a style (its ID)",
        )?;
        let old = found(world.detailing.styles().get(&id), &id)?;
        let made = line_style_of(
            world,
            args.pen.unwrap_or(old.style().pen),
            args.line_pattern.unwrap_or(old.style().pattern.0),
            &args
                .colour
                .clone()
                .unwrap_or_else(|| colour_spelled(old.style().colour)),
        )?;
        let name = args.name.clone().unwrap_or_else(|| old.name().to_owned());
        if world.detailing.styles().name_taken(&name, Some(&id)) {
            return Err(format!("There is already a line style named {name:?}."));
        }
        let changed = old.clone().with(&name, made).map_err(|e| words(&e))?;
        Ok(changed.to_record(run.document))
    })();
    replace_record(run, world, record).await
}

async fn style_delete<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let gone = (|| {
        let id = need(
            args.style.as_deref().and_then(|s| Id::parse(s).ok()),
            "a style (its ID)",
        )?;
        found(world.detailing.styles().get(&id), &id)?;
        let users = world.detailing.using_style(&StyleRef::Own(id.clone()));
        if let Some(first) = users.first() {
            return Err(format!(
                "The style is used by {} item(s), such as {first}: give them another style first.",
                users.len()
            ));
        }
        let (hash, _) = world
            .record(&id)
            .ok_or_else(|| DetailError::Unknown(id.clone()).to_string())?;
        Ok(Write::Remove(id, hash.clone()))
    })();
    match gone {
        Ok(write) => run.write_all(vec![write]).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

// -- lines

async fn line_create<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(args.line.clone(), "a line (the new ID)")?;
        free(world, &id)?;
        let view = need(args.view.clone(), "a view")?;
        view_refusal(world, &view)?;
        let style = match &args.style {
            Some(text) => style_ref(world, text)?,
            None => StyleRef::Built(BuiltIn::Standard),
        };
        let curve = curve_of(need(args.curve.as_ref(), "a curve")?)?;
        Ok(DetailLine::new(id, view, style, curve).to_record(run.document))
    })();
    create_record(run, record).await
}

async fn line_points<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(args.line.clone(), "a line")?;
        let old = found(world.detailing.line(&id), &id)?;
        let curve = curve_of(need(args.curve.as_ref(), "a curve")?)?;
        Ok(old.clone().with_curve(curve).to_record(run.document))
    })();
    replace_record(run, world, record).await
}

async fn line_style<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(args.line.clone(), "a line")?;
        let old = found(world.detailing.line(&id), &id)?;
        let style = style_ref(world, &need(args.style.clone(), "a style")?)?;
        Ok(old.clone().with_style(style).to_record(run.document))
    })();
    replace_record(run, world, record).await
}

// -- regions

fn boundary_of_args(world: &World, value: &serde_json::Value) -> Result<Option<StyleRef>, String> {
    match value {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(text) => style_ref(world, text).map(Some),
        _ => Err("A boundary is a style, or null for none.".to_owned()),
    }
}

fn fill_of(world: &World, pattern: Option<&str>, colour: Option<&str>) -> Result<Fill, String> {
    let pattern = need(pattern, "a pattern")?;
    if world.fills.get(pattern).is_none() {
        return Err(words(&DetailError::UnknownPattern(pattern.to_owned())));
    }
    Ok(Fill::Pattern {
        pattern: pattern.to_owned(),
        colour: colour_of(need(colour, "a colour")?).map_err(|e| words(&e))?,
    })
}

async fn region_create<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(args.region.clone(), "a region (the new ID)")?;
        free(world, &id)?;
        let view = need(args.view.clone(), "a view")?;
        view_refusal(world, &view)?;
        let fill = match need(args.kind.as_deref(), "a kind (filled or masking)")? {
            "filled" => fill_of(world, args.pattern.as_deref(), args.colour.as_deref())?,
            "masking" => {
                if args.pattern.is_some() || args.colour.is_some() {
                    return Err(
                        "A masking region is opaque and has no pattern or colour.".to_owned()
                    );
                }
                Fill::Masking
            }
            other => {
                return Err(format!(
                    "{other:?} is not a kind of region: filled or masking."
                ));
            }
        };
        let boundary = match &args.boundary {
            Some(value) => boundary_of_args(world, value)?,
            None => Some(StyleRef::Built(BuiltIn::Standard)),
        };
        let outline = points_of(need(args.outline.as_deref(), "an outline")?)?;
        Ok(Region::new(id, view, fill, boundary, outline)
            .map_err(|e| words(&e))?
            .to_record(run.document))
    })();
    create_record(run, record).await
}

async fn region_points<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(args.region.clone(), "a region")?;
        let old = found(world.detailing.region(&id), &id)?;
        let outline = points_of(need(args.outline.as_deref(), "an outline")?)?;
        Ok(old
            .clone()
            .with_outline(outline)
            .map_err(|e| words(&e))?
            .to_record(run.document))
    })();
    replace_record(run, world, record).await
}

async fn region_style<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(args.region.clone(), "a region")?;
        let old = found(world.detailing.region(&id), &id)?;
        let boundary = boundary_of_args(
            world,
            need(args.boundary.as_ref(), "a boundary (a style, or null)")?,
        )?;
        Ok(old.clone().with_boundary(boundary).to_record(run.document))
    })();
    replace_record(run, world, record).await
}

async fn region_fill<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(args.region.clone(), "a region")?;
        let old = found(world.detailing.region(&id), &id)?;
        if old.is_masking() {
            return Err("A masking region is opaque and has no pattern.".to_owned());
        }
        let fill = fill_of(world, args.pattern.as_deref(), args.colour.as_deref())?;
        Ok(old
            .clone()
            .with_fill(fill)
            .map_err(|e| words(&e))?
            .to_record(run.document))
    })();
    replace_record(run, world, record).await
}

// -- components and repeats

fn rotation_of(args: &Args, otherwise: Angle) -> Result<Angle, String> {
    args.rotation
        .as_deref()
        .map_or(Ok(otherwise), |t| angle(t, "The rotation"))
}

async fn component_create<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(args.component.clone(), "a component (the new ID)")?;
        free(world, &id)?;
        let view = need(args.view.clone(), "a view")?;
        view_refusal(world, &view)?;
        let recipe = need(args.recipe.as_deref(), "a recipe")?;
        let position = point_of(need(args.position.as_ref(), "a position")?)?;
        let turn = rotation_of(&args, Angle::from_units(0))?;
        Ok(
            DetailComponent::new(id, view, &world.library, recipe, position, turn)
                .map_err(|e| words(&e))?
                .to_record(run.document),
        )
    })();
    create_record(run, record).await
}

async fn component_move<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(args.component.clone(), "a component")?;
        let old = found(world.detailing.component(&id), &id)?;
        let position = match &args.position {
            Some(p) => point_of(p)?,
            None => old.position(),
        };
        let turn = rotation_of(&args, old.rotation())?;
        if args.position.is_none() && args.rotation.is_none() {
            return Err("This command needs a position or a rotation.".to_owned());
        }
        Ok(old
            .clone()
            .with_placement(&world.library, position, turn)
            .map_err(|e| words(&e))?
            .to_record(run.document))
    })();
    replace_record(run, world, record).await
}

async fn repeat_create<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(args.repeat.clone(), "a repeat (the new ID)")?;
        free(world, &id)?;
        let view = need(args.view.clone(), "a view")?;
        view_refusal(world, &view)?;
        let recipe = need(args.recipe.as_deref(), "a recipe")?;
        let line = (
            point_of(need(args.from.as_ref(), "a from point")?)?,
            point_of(need(args.to.as_ref(), "a to point")?)?,
        );
        let spacing = length(&need(args.spacing.clone(), "a spacing")?)?;
        let turn = rotation_of(&args, Angle::from_units(0))?;
        Ok(
            DetailRepeat::new(id, view, &world.library, recipe, line, spacing, turn)
                .map_err(|e| words(&e))?
                .to_record(run.document),
        )
    })();
    create_record(run, record).await
}

async fn repeat_line<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(args.repeat.clone(), "a repeat")?;
        let old = found(world.detailing.repeat(&id), &id)?;
        let line = (
            point_of(need(args.from.as_ref(), "a from point")?)?,
            point_of(need(args.to.as_ref(), "a to point")?)?,
        );
        Ok(old
            .clone()
            .with_line(&world.library, line)
            .map_err(|e| words(&e))?
            .to_record(run.document))
    })();
    replace_record(run, world, record).await
}

async fn repeat_spacing<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let record = (|| {
        let id = need(args.repeat.clone(), "a repeat")?;
        let old = found(world.detailing.repeat(&id), &id)?;
        if args.spacing.is_none() && args.rotation.is_none() {
            return Err("This command needs a spacing or a rotation.".to_owned());
        }
        let spacing = match &args.spacing {
            Some(text) => length(text)?,
            None => old.spacing(),
        };
        let turn = rotation_of(&args, old.rotation())?;
        Ok(old
            .clone()
            .with_spacing(&world.library, spacing, turn)
            .map_err(|e| words(&e))?
            .to_record(run.document))
    })();
    replace_record(run, world, record).await
}

// -- slopes

/// The refusal a slope earns now: its face read at its point.
async fn slope_reads<T: Txn>(
    run: &mut Run<'_, T>,
    slope: &SpotSlope,
) -> Result<Result<(), String>, StoreError> {
    let wanted = BTreeSet::from([slope.face().clone()]);
    let faces = Table::read(run.txn, run.project, run.document, &wanted).await?;
    Ok(slope.read(&faces).map(|_| ()).map_err(|e| words(&e)))
}

async fn slope_create<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let made = (|| {
        let id = need(args.slope.clone(), "a slope (the new ID)")?;
        free(world, &id)?;
        let view = need(args.view.clone(), "a view")?;
        plan_refusal(world, &view)?;
        let face = need(args.face.clone(), "a face (a floor or a roof)")?;
        let at = point_of(need(args.at.as_ref(), "a point")?)?;
        SpotSlope::new(id, view, face, at).map_err(|e| words(&e))
    })();
    let slope = match made {
        Ok(slope) => slope,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(refusal) = slope_reads(run, &slope).await? {
        return Ok(Err(refusal));
    }
    create_record(run, Ok(slope.to_record(run.document))).await
}

async fn slope_move<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let made = (|| {
        let id = need(args.slope.clone(), "a slope")?;
        let old = found(world.detailing.slope(&id), &id)?;
        let at = point_of(need(args.at.as_ref(), "a point")?)?;
        old.clone().with_point(at).map_err(|e| words(&e))
    })();
    let slope = match made {
        Ok(slope) => slope,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(refusal) = slope_reads(run, &slope).await? {
        return Ok(Err(refusal));
    }
    replace_record(run, world, Ok(slope.to_record(run.document))).await
}

// -- delete

async fn delete<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args, cmd: &str) -> Answer {
    let (id, kinds): (Option<Id>, &[ItemKind]) = match cmd {
        "detail.line.delete" => (args.line, &[ItemKind::Line]),
        "detail.region.delete" => (args.region, &[ItemKind::Region, ItemKind::Masking]),
        "detail.component.delete" => (args.component, &[ItemKind::Component]),
        "detail.repeat.delete" => (args.repeat, &[ItemKind::Repeat]),
        _ => (args.slope, &[ItemKind::Slope]),
    };
    let Some(id) = id else {
        return Ok(Err("This command needs the item to delete.".to_owned()));
    };
    match world.detailing.kind_of(&id) {
        Some(kind) if kinds.contains(&kind) => {}
        _ => return Ok(Err(DetailError::Unknown(id).to_string())),
    }
    let Some((hash, _)) = world.record(&id) else {
        return Ok(Err(DetailError::Unknown(id).to_string()));
    };
    run.write_all(vec![Write::Remove(id.clone(), hash.clone())])
        .await
}

/// Why a view cannot be deleted: it holds detail items. `None` when it holds none (or no detail
/// record reads, which the detail commands report).
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
    let held = world.detailing.of_view(view);
    let first = [
        held.lines.first().map(|i| i.id()),
        held.regions.first().map(|i| i.id()),
        held.components.first().map(|i| i.id()),
        held.repeats.first().map(|i| i.id()),
        held.slopes.first().map(|i| i.id()),
    ]
    .into_iter()
    .flatten()
    .next();
    Ok(first.map(|first| {
        format!(
            "The view holds {} detail item(s), such as {first}; delete them with the detail commands first.",
            held.len()
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

/// The generic commands do not go behind the detail commands: `element.create` of a detail item
/// and `element.set`, `element.move` or `element.delete` of one are refused, naming the commands to
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
        .is_some_and(|recipe| is_detail_kind(&records::kind_name(recipe)));
    let by_kind = args.kind.as_deref().is_some_and(is_detail_kind);
    Ok(match edit.cmd.as_str() {
        "element.create" if by_recipe || by_kind => Err(format!(
            "A detail item is made: use the detail commands. {BY_DETAIL_COMMANDS}"
        )),
        "element.set" | "element.move" | "element.delete" => {
            let Some(id) = &args.element else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, &edit.target, id).await? {
                Some((_, record)) if is_detail_kind(&record.kind) => Err(match edit.cmd.as_str() {
                    "element.delete" => {
                        "A detail item is deleted: use detail.line.delete, detail.region.delete, \
                             detail.component.delete, detail.repeat.delete, detail.slope.delete or \
                             detail.style.delete."
                            .to_owned()
                    }
                    _ => "A detail item is changed by the detail commands (detail.line.points, \
                              detail.region.points, detail.component.move and the rest)."
                        .to_owned(),
                }),
                Some(_) if by_recipe => Err(format!(
                    "A detail item is made: use the detail commands. {BY_DETAIL_COMMANDS}"
                )),
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    })
}

/// Whether the detail items an undo wrote still stand: every record reads, every item is in a view
/// that is there, every style an item names is there and every spot slope is in a plan. The whole
/// document is checked whenever a detail record or a view was written or taken away.
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
    let touched = written.iter().any(|record| {
        is_detail_kind(&record.kind) || record.kind == "view" || record.kind == TYPE_KIND
    });
    // A piece taken away may be a view or an item: its kind is not known any more.
    if !touched && deleted.is_empty() {
        return Ok(Ok(()));
    }
    let world = match read_world(txn, project, document).await? {
        Ok(world) => world,
        Err(why) => return Ok(Err(why)),
    };
    for record in world.all.values().map(|(_, record)| record) {
        if !is_detail_kind(&record.kind) || record.kind == "detail.linestyle" {
            continue;
        }
        let Some(view) = record
            .props
            .get(construct_detailing::VIEW_KEY)
            .and_then(|text| Id::parse(text).ok())
        else {
            continue;
        };
        if let Err(why) = view_refusal(&world, &view) {
            return Ok(Err(format!(
                "the detail item {} would be left in a view that is not there: {why}",
                record.id
            )));
        }
        if record.kind == SLOPE_KIND
            && let Err(why) = plan_refusal(&world, &view)
        {
            return Ok(Err(format!(
                "the spot slope {} would be left in a view that is not a plan: {why}",
                record.id
            )));
        }
    }
    for line in world
        .all
        .values()
        .filter_map(|(_, r)| (r.kind == "detail.line").then_some(r))
    {
        if let Ok(item) = DetailLine::from_record(line)
            && let Err(why) = style_ref(&world, &item.style().stored())
        {
            return Ok(Err(format!(
                "the detail line {} would be left with a style that is not there: {why}",
                line.id
            )));
        }
    }
    Ok(Ok(()))
}

/// Reads a floor's or roof's slope at a point, as a spot slope there would: for tests and tools
/// that ask the records without making an item.
///
/// # Errors
/// A [`StoreError`] when the store fails; the inner error is the refusal, by name.
pub async fn slope_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    face: &Id,
    at: Point,
) -> Result<Result<Reading, SlopeRefusal>, StoreError> {
    let wanted = BTreeSet::from([face.clone()]);
    let faces = Table::read(reader, project, document, &wanted).await?;
    Ok(faces.plane_at(face, at).and_then(|plane| read(&plane)))
}
