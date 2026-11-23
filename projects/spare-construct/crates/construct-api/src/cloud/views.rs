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
//! | `view.delete` | `view` | Its pointer goes |
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
use construct_categories::{CategoryTable, VIEWS};
use construct_levels::{Level, Levels, parse_length};
use construct_model::element::Element;
use construct_model::geometry::{Bounds, Recipe};
use construct_params::{Length, Units, Value};
use construct_phases::{DesignOptions, ElementStaging, PhaseList, ViewSettings};
use construct_regen::levels::read_levels;
use construct_regen::parameters as records;
use construct_views::{
    CutLine, DetailLevel, Model, PlanSetup, Point, Scale, Side, Tie, VIEW_KIND, View, ViewElement,
    ViewKind, ViewRange, Views, list,
};
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, Reader, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::{Deserialize, Serialize};

/// Every command here.
pub const COMMANDS: [&str; 5] = [
    "view.create",
    "view.rename",
    "view.set",
    "view.duplicate",
    "view.delete",
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
}

/// The summary of one view.
#[must_use]
pub fn summary(view: &View, levels: &Levels) -> ViewSummary {
    let level = view.kind().plan().map(|plan| plan.level.clone());
    ViewSummary {
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
        return Err(format!("{}'s elevation {text:?} is not a length.", shaped.record.name));
    };
    let too_high = || format!("{} stands too high to be tested.", shaped.record.name);
    let per = Length::STEPS_PER_MM;
    let down = elevation.steps().div_euclid(per);
    let up = if elevation.steps().rem_euclid(per) == 0 {
        down
    } else {
        down.checked_add(1).ok_or_else(too_high)?
    };
    let [low, high] = [(bounds.min[2], down), (bounds.max[2], up)].map(|(at, by)| at.checked_add(by));
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
        let category = records::stored_category(record)
            .or_else(|| records::category_of(&record.recipe).ok());
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
    let mut run = Run {
        txn,
        project,
        document: &edit.target,
        edit,
        now_ms: context.now_ms,
        claim_ms,
    };
    match edit.cmd.as_str() {
        "view.create" => create(&mut run, args).await,
        "view.rename" => rename(&mut run, args).await,
        "view.set" => set(&mut run, args).await,
        "view.duplicate" => duplicate(&mut run, args).await,
        "view.delete" => delete(&mut run, args).await,
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
        .map(|copy| copy.with_phasing(view.phasing().cloned()))
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

/// The part of a generic command the rules below read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    recipe: Option<Recipe>,
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
