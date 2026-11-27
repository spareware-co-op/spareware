//! The commands that make and change a document's views (CON-A8-F1-P1, CON-A8-F1-P2). They go
//! through the same edit log as `element.*`, with the same access check (`model.edit`) and the
//! same claims: a piece another person is working on is refused, and one person's edits land in
//! log order.
//!
//! A view is an element record of the views category with no shape ([`construct_views::View`]).
//! Each command reads the document's levels and views, makes the change with construct-views'
//! own types, checks the result with [`construct_views::View::check`] and the name rule of
//! [`construct_views::Views`], and writes the one record it changed. A refusal is the command's
//! result, in construct-views' words, and changes nothing. Nothing here reads a clock or a random
//! number.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `view.create` | `view` (the new ID), `kind`, `name`; a plan (`floor_plan`, `ceiling_plan`): `level`, optional `range`; a `section` or `elevation`: `from`, `to`, `look`, `depth`; optional `scale`, `detail`, `staging`, `phasing` | A view record |
//! | `view.rename` | `view`, `name` | The view. A name is unique among the document's views, case aside |
//! | `view.set` | `view`, and any of `scale`, `detail`, `level` and `range` (a plan), `from`, `to`, `look` and `depth` (a section or an elevation), `staging`, `phasing` (`null` for none) | The view; nothing at all when it already is so |
//! | `view.duplicate` | `view`, `new` (the new ID), optional `name` | A view like the first, named `<name> copy` (then `copy 2`...) unless a name is given |
//! | `view.delete` | `view` | Its pointer goes; refused while the view is placed on a sheet |
//! | `view.graphics.set` | `view`, `category`, `edge` (`cut` or `projection`), `style` | One graphics rule of the view |
//! | `view.graphics.clear` | `view`, `category`, optional `edge` | The rule, or both of the category's rules; nothing at all when there is none |
//! | `view.crop.set` | `view`, and any of `region`, `annotation` (`null` for none), `on` | The view's crop: made from `region` when the view has none, else the named parts change |
//! | `view.crop.clear` | `view` | The crop goes; nothing at all when there is none |
//! | `view.crop.show` | `view`, `shown` (true or false) | Whether the crop's outline is shown in the view |
//! | `view.line.move` | `view`, `by` (`{"x", "y"}`, typed lengths) | A section's or elevation's line, both ends moved by `by`; nothing at all when `by` is nothing |
//! | `view.line.flip` | `view` | A section's or elevation's line looking to its other side |
//! | `view.elevation.create` | `view`, `name`, `face` (`north`, `south`, `east` or `west`), `from` (`{"x", "y"}`), `width`, `depth`; optional `scale`, `detail`, `staging`, `phasing` | An elevation of that face of the building: its line `width` long from `from`, square to the face's direction, looking at the face |
//!
//! A section is made by its line with `view.create` (`from`, `to`, `look`, `depth`), its depth
//! is set with `view.set`, and it is moved with `view.line.move` or turned to look the other way
//! with `view.line.flip`; both read the line as it is when they run, so two people's moves add up
//! in the log's order. An elevation by direction (`view.elevation.create`) names the face it
//! shows, as an architect names elevations: the north elevation shows the building's north face,
//! seen from the north, looking south. Its line runs from `from`, its left end as one faces the
//! building, `width` across, and it looks to its left (at the building) as far as `depth`; its line
//! is along a grid axis, so it is drawn. A section's or an elevation's line not along a grid axis
//! is stored, and refused only when it is drawn.
//!
//! A crop (CON-A8-F1-P3) is a plan's, section's or elevation's window onto the model: a `region`
//! is `{"rectangle": {"min": {"x", "y"}, "max": {"x", "y"}}}` or `{"outline": [{"x", "y"}, ...]}`
//! (corners in order, typed lengths in the view's own coordinates), and the same for the
//! `annotation` region. A new crop is on and shown. It is refused for a view that holds none (a
//! three-dimensional view, a drafting view, a legend), for a rectangle whose minimum is not below
//! its maximum, and for an outline with fewer than three corners, a corner twice in a row, edges
//! that cross, touch or lie along each other, or no area (all checked exactly, in construct-views'
//! words); `view.crop.show` is refused for a view with no crop. The generic commands do not go
//! behind these, and no command takes the crop's parts but these. The server's listing gives the
//! crop and a plan's range with each view; a viewport's size on its sheet is the crop's when it is
//! on.
//!
//! The values: `kind` is one of `floor_plan`, `ceiling_plan`, `section`, `elevation`, `three_d`,
//! `drafting`, `legend`. `scale` is the `n` of 1:n, a whole number from 1 to 1,000,000 (default
//! [`DEFAULT_SCALE`]); `detail` is `coarse`, `medium` or `fine` (default medium). `level` is a
//! level's ID. A plan's `range` holds any of `top`, `cut`, `bottom`, `depth`, each
//! `{"level": <ID>, "offset": <typed text>}` with either part optional: a new plan has its four
//! heights on its own level at [`DEFAULT_RANGE_MM`], and a height not named keeps what it has.
//! `from` and `to` are `{"x": <typed text>, "y": <typed text>}`, `look` is `left` or `right`,
//! `depth` typed text above zero. `staging` is the phases format's `{"created", "demolished",
//! "design_option"}` and `phasing` its `{"phase", "filter", "choices"}`. Typed text is read with
//! construct-params' metric display settings until a project keeps its own: `3.2 m`, `-450`.
//!
//! A graphics rule (CON-A8-F3-P2) is how one category or subcategory looks in one view, for what
//! the view cuts (`cut`) or sees beyond the cut plane (`projection`). `category` is a category's
//! number, a subcategory's included. `style` holds any of `pen` (a pen's number: the line
//! weight), `pattern` (a line pattern's number), `colour` (`[red, green, blue]`, each 0 to 255),
//! `fill` (`{"pattern": <name>, "colour": [r, g, b]}`, or `null` for none; a cut only),
//! `halftone`, `transparency` (a whole percentage, 0 to 100) and `shown`. A part not named keeps
//! what the element looks like in that view now (the rule it has, else its category's, else the
//! project's object style), so a rule that only hides something names only `shown`. A rule is
//! refused for an unknown category, for the views category, for a cut of what cannot be cut, for
//! a pen, pattern or fill pattern there is not, for a value out of range, and for a view that
//! cannot hold one (a drafting view or a legend, and a cut rule in a three-dimensional view).
//! construct-views' `resolve` is how an element's look is worked out from the rules.
//!
//! A view's kind never changes: a plan is not made a section; a new view is made. An argument the
//! kind has no use for (a `level` for a section, a `range` for a legend) is refused, naming the
//! kind. A new plan's range is not asked for the level's own height: it is the level and an
//! offset, so it moves with the level (construct-views' FORMAT.md).
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a view
//! and `element.set`, `element.move` or `element.delete` of one are refused, naming the view
//! command to use. Views are not elements with a shape, so nothing builds them
//! (`construct_regen::parameters::has_shape`).
//!
//! Interim: a document stores no phases or design options yet, so the phases a view's `staging`
//! and `phasing` may name are the two every project starts with ([`phases`]), and no design option
//! exists to name. The job that stores phases through the log replaces [`phases`] and
//! [`options`] with the document's own, and no caller changes.

use crate::cloud::elements::write_record;
use construct_categories::{CategoryId, CategoryKind, CategoryTable, VIEWS};
use construct_levels::{Level, Levels, parse_length};
use construct_model::element::Element;
use construct_model::geometry::{Bounds, Recipe};
use construct_params::{Length, Units, Value};
use construct_phases::{DesignOptions, ElementStaging, PhaseList, ViewSettings};
use construct_regen::levels::read_levels;
use construct_regen::parameters as records;
use construct_styles::{Colour, Edge, LineStyles, ObjectStyles, PatternId};
use construct_views::{
    Crop, CutLine, DetailLevel, Fill, Model, PlanSetup, Point, Region, RuleStyle, Scale, Side, Tie,
    VIEW_KIND, View, ViewElement, ViewError, ViewKind, ViewRange, Views, edge_from_key, list,
    resolve,
};
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, Reader, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::{Deserialize, Serialize};

/// Every command here.
pub const COMMANDS: [&str; 13] = [
    "view.create",
    "view.rename",
    "view.set",
    "view.duplicate",
    "view.delete",
    "view.graphics.set",
    "view.graphics.clear",
    "view.crop.set",
    "view.crop.clear",
    "view.crop.show",
    "view.line.move",
    "view.line.flip",
    "view.elevation.create",
];

/// The scale of a view made without one: 1:100.
pub const DEFAULT_SCALE: u32 = 100;
/// The detail level of a view made without one.
pub const DEFAULT_DETAIL: DetailLevel = DetailLevel::Medium;
/// A new plan's range, in whole millimetres from its own level: top, cut plane, bottom, view
/// depth. The cut plane is about 1.2 m up, as drawing practice has it (construct-views, range).
pub const DEFAULT_RANGE_MM: [i32; 4] = [2300, 1200, 0, -500];
/// The phase an element made today is created in.
pub const NEW_PHASE: &str = "phase-new";
/// The phase of what stood before the work.
pub const EXISTING_PHASE: &str = "phase-existing";

/// The answer to a command that would make or change a view as an element.
pub const BY_VIEW_COMMANDS: &str = "Views are made and changed by the view commands.";

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
    view: Option<Id>,
    /// The ID a duplicate is made under.
    #[serde(default, rename = "new", with = "kit_ops::id::option")]
    copy: Option<Id>,
    kind: Option<String>,
    name: Option<String>,
    level: Option<String>,
    scale: Option<u32>,
    detail: Option<String>,
    range: Option<RangeArgs>,
    from: Option<PointArgs>,
    to: Option<PointArgs>,
    look: Option<String>,
    depth: Option<String>,
    staging: Option<ElementStaging>,
    /// The phase settings; `null` takes them away, so `null` is kept apart from none.
    #[serde(default, deserialize_with = "present")]
    phasing: Option<serde_json::Value>,
    /// A graphics rule's category, a subcategory's included.
    category: Option<i64>,
    /// A graphics rule's edge: `cut` or `projection`.
    edge: Option<String>,
    /// A graphics rule's style.
    style: Option<StyleArgs>,
    /// A crop's region.
    region: Option<RegionArgs>,
    /// A crop's annotation region; `null` takes it away, so `null` is kept apart from none.
    #[serde(default, deserialize_with = "present")]
    annotation: Option<serde_json::Value>,
    /// Whether a crop is on.
    on: Option<bool>,
    /// Whether a crop's outline is shown.
    shown: Option<bool>,
    /// How far a line is moved (`view.line.move`).
    by: Option<PointArgs>,
    /// The face of the building an elevation shows (`view.elevation.create`).
    face: Option<String>,
    /// How long an elevation's line is (`view.elevation.create`).
    width: Option<String>,
}

/// A crop region as a person names it.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum RegionArgs {
    /// The lower left and the upper right corner.
    Rectangle { min: PointArgs, max: PointArgs },
    /// The corners of a closed outline in order.
    Outline(Vec<PointArgs>),
}

/// The parts of a graphics rule's style a person names; the rest keep what they were.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct StyleArgs {
    pen: Option<i64>,
    pattern: Option<i64>,
    colour: Option<Vec<i64>>,
    /// The fill; `null` takes it away, so `null` is kept apart from none.
    #[serde(default, deserialize_with = "present")]
    fill: Option<serde_json::Value>,
    halftone: Option<bool>,
    transparency: Option<i64>,
    shown: Option<bool>,
}

/// A fill as a person names it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FillArgs {
    pattern: String,
    colour: Vec<i64>,
}

/// An argument that is there, `null` included.
fn present<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<serde_json::Value>, D::Error> {
    serde_json::Value::deserialize(deserializer).map(Some)
}

/// One height of a plan's range as a person sends it: either part may be left out.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct TieArgs {
    level: Option<String>,
    offset: Option<String>,
}

/// The heights of a plan's range a command names.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RangeArgs {
    top: Option<TieArgs>,
    cut: Option<TieArgs>,
    bottom: Option<TieArgs>,
    depth: Option<TieArgs>,
}

/// A point of the plan as typed text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PointArgs {
    x: String,
    y: String,
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

fn units() -> Units {
    Units::default()
}

fn length(text: &str) -> Result<Length, String> {
    parse_length(text, &units()).map_err(|e| e.to_string())
}

fn id_of(text: &str, what: &str) -> Result<Id, String> {
    Id::parse(text).map_err(|_| format!("{text:?} is not {what}'s ID."))
}

/// The phases every project starts with: what stood before the work, and the new work.
///
/// # Errors
/// The phases crate's own words, which the fixed list never gives.
pub fn phases() -> Result<PhaseList, String> {
    let id = |text: &str| Id::parse(text).map_err(|e| e.to_string());
    let (existing, new) = (id(EXISTING_PHASE)?, id(NEW_PHASE)?);
    PhaseList::new()
        .add(existing, "Existing")
        .and_then(|list| list.add(new, "New construction"))
        .map_err(|e| e.to_string())
}

/// The design options a project starts with: none.
#[must_use]
pub fn options() -> DesignOptions {
    DesignOptions::new()
}

/// A document as a view's rules read it: its levels, its views and the rest of what a view's
/// question reads.
pub struct World {
    levels: Levels,
    categories: CategoryTable,
    phases: PhaseList,
    options: DesignOptions,
    views: Views,
    lines: LineStyles,
    objects: ObjectStyles,
}

impl World {
    /// The parts of the document a view is checked against.
    #[must_use]
    pub fn model(&self) -> Model<'_> {
        Model {
            levels: &self.levels,
            categories: &self.categories,
            phases: &self.phases,
            options: &self.options,
        }
    }

    /// The document's views.
    #[must_use]
    pub fn views(&self) -> &Views {
        &self.views
    }

    /// The document's levels.
    #[must_use]
    pub fn levels(&self) -> &Levels {
        &self.levels
    }

    /// The categories views are drawn in.
    #[must_use]
    pub fn categories(&self) -> &CategoryTable {
        &self.categories
    }

    /// The pens and line patterns a rule is checked against.
    #[must_use]
    pub fn line_styles(&self) -> &LineStyles {
        &self.lines
    }

    /// The project's object styles: what a view with no rule for a category looks like.
    #[must_use]
    pub fn object_styles(&self) -> &ObjectStyles {
        &self.objects
    }

    fn check(&self, view: &View) -> Result<(), String> {
        view.check(&self.model()).map_err(|e| e.to_string())
    }
}

/// A document's levels and views as they are now. The inner error is a record that is not as
/// written, in words.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn read_world<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<World, String>, StoreError> {
    let (levels, all) = match read_levels(reader, project, document).await? {
        Ok(read) => read,
        Err(why) => return Ok(Err(why)),
    };
    let views = match Views::from_records(&all) {
        Ok(views) => views,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let phases = match phases() {
        Ok(phases) => phases,
        Err(why) => return Ok(Err(why)),
    };
    Ok(Ok(World {
        levels,
        categories: CategoryTable::new(),
        phases,
        options: options(),
        views,
        // Interim: a document stores no pens, patterns or object styles yet, so every document has
        // our defaults. The job that stores them through the log replaces these two lines.
        lines: LineStyles::default_metric(),
        objects: ObjectStyles::default_metric(),
    }))
}

/// A view as a page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ViewSummary {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its kind, as it is stored: `floor_plan`, `section`...
    pub kind: String,
    /// The level a plan is of, if it is one.
    pub level: Option<String>,
    /// That level's name.
    pub level_name: Option<String>,
    /// The `n` of 1:n.
    pub scale: u32,
    /// `coarse`, `medium` or `fine`.
    pub detail: String,
    /// A plan's view range, or `None` for any other view.
    pub range: Option<RangeSummary>,
    /// The view's crop, or `None` when it has none.
    pub crop: Option<CropSummary>,
}

/// One height of a view range as a page lists it: the level and the offset from it, in the text
/// a person reads (`1200 mm`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TieSummary {
    /// The level's ID.
    pub level: String,
    /// The offset from the level.
    pub offset: String,
}

/// The four heights of a view range above the project's zero, each as text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HeightsSummary {
    /// The top.
    pub top: String,
    /// The cut plane.
    pub cut: String,
    /// The bottom.
    pub bottom: String,
    /// The view depth.
    pub depth: String,
}

/// A plan's view range as a page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RangeSummary {
    /// The top.
    pub top: TieSummary,
    /// The cut plane.
    pub cut: TieSummary,
    /// The bottom.
    pub bottom: TieSummary,
    /// The view depth.
    pub depth: TieSummary,
    /// The heights the levels give them, or `None` when they do not resolve.
    pub heights: Option<HeightsSummary>,
}

/// A corner of a crop as text, in the view's own coordinates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CornerSummary {
    /// Along the view's first axis.
    pub x: String,
    /// Along its second.
    pub y: String,
}

/// A crop region as a page lists it: `rectangle` or `outline`, and its corners in order (a
/// rectangle's four run counter-clockwise from the lower left).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RegionSummary {
    /// `rectangle` or `outline`.
    pub shape: String,
    /// The corners.
    pub corners: Vec<CornerSummary>,
}

/// A view's crop as a page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CropSummary {
    /// The region the model is cropped to.
    pub region: RegionSummary,
    /// The annotation region, if the crop has its own.
    pub annotation: Option<RegionSummary>,
    /// Whether the crop applies.
    pub on: bool,
    /// Whether its outline is shown.
    pub shown: bool,
}

fn tie_summary(tie: &Tie) -> TieSummary {
    TieSummary {
        level: tie.level.to_string(),
        offset: tie.offset.to_string(),
    }
}

fn region_summary(region: &Region) -> RegionSummary {
    RegionSummary {
        shape: region.key().to_owned(),
        corners: region
            .corners()
            .iter()
            .map(|p| CornerSummary {
                x: p.x.to_string(),
                y: p.y.to_string(),
            })
            .collect(),
    }
}

/// The summary of one view.
#[must_use]
pub fn summary(view: &View, levels: &Levels) -> ViewSummary {
    let level = view.kind().plan().map(|plan| plan.level.clone());
    ViewSummary {
        range: view.kind().plan().map(|plan| RangeSummary {
            top: tie_summary(&plan.range.top),
            cut: tie_summary(&plan.range.cut),
            bottom: tie_summary(&plan.range.bottom),
            depth: tie_summary(&plan.range.depth),
            heights: plan
                .range
                .resolve(levels)
                .ok()
                .map(|heights| HeightsSummary {
                    top: heights.top.to_string(),
                    cut: heights.cut.to_string(),
                    bottom: heights.bottom.to_string(),
                    depth: heights.depth.to_string(),
                }),
        }),
        crop: view.crop().map(|crop| CropSummary {
            region: region_summary(crop.region()),
            annotation: crop.annotation().map(region_summary),
            on: crop.is_on(),
            shown: crop.is_shown(),
        }),
        id: view.id().to_string(),
        name: view.name().to_owned(),
        kind: view.kind().key().to_owned(),
        level_name: level
            .as_ref()
            .and_then(|id| levels.resolve(id))
            .map(|found: Level| found.name().to_owned()),
        level: level.map(|id| id.to_string()),
        scale: view.scale().denominator(),
        detail: view.detail().key().to_owned(),
    }
}

/// Every view of the world, by name (case aside), then by ID.
#[must_use]
pub fn summaries(world: &World) -> Vec<ViewSummary> {
    world
        .views
        .ordered()
        .into_iter()
        .map(|view| summary(view, &world.levels))
        .collect()
}

/// What a view shows, as a page lists it: the IDs of the elements the view's cut plane or line
/// crosses, the ones seen in it, and the ones seen below a plan's bottom, each sorted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ViewListing {
    /// The view.
    pub view: ViewSummary,
    /// The elements cut.
    pub cut: Vec<String>,
    /// The elements seen.
    pub seen: Vec<String>,
    /// The elements of a plan seen below its bottom, to the view depth.
    pub beyond: Vec<String>,
}

/// An element and the box its shape took, as regeneration keeps them.
#[derive(Clone, Debug)]
pub struct Shaped {
    /// The element's record.
    pub record: Element,
    /// The box of its shape, whole millimetres, from the shape's own base.
    pub bounds: Bounds,
    /// How high its base sits above the project's zero, in construct-params' stored length text,
    /// as the built row holds it: `None` on no level.
    pub elevation: Option<String>,
}

/// The box lifted to where the element stands: the shape's own box with its base's elevation added
/// to both heights, the low one rounded down and the high one up to whole millimetres, so the box
/// still holds the shape.
fn lifted(shaped: &Shaped) -> Result<Bounds, String> {
    let mut bounds = shaped.bounds;
    let Some(text) = &shaped.elevation else {
        return Ok(bounds);
    };
    let Some(Value::Length(elevation)) = Value::from_stored(text) else {
        return Err(format!(
            "{}'s elevation {text:?} is not a length.",
            shaped.record.name
        ));
    };
    let too_high = || format!("{} stands too high to be tested.", shaped.record.name);
    let per = Length::STEPS_PER_MM;
    let down = elevation.steps().div_euclid(per);
    let up = if elevation.steps().rem_euclid(per) == 0 {
        down
    } else {
        down.checked_add(1).ok_or_else(too_high)?
    };
    let [low, high] =
        [(bounds.min[2], down), (bounds.max[2], up)].map(|(at, by)| at.checked_add(by));
    bounds.min[2] = low.ok_or_else(too_high)?;
    bounds.max[2] = high.ok_or_else(too_high)?;
    Ok(bounds)
}

/// What the view `id` shows of the elements given, each as its record, the box its shape took and
/// the height of its base: construct-views' [`construct_views::list`]. An element of no known
/// category is not drawn. `None` when the document has no such view.
///
/// Interim: elements carry no phase or design option in their records yet, so each is read as
/// created in [`NEW_PHASE`], in no option, until the job that stores staging on every element
/// replaces this one function ([`new_staging`]).
///
/// # Errors
/// The crate's words for a view that does not stand in the document or a box it cannot test.
pub fn listing_of(
    world: &World,
    id: &Id,
    shaped: &[Shaped],
) -> Result<Option<ViewListing>, String> {
    let Some(view) = world.views.get(id) else {
        return Ok(None);
    };
    let staging = new_staging(world)?;
    let mut elements = Vec::new();
    for one in shaped {
        let record = &one.record;
        let category =
            records::stored_category(record).or_else(|| records::category_of(&record.recipe).ok());
        if let Some(category) = category {
            elements.push(ViewElement {
                id: record.id.clone(),
                category,
                bounds: lifted(one)?,
                staging: staging.clone(),
            });
        }
    }
    let listing = list(view, &world.model(), &elements).map_err(|e| e.to_string())?;
    let ids = |found: Vec<&Id>| found.into_iter().map(ToString::to_string).collect();
    Ok(Some(ViewListing {
        view: summary(view, &world.levels),
        cut: ids(listing.cut()),
        seen: ids(listing.seen()),
        beyond: ids(listing.beyond()),
    }))
}

/// How a floor plan is cut and printed: the height of its cut plane above the project's zero,
/// resolved through its range's level, and its scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanCut {
    /// The cut plane's height.
    pub cut: Length,
    /// The scale it is printed at.
    pub scale: Scale,
}

/// The cut of the floor plan `id`: `None` when the document has no such view.
///
/// # Errors
/// In words, when the view is not a floor plan (only floor plans are drawn yet) or its range does
/// not resolve against the document's levels.
pub fn plan_cut(world: &World, id: &Id) -> Result<Option<PlanCut>, String> {
    let Some(view) = world.views.get(id) else {
        return Ok(None);
    };
    let ViewKind::FloorPlan(setup) = view.kind() else {
        return Err(format!(
            "{} is a {} view; only floor plans are drawn yet.",
            view.name(),
            view.kind().key()
        ));
    };
    let heights = setup
        .range
        .resolve(&world.levels)
        .map_err(|e| e.to_string())?;
    Ok(Some(PlanCut {
        cut: heights.cut,
        scale: view.scale(),
    }))
}

/// The staging of what is made today: created in the new phase, in no design option.
fn new_staging(world: &World) -> Result<ElementStaging, String> {
    let created = Id::parse(NEW_PHASE).map_err(|e| e.to_string())?;
    ElementStaging::new(&world.phases, &world.options, created, None, None)
        .map_err(|e| e.to_string())
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

    /// The document's current record of a piece, and its hash.
    async fn current(&mut self, id: &Id) -> Result<Option<(Id, Element)>, StoreError> {
        records::read_element(self.txn, self.project, self.document, id).await
    }

    /// The document's levels and views as they are now.
    async fn world(&mut self) -> Result<Result<World, String>, StoreError> {
        read_world(self.txn, self.project, self.document).await
    }

    /// The view of this ID as it is now, with its record's hash.
    async fn view_of(&mut self, id: &Id) -> Result<Result<(Id, View), String>, StoreError> {
        Ok(match self.current(id).await? {
            Some((hash, record)) if record.kind == VIEW_KIND => View::from_record(&record)
                .map(|view| (hash, view))
                .map_err(|e| e.to_string()),
            _ => Err(construct_views::ViewError::UnknownView(id.clone()).to_string()),
        })
    }

    /// Writes one record whose piece this person claims, as the command's whole result.
    async fn put_claimed(&mut self, record: &Element, before: Option<Id>) -> Answer {
        let claim = match self.claim(&record.id).await? {
            Ok(claim) => claim,
            Err(refusal) => return Ok(Err(refusal)),
        };
        let hash = write_record(self.txn, self.project, record).await?;
        let pointer = Place::Piece {
            project: self.project.clone(),
            document: self.document.clone(),
            piece: record.id.clone(),
        };
        self.txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
        let claim = claims::take(self.txn, self.project, self.document, &record.id, claim).await?;
        Ok(Ok(Applied {
            changes: vec![Change {
                piece: record.id.clone(),
                before,
                after: Some(hash),
            }],
            claims: vec![claim],
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
    let graphics = edit.cmd.starts_with("view.graphics.");
    if !graphics && (args.category.is_some() || args.edge.is_some() || args.style.is_some()) {
        return Ok(Err(format!(
            "{} has no category, edge or style: graphics rules are set with view.graphics.set.",
            edit.cmd
        )));
    }
    if !edit.cmd.starts_with("view.crop.")
        && (args.region.is_some()
            || args.annotation.is_some()
            || args.on.is_some()
            || args.shown.is_some())
    {
        return Ok(Err(format!(
            "{} has no region, annotation, on or shown: a crop is set with view.crop.set.",
            edit.cmd
        )));
    }
    if edit.cmd != "view.line.move" && args.by.is_some() {
        return Ok(Err(format!(
            "{} has no by: a line is moved with view.line.move.",
            edit.cmd
        )));
    }
    if edit.cmd != "view.elevation.create" && (args.face.is_some() || args.width.is_some()) {
        return Ok(Err(format!(
            "{} has no face or width: an elevation by direction is made with \
             view.elevation.create.",
            edit.cmd
        )));
    }
    let mut run = Run {
        txn,
        project,
        document: &edit.target,
        edit,
        now_ms: context.now_ms,
        claim_ms,
    };
    match edit.cmd.as_str() {
        "view.line.move" => move_line(&mut run, args).await,
        "view.line.flip" => flip_line(&mut run, args).await,
        "view.elevation.create" => create_elevation(&mut run, args).await,
        "view.crop.set" => set_crop(&mut run, args).await,
        "view.crop.clear" => clear_crop(&mut run, args).await,
        "view.crop.show" => show_crop(&mut run, args).await,
        "view.create" => create(&mut run, args).await,
        "view.rename" => rename(&mut run, args).await,
        "view.set" => set(&mut run, args).await,
        "view.duplicate" => duplicate(&mut run, args).await,
        "view.delete" => delete(&mut run, args).await,
        "view.graphics.set" => set_graphics(&mut run, args).await,
        "view.graphics.clear" => clear_graphics(&mut run, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// The kinds a person may name, for the refusal that lists them.
fn kinds() -> String {
    ViewKind::KEYS.join(", ")
}

fn scale_of(n: u32) -> Result<Scale, String> {
    Scale::new(n).map_err(|e| e.to_string())
}

fn detail_of(key: &str) -> Result<DetailLevel, String> {
    DetailLevel::from_key(key)
        .ok_or_else(|| format!("{key:?} is not a detail level: coarse, medium or fine."))
}

fn side_of(key: &str) -> Result<Side, String> {
    Side::from_key(key).ok_or_else(|| format!("{key:?} is not a side: left or right."))
}

/// A tie with the parts a person named put over the ones it had.
fn tie_over(base: &Tie, args: Option<&TieArgs>) -> Result<Tie, String> {
    let Some(args) = args else {
        return Ok(base.clone());
    };
    let level = match &args.level {
        Some(text) => id_of(text, "a level")?,
        None => base.level.clone(),
    };
    let offset = match &args.offset {
        Some(text) => length(text)?,
        None => base.offset,
    };
    Ok(Tie { level, offset })
}

/// A new plan's range: the four default heights, on its own level.
fn default_range(level: &Id) -> ViewRange {
    let [top, cut, bottom, depth] = DEFAULT_RANGE_MM.map(Length::from_mm);
    ViewRange::on_level(level, top, cut, bottom, depth)
}

fn range_over(base: &ViewRange, args: Option<&RangeArgs>) -> Result<ViewRange, String> {
    let Some(args) = args else {
        return Ok(base.clone());
    };
    Ok(ViewRange {
        top: tie_over(&base.top, args.top.as_ref())?,
        cut: tie_over(&base.cut, args.cut.as_ref())?,
        bottom: tie_over(&base.bottom, args.bottom.as_ref())?,
        depth: tie_over(&base.depth, args.depth.as_ref())?,
    })
}

/// A plan from the arguments, over `base` when the view already is one.
fn plan_of(args: &Args, base: Option<&PlanSetup>) -> Result<PlanSetup, String> {
    let level = match (&args.level, base) {
        (Some(text), _) => id_of(text, "a level")?,
        (None, Some(base)) => base.level.clone(),
        (None, None) => return Err("A plan needs a level.".to_owned()),
    };
    let range = match base {
        Some(base) => range_over(&base.range, args.range.as_ref())?,
        None => range_over(&default_range(&level), args.range.as_ref())?,
    };
    Ok(PlanSetup { level, range })
}

fn point_over(base: Option<Point>, args: Option<&PointArgs>, what: &str) -> Result<Point, String> {
    match (args, base) {
        (Some(args), _) => Ok(Point {
            x: length(&args.x)?,
            y: length(&args.y)?,
        }),
        (None, Some(base)) => Ok(base),
        (None, None) => Err(format!("A section or elevation needs {what}.")),
    }
}

/// A section's or elevation's line from the arguments, over `base` when the view already has one.
fn line_of(args: &Args, base: Option<&CutLine>) -> Result<CutLine, String> {
    let from = point_over(
        base.map(CutLine::from),
        args.from.as_ref(),
        "a first end (from)",
    )?;
    let to = point_over(base.map(CutLine::to), args.to.as_ref(), "a second end (to)")?;
    let look = match (&args.look, base) {
        (Some(key), _) => side_of(key)?,
        (None, Some(base)) => base.look(),
        (None, None) => return Err("A section or elevation needs a side to look to (look).".into()),
    };
    let far = match (&args.depth, base) {
        (Some(text), _) => length(text)?,
        (None, Some(base)) => base.far(),
        (None, None) => return Err("A section or elevation needs a depth.".to_owned()),
    };
    CutLine::new(from, to, look, far).map_err(|e| e.to_string())
}

/// Refuses an argument the kind has no use for, naming the kind.
fn fit(args: &Args, key: &str) -> Result<(), String> {
    let plan = matches!(key, "floor_plan" | "ceiling_plan");
    let cut = matches!(key, "section" | "elevation");
    let words = key.replace('_', " ");
    if !plan && (args.level.is_some() || args.range.is_some()) {
        let what = if args.level.is_some() {
            "level"
        } else {
            "view range"
        };
        return Err(format!("A {words} has no {what}."));
    }
    let line_given =
        args.from.is_some() || args.to.is_some() || args.look.is_some() || args.depth.is_some();
    if !cut && line_given {
        return Err(format!("A {words} has no line."));
    }
    Ok(())
}

/// The data of a kind from the arguments, over `base` when the view already is of that kind.
fn kind_of(args: &Args, key: &str, base: Option<&ViewKind>) -> Result<ViewKind, String> {
    fit(args, key)?;
    Ok(match key {
        "floor_plan" => ViewKind::FloorPlan(plan_of(args, base.and_then(ViewKind::plan))?),
        "ceiling_plan" => ViewKind::CeilingPlan(plan_of(args, base.and_then(ViewKind::plan))?),
        "section" => ViewKind::Section(line_of(args, base.and_then(ViewKind::cut_line))?),
        "elevation" => ViewKind::Elevation(line_of(args, base.and_then(ViewKind::cut_line))?),
        "three_d" => ViewKind::ThreeD,
        "drafting" => ViewKind::Drafting,
        "legend" => ViewKind::Legend,
        other => {
            return Err(format!(
                "{other:?} is not a kind of view. The kinds are {}.",
                kinds()
            ));
        }
    })
}

/// Phase settings from `phasing`: none for `null`.
fn phasing_of(value: &serde_json::Value) -> Result<Option<ViewSettings>, String> {
    if value.is_null() {
        return Ok(None);
    }
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|e| format!("That view's phase settings aren't valid: {e}"))
}

/// A view's record, written as it is stored.
fn record_of(view: &View, document: &Id) -> Element {
    view.to_record(document)
}

async fn create<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let world = match run.world().await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let made = make(&args, &world);
    let view = match made {
        Ok(view) => view,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(view.id()).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let mut views = world.views.clone();
    if let Err(e) = views.add(view.clone()) {
        return Ok(Err(e.to_string()));
    }
    if let Err(refusal) = world.check(&view) {
        return Ok(Err(refusal));
    }
    run.put_claimed(&record_of(&view, run.document), None).await
}

/// The staging a new view is made with: the one asked for, else created in the new phase, in no
/// design option.
fn staging_of(args: &Args, world: &World) -> Result<ElementStaging, String> {
    if let Some(staging) = &args.staging {
        return Ok(staging.clone());
    }
    new_staging(world)
}

/// The view a `view.create` asks for, before it is checked against the document.
fn make(args: &Args, world: &World) -> Result<View, String> {
    let id = need(args.view.clone(), "a view")?;
    let name = need(args.name.as_deref(), "a name")?;
    let key = need(args.kind.as_deref(), "a kind")?;
    let phasing = match &args.phasing {
        Some(value) => phasing_of(value)?,
        None => None,
    };
    let kind = kind_of(args, key, None)?;
    let scale = scale_of(args.scale.unwrap_or(DEFAULT_SCALE))?;
    let detail = match &args.detail {
        Some(key) => detail_of(key)?,
        None => DEFAULT_DETAIL,
    };
    let staging = staging_of(args, world)?;
    View::new(id, name, scale, detail, kind, staging)
        .map(|view| view.with_phasing(phasing))
        .map_err(|e| e.to_string())
}

/// Replaces a view with a changed one: checks it against the document and writes it.
async fn put_changed<T: Txn>(
    run: &mut Run<'_, T>,
    world: &World,
    before: (Id, View),
    changed: View,
) -> Answer {
    let (hash, old) = before;
    if changed == old {
        // Nothing changed: no record is written and no step is made.
        return Ok(Ok(Applied::default()));
    }
    let mut views = world.views.clone();
    if let Err(e) = views.replace(changed.clone()) {
        return Ok(Err(e.to_string()));
    }
    if let Err(refusal) = world.check(&changed) {
        return Ok(Err(refusal));
    }
    run.put_claimed(&record_of(&changed, run.document), Some(hash))
        .await
}

/// The world and the view a command is about.
async fn subject<T: Txn>(
    run: &mut Run<'_, T>,
    view: Option<Id>,
) -> Result<Result<(World, Id, View), String>, StoreError> {
    let id = match need(view, "a view") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let world = match run.world().await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(match run.view_of(&id).await? {
        Ok((hash, view)) => Ok((world, hash, view)),
        Err(refusal) => Err(refusal),
    })
}

async fn rename<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, view) = match subject(run, args.view.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let renamed = need(args.name.as_deref(), "a name")
        .and_then(|name| view.clone().renamed(name).map_err(|e| e.to_string()));
    match renamed {
        Ok(changed) => put_changed(run, &world, (hash, view), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// The view after a `view.set`, or why it cannot be.
fn changed_by(args: &Args, view: View) -> Result<View, String> {
    if args.kind.is_some() {
        return Err("A view's kind is not changed: make another view.".to_owned());
    }
    if args.name.is_some() {
        return Err("A view is renamed with view.rename.".to_owned());
    }
    let mut view = view;
    if let Some(n) = args.scale {
        view = view.with_scale(scale_of(n)?);
    }
    if let Some(key) = &args.detail {
        view = view.with_detail(detail_of(key)?);
    }
    let key = view.kind().key();
    fit(args, key)?;
    if args.level.is_some()
        || args.range.is_some()
        || args.from.is_some()
        || args.to.is_some()
        || args.look.is_some()
        || args.depth.is_some()
    {
        let kind = kind_of(args, key, Some(view.kind()))?;
        view = view.with_kind(kind);
    }
    if let Some(staging) = &args.staging {
        view = view.with_staging(staging.clone());
    }
    if let Some(value) = &args.phasing {
        view = view.with_phasing(phasing_of(value)?);
    }
    Ok(view)
}

async fn set<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, view) = match subject(run, args.view.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    match changed_by(&args, view.clone()) {
        Ok(changed) => put_changed(run, &world, (hash, view), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// The first name `<stem>`, `<stem> 2`, `<stem> 3`... that no view has, case aside.
fn free_name(views: &Views, stem: &str) -> Result<String, String> {
    let taken: Vec<String> = views
        .ordered()
        .iter()
        .map(|view| view.name().to_lowercase())
        .collect();
    for n in 1..=taken.len().saturating_add(1) {
        let candidate = if n == 1 {
            stem.to_owned()
        } else {
            format!("{stem} {n}")
        };
        if !taken.contains(&candidate.to_lowercase()) {
            return Ok(candidate);
        }
    }
    Err(format!("No free name was found for a copy of {stem}."))
}

async fn duplicate<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, _, view) = match subject(run, args.view.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let copied = need(args.copy.clone(), "an ID for the copy (new)").and_then(|id| {
        let name = match &args.name {
            Some(name) => name.clone(),
            None => free_name(&world.views, &format!("{} copy", view.name()))?,
        };
        View::new(
            id,
            &name,
            view.scale(),
            view.detail(),
            view.kind().clone(),
            view.staging().clone(),
        )
        .map(|copy| {
            copy.with_phasing(view.phasing().cloned())
                .with_graphics(view.graphics().clone())
                .with_crop(view.crop().cloned())
        })
        .map_err(|e| e.to_string())
    });
    let copy = match copied {
        Ok(copy) => copy,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(copy.id()).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let mut views = world.views.clone();
    if let Err(e) = views.add(copy.clone()) {
        return Ok(Err(e.to_string()));
    }
    run.put_claimed(&record_of(&copy, run.document), None).await
}

async fn delete<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match need(args.view, "a view") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let (hash, _) = match run.view_of(&id).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    // A view that is placed on a sheet is not deleted from under its viewport.
    if let Some(refusal) =
        crate::cloud::sheets::placed_refusal(run.txn, run.project, run.document, &id).await?
    {
        return Ok(Err(refusal));
    }
    if let Err(refusal) = run.claim(&id).await? {
        return Ok(Err(refusal));
    }
    let pointer = Place::Piece {
        project: run.project.clone(),
        document: run.document.clone(),
        piece: id.clone(),
    };
    run.txn.delete(pointer.key()).await?;
    let released = claims::release(run.txn, run.project, run.document, &id).await?;
    Ok(Ok(Applied {
        changes: vec![Change {
            piece: id,
            before: Some(hash),
            after: None,
        }],
        claims: vec![released],
        ..Applied::default()
    }))
}

/// The line of `view`, a section or an elevation, or why it has none.
fn line_or_refusal(view: &View) -> Result<CutLine, String> {
    view.kind().cut_line().copied().ok_or_else(|| {
        format!(
            "A {} has no line: only a section or an elevation is moved or flipped.",
            view.kind().key().replace('_', " ")
        )
    })
}

/// `view` with its line put back as `line`, keeping its kind.
fn with_line(view: View, line: CutLine) -> View {
    let kind = match view.kind() {
        ViewKind::Elevation(_) => ViewKind::Elevation(line),
        _ => ViewKind::Section(line),
    };
    view.with_kind(kind)
}

/// The view after a `view.line.move`, or why it cannot be.
fn moved_by(args: &Args, view: View) -> Result<View, String> {
    let line = line_or_refusal(&view)?;
    let by = need(args.by.as_ref(), "how far to move it (by)")?;
    let (dx, dy) = (length(&by.x)?, length(&by.y)?);
    let shift = |p: Point| -> Result<Point, String> {
        let add = |a: Length, b: Length| {
            a.steps()
                .checked_add(b.steps())
                .map(Length::from_steps)
                .ok_or_else(|| "That move takes the line past the grid's reach.".to_owned())
        };
        Ok(Point {
            x: add(p.x, dx)?,
            y: add(p.y, dy)?,
        })
    };
    let moved = CutLine::new(
        shift(line.from())?,
        shift(line.to())?,
        line.look(),
        line.far(),
    )
    .map_err(|e| e.to_string())?;
    Ok(with_line(view, moved))
}

async fn move_line<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    if let Err(refusal) = alone(&args, "view.line.move", &["by"]) {
        return Ok(Err(refusal));
    }
    let (world, hash, view) = match subject(run, args.view.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    match moved_by(&args, view.clone()) {
        Ok(changed) => put_changed(run, &world, (hash, view), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// The view after a `view.line.flip`: its line looking to its other side.
fn flipped(view: View) -> Result<View, String> {
    let line = line_or_refusal(&view)?;
    let look = match line.look() {
        Side::Left => Side::Right,
        Side::Right => Side::Left,
    };
    let turned =
        CutLine::new(line.from(), line.to(), look, line.far()).map_err(|e| e.to_string())?;
    Ok(with_line(view, turned))
}

async fn flip_line<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    if let Err(refusal) = alone(&args, "view.line.flip", &[]) {
        return Ok(Err(refusal));
    }
    let (world, hash, view) = match subject(run, args.view.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    match flipped(view.clone()) {
        Ok(changed) => put_changed(run, &world, (hash, view), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// Refuses every argument of `args` but `allowed`, naming the command: a command that moves or
/// flips a line takes nothing else.
fn alone(args: &Args, cmd: &str, allowed: &[&str]) -> Result<(), String> {
    let given = [
        ("new", args.copy.is_some()),
        ("kind", args.kind.is_some()),
        ("name", args.name.is_some()),
        ("level", args.level.is_some()),
        ("scale", args.scale.is_some()),
        ("detail", args.detail.is_some()),
        ("range", args.range.is_some()),
        ("from", args.from.is_some()),
        ("to", args.to.is_some()),
        ("look", args.look.is_some()),
        ("depth", args.depth.is_some()),
        ("staging", args.staging.is_some()),
        ("phasing", args.phasing.is_some()),
        ("by", args.by.is_some()),
        ("face", args.face.is_some()),
        ("width", args.width.is_some()),
    ];
    for (key, there) in given {
        if there && !allowed.contains(&key) {
            return Err(format!("{cmd} takes no {key}."));
        }
    }
    Ok(())
}

/// The line of an elevation of the face `face` of the building, from `from` (its left end as one
/// faces the building) `width` along it: its second end, and that it looks to its left.
fn elevation_line(face: &str, from: Point, width: Length) -> Result<(Point, Side), String> {
    let w = width.steps();
    let back = w
        .checked_neg()
        .ok_or_else(|| "That elevation's line reaches past the grid.".to_owned())?;
    // Walking from the first end to the second, the building is on the left.
    let (dx, dy) = match face {
        // Looking north at the south face: the line runs east.
        "south" => (w, 0),
        // Looking south at the north face: the line runs west.
        "north" => (back, 0),
        // Looking east at the west face: the line runs south.
        "west" => (0, back),
        // Looking west at the east face: the line runs north.
        "east" => (0, w),
        other => {
            return Err(format!(
                "{other:?} is not a face of the building: north, south, east or west."
            ));
        }
    };
    let add = |a: Length, d: i64| {
        a.steps()
            .checked_add(d)
            .map(Length::from_steps)
            .ok_or_else(|| "That elevation's line reaches past the grid.".to_owned())
    };
    Ok((
        Point {
            x: add(from.x, dx)?,
            y: add(from.y, dy)?,
        },
        Side::Left,
    ))
}

/// The elevation a `view.elevation.create` asks for, before it is checked against the document.
fn make_elevation(args: &Args, world: &World) -> Result<View, String> {
    alone(
        args,
        "view.elevation.create",
        &[
            "name", "from", "depth", "scale", "detail", "staging", "phasing", "face", "width",
        ],
    )?;
    let id = need(args.view.clone(), "a view")?;
    let name = need(args.name.as_deref(), "a name")?;
    let face = need(args.face.as_deref(), "the face it shows (face)")?;
    let from = point_over(None, args.from.as_ref(), "its left end (from)")?;
    let width = length(need(args.width.as_deref(), "a width")?)?;
    if width <= Length::from_steps(0) {
        return Err("An elevation's width must be above zero.".to_owned());
    }
    let far = length(need(args.depth.as_deref(), "a depth")?)?;
    let (to, look) = elevation_line(face, from, width)?;
    let line = CutLine::new(from, to, look, far).map_err(|e| e.to_string())?;
    let phasing = match &args.phasing {
        Some(value) => phasing_of(value)?,
        None => None,
    };
    let scale = scale_of(args.scale.unwrap_or(DEFAULT_SCALE))?;
    let detail = match &args.detail {
        Some(key) => detail_of(key)?,
        None => DEFAULT_DETAIL,
    };
    let staging = staging_of(args, world)?;
    View::new(id, name, scale, detail, ViewKind::Elevation(line), staging)
        .map(|view| view.with_phasing(phasing))
        .map_err(|e| e.to_string())
}

async fn create_elevation<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let world = match run.world().await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let view = match make_elevation(&args, &world) {
        Ok(view) => view,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(view.id()).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let mut views = world.views.clone();
    if let Err(e) = views.add(view.clone()) {
        return Ok(Err(e.to_string()));
    }
    if let Err(refusal) = world.check(&view) {
        return Ok(Err(refusal));
    }
    run.put_claimed(&record_of(&view, run.document), None).await
}

/// How a section or an elevation is drawn: its line, whether it cuts, and its scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionCut {
    /// The line, the side it looks to and how far it sees.
    pub line: CutLine,
    /// `true` for a section (what its plane crosses is cut), `false` for an elevation.
    pub cuts: bool,
    /// The scale it is printed at.
    pub scale: Scale,
}

/// The line of the section or elevation `id`: `None` when the document has no such view.
///
/// # Errors
/// In words, when the view is not a section or an elevation.
pub fn section_cut(world: &World, id: &Id) -> Result<Option<SectionCut>, String> {
    let Some(view) = world.views.get(id) else {
        return Ok(None);
    };
    let (line, cuts) = match view.kind() {
        ViewKind::Section(line) => (*line, true),
        ViewKind::Elevation(line) => (*line, false),
        other => {
            return Err(format!(
                "{} is a {} view; only a section or an elevation is drawn here.",
                view.name(),
                other.key().replace('_', " ")
            ));
        }
    };
    Ok(Some(SectionCut {
        line,
        cuts,
        scale: view.scale(),
    }))
}

/// Where a level or a grid is, as a section or an elevation draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DatumPlace {
    /// A level, at its height above the project's zero.
    Level(Length),
    /// A straight grid, from one end to the other in plan.
    Grid {
        /// Its first end.
        from: Point,
        /// Its second end.
        to: Point,
    },
    /// An arc grid.
    ArcGrid,
}

/// A level or a grid of a document, for a section or an elevation to draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatumLine {
    /// Its permanent ID.
    pub id: Id,
    /// Its category: levels or grids.
    pub category: CategoryId,
    /// The name people see.
    pub name: String,
    /// Where it is.
    pub at: DatumPlace,
}

/// Every level of `world` (lowest first) and every grid of the document (by ID), as a section or
/// an elevation draws them. The inner error is a grid record that is not as written, in words.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn datums_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    world: &World,
) -> Result<Result<Vec<DatumLine>, String>, StoreError> {
    let mut found: Vec<DatumLine> = world
        .levels
        .ordered()
        .into_iter()
        .map(|level| DatumLine {
            id: level.id().clone(),
            category: construct_categories::LEVELS,
            name: level.name().to_owned(),
            at: DatumPlace::Level(level.elevation()),
        })
        .collect();
    let all = records::read_document(reader, project, document).await?;
    let datums = match construct_grids::Datums::from_records(all.values().map(|(_, r)| r)) {
        Ok(datums) => datums,
        Err(e) => return Ok(Err(e.to_string())),
    };
    for grid in datums.grids() {
        let at = match grid.shape() {
            construct_grids::Shape::Line { from, to } => DatumPlace::Grid {
                from: Point {
                    x: from.x,
                    y: from.y,
                },
                to: Point { x: to.x, y: to.y },
            },
            construct_grids::Shape::Arc { .. } => DatumPlace::ArcGrid,
        };
        found.push(DatumLine {
            id: grid.id().clone(),
            category: construct_categories::GRIDS,
            name: grid.name().to_owned(),
            at,
        });
    }
    Ok(Ok(found))
}

/// A whole number as a colour part, 0 to 255.
fn colour_of(parts: &[i64]) -> Result<Colour, String> {
    let words = || "A colour is [red, green, blue], each a whole number from 0 to 255.".to_owned();
    match parts {
        [r, g, b] => {
            let part = |n: &i64| u8::try_from(*n).map_err(|_| words());
            Ok(Colour(part(r)?, part(g)?, part(b)?))
        }
        _ => Err(words()),
    }
}

/// The category a graphics command names, as the document has it.
fn category_of(args: &Args, world: &World) -> Result<CategoryId, String> {
    let n = need(args.category, "a category (its number)")?;
    let missing = || format!("There is no category {n}.");
    let id = CategoryId(u32::try_from(n).map_err(|_| missing())?);
    world.categories.get(id).map(|_| id).ok_or_else(missing)
}

/// The edge a graphics command names.
fn edge_of(key: &str) -> Result<Edge, String> {
    edge_from_key(key).ok_or_else(|| format!("{key:?} is not an edge: cut or projection."))
}

/// How the element looks in the view now, on one edge: the base a rule's named parts go over.
fn look_now(
    world: &World,
    view: &View,
    category: CategoryId,
    edge: Edge,
) -> Result<RuleStyle, String> {
    let parent = world
        .categories
        .get(category)
        .and_then(|found| found.parent);
    let (top, sub) = match parent {
        Some(parent) => (parent, Some(category)),
        None => (category, None),
    };
    resolve(
        view,
        &world.categories,
        &world.objects,
        &world.lines,
        top,
        sub,
        edge,
    )
    .map(|found| found.style)
    .map_err(|e| e.to_string())
}

/// A style with the parts a person named put over `base`.
fn style_over(base: RuleStyle, args: &StyleArgs) -> Result<RuleStyle, String> {
    let mut style = base;
    if let Some(n) = args.pen {
        style.pen =
            u16::try_from(n).map_err(|_| format!("There is no pen {n} for this category."))?;
    }
    if let Some(n) = args.pattern {
        style.pattern = u32::try_from(n)
            .map(PatternId)
            .map_err(|_| format!("There is no line pattern {n}."))?;
    }
    if let Some(parts) = &args.colour {
        style.colour = colour_of(parts)?;
    }
    if let Some(value) = &args.fill {
        style.fill = if value.is_null() {
            None
        } else {
            let fill: FillArgs = serde_json::from_value(value.clone()).map_err(|e| {
                format!("A fill is {{\"pattern\": <name>, \"colour\": [r, g, b]}} or null: {e}")
            })?;
            Some(Fill {
                pattern: fill.pattern,
                colour: colour_of(&fill.colour)?,
            })
        };
    }
    if let Some(on) = args.halftone {
        style.halftone = on;
    }
    if let Some(n) = args.transparency {
        style.transparency = u8::try_from(n)
            .ok()
            .filter(|n| *n <= construct_views::MOST_TRANSPARENCY)
            .ok_or_else(|| format!("Transparency is a percentage from 0 to 100, not {n}."))?;
    }
    if let Some(on) = args.shown {
        style.shown = on;
    }
    Ok(style)
}

fn crop_point(args: &PointArgs) -> Result<Point, String> {
    Ok(Point {
        x: length(&args.x)?,
        y: length(&args.y)?,
    })
}

/// A crop region from the arguments, checked exactly by construct-views.
fn region_of(args: &RegionArgs) -> Result<Region, String> {
    match args {
        RegionArgs::Rectangle { min, max } => {
            Region::rectangle(crop_point(min)?, crop_point(max)?).map_err(|e| e.to_string())
        }
        RegionArgs::Outline(corners) => Region::outline(
            corners
                .iter()
                .map(crop_point)
                .collect::<Result<Vec<_>, String>>()?,
        )
        .map_err(|e| e.to_string()),
    }
}

/// What a crop command takes that is not a crop's: refused, so nothing is silently dropped.
fn crop_alone(args: &Args, cmd: &str) -> Result<(), String> {
    let others = args.category.is_some()
        || args.edge.is_some()
        || args.style.is_some()
        || args.name.is_some()
        || args.kind.is_some()
        || args.scale.is_some()
        || args.detail.is_some()
        || args.level.is_some()
        || args.range.is_some()
        || args.from.is_some()
        || args.to.is_some()
        || args.look.is_some()
        || args.depth.is_some()
        || args.staging.is_some()
        || args.phasing.is_some()
        || args.copy.is_some();
    if others {
        return Err(format!("{cmd} takes only a view and the crop's own parts."));
    }
    Ok(())
}

/// The view after a `view.crop.set`, or why it cannot be.
fn crop_set(args: &Args, view: &View) -> Result<View, String> {
    crop_alone(args, "view.crop.set")?;
    if args.shown.is_some() {
        return Err(
            "view.crop.set does not show or hide the outline: use view.crop.show.".to_owned(),
        );
    }
    Crop::check_kind(view.kind()).map_err(|e| e.to_string())?;
    let region = args.region.as_ref().map(region_of).transpose()?;
    let mut crop = match (view.crop(), region) {
        (Some(crop), Some(region)) => crop.clone().with_region(region),
        (Some(crop), None) => crop.clone(),
        (None, Some(region)) => Crop::new(region),
        (None, None) => {
            return Err("This view has no crop yet: view.crop.set needs a region.".to_owned());
        }
    };
    if let Some(value) = &args.annotation {
        let annotation = if value.is_null() {
            None
        } else {
            let named: RegionArgs = serde_json::from_value(value.clone())
                .map_err(|e| format!("That annotation region isn't valid: {e}"))?;
            Some(region_of(&named)?)
        };
        crop = crop.with_annotation(annotation);
    }
    if let Some(on) = args.on {
        crop = crop.with_on(on);
    }
    Ok(view.clone().with_crop(Some(crop)))
}

async fn set_crop<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, view) = match subject(run, args.view.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    match crop_set(&args, &view) {
        Ok(changed) => put_changed(run, &world, (hash, view), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// The view after a `view.crop.clear`.
fn crop_cleared(args: &Args, view: &View) -> Result<View, String> {
    crop_alone(args, "view.crop.clear")?;
    if args.region.is_some()
        || args.annotation.is_some()
        || args.on.is_some()
        || args.shown.is_some()
    {
        return Err("view.crop.clear takes no part of a crop.".to_owned());
    }
    Ok(view.clone().with_crop(None))
}

async fn clear_crop<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, view) = match subject(run, args.view.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    match crop_cleared(&args, &view) {
        Ok(changed) => put_changed(run, &world, (hash, view), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// The view after a `view.crop.show`.
fn crop_shown(args: &Args, view: &View) -> Result<View, String> {
    crop_alone(args, "view.crop.show")?;
    if args.region.is_some() || args.annotation.is_some() || args.on.is_some() {
        return Err("view.crop.show takes only shown.".to_owned());
    }
    let shown = need(args.shown, "shown (true or false)")?;
    let crop = view
        .crop()
        .ok_or_else(|| ViewError::NoCrop(view.id().clone()).to_string())?;
    Ok(view.clone().with_crop(Some(crop.clone().with_shown(shown))))
}

async fn show_crop<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, view) = match subject(run, args.view.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    match crop_shown(&args, &view) {
        Ok(changed) => put_changed(run, &world, (hash, view), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// The view after a `view.graphics.set`, or why it cannot be.
fn graphics_set(args: &Args, world: &World, view: &View) -> Result<View, String> {
    let category = category_of(args, world)?;
    let edge = edge_of(need(args.edge.as_deref(), "an edge (cut or projection)")?)?;
    let named = need(args.style.as_ref(), "a style")?;
    let style = style_over(look_now(world, view, category, edge)?, named)?;
    let graphics = view.graphics().with_style(category, edge, style);
    graphics
        .check(view.kind(), &world.categories, &world.lines)
        .map_err(|e| e.to_string())?;
    Ok(view.clone().with_graphics(graphics))
}

async fn set_graphics<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, view) = match subject(run, args.view.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    match graphics_set(&args, &world, &view) {
        Ok(changed) => put_changed(run, &world, (hash, view), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// The view after a `view.graphics.clear`.
fn graphics_cleared(args: &Args, world: &World, view: &View) -> Result<View, String> {
    if args.style.is_some() {
        return Err("view.graphics.clear takes no style.".to_owned());
    }
    let category = category_of(args, world)?;
    let edge = args.edge.as_deref().map(edge_of).transpose()?;
    Ok(view
        .clone()
        .with_graphics(view.graphics().without(category, edge)))
}

async fn clear_graphics<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, view) = match subject(run, args.view.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    match graphics_cleared(&args, &world, &view) {
        Ok(changed) => put_changed(run, &world, (hash, view), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// One category's rule in a view, as a page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuleSummary {
    /// The category's number (a subcategory's included).
    pub category: u32,
    /// The category's name.
    pub name: String,
    /// The category it is a subcategory of, if it is one.
    pub parent: Option<u32>,
    /// How it looks where the view cuts it, if the view says.
    pub cut: Option<RuleStyle>,
    /// How it looks where the view sees it beyond the cut plane, if the view says.
    pub projection: Option<RuleStyle>,
}

/// How one edge of one category looks in a view now, as [`construct_views::resolve`] answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LookOf {
    /// The style.
    pub style: RuleStyle,
    /// The pen's printed width at the view's scale, in micrometres.
    pub width: u32,
    /// Which level gave it: `subcategory`, `category` or `project`.
    pub origin: &'static str,
}

/// How a category looks in a view now, each edge as it resolves: what an editor starts from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LookSummary {
    /// The category's number (a subcategory's included).
    pub category: u32,
    /// The category's name.
    pub name: String,
    /// The category it is a subcategory of, if it is one.
    pub parent: Option<u32>,
    /// Whether the view can cut it.
    pub cuttable: bool,
    /// Whether it takes the annotation pens (the others take the model pens).
    pub annotation: bool,
    /// How it looks where the view cuts it; none for what cannot be cut.
    pub cut: Option<LookOf>,
    /// How it looks where the view sees it beyond the cut plane.
    pub projection: Option<LookOf>,
}

/// A pen a rule may name, with its printed width.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PenSummary {
    /// Its number.
    pub number: u16,
    /// Its name.
    pub name: String,
    /// Its printed width at the view's scale, in micrometres.
    pub width: u32,
}

/// A line pattern a rule may name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PatternSummary {
    /// Its number.
    pub id: u32,
    /// Its name.
    pub name: String,
}

/// A view's graphics rules as a page lists them: the rules the view holds, by category, how every
/// category looks in the view now, and what a rule may name. A category with no rule is as the
/// project's object styles say.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GraphicsListing {
    /// The view.
    pub view: ViewSummary,
    /// Its rules, in ascending category.
    pub rules: Vec<RuleSummary>,
    /// Every category the view can say anything about, a category before its subcategories.
    pub looks: Vec<LookSummary>,
    /// The model pens, at the view's scale.
    pub pens: Vec<PenSummary>,
    /// The annotation pens.
    pub annotation_pens: Vec<PenSummary>,
    /// The line patterns.
    pub line_patterns: Vec<PatternSummary>,
    /// The fill patterns a cut may be given.
    pub fill_patterns: Vec<&'static str>,
}

/// The graphics rules of the view `id`: `None` when the document has no such view.
#[must_use]
pub fn graphics_of(world: &World, id: &Id) -> Option<GraphicsListing> {
    let view = world.views.get(id)?;
    let rules = view
        .graphics()
        .rules()
        .filter_map(|(category, rule)| {
            let found = world.categories.get(category)?;
            Some(RuleSummary {
                category: category.0,
                name: found.name,
                parent: found.parent.map(|parent| parent.0),
                cut: rule.cut.clone(),
                projection: rule.projection.clone(),
            })
        })
        .collect();
    let scale = view.scale();
    let looked = |category: CategoryId, edge: Edge| {
        look_in(world, id, category, edge).ok().map(|found| LookOf {
            width: found.width.get(),
            origin: found.origin.key(),
            style: found.style,
        })
    };
    let mut all = world.categories.all();
    all.retain(|found| found.id != VIEWS && found.parent != Some(VIEWS));
    all.sort_by_key(|found| {
        (
            found.parent.unwrap_or(found.id),
            found.parent.is_some(),
            found.id,
        )
    });
    let looks = all
        .into_iter()
        .filter_map(|found| {
            let projection = looked(found.id, Edge::Projected)?;
            Some(LookSummary {
                category: found.id.0,
                name: found.name,
                parent: found.parent.map(|parent| parent.0),
                cuttable: found.cuttable,
                annotation: matches!(found.kind, CategoryKind::Annotation | CategoryKind::View),
                cut: if found.cuttable {
                    looked(found.id, Edge::Cut)
                } else {
                    None
                },
                projection: Some(projection),
            })
        })
        .collect();
    let pens = world
        .lines
        .model_pens
        .pens()
        .iter()
        .filter_map(|pen| {
            let width = world.lines.model_pens.width(pen.number, scale).ok()?;
            Some(PenSummary {
                number: pen.number,
                name: pen.name.clone(),
                width: width.get(),
            })
        })
        .collect();
    let annotation_pens = world
        .lines
        .annotation_pens
        .pens()
        .iter()
        .map(|pen| PenSummary {
            number: pen.number,
            name: pen.name.clone(),
            width: pen.width.get(),
        })
        .collect();
    let line_patterns = world
        .lines
        .patterns
        .patterns()
        .iter()
        .map(|pattern| PatternSummary {
            id: pattern.id.0,
            name: pattern.name.clone(),
        })
        .collect();
    Some(GraphicsListing {
        view: summary(view, &world.levels),
        rules,
        looks,
        pens,
        annotation_pens,
        line_patterns,
        fill_patterns: construct_views::FILL_PATTERNS.to_vec(),
    })
}

/// How an element of `category` looks on `edge` in the view `id`, by the one rule
/// ([`construct_views::resolve`]): what a drawing of the view asks for each line and fill. A
/// subcategory is resolved as itself under its category.
///
/// # Errors
/// In words, when the document has no such view or the category cannot be resolved there (an
/// unknown one, or the cut of what cannot be cut).
pub fn look_in(
    world: &World,
    id: &Id,
    category: CategoryId,
    edge: Edge,
) -> Result<construct_views::Resolved, String> {
    let view = world
        .views
        .get(id)
        .ok_or_else(|| format!("There is no view {id}."))?;
    let parent = world
        .categories
        .get(category)
        .and_then(|found| found.parent);
    let (top, sub) = match parent {
        Some(parent) => (parent, Some(category)),
        None => (category, None),
    };
    construct_views::resolve(
        view,
        &world.categories,
        &world.objects,
        &world.lines,
        top,
        sub,
        edge,
    )
    .map_err(|e| e.to_string())
}

/// The part of a generic command the rules below read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    recipe: Option<Recipe>,
}

/// Whether a view's record is a section's or an elevation's, by its stored kind.
fn has_line(record: &Element) -> bool {
    View::from_record(record).is_ok_and(|view| view.kind().cut_line().is_some())
}

/// Whether a recipe is a view's.
fn a_view_recipe(recipe: &Recipe) -> bool {
    records::category_of(recipe).is_ok_and(|category| category == VIEWS)
}

/// The generic commands do not go behind the view commands: `element.create` of a view and
/// `element.set`, `element.move` or `element.delete` of one are refused, naming the command to
/// use. Anything else, and a command that does not read (or an element that is not there), is
/// left to the command, which answers it as it always has.
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
    let by_recipe = args.recipe.as_ref().is_some_and(a_view_recipe);
    let use_instead = |what: &str, command: &str| format!("{what}: use {command}.");
    Ok(match edit.cmd.as_str() {
        "element.create" if by_recipe || args.kind.as_deref() == Some(VIEW_KIND) => {
            Err(use_instead("A view is made", "view.create"))
        }
        "element.set" | "element.move" | "element.delete" => {
            let Some(id) = &args.element else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, &edit.target, id).await? {
                Some((_, record)) if record.kind == VIEW_KIND => Err(match edit.cmd.as_str() {
                    "element.delete" => use_instead("A view is deleted", "view.delete"),
                    "element.move" if has_line(&record) => use_instead(
                        "A section's or an elevation's line is moved",
                        "view.line.move",
                    ),
                    _ => use_instead(
                        "A view is changed",
                        "view.set, view.rename or view.duplicate",
                    ),
                }),
                Some(_) if by_recipe => Err(use_instead("A view is made", "view.create")),
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    })
}

/// Whether the views an undo wrote still stand in the document: each reads as a view and its
/// levels, phases and options are in the model. `whole`: a level, a type or the parameter table
/// was written or taken away, so every view of the document is checked.
///
/// # Errors
/// A store error; the inner error names the view and what it would lack.
pub async fn undo_rules<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    written: &[Element],
    whole: bool,
) -> Result<Result<(), String>, StoreError> {
    let world = match read_world(txn, project, document).await? {
        Ok(world) => world,
        Err(why) => return Ok(Err(why)),
    };
    let checked: Vec<&View> = if whole {
        world.views.ordered()
    } else {
        written
            .iter()
            .filter(|record| record.kind == VIEW_KIND)
            .filter_map(|record| world.views.get(&record.id))
            .collect()
    };
    for view in checked {
        if let Err(why) = world.check(view) {
            return Ok(Err(format!("{}: {why}", view.name())));
        }
    }
    Ok(Ok(()))
}
